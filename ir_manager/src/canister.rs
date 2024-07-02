use crate::{
    api::execute_strategy,
    evm_rpc::Service,
    signer::{get_canister_public_key, pubkey_bytes_to_address},
    state::*,
    types::{DerivationPath, StrategyData, StrategyInput},
    utils::set_public_keys,
};
use alloy_primitives::U256;
use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
use ic_exports::{
    candid::Principal,
    ic_cdk::api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
    ic_cdk_timers::{set_timer, set_timer_interval},
    ic_kit::ic::{spawn, time},
};
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
        strategies: Vec<StrategyInput>,
    ) {
        // generating keys
        let mut strategies_data = HashMap::<u32, StrategyData>::new();
        let keys: Vec<DerivationPath> = vec![];
        let strategies_count = managers.len() * strategies.len();

        let timestamp = time();
        for id in 0..strategies_count {
            let derivation_path = vec![id.to_be_bytes().to_vec()];

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
        }

        RPC_CANISTER.with(|rpc_canister| *rpc_canister.borrow_mut() = Service(rpc_principal));
        RPC_URL.with(|rpc| *rpc.borrow_mut() = rpc_url);
        COLLATERAL_REGISTRY.with(|collateral_registry_address| {
            *collateral_registry_address.borrow_mut() = collateral_registry
        });
        MANAGERS.with(|managers_vector| *managers_vector.borrow_mut() = managers);
        STRATEGY_DATA.with(|data| *data.borrow_mut() = strategies_data);
    }

    fn start_timers() {
        // assign public keys to the different strategy EOAs
        set_timer(Duration::from_secs(1), || spawn(set_public_keys()));

        // assign a separate timer for each strategy
        let strategies : Vec<(u32, StrategyData)> = STRATEGY_DATA.with(|vector_data| vector_data.borrow().clone()).into_iter().collect();

        for (key, strategy) in strategies {
            set_timer_interval(Duration::from_secs(3600), move || {
                spawn(
                    execute_strategy(key, &strategy)
                );
            });
        }
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
