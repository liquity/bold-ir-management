use crate::{
    charger::{check_threshold, transfer_cketh},
    evm_rpc::Service,
    state::*,
    strategy::StrategyData,
    types::{
        DerivationPath, InitArgs, ManagerError, StrategyQueryData, SwapResponse,
    },
};
use alloy_primitives::U256;
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_cdk::caller, ic_kit::ic::time};
use std::{collections::HashMap, str::FromStr};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    // INITIALIZATION
    #[update]
    pub fn start(&mut self, init_args: InitArgs) {
        // Assigning init_args field values to variables
        let collateral_registry = init_args.collateral_registry;
        let multi_trove_getters = init_args.multi_trove_getters;
        let rpc_principal = init_args.rpc_principal;
        let strategies = init_args.strategies;
        let managers = init_args.managers;
        let rpc_url = init_args.rpc_url;

        // Creating variables that are needed in the computations
        let mut strategies_data: HashMap<u32, StrategyData> = HashMap::new();
        let strategies_count: usize = managers.len() * strategies.len();
        let keys: Vec<DerivationPath> = vec![];

        (0..strategies_count).map(|id| {
            let derivation_path = vec![id.to_be_bytes().to_vec()];
            let timestamp = time();

            let strategy = strategies[id % strategies.len()];
            let strategy_data = StrategyData {
                manager: managers[(id + 1) / strategies.len()].clone(),
                multi_trove_getter: multi_trove_getters[(id + 1) / strategies.len()].clone(),
                latest_rate: U256::from(0),
                derivation_path,
                target_min: U256::from(strategy.target_min),
                upfront_fee_period: U256::from(strategy.upfront_fee_period),
                eoa_nonce: 0,
                eoa_pk: None,
                last_update: timestamp,
                lock: false,
                rpc_canister: Service(rpc_principal.clone()),
                rpc_url: rpc_url.clone(),
                collateral_registry: collateral_registry.clone()
            };

            strategies_data.insert(id as u32, strategy_data);
        });
        
        MANAGERS.with(|managers_vector| *managers_vector.borrow_mut() = managers);
        STRATEGY_DATA.with(|data| *data.borrow_mut() = strategies_data);
    }

    #[query]
    pub fn get_strategies(&self) -> Vec<StrategyQueryData> {
        STRATEGY_DATA.with(|vector_data| {
            vector_data
                .borrow()
                .values()
                .map(|strategy| StrategyQueryData::from(strategy.clone()))
                .collect()
        })
    }

    #[update]
    pub async fn swap_cketh(&self) -> Result<SwapResponse, ManagerError> {
        // lock / unlock based on the current cycles balance of the canister
        check_threshold().await?;
        transfer_cketh(caller()).await
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
