//! Cleanup functionality for managing system state and provider reputations.
//!
//! This module provides functionality for periodic cleanup operations including:
//! - Journal log management and pruning
//! - RPC provider reputation resets and randomization
//! - System state maintenance
//!
//! The cleanup operations help maintain system performance and ensure fair provider selection
//! by periodically resetting reputations and removing excess logs.
//!
//! # Examples
//!
//! ```
//! // Perform a complete daily cleanup
//! daily_cleanup().await;
//!
//! // Clean up just the journal logs
//! journal_cleanup();
//!
//! // Reset and randomize provider reputations
//! reputations_cleanup().await?;
//! ```
//!
//! # Architecture
//!
//! The cleanup system operates on three main components:
//!
//! 1. **Journal Management**: Removes excess logs and reputation change entries while maintaining
//!    the most recent 300 entries.
//!
//! 2. **Provider Reputations**: Periodically resets and randomizes provider rankings to ensure
//!    fair selection and prevent gaming of the reputation system.
//!
//! 3. **State Cleanup**: Maintains system state by removing stale data and ensuring data structures
//!    stay within size limits.

use ic_exports::ic_cdk::api::management_canister::main::raw_rand;
use rand::seq::SliceRandom;
use rand_chacha::rand_core::SeedableRng;

#[cfg(feature = "mainnet")]
use crate::constants::MAINNET_PROVIDERS;
#[cfg(feature = "sepolia")]
use crate::constants::SEPOLIA_PROVIDERS;
use crate::journal::JournalCollection;
use crate::journal::LogType;
use crate::state::JOURNAL;
use crate::state::RPC_REPUTATIONS;
use crate::utils::common::extract_call_result;
use crate::utils::error::ManagerError;
use crate::utils::error::ManagerResult;

/// Performs daily cleanup tasks including journal pruning and reputation resets.
///
/// This function orchestrates the complete cleanup process by:
/// - Cleaning up the journal logs
/// - Resetting provider reputations
/// - Logging the cleanup operations
///
/// The function creates a new journal collection to log the cleanup process and
/// its results.
pub async fn daily_cleanup() {
    // Create a new journal collection
    let mut journal = JournalCollection::open(None);

    journal_cleanup();

    journal.append_note(
        Ok(()),
        LogType::Info,
        "Cleaned up the journal by removing excess logs and all reputation change entries.",
    );

    let reputations_cleanup_result = reputations_cleanup().await;
    match reputations_cleanup_result {
        Ok(()) => journal.append_note(
            Ok(()),
            LogType::Info,
            "Reset provider reputations back to zero and shuffled the list.",
        ),
        Err(err) => journal.append_note(
            Err(err),
            LogType::Info,
            "Failed to reset the provider reputations list.",
        ),
    };

    journal.append_note(Ok(()), LogType::Info, "Finished the cleanup successfully.");
}

/// Resets and randomizes the RPC provider reputation rankings.
///
/// This function:
/// 1. Creates a new randomized ordering of providers using a secure RNG seed from the IC
/// 2. Resets all provider reputations to zero
/// 3. Updates the global reputation state with the new rankings
///
/// # Returns
/// - `Ok(())` if the cleanup succeeds
/// - `Err(ManagerError)` if there are issues with seed generation or state updates
///
/// # Errors
/// - Returns `ManagerError::DecodingError` if the random seed cannot be properly formatted
pub async fn reputations_cleanup() -> ManagerResult<()> {
    #[cfg(feature = "sepolia")]
    let mut providers = SEPOLIA_PROVIDERS.to_vec();
    #[cfg(feature = "mainnet")]
    let mut providers = MAINNET_PROVIDERS.to_vec();

    // Create a seeded RNG using IC timestamp
    let call_result = raw_rand().await;

    let seed: Vec<u8> = extract_call_result(call_result)?;

    // Ensure the seed is exactly 32 bytes
    let seed_array: [u8; 32] = seed.try_into().map_err(|_| {
        ManagerError::DecodingError(
            "Couldn't convert the seed bytes into a fixed length slice.".to_string(),
        )
    })?;

    let mut rng = rand_chacha::ChaCha8Rng::from_seed(seed_array);

    // Use standard shuffle with our seeded RNG
    providers.shuffle(&mut rng);

    let new_reputations = providers
        .into_iter()
        .map(|provider| (0, provider))
        .collect();

    RPC_REPUTATIONS.with(|reputations| {
        *reputations.borrow_mut() = new_reputations;
    });

    Ok(())
}

/// Manages the cleanup of the system journal logs.
///
/// This function performs two main cleanup operations:
/// 1. Removes all provider reputation change log entries
/// 2. Trims the journal to the most recent 300 entries if it exceeds that size
///
/// The cleanup process maintains only essential logs while preventing unbounded
/// growth of the journal storage.
pub fn journal_cleanup() {
    crate::state::JOURNAL.with(|journal| {
        let mut binding = journal.borrow_mut();

        // Initialize a new StableVec safely and return if initialization fails
        let temp = if let Ok(vec) =
            ic_stable_structures::Vec::init(ic_stable_structures::DefaultMemoryImpl::default())
        {
            vec
        } else {
            return; // Exit if initialization fails
        };

        for collection in binding.iter() {
            if !collection.is_reputation_change() {
                let _ = temp.push(&collection.clone());
            }
        }

        *binding = temp;
    });

    JOURNAL.with(|journal| {
        let binding = journal.borrow_mut();

        // Check if the journal has more than 300 items
        let len = binding.len();
        if len > 300 {
            let excess = len - 300;

            // Shift all items to remove the oldest ones
            for i in excess..len {
                if let Some(item) = binding.get(i) {
                    binding.set(i - excess, &item);
                }
            }

            // Pop the remaining items to resize the vector
            for _ in 0..excess {
                binding.pop();
            }
        }
    });
}
