//! Implementation of a reputation-based ranking system for the RPC providers

use evm_rpc_types::EthSepoliaService;

use crate::{constants::PROVIDER_COUNT, state::RPC_REPUTATIONS};

/// Getter function to retrieve the ranked list of providers from the thread's local storage
fn fetch_provider_list() -> Vec<(i64, EthSepoliaService)> {
    RPC_REPUTATIONS.with(|leaderboard| leaderboard.borrow().clone())
}

/// Sorts the providers and returns the top ones.
pub fn ranked_provider_list() -> Vec<EthSepoliaService> {
    let mut provider_list = fetch_provider_list();

    // Sort the providers by the first element in descending order
    provider_list.sort_by(|a, b| b.0.cmp(&a.0));

    // Extract only the `EthSepoliaService` values
    let mut provider_list: Vec<EthSepoliaService> = provider_list.into_iter().map(|(_, x)| x).collect();

    // Truncate the list to a maximum of `PROVIDER_COUNT`
    if provider_list.len() > PROVIDER_COUNT as usize {
        provider_list.truncate(PROVIDER_COUNT as usize);
    }

    provider_list
}

/// Increments the score of a specific provider by 1
pub fn increment_provider_score(provider: &EthSepoliaService) {
    RPC_REPUTATIONS.with(|leaderboard| {
        let mut leaderboard = leaderboard.borrow_mut();

        // Find the provider in the leaderboard
        if let Some(entry) = leaderboard.iter_mut().find(|(_, p)| p == provider) {
            entry.0 += 1; // Increment the score
        }
    });
}

/// Decrements the score of a specific provider by 1
pub fn decrement_provider_score(provider: &EthSepoliaService) {
    RPC_REPUTATIONS.with(|leaderboard| {
        let mut leaderboard = leaderboard.borrow_mut();

        // Find the provider in the leaderboard
        if let Some(entry) = leaderboard.iter_mut().find(|(_, p)| p == provider) {
            entry.0 -= 1; // Decrement the score
        }
    });
}