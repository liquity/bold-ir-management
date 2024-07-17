use std::{str::FromStr, time::Duration};

use alloy_primitives::{Bytes, FixedBytes, U256};
use alloy_sol_types::SolCall;
use candid::Principal;
use ic_exports::{
    candid::Nat,
    ic_cdk::{
        api::{
            self,
            call::{self, msg_cycles_accept, msg_cycles_available},
            canister_balance, canister_balance128,
        },
        call,
    },
    ic_cdk_timers::set_timer,
    ic_kit::{
        ic::{self, id},
        CallResult,
    },
};
use icrc_ledger_types::icrc1::{
    account::Account,
    transfer::{TransferArg, TransferError},
};
use serde_json::json;

use crate::{
    evm_rpc::{RpcService, Service},
    signer::get_canister_public_key,
    state::{
        CKETH_HELPER, CKETH_LEDGER, CKETH_THRESHOLD, CYCLES_THRESHOLD, ETHER_RECHARGE_VALUE,
        RPC_CANISTER, RPC_URL, STRATEGY_DATA,
    },
    types::{depositCall, depositReturn, DerivationPath, ManagerError, StrategyData, SwapResponse},
    utils::{
        decode_request_response, decode_response, fetch_cketh_balance, fetch_ether_cycles_rate,
        rpc_provider, send_raw_transaction,
    },
};

pub async fn check_threshold() -> Result<(), ManagerError> {
    let threshold = CYCLES_THRESHOLD.get();
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
    let ether_value = ETHER_RECHARGE_VALUE.with(|ether_value| ether_value.borrow().clone());
    let cketh_helper: String = CKETH_HELPER.with(|cketh_helper| cketh_helper.borrow().clone());
    let rpc_canister: Service = RPC_CANISTER.with(|canister| canister.borrow().clone());
    let rpc_url: String = RPC_URL.with(|rpc| rpc.borrow().clone());
    let strategies: Vec<StrategyData> = STRATEGY_DATA
        .with(|strategies_hashmap| strategies_hashmap.borrow().clone().into_values().collect());

    let mut derivation_path: DerivationPath;
    let mut nonce: u64;
    for strategy in strategies {
        let balance = fetch_balance(&rpc_canister, &rpc_url, strategy.eoa_pk.unwrap()).await;
        if balance > ether_value {
            derivation_path = strategy.derivation_path;
            nonce = strategy.eoa_nonce;
        }
    }

    let encoded_canister_id: FixedBytes<32> =
        FixedBytes::<32>::from_str(&api::id().to_string()).unwrap();

    let deposit_call = depositCall {
        _principal: encoded_canister_id,
    };

    let transaction_data = deposit_call.abi_encode();

    // todo: fetch the cycles with estimation
    send_raw_transaction(
        cketh_helper,
        transaction_data,
        ether_value,
        nonce,
        derivation_path,
        &rpc_canister,
        &rpc_url,
        100000000,
    )
    .await
    .map(|_| Ok(()))
    .unwrap_or_else(|e| Err(e))
}

async fn fetch_balance(rpc_canister: &Service, rpc_url: &str, pk: String) -> U256 {
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

    let decoded_hex = decode_request_response(request_response).unwrap();
    let mut padded = [0u8; 32];
    let start = 32 - decoded_hex.len();
    padded[start..].copy_from_slice(&decoded_hex);

    U256::from_be_bytes(padded)
}

pub async fn transfer_cketh(receiver: Principal) -> Result<SwapResponse, ManagerError> {
    // todo: account for the fee
    let rate = fetch_ether_cycles_rate().await?;
    let attached_cycles = msg_cycles_available();
    let maximum_returned_ether_amount = attached_cycles * rate;

    // first check if the balance permits the max transfer amount
    let balance = fetch_cketh_balance().await?;
    // second calculate the amount to transfer and accept cycles first
    let (transfer_amount, cycles_to_accept) = if balance > maximum_returned_ether_amount {
        (maximum_returned_ether_amount, attached_cycles)
    } else {
        (balance, balance / rate)
    };
    msg_cycles_accept(cycles_to_accept);
    // third send the cketh to the user
    let ledger_principal = CKETH_LEDGER.with(|ledger| ledger.borrow().clone());

    let args = TransferArg {
        from_subaccount: None,
        to: receiver.into(),
        fee: todo!(),
        created_at_time: None,
        memo: None,
        amount: transfer_amount,
    };

    let call_response: CallResult<(Result<Nat, TransferError>,)> =
        call(ledger_principal, "icrc1_transfer", (args,)).await;

    match call_response {
        Ok(response) => Ok(SwapResponse {
            accepted_cycles: cycles_to_accept,
            returning_ether: transfer_amount,
        }),
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}
