use alloy_primitives::{Address, U256};

use crate::{types::DerivationPath, utils::evm_rpc::Service};

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
