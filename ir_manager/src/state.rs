use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
};

use alloy_primitives::{Address, U256};
use candid::Nat;
use evm_rpc_types::RpcService;
use ic_exports::candid::Principal;
use ic_stable_structures::{DefaultMemoryImpl, Vec as StableVec};

use crate::journal::JournalEntry;
use crate::strategy::StrategyData;

pub const SCALE: u128 = 1_000_000_000_000_000_000; // e18

thread_local! {
    /// Chain ID
    pub static CHAIN_ID: Cell<u64> = Cell::from(1);
    /// Tolerance margin up formula constant
    pub static TOLERANCE_MARGIN_UP: Cell<U256> = Cell::new(U256::from(2 * SCALE / 100)); // 2*10^16 => 20%
    /// Tolerance margin down formula constant
    pub static TOLERANCE_MARGIN_DOWN: Cell<U256> = Cell::new(U256::from(2 * SCALE / 100)); // 2*10^16 => 20%
    /// Max number of retry attempts
    pub static MAX_RETRY_ATTEMPTS: Cell<u8> = Cell::new(3);
    /// Max number of troves to fetch in one call
    pub static MAX_NUMBER_OF_TROVES: Cell<u128> = Cell::new(100);
    /// Cycles balance threshold of the canister
    pub static CYCLES_THRESHOLD: Cell<u64> = Cell::new(50_000_000_000);
    /// ckETH token transfer fee
    pub static CKETH_FEE: RefCell<Nat> = RefCell::new(Nat::from(2_000_000_000_000_u64));
    /// ckETH mint value
    /// The amount of Ether that will be used to mint new ckETH tokens when the balance is below the threshold
    pub static ETHER_RECHARGE_VALUE: Cell<U256> = Cell::new(U256::from(30_000_000_000_000_000_u64)); // 0.03 ETH in WEI
    /// Cycles discount percentage
    pub static CYCLES_DISCOUNT_PERCENTAGE: Cell<u64> = Cell::new(97); // 3% discount is provided.
    /// ckETH balance threshold of the canister.
    /// The recharging cycle will mint more ckETH if the balance falls below this number
    pub static CKETH_THRESHOLD: RefCell<Nat> = RefCell::new(Nat::from(100_000_000_000_000_u64)); // 100 Trillion Cycles
    pub static DEFAULT_MAX_RESPONSE_BYTES: Cell<u64> = Cell::new(8_000);

    /// Exchange rate canister's principal ID
    pub static EXCHANGE_RATE_CANISTER: Cell<Principal> = Cell::new(Principal::from_slice("uf6dk-hyaaa-aaaaq-qaaaq-cai".as_bytes()));
    /// ckETH smart contract on Ethereum mainnet
    pub static CKETH_HELPER: RefCell<String> = RefCell::new("0x7574eB42cA208A4f6960ECCAfDF186D627dCC175".to_string());
    /// ckETH ledger canister's principal ID
    pub static CKETH_LEDGER: Cell<Principal> = Cell::new(Principal::from_slice("ss2fx-dyaaa-aaaar-qacoq-cai".as_bytes()));
    /// Swap ckETH Lock
    pub static SWAP_LOCK: Cell<bool> = Cell::new(false);

    /// HashMap containing all strategies' information
    pub static STRATEGY_DATA: RefCell<HashMap<u32, StrategyData>> = RefCell::new(HashMap::new());
    /// Vector of all manager addreses
    pub static MANAGERS: RefCell<Vec<Address>> = RefCell::new(Vec::new());
    /// A counter that tracks EOA turns for minting ckETH
    pub static CKETH_EOA_TURN_COUNTER: Cell<u8> = Cell::new(0);
    /// Journal
    pub static JOURNAL: RefCell<StableVec<JournalEntry, DefaultMemoryImpl>> = RefCell::new(StableVec::init(DefaultMemoryImpl::default()).expect("Failed to create default memory."));
    /// RPC Service Vec Deque
    pub static RPC_SERVICE: RefCell<VecDeque<RpcService>> = RefCell::new(VecDeque::from([
        RpcService::EthMainnet(evm_rpc_types::EthMainnetService::Alchemy),
        RpcService::EthMainnet(evm_rpc_types::EthMainnetService::Ankr),
        RpcService::EthMainnet(evm_rpc_types::EthMainnetService::BlockPi),
        RpcService::EthMainnet(evm_rpc_types::EthMainnetService::Cloudflare),
        RpcService::EthMainnet(evm_rpc_types::EthMainnetService::Llama),
        RpcService::EthMainnet(evm_rpc_types::EthMainnetService::PublicNode)
    ]));
}

/// Inserts a new journal entry
pub fn insert_journal_entry(entry: &mut JournalEntry) {
    let _ = JOURNAL.with_borrow_mut(|vec| vec.push(entry));
}
