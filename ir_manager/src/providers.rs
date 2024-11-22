//! Implementation of a reputation-based ranking system for the RPC providers
use std::fmt::Debug;

use evm_rpc_types::{MultiRpcResult, RpcServices};

use crate::{
    constants::PROVIDER_COUNT,
    journal::JournalEntry,
    state::RPC_REPUTATIONS,
    types::ProviderService,
    utils::error::{ManagerError, ManagerResult},
};

/// Getter function to retrieve the ranked list of providers from the thread's local storage
fn fetch_provider_list() -> Vec<(i64, ProviderService)> {
    RPC_REPUTATIONS.with(|leaderboard| leaderboard.borrow().clone())
}

/// Sorts the providers and returns the top ones.
fn ranked_provider_list() -> Vec<ProviderService> {
    let mut provider_list = fetch_provider_list();

    // Sort the providers by the first element in descending order
    provider_list.sort_by(|a, b| b.0.cmp(&a.0));

    // Extract the top PROVIDER_COUNT providers
    let mut result = Vec::new();
    let mut count = 0;

    for i in 0..provider_list.len() {
        if count >= PROVIDER_COUNT as usize {
            break;
        }

        result.push(provider_list[i].1.clone());
        count += 1;

        // Check if the next provider is exactly one behind the current one
        if i + 1 < provider_list.len() && count < PROVIDER_COUNT as usize {
            if provider_list[i].0 - provider_list[i + 1].0 == 1 {
                result.push(provider_list[i + 1].1.clone());
                count += 1;
            }
        }
    }

    result.truncate(PROVIDER_COUNT as usize);
    result
}

/// Increments the score of a specific provider by 1, using saturating arithmetic
pub fn increment_provider_score(provider: &ProviderService) {
    RPC_REPUTATIONS.with(|leaderboard| {
        let mut leaderboard = leaderboard.borrow_mut();

        // Find the provider in the leaderboard
        if let Some(entry) = leaderboard.iter_mut().find(|(_, p)| p == provider) {
            entry.0 = entry.0.saturating_add(1); // Increment the score, saturating at i64::MAX
            JournalEntry::new(Ok(()))
                .note(format!(
                    "Provider {:#?} reputation change: +1 | new reputation: {}",
                    provider, entry.0
                ))
                .commit();
        }
    });
}

/// Decrements the score of a specific provider by 1, using saturating arithmetic
pub fn decrement_provider_score(provider: &ProviderService) {
    RPC_REPUTATIONS.with(|leaderboard| {
        let mut leaderboard = leaderboard.borrow_mut();

        // Find the provider in the leaderboard
        if let Some(entry) = leaderboard.iter_mut().find(|(_, p)| p == provider) {
            entry.0 = entry.0.saturating_sub(1); // Decrement the score, saturating at i64::MIN
            JournalEntry::new(Ok(()))
                .note(format!(
                    "Provider {:#?} reputation change: -1 | new reputation: {}",
                    provider, entry.0
                ))
                .commit();
        }
    });
}

/// Returns the top ranking providers from the leaderboard
pub fn get_ranked_rpc_providers() -> RpcServices {
    let ranked_provider_list = ranked_provider_list();

    RpcServices::EthSepolia(Some(ranked_provider_list))
}

/// Returns the top ranking provider from the leaderboard
pub fn get_ranked_rpc_provider() -> RpcServices {
    let ranked_provider_list = ranked_provider_list();

    RpcServices::EthSepolia(Some(ranked_provider_list[..1].to_vec()))
}

/// Updates the provider rankings based on the providers used in a call and the outcome of that call.
pub fn extract_multi_rpc_result<T: Debug>(
    providers: RpcServices,
    result: MultiRpcResult<T>,
) -> ManagerResult<T> {
    match result {
        MultiRpcResult::Consistent(response) => {
            if let RpcServices::EthSepolia(services) = providers {
                let providers_unwrapped = services.unwrap();
                providers_unwrapped
                    .iter()
                    .for_each(|provider| increment_provider_score(provider));
            }

            return response.map_err(ManagerError::RpcResponseError);
        }
        MultiRpcResult::Inconsistent(responses) => {
            responses.iter().for_each(|(provider, result)| {
                match provider {
                    evm_rpc_types::RpcService::EthSepolia(eth_sepolia_service) => {
                        if result.is_ok() {
                            increment_provider_score(eth_sepolia_service);
                        } else {
                            decrement_provider_score(eth_sepolia_service);
                        }
                    }
                    _ => {} // Unsupported/unexpected provider...
                }
            });
            Err(ManagerError::NoConsensus(format!("{:#?}", responses)))
        }
    }
}
