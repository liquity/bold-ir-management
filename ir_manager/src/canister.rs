//! The canister's public methods

use std::{sync::Arc, time::Duration};

use crate::constants::MAX_RETRY_ATTEMPTS;
use crate::constants::MINIMUM_ATTACHED_CYCLES;
use crate::journal::JournalCollection;
use crate::journal::StableJournalCollection;
use crate::strategy::data::StrategyData;
use crate::strategy::run::run_strategy;
use crate::strategy::settings::StrategySettings;
use crate::strategy::stale::StableStrategy;
use crate::utils::common::*;
use crate::utils::error::*;
use crate::utils::evm_rpc::Service;
use crate::utils::signer::*;
use crate::{
    charger::{check_threshold, recharge_cketh, transfer_cketh, SwapLock},
    state::*,
    types::{StrategyInput, StrategyQueryData, SwapResponse},
};
use candid::Nat;
use ic_canister::{generate_idl, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk::api::call::msg_cycles_available;
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
    /// Mints a new strategy with the provided input.
    /// This function checks if the strategy key is already in use and if not, it creates a new strategy.
    /// It also handles the necessary conversions for addresses and values.
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
        let eoa_pk = string_to_address(pubkey_bytes_to_address(&public_key_bytes))?;
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

        // The following line sets the nonce, latest rate, and latest update timestamp to 0.
        // We don't care about any of those at this point.
        // The nonce will be recalculated.
        // The latest rate will be adjusted when the `set_batch_manager` function is called.
        // The timestamp will stay as 0 until the first strategy rate adjustment tx is sent.
        let strategy_data = StrategyData::default();

        StableStrategy::default()
            .settings(strategy_settings)
            .data(strategy_data)
            .mint()?;

        Ok(eoa_pk.to_string())
    }

    /// Sets the batch manager for a given strategy key.
    /// This function ensures that only the controller can call it and updates the strategy's settings.
    #[update]
    pub async fn set_batch_manager(
        &self,
        key: u32,
        batch_manager: String,
        current_rate: Nat,
    ) -> ManagerResult<()> {
        only_controller(caller())?;
        let batch_manager_address = string_to_address(batch_manager)?;
        STRATEGY_STATE.with(|strategies| {
            let mut binding = strategies.borrow_mut();
            let strategy = binding
                .get_mut(&key)
                .ok_or(ManagerError::NonExistentValue)?;
            strategy.settings.batch_manager = batch_manager_address;
            strategy.data.latest_rate = nat_to_u256(&current_rate)?;
            Ok(())
        })
    }

    /// Starts timers for executing strategies and managing the canister's cycle balance.
    /// Each strategy executes on a 1-hour interval, and cycle balance checks happen every 24 hours.
    #[update]
    pub async fn start_timers(&self) -> ManagerResult<()> {
        only_controller(caller())?;
        // Retrieve all strategies for setting up timers
        let strategies: Vec<u32> = STRATEGY_STATE
            .with(|vector_data| vector_data.borrow().iter().map(|(key, _)| *key).collect());

        let max_retry_attempts = Arc::new(MAX_RETRY_ATTEMPTS);

        // Start all strategies immediately
        strategies.clone().into_iter().for_each(|key| {
            spawn(run_strategy(key));
        });

        // Set timers for each strategy (execute every 1 hour)
        strategies.into_iter().for_each(|key| {
            set_timer_interval(Duration::from_secs(3_600), move || {
                spawn(run_strategy(key));
            });
        });

        // Set a recurring timer for recharging ckETH balance (execute every 24 hours)
        set_timer_interval(Duration::from_secs(86_400), move || {
            let max_retry_attempts = Arc::clone(&max_retry_attempts);
            spawn(async move {
                let mut journal = JournalCollection::open(None);
                for turn in 1..=*max_retry_attempts {
                    let result = recharge_cketh().await;
                    // log the result
                    journal.append_note(
                        result.clone(),
                        crate::journal::LogType::Recharge,
                        format!("Turn {}/{}", turn, max_retry_attempts),
                    );

                    if result.is_ok() {
                        break;
                    }
                }
            });
        });

        // Recurring timer (24h) that:
        // - clears all reputation change logs and resets the reputations
        // - checks if the logs have more than 300 items, if so, clear the surplus
        set_timer_interval(Duration::from_secs(86_400), || {
            JOURNAL.with(|journal| {
                let mut binding = journal.borrow_mut();

                // Initialize a new StableVec safely and return if initialization fails
                let temp = if let Ok(vec) = ic_stable_structures::Vec::init(
                    ic_stable_structures::DefaultMemoryImpl::default(),
                ) {
                    vec
                } else {
                    return; // Exit if initialization fails
                };

                for collection in binding.iter() {
                    if !collection.is_reputation_change() {
                        let _ = temp.push(&collection.clone());
                    }
                }

                *binding = temp;
            });

            RPC_REPUTATIONS.with(|reputations| {
                *reputations.borrow_mut() = vec![
                    // AUDIT: The following enums will be replaced by the Ethereum main-net providers. Out of scope.
                    (0, evm_rpc_types::EthSepoliaService::Ankr),
                    (0, evm_rpc_types::EthSepoliaService::BlockPi),
                    (0, evm_rpc_types::EthSepoliaService::PublicNode),
                    (0, evm_rpc_types::EthSepoliaService::Sepolia),
                    (0, evm_rpc_types::EthSepoliaService::Alchemy),
                ]
            });

            JOURNAL.with(|journal| {
                let binding = journal.borrow_mut();

                // Check if the journal has more than 300 items
                let len = binding.len();
                if len > 300 {
                    let excess = len - 300;

                    // Shift all items to remove the oldest ones
                    for i in excess..len {
                        if let Some(item) = binding.get(i) {
                            binding.set(i - excess, &item);
                        }
                    }

                    // Pop the remaining items to resize the vector
                    for _ in 0..excess {
                        binding.pop();
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
        // Ensure the caller has attached enough cycles
        if msg_cycles_available() < MINIMUM_ATTACHED_CYCLES {
            return Err(ManagerError::Custom(format!(
                "The attached cycles amount ({}) is less than the minimum accepted amount ({})",
                msg_cycles_available(),
                MINIMUM_ATTACHED_CYCLES
            )));
        }
        // Ensure the cycle balance is above a certain threshold before proceeding
        let mut swap_lock = SwapLock::default();
        swap_lock.lock()?;
        check_threshold().await?;
        transfer_cketh(caller()).await
    }

    #[query]
    pub async fn get_logs(&self, depth: u64) -> ManagerResult<Vec<StableJournalCollection>> {
        let entries = JOURNAL.with(|m| m.borrow().iter().collect::<Vec<StableJournalCollection>>());

        Ok(entries[entries.len().saturating_sub(depth as usize)..].to_vec())
    }

    #[query]
    pub async fn get_strategy_logs(
        &self,
        depth: u64,
        strategy_key: u32,
    ) -> ManagerResult<Vec<StableJournalCollection>> {
        // Filter the journal entries by strategy_key
        let entries: Vec<StableJournalCollection> = JOURNAL.with(|n| {
            n.borrow()
                .iter()
                .filter(|entry| entry.strategy == Some(strategy_key))
                .collect()
        });

        // Limit the results to the desired depth
        Ok(entries[entries.len().saturating_sub(depth as usize)..].to_vec())
    }

    /// Generates the IDL for the canister interface.
    /// This function uses the `generate_idl!()` macro to create the IDL.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}
