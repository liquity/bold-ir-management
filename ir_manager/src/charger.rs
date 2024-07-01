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
    signer::get_canister_public_key,
    state::{
        CKETH_HELPER, CKETH_LEDGER, ETHER_RECHARGE_VALUE, RPC_CANISTER, RPC_URL, STRATEGY_DATA,
    },
    types::{depositCall, depositReturn, DerivationPath, ManagerError, StrategyData},
    utils::{rpc_provider, send_raw_transaction},
};

pub async fn recharge() {
    // The canister cycles balance has fallen below threshold

    // Deposit ether from one of the EOAs that has enough balance
    ether_deposit().await;

    // Set a one-off timer for the next 20 minutes (the time cketh takes to load balance on the ic side)
    set_timer(Duration::from_secs(1200), || {
        ic::spawn(async {
            let _ = resume_recharging().await;
        })
    });
    // Burn cketh for cycles and recharge
}

async fn resume_recharging() {
    let cketh_ledger = CKETH_LEDGER.with(|ledger| ledger.borrow().clone());
    let account = Account {
        owner: id(),
        subaccount: None,
    };

    let (balance,): (Nat,) = call(cketh_ledger, "icrc1_balance_of", (account,))
        .await
        .unwrap();

    // Todo: check if the balance matches the deposit value minus fee
}

async fn ether_deposit() -> Result<(), ManagerError> {
    let ether_value = ETHER_RECHARGE_VALUE.with(|ether_value| ether_value.borrow().clone());
    let cketh_helper: String = CKETH_HELPER.with(|cketh_helper| cketh_helper.borrow().clone());
    let rpc_canister: Service = RPC_CANISTER.with(|canister| canister.borrow().clone());
    let rpc_url: String = RPC_URL.with(|rpc| rpc.borrow().clone());
    let strategies: Vec<StrategyData> = STRATEGY_DATA
        .with(|strategies_hashmap| strategies_hashmap.borrow().clone().into_values().collect());
    
    let mut derivation_path: DerivationPath;
    let mut nonce : U256;
    for strategy in strategies {
        let balance = fetch_balance(strategy.eoa_pk.unwrap());
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
    let submission_result = send_raw_transaction(
        cketh_helper,
        transaction_data,
        ether_value,
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
