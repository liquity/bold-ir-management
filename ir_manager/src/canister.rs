use crate::{
    api::execute_strategy,
    evm_rpc::Service,
    state::*,
    types::{DerivationPath, StrategyData, StrategyInput},
};
use alloy_primitives::U256;
use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_cdk_timers::set_timer_interval, ic_kit::ic::spawn};
use std::{collections::HashMap, str::FromStr, time::Duration};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    // INITIALIZATION
    #[init]
    pub fn init(
        &mut self,
        rpc_principal: Principal,
        rpc_url: String,
        managers: Vec<String>,
        multi_trove_getters: Vec<String>,
        collateral_registry: String,
        strategies: Vec<StrategyInput>
    ) {
        // generating keys
        let mut strategies_data = HashMap::<u32, StrategyData>::new();
        let keys: Vec<DerivationPath> = vec![];
        let strategies_count = managers.len() * strategies.len();
        for id in 0..strategies_count {
            let strategy = strategies[id % strategies.len()];
            let strategy_data = StrategyData {
                manager: managers[(id + 1) / strategies.len()].clone(),
                multi_trove_getter: multi_trove_getters[(id + 1) / strategies.len()].clone(),
                latest_rate: U256::from(0),
                derivation_path: vec![id.to_be_bytes().to_vec()],
                target_min: U256::from_str(&strategy.target_min).unwrap(),
                upfront_fee_period: U256::from_str(&strategy.upfront_fee_period).unwrap(),
                eoa_nonce: U256::from(0)
            };

            strategies_data.insert(id as u32, strategy_data);

            set_timer_interval(Duration::from_secs(3600), move || {
                spawn(async move {
                    execute_strategy(id as u32).await;
                });
            });
        }

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
