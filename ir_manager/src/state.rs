use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use alloy_primitives::U256;
use candid::Nat;
use ic_exports::candid::Principal;

use crate::strategy::StrategyData;

thread_local! {
    /// HashMap containing all strategies' information
    pub static STRATEGY_DATA: RefCell<HashMap<u32, StrategyData>> = RefCell::new(HashMap::new());
    /// Vector of all manager addreses
    pub static MANAGERS: RefCell<Vec<String>> = RefCell::new(Vec::new());
    /// Tolerance margin up formula constant
    pub static TOLERANCE_MARGIN_UP: Cell<U256> = Cell::new(U256::from(5));
    /// Tolerance margin down formula constant
    pub static TOLERANCE_MARGIN_DOWN: Cell<U256> = Cell::new(U256::from(5));
    /// Exchange rate canister's principal ID
    pub static EXCHANGE_RATE_CANISTER: Cell<Principal> = Cell::new(Principal::from_slice("uf6dk-hyaaa-aaaaq-qaaaq-cai".as_bytes()));
    /// Max number of retry attempts
    pub static MAX_RETRY_ATTEMPTS: Cell<u8> = Cell::new(2);
    /// Cycles balance threshold of the canister
    pub static CYCLES_THRESHOLD: Cell<u64> = Cell::new(50_000_000_000);
    /// A counter that tracks EOA turns for minting ckETH
    pub static CKETH_EOA_TURN_COUNTER: Cell<u8> = Cell::new(0);
    /// ckETH smart contract on Ethereum mainnet
    pub static CKETH_HELPER: RefCell<String> = RefCell::new("0x7574eB42cA208A4f6960ECCAfDF186D627dCC175".to_string());
    /// ckETH ledger canister's principal ID
    pub static CKETH_LEDGER: Cell<Principal> = Cell::new(Principal::from_slice("ss2fx-dyaaa-aaaar-qacoq-cai".as_bytes()));
    /// ckETH token transfer fee
    pub static CKETH_FEE: RefCell<Nat> = RefCell::new(Nat::from(2_000_000_000_000 as u64));
    /// ckETH mint value
    /// The amount of Ether that will be used to mint new ckETH tokens when the balance is below the threshold
    pub static ETHER_RECHARGE_VALUE: Cell<U256> = Cell::new(U256::from(30_000_000_000_000_000 as u64)); // 0.03 ETH in WEI
    /// Cycles discount percentage
    pub static CYCLES_DISCOUNT_PERCENTAGE: Cell<u64> = Cell::new(2); // 0.03 ETH in WEI
    /// ckETH balance threshold of the canister.
    /// The recharging cycle will mint more ckETH if the balance falls below this number
    pub static CKETH_THRESHOLD: RefCell<Nat> = RefCell::new(Nat::from(30_000_000_000_000_000 as u64)); // 0.03 ETH in WEI
}
