use crate::{
    api::execute_strategy,
    evm_rpc::Service,
    state::*,
    types::{DerivationPath, StrategyData},
};
use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_cdk_timers::set_timer_interval, ic_kit::ic::spawn};
use std::{collections::HashMap, time::Duration};

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
        multi_trove_getter: String,
    ) {
        RPC_CANISTER.with(|rpc_canister| *rpc_canister.borrow_mut() = Service(rpc_principal));
        RPC_URL.with(|rpc| *rpc.borrow_mut() = rpc_url);
        MANAGERS.with(|managers_vector| *managers_vector.borrow_mut() = managers);

        // generating keys
        let strategies_data = HashMap::<u32, StrategyData>::new();
        let keys: Vec<DerivationPath> = vec![];
        let strategies_count = managers.len() * 3;
        for id in 0..strategies_count {
            let t_min = match id % 3 {
                0 => 5,
                1 => 10,
                2 => 20,
            };
            let strategy_data = StrategyData {
                manager: managers[id / 3],
                latest_rate: 0,
                derivation_path: vec![id.to_be_bytes().to_vec()],
                target_min: t_min,
                upfront_fee_period: t_min / 5,
            };

            strategies_data.insert(id as u32, strategy_data);

            set_timer_interval(Duration::from_secs(3600), || {
                spawn(async {
                    execute_strategy(id as u32).await;
                });
            });
        }

        STRATEGY_DATA.with(|data| *data.borrow_mut() = strategies_data);
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
