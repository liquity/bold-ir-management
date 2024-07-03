use std::time::Duration;

use ic_exports::{ic_cdk::spawn, ic_cdk_timers::{set_timer, set_timer_interval}};

use crate::{api::execute_strategy, state::STRATEGY_DATA, types::StrategyData, utils::set_public_keys};

pub fn start_timers() {
    // assign public keys to the different strategy EOAs
    set_timer(Duration::ZERO, || spawn(set_public_keys()));

    // assign a separate timer for each strategy
    let strategies: Vec<(u32, StrategyData)> = STRATEGY_DATA
        .with(|vector_data| vector_data.borrow().clone())
        .into_iter()
        .collect();

    for (key, strategy) in strategies {
        set_timer_interval(Duration::from_secs(3600), move || {
            spawn(execute_strategy(key, &strategy));
        });
    }
}