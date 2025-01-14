//! Mutable strategy data

use alloy_primitives::U256;

/// Struct containing all mutable data necessary to execute a strategy
#[derive(Clone, Default)]
pub struct StrategyData {
    /// Latest rate determined by the canister in the previous cycle
    pub latest_rate: U256,
    /// Timestamp of the last time the strategy had updated the batch's interest rate.
    /// Denominated in seconds.
    pub last_update: u64,
    /// The EOA's nonce
    pub eoa_nonce: u64,
    /// Timestamp of the last successful exit of the strategy
    pub last_ok_exit: u64
}

impl StrategyData {
    /// Sets the latest rate for the strategy.
    pub fn latest_rate(&mut self, latest_rate: U256) -> &mut Self {
        self.latest_rate = latest_rate;
        self
    }

    /// Sets the last update timestamp for the strategy.
    pub fn last_update(&mut self, last_update: u64) -> &mut Self {
        self.last_update = last_update;
        self
    }

    /// Sets the EOA nonce for the strategy.
    pub fn eoa_nonce(&mut self, eoa_nonce: u64) -> &mut Self {
        self.eoa_nonce = eoa_nonce;
        self
    }

    /// Sets Timestamp of the last successful exit of the strategy.
    pub fn last_ok_exit(&mut self, time: u64) -> &mut Self {
        self.last_ok_exit = time;
        self
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
