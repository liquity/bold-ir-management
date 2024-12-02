//! Journal for logging events within the canister.
//! This is mainly (and maybe exclusively) used for
//! - recording what happens in each strategy execution cycle
//! - logging responses that are not returned to any user/caller

use std::borrow::Cow;

use candid::{CandidType, Decode, Encode};
use chrono::{DateTime, Utc};
use ic_exports::ic_cdk::api::time;
use ic_stable_structures::{storable::Bound, Storable};
use serde::Deserialize;

use crate::{state::insert_journal_collection, utils::error::*};

#[derive(CandidType, Deserialize, Clone)]
pub struct JournalCollection {
    pub start_date_and_time: String,
    pub end_date_and_time: String,
    pub strategy: Option<u32>,
    pub entries: Vec<JournalEntry>,
}

/// Journal entry
#[derive(CandidType, Deserialize, Clone)]
pub struct JournalEntry {
    pub date_and_time: String,
    pub entry: ManagerResult<()>,
    pub note: Option<String>,
    pub log_type: LogType,
}

/// Log type
#[derive(PartialEq, CandidType, Deserialize, Clone)]
pub enum LogType {
    RateAdjustment,
    ExecutionResult,
    Info,
    ProviderReputationChange,
    Recharge,
}

/// Builder for journal entries
impl JournalCollection {
    /// Create a new journal collection
    pub fn open(strategy: Option<u32>) -> Self {
        Self {
            start_date_and_time: date_and_time(),
            end_date_and_time: "".to_string(),
            strategy,
            // It will re-allocate only after the capacity is filled.
            entries: Vec::with_capacity(16),
        }
    }

    /// Closes a journal collection by committing it to the state.
    pub fn close(&mut self) {
        self.end_date_and_time = date_and_time();
        insert_journal_collection(self);
    }

    /// Appends a new entry to the collection
    pub fn append_note<S: AsRef<str>>(
        &mut self,
        entry: ManagerResult<()>,
        log_type: LogType,
        note: S,
    ) -> &mut Self {
        let journal_entry = JournalEntry::new(entry, log_type, Some(note.as_ref().to_string()));
        self.entries.push(journal_entry);
        self
    }

    /// Checks if the collection has only one entry that is a reputation change.
    pub fn is_reputation_change(&self) -> bool {
        if self.entries.len() == 1 {
            return matches!(self.entries[0].log_type, LogType::ProviderReputationChange);
        }
        false
    }
}

impl Storable for JournalCollection {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    const BOUND: Bound = Bound::Bounded {
        max_size: 32_768, // 32 KB
        is_fixed_size: false,
    };
}

impl Drop for JournalCollection {
    fn drop(&mut self) {
        self.close();
    }
}

impl JournalEntry {
    /// Opens a new entry
    fn new(entry: ManagerResult<()>, log_type: LogType, note: Option<String>) -> Self {
        Self {
            date_and_time: date_and_time(),
            entry,
            note,
            log_type,
        }
    }
}

fn date_and_time() -> String {
    let timestamp_s: i64 = time() as i64 / 1_000_000_000;

    let datetime = DateTime::<Utc>::from_timestamp(timestamp_s, 0).expect("Invalid timestamp");

    // Format the DateTime as "dd-mm-yyyy hh:mm:ss"
    datetime.format("%d-%m-%Y %H:%M:%S").to_string()
}
