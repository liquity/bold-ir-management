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

/// Executable strategy that handles runtime operations and state transitions.
///
/// Key responsibilities:
/// - Atomic execution control
/// - Rate calculation and validation  
/// - Transaction management
/// - State consistency
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

        let current_debt_in_front = match self.get_current_debt_in_front(troves.clone()) {
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

        // Fetch the redemption fee rate
        let redemption_fee = self.fetch_redemption_rate(block_tag.clone()).await?;

        // Calculate the total unbacked collateral
        let total_unbacked = self.fetch_total_unbacked(block_tag.clone()).await?;

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!(
                "Calculated: total unbacked: {}, unbacked_portion: {}, entire system debt: {}",
                total_unbacked, unbacked_portion, entire_system_debt,
            ),
        );

        let maximum_redeemable_against_collateral = unbacked_portion
            .saturating_mul(entire_system_debt)
            .checked_div(total_unbacked)
            .ok_or(arithmetic_err("total unbacked was 0."))?;

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
                "Calculated: maximum redeemable against collateral: {}, target_percentage: {} (numerator: {}, redemption_fee: {}, denominator: {})",
                maximum_redeemable_against_collateral,
                target_percentage,
                target_percentage_numerator,
                redemption_fee,
                target_percentage_denominator
            ),
        );

        // Execute the strategy logic based on calculated values and collected troves
        let strategy_result = self
            .run_strategy(
                journal,
                troves,
                current_debt_in_front,
                time_since_last_update,
                self.settings.upfront_fee_period,
                maximum_redeemable_against_collateral,
                U256::from(target_percentage),
                block_tag.clone(),
            )
            .await?;

        // If the strategy successfully calculates a new rate, send a signed transaction to update it
        if let Some((new_rate, max_upfront_fee)) = strategy_result {
            let hints = self
                .calculate_hints(new_rate, troves_count, block_tag.clone())
                .await?;

            // Prepare the payload for updating the interest rate
            let payload = setNewRateCall {
                _newAnnualInterestRate: new_rate.to::<u128>(),
                _upperHint: hints.0,
                _lowerHint: hints.1,
                _maxUpfrontFee: max_upfront_fee
                    .saturating_add(U256::from(1_000_000_000_000_000_u128)), // + %0.001 ,
            };

            for _ in 1..=MAX_RETRY_ATTEMPTS + 1 {
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
                match result {
                    SendRawTransactionStatus::Ok(tx_hash) => {
                        journal.append_note(
                            Ok(()),
                            LogType::RateAdjustment,
                            format!("The rate adjustment transaction was successful. Transaction hash: {:?}", tx_hash),
                        );

                        self.data.eoa_nonce += 1;
                        self.data.last_update = time() / 1_000_000_000;
                        self.data.latest_rate = new_rate;
                        self.apply_change();
                        break;
                    }
                    SendRawTransactionStatus::InsufficientFunds => {
                        return Err(ManagerError::Custom(
                            "Not enough balance to cover the gas fee.".to_string(),
                        ))
                    }
                    SendRawTransactionStatus::NonceTooLow
                    | SendRawTransactionStatus::NonceTooHigh => {
                        journal.append_note(Ok(()), LogType::Info,"The rate adjustment transaction failed due to wrong nonce. Adjusting the nonce...");
                        self.update_nonce().await?;
                    }
                }
            }
        } else {
            journal.append_note(
                Ok(()),
                LogType::Info,
                "The rate adjustment requirements were not met. No need to submit a transaction.",
            );
        }

        // Unlock the strategy after attempting execution
        self.data.last_update(time() / 1_000_000_000);
        self.apply_change();
        self.unlock();
        Ok(())
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

    /// Fetches total system debt across all markets
    async fn fetch_entire_system_debt(&self, block_tag: BlockTag) -> ManagerResult<U256> {
        let managers = MANAGERS.with(|managers_vector| managers_vector.borrow().clone());

        let mut total_debt = U256::ZERO;

        for manager in managers {
            let rpc_canister_response = call_with_dynamic_retries(
                &self.settings.rpc_canister,
                block_tag.clone(),
                manager,
                getEntireSystemDebtCall::SELECTOR.to_vec(),
            )
            .await?;

            total_debt +=
                decode_abi_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(
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
                manager.to_string(),
                self.settings.key
            ));
            total_unbacked += self
                .fetch_unbacked_portion_price_and_redeemablity(Some(manager), block_tag.clone())
                .await?
                ._0;
        }

        Ok(total_unbacked)
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
    #[allow(clippy::too_many_arguments)]
    async fn run_strategy(
        &mut self,
        journal: &mut JournalCollection,
        troves: Vec<DebtPerInterestRate>,
        current_debt_in_front: U256,
        time_since_last_update: U256,
        upfront_fee_period: U256,
        maximum_redeemable_against_collateral: U256,
        target_percentage: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<Option<(U256, U256)>> {
        journal.append_note(
            Ok(()),
            LogType::Info,
            format!("Current debt in front: {}", current_debt_in_front),
        );

        // Calculate new rate
        let new_rate = self
            .calculate_new_rate(
                journal,
                troves,
                target_percentage,
                maximum_redeemable_against_collateral,
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
        }

        // Predict upfront fee
        let upfront_fee = self.predict_upfront_fee(new_rate, block_tag).await?;

        // Check conditions to execute the strategy
        if self.increase_check(
            journal,
            current_debt_in_front,
            maximum_redeemable_against_collateral,
            target_percentage,
        ) || (self.first_decrease_check(
            journal,
            current_debt_in_front,
            maximum_redeemable_against_collateral,
            target_percentage,
        ) && self.second_decrease_check(
            journal,
            time_since_last_update,
            upfront_fee_period,
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

        let mut full_debt = U256::ZERO;
        journal.append_note(
            Ok(()),
            LogType::Info,
            format!("Calculated target debt in front: {}", target_debt),
        );
        let mut last_debt = U256::ZERO;

        troves
            .iter()
            .filter(|t| t.interestBatchManager != self.settings.batch_manager)
            .for_each(|trove| {
                full_debt += trove.debt;
                last_debt = trove.debt;
            });

        for (index, trove) in troves
            .iter()
            .filter(|t| t.interestBatchManager != self.settings.batch_manager)
            .enumerate()
        {
            counted_debt = counted_debt
                .checked_add(trove.debt)
                .ok_or_else(|| arithmetic_err("Counted debt overflowed."))?;

            journal.append_note(
                    Ok(()),
                    LogType::Info,
                    format!(
                        "Adding the debt of trove at position {} with {} new counted debt is {} equivalent of {}% of the market",
                        index, trove.debt, counted_debt, counted_debt.saturating_mul(U256::from(100)).div(full_debt)
                    ),
                );

            if counted_debt > target_debt {
                new_rate = trove
                    .interestRate
                    .saturating_add(U256::from(100_000_000_000_000_u128)); // Increment rate by 1 bps (0.01%)

                journal.append_note(
                    Ok(()),
                    LogType::Info,
                    format!(
                        "Positioning batch after trove id: {} with debt {}",
                        index, trove.debt
                    ),
                );
                break;
            }
        }

        journal.append_note(
            Ok(()),
            LogType::Info,
            format!(
                "Calculated new rate: {}. Debt in front percentage is {}. Counted debt {}, full debt {}, number of troves {}, last trove counted had debt {}",
                new_rate,
                counted_debt.saturating_mul(U256::from(100)).div(full_debt),
                counted_debt,
                full_debt,
                troves.len(),
                last_debt
            ),
        );

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
        let r = time_since_last_update
            .checked_div(upfront_fee_period)
            .ok_or(arithmetic_err("Upfront fee period was 0."))?;
        journal.append_note(Ok(()), LogType::Info,format!("second decrease check: time since last update {} upfront fee period {} latest rate {} new rate {} average rate {}", time_since_last_update, upfront_fee_period, self.data.latest_rate, new_rate, average_rate));

        if (U256::from(1) - r) * (self.data.latest_rate - new_rate) > average_rate
            || time_since_last_update > upfront_fee_period
        {
            print("second decrease check passed");
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
