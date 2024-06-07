use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
};

use ic_canister::{generate_idl, init, query, Canister, Idl, PreUpdate};
use ic_exports::candid::Principal;

use crate::state::IrState;

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
        rpc_canister: Principal,
        weth_manager: String,
        reth_manager: String,
        wsteth_manager: String,
    ) {
        self.mut_state().rpc_canister = Some(rpc_canister);
        self.mut_state().weth_manager = weth_manager;
        self.mut_state().reth_manager = reth_manager;
        self.mut_state().wsteth_manager = wsteth_manager;

        // Timers need to start here
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
