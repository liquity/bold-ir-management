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
pub struct StableJournalCollection {
    pub start_date_and_time: String,
    pub end_date_and_time: String,
    pub strategy: Option<u32>,
    pub entries: Vec<JournalEntry>,
}

impl StableJournalCollection {
    /// Checks if the collection has only one entry that is a reputation change.
    pub fn is_reputation_change(&self) -> bool {
        if self.entries.len() == 1 {
            return matches!(self.entries[0].log_type, LogType::ProviderReputationChange);
        }
        false
    }
}

impl Storable for StableJournalCollection {
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
#[derive(PartialEq, CandidType, Deserialize, Clone, Debug)]
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
            end_date_and_time: String::new(),
            strategy,
            // It will re-allocate only after the capacity is filled.
            entries: Vec::with_capacity(16),
        }
    }

    /// Closes a journal collection by committing it to the state.
    /// Only called by the drop trait impl.
    fn close(&mut self) {
        self.end_date_and_time = date_and_time();
        let stable_jc = StableJournalCollection {
            start_date_and_time: self.start_date_and_time.clone(),
            end_date_and_time: self.end_date_and_time.clone(),
            strategy: self.strategy,
            entries: self.entries.clone(),
        };
        insert_journal_collection(stable_jc);
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

#[cfg(not(test))]
fn date_and_time() -> String {
    let timestamp_s: i64 = time() as i64 / 1_000_000_000;

    let datetime = DateTime::<Utc>::from_timestamp(timestamp_s, 0).expect("Invalid timestamp");

    // Format the DateTime as "dd-mm-yyyy hh:mm:ss"
    datetime.format("%d-%m-%Y %H:%M:%S").to_string()
}

#[cfg(test)]
fn date_and_time() -> String {
    "03-01-2009 10:15:05".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::error::ManagerResult;

    #[test]
    fn test_journal_collection_open() {
        let strategy_id = Some(42);
        let collection = JournalCollection::open(strategy_id);

        assert_eq!(collection.strategy, strategy_id);
        assert!(!collection.start_date_and_time.is_empty());
        assert!(collection.end_date_and_time.is_empty());
        assert!(collection.entries.is_empty());
    }

    #[test]
    fn test_append_note() {
        let mut collection = JournalCollection::open(None);
        let log_type = LogType::Info;

        collection.append_note(ManagerResult::Ok(()), log_type.clone(), "Test note");

        assert_eq!(collection.entries.len(), 1);
        let entry = &collection.entries[0];

        assert_eq!(entry.log_type, log_type);
        assert_eq!(entry.note.as_deref(), Some("Test note"));
        assert!(!entry.date_and_time.is_empty());
    }

    #[test]
    fn test_close_sets_end_time_and_calls_insert() {
        let mut collection = JournalCollection::open(Some(1));
        let log_type = LogType::ExecutionResult;

        // Mock entries to test the close function
        collection.append_note(ManagerResult::Ok(()), log_type, "Finalizing");

        collection.close();

        assert!(!collection.end_date_and_time.is_empty());
    }

    #[test]
    fn test_journal_entry_new() {
        let log_type = LogType::ProviderReputationChange;
        let note = "Reputation increased";

        let entry = JournalEntry::new(ManagerResult::Ok(()), log_type.clone(), Some(note.to_string()));

        assert_eq!(entry.log_type, log_type);
        assert_eq!(entry.note.as_deref(), Some(note));
        assert!(!entry.date_and_time.is_empty());
    }

    #[test]
    fn test_stable_journal_collection_reputation_change() {
        let reputation_entry = JournalEntry::new(
            ManagerResult::Ok(()),
            LogType::ProviderReputationChange,
            Some("Reputation update".to_string()),
        );

        let collection = StableJournalCollection {
            start_date_and_time: "01-01-2024 10:00:00".to_string(),
            end_date_and_time: "01-01-2024 10:05:00".to_string(),
            strategy: None,
            entries: vec![reputation_entry],
        };

        assert!(collection.is_reputation_change());
    }

    #[test]
    fn test_stable_journal_collection_not_reputation_change() {
        let other_entry = JournalEntry::new(
            ManagerResult::Ok(()),
            LogType::Info,
            Some("Info log".to_string()),
        );

        let collection = StableJournalCollection {
            start_date_and_time: "01-01-2024 10:00:00".to_string(),
            end_date_and_time: "01-01-2024 10:05:00".to_string(),
            strategy: None,
            entries: vec![other_entry],
        };

        assert!(!collection.is_reputation_change());
    }

    #[test]
    fn test_storable_to_and_from_bytes() {
        let entry = JournalEntry::new(
            ManagerResult::Ok(()),
            LogType::RateAdjustment,
            Some("Rate adjusted".to_string()),
        );

        let stable_collection = StableJournalCollection {
            start_date_and_time: "01-01-2024 10:00:00".to_string(),
            end_date_and_time: "01-01-2024 10:10:00".to_string(),
            strategy: Some(123),
            entries: vec![entry],
        };

        let bytes = stable_collection.to_bytes();
        let decoded = StableJournalCollection::from_bytes(bytes);

        assert_eq!(decoded.start_date_and_time, stable_collection.start_date_and_time);
        assert_eq!(decoded.end_date_and_time, stable_collection.end_date_and_time);
        assert_eq!(decoded.strategy, stable_collection.strategy);
        assert_eq!(decoded.entries.len(), 1);
        assert_eq!(decoded.entries[0].log_type, LogType::RateAdjustment);
    }

    #[test]
    fn test_is_reputation_change_empty_entries() {
        let collection = StableJournalCollection {
            start_date_and_time: "01-01-2024 10:00:00".to_string(),
            end_date_and_time: "01-01-2024 10:10:00".to_string(),
            strategy: None,
            entries: vec![],
        };

        assert!(!collection.is_reputation_change());
    }

    #[test]
    fn test_is_reputation_change_multiple_entries() {
        let entry1 = JournalEntry::new(
            ManagerResult::Ok(()),
            LogType::ProviderReputationChange,
            Some("Reputation update".to_string()),
        );

        let entry2 = JournalEntry::new(
            ManagerResult::Ok(()),
            LogType::ExecutionResult,
            None,
        );

        let collection = StableJournalCollection {
            start_date_and_time: "01-01-2024 10:00:00".to_string(),
            end_date_and_time: "01-01-2024 10:15:00".to_string(),
            strategy: None,
            entries: vec![entry1, entry2],
        };

        assert!(!collection.is_reputation_change());
    }
}
