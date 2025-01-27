//! The canister's public methods
#![allow(missing_docs)]

use std::{sync::Arc, time::Duration};

use crate::cleanup::daily_cleanup;
use crate::constants::MAX_RETRY_ATTEMPTS;
use crate::halt::{is_functional, Halt, update_halt_status};
use crate::constants::MINIMUM_ATTACHED_CYCLES;
use crate::journal::JournalCollection;
use crate::journal::StableJournalCollection;
use crate::strategy::data::StrategyData;
use crate::strategy::run::run_strategy;
use crate::strategy::settings::StrategySettings;
use crate::strategy::stable::StableStrategy;
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

/// The IrManager canister struct (a `canister-sdk` requirement)
#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    /// Mints a new strategy with the provided configuration parameters.
    /// 
    /// This function creates a new Interest Rate Management strategy and initializes it with the provided settings.
    /// The strategy gets assigned a unique EOA (Externally Owned Account) through tECDSA key derivation.
    /// 
    /// # Arguments
    /// 
    /// * `strategy` - Configuration parameters for the new strategy including:
    ///   - key: Unique identifier for the strategy
    ///   - target_min: Minimum target for debt positioning
    ///   - manager: Address of the Trove Manager contract
    ///   - multi_trove_getter: Contract address for fetching multiple trove data
    ///   - collateral_index: Index of the collateral type
    ///   - rpc_principal: Principal ID of the EVM RPC canister
    ///   - upfront_fee_period: Cooldown period for rate adjustments in seconds
    ///   - collateral_registry: Address of the collateral registry contract
    ///   - hint_helper: Address of the hint helper contract
    /// 
    /// # Returns
    /// 
    /// * `Ok(String)` - The address of the newly generated EOA for this strategy
    /// * `Err(ManagerError)` - If strategy creation fails due to:
    ///   - Key already in use
    ///   - Invalid addresses
    ///   - tECDSA key generation failure
    /// 
    /// # Access Control
    /// 
    /// Only the canister controller can call this function.
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
            get_canister_public_key(key_id, None, derivation_path.clone()).await?;
        let eoa_pk = string_to_address(pubkey_bytes_to_address(&public_key_bytes)?)?;
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

    /// Sets the batch manager contract address for a given strategy.
    /// 
    /// This function associates a batch manager contract with an existing strategy and 
    /// initializes its current interest rate. Must be called after strategy minting
    /// but before the strategy can begin executing.
    /// 
    /// # Arguments
    /// 
    /// * `key` - The unique identifier of the existing strategy
    /// * `batch_manager` - Ethereum address of the batch manager contract
    /// * `current_rate` - Initial interest rate for the batch manager
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - If batch manager was successfully set
    /// * `Err(ManagerError)` - If operation fails due to:
    ///   - Strategy not found
    ///   - Invalid batch manager address
    ///   - Rate conversion error
    /// 
    /// # Access Control
    /// 
    /// Only the canister controller can call this function.
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

    /// Starts all system timers for strategy execution and maintenance tasks.
    /// 
    /// This function initializes recurring timers for:
    /// - Hourly strategy execution cycles
    /// - Daily ckETH balance monitoring and recharging
    /// - Daily provider reputation management and cleanup
    /// - Daily halt condition evaluation
    /// 
    /// Each strategy immediately executes once when timers start, then begins its hourly cycle.
    /// The recharge cycle monitors both cycle and ckETH balances, triggering recharge
    /// operations when thresholds are reached.
    /// 
    /// # Returns
    /// 
    /// * `Ok(())` - If all timers were successfully started
    /// * `Err(ManagerError)` - If timer initialization fails
    /// 
    /// # Access Control
    /// 
    /// Only the canister controller can call this function.
    /// 
    /// # Note
    /// 
    /// This should typically be called once after all strategies are configured
    /// and before the canister is made immutable.
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
                assert!(is_functional());
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
            spawn(daily_cleanup());
        });
      
        set_timer_interval(Duration::from_secs(86_400), || {
            update_halt_status();
        });
      
        Ok(())
    }

    /// Retrieves current data for all strategies in the system.
    /// 
    /// Returns information about each strategy including:
    /// - Contract addresses (trove manager, batch manager)
    /// - Lock status and timing
    /// - Current interest rate
    /// - Target minimums
    /// - EOA public key
    /// - Last update timestamp
    /// 
    /// # Returns
    /// 
    /// A vector of StrategyQueryData structs containing current strategy states.
    /// Returns an empty vector if no strategies exist.
    #[query]
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

    /// Retrieves the EOA address associated with a specific strategy.
    /// 
    /// # Arguments
    /// 
    /// * `index` - The unique identifier of the strategy
    /// 
    /// # Returns
    /// 
    /// * `Some(String)` - The EOA address if the strategy exists and has an assigned EOA
    /// * `None` - If strategy doesn't exist or has no EOA assigned
    #[query]
    pub fn get_strategy_address(&self, index: u32) -> Option<String> {
        STRATEGY_STATE.with(|data| {
            data.borrow()
                .get(&index)
                .and_then(|strategy| strategy.settings.eoa_pk.map(|pk| pk.to_string()))
        })
    }

    /// Facilitates ckETH<>Cycles arbitrage operations.
    /// 
    /// This function allows arbitrageurs to provide cycles to the canister in exchange
    /// for discounted ckETH. The exchange includes:
    /// - Verifying minimum cycle requirements
    /// - Checking canister cycle balance thresholds
    /// - Applying a discount on ckETH transfer
    /// - Atomic swap execution with rollback on failure
    /// 
    /// # Returns
    /// 
    /// * `Ok(SwapResponse)` - Details of the successful swap including:
    ///   - accepted_cycles: Amount of cycles accepted
    ///   - returning_ether: Amount of ckETH transferred
    /// * `Err(ManagerError)` - If swap fails due to:
    ///   - Insufficient cycles attached
    ///   - Cycles balance above threshold
    ///   - ckETH transfer failure
    ///   - Lock acquisition failure
    /// 
    /// # Panics
    /// 
    /// Panics if the canister is not in a functional state.
    #[update]
    pub async fn swap_cketh(&self) -> ManagerResult<SwapResponse> {
        assert!(is_functional());
      
        // Ensure the caller has attached enough cycles
        if msg_cycles_available() < MINIMUM_ATTACHED_CYCLES {
            return Err(ManagerError::Custom(format!(
                "The attached cycles amount ({}) is less than the minimum accepted amount ({})",
                msg_cycles_available(),
                MINIMUM_ATTACHED_CYCLES
            )));
        }
      
        let mut swap_lock = SwapLock::default();
        swap_lock.lock()?;
        check_threshold().await?;
        transfer_cketh(caller()).await
    }
    
    /// Retrieves recent system logs up to specified depth.
    /// 
    /// Returns the most recent journal collections containing logs of:
    /// - Strategy executions
    /// - Rate adjustments
    /// - Recharge operations
    /// - Provider reputation changes
    /// 
    /// # Arguments
    /// 
    /// * `depth` - Number of most recent journal collections to return
    /// 
    /// # Returns
    /// 
    /// * `Ok(Vec<StableJournalCollection>)` - Vector of journal collections
    /// * `Err(ManagerError)` - If log retrieval fails
    #[query]
    pub async fn get_logs(&self, depth: u64) -> ManagerResult<Vec<StableJournalCollection>> {
        let entries = JOURNAL.with(|m| m.borrow().iter().collect::<Vec<StableJournalCollection>>());

        Ok(entries[entries.len().saturating_sub(depth as usize)..].to_vec())
    }

    /// Retrieves logs for a specific strategy up to specified depth.
    /// 
    /// Returns journal collections filtered to only include entries related
    /// to the specified strategy.
    /// 
    /// # Arguments
    /// 
    /// * `depth` - Number of most recent journal collections to return
    /// * `strategy_key` - Unique identifier of the strategy
    /// 
    /// # Returns
    /// 
    /// * `Ok(Vec<StableJournalCollection>)` - Vector of filtered journal collections
    /// * `Err(ManagerError)` - If log retrieval fails
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

    /// Returns the current halt status of the canister.
    /// 
    /// The halt status indicates whether the canister is:
    /// - Functional: Operating normally
    /// - HaltingInProgress: In 7-day warning period before halt
    /// - Halted: Permanently stopped due to system conditions
    /// 
    /// # Returns
    /// 
    /// Current Halt struct containing status and optional message
    #[query]
    pub fn halt_status(&self) -> Halt {
        HALT_STATE.with(|state| state.borrow().clone())
    }

    /// Generates the canister interface IDL.
    /// 
    /// Creates a Candid interface description for all public canister methods.
    /// Used for canister-to-canister communication and API documentation.
    /// 
    /// # Returns
    /// 
    /// Generated Candid IDL for the canister
    pub fn idl() -> Idl {
        generate_idl!()
    }
}
