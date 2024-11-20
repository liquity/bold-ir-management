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
}
