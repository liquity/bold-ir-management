//! Strategy State Management
//!
//! Manages the mutable state of strategy execution, providing thread-safe access
//! to critical runtime data and serialization-friendly query representations.
//!
//! ```plain
//! State Component Layout:
//!
//!                                     StrategyData
//!                                    ┌────────────────┐
//!                                    │   Rate State   │
//!                          ┌────────►│  latest_rate   │
//!                          │         └────────────────┘
//!                          │
//! ┌─────────────┐    ┌─────────┐     ┌────────────────┐
//! │   Query     │◄───┤ Runtime │     │  Time State    │
//! │ Conversion  │    │  State  │     │  last_update   │
//! └─────────────┘    └─────────┘     │  last_ok_exit  │
//!                          │         └────────────────┘
//!                          │
//!                          │         ┌────────────────┐
//!                          └────────►│    EOA State   │
//!                                    │   eoa_nonce    │
//!                                    └────────────────┘
//! ```

use alloy_primitives::U256;
use candid::CandidType;
use chrono::{DateTime, Utc};
use ic_exports::ic_cdk::api::time;

use crate::utils::{common::u256_to_nat, error::ManagerError};

/// Core strategy runtime state containing mutable execution data.
///
/// Tracks three key state components:
/// - Interest rate state (latest applied rate)
/// - Timing state (update/exit timestamps)
/// - Transaction state (nonce management)
#[derive(Clone, Default)]
pub struct StrategyData {
    /// Current interest rate from last execution
    pub latest_rate: U256,
    /// Last rate update timestamp (seconds)
    pub last_update: u64,
    /// Current EOA transaction nonce
    pub eoa_nonce: u64,
    /// Last successful strategy completion
    pub last_ok_exit: u64,
}

impl StrategyData {
    /// Updates the current interest rate.
    ///
    /// Used during rate adjustments to track applied changes.
    pub fn latest_rate(&mut self, latest_rate: U256) -> &mut Self {
        self.latest_rate = latest_rate;
        self
    }

    /// Records rate update timestamp.
    ///
    /// Tracks timing for upfront fee calculations.
    pub fn last_update(&mut self, last_update: u64) -> &mut Self {
        self.last_update = last_update;
        self
    }

    /// Manages EOA nonce for transaction sequencing.
    pub fn eoa_nonce(&mut self, eoa_nonce: u64) -> &mut Self {
        self.eoa_nonce = eoa_nonce;
        self
    }

    /// Records successful strategy completion time.
    pub fn record_last_ok_exit(&mut self) -> &mut Self {
        self.last_ok_exit = time() / 1_000_000_000;
        self
    }
}

/// Serialization-optimized view of strategy state for external queries.
///
/// Provides Candid-compatible types while maintaining semantic equivalence
/// with internal state representation.
#[derive(Clone, Default, CandidType)]
pub struct StrategyDataQuery {
    /// Interest rate in Candid-compatible format
    pub latest_rate: candid::Nat,
    /// Last update time
    pub last_update: String,
    /// Current transaction nonce
    pub eoa_nonce: u64,
    /// Last successful completion time
    pub last_ok_exit: String,
}

/// Validated conversion from runtime to query state
impl TryFrom<StrategyData> for StrategyDataQuery {
    type Error = ManagerError;

    fn try_from(value: StrategyData) -> Result<Self, Self::Error> {
        let last_update_datetime = DateTime::<Utc>::from_timestamp(value.last_update as i64, 0)
            .expect("Invalid timestamp");
        let last_update = last_update_datetime.format("%d-%m-%Y %H:%M:%S").to_string();
        let last_ok_exit_datetime = DateTime::<Utc>::from_timestamp(value.last_ok_exit as i64, 0)
            .expect("Invalid timestamp");
        let last_ok_exit = last_ok_exit_datetime
            .format("%d-%m-%Y %H:%M:%S")
            .to_string();

        Ok(Self {
            latest_rate: u256_to_nat(&value.latest_rate)?,
            last_update,
            eoa_nonce: value.eoa_nonce,
            last_ok_exit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::U256;
    use proptest::prelude::*;

    #[test]
    fn test_strategy_data_setters() {
        let mut data = StrategyData::default();

        let latest_rate = U256::from(12345u64);
        let last_update = 1700000000u64; // Example Unix timestamp
        let eoa_nonce = 42u64;

        // Use setters
        data.latest_rate(latest_rate)
            .last_update(last_update)
            .eoa_nonce(eoa_nonce);

        // Check values
        assert_eq!(data.latest_rate, latest_rate);
        assert_eq!(data.last_update, last_update);
        assert_eq!(data.eoa_nonce, eoa_nonce);
    }

    // Property-based testing for StrategyData
    proptest! {
        #[test]
        fn test_strategy_data_proptest(
            latest_rate in any::<u64>(),
            last_update in any::<u64>(),
            eoa_nonce in any::<u64>(),
        ) {
            let mut data = StrategyData::default();

            let latest_rate = U256::from(latest_rate);

            data.latest_rate(latest_rate)
                .last_update(last_update)
                .eoa_nonce(eoa_nonce);

            prop_assert_eq!(data.latest_rate, latest_rate);
            prop_assert_eq!(data.last_update, last_update);
            prop_assert_eq!(data.eoa_nonce, eoa_nonce);
        }
    }
}
