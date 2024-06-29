use std::{str::FromStr, time::Duration};

use alloy_primitives::{Bytes, FixedBytes, U256};
use alloy_sol_types::SolCall;
use ic_exports::{
    candid::Nat,
    ic_cdk::{
        api::{self, call},
        call,
    },
    ic_cdk_timers::set_timer,
    ic_kit::ic::{self, id},
};
use icrc_ledger_types::icrc1::account::Account;

use crate::{
    evm_rpc::{RpcService, Service},
    state::{CKETH_HELPER, CKETH_LEDGER, RPC_CANISTER, RPC_URL},
    types::{depositCall, depositReturn, DerivationPath, ManagerError},
    utils::{rpc_provider, send_raw_transaction},
};

pub async fn recharge() {
    // The canister cycles balance has fallen below threshold

    // Deposit ether from one of the EOAs that has enough balance
    ether_deposit(value, nonce, derivation_path).await;

    // Set a one-off timer for the next 20 minutes (the time cketh takes to load balance on the ic side)
    set_timer(
        Duration::from_secs(1200),
        ic::spawn(resume_recharging().await),
    ); // 20 minutes
       // Burn cketh for cycles and recharge
}

async fn resume_recharging() {
    let cketh_ledger = CKETH_LEDGER.with(|ledger| ledger.borrow().clone());
    let account = Account {
        owner: id(),
        subaccount: None,
    };

    let (balance,) : (Nat, ) = call(cketh_ledger, "icrc1_balance_of", (account,)).await.unwrap();
    
    // Todo: check if the balance matches the deposit value minus fee

    
}

async fn ether_deposit(
    value: U256,
    nonce: u64,
    derivation_path: DerivationPath,
) -> Result<(), ManagerError> {
    let cketh_helper: String = CKETH_HELPER.with(|cketh_helper| cketh_helper.borrow().clone());
    let rpc_canister: Service = RPC_CANISTER.with(|canister| canister.borrow().clone());
    let rpc_url: String = RPC_URL.with(|rpc| rpc.borrow().clone());

    let encoded_canister_id: FixedBytes<32> =
        FixedBytes::<32>::from_str(&api::id().to_string()).unwrap();

    let deposit_call = depositCall {
        _principal: encoded_canister_id,
    };

    let transaction_data = deposit_call.abi_encode();

    // todo: fetch the cycles with estimation
    let submission_result = send_raw_transaction(
        cketh_helper,
        transaction_data,
        value,
        nonce,
        derivation_path,
        &rpc_canister,
        &rpc_url,
        100000000,
    )
    .await;

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<depositReturn, depositCall>(rpc_canister_response)
        .map(|data| Ok(data))
        .unwrap_or_else(|e| Err(e))
}
