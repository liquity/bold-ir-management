//! Strategy Locking System
//!
//! A timeout-based locking mechanism that prevents concurrent strategy execution
//! while providing automatic deadlock recovery. The system maintains both runtime
//! and persistent lock representations with safe state transitions.
//!
//! ```plain
//! Lock State Machine:
//!
//!                   ┌──────────┐
//!              ┌────► Unlocked │◄─────┐
//!              │    └──────────┘      │
//!              │         │            │
//! Auto-Unlock  │     try_lock        try_unlock
//! (Timeout)    │         │            │
//!              │         ▼            │
//!              │    ┌─────────┐       │
//!              └────┤ Locked  ├───────┘
//!                   └─────────┘
//!
//! Timeout = STRATEGY_LOCK_TIMEOUT (3600s)
//! ```

use candid::CandidType;
use ic_exports::ic_cdk::api::time;

use crate::{
    constants::STRATEGY_LOCK_TIMEOUT,
    utils::error::{ManagerError, ManagerResult},
};

/// Runtime lock implementation with automatic timeout recovery.
///
/// Key features:
/// - Atomic lock operations
/// - Timeout-based deadlock prevention
/// - Last access tracking
/// - Builder pattern interface
#[derive(Clone, Default)]
pub struct Lock {
    /// Current lock state
    pub is_locked: bool,
    /// Last successful lock acquisition time
    pub last_locked_at: Option<u64>,
}

impl Lock {
    /// Attempts to acquire the lock with timeout validation.
    ///
    /// Succeeds if either:
    /// 1. Lock is currently free (unlocked)
    /// 2. Existing lock has exceeded timeout period
    ///
    /// # Returns
    /// * `Ok(())` - Lock successfully acquired
    /// * `Err(ManagerError::Locked)` - Lock unavailable
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

    /// Releases the lock if it was legitimately acquired.
    ///
    /// Also handles timeout-based cleanup of abandoned locks.
    ///
    /// # Arguments
    /// * `acquired_lock` - Whether the caller previously acquired the lock
    ///
    /// # Returns
    /// Mutable reference for method chaining
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

/// Persistent lock state for stable storage.
///
/// Provides:
/// - Candid serialization support
/// - Minimal memory footprint
/// - Direct state access
///
/// Note: Does not implement locking logic.
#[derive(Clone, Default, CandidType)]
pub struct StableLock {
    /// Status of the lock. `true` represents locked and `false` unlocked
    pub is_locked: bool,
    /// Last locked timstamp in milliseconds
    pub last_locked_at: Option<u64>,
}

/// Conversion from storage to runtime lock
impl From<StableLock> for Lock {
    fn from(value: StableLock) -> Self {
        Self {
            is_locked: value.is_locked,
            last_locked_at: value.last_locked_at,
        }
    }
}

/// Conversion from runtime to storage lock
impl From<Lock> for StableLock {
    fn from(value: Lock) -> Self {
        Self {
            is_locked: value.is_locked,
            last_locked_at: value.last_locked_at,
        }
    }
}
