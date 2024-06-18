use std::str::FromStr;

use alloy_primitives::{I256, U256};
use alloy_sol_types::{sol, SolCall};
use ic_exports::candid::Principal;

use crate::utils::{decode_response, eth_call_args};
use crate::{
    evm_rpc::{RpcService, Service},
    strategy::run_strategy,
    types::ManagerError,
    utils::rpc_provider,
};

sol!(
    struct CombinedTroveData {
        uint256 id;
        uint256 debt;
        uint256 coll;
        uint256 stake;
        uint256 snapshotETH;
        uint256 snapshotBoldDebt;
    }
    function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);
    function getMultipleSortedTroves(int256 _startIdx, uint256 _count) external view returns (CombinedTroveData[] memory _troves);

);

pub async fn execute_strategy(
    rpc_principal: Principal,
    rpc_url: String,
    liquity_base: String,
    manager: String,
    multi_trove_getter: String,
) {
    let rpc_canister_instance: Service = Service(rpc_principal);

    // Fetch data
    let entire_system_debt: U256 =
        fetch_entire_system_debt(&rpc_canister_instance, &rpc_url, liquity_base)
            .await
            .unwrap()
            .entireSystemDebt;
    let unbacked_portion_price_and_redeemability =
        fetch_unbacked_portion_price_and_redeemablity(&rpc_canister_instance, &rpc_url, manager)
            .await
            .unwrap();
    let troves = fetch_multiple_sorted_troves(&rpc_canister_instance, &rpc_url, multi_trove_getter, U256::from_str("1000").unwrap()).await.unwrap();
}

async fn fetch_entire_system_debt(
    rpc_canister: &Service,
    rpc_url: &str,
    liquity_base: String,
) -> Result<getEntireSystemDebtReturn, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let json_data = eth_call_args(liquity_base, getEntireSystemDebtCall::SELECTOR.to_vec());

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(rpc_canister_response)
        .map(|data| Ok(data))
        .unwrap_or_else(|e| Err(e))
}

async fn fetch_unbacked_portion_price_and_redeemablity(
    rpc_canister: &Service,
    rpc_url: &str,
    manager: String,
) -> Result<getUnbackedPortionPriceAndRedeemabilityReturn, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let json_data = eth_call_args(
        manager,
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
    multi_trove_getter: String,
    count: U256
) -> Result<getMultipleSortedTrovesReturn, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let parameters = getMultipleSortedTrovesCall {
        _startIdx: I256::from_str("0").unwrap(),
        _count: count,
    };

    let json_data = eth_call_args(
        multi_trove_getter,
        getMultipleSortedTrovesCall::abi_encode(&parameters),
    );

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<
        getMultipleSortedTrovesReturn,
        getMultipleSortedTrovesCall,
    >(rpc_canister_response)
}
