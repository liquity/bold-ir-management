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
    pub note: Option<String>,
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
            note: None,
        }
    }

    /// Fills the `strategy_id` field of the entry
    pub fn strategy(&mut self, id: u32) -> &mut Self {
        self.strategy_id = Some(id);
        self
    }

    /// Fills the `turn` field of the entry
    pub fn turn(&mut self, turn: u8) -> &mut Self {
        self.turn = Some(turn);
        self
    }

    /// Fills the `note` field of the entry
    pub fn note<S: AsRef<str>>(&mut self, text: S) -> &mut Self {
        self.note = Some(text.as_ref().to_string());
        self
    }

    /// Commits the entry to the stable storage vector
    pub fn commit(&mut self) {
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
