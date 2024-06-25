use std::{cell::{Cell, RefCell}, collections::HashMap, str::FromStr};

use alloy_primitives::U256;
use ic_exports::{candid::{CandidType, Principal}, ic_cdk::api::management_canister::ecdsa::EcdsaKeyId};
use ic_storage::{stable::Versioned, IcStorage};
use serde::Deserialize;

use crate::evm_rpc::Service;

pub type DerivationPath = Vec<Vec<u8>>;

pub struct StrategyData {
    pub manager: String,
    pub latest_rate: U256,
    pub derivation_path: DerivationPath
}

thread_local! {
    pub static RPC_CANISTER: RefCell<Service> = RefCell::new(Service(Principal::anonymous()));
    pub static RPC_URL: RefCell<String> = RefCell::new("".to_string());
    pub static STRATEGY_DATA: RefCell<HashMap<u32, StrategyData>> = RefCell::new(HashMap::new());
    pub static MANAGERS: RefCell<Vec<String>> = RefCell::new(Vec::new());
}