//! Persistent Strategy State Management
//!
//! A stable strategy implementation designed for permanent storage with optimized data structures
//! and controlled state transitions. This module acts as the source of truth for all strategy data.
//!
//! ```plain
//! Strategy State Flow:
//!                                                    
//!           ┌──────────┐         ┌──────────┐         ┌──────────┐
//! Create -> │  Stable  │ ─Into─> │Executable│ Process │  Stable  │
//!           │ Strategy │ <─From─ │ Strategy │ ──Into─>│ Strategy │
//!           └──────────┘         └──────────┘         └──────────┘
//!                │                                          │
//!                │              ┌─────────┐                 │
//!                └─TryInto────> │  Query  │ <─────TryInto──┘
//!                               │ Strategy│
//!                               └─────────┘
//! ```

use candid::CandidType;

use crate::{
    state::STRATEGY_STATE,
    utils::error::{ManagerError, ManagerResult},
};

use super::{
    data::{StrategyData, StrategyDataQuery},
    executable::ExecutableStrategy,
    lock::{LockQuery, StableLock},
    settings::{StrategySettings, StrategySettingsQuery},
};

/// A persistent strategy representation optimized for stable storage and state management.
///
/// This structure provides:
/// - Immutable configuration via `settings`
/// - Mutable runtime state via `data`
/// - Atomic execution control via `lock`
///
/// The stable strategy serves as the canonical source of truth, while executable strategies
/// handle runtime operations.
#[derive(Clone, Default)]
pub struct StableStrategy {
    /// Core configuration parameters that remain constant after initialization
    pub settings: StrategySettings,
    /// Dynamic state that changes during strategy execution
    pub data: StrategyData,
    /// Atomic execution lock to prevent concurrent operations
    pub lock: StableLock,
}

impl StableStrategy {
    /// Configures strategy settings using builder pattern.
    ///
    /// # Arguments
    /// * `settings` - Core configuration parameters
    ///
    /// # Returns
    /// Mutable reference for method chaining
    pub fn settings(&mut self, settings: StrategySettings) -> &mut Self {
        self.settings = settings;
        self
    }

    /// Updates strategy runtime data.
    ///
    /// # Arguments
    /// * `data` - New runtime state
    ///
    /// # Returns
    /// Mutable reference for method chaining
    pub fn data(&mut self, data: StrategyData) -> &mut Self {
        self.data = data;
        self
    }

    /// Persists the strategy in stable storage.
    ///
    /// Performs atomic registration ensuring:
    /// - No key collisions
    /// - Consistent state
    /// - Persistent storage
    ///
    /// # Returns
    /// * `Ok(())` - Strategy successfully registered
    /// * `Err(ManagerError)` - Registration failed due to key collision
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

/// Bidirectional conversion between stable and executable strategies
impl From<&StableStrategy> for ExecutableStrategy {
    fn from(value: &StableStrategy) -> Self {
        ExecutableStrategy::new(
            value.settings.clone(),
            value.data.clone(),
            value.lock.clone().into(),
        )
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

/// Query-optimized strategy representation for external inspection.
///
/// This structure provides a serialization-friendly view of strategy state
/// while maintaining strict data validation during conversion.
#[derive(Clone, Default, CandidType)]
pub struct StableStrategyQuery {
    /// Validated configuration settings
    pub settings: StrategySettingsQuery,
    /// Sanitized runtime state
    pub data: StrategyDataQuery,
    /// Current execution lock status
    pub lock: LockQuery,
}

/// Validated conversion from full strategy to query representation
impl TryFrom<StableStrategy> for StableStrategyQuery {
    type Error = ManagerError;

    fn try_from(value: StableStrategy) -> Result<Self, Self::Error> {
        let settings = StrategySettingsQuery::try_from(value.settings)?;
        let data = StrategyDataQuery::try_from(value.data)?;
        let lock = LockQuery::try_from(value.lock)?;

        Ok(Self {
            settings,
            data,
            lock,
        })
    }
}
