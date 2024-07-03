use std::time::Duration;

use ic_exports::{
    ic_cdk::spawn,
    ic_cdk_timers::{set_timer, set_timer_interval},
};

use crate::{
    api::execute_strategy,
    state::{MAX_RETRY_ATTEMPTS, STRATEGY_DATA},
    types::StrategyData,
    utils::{retry, set_public_keys},
};

pub fn start_timers() {
    // assign public keys to the different strategy EOAs
    set_timer(Duration::ZERO, || spawn(set_public_keys()));

    // assign a separate timer for each strategy
    let strategies: Vec<(u32, StrategyData)> = STRATEGY_DATA
        .with(|vector_data| vector_data.borrow().clone())
        .into_iter()
        .collect();

    let max_retry_attempts = MAX_RETRY_ATTEMPTS.with(|max_value| max_value.get());

    for (key, strategy) in strategies {
        set_timer_interval(Duration::from_secs(3600), move || {
            spawn(async {
                for _ in 0..=max_retry_attempts {
                    let result = match execute_strategy(key, &strategy).await {
                        Ok(()) => Ok(()),
                        Err(error) => retry(key, &strategy, error).await,
                    };

                    if result.is_ok() {
                        break;
                    }
                }
            });
        });
    }
}
