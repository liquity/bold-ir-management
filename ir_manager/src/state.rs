use std::str::FromStr;

use alloy_primitives::Address;
use ic_exports::candid::{CandidType, Principal};
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;

#[derive(Default, CandidType, IcStorage, Deserialize)]
pub struct IrState {
    pub rpc_canister: Option<Principal>,
    pub weth_manager: String,
    pub reth_manager: String,
    pub wsteth_manager: String,
}

impl IrState {
    pub fn weth_manager_address(&self) -> Address {
        Address::from_str(&self.weth_manager).unwrap()
    }

    pub fn reth_manager_address(&self) -> Address {
        Address::from_str(&self.reth_manager).unwrap()
    }

    pub fn wsteth_manager_address(&self) -> Address {
        Address::from_str(&self.wsteth_manager).unwrap()
    }
}

impl Versioned for IrState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}
