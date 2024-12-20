use crate::{
    constants::MAX_RETRY_ATTEMPTS, halt::is_functional, journal::{JournalCollection, LogType}, state::STRATEGY_STATE, utils::error::ManagerError
};

use super::executable::ExecutableStrategy;

/// Runs the strategy by creating an executable instance of it that implements the drop trait.
/// Starts a new log collection.
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

        for turn in 0..MAX_RETRY_ATTEMPTS {
            let result = executable_strategy.execute(&mut journal).await;

            // log the result
            journal.append_note(
                result.clone(),
                LogType::ExecutionResult,
                format!(
                    "Strategy execution attempt is finished. Attempt {}/{}",
                    turn,
                    MAX_RETRY_ATTEMPTS - 1
                ),
            );

            // Handle success or failure for each strategy execution attempt
            match result {
                Ok(()) => break,
                Err(_) => {
                    executable_strategy.unlock(); // Unlock on failure
                }
            }
        }
    }
    // The executable strategy will go out of scope by this line, in any way possible.
    // When it goes out of scope, Drop is called and the stable strategy will be unlocked.
}
