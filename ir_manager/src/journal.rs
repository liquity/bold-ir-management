use std::borrow::Cow;

use candid::{CandidType, Decode, Encode};
use ic_exports::ic_cdk::api::time;
use ic_stable_structures::{storable::Bound, Storable};
use serde::Deserialize;

use crate::{state::insert_journal_entry, types::ManagerResult};

/// Journal entry
#[derive(CandidType, Deserialize)]
pub struct JournalEntry {
    pub timestamp: u64,
    pub entry: ManagerResult<()>,
    pub strategy_id: Option<u32>,
    pub turn: Option<u8>,
}

/// Builder for journal entries
impl JournalEntry {
    /// Create a new instance of a journal entry
    /// Fills the `timestamp` and `entry` fields
    pub fn new(entry: ManagerResult<()>) -> Self {
        Self {
            timestamp: time(),
            entry,
            strategy_id: None,
            turn: None,
        }
    }

    /// Fills the `strategy_id` field of the entry
    pub fn strategy(&mut self, id: u32) {
        self.strategy_id = Some(id);
    }

    /// Fills the `turn` field of the entry
    pub fn turn(&mut self, turn: u8) {
        self.turn = Some(turn);
    }

    /// Commits the entry to the stable storage vector
    pub fn commit(self) {
        insert_journal_entry(self);
    }
}

impl Storable for JournalEntry {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 500,
        is_fixed_size: false,
    };
}
