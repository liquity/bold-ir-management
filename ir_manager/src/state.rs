use std::str::FromStr;

use ic_exports::candid::{CandidType, Principal};
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;

#[derive(Default, CandidType, IcStorage, Deserialize)]
pub struct IrState {
    pub rpc_canister: Option<Principal>,
    pub managers: Vec<String>,
    pub liquity_base: String,
    pub rpc_url: String,
}

impl Versioned for IrState {
    type Previous = ();

    fn upgrade((): ()) -> Self {
        Self::default()
    }
}
