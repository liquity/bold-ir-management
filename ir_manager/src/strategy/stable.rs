//! Stale strategy implementation that is only used in the state

use crate::{
    state::STRATEGY_STATE,
    utils::error::{ManagerError, ManagerResult},
};

use super::{
    data::StrategyData, executable::ExecutableStrategy, lock::{Lock, StableLock},
    settings::StrategySettings,
};

/// Stale strategy struct
#[derive(Clone, Default)]
pub struct StableStrategy {
    /// Immutable settings and configurations
    pub settings: StrategySettings,
    /// Mutable state
    pub data: StrategyData,
    /// Lock for the strategy. Determines if the strategy is currently being executed.
    pub lock: StableLock,
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
            lock: value.lock.clone().into(),
        }
    }
}

impl From<&ExecutableStrategy> for StableStrategy {
    fn from(value: &ExecutableStrategy) -> Self {
        StableStrategy {
            settings: value.settings.clone(),
            data: value.data.clone(),
            lock: value.lock.clone().into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        state::STRATEGY_STATE, strategy::data::StrategyData,
        strategy::executable::ExecutableStrategy, strategy::settings::StrategySettings,
        utils::error::ManagerError,
    };
    use alloy_primitives::{Address, U256};
    use std::collections::HashMap;

    #[test]
    fn test_builder_setters() {
        let key = 42;
        let batch_manager = Address::repeat_byte(0x11);
        let collateral_index = U256::from(100u64);
        let latest_rate = U256::from(12345u64);
        let last_update = 1700000000u64;

        let mut stable_strategy = StableStrategy::default();

        // Set settings
        let settings = StrategySettings {
            key,
            batch_manager,
            collateral_index,
            ..Default::default()
        };
        stable_strategy.settings(settings.clone());

        // Set data
        let data = StrategyData {
            latest_rate,
            last_update,
            ..Default::default()
        };
        stable_strategy.data(data.clone());

        assert_eq!(stable_strategy.settings.key, key);
        assert_eq!(stable_strategy.settings.batch_manager, batch_manager);
        assert_eq!(stable_strategy.data.latest_rate, latest_rate);
        assert_eq!(stable_strategy.data.last_update, last_update);
    }

    #[test]
    fn test_mint_strategy_success() {
        let key = 42;
        let mut stable_strategy = StableStrategy::default();
        stable_strategy.settings.key = key;

        // Ensure the state is empty
        STRATEGY_STATE.with(|state| *state.borrow_mut() = HashMap::new());

        // Mint the strategy
        let result = stable_strategy.mint();
        assert!(result.is_ok(), "Minting should succeed");

        // Verify the strategy is in the state
        STRATEGY_STATE.with(|state| {
            let borrowed_state = state.borrow();
            assert!(
                borrowed_state.contains_key(&key),
                "Strategy should be in the state"
            );
        });
    }

    #[test]
    fn test_mint_strategy_duplicate_key() {
        let key = 42;
        let mut stable_strategy_1 = StableStrategy::default();
        stable_strategy_1.settings.key = key;

        let mut stable_strategy_2 = StableStrategy::default();
        stable_strategy_2.settings.key = key;

        // Insert the first strategy into the state
        STRATEGY_STATE.with(|state| {
            state.borrow_mut().insert(key, stable_strategy_1.clone());
        });

        // Attempt to mint the second strategy with the same key
        let result = stable_strategy_2.mint();
        assert!(result.is_err(), "Minting should fail for duplicate key");
        if let Err(ManagerError::Custom(message)) = result {
            assert!(
                message.contains("already mined"),
                "Error message should indicate duplicate key"
            );
        }
    }

    #[test]
    fn test_conversion_to_executable_strategy() {
        let key = 42;
        let batch_manager = Address::repeat_byte(0x11);
        let latest_rate = U256::from(12345u64);
        let last_update = 1700000000u64;

        let stable_strategy = StableStrategy {
            settings: StrategySettings {
                key,
                batch_manager,
                ..Default::default()
            },
            data: StrategyData {
                latest_rate,
                last_update,
                ..Default::default()
            },
            lock: true,
        };

        let executable_strategy: ExecutableStrategy = (&stable_strategy).into();

        assert_eq!(executable_strategy.settings.key, key);
        assert_eq!(executable_strategy.settings.batch_manager, batch_manager);
        assert_eq!(executable_strategy.data.latest_rate, latest_rate);
        assert_eq!(executable_strategy.data.last_update, last_update);
        assert!(executable_strategy.lock, "Lock should be set to true");
    }

    #[test]
    fn test_conversion_to_stable_strategy() {
        let key = 42;
        let batch_manager = Address::repeat_byte(0x11);
        let latest_rate = U256::from(12345u64);
        let last_update = 1700000000u64;

        let executable_strategy = ExecutableStrategy {
            settings: StrategySettings {
                key,
                batch_manager,
                ..Default::default()
            },
            data: StrategyData {
                latest_rate,
                last_update,
                ..Default::default()
            },
            lock: false,
        };

        let stable_strategy: StableStrategy = (&executable_strategy).into();

        assert_eq!(stable_strategy.settings.key, key);
        assert_eq!(stable_strategy.settings.batch_manager, batch_manager);
        assert_eq!(stable_strategy.data.latest_rate, latest_rate);
        assert_eq!(stable_strategy.data.last_update, last_update);
        assert!(!stable_strategy.lock, "Lock should be set to false");
    }
}
