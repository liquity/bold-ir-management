//! # Journal Module
//!
//! A module for logging events within the canister. This is primarily used for:
//! - Recording activities during strategy execution cycles.
//! - Logging responses that are not returned to any user/caller.
//!
//! The module defines a `JournalCollection` and its stable counterpart, `StableJournalCollection`.
//! Journals store time-bound logs, including specific notes and log types.

//! # Overview
//!
//! - `StableJournalCollection`: A storable structure representing the persistent journal state.
//! - `JournalCollection`: An in-memory journal that appends and commits logs.
//! - `JournalEntry`: Represents a single log entry with metadata.
//! - `LogType`: Enum to categorize log entries.
//!
//! Journals are automatically closed and committed to the state upon dropping their instance.

use std::borrow::Cow;

use candid::{CandidType, Decode, Encode};
use chrono::{DateTime, Utc};
use ic_exports::ic_cdk::api::time;
use ic_stable_structures::{storable::Bound, Storable};
use serde::Deserialize;

use crate::{state::insert_journal_collection, utils::error::*};

/// A stable representation of the journal collection.
///
/// This structure is storable in stable memory and is used for persisting journal entries.
#[derive(CandidType, Deserialize, Clone)]
pub struct StableJournalCollection {
    /// Start timestamp when the journal was created
    pub start_date_and_time: String,
    /// End timestamp when the journal was closed
    pub end_date_and_time: String,
    /// Optional strategy ID associated with the journal.
    pub strategy: Option<u32>,
    /// A list of `JournalEntry` instances representing individual logs
    pub entries: Vec<JournalEntry>,
}

impl StableJournalCollection {
    /// Checks if the collection has exactly one entry and the log type is `ProviderReputationChange`.
    ///
    /// # Returns
    /// - `true` if there is only one entry and it indicates a reputation change.
    /// - `false` otherwise.
    pub fn is_reputation_change(&self) -> bool {
        if self.entries.len() == 1 {
            return matches!(self.entries[0].log_type, LogType::ProviderReputationChange);
        }
        false
    }
}

impl Storable for StableJournalCollection {
    /// Serializes the collection to bytes for stable storage.
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    /// Deserializes a collection from bytes.
    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }

    /// Specifies the maximum size and dynamic nature of the stored collection.
    const BOUND: Bound = Bound::Bounded {
        max_size: 32_768, // 32 KB
        is_fixed_size: false,
    };
}

/// A runtime journal collection for recording log entries.
///
/// This structure represents an open, time-bound log journal. Upon dropping the collection,
/// it is closed and its data is committed to the state.
#[derive(CandidType, Deserialize, Clone)]
pub struct JournalCollection {
    /// Timestamp when the journal was opened.
    pub start_date_and_time: String,
    /// Timestamp when the journal was closed.
    pub end_date_and_time: String,
    /// Optional strategy ID.
    pub strategy: Option<u32>,
    /// A vector of `JournalEntry` instances.
    pub entries: Vec<JournalEntry>,
}

/// Represents a single log entry within a journal.
#[derive(CandidType, Deserialize, Clone)]
pub struct JournalEntry {
    /// Timestamp when the entry was created.
    pub date_and_time: String,
    /// The result or status associated with the log.
    pub entry: ManagerResult<()>,
    /// Optional note providing additional details.
    pub note: Option<String>,
    /// The type/category of the log.
    pub log_type: LogType,
}

/// Enum representing the type of a log entry.
#[derive(PartialEq, CandidType, Deserialize, Clone, Debug)]
pub enum LogType {
    /// Log related to rate adjustments.
    RateAdjustment,
    /// Logs results of executions.
    ExecutionResult,
    /// General information logs.
    Info,
    /// Logs changes in provider reputation.
    ProviderReputationChange,
    /// Logs related to recharges.
    Recharge,
}

impl JournalCollection {
    /// Opens a new journal collection for logging.
    ///
    /// # Arguments
    /// - `strategy`: An optional strategy ID associated with the journal.
    ///
    /// # Returns
    /// A new `JournalCollection` instance with the start time initialized.
    pub fn open(strategy: Option<u32>) -> Self {
        Self {
            start_date_and_time: date_and_time(),
            end_date_and_time: String::new(),
            strategy,
            entries: Vec::with_capacity(16), // Pre-allocated capacity for efficiency.
        }
    }

    /// Closes the journal and commits it to the state.
    ///
    /// This method sets the end time and stores the journal into stable storage.
    /// It is automatically called when the journal is dropped.
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

    /// Appends a new log entry with a note to the journal.
    ///
    /// # Arguments
    /// - `entry`: A `ManagerResult` representing the status of the log entry.
    /// - `log_type`: The type of log (`LogType`).
    /// - `note`: Additional textual information (optional).
    ///
    /// # Returns
    /// A mutable reference to the updated journal collection.
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
    /// Ensures the journal is closed and persisted upon going out of scope.
    fn drop(&mut self) {
        self.close();
    }
}

impl JournalEntry {
    /// Creates a new journal entry.
    ///
    /// # Arguments
    /// - `entry`: A `ManagerResult` representing the status of the log entry.
    /// - `log_type`: The type of log (`LogType`).
    /// - `note`: Optional note providing additional context.
    ///
    /// # Returns
    /// A new `JournalEntry` instance.
    fn new(entry: ManagerResult<()>, log_type: LogType, note: Option<String>) -> Self {
        Self {
            date_and_time: date_and_time(),
            entry,
            note,
            log_type,
        }
    }
}

/// Generates the current date and time as a formatted string.
///
/// # Returns
/// A string representing the current UTC time in the format `dd-mm-yyyy hh:mm:ss`.
#[cfg(not(test))]
fn date_and_time() -> String {
    let timestamp_s: i64 = time() as i64 / 1_000_000_000;
    let datetime = DateTime::<Utc>::from_timestamp(timestamp_s, 0).expect("Invalid timestamp");

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
