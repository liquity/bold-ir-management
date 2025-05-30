//! Runtime Strategy Execution Engine
//!
//! Provides the core strategy execution logic with transaction management,
//! rate calculations, and state transitions. This module handles the actual
//! running of strategies while maintaining atomic execution and state consistency.
//!
//! ```plain
//! Execution Flow:
//!
//!           ┌────────┐
//! Start ───►│  Lock  │
//!           └───┬────┘
//!               ▼
//!        ┌───────────┐    ┌─────────────┐
//!        │  Collect  │───►│  Calculate   │
//!        │   State   │    │  New Rate   │
//!        └───────────┘    └──────┬──────┘
//!                                ▼
//!        ┌───────────┐    ┌─────────────┐
//!        │Transaction│◄───│ Condition   │
//!        │  Submit   │    │   Checks    │
//!        └─────┬─────┘    └─────────────┘
//!              ▼
//!         ┌─────────┐
//! End ◄───│ Unlock  │
//!         └─────────┘
//! ```

use std::ops::Div;

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use ic_exports::ic_cdk::{api::time, print};

use crate::{
    constants::{
        max_number_of_troves, scale, tolerance_margin_down, tolerance_margin_up, MAX_RETRY_ATTEMPTS,
    },
    journal::{JournalCollection, LogType},
    state::{MANAGERS, STRATEGY_STATE},
    types::*,
    utils::{
        common::*,
        error::*,
        evm_rpc::{BlockTag, SendRawTransactionStatus},
        transaction_builder::TransactionBuilder,
    },
};

use super::{data::StrategyData, lock::Lock, settings::StrategySettings};

/// An atomic execution context that manages rate adjustments while maintaining
/// strict state consistency. Implements sophisticated concurrency control through
/// a multi-phase locking protocol.
///
/// The strategy maintains three core invariants:
///
/// 1. State Consistency: All modifications are atomic and durable
/// 2. Resource Safety: Resources are released even after panics
/// 3. Transaction Validity: All blockchain updates are verified
///
/// # Lifecycle
/// ```plain
///                  ┌─────────┐
/// Creation ───────►│ Created │
///                  └────┬────┘
///                       │
///                       ▼
///                  ┌─────────┐
/// execute() ──────►│ Locked  │
///                  └────┬────┘
///                       │
///                       ▼
///                  ┌─────────┐
/// drop() ─────────►│Released │
///                  └─────────┘
/// ```
#[derive(Clone, Default)]
pub struct ExecutableStrategy {
    /// Core configuration that remains constant during execution
    pub settings: StrategySettings,
    /// Mutable state that changes during execution
    pub data: StrategyData,
    /// Atomic execution lock
    pub lock: Lock,
    /// Lock acquisition status for clean Drop behavior
    acquired_lock: bool,
}

// State management functions
impl ExecutableStrategy {
    /// Creates a new executable strategy instance.
    pub fn new(settings: StrategySettings, data: StrategyData, lock: Lock) -> ExecutableStrategy {
        ExecutableStrategy {
            settings,
            data,
            lock,
            acquired_lock: false,
        }
    }

    /// Updates strategy state in persistent storage.
    fn apply_change(&self) {
        STRATEGY_STATE.with(|strategies| {
            strategies
                .borrow_mut()
                .insert(self.settings.key, self.into());
        });
    }

    /// Acquires execution lock with state consistency guarantees.
    fn lock(&mut self) -> ManagerResult<()> {
        self.lock.try_lock().map(|_| {
            self.acquired_lock = true;
            self.apply_change();
        })
    }

    /// Releases execution lock and persists final state.
    pub fn unlock(&mut self) {
        self.lock.try_unlock(self.acquired_lock);
        self.apply_change();
    }
}

#[derive(Clone)]
struct ExecutionContext {
    pub block_tag: BlockTag,
    pub troves: Vec<DebtPerInterestRate>,
    pub maximum_redeemable_against_collateral: U256,
    pub target_percentage: U256,
    pub time_since_last_update: U256,
    pub troves_count: U256,
}

// Query functions that gather the execution context required for running the strategy
impl ExecutableStrategy {
    async fn prepare_execution_context(
        &self,
        journal: &mut JournalCollection,
    ) -> ManagerResult<ExecutionContext> {
        // Fetch the current block tag
        let block_tag = get_block_tag(&self.settings.rpc_canister, true).await?;
        journal.append_note(
            Ok(()),
            LogType::Info,
            format!("Fixed block tag: {:?}.", block_tag),
        );

        // Calculate time since last update
        let time_since_last_update = U256::from(time().div(1_000_000_000) - self.data.last_update);

        // Fetch the entire system debt from the blockchain
        let entire_system_debt: U256 = self.fetch_entire_system_debt(block_tag.clone()).await?;

        // Fetch the unbacked portion price and redeemability status
        let unbacked_portion = self
            .fetch_unbacked_portion_price_and_redeemablity(None, block_tag.clone())
            .await?
            ._0;

        // Fetch and collect troves
        let mut troves: Vec<DebtPerInterestRate> = vec![];
        let mut troves_index = U256::from(0);
        let max_count = max_number_of_troves();
        loop {
            let (fetched_troves, curr_id) = self
                .fetch_multiple_sorted_troves(troves_index, max_count, block_tag.clone())
                .await?;

            let last_trove = fetched_troves
                .last()
                .ok_or(ManagerError::NonExistentValue)?
                .clone();
            troves.extend(fetched_troves);
            if last_trove.debt == U256::ZERO && last_trove.interestRate == U256::ZERO {
                break;
            }
            troves_index = curr_id;
        }

        troves.retain(|trove| trove.debt != U256::ZERO && trove.interestRate != U256::ZERO);
        let troves_count = U256::from(troves.len());

        // Fetch the redemption fee rate
        let redemption_fee = self.fetch_redemption_rate(block_tag.clone()).await?;

        // Calculate the total unbacked collateral
        let total_unbacked = self.fetch_total_unbacked(block_tag.clone()).await?;

        if total_unbacked == U256::ZERO {
            return Err(arithmetic_err("total unbacked was 0."));
        }

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!(
                "Total unbacked: {}, unbacked_portion: {}, entire system debt: {}",
                total_unbacked, unbacked_portion, entire_system_debt,
            ),
        );

        let two_digit_accuracy_split = unbacked_portion
            .saturating_mul(U256::from(100))
            .div(total_unbacked);

        let maximum_redeemable_against_collateral = if two_digit_accuracy_split < U256::from(1) {
            // less than 1% split
            // we saturate the split at 1%
            entire_system_debt.div(U256::from(100))
        } else {
            unbacked_portion
                .saturating_mul(entire_system_debt)
                .div(total_unbacked)
        };

        let target_percentage_numerator = self
            .settings
            .target_min
            .saturating_mul(U256::from(2))
            .saturating_mul(redemption_fee);
        let target_percentage_denominator =
            redemption_fee.saturating_add(U256::from(5 * 10_u128.pow(15)));
        let target_percentage = target_percentage_numerator
            .checked_div(target_percentage_denominator)
            .ok_or(arithmetic_err("Target percentage's denominator was zero."))?;

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!(
                "Maximum redeemable against collateral: {}, target_percentage: {} (numerator: {}, redemption_fee: {}, denominator: {})",
                maximum_redeemable_against_collateral,
                target_percentage,
                target_percentage_numerator,
                redemption_fee,
                target_percentage_denominator
            ),
        );

        Ok(ExecutionContext {
            block_tag,
            troves,
            maximum_redeemable_against_collateral,
            target_percentage,
            time_since_last_update,
            troves_count,
        })
    }

    /// Fetches total system debt across all markets
    async fn fetch_entire_system_debt(&self, block_tag: BlockTag) -> ManagerResult<U256> {
        let managers = MANAGERS.with(|managers_vector| managers_vector.borrow().clone());

        let mut total_debt = U256::ZERO;

        for manager in managers {
            let rpc_canister_response = call_with_dynamic_retries(
                &self.settings.rpc_canister,
                block_tag.clone(),
                manager,
                getEntireBranchDebtCall::SELECTOR.to_vec(),
            )
            .await?;

            total_debt +=
                decode_abi_response::<getEntireBranchDebtReturn, getEntireBranchDebtCall>(
                    rpc_canister_response,
                )?
                .entireSystemDebt;
        }

        Ok(total_debt)
    }

    /// Gets current redemption rate with decay
    async fn fetch_redemption_rate(&self, block_tag: BlockTag) -> ManagerResult<U256> {
        let rpc_canister_response = call_with_dynamic_retries(
            &self.settings.rpc_canister,
            block_tag,
            self.settings.collateral_registry,
            getRedemptionRateWithDecayCall::SELECTOR.to_vec(),
        )
        .await?;

        decode_abi_response::<getRedemptionRateWithDecayReturn, getRedemptionRateWithDecayCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data._0))?
    }

    /// Fetches unbacked portion metrics
    async fn fetch_unbacked_portion_price_and_redeemablity(
        &self,
        manager: Option<Address>,
        block_tag: BlockTag,
    ) -> ManagerResult<getUnbackedPortionPriceAndRedeemabilityReturn> {
        let call_manager = match manager {
            Some(value) => value,
            None => self.settings.manager,
        };

        let rpc_canister_response = call_with_dynamic_retries(
            &self.settings.rpc_canister,
            block_tag,
            call_manager,
            getUnbackedPortionPriceAndRedeemabilityCall::SELECTOR.to_vec(),
        )
        .await?;

        decode_abi_response::<
            getUnbackedPortionPriceAndRedeemabilityReturn,
            getUnbackedPortionPriceAndRedeemabilityCall,
        >(rpc_canister_response)
    }

    /// Retrieves sorted trove list from given index
    async fn fetch_multiple_sorted_troves(
        &self,
        index: U256,
        count: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<(Vec<DebtPerInterestRate>, U256)> {
        let parameters = getDebtPerInterestRateAscendingCall {
            _collIndex: self.settings.collateral_index,
            _startId: index,
            _maxIterations: count,
        };

        let data = getDebtPerInterestRateAscendingCall::abi_encode(&parameters);
        let rpc_canister_response = call_with_dynamic_retries(
            &self.settings.rpc_canister,
            block_tag,
            self.settings.multi_trove_getter,
            data,
        )
        .await?;

        decode_abi_response::<
            getDebtPerInterestRateAscendingReturn,
            getDebtPerInterestRateAscendingCall,
        >(rpc_canister_response)
        .map(|data| Ok((data._0, data.currId)))?
    }

    /// Gets total unbacked amount across markets
    async fn fetch_total_unbacked(&self, block_tag: BlockTag) -> ManagerResult<U256> {
        let managers: Vec<Address> =
            MANAGERS.with(|managers_vector| managers_vector.borrow().clone());

        let mut total_unbacked = U256::ZERO;

        for manager in managers {
            print(format!(
                "Calling manager {} strategy {}",
                manager, self.settings.key
            ));
            total_unbacked += self
                .fetch_unbacked_portion_price_and_redeemablity(Some(manager), block_tag.clone())
                .await?
                ._0;
        }

        Ok(total_unbacked)
    }
}

// Handles transaction building, submission, and handling
impl ExecutableStrategy {
    async fn send_rate_adjustment_transaction(
        &mut self,
        journal: &mut JournalCollection,
        new_rate: U256,
        max_upfront_fee: U256,
        execution_context: &ExecutionContext,
    ) -> ManagerResult<()> {
        let hints = self
            .calculate_hints(
                new_rate,
                execution_context.troves_count,
                execution_context.block_tag.clone(),
            )
            .await?;

        // Prepare the payload for updating the interest rate
        let payload = setNewRateCall {
            _newAnnualInterestRate: new_rate.to::<u128>(),
            _upperHint: hints.0,
            _lowerHint: hints.1,
            _maxUpfrontFee: max_upfront_fee.saturating_add(U256::from(1_000_000_000_000_000_u128)), // + %0.001 ,
        };

        // we want at least 2 runs in case the nonce needs adjustment
        let max_attempts = MAX_RETRY_ATTEMPTS.max(2);

        for _ in 1..=max_attempts {
            let eoa = self
                .settings
                .eoa_pk
                .ok_or(ManagerError::NonExistentValue)?
                .to_string();

            journal.append_note(
                Ok(()),
                LogType::Info,
                format!(
                    "Sending a rate adjustment transaction with rate: {}",
                    new_rate
                ),
            );

            let result = TransactionBuilder::default()
                .to(self.settings.batch_manager.to_string())
                .from(eoa)
                .data(payload.abi_encode())
                .value(U256::ZERO)
                .nonce(self.data.eoa_nonce)
                .derivation_path(self.settings.derivation_path.clone())
                .cycles(40_000_000_000_u128)
                .send(&self.settings.rpc_canister)
                .await?;

            journal.append_note(
                Ok(()),
                LogType::Info,
                "The rate adjustment transaction is sent.",
            );

            // Handle different transaction statuses
            if self.handle_transaction_response(journal, result, new_rate)? {
                break;
            } else {
                self.update_nonce().await?;
            }
        }
        Ok(())
    }

    /// True means break the loop, the tx was successful. False means nonce needs adjustment, continue the loop and adjust. Err means error occured, abort.
    fn handle_transaction_response(
        &mut self,
        journal: &mut JournalCollection,
        result: SendRawTransactionStatus,
        new_rate: U256,
    ) -> ManagerResult<bool> {
        match result {
            SendRawTransactionStatus::Ok(tx_hash) => {
                journal.append_note(
                    Ok(()),
                    LogType::RateAdjustment,
                    format!(
                        "The rate adjustment transaction was successful. Transaction hash: {:?}",
                        tx_hash
                    ),
                );

                self.data.eoa_nonce += 1;
                self.data.last_update = time() / 1_000_000_000;
                self.data.latest_rate = new_rate;
                self.apply_change();
                Ok(true)
            }
            SendRawTransactionStatus::InsufficientFunds => Err(ManagerError::Custom(
                "Not enough balance to cover the gas fee.".to_string(),
            )),
            SendRawTransactionStatus::NonceTooLow | SendRawTransactionStatus::NonceTooHigh => {
                journal.append_note(Ok(()), LogType::Info,"The rate adjustment transaction failed due to wrong nonce. Adjusting the nonce...");
                Ok(false)
            }
        }
    }

    /// Syncs EOA nonce with current chain state
    async fn update_nonce(&mut self) -> ManagerResult<()> {
        // Fetch the nonce for the given account
        let account = self.settings.eoa_pk.ok_or(ManagerError::NonExistentValue)?;
        self.data.eoa_nonce = get_nonce(&self.settings.rpc_canister, account)
            .await?
            .to::<u64>();
        self.apply_change();
        Ok(())
    }

    /// Calculates trove traversal hints
    async fn calculate_hints(
        &self,
        new_rate: U256,
        troves_count: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<(U256, U256)> {
        let approximate_hint = self
            .fetch_approximate_hint(new_rate, troves_count, block_tag.clone())
            .await?;

        let hints = self
            .fetch_insert_position(new_rate, approximate_hint, block_tag)
            .await?;

        Ok(hints)
    }

    /// Gets approximate hint for trove insertion
    async fn fetch_approximate_hint(
        &self,
        new_rate: U256,
        troves_count: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<U256> {
        let num_trials = U256::from(10) * troves_count.root(2);
        let arguments = getApproxHintCall {
            _collIndex: self.settings.collateral_index,
            _interestRate: new_rate,
            _numTrials: num_trials,
            _inputRandomSeed: U256::ZERO, // We don't care about the pseudo-random seed.
        };

        let data = getApproxHintCall::abi_encode(&arguments);

        let rpc_canister_response = call_with_dynamic_retries(
            &self.settings.rpc_canister,
            block_tag,
            self.settings.hint_helper,
            data,
        )
        .await?;
        decode_abi_response::<getApproxHintReturn, getApproxHintCall>(rpc_canister_response)
            .map(|data| Ok(data.hintId))?
    }

    /// Gets exact insert position for trove
    async fn fetch_insert_position(
        &self,
        new_rate: U256,
        approximate_hint: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<(U256, U256)> {
        let arguments = findInsertPositionCall {
            _annualInterestRate: new_rate,
            _prevId: approximate_hint,
            _nextId: approximate_hint,
        };
        let data = findInsertPositionCall::abi_encode(&arguments);
        let rpc_canister_response = call_with_dynamic_retries(
            &self.settings.rpc_canister,
            block_tag,
            self.settings.sorted_troves,
            data,
        )
        .await?;

        decode_abi_response::<findInsertPositionReturn, findInsertPositionCall>(
            rpc_canister_response,
        )
        .map(|data| Ok((data._0, data._1)))?
    }
}

// Execution Functions
impl ExecutableStrategy {
    /// Main strategy execution entrypoint.
    ///
    /// Execution phases:
    /// 1. Lock acquisition
    /// 2. State collection
    /// 3. Rate calculation
    /// 4. Condition validation  
    /// 5. Transaction submission
    /// 6. State persistence
    pub async fn execute(&mut self, journal: &mut JournalCollection) -> ManagerResult<()> {
        // Lock the strategy to prevent concurrent execution
        self.lock()?;

        let execution_context = self.prepare_execution_context(journal).await?;

        let current_debt_in_front =
            match self.get_current_debt_in_front(execution_context.troves.clone()) {
                Some(debt) => debt,
                None => {
                    journal.append_note(
                        Ok(()),
                        LogType::Info,
                        "No trove has delegated to this batch manager.",
                    );
                    return Ok(());
                }
            };

        // Execute the strategy logic based on calculated values and collected troves
        let strategy_result = self
            .run_strategy(journal, current_debt_in_front, &execution_context)
            .await?;

        // If the strategy successfully calculates a new rate, send a signed transaction to update it
        if let Some((new_rate, max_upfront_fee)) = strategy_result {
            self.send_rate_adjustment_transaction(
                journal,
                new_rate,
                max_upfront_fee,
                &execution_context,
            )
            .await?;
        } else {
            journal.append_note(
                Ok(()),
                LogType::Info,
                "The rate adjustment requirements were not met. No need to submit a transaction.",
            );
        }

        // Unlock the strategy after attempting execution
        self.unlock();
        Ok(())
    }

    /// Estimates upfront fee cost for rate change
    async fn predict_upfront_fee(
        &self,
        new_rate: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<U256> {
        let arguments = predictAdjustBatchInterestRateUpfrontFeeCall {
            _collIndex: self.settings.collateral_index,
            _batchAddress: self.settings.batch_manager,
            _newInterestRate: new_rate,
        };

        let data = predictAdjustBatchInterestRateUpfrontFeeCall::abi_encode(&arguments);

        let rpc_canister_response = call_with_dynamic_retries(
            &self.settings.rpc_canister,
            block_tag,
            self.settings.hint_helper,
            data,
        )
        .await?;
        decode_abi_response::<
            predictAdjustBatchInterestRateUpfrontFeeReturn,
            predictAdjustBatchInterestRateUpfrontFeeCall,
        >(rpc_canister_response)
        .map(|data| Ok(data._0))?
    }

    /// Calculates debt in front of current batch
    fn get_current_debt_in_front(&mut self, troves: Vec<DebtPerInterestRate>) -> Option<U256> {
        let mut counted_debt = U256::from(0);

        for trove in troves.iter() {
            if trove.interestBatchManager == self.settings.batch_manager {
                // update the current interest rate
                self.data.latest_rate(trove.interestRate);
                return Some(counted_debt);
            }
            counted_debt = counted_debt.saturating_add(trove.debt);
        }
        None
    }

    /// Core strategy execution logic
    async fn run_strategy(
        &mut self,
        journal: &mut JournalCollection,
        current_debt_in_front: U256,
        execution_context: &ExecutionContext,
    ) -> ManagerResult<Option<(U256, U256)>> {
        // Calculate new rate
        let new_rate = self
            .calculate_new_rate(
                journal,
                execution_context.troves.clone(),
                execution_context.target_percentage,
                execution_context.maximum_redeemable_against_collateral,
            )
            .await?;

        if new_rate == self.data.latest_rate {
            // we don't want to adjust the rate with the same value.
            journal.append_note(
                Ok(()),
                LogType::Info,
                "The calculated rate is the same as the current rate. No need to progress further.",
            );

            return Ok(None);
        } else if new_rate == U256::ZERO {
            journal.append_note(
                Ok(()),
                LogType::Info,
                "The calculated rate is zero. No need to progress further.",
            );

            return Ok(None);
        }

        // Predict upfront fee
        let upfront_fee = self
            .predict_upfront_fee(new_rate, execution_context.block_tag.clone())
            .await?;

        // Check conditions to execute the strategy
        if self.increase_check(
            journal,
            current_debt_in_front,
            execution_context.maximum_redeemable_against_collateral,
            execution_context.target_percentage,
        ) || (self.first_decrease_check(
            journal,
            current_debt_in_front,
            execution_context.maximum_redeemable_against_collateral,
            execution_context.target_percentage,
        ) && self.second_decrease_check(
            journal,
            execution_context.time_since_last_update,
            self.settings.upfront_fee_period,
            new_rate,
            upfront_fee,
        )?) {
            return Ok(Some((new_rate, upfront_fee)));
        }

        Ok(None)
    }

    /// Calculates optimal new interest rate
    async fn calculate_new_rate(
        &self,
        journal: &mut JournalCollection,
        troves: Vec<DebtPerInterestRate>,
        target_percentage: U256,
        maximum_redeemable_against_collateral: U256,
    ) -> ManagerResult<U256> {
        let mut counted_debt = U256::ZERO;
        let mut new_rate = U256::ZERO;
        let target_debt = target_percentage * maximum_redeemable_against_collateral / scale();

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!("Calculated target debt in front: {}, number of troves in this collateral market (including the batch): {}",
            target_debt,
            troves.len()),
        );

        if target_debt == U256::ZERO {
            return Err(ManagerError::Custom(
                "The target amount is zero. Not proceeding.".to_string(),
            ));
        }

        for trove in troves
            .iter()
            .filter(|t| t.interestBatchManager != self.settings.batch_manager)
        {
            counted_debt = counted_debt
                .checked_add(trove.debt)
                .ok_or_else(|| arithmetic_err("Counted debt overflowed."))?;

            if counted_debt > target_debt {
                new_rate = trove
                    .interestRate
                    .saturating_add(U256::from(100_000_000_000_000_u128)); // Increment rate by 1 bps (0.01%)

                journal.append_note(
                    Ok(()),
                    LogType::Info,
                    format!("Positioning the batch after trove with debt {}", trove.debt),
                );
                break;
            }
        }

        if new_rate == U256::ZERO
            && troves.last().unwrap().interestBatchManager != self.settings.batch_manager
        {
            // There was not enough debt in the market
            // the trove should be positioned at the end of the market.
            new_rate = troves
                .last()
                .unwrap()
                .interestRate
                .saturating_add(U256::from(100_000_000_000_000_u128)); // Increment rate by 1 bps (0.01%)

            journal.append_note(
                Ok(()),
                LogType::Info,
                format!("Not enough debt in the market, moving the batch to the end."),
            );
        }

        Ok(new_rate)
    }

    /// Validates rate increase conditions
    fn increase_check(
        &self,
        journal: &mut JournalCollection,
        debt_in_front: U256,
        maximum_redeemable_against_collateral: U256,
        target_percentage: U256,
    ) -> bool {
        let target_debt = target_percentage * maximum_redeemable_against_collateral / scale();
        let target_debt_with_margin = target_debt * (scale() - tolerance_margin_down()) / scale();

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!(
                "increase check: {} < {}",
                debt_in_front, target_debt_with_margin
            ),
        );

        if debt_in_front < target_debt_with_margin {
            return true;
        }
        false
    }

    /// First phase decrease validation
    fn first_decrease_check(
        &self,
        journal: &mut JournalCollection,
        debt_in_front: U256,
        maximum_redeemable_against_collateral: U256,
        target_percentage: U256,
    ) -> bool {
        let target_debt = target_percentage * maximum_redeemable_against_collateral / scale();
        let target_debt_with_margin = target_debt * (scale() + tolerance_margin_up()) / scale();

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!(
                "first decrease check: {} > {}",
                debt_in_front, target_debt_with_margin
            ),
        );

        if debt_in_front > target_debt_with_margin {
            return true;
        }
        false
    }

    /// Second phase decrease validation
    fn second_decrease_check(
        &self,
        journal: &mut JournalCollection,
        time_since_last_update: U256,
        upfront_fee_period: U256,
        new_rate: U256,
        average_rate: U256,
    ) -> ManagerResult<bool> {
        // Check if time exceeds period first
        if time_since_last_update > upfront_fee_period {
            journal.append_note(
                Ok(()),
                LogType::Info,
                "second decrease check passed: time exceeded period",
            );
            return Ok(true);
        }

        // Scale the division by 100 to get 2 decimal places
        let scale_hundred: U256 = U256::from(100);

        // Calculate r with 2 decimal precision
        let scaled_time = time_since_last_update
            .checked_mul(scale_hundred)
            .ok_or(arithmetic_err("Overflow in time scaling"))?;

        let r = scaled_time
            .checked_div(upfront_fee_period)
            .ok_or(arithmetic_err("Upfront fee period was 0."))?;

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!(
                "second decrease check: time since last update {} upfront fee period {} latest rate {} new rate {} average rate {} scaled r {}", 
                time_since_last_update,
                upfront_fee_period,
                self.data.latest_rate,
                new_rate,
                average_rate,
                r
            )
        );

        // For the main condition, we need to scale the computation
        // (1 - r/100) * (latest_rate - new_rate) > average_rate
        let scaled_diff = scale_hundred
            .checked_sub(r)
            .ok_or(arithmetic_err("Error in r subtraction"))?;

        let rate_diff = self
            .data
            .latest_rate
            .checked_sub(new_rate)
            .ok_or(arithmetic_err("Error in rate difference calculation"))?;

        let scaled_product = scaled_diff
            .checked_mul(rate_diff)
            .ok_or(arithmetic_err("Overflow in scaled product"))?;

        let scaled_average = average_rate
            .checked_mul(scale_hundred)
            .ok_or(arithmetic_err("Error in scaling average rate"))?;

        if scaled_product > scaled_average {
            journal.append_note(
                Ok(()),
                LogType::Info,
                "second decrease check passed: rate condition",
            );
            return Ok(true);
        }

        Ok(false)
    }
}

/// Ensures strategy unlocking on scope exit
impl Drop for ExecutableStrategy {
    /// Unlocks the strategy when the instance goes out of scope
    /// Ensures that resources are freed and the strategy is no longer locked
    fn drop(&mut self) {
        self.unlock();
    }
}

/*
========================================
= May the rates be ever in your favor  =
========================================
*/
