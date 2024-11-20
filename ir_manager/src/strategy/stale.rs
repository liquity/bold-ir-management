//! Stale strategy implementation that is only used in the state

use crate::{
    state::STRATEGY_STATE,
    utils::error::{ManagerError, ManagerResult},
};

use super::{data::StrategyData, executable::ExecutableStrategy, settings::StrategySettings};

/// Stale strategy struct
#[derive(Clone, Default)]
pub struct StableStrategy {
    /// Immutable settings and configurations
    pub settings: StrategySettings,
    /// Mutable state
    pub data: StrategyData,
    /// Lock for the strategy. Determines if the strategy is currently being executed.
    pub lock: bool,
}

impl StableStrategy {
    /// Builder-style setter functions for the struct

    /// Set the strategy settings
    pub fn settings(&mut self, settings: StrategySettings) -> &mut Self {
        self.settings = settings;
        self
    }

    /// Set the strategy data
    pub fn data(&mut self, data: StrategyData) -> &mut Self {
        self.data = data;
        self
    }

    /// Mint the strategy by adding it to the state
    /// "Minting" here means registering the strategy in a persistent state.
    pub fn mint(&self) -> ManagerResult<()> {
        STRATEGY_STATE.with(|strategies| {
            let mut binding = strategies.borrow_mut();
            // Ensure that we do not overwrite an existing strategy with the same key
            if binding.get(&self.settings.key).is_some() {
                return Err(ManagerError::Custom(
                    "This strategy key is already mined.".to_string(),
                ));
            }
            binding.insert(self.settings.key, self.clone());
            Ok(())
        })
    }
}

impl From<&StableStrategy> for ExecutableStrategy {
    fn from(value: &StableStrategy) -> Self {
        ExecutableStrategy {
            settings: value.settings.clone(),
            data: value.data.clone(),
            lock: value.lock,
        }
    }
}

impl From<&ExecutableStrategy> for StableStrategy {
    fn from(value: &ExecutableStrategy) -> Self {
        StableStrategy {
            settings: value.settings.clone(),
            data: value.data.clone(),
            lock: value.lock,
        }
    }
}
