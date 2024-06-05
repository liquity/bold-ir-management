use ic_canister::{generate_idl, post_upgrade, pre_upgrade, Canister, Idl, PreUpdate};
use ic_exports::candid::Principal;

use crate::state::IrManager;

impl PreUpdate for IrManager {}

impl IrManager {
    pub fn idl() -> Idl {
        generate_idl!()
    }
}
