use ic_exports::ic_cdk::api::management_canister::main::raw_rand;
use rand::seq::SliceRandom;
use rand_chacha::rand_core::SeedableRng;

use crate::constants::MAINNET_PROVIDERS;
use crate::constants::SEPOLIA_PROVIDERS;
use crate::journal::JournalCollection;
use crate::journal::LogType;
use crate::state::JOURNAL;
use crate::state::RPC_REPUTATIONS;
use crate::utils::common::extract_call_result;
use crate::utils::error::ManagerError;
use crate::utils::error::ManagerResult;

/// Cleans up the journal and the reputations leaderboard
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

/// Cleans up the reputations leaderboard by:
/// 1- Shuffling the new leaderboard using a PRNG seed
/// 2- Assigning as each provider's new reputation
pub async fn reputations_cleanup() -> ManagerResult<()> {
    let mut providers = SEPOLIA_PROVIDERS.to_vec();

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

/// Cleans up the journal by:
/// 1 - Removing all provider reputation change logs
/// 2 - Removing any surplus of logs
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
