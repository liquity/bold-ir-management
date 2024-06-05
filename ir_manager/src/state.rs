use std::{cell::RefCell, rc::Rc};

use ic_canister::Canister;
use ic_exports::candid::{Principal, CandidType};
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;

#[derive(Default, CandidType, IcStorage, Deserialize)]
pub struct IrState {
    
}

impl Versioned for IrState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,

    #[state]
    pub state: Rc<RefCell<IrState>>
}