use std::{cell::RefCell, collections::HashMap};

use ic_exports::candid::Principal;

use crate::{evm_rpc::Service, types::StrategyData};

thread_local! {
    pub static RPC_CANISTER: RefCell<Service> = RefCell::new(Service(Principal::anonymous()));
    pub static RPC_URL: RefCell<String> = RefCell::new("".to_string());
    pub static STRATEGY_DATA: RefCell<HashMap<u32, StrategyData>> = RefCell::new(HashMap::new());
    pub static COLLATERAL_REGISTRY: RefCell<String> = RefCell::new("".to_string());
    pub static MANAGERS: RefCell<Vec<String>> = RefCell::new(Vec::new());
}
