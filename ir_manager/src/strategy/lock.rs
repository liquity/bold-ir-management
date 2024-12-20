//! Locking system for strategies

use ic_exports::ic_cdk::api::time;

use crate::{
    constants::STRATEGY_LOCK_TIMEOUT,
    utils::error::{ManagerError, ManagerResult},
};

/// Lock with a built-in timeout mechanism
#[derive(Clone, Copy, Default)]
pub struct Lock {
    /// Status of the lock. `true` represents locked and `false` unlocked
    is_locked: bool,
    /// Last locked timstamp in milliseconds
    last_locked_at: Option<u64>,
}

impl Lock {
    /// Sets the lock status to `locked`/`true`, if either of the conditions are satisfied:
    /// 1. The current status is unlocked.
    /// 2. The duration exceeds the timeout constant
    pub fn try_lock(&mut self) -> ManagerResult<()> {
        let current_time = time() / 1_000_000_000; // current time in millis

        if let Some(last_locked_at) = self.last_locked_at {
            if self.is_locked && current_time - last_locked_at > STRATEGY_LOCK_TIMEOUT {
                self.is_locked = false;
            }
        }

        if !self.is_locked {
            self.is_locked = true;
            self.last_locked_at = Some(current_time);
            return Ok(());
        } else {
            return Err(ManagerError::Locked);
        }
    }

    /// Sets the lock status to `unlocked`/`false`
    pub fn unlock(&mut self) -> &mut Self {
        self.is_locked = false;
        self.last_locked_at = None;
        self
    }
}
