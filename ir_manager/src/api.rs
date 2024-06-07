use std::{cell::{Ref, RefCell, RefMut}, rc::Rc};

use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
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

    fn state(&self) -> Ref<IrState> {
        RefCell::borrow(&self.state)
    }

    fn mut_state(&mut self) -> RefMut<IrState> {
        RefCell::borrow_mut(&self.state)
    }

    #[init]
    pub fn init(&mut self, evm_rpc: Principal) {
        self.mut_state().evm_rpc = Some(evm_rpc);
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
