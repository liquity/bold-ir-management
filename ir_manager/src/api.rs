use alloy::primitives::U256;
use alloy::sol;
use alloy::sol_types::SolCall;
use ic_exports::candid::Principal;

use crate::utils::{decode_response, eth_call_args};
use crate::{
    evm_rpc::{RpcService, Service},
    strategy::run_strategy,
    types::ManagerError,
    utils::rpc_provider,
};

sol!(
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);
);

pub async fn execute_strategy(
    rpc_principal: Principal,
    rpc_url: String,
    liquity_base: String,
    manager: String,
) {
    let rpc_canister_instance: Service = Service(rpc_principal);

    // Fetch data
    let entire_system_debt: U256 =
        fetch_entire_system_debt(&rpc_canister_instance, &rpc_url, liquity_base)
            .await
            .unwrap();
    let unbacked_portion_price_and_redeemability =
        fetch_unbacked_portion_price_and_redeemablity(&rpc_canister_instance, &rpc_url, manager)
            .await
            .unwrap();
}

async fn fetch_entire_system_debt(
    rpc_canister: &Service,
    rpc_url: &str,
    liquity_base: String,
) -> Result<U256, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    sol!(
        function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
    );

    let json_data = eth_call_args(liquity_base, getEntireSystemDebtCall::SELECTOR.to_vec());

    let rpc_canister_response = rpc_canister
        .request(rpc, json_data, 500000, 10_000_000_000)
        .await;

    decode_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(rpc_canister_response)
        .map(|data| Ok(data.entireSystemDebt))
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
