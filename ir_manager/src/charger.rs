//! Responsible for:
//! - facilitating the ckETH<>Cycles arbitrage opportunity
//! - minting ckETH from ETH.

use std::str::FromStr;

use crate::{
    constants::{
        cketh_fee, cketh_ledger, cketh_threshold, ether_recharge_value, CKETH_HELPER,
        CYCLES_DISCOUNT_PERCENTAGE, CYCLES_THRESHOLD, SCALE,
    },
    utils::{
        common::{
            extract_call_result, fetch_cketh_balance, fetch_ether_cycles_rate, get_rpc_service,
        },
        error::*,
        evm_rpc::Service,
        transaction_builder::TransactionBuilder,
    },
};
use crate::{
    state::*,
    strategy::executable::ExecutableStrategy,
    types::{depositCall, SwapResponse},
};
use alloy_primitives::{FixedBytes, U256};
use alloy_sol_types::SolCall;
use candid::Principal;
use ic_exports::ic_cdk::{
    api::{
        self,
        call::{msg_cycles_accept, msg_cycles_available},
        canister_balance,
    },
    call,
};
use ic_exports::{candid::Nat, ic_kit::CallResult};
use icrc_ledger_types::icrc1::transfer::{TransferArg, TransferError};
use num_traits::ToPrimitive;
use serde_json::json;

pub async fn check_threshold() -> ManagerResult<()> {
    let threshold = CYCLES_THRESHOLD;
    if canister_balance() <= threshold {
        return Ok(());
    }
    Err(ManagerError::CyclesBalanceAboveRechargingThreshold)
}

pub async fn recharge_cketh() -> ManagerResult<()> {
    let current_balance = fetch_cketh_balance().await?;
    let cketh_threshold = cketh_threshold();
    if current_balance < cketh_threshold {
        // Deposit ether from one of the EOAs that has enough balance
        return ether_deposit().await;
    }
    Ok(())
}

async fn ether_deposit() -> ManagerResult<()> {
    let ether_value = ether_recharge_value();
    let cketh_helper: String = CKETH_HELPER.to_string();
    let mut strategies: Vec<ExecutableStrategy> = STRATEGY_STATE
        .with(|strategies_hashmap| strategies_hashmap.borrow().clone().into_values().collect());

    let turn = CKETH_EOA_TURN_COUNTER.with(|counter| counter.get());

    strategies.rotate_left(turn as usize);

    for (index, strategy) in strategies.iter().enumerate() {
        let eoa = match strategy.settings.eoa_pk {
            Some(pk) => pk,
            None => continue, // Skip if eoa_pk is None
        };

        let balance = match fetch_balance(&strategy.settings.rpc_canister, eoa.to_string()).await {
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
            let eoa = strategy
                .settings
                .eoa_pk
                .ok_or(ManagerError::NonExistentValue)?
                .to_string();
            return TransactionBuilder::default()
                .to(cketh_helper)
                .from(eoa)
                .data(transaction_data)
                .value(ether_value)
                .nonce(strategy.data.eoa_nonce)
                .derivation_path(strategy.settings.derivation_path.clone())
                .cycles(10_000_000_000)
                .send(&strategy.settings.rpc_canister)
                .await
                .map(|_| Ok(()))?;
        }
    }

    Err(ManagerError::Custom(
        "No EOA had enough balance.".to_string(),
    ))
}

async fn fetch_balance(rpc_canister: &Service, pk: String) -> ManagerResult<U256> {
    let rpc = get_rpc_service();
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

    let call_result = rpc_canister.request(rpc, json_args, 50000, 10000000).await;
    let canister_response = extract_call_result(call_result)?;
    let hex = canister_response.map_err(ManagerError::RpcResponseError)?;

    let mut padded = [0u8; 32];
    let start = 32 - hex.len();
    padded[start..].copy_from_slice(hex.as_bytes());

    Ok(U256::from_be_bytes(padded))
}

pub async fn transfer_cketh(receiver: Principal) -> ManagerResult<SwapResponse> {
    let discount_percentage = CYCLES_DISCOUNT_PERCENTAGE;
    let rate = fetch_ether_cycles_rate().await? * discount_percentage / 100;
    if rate == 0 {
        return Err(arithmetic_err("The calculated ETH/CXDR rate is zero."));
    }
    let attached_cycles = msg_cycles_available() as u128;
    let maximum_returned_ether_amount = Nat::from(
        attached_cycles
            .saturating_mul(rate as u128)
            .saturating_mul(SCALE),
    ); // SCALE here is the decimals ckETH tokens have (10^18)

    // first check if the balance permits the max transfer amount
    let cketh_balance = fetch_cketh_balance().await?;
    // second calculate the amount to transfer and accept cycles first
    let (transfer_amount, cycles_to_accept) = if cketh_balance > maximum_returned_ether_amount {
        (maximum_returned_ether_amount, attached_cycles)
    } else {
        let cycles_to_accept = (cketh_balance.clone() / SCALE / rate)
            .0
            .to_u64()
            .ok_or_else(|| {
                ManagerError::DecodingError(
                    "Error while decoding the amount of cycles to accept to u64".to_string(),
                )
            })?;
        (cketh_balance, cycles_to_accept as u128)
    };

    msg_cycles_accept(cycles_to_accept as u64); // we are not worried about casting like this as `msg_cycles_available()` had returned a u64 before

    // third send the cketh to the user
    let ledger_principal = cketh_ledger();

    let args = TransferArg {
        from_subaccount: None,
        to: receiver.into(),
        fee: Some(cketh_fee()),
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

#[derive(Default)]
pub struct SwapLock(bool);

impl SwapLock {
    fn apply(&mut self) {
        SWAP_LOCK.with(|lock| lock.set(self.0));
    }

    pub fn lock(&mut self) -> ManagerResult<()> {
        if self.0 || SWAP_LOCK.with(|lock| lock.get()) {
            return Err(ManagerError::Locked);
        }
        self.0 = true;
        self.apply();
        Ok(())
    }

    pub fn unlock(&mut self) {
        self.0 = false;
        self.apply();
    }
}

impl Drop for SwapLock {
    fn drop(&mut self) {
        self.unlock();
    }
}
