use std::str::FromStr;

use alloy_primitives::{I256, U256};
use alloy_sol_types::SolCall;

use crate::state::{COLLATERAL_REGISTRY, MANAGERS, RPC_CANISTER, RPC_URL, STRATEGY_DATA};
use crate::types::*;
use crate::utils::{decode_response, eth_call_args};
use crate::{
    evm_rpc::{RpcService, Service},
    strategy::run_strategy,
    types::ManagerError,
    utils::rpc_provider,
};

pub async fn execute_strategy(id: u32) {
    let rpc_canister: Service = RPC_CANISTER.with(|canister| canister.borrow().clone());
    let rpc_url = RPC_URL.with(|rpc| rpc.borrow().clone());
    let collateral_registry = COLLATERAL_REGISTRY.with(|collateral_registry_address| collateral_registry_address.borrow().clone());
    let (manager, multi_trove_getter, latest_rate, target_min, upfront_fee_period) =
        STRATEGY_DATA.with(|strategy_data| {
            let binding = strategy_data.borrow();
            let borrowed_data = binding.get(&id).unwrap();
            (
                borrowed_data.manager.clone(),
                borrowed_data.multi_trove_getter.clone(),
                borrowed_data.latest_rate.clone(),
                borrowed_data.target_min.clone(),
                borrowed_data.upfront_fee_period.clone(),
            )
        });

    // Fetch data
    let entire_system_debt: U256 = fetch_entire_system_debt(&rpc_canister, &rpc_url, &manager)
        .await
        .unwrap()
        .entireSystemDebt;

    let unbacked_portion_price_and_redeemability =
        fetch_unbacked_portion_price_and_redeemablity(&rpc_canister, &rpc_url, &manager)
            .await
            .unwrap();

    let troves = fetch_multiple_sorted_troves(
        &rpc_canister,
        &rpc_url,
        &multi_trove_getter,
        U256::from_str("1000").unwrap(),
    )
    .await
    .unwrap()
    ._troves;

    // Calculate
    let redemption_fee = fetch_redemption_rate(&rpc_canister, &rpc_url, &collateral_registry)
        .await
        .unwrap()
        ._0;
    let redemption_split = unbacked_portion_price_and_redeemability._0
        / fetch_total_unbacked(&rpc_canister, &rpc_url, vec![&manager])
            .await
            .unwrap();
    let target_amount =
        redemption_split * entire_system_debt * ((redemption_fee * target_min) / U256::from(5))
            / U256::from(1000);
}

async fn fetch_entire_system_debt(
    rpc_canister: &Service,
    rpc_url: &str,
    liquity_base: &str,
) -> Result<getEntireSystemDebtReturn, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let json_data = eth_call_args(
        liquity_base.to_string(),
        getEntireSystemDebtCall::SELECTOR.to_vec(),
    );

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(rpc_canister_response)
        .map(|data| Ok(data))
        .unwrap_or_else(|e| Err(e))
}

async fn fetch_redemption_rate(
    rpc_canister: &Service,
    rpc_url: &str,
    collateral_registry: &str,
) -> Result<getRedemptionRateWithDecayReturn, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let json_data = eth_call_args(
        collateral_registry.to_string(),
        getRedemptionRateWithDecayCall::SELECTOR.to_vec(),
    );

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<getRedemptionRateWithDecayReturn, getRedemptionRateWithDecayCall>(
        rpc_canister_response,
    )
    .map(|data| Ok(data))
    .unwrap_or_else(|e| Err(e))
}

async fn fetch_unbacked_portion_price_and_redeemablity(
    rpc_canister: &Service,
    rpc_url: &str,
    manager: &str,
) -> Result<getUnbackedPortionPriceAndRedeemabilityReturn, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let json_data = eth_call_args(
        manager.to_string(),
        getUnbackedPortionPriceAndRedeemabilityCall::SELECTOR.to_vec(),
    );

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<
        getUnbackedPortionPriceAndRedeemabilityReturn,
        getUnbackedPortionPriceAndRedeemabilityCall,
    >(rpc_canister_response)
}

async fn fetch_multiple_sorted_troves(
    rpc_canister: &Service,
    rpc_url: &str,
    multi_trove_getter: &str,
    count: U256,
) -> Result<getMultipleSortedTrovesReturn, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let parameters = getMultipleSortedTrovesCall {
        _startIdx: I256::from_str("0").unwrap(),
        _count: count,
    };

    let json_data = eth_call_args(
        multi_trove_getter.to_string(),
        getMultipleSortedTrovesCall::abi_encode(&parameters),
    );

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<getMultipleSortedTrovesReturn, getMultipleSortedTrovesCall>(
        rpc_canister_response,
    )
}

/// Fetches the total unbacked amount across all collateral markets excluding the ones defined in the parameter.
pub async fn fetch_total_unbacked(
    rpc_canister: &Service,
    rpc_url: &str,
    excluded_managers: Vec<&str>,
) -> Result<U256, ManagerError> {
    let managers: Vec<String> = MANAGERS.with(|managers_vector| {
        let mut filtered_managers = managers_vector.borrow().clone(); // Clone the vector
        filtered_managers.retain(|x| !excluded_managers.contains(&x.as_str())); // Then retain elements
        filtered_managers // Return the filtered vector
    });

    let mut total_unbacked = U256::from(0);

    for manager in managers {
        total_unbacked +=
            fetch_unbacked_portion_price_and_redeemablity(rpc_canister, rpc_url, &manager)
                .await
                .unwrap()
                ._0;
    }

    Ok(total_unbacked)
}
