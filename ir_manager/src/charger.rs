use std::str::FromStr;

use alloy_primitives::{FixedBytes, U256};
use alloy_sol_types::SolCall;
use candid::Principal;
use ic_exports::{
    candid::Nat,
    ic_cdk::{
        api::{
            self,
            call::{msg_cycles_accept, msg_cycles_available},
            canister_balance,
        },
        call,
    },
    ic_kit::CallResult,
};
use icrc_ledger_types::icrc1::transfer::{TransferArg, TransferError};
use num_traits::cast::ToPrimitive;
use serde_json::json;

use crate::{
    evm_rpc::{RpcService, Service},
    state::*,
    strategy::StrategyData,
    types::{depositCall, ManagerError, SwapResponse},
    utils::*,
};

pub async fn check_threshold() -> Result<(), ManagerError> {
    let threshold = CYCLES_THRESHOLD.with(|threshold| threshold.get());
    if canister_balance() <= threshold {
        return Ok(());
    }
    Err(ManagerError::CyclesBalanceAboveRechargingThreshold)
}

pub async fn recharge_cketh() -> Result<(), ManagerError> {
    let current_balance = fetch_cketh_balance().await?;
    let cketh_threshold = CKETH_THRESHOLD.with(|threshold| threshold.borrow().clone());
    if current_balance < cketh_threshold {
        // Deposit ether from one of the EOAs that has enough balance
        return ether_deposit().await;
    }
    Ok(())
}

async fn ether_deposit() -> Result<(), ManagerError> {
    let ether_value = ETHER_RECHARGE_VALUE.with(|ether_value| ether_value.get());
    let cketh_helper: String = CKETH_HELPER.with(|cketh_helper| cketh_helper.borrow().clone());
    let mut strategies: Vec<StrategyData> = STRATEGY_DATA
        .with(|strategies_hashmap| strategies_hashmap.borrow().clone().into_values().collect());

    let turn = CKETH_EOA_TURN_COUNTER.with(|counter| counter.get());

    strategies.rotate_left(turn as usize);

    for (index, strategy) in strategies.iter().enumerate() {
        let eoa = match strategy.eoa_pk {
            Some(pk) => pk,
            None => continue, // Skip if eoa_pk is None
        };

        let balance =
            match fetch_balance(&strategy.rpc_canister, &strategy.rpc_url, eoa.to_string()).await {
                Ok(balance) => balance,
                Err(_) => continue, // Skip on error
            };

        if balance > ether_value {
            let encoded_canister_id: FixedBytes<32> =
                match FixedBytes::<32>::from_str(&api::id().to_string()) {
                    Ok(id) => id,
                    Err(err) => return Err(ManagerError::Custom(format!("{:#?}", err))),
                };

            let deposit_call = depositCall {
                _principal: encoded_canister_id,
            };

            let transaction_data = deposit_call.abi_encode();

            // Update turn counter
            let new_counter = (index as u8 + turn + 1) % strategies.len() as u8;
            CKETH_EOA_TURN_COUNTER.with(|counter| counter.set(new_counter));

            // Fetch the cycles with estimation and send transaction
            return send_raw_transaction(
                cketh_helper,
                transaction_data,
                ether_value,
                strategy.eoa_nonce,
                strategy.derivation_path.clone(),
                &strategy.rpc_canister,
                &strategy.rpc_url,
                100000000,
            )
            .await
            .map(|_| Ok(()))?;
        }
    }

    Err(ManagerError::Custom(
        "No EOA had enough balance.".to_string(),
    ))
}

async fn fetch_balance(
    rpc_canister: &Service,
    rpc_url: &str,
    pk: String,
) -> Result<U256, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);
    let json_args = json!({
        "id": 1,
        "jsonrpc": "2.0",
        "params": [
            pk,
            "latest"
        ],
        "method": "eth_getBalance"
    })
    .to_string();
    let request_response = rpc_canister.request(rpc, json_args, 50000, 10000000).await;

    let decoded_hex = decode_request_response(request_response)?;
    let mut padded = [0u8; 32];
    let start = 32 - decoded_hex.len();
    padded[start..].copy_from_slice(&decoded_hex);

    Ok(U256::from_be_bytes(padded))
}

pub async fn transfer_cketh(receiver: Principal) -> Result<SwapResponse, ManagerError> {
    // todo: account for the fee
    let discount_percentage = CYCLES_DISCOUNT_PERCENTAGE.with(|percentage| percentage.get());
    let rate = fetch_ether_cycles_rate().await? * discount_percentage;
    let attached_cycles = msg_cycles_available();
    let maximum_returned_ether_amount = Nat::from(attached_cycles * rate);

    // first check if the balance permits the max transfer amount
    let balance = fetch_cketh_balance().await?;
    // second calculate the amount to transfer and accept cycles first
    let (transfer_amount, cycles_to_accept) = if balance > maximum_returned_ether_amount {
        (maximum_returned_ether_amount, attached_cycles)
    } else {
        let cycles_to_accept = (balance.clone() / rate).0.to_u64().ok_or_else(|| {
            ManagerError::DecodingError(
                "Error while decoding the amount of cycles to accept to u64".to_string(),
            )
        })?;
        (balance, cycles_to_accept)
    };
    msg_cycles_accept(cycles_to_accept);
    // third send the cketh to the user
    let ledger_principal = CKETH_LEDGER.with(|ledger| ledger.get());

    let args = TransferArg {
        from_subaccount: None,
        to: receiver.into(),
        fee: Some(CKETH_FEE.with(|fee| fee.borrow().clone())),
        created_at_time: None,
        memo: None,
        amount: transfer_amount.clone(),
    };

    let call_response: CallResult<(Result<Nat, TransferError>,)> =
        call(ledger_principal, "icrc1_transfer", (args,)).await;

    match call_response {
        Ok(_) => Ok(SwapResponse {
            accepted_cycles: Nat::from(cycles_to_accept),
            returning_ether: transfer_amount,
        }),
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}
