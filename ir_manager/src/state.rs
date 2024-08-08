use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use alloy_primitives::U256;
use candid::Nat;
use ic_exports::candid::Principal;

use crate::{evm_rpc::Service, strategy::StrategyData};

thread_local! {
    // DYNAMIC STRATEGY DATA
    pub static STRATEGY_DATA: RefCell<HashMap<u32, StrategyData>> = RefCell::new(HashMap::new());

    // LIQUITY V2 CONTRACTS
    pub static COLLATERAL_REGISTRY: RefCell<String> = RefCell::new("TODO".to_string());
    pub static MANAGERS: RefCell<Vec<String>> = RefCell::new(Vec::new());

    // FORMULA CONSTANTS
    pub static TOLERANCE_MARGIN_UP: Cell<U256> = Cell::new(U256::from(5));
    pub static TOLERANCE_MARGIN_DOWN: Cell<U256> = Cell::new(U256::from(5));

    // CANISTER SETTINGS
    pub static RPC_CANISTER: RefCell<Service> = RefCell::new(Service::default());
    pub static RPC_URL: RefCell<String> = RefCell::new(String::new());
    pub static EXCHANGE_RATE_CANISTER: Cell<Principal> = Cell::new(Principal::from_slice("uf6dk-hyaaa-aaaaq-qaaaq-cai".as_bytes()));
    pub static MAX_RETRY_ATTEMPTS: Cell<u8> = Cell::new(3);
    pub static CYCLES_THRESHOLD: Cell<u64> = Cell::new(50_000_000_000);

    // CKETH SETTINGS
    pub static CKETH_HELPER: RefCell<String> = RefCell::new("0x7574eB42cA208A4f6960ECCAfDF186D627dCC175".to_string());
    pub static CKETH_LEDGER: Cell<Principal> = Cell::new(Principal::from_slice("ss2fx-dyaaa-aaaar-qacoq-cai".as_bytes()));
    pub static CKETH_FEE: RefCell<Nat> = RefCell::new(Nat::from(2_000_000_000_000 as u64));
    pub static ETHER_RECHARGE_VALUE: Cell<U256> = Cell::new(U256::from(30_000_000_000_000_000 as u64)); // 0.03 ETH in WEI
    pub static CKETH_THRESHOLD: RefCell<Nat> = RefCell::new(Nat::from(30_000_000_000_000_000 as u64)); // 0.03 ETH in WEI
}
