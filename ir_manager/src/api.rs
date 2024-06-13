use alloy::primitives::U256;
use alloy::sol_types::SolCall;
use alloy::{primitives::keccak256, sol};
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_cdk_timers::set_timer_interval, ic_kit::ic::spawn};
use serde_json::json;
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    time::Duration,
};

use crate::utils::{decode_response, eth_call_args};
use crate::{
    evm_rpc::{RpcService, Service},
    state::IrState,
    strategy::run_strategy,
    types::ManagerError,
    utils::{rpc_canister, rpc_provider},
};

sol!(
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);
);

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,

    #[state]
    pub state: Rc<RefCell<IrState>>,
}

impl PreUpdate for IrManager {}

impl IrManager {
    // STATE FUNCTIONS
    fn state(&self) -> Ref<IrState> {
        RefCell::borrow(&self.state)
    }

    fn mut_state(&mut self) -> RefMut<IrState> {
        RefCell::borrow_mut(&self.state)
    }

    // INITIALIZATION
    pub fn init(
        &mut self,
        rpc_principal: Principal,
        rpc_url: String,
        liquity_base: String,
        managers: Vec<String>,
    ) {
        self.mut_state().rpc_canister = Some(rpc_principal);
        self.mut_state().rpc_url = rpc_url;
        self.mut_state().liquity_base = liquity_base;
        self.mut_state().managers = managers.clone();
    
        for manager in managers {
            let state_clone = Rc::clone(&self.state);
            set_timer_interval(Duration::from_secs(3600), move || {
                let state_clone_inner = Rc::clone(&state_clone);
                spawn(async move {
                    let mut state = state_clone_inner.borrow_mut();
                    state.execute_strategy(manager.clone()).await;
                });
            });
        }
    }

    async fn execute_strategy(&mut self, manager: String) {
        let rpc_canister_instance: Service = rpc_canister(self.state().rpc_canister).unwrap();
        let rpc: RpcService = rpc_provider(&self.state().rpc_url);

        // Fetch data
        let entire_system_debt: U256 = self
            .fetch_entire_system_debt(rpc_canister_instance, rpc)
            .await
            .unwrap();
    }

    async fn fetch_entire_system_debt(
        &mut self,
        rpc_canister: Service,
        rpc: RpcService,
    ) -> Result<U256, ManagerError> {
        let liquity_base = self.state().liquity_base.clone();

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
        &mut self,
        rpc_canister: Service,
        rpc: RpcService,
        manager: String,
    ) -> Result<getUnbackedPortionPriceAndRedeemabilityReturn, ManagerError> {

        let json_data = eth_call_args(manager, getUnbackedPortionPriceAndRedeemabilityCall::SELECTOR.to_vec());

        let rpc_canister_response = rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getUnbackedPortionPriceAndRedeemabilityReturn, getUnbackedPortionPriceAndRedeemabilityCall>(rpc_canister_response)
    }

    // QUERY FUNCTIONS
    #[query]
    pub fn get_rpc_canister(&self) -> Option<Principal> {
        self.state().rpc_canister
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
