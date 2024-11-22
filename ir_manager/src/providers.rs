//! Implementation of a reputation-based ranking system for the RPC providers
use std::fmt::Debug;

use evm_rpc_types::{MultiRpcResult, RpcServices};

use crate::{
    constants::PROVIDER_COUNT,
    state::RPC_REPUTATIONS,
    types::ProviderService,
    utils::error::{ManagerError, ManagerResult},
};

/// Getter function to retrieve the ranked list of providers from the thread's local storage
fn fetch_provider_list() -> Vec<(i64, ProviderService)> {
    RPC_REPUTATIONS.with(|leaderboard| leaderboard.borrow().clone())
}

/// Sorts the providers and returns the top ones.
pub fn ranked_provider_list() -> Vec<ProviderService> {
    let mut provider_list = fetch_provider_list();

    // Sort the providers by the first element in descending order
    provider_list.sort_by(|a, b| b.0.cmp(&a.0));

    // Extract only the `ProviderService` values
    let mut provider_list: Vec<ProviderService> =
        provider_list.into_iter().map(|(_, x)| x).collect();

    // Truncate the list to a maximum of `PROVIDER_COUNT`
    if provider_list.len() > PROVIDER_COUNT as usize {
        provider_list.truncate(PROVIDER_COUNT as usize);
    }

    provider_list
}

/// Increments the score of a specific provider by 1, using saturating arithmetic
pub fn increment_provider_score(provider: &ProviderService) {
    RPC_REPUTATIONS.with(|leaderboard| {
        let mut leaderboard = leaderboard.borrow_mut();

        // Find the provider in the leaderboard
        if let Some(entry) = leaderboard.iter_mut().find(|(_, p)| p == provider) {
            entry.0 = entry.0.saturating_add(1); // Increment the score, saturating at i64::MAX
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
        }
    });
}

/// Returns the top ranking providers from the leaderboard
pub fn get_ranked_rpc_providers() -> RpcServices {
    let ranked_provider_list = ranked_provider_list();

    RpcServices::EthSepolia(Some(ranked_provider_list))
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
