//! The thread storage of the canister containing mutable data structures

use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
};

use alloy_primitives::Address;
use evm_rpc_types::RpcService;
use ic_stable_structures::{DefaultMemoryImpl, Vec as StableVec};

use crate::journal::JournalEntry;
use crate::strategy::executable::ExecutableStrategy;

thread_local! {
    /// Swap ckETH Lock
    pub static SWAP_LOCK: Cell<bool> = Cell::new(false);
    /// HashMap containing all strategies' information
    pub static STRATEGY_STATE: RefCell<HashMap<u32, ExecutableStrategy>> = RefCell::new(HashMap::new());
    /// Vector of all manager addresses
    pub static MANAGERS: RefCell<Vec<Address>> = RefCell::new(Vec::new());
    /// A counter that tracks EOA turns for minting ckETH
    pub static CKETH_EOA_TURN_COUNTER: Cell<u8> = Cell::new(0);
    /// Journal
    pub static JOURNAL: RefCell<StableVec<JournalEntry, DefaultMemoryImpl>> = RefCell::new(
        StableVec::init(DefaultMemoryImpl::default()).expect("Failed to create default memory.")
    );
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
