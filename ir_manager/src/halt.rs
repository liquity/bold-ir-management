//! Halting service to address canister failure

use chrono::Duration;
use ic_exports::{ic_cdk::api::time, ic_cdk_timers::set_timer};

use crate::{state::{HALT_STATE, STRATEGY_STATE}, strategy::stale::StableStrategy};

/// Halt struct containing reasoning and status
#[derive(Clone, PartialEq)]
pub struct Halt {
    pub status: HaltStatus,
    pub message: Option<String>
}

impl Default for Halt {
    fn default() -> Self {
        Self { status: HaltStatus::Functional, message: None }
    }
}

/// Halt Status enum determining the stage the canister is at
#[derive(Clone, PartialEq)]
pub enum HaltStatus {
    /// Functioning as expected
    Functional,
    /// Fully halted
    Halted,
    /// Has a timer scheduled to fully halt the canister soon.
    /// In this stage the canister continues to function normally.
    HaltingInProgress
}

/// Returns `true` if the canister is not set to `Halted`, and `false` if not.
pub fn is_functional() -> bool {
    HALT_STATE.with(|halt| {
        let state = halt.borrow().clone();
        HaltStatus::Halted != state.status
    })
}

/// Returns `true` if the canister status is explicitly set to `Functional`.
fn is_explicitly_functional() -> bool {
    HALT_STATE.with(|halt| {
        let state = halt.borrow().clone();
        HaltStatus::Functional == state.status
    })
}

/// Determins if the canister needs to be halted or not.
/// If yes, it will schedule a force-halt timer in 7 days.
/// Runs every 24 hours via a recurring timer.
pub fn update_halt_status() {
    // There is no need to run the function if the canister is halted or has a halt in progress.
    assert!(is_explicitly_functional());
    
    // Check for strategy successful exits
    if check_strategy_exits() {
        return;
    }

    // Checks if strategies have adjusted any rate in the past 3 months.
    if check_strategy_updates() {
        return;
    }
}

/// Checks if any strategy has updated a rate in the past 3 months.
/// If no, it means that most likely no trove has delegated to any of the strategies on this canister.
/// Returns `true`, if it schedules a halt.
fn check_strategy_updates() -> bool {
    let strategies: Vec<StableStrategy> = STRATEGY_STATE
    .with(|vector_data| vector_data.borrow().iter().map(|(_, stale)| stale.clone()).collect());
    
    let mut no_update_strategies = 0;
    
    strategies.iter().for_each(|strategy| {
        if is_older_than(strategy.data.last_ok_exit, 90) {
            no_update_strategies += 1;
        }
    });

    if no_update_strategies == strategies.len() {
        schedule_halt("No strategy has updated a rate in the past 90 days.".to_string());
        return true;
    }

    false
}

/// Checks all strategy exits for successful returns in the past 7 days.
/// If none is found, starts the process of halting the canister.
/// Returns `true` if a halt is scheduled.
fn check_strategy_exits() -> bool {
    // If no strategy has had a successful exit in the past 7 days, halt.
    let strategies: Vec<StableStrategy> = STRATEGY_STATE
    .with(|vector_data| vector_data.borrow().iter().map(|(_, stale)| stale.clone()).collect());
    
    let mut unsuccessful_strategies = 0;
    
    strategies.iter().for_each(|strategy| {
        if is_older_than(strategy.data.last_ok_exit, 7) {
            unsuccessful_strategies += 1;
        }
    });

    if unsuccessful_strategies == strategies.len() {
        schedule_halt("No strategy has had a successful exit in the past 7 days.".to_string());
        return true;
    }

    false
}

/// Schedules a halt in 7 days
fn schedule_halt(message: String) {
    // Update the current status to `HaltingInProgress`
    HALT_STATE.with(|halt| *halt.borrow_mut() = Halt {
        status: HaltStatus::HaltingInProgress,
        message: Some(message.clone())
    });

    // Schedule a timer for 7 days from now.
    set_timer(std::time::Duration::from_secs(604_800), || {
        HALT_STATE.with(|halt| *halt.borrow_mut() = Halt {
            status: HaltStatus::Halted,
            message: Some(message)
        });
    });
}

/// Check if a given timestamp (milliseconds) is older than the given number of days
fn is_older_than(timestamp_ms: u64, days: u64) -> bool {
    // Get current time in milliseconds
    let current_time_ms = (time() / 1_000_000) as u64;

    // Define the threshold
    let threshold = current_time_ms - Duration::days(days as i64).num_milliseconds() as u64;

    // Compare timestamps
    timestamp_ms < threshold
}