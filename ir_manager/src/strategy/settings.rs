//! Lazily initialized strategy settings

use alloy_primitives::{Address, U256};
use candid::{CandidType, Nat};

use crate::{
    types::DerivationPath,
    utils::{common::u256_to_nat, error::ManagerError, evm_rpc::Service},
};

/// Lazily initialized settings
/// These settings are only set once after spawning with their default values
#[derive(Clone, Default)]
pub struct StrategySettings {
    /// Key in the Hashmap<u32, StrategyData> that is `STRATEGY_DATA`
    pub key: u32,
    /// Batch manager contract address for this strategy
    pub batch_manager: Address,
    /// Hint helper contract address.
    pub hint_helper: Address,
    /// Manager contract address for this strategy
    pub manager: Address,
    /// Collateral registry contract address
    pub collateral_registry: Address,
    /// Multi trove getter contract address for this strategy
    pub multi_trove_getter: Address,
    /// Sorted troves contract address for this strategy
    pub sorted_troves: Address,
    /// Collateral index
    pub collateral_index: U256,
    /// Derivation path of the ECDSA signature
    pub derivation_path: DerivationPath,
    /// Minimum target for this strategy
    pub target_min: U256,
    /// Upfront fee period constant denominated in seconds
    pub upfront_fee_period: U256,
    /// The EOA's public key
    pub eoa_pk: Option<Address>,
    /// RPC canister service
    pub rpc_canister: Service,
}

impl StrategySettings {
    /// Sets the key for the strategy.
    pub fn key(&mut self, key: u32) -> &mut Self {
        self.key = key;
        self
    }

    /// Sets the batch manager contract address for this strategy.
    pub fn batch_manager(&mut self, batch_manager: Address) -> &mut Self {
        self.batch_manager = batch_manager;
        self
    }

    /// Sets the hint helper contract address for this strategy.
    pub fn hint_helper(&mut self, hint_helper: Address) -> &mut Self {
        self.hint_helper = hint_helper;
        self
    }

    /// Sets the manager contract address for this strategy.
    pub fn manager(&mut self, manager: Address) -> &mut Self {
        self.manager = manager;
        self
    }

    /// Sets the collateral registry contract address for this strategy.
    pub fn collateral_registry(&mut self, collateral_registry: Address) -> &mut Self {
        self.collateral_registry = collateral_registry;
        self
    }

    /// Sets the multi-trove getter contract address for this strategy.
    pub fn multi_trove_getter(&mut self, multi_trove_getter: Address) -> &mut Self {
        self.multi_trove_getter = multi_trove_getter;
        self
    }

    /// Sets the sorted troves contract address for this strategy.
    pub fn sorted_troves(&mut self, sorted_troves: Address) -> &mut Self {
        self.sorted_troves = sorted_troves;
        self
    }

    /// Sets the collateral index for this strategy.
    pub fn collateral_index(&mut self, collateral_index: U256) -> &mut Self {
        self.collateral_index = collateral_index;
        self
    }

    /// Sets the derivation path of the ECDSA signature for this strategy.
    pub fn derivation_path(&mut self, derivation_path: DerivationPath) -> &mut Self {
        self.derivation_path = derivation_path;
        self
    }

    /// Sets the minimum target for this strategy.
    pub fn target_min(&mut self, target_min: U256) -> &mut Self {
        self.target_min = target_min;
        self
    }

    /// Sets the upfront fee period constant, denominated in seconds.
    pub fn upfront_fee_period(&mut self, upfront_fee_period: U256) -> &mut Self {
        self.upfront_fee_period = upfront_fee_period;
        self
    }

    /// Sets the EOA public key for the strategy.
    pub fn eoa_pk(&mut self, eoa_pk: Option<Address>) -> &mut Self {
        self.eoa_pk = eoa_pk;
        self
    }

    /// Sets the RPC canister service for the strategy.
    pub fn rpc_canister(&mut self, rpc_canister: Service) -> &mut Self {
        self.rpc_canister = rpc_canister;
        self
    }
}

/// Lazily initialized settings
/// These settings are only set once after spawning with their default values
#[derive(Clone, Default, CandidType)]
pub struct StrategySettingsQuery {
    /// Key in the Hashmap<u32, StrategyData> that is `STRATEGY_DATA`
    pub key: u32,
    /// Batch manager contract address for this strategy
    pub batch_manager: String,
    /// Hint helper contract address.
    pub hint_helper: String,
    /// Manager contract address for this strategy
    pub manager: String,
    /// Collateral registry contract address
    pub collateral_registry: String,
    /// Multi trove getter contract address for this strategy
    pub multi_trove_getter: String,
    /// Sorted troves contract address for this strategy
    pub sorted_troves: String,
    /// Collateral index
    pub collateral_index: Nat,
    /// Minimum target for this strategy
    pub target_min: Nat,
    /// Upfront fee period constant denominated in seconds
    pub upfront_fee_period: Nat,
    /// The EOA's public key
    pub eoa_pk: Option<String>,
}

impl TryFrom<StrategySettings> for StrategySettingsQuery {
    type Error = ManagerError;

    fn try_from(value: StrategySettings) -> Result<Self, Self::Error> {
        Ok(Self {
            key: value.key,
            batch_manager: value.batch_manager.to_string(),
            hint_helper: value.hint_helper.to_string(),
            manager: value.manager.to_string(),
            collateral_registry: value.collateral_registry.to_string(),
            multi_trove_getter: value.multi_trove_getter.to_string(),
            sorted_troves: value.sorted_troves.to_string(),
            collateral_index: u256_to_nat(&value.collateral_index)?,
            target_min: u256_to_nat(&value.target_min)?,
            upfront_fee_period: u256_to_nat(&value.upfront_fee_period)?,
            eoa_pk: value.eoa_pk.map(|address| address.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, U256};
    use proptest::prelude::*;

    // Mock Service for testing
    use crate::utils::evm_rpc::Service;

    #[test]
    fn test_strategy_settings_setters() {
        let mut settings = StrategySettings::default();

        let key = 42;
        let batch_manager = Address::repeat_byte(0x11);
        let hint_helper = Address::repeat_byte(0x22);
        let manager = Address::repeat_byte(0x33);
        let collateral_registry = Address::repeat_byte(0x44);
        let multi_trove_getter = Address::repeat_byte(0x55);
        let collateral_index = U256::from(100u64);
        let derivation_path = vec![vec![1u8, 2u8, 3u8]];
        let target_min = U256::from(500u64);
        let upfront_fee_period = U256::from(3600u64);
        let eoa_pk = Some(Address::repeat_byte(0x66));
        let rpc_service = Service::default();

        settings
            .key(key)
            .batch_manager(batch_manager)
            .hint_helper(hint_helper)
            .manager(manager)
            .collateral_registry(collateral_registry)
            .multi_trove_getter(multi_trove_getter)
            .collateral_index(collateral_index)
            .derivation_path(derivation_path.clone())
            .target_min(target_min)
            .upfront_fee_period(upfront_fee_period)
            .eoa_pk(eoa_pk)
            .rpc_canister(rpc_service.clone());

        assert_eq!(settings.key, key);
        assert_eq!(settings.batch_manager, batch_manager);
        assert_eq!(settings.hint_helper, hint_helper);
        assert_eq!(settings.manager, manager);
        assert_eq!(settings.collateral_registry, collateral_registry);
        assert_eq!(settings.multi_trove_getter, multi_trove_getter);
        assert_eq!(settings.collateral_index, collateral_index);
        assert_eq!(settings.derivation_path, derivation_path);
        assert_eq!(settings.target_min, target_min);
        assert_eq!(settings.upfront_fee_period, upfront_fee_period);
        assert_eq!(settings.eoa_pk, eoa_pk);
    }

    // Property-based test for StrategySettings setters
    proptest! {
        #[test]
        fn test_strategy_settings_proptest(
            key in 0u32..u32::MAX,
            batch_manager in any::<[u8; 20]>(),
            hint_helper in any::<[u8; 20]>(),
            manager in any::<[u8; 20]>(),
            collateral_registry in any::<[u8; 20]>(),
            multi_trove_getter in any::<[u8; 20]>(),
            collateral_index in any::<u64>(),
            target_min in any::<u64>(),
            upfront_fee_period in any::<u64>(),
        ) {
            let mut settings = StrategySettings::default();

            let batch_manager = Address::from_slice(&batch_manager);
            let hint_helper = Address::from_slice(&hint_helper);
            let manager = Address::from_slice(&manager);
            let collateral_registry = Address::from_slice(&collateral_registry);
            let multi_trove_getter = Address::from_slice(&multi_trove_getter);
            let collateral_index = U256::from(collateral_index);
            let target_min = U256::from(target_min);
            let upfront_fee_period = U256::from(upfront_fee_period);

            settings.key(key)
                .batch_manager(batch_manager)
                .hint_helper(hint_helper)
                .manager(manager)
                .collateral_registry(collateral_registry)
                .multi_trove_getter(multi_trove_getter)
                .collateral_index(collateral_index)
                .target_min(target_min)
                .upfront_fee_period(upfront_fee_period);

            prop_assert_eq!(settings.key, key);
            prop_assert_eq!(settings.batch_manager, batch_manager);
            prop_assert_eq!(settings.hint_helper, hint_helper);
            prop_assert_eq!(settings.manager, manager);
            prop_assert_eq!(settings.collateral_registry, collateral_registry);
            prop_assert_eq!(settings.multi_trove_getter, multi_trove_getter);
            prop_assert_eq!(settings.collateral_index, collateral_index);
            prop_assert_eq!(settings.target_min, target_min);
            prop_assert_eq!(settings.upfront_fee_period, upfront_fee_period);
        }
    }
}
