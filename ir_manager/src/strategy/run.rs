//! Strategy Execution Runner
//!
//! Orchestrates the lifecycle of strategy execution, including:
//! - Strategy instantiation
//! - Retry management  
//! - Execution monitoring
//! - State transitions
//!
//! ```plain
//! Execution Flow:                         
//!                                        
//! ┌──────┐     ┌─────────┐     ┌───────┐
//! │Start │────►│Load     │────►│Execute│
//! └──────┘     │Strategy │     └───┬───┘
//!              └─────────┘         │ ╲    
//!                      ┌───────────┘  ╲   
//!                      ▼              ╲
//!               ┌────────────┐    ┌────┐
//! Retry Loop:   │  Success   │    │Fail│
//! MAX_RETRY     │   Exit     │    └──┬─┘
//! ATTEMPTS      └────────────┘       │
//!                                    └─► Retry
//! ```

use crate::{
    constants::MAX_RETRY_ATTEMPTS,
    halt::is_functional,
    journal::{JournalCollection, LogType},
    state::STRATEGY_STATE,
    utils::error::ManagerError,
};

use super::executable::ExecutableStrategy;

/// Executes a strategy with retry logic and state management.
///
/// Creates and manages a strategy execution lifecycle:
/// 1. Validates system functionality
/// 2. Opens execution journal
/// 3. Loads strategy from state
/// 4. Executes with automatic retries
/// 5. Handles cleanup via Drop trait
///
/// # Arguments
/// * `key` - Unique identifier of the strategy to execute
pub async fn run_strategy(key: u32) {
    assert!(is_functional());
    let mut journal = JournalCollection::open(Some(key));

    // Create an executable instance of the strategy
    let strategy: Option<ExecutableStrategy> = STRATEGY_STATE.with(|state| {
        state.borrow().get(&key).map_or_else(
            || {
                journal.append_note(Err(ManagerError::NonExistentValue), LogType::Info , "This strategy key was not found in the state. The execution could not be started.");
                None
            },
            |stable_strategy| {
                Some(stable_strategy.into())
            },
        )
    });

    if let Some(mut executable_strategy) = strategy {
        journal.append_note(Ok(()), LogType::Info, "Executable strategy is created.");

        for turn in 1..=MAX_RETRY_ATTEMPTS {
            let result = executable_strategy.execute(&mut journal).await;
            executable_strategy.unlock();

            // log the result
            journal.append_note(
                result.clone(),
                LogType::ExecutionResult,
                format!(
                    "Strategy execution attempt is finished. Attempts remaining: {}",
                    MAX_RETRY_ATTEMPTS - turn
                ),
            );

            if result.is_ok() {
                executable_strategy.data.record_last_ok_exit();
                break;
            }
        }
    }
}
