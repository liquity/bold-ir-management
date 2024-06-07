use std::{cell::RefCell, rc::Rc};

use ic_canister::Canister;
use ic_exports::candid::{CandidType, Principal};
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;

#[derive(Default, CandidType, IcStorage, Deserialize)]
pub struct IrState {
    pub evm_rpc: Option<Principal>,
}

impl Versioned for IrState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}