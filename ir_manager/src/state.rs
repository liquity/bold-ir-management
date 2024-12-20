//! The thread storage of the canister containing mutable data structures

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
};

use alloy_primitives::Address;
use evm_rpc_types::{EthMainnetService, EthSepoliaService, RpcService};
use ic_stable_structures::{DefaultMemoryImpl, Vec as StableVec};

use crate::{journal::StableJournalCollection, strategy::stale::StableStrategy};

thread_local! {
    /// Latest safe block
    pub static LAST_SAFE_BLOCK: Cell<u128> = Cell::new(0);
    /// Swap ckETH Lock
    pub static SWAP_LOCK: Cell<bool> = Cell::new(false);
    /// HashMap containing all strategies' information
    pub static STRATEGY_STATE: RefCell<HashMap<u32, StableStrategy>> = RefCell::new(HashMap::new());
    /// Tracks if STRATEGY_STATE is mutably borrowed
    pub static STRATEGY_STATE_BORROW: Cell<bool> = Cell::new(false);
    /// Vector of all manager addresses
    pub static MANAGERS: RefCell<Vec<Address>> = RefCell::new(Vec::new());
    /// A counter that tracks EOA turns for minting ckETH
    pub static CKETH_EOA_TURN_COUNTER: Cell<u8> = Cell::new(0);
    /// Journal
    pub static JOURNAL: RefCell<StableVec<StableJournalCollection, DefaultMemoryImpl>> = RefCell::new(
        StableVec::init(DefaultMemoryImpl::default()).expect("Failed to create default memory.")
    );
    /// RPC Service Vec Deque
    pub static RPC_SERVICE: RefCell<VecDeque<RpcService>> = RefCell::new(VecDeque::from([
        // AUDIT: The following enums will be replaced by the Ethereum main-net providers.
        // AUDIT: Misconfiguration due to Sepolia types is out of scope.
        RpcService::EthSepolia(evm_rpc_types::EthSepoliaService::Alchemy),
        RpcService::EthSepolia(evm_rpc_types::EthSepoliaService::Ankr),
    ]));
    /// Reputation-based ranking list of all providers
    // AUDIT: The following enums will be replaced by the Ethereum main-net providers.
    // AUDIT: Misconfiguration due to Sepolia types is out of scope.
    #[cfg(feature = "sepolia")]
    pub static RPC_REPUTATIONS: RefCell<Vec<(i64, EthSepoliaService)>> = RefCell::new(vec![(0, EthSepoliaService::Ankr), (0, EthSepoliaService::BlockPi), (0, EthSepoliaService::PublicNode), (0, EthSepoliaService::Sepolia), (0, EthSepoliaService::Alchemy)]);
    #[cfg(feature = "mainnet")]
    pub static RPC_REPUTATIONS: RefCell<Vec<(i64, EthMainnetService)>> = RefCell::new(vec![(0, EthMainnetService::Ankr), (0, EthMainnetService::BlockPi), (0, EthMainnetService::PublicNode), (0, EthMainnetService::Cloudflare), (0, EthMainnetService::Alchemy)]);
}

/// Inserts a new journal collection
pub fn insert_journal_collection(entry: StableJournalCollection) {
    let _ = JOURNAL.with_borrow_mut(|vec| vec.push(&entry));
}
