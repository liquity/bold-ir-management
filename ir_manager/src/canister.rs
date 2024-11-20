use std::{str::FromStr, sync::Arc, time::Duration};

use crate::journal::JournalEntry;
use crate::utils::common::*;
use crate::utils::error::*;
use crate::utils::evm_rpc::Service;
use crate::utils::signer::*;
use crate::{
    charger::{check_threshold, recharge_cketh, transfer_cketh, SwapLock},
    state::*,
    strategy::StrategyData,
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

        let strategies = STRATEGY_DATA.with(|strategies| strategies.borrow().clone());

        if strategies.get(&strategy.key).is_some() {
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
        let strategy_data = StrategyData::new(
            strategy.key,
            manager,
            strategy.collateral_registry,
            strategy.multi_trove_getter,
            nat_to_u256(&strategy.target_min)?,
            rpc_canister,
            nat_to_u256(&strategy.upfront_fee_period)?,
            nat_to_u256(&strategy.collateral_index)?,
            strategy.hint_helper,
            Some(eoa_pk),
            derivation_path,
        )?;

        strategy_data.mint()?;

        Ok(eoa_pk.to_string())
    }

    #[update]
    pub async fn set_batch_manager(&self, key: u32, batch_manager: String) -> ManagerResult<()> {
        only_controller(caller())?;
        let address = Address::from_str(&batch_manager).unwrap();
        StrategyData::set_batch_manager(key, address)
    }

    /// Starts timers for executing strategies and managing the canister's cycle balance.
    /// Each strategy executes on a 1-hour interval, and cycle balance checks happen every 24 hours.
    #[update]
    pub async fn start_timers(&self) -> ManagerResult<()> {
        only_controller(caller())?;
        // Retrieve all strategies for setting up timers
        let strategies = STRATEGY_DATA.with(|vector_data| vector_data.borrow().clone());
        let max_retry_attempts = Arc::new(MAX_RETRY_ATTEMPTS.with(|attempts| attempts.get()));

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
                                let _ = strategy.unlock(); // Unlock on failure
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
                                let _ = strategy.unlock();
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
        STRATEGY_DATA.with(|vector_data| {
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
        STRATEGY_DATA.with(|data| {
            let binding = data.borrow();
            match binding.get(&index) {
                Some(strategy) => {
                    if let Some(pk) = strategy.eoa_pk {
                        return Some(pk.to_string());
                    }
                    None
                }
                None => None,
            }
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
        let state = JOURNAL.with(|m| {
            m.borrow()
                .iter()
                .map(|s| s.clone())
                .collect::<Vec<JournalEntry>>()
        });

        let entries: Vec<JournalEntry> = state
            .try_into()
            .map_err(|err| ManagerError::Custom(format!("{:#?}", err)))?;

        Ok(entries[entries.len().saturating_sub(depth as usize)..].to_vec())
    }

    /// Generates the IDL for the canister interface.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}
