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

use crate::{state::insert_journal_entry, utils::error::*};

/// Log type
#[derive(PartialEq, CandidType, Deserialize, Clone)]
pub enum LogType {
    RateAdjustment,
    ExecutionResult,
    Info,
    ProviderReputationChange,
}

/// Journal entry
#[derive(CandidType, Deserialize, Clone)]
pub struct JournalEntry {
    pub date_and_time: String,
    pub entry: ManagerResult<()>,
    pub strategy_id: Option<u32>,
    pub turn: Option<u8>,
    pub note: Option<String>,
    pub log_type: LogType,
}

/// Builder for journal entries
impl JournalEntry {
    /// Create a new instance of a journal entry
    /// Fills the `timestamp` and `entry` fields
    pub fn new(entry: ManagerResult<()>, log_type: LogType) -> Self {
        // Convert nanoseconds to seconds
        let timestamp_s: i64 = time() as i64 / 1_000_000_000;

        let datetime = DateTime::<Utc>::from_timestamp(timestamp_s, 0).expect("Invalid timestamp");

        // Format the DateTime as "dd-mm-yyyy hh:mm:ss"
        let formatted_date = datetime.format("%d-%m-%Y %H:%M:%S").to_string();

        Self {
            date_and_time: formatted_date,
            entry,
            strategy_id: None,
            turn: None,
            note: None,
            log_type,
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
        max_size: 16_384, // 16 KB
        is_fixed_size: false,
    };
}
