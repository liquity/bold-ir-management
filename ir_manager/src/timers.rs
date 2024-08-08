use std::{sync::Arc, time::Duration};

use ic_exports::{
    ic_cdk::spawn,
    ic_cdk_timers::{set_timer, set_timer_interval},
};

use crate::{
    charger::recharge_cketh,
    state::{MAX_RETRY_ATTEMPTS, STRATEGY_DATA},
    utils::{retry, set_public_keys},
};

/// Starts timers for all strategies, and a recurring timer for cycle balance checks.
pub fn start_timers() {
    // assign public keys to the different strategy EOAs
    set_timer(Duration::ZERO, || spawn(set_public_keys()));

    // assign a separate timer for each strategy
    let strategies = STRATEGY_DATA.with(|vector_data| vector_data.borrow().clone());

    let max_retry_attempts = Arc::new(MAX_RETRY_ATTEMPTS.with(|attempts| attempts.get()));

    // STRATEGY TIMER | EVERY 1 HOUR
    strategies.into_iter().for_each(|(key, strategy)| {
        let max_retry_attempts = Arc::clone(&max_retry_attempts);
        set_timer_interval(Duration::from_secs(3600), move || {
            let mut strategy = strategy.clone();
            let max_retry_attempts = Arc::clone(&max_retry_attempts);
            spawn(async move {
                for _ in 0..=*max_retry_attempts {
                    let result = match strategy.execute().await {
                        Ok(()) => Ok(()),
                        Err(error) => retry(key, &mut strategy.clone(), error).await,
                    };

                    if result.is_ok() {
                        break;
                    }
                }
            });
        });
    });

    // CKETH RECHARGER | EVERY 24 HOURS
    set_timer_interval(Duration::from_secs(86_400), move || {
        let max_retry_attempts = Arc::clone(&max_retry_attempts);
        spawn(async move {
            for _ in 0..=*max_retry_attempts {
                let result = match recharge_cketh().await {
                    Ok(()) => Ok(()),
                    Err(_error) => recharge_cketh().await,
                };

                if result.is_ok() {
                    break;
                }
            }
        });
    });
}
