use crate::{
    constants::MAX_RETRY_ATTEMPTS,
    journal::{JournalEntry, LogType},
    state::STRATEGY_STATE,
    utils::error::ManagerError,
};

use super::executable::ExecutableStrategy;

pub async fn run_strategy(key: u32) {
    // Create an executable instance of the strategy
    let strategy: Option<ExecutableStrategy> = STRATEGY_STATE.with(|state| {
        state.borrow().get(&key).map_or_else(
            || {
                JournalEntry::new(Err(ManagerError::NonExistentValue), LogType::Info)
                    .strategy(key)
                    .note("This strategy key was not found in the state. The execution could not be started.")
                    .commit();
                None
            },
            |stable_strategy| {
                Some(stable_strategy.into())
            },
        )
    });

    if let Some(mut executable_strategy) = strategy {
        JournalEntry::new(Ok(()), LogType::Info)
            .note("Executable strategy is created.")
            .strategy(key)
            .commit();

        for turn in 0..MAX_RETRY_ATTEMPTS {
            let result = executable_strategy.execute().await;

            // log the result
            JournalEntry::new(result.clone(), LogType::ExecutionResult)
                .strategy(key)
                .turn(turn)
                .note(format!(
                    "Strategy execution attempt is finished. Attempt {}/{}",
                    turn,
                    MAX_RETRY_ATTEMPTS - 1
                ))
                .commit();

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
