//! RPC Provider Reputation System
//!
//! A sophisticated ranking mechanism that maintains and updates provider reputations based on their
//! performance. The system employs a reward-penalty model where providers' scores are adjusted according
//! to their response quality and consensus participation.
//!
//! ```plain
//! Provider Reputation Flow:
//!
//!   Success     ┌─────────────┐     Failure
//! ─────────────►│   Provider  │◄────────────
//!    +1         │  Reputation │     -1
//!               └─────────────┘
//!                     │
//!                     ▼
//!               ┌─────────────┐
//!               │   Ranking   │
//!               │   System    │
//!               └─────────────┘
//!                     │
//!            ┌────────────────┐
//!            ▼       ▼        ▼
//!        Top-1    Top-2    Top-3
//! ```
//!
//! The system features:
//! - Saturation arithmetic to prevent overflow/underflow
//! - Periodic reputation logging (every 10 score changes)
//! - Provider ranking with tie-breaking mechanisms
//! - Consensus-based reputation updates

use std::fmt::Debug;

use evm_rpc_types::{MultiRpcResult, RpcServices};

use crate::{
    constants::PROVIDER_COUNT,
    journal::JournalCollection,
    state::RPC_REPUTATIONS,
    types::ProviderService,
    utils::{
        error::{ManagerError, ManagerResult},
        evm_rpc::SendRawTransactionStatus,
    },
};

/// Retrieves the current provider rankings from thread-local storage.
///
/// Returns a vector of tuples containing each provider's score and identifier,
/// maintaining the original order from storage.
fn fetch_provider_list() -> Vec<(i64, ProviderService)> {
    RPC_REPUTATIONS.with(|leaderboard| leaderboard.borrow().clone())
}

/// Computes and returns the top-ranked providers based on reputation scores.
///
/// The ranking algorithm:
/// 1. Sorts providers by score in descending order
/// 2. Selects providers up to PROVIDER_COUNT
/// 3. Includes tied providers if they are exactly 1 point behind
///
/// Example ranking with PROVIDER_COUNT = 3:
/// ```plain
/// Scores:  10   10   9    8    7
/// Result:  [P1, P2, P3]  // P3 included despite being 1 point lower
/// ```
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

        result.push(provider_list[i].1);
        count += 1;

        // Check if the next provider is exactly one behind the current one
        if i + 1 < provider_list.len()
            && count < PROVIDER_COUNT as usize
            && provider_list[i].0 - provider_list[i + 1].0 == 1
        {
            result.push(provider_list[i + 1].1);
            count += 1;
        }
    }

    result.truncate(PROVIDER_COUNT as usize);
    result
}

/// Increments a provider's reputation score by 1, using saturating arithmetic.
///
/// - Uses saturating addition to prevent overflow at i64::MAX
/// - Logs reputation changes at every 10th increment
/// - Thread-safe through RefCell borrow_mut
///
/// # Arguments
/// * `provider` - Reference to the provider whose score should be incremented
pub fn increment_provider_score(provider: &ProviderService) {
    RPC_REPUTATIONS.with(|leaderboard| {
        let mut leaderboard = leaderboard.borrow_mut();

        // Find the provider in the leaderboard
        if let Some(entry) = leaderboard.iter_mut().find(|(_, p)| p == provider) {
            entry.0 = entry.0.saturating_add(1); // Increment the score, saturating at i64::MAX

            if entry.0 % 10 == 0 {
                JournalCollection::open(None).append_note(
                    Ok(()),
                    crate::journal::LogType::ProviderReputationChange,
                    format!(
                        "Provider {:#?} reputation change: +1 | new reputation: {}",
                        provider, entry.0
                    ),
                );
            }
        }
    });
}

/// Decrements a provider's reputation score by 1, using saturating arithmetic.
///
/// - Uses saturating subtraction to prevent underflow at i64::MIN
/// - Logs reputation changes at every 10th decrement
/// - Thread-safe through RefCell borrow_mut
///
/// # Arguments
/// * `provider` - Reference to the provider whose score should be decremented
pub fn decrement_provider_score(provider: &ProviderService) {
    RPC_REPUTATIONS.with(|leaderboard| {
        let mut leaderboard = leaderboard.borrow_mut();

        // Find the provider in the leaderboard
        if let Some(entry) = leaderboard.iter_mut().find(|(_, p)| p == provider) {
            entry.0 = entry.0.saturating_sub(1); // Decrement the score, saturating at i64::MIN
            if entry.0 % 10 == 0 {
                JournalCollection::open(None).append_note(
                    Ok(()),
                    crate::journal::LogType::ProviderReputationChange,
                    format!(
                        "Provider {:#?} reputation change: -1 | new reputation: {}",
                        provider, entry.0
                    ),
                );
            }
        }
    });
}

/// Returns the current top-ranked providers as an RPC service collection.
///
/// The ranking considers reputation scores and includes providers up to PROVIDER_COUNT.
/// Returns appropriate enum variant based on compile-time network selection (mainnet/sepolia).
pub fn get_ranked_rpc_providers() -> RpcServices {
    let ranked_provider_list = ranked_provider_list();
    #[cfg(feature = "sepolia")]
    return RpcServices::EthSepolia(Some(ranked_provider_list));
    #[cfg(feature = "mainnet")]
    return RpcServices::EthMainnet(Some(ranked_provider_list));
}

/// Returns the single highest-ranked provider as an RPC service.
///
/// Selects the provider with the highest reputation score.
/// Returns appropriate enum variant based on compile-time network selection (mainnet/sepolia).
pub fn get_ranked_rpc_provider() -> RpcServices {
    let ranked_provider_list = ranked_provider_list();
    #[cfg(feature = "sepolia")]
    return RpcServices::EthSepolia(Some(ranked_provider_list[..1].to_vec()));
    #[cfg(feature = "mainnet")]
    return RpcServices::EthMainnet(Some(ranked_provider_list[..1].to_vec()));
}

/// Processes multi-RPC results and updates provider reputations accordingly.
///
/// # Reputation Updates
/// - Consistent successful responses: All providers gain reputation
/// - Consistent failed responses: All providers lose reputation
/// - Inconsistent responses: Individual providers gain/lose based on their responses
///
/// # Arguments
/// * `providers` - The RPC services used for the request
/// * `result` - The multi-RPC result to process
///
/// # Returns
/// * `Ok(T)` - The successful result value
/// * `Err(ManagerError)` - Error indicating consensus failure or RPC issues
pub fn extract_multi_rpc_result<T: Debug>(
    providers: RpcServices,
    result: MultiRpcResult<T>,
) -> ManagerResult<T> {
    match result {
        MultiRpcResult::Consistent(response) => {
            if response.is_ok() {
                #[cfg(feature = "sepolia")]
                if let RpcServices::EthSepolia(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(increment_provider_score);
                }

                #[cfg(feature = "mainnet")]
                if let RpcServices::EthMainnet(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(increment_provider_score);
                }
            } else {
                #[cfg(feature = "sepolia")]
                if let RpcServices::EthSepolia(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(decrement_provider_score);
                }

                #[cfg(feature = "mainnet")]
                if let RpcServices::EthMainnet(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(decrement_provider_score);
                }
            }

            response.map_err(ManagerError::RpcResponseError)
        }
        MultiRpcResult::Inconsistent(responses) => {
            responses.iter().for_each(|(provider, result)| {
                #[cfg(feature = "sepolia")]
                if let evm_rpc_types::RpcService::EthSepolia(eth_sepolia_service) = provider {
                    if result.is_ok() {
                        increment_provider_score(eth_sepolia_service);
                    } else {
                        decrement_provider_score(eth_sepolia_service);
                    }
                }

                #[cfg(feature = "mainnet")]
                if let evm_rpc_types::RpcService::EthMainnet(eth_mainnet_service) = provider {
                    if result.is_ok() {
                        increment_provider_score(eth_mainnet_service);
                    } else {
                        decrement_provider_score(eth_mainnet_service);
                    }
                }
            });
            Err(ManagerError::NoConsensus(format!("{:#?}", responses)))
        }
    }
}

/// Specialized handler for raw transaction submission results across multiple providers.
///
/// Extends the base multi-RPC result handling with transaction-specific logic:
/// - Prioritizes nonce-related responses
/// - Handles transaction hash returns
/// - Maintains provider reputation based on response quality
///
/// # Arguments
/// * `providers` - The RPC services used for the transaction
/// * `result` - The multi-provider transaction submission result
///
/// # Returns
/// * `Ok(SendRawTransactionStatus)` - The transaction status
/// * `Err(ManagerError)` - Error indicating consensus failure or RPC issues
pub fn extract_multi_rpc_send_raw_transaction_status(
    providers: RpcServices,
    result: MultiRpcResult<SendRawTransactionStatus>,
) -> ManagerResult<SendRawTransactionStatus> {
    match result {
        MultiRpcResult::Consistent(response) => {
            if response.is_ok() {
                #[cfg(feature = "sepolia")]
                if let RpcServices::EthSepolia(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(increment_provider_score);
                }

                #[cfg(feature = "mainnet")]
                if let RpcServices::EthMainnet(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(increment_provider_score);
                }
            } else {
                #[cfg(feature = "sepolia")]
                if let RpcServices::EthSepolia(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(decrement_provider_score);
                }

                #[cfg(feature = "mainnet")]
                if let RpcServices::EthMainnet(services) = providers {
                    let providers_unwrapped = services.ok_or(ManagerError::NonExistentValue)?;
                    providers_unwrapped
                        .iter()
                        .for_each(decrement_provider_score);
                }
            }

            response.map_err(ManagerError::RpcResponseError)
        }
        MultiRpcResult::Inconsistent(responses) => {
            for response in responses.clone() {
                if response.1.is_ok() {
                    if let Ok(SendRawTransactionStatus::NonceTooLow) = response.1 {
                        return Ok(SendRawTransactionStatus::NonceTooLow);
                    } else if let Ok(SendRawTransactionStatus::NonceTooHigh) = response.1 {
                        return Ok(SendRawTransactionStatus::NonceTooHigh);
                    } else if let Ok(SendRawTransactionStatus::Ok(transaction_hash)) = response.1 {
                        return Ok(SendRawTransactionStatus::Ok(transaction_hash));
                    }
                }
            }

            responses.iter().for_each(|(provider, result)| {
                #[cfg(feature = "sepolia")]
                if let evm_rpc_types::RpcService::EthSepolia(eth_sepolia_service) = provider {
                    if result.is_ok() {
                        increment_provider_score(eth_sepolia_service);
                    } else {
                        decrement_provider_score(eth_sepolia_service);
                    }
                }

                #[cfg(feature = "mainnet")]
                if let evm_rpc_types::RpcService::EthMainnet(eth_mainnet_service) = provider {
                    if result.is_ok() {
                        increment_provider_score(eth_mainnet_service);
                    } else {
                        decrement_provider_score(eth_mainnet_service);
                    }
                }
            });
            Err(ManagerError::NoConsensus(format!("{:#?}", responses)))
        }
    }
}
