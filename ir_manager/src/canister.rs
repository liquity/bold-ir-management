//! The canister's public methods

use std::{str::FromStr, sync::Arc, time::Duration};

use crate::constants::MAX_RETRY_ATTEMPTS;
use crate::journal::JournalEntry;
use crate::strategy::data::StrategyData;
use crate::strategy::executable::ExecutableStrategy;
use crate::strategy::settings::StrategySettings;
use crate::utils::common::*;
use crate::utils::error::*;
use crate::utils::evm_rpc::Service;
use crate::utils::signer::*;
use crate::{
    charger::{check_threshold, recharge_cketh, transfer_cketh, SwapLock},
    state::*,
    types::{StrategyInput, StrategyQueryData, SwapResponse},
};
use alloy_primitives::Address;
use ic_canister::{generate_idl, query, update, Canister, Idl, PreUpdate};
use ic_exports::{
    candid::Principal,
    ic_cdk::{
        api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
        caller, spawn,
    },
    ic_cdk_timers::set_timer_interval,
};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    #[update]
    pub async fn mint_strategy(&self, strategy: StrategyInput) -> ManagerResult<String> {
        only_controller(caller())?;

        let strategies = STRATEGY_STATE.with(|strategies| strategies.borrow().clone());

        if strategies.contains_key(&strategy.key) {
            return Err(ManagerError::Custom(
                "This key is already being used.".to_string(),
            ));
        }

        let manager = string_to_address(strategy.manager)?;
        MANAGERS.with(|managers| managers.borrow_mut().push(manager));

        let derivation_path = vec![strategy.key.to_be_bytes().to_vec()];
        let key_id = EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: String::from("key_1"),
        };
        let public_key_bytes =
            get_canister_public_key(key_id, None, Some(derivation_path.clone())).await;
        let eoa_pk = Address::from_str(&pubkey_bytes_to_address(&public_key_bytes)).unwrap();
        let rpc_canister = Service(strategy.rpc_principal);

        // Convert String addresses to Address ones
        let collateral_registry_address = string_to_address(strategy.collateral_registry)?;
        let multi_trove_getter_address = string_to_address(strategy.multi_trove_getter)?;
        let hint_helper_address = string_to_address(strategy.hint_helper)?;

        // Convert Nat values to U256 ones
        let target_min_u256 = nat_to_u256(&strategy.target_min)?;
        let upfront_fee_period_u256 = nat_to_u256(&strategy.upfront_fee_period)?;
        let collateral_index_u256 = nat_to_u256(&strategy.collateral_index)?;

        let strategy_settings = StrategySettings::default()
            .key(strategy.key)
            .manager(manager)
            .collateral_registry(collateral_registry_address)
            .multi_trove_getter(multi_trove_getter_address)
            .hint_helper(hint_helper_address)
            .upfront_fee_period(upfront_fee_period_u256)
            .collateral_index(collateral_index_u256)
            .eoa_pk(Some(eoa_pk))
            .derivation_path(derivation_path)
            .target_min(target_min_u256)
            .rpc_canister(rpc_canister)
            .clone();

        let strategy_data = StrategyData::default();

        ExecutableStrategy::default()
            .settings(strategy_settings)
            .data(strategy_data)
            .mint()?;

        Ok(eoa_pk.to_string())
    }

    #[update]
    pub async fn set_batch_manager(&self, key: u32, batch_manager: String) -> ManagerResult<()> {
        only_controller(caller())?;
        let batch_manager_address = string_to_address(batch_manager)?;
        STRATEGY_STATE.with(|strategies| {
            let mut binding = strategies.borrow_mut();
            let strategy = binding
                .get_mut(&key)
                .ok_or(ManagerError::NonExistentValue)?;
            strategy.settings.batch_manager = batch_manager_address;
            Ok(())
        })
    }

    /// Starts timers for executing strategies and managing the canister's cycle balance.
    /// Each strategy executes on a 1-hour interval, and cycle balance checks happen every 24 hours.
    #[update]
    pub async fn start_timers(&self) -> ManagerResult<()> {
        only_controller(caller())?;
        // Retrieve all strategies for setting up timers
        let strategies = STRATEGY_STATE.with(|vector_data| vector_data.borrow().clone());
        let max_retry_attempts = Arc::new(MAX_RETRY_ATTEMPTS);

        // Start all strategies immediately
        strategies
            .clone()
            .into_iter()
            .for_each(|(id, mut strategy)| {
                let max_retry_attempts = Arc::clone(&max_retry_attempts);
                spawn(async move {
                    for turn in 1..=*max_retry_attempts {
                        let result = strategy.execute().await;
                        // log the result
                        JournalEntry::new(result.clone())
                            .strategy(id)
                            .turn(turn)
                            .commit();

                        // Handle success or failure for each strategy execution attempt
                        match result {
                            Ok(()) => break,
                            Err(_) => {
                                strategy.unlock(); // Unlock on failure
                            }
                        }
                    }
                });
            });

        // Set timers for each strategy (execute every 1 hour)
        strategies.into_iter().for_each(|(id, strategy)| {
            let max_retry_attempts = Arc::clone(&max_retry_attempts);

            set_timer_interval(Duration::from_secs(3_600), move || {
                let mut strategy = strategy.clone();
                let max_retry_attempts = Arc::clone(&max_retry_attempts);
                spawn(async move {
                    for turn in 1..=*max_retry_attempts {
                        let result = strategy.execute().await;
                        // log the result
                        JournalEntry::new(result.clone())
                            .strategy(id)
                            .turn(turn)
                            .commit();

                        // Handle success or failure for each strategy execution attempt
                        match result {
                            Ok(()) => break, // Exit on success
                            Err(_) => {
                                strategy.unlock();
                            }
                        }
                    }
                });
            });
        });

        // Set a recurring timer for recharging ckETH balance (execute every 24 hours)
        set_timer_interval(Duration::from_secs(86_400), move || {
            let max_retry_attempts = Arc::clone(&max_retry_attempts);
            spawn(async move {
                for turn in 1..=*max_retry_attempts {
                    let result = recharge_cketh().await;
                    // log the result
                    JournalEntry::new(result.clone())
                        .turn(turn)
                        .note("ckETH recharging cycle")
                        .commit();

                    if result.is_ok() {
                        break;
                    }
                }
            });
        });

        Ok(())
    }

    /// Retrieves a list of strategies currently stored in the state.
    #[update]
    pub fn get_strategies(&self) -> Vec<StrategyQueryData> {
        STRATEGY_STATE.with(|vector_data| {
            let binding = vector_data.borrow();
            let values = binding.values();
            if values.len() == 0 {
                return vec![];
            }
            values
                .map(|strategy| StrategyQueryData::from(strategy.clone()))
                .collect()
        })
    }

    /// Returns the strategy EOA
    #[update]
    pub fn get_strategy_address(&self, index: u32) -> Option<String> {
        STRATEGY_STATE.with(|data| {
            data.borrow()
                .get(&index)
                .and_then(|strategy| strategy.settings.eoa_pk.map(|pk| pk.to_string()))
        })
    }

    /// Swaps ckETH by first checking the cycle balance, then transferring ckETH to the caller.
    #[update]
    pub async fn swap_cketh(&self) -> ManagerResult<SwapResponse> {
        // Ensure the cycle balance is above a certain threshold before proceeding
        let mut swap_lock = SwapLock::default();
        swap_lock.lock()?;
        check_threshold().await?;
        transfer_cketh(caller()).await
    }

    #[query]
    pub async fn get_logs(&self, depth: u64) -> ManagerResult<Vec<JournalEntry>> {
        let entries = JOURNAL.with(|m| m.borrow().iter().collect::<Vec<JournalEntry>>());

        Ok(entries[entries.len().saturating_sub(depth as usize)..].to_vec())
    }

    /// Generates the IDL for the canister interface.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}
