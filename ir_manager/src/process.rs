use alloy_primitives::{I256, U256};
use alloy_sol_types::SolCall;

use crate::{
    evm_rpc::{RpcService, Service},
    state::*,
    types::{
        getEntireSystemDebtCall, getEntireSystemDebtReturn, getMultipleSortedTrovesCall,
        getMultipleSortedTrovesReturn, getRedemptionRateWithDecayCall,
        getRedemptionRateWithDecayReturn, getUnbackedPortionPriceAndRedeemabilityCall,
        getUnbackedPortionPriceAndRedeemabilityReturn, CombinedTroveData, ManagerError,
        StrategyData,
    },
    utils::{decode_response, eth_call_args, rpc_provider},
};

pub struct LiquityProcess {
    pub rpc_canister: Service,
    pub rpc_url: String,
    pub manager: String,
    pub collateral_registry: String,
    pub multi_trove_getter: String,
}

impl LiquityProcess {
    pub fn new(strategy: &StrategyData) -> Self {
        let rpc_canister: Service = RPC_CANISTER.with(|canister| canister.borrow().clone());
        let rpc_url = RPC_URL.with(|rpc| rpc.borrow().clone());
        let collateral_registry = COLLATERAL_REGISTRY
            .with(|collateral_registry_address| collateral_registry_address.borrow().clone());
        Self {
            rpc_canister,
            rpc_url,
            collateral_registry,
            manager: strategy.manager,
            multi_trove_getter: strategy.multi_trove_getter,
        }
    }

    pub async fn fetch_entire_system_debt(&mut self) -> Result<U256, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let json_data = eth_call_args(
            self.manager.to_string(),
            getEntireSystemDebtCall::SELECTOR.to_vec(),
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(rpc_canister_response)
            .map(|data| Ok(data.entireSystemDebt))
            .unwrap_or_else(|e| Err(e))
    }

    pub async fn fetch_redemption_rate(&mut self) -> Result<U256, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let json_data = eth_call_args(
            self.collateral_registry.to_string(),
            getRedemptionRateWithDecayCall::SELECTOR.to_vec(),
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getRedemptionRateWithDecayReturn, getRedemptionRateWithDecayCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data._0))
        .unwrap_or_else(|e| Err(e))
    }

    pub async fn fetch_unbacked_portion_price_and_redeemablity(
        &mut self,
        manager: Option<String>
    ) -> Result<getUnbackedPortionPriceAndRedeemabilityReturn, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let call_manager = match manager {
            Some(value) => value,
            None => self.manager.clone()
        }

        let json_data = eth_call_args(
            call_manager,
            getUnbackedPortionPriceAndRedeemabilityCall::SELECTOR.to_vec(),
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<
            getUnbackedPortionPriceAndRedeemabilityReturn,
            getUnbackedPortionPriceAndRedeemabilityCall,
        >(rpc_canister_response)
    }

    pub async fn fetch_multiple_sorted_troves(
        &mut self,
        count: U256,
    ) -> Result<Vec<CombinedTroveData>, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let parameters = getMultipleSortedTrovesCall {
            _startIdx: I256::from_str("0").unwrap(),
            _count: count,
        };

        let json_data = eth_call_args(
            self.multi_trove_getter.to_string(),
            getMultipleSortedTrovesCall::abi_encode(&parameters),
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getMultipleSortedTrovesReturn, getMultipleSortedTrovesCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data._troves))
        .unwrap_or_else(|e| Err(e))
    }

    /// Fetches the total unbacked amount across all collateral markets excluding the ones defined in the parameter.
    pub async fn fetch_total_unbacked(
        &mut self,
        excluded_managers: Vec<&str>,
    ) -> Result<U256, ManagerError> {
        let managers: Vec<String> = MANAGERS.with(|managers_vector| {
            let mut filtered_managers = managers_vector.borrow().clone(); // Clone the vector
            filtered_managers.retain(|x| !excluded_managers.contains(&x.as_str())); // Then retain elements
            filtered_managers // Return the filtered vector
        });

        let mut total_unbacked = U256::from(0);

        for manager in managers {
            total_unbacked += self
                .fetch_unbacked_portion_price_and_redeemablity(Some(manager))
                .await
                .unwrap()
                ._0;
        }

        Ok(total_unbacked)
    }
}
