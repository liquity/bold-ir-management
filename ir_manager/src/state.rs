use std::{cell::{Cell, RefCell}, collections::HashMap};

use alloy_primitives::U256;
use ic_exports::candid::Principal;

use crate::{evm_rpc::Service, types::StrategyData};

thread_local! {
    pub static RPC_CANISTER: RefCell<Service> = RefCell::new(Service(Principal::anonymous()));
    pub static RPC_URL: RefCell<String> = RefCell::new("".to_string());
    pub static STRATEGY_DATA: RefCell<HashMap<u32, StrategyData>> = RefCell::new(HashMap::new());
    pub static COLLATERAL_REGISTRY: RefCell<String> = RefCell::new("".to_string());
    pub static MANAGERS: RefCell<Vec<String>> = RefCell::new(Vec::new());

    /// CONSTANTS
    pub static TOLERANCE_MARGIN_UP: Cell<U256> = Cell::new(U256::from(5));
    pub static TOLERANCE_MARGIN_DOWN: Cell<U256> = Cell::new(U256::from(5));
    pub static CYCLES_THRESHOLD: Cell<u64> = Cell::new(50_000_000_000); // Fifty billion cycles
    pub static CKETH_HELPER: RefCell<String> = RefCell::new("0x7574eB42cA208A4f6960ECCAfDF186D627dCC175".to_string());
    pub static CKETH_LEDGER: RefCell<Principal> = RefCell::new(Principal::from_text("ss2fx-dyaaa-aaaar-qacoq-cai").unwrap());

}
