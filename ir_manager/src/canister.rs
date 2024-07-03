use crate::{
    evm_rpc::Service,
    state::*,
    types::{DerivationPath, InitArgs, StrategyData},
};
use alloy_primitives::U256;
use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_kit::ic::time};
use std::{collections::HashMap, str::FromStr};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    // INITIALIZATION
    #[init]
    pub fn init(&mut self, init_args: InitArgs) {
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
                target_min: U256::from_str(&strategy.target_min).unwrap(),
                upfront_fee_period: U256::from_str(&strategy.upfront_fee_period).unwrap(),
                eoa_nonce: 0,
                eoa_pk: None,
                last_update: timestamp,
            };

            strategies_data.insert(id as u32, strategy_data);
        });

        RPC_CANISTER.with(|rpc_canister| *rpc_canister.borrow_mut() = Service(rpc_principal));
        RPC_URL.with(|rpc| *rpc.borrow_mut() = rpc_url);
        COLLATERAL_REGISTRY.with(|collateral_registry_address| {
            *collateral_registry_address.borrow_mut() = collateral_registry
        });
        MANAGERS.with(|managers_vector| *managers_vector.borrow_mut() = managers);
        STRATEGY_DATA.with(|data| *data.borrow_mut() = strategies_data);
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
