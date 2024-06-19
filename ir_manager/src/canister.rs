use crate::{api::execute_strategy, state::IrState};
use ic_canister::{generate_idl, init, query, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_cdk_timers::set_timer_interval, ic_kit::ic::spawn};
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    time::Duration,
};

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
    #[init]
    pub fn init(
        &mut self,
        rpc_principal: Principal,
        rpc_url: String,
        managers: Vec<String>,
        multi_trove_getter: String,
    ) {
        for manager in managers {
            // Clone the variables for each manager inside the loop to avoid them being moved in the first iteration.
            let rpc_principal_cloned = rpc_principal.clone();
            let rpc_url_cloned = rpc_url.clone();
            let manager_cloned = manager.clone();
            let multi_trove_getter_cloned = multi_trove_getter.clone();

            set_timer_interval(Duration::from_secs(3600), move || {
                // Now use the freshly cloned variables, which are unique to this iteration of the loop.
                let rpc_principal_per_manager = rpc_principal_cloned.clone();
                let rpc_url_per_manager = rpc_url_cloned.clone();
                let manager_cloned = manager_cloned.clone();
                let multi_trove_getter_per_manager = multi_trove_getter_cloned.clone();

                spawn(async move {
                    execute_strategy(
                        rpc_principal_per_manager,
                        rpc_url_per_manager,
                        manager_cloned,
                        multi_trove_getter_per_manager,
                    )
                    .await;
                });
            });
        }
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
