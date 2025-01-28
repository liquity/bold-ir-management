//! Locking system for strategies

use ic_exports::ic_cdk::api::time;

use crate::{
    constants::STRATEGY_LOCK_TIMEOUT,
    utils::error::{ManagerError, ManagerResult},
};

/// Lock with a built-in timeout mechanism
#[derive(Clone, Default)]
pub struct Lock {
    /// Status of the lock. `true` represents locked and `false` unlocked
    pub is_locked: bool,
    /// Last locked timstamp in milliseconds
    pub last_locked_at: Option<u64>,
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
            Ok(())
        } else {
            Err(ManagerError::Locked)
        }
    }

    /// Sets the lock status to `unlocked`/`false`
    pub fn try_unlock(&mut self, acquired_lock: bool) -> &mut Self {
        if acquired_lock {
            self.is_locked = false;
            self.last_locked_at = None;
        } else if let Some(last_locked_at) = self.last_locked_at {
            let current_time = time() / 1_000_000_000; // current time in millis

            if self.is_locked && current_time - last_locked_at > STRATEGY_LOCK_TIMEOUT {
                self.is_locked = false;
                self.last_locked_at = None;
            }
        }

        self
    }
}

/// Stable version of the lock with a built-in timeout mechanism.
/// #### Usage
/// Only used by `StableStrategy`
/// #### Implementation Note
/// Does not implement the `Drop` trait and `try_lock` and `try_unlock` methods.
#[derive(Clone, Default)]
pub struct StableLock {
    /// Status of the lock. `true` represents locked and `false` unlocked
    pub is_locked: bool,
    /// Last locked timstamp in milliseconds
    pub last_locked_at: Option<u64>,
}

impl From<StableLock> for Lock {
    fn from(value: StableLock) -> Self {
        Self {
            is_locked: value.is_locked,
            last_locked_at: value.last_locked_at,
        }
    }
}

impl From<Lock> for StableLock {
    fn from(value: Lock) -> Self {
        Self {
            is_locked: value.is_locked,
            last_locked_at: value.last_locked_at,
        }
    }
}
