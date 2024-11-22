//! The executable strategy wrapper that runs the strategy.

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use ic_exports::ic_cdk::{api::time, print};

use crate::{
    constants::{max_number_of_troves, tolerance_margin_down, tolerance_margin_up, SCALE},
    journal::JournalEntry,
    state::{MANAGERS, STRATEGY_STATE},
    types::*,
    utils::{
        common::*,
        error::*,
        evm_rpc::{BlockTag, SendRawTransactionStatus},
        transaction_builder::TransactionBuilder,
    },
};

use super::{data::StrategyData, settings::StrategySettings};

#[derive(Clone, Default)]
pub struct ExecutableStrategy {
    /// Immutable settings and configurations
    pub settings: StrategySettings,
    /// Mutable state
    pub data: StrategyData,
    /// Lock for the strategy. Determines if the strategy is currently being executed.
    pub lock: bool,
}

impl ExecutableStrategy {
    /// Replaces the strategy data in the HashMap
    /// This function updates the state of the strategy in the HashMap
    fn apply_change(&self) {
        STRATEGY_STATE.with(|strategies| {
            strategies
                .borrow_mut()
                .insert(self.settings.key, self.into());
        });
    }

    /// Locks the strategy.
    /// Prevents concurrent execution of the strategy to ensure consistent state.
    fn lock(&mut self) -> ManagerResult<()> {
        let state_lock = STRATEGY_STATE.with(|strategies| {
            Ok(strategies
                .borrow()
                .get(&self.settings.key)
                .cloned()
                .ok_or(ManagerError::NonExistentValue)?
                .lock)
        })?;
        if self.lock || state_lock {
            // Already locked, indicating the strategy is being processed elsewhere
            return Err(ManagerError::Locked);
        }
        self.lock = true;
        self.apply_change();
        Ok(())
    }

    /// Unlocks the strategy.
    /// Releases the lock to allow future executions.
    pub fn unlock(&mut self) {
        self.lock = false;
        self.apply_change();
    }

    /// The main entry point to execute the strategy.
    /// Runs the strategy logic asynchronously.
    pub async fn execute(&mut self) -> ManagerResult<()> {
        // Lock the strategy to prevent concurrent execution
        self.lock()?;
        // print(format!("Reached before block tag"));

        // Fetch the current block tag
        let block_tag = get_block_tag(&self.settings.rpc_canister, false).await?;

        // print(format!("Reached after block tag: {:#?}", &block_tag));

        // Calculate time since last update
        let time_since_last_update = U256::from(time() - self.data.last_update);
        // print(format!(
        //     "Reached after time since last update: {:#?}",
        //     &time_since_last_update
        // ));
        // print(format!("Reached before entire_system_debt"));

        // Fetch the entire system debt from the blockchain
        let entire_system_debt: U256 = self.fetch_entire_system_debt(block_tag.clone()).await?;
        // print(format!(
        //     "Reached after entire_system_debt: {:#?}",
        //     entire_system_debt
        // ));
        // print(format!(
        //     "Reached before unbacked_portion_price_and_redeemability"
        // ));

        // Fetch the unbacked portion price and redeemability status
        let unbacked_portion_price_and_redeemability = self
            .fetch_unbacked_portion_price_and_redeemablity(None, block_tag.clone())
            .await?;
        // print(format!(
        //     "Reached after unbacked_portion_price_and_redeemability: {:#?}",
        //     unbacked_portion_price_and_redeemability._0
        // ));

        // Fetch and collect troves
        let mut troves: Vec<DebtPerInterestRate> = vec![];
        let mut troves_index = U256::from(0);
        let max_count = max_number_of_troves();
        loop {
            // print(format!("Reached before fetched_troves"));
            let fetched_troves = self
                .fetch_multiple_sorted_troves(troves_index, max_count, block_tag.clone())
                .await?;
            // print(format!(
            //     "Reached after fetched_troves: {:#?}",
            //     fetched_troves[0].debt
            // ));
            let last_trove = fetched_troves
                .last()
                .ok_or(ManagerError::NonExistentValue)?
                .clone();
            troves.extend(fetched_troves);
            if last_trove.debt == U256::ZERO && last_trove.interestRate == U256::ZERO {
                break;
            }
            troves_index += max_count;
        }
        // print(format!("Reached before redemption_fee"));

        // Fetch the redemption fee rate
        let redemption_fee = self.fetch_redemption_rate(block_tag.clone()).await?;
        // print(format!(
        //     "Reached after redemption_fee: {:#?}",
        //     redemption_fee
        // ));
        // print(format!("Reached before total_unbacked"));

        // Calculate the total unbacked collateral
        let total_unbacked = self
            .fetch_total_unbacked(
                unbacked_portion_price_and_redeemability._0,
                block_tag.clone(),
            )
            .await?;
        // print(format!(
        //     "Reached after total_unbacked: {:#?}",
        //     total_unbacked
        // ));
        // print(format!("Reached before redemption_split"));

        // Calculate redemption split and maximum redeemable against collateral
        let redemption_split = unbacked_portion_price_and_redeemability
            ._0
            .checked_div(total_unbacked)
            .ok_or(arithmetic_err("Total unbacked was 0."))?;
        let maximum_redeemable_against_collateral =
            redemption_split.saturating_mul(entire_system_debt);
        // print(format!(
        //     "Reached after redemption_split: {:#?}",
        //     redemption_split
        // ));
        // print(format!("Reached before target_percentage"));
        // Calculate the target percentage based on redemption fee and settings
        let exponent: U256 = U256::from(5 * 1_000_000_000_000_000_u128)
            .checked_div(redemption_fee)
            .ok_or(arithmetic_err("redemption fee was 0."))?;
        let target_percentage = self.settings.target_min.pow(exponent);
        // print(format!(
        //     "Reached after target_percentage: {:#?}",
        //     target_percentage
        // ));
        // Execute the strategy logic based on calculated values and collected troves
        let strategy_result = self
            .run_strategy(
                troves,
                time_since_last_update,
                self.settings.upfront_fee_period,
                maximum_redeemable_against_collateral,
                U256::from(target_percentage),
                block_tag.clone(),
            )
            .await?;

        // If the strategy successfully calculates a new rate, send a signed transaction to update it
        if let Some((new_rate, max_upfront_fee)) = strategy_result {
            // Send a signed transaction to update the rate for the batch
            // Get hints (upper/lower) to minimize gas
            let upper_hint = U256::from(0);
            let lower_hint = U256::from(0);

            // Prepare the payload for updating the interest rate
            let payload = setNewRateCall {
                _newAnnualInterestRate: new_rate.to::<u128>(),
                _upperHint: upper_hint,
                _lowerHint: lower_hint,
                _maxUpfrontFee: max_upfront_fee
                    .saturating_add(U256::from(1_000_000_000_000_000_u128)), // + %0.001 ,
            };

            // Retry the transaction twice if necessary
            for _ in 0..2 {
                let eoa = self
                    .settings
                    .eoa_pk
                    .ok_or(ManagerError::NonExistentValue)?
                    .to_string();

                JournalEntry::new(Ok(()))
                    .note(format!(
                        "Sending a rate adjustment transaction with this rate: {}",
                        new_rate
                    ))
                    .strategy(self.settings.key)
                    .commit();

                let tx_response = TransactionBuilder::default()
                    .to(self.settings.batch_manager.to_string())
                    .from(eoa)
                    .data(payload.abi_encode())
                    .value(U256::ZERO)
                    .nonce(self.data.eoa_nonce)
                    .derivation_path(self.settings.derivation_path.clone())
                    .cycles(40_000_000_000_u128)
                    .send(&self.settings.rpc_canister)
                    .await?;

                JournalEntry::new(Ok(()))
                    .note("The rate adjustment transaction is sent.")
                    .strategy(self.settings.key)
                    .commit();

                let result = extract_multi_rpc_result(tx_response)?;

                // Handle different transaction statuses
                match result {
                    SendRawTransactionStatus::Ok(a) => {
                        JournalEntry::new(Ok(()))
                            .note("The rate adjustment transaction was successful.")
                            .strategy(self.settings.key)
                            .commit();

                        print(format!("{:#?}", a));
                        self.data.eoa_nonce += 1;
                        self.data.last_update = time();
                        self.data.latest_rate = new_rate;
                        self.apply_change();
                        self.unlock();
                        return Ok(());
                    }
                    SendRawTransactionStatus::InsufficientFunds => {
                        return Err(ManagerError::Custom(
                            "Not enough balance to cover the gas fee.".to_string(),
                        ))
                    }
                    SendRawTransactionStatus::NonceTooLow
                    | SendRawTransactionStatus::NonceTooHigh => {
                        JournalEntry::new(Ok(()))
                            .note("The rate adjustment transaction failed due to wrong nonce. Adjusting the nonce...")
                            .strategy(self.settings.key)
                            .commit();
                        self.update_nonce().await?;
                    }
                }
            }
        } else {
            JournalEntry::new(Ok(()))
                            .note("The rate adjustment requirements were not met. No need to submit a transaction.")
                            .strategy(self.settings.key)
                            .commit();
        }

        // Unlock the strategy after attempting execution
        self.unlock();
        Ok(())
    }

    /// Updates the nonce for the externally owned account (EOA)
    async fn update_nonce(&mut self) -> ManagerResult<()> {
        // Fetch the nonce for the given account
        let account = self.settings.eoa_pk.ok_or(ManagerError::NonExistentValue)?;
        self.data.eoa_nonce = get_nonce(&self.settings.rpc_canister, account)
            .await?
            .to::<u64>();
        self.apply_change();
        Ok(())
    }

    /// Predicts the upfront fee for a given new rate
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

    /// Returns the debt of the entire system across all markets if successful.
    async fn fetch_entire_system_debt(&self, block_tag: BlockTag) -> ManagerResult<U256> {
        let rpc_canister_response = call_with_dynamic_retries(
            &self.settings.rpc_canister,
            block_tag,
            self.settings.manager,
            getEntireSystemDebtCall::SELECTOR.to_vec(),
        )
        .await?;

        decode_abi_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data.entireSystemDebt))?
    }

    /// Fetches the redemption rate (including decay) for the current state.
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

    /// Fetches the unbacked portion price and redeemability.
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

    /// Fetches multiple sorted troves starting from a given index.
    async fn fetch_multiple_sorted_troves(
        &self,
        index: U256,
        count: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<Vec<DebtPerInterestRate>> {
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
        .map(|data| Ok(data._0))?
    }

    /// Fetches the total unbacked amount across all collateral markets.
    async fn fetch_total_unbacked(
        &self,
        initial_value: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<U256> {
        let managers: Vec<Address> =
            MANAGERS.with(|managers_vector| managers_vector.borrow().clone());

        let mut total_unbacked = initial_value;

        for manager in managers {
            total_unbacked += self
                .fetch_unbacked_portion_price_and_redeemablity(Some(manager), block_tag.clone())
                .await?
                ._0;
        }

        Ok(total_unbacked)
    }

    /// Returns the current debt in front of the user's batch.
    fn get_current_debt_in_front(&self, troves: Vec<DebtPerInterestRate>) -> Option<U256> {
        let mut counted_debt = U256::from(0);

        for trove in troves.iter() {
            if trove.interestBatchManager == self.settings.batch_manager {
                return Some(trove.debt);
            }
            counted_debt = counted_debt.saturating_add(trove.debt);
        }
        None
    }

    /// Runs the strategy by analyzing troves and calculating changes if necessary.
    async fn run_strategy(
        &self,
        troves: Vec<DebtPerInterestRate>,
        time_since_last_update: U256,
        upfront_fee_period: U256,
        maximum_redeemable_against_collateral: U256,
        target_percentage: U256,
        block_tag: BlockTag,
    ) -> ManagerResult<Option<(U256, U256)>> {
        if let Some(current_debt_in_front) = self.get_current_debt_in_front(troves.clone()) {
            // Calculate new rate
            let new_rate = self
                .calculate_new_rate(
                    troves,
                    target_percentage,
                    maximum_redeemable_against_collateral,
                )
                .await?;

            // Predict upfront fee
            let upfront_fee = self.predict_upfront_fee(new_rate, block_tag).await?;

            // Check conditions to execute the strategy
            if self.increase_check(
                current_debt_in_front,
                maximum_redeemable_against_collateral,
                target_percentage,
            ) || (self.first_decrease_check(
                current_debt_in_front,
                maximum_redeemable_against_collateral,
                target_percentage,
            ) && self.second_decrease_check(
                time_since_last_update,
                upfront_fee_period,
                new_rate,
                upfront_fee,
            )?) {
                return Ok(Some((new_rate, upfront_fee)));
            }
        }

        Ok(None)
    }

    /// Calculates the new rate for interest.
    async fn calculate_new_rate(
        &self,
        troves: Vec<DebtPerInterestRate>,
        target_percentage: U256,
        maximum_redeemable_against_collateral: U256,
    ) -> ManagerResult<U256> {
        let mut counted_debt = U256::from(0);
        let mut new_rate = U256::from(0);
        for trove in troves.iter() {
            if counted_debt > target_percentage * maximum_redeemable_against_collateral {
                new_rate = trove
                    .interestRate
                    .saturating_add(U256::from(100_000_000_000_000_u128)); // Increment rate by 1 bps (0.01%)
                break;
            }
            counted_debt += trove.debt;
        }
        Ok(new_rate)
    }

    /// Checks if the conditions for increasing debt are met.
    fn increase_check(
        &self,
        debt_in_front: U256,
        maximum_redeemable_against_collateral: U256,
        target_percentage: U256,
    ) -> bool {
        if debt_in_front
            < (U256::from(SCALE) - tolerance_margin_down())
                * target_percentage
                * maximum_redeemable_against_collateral
        {
            return true;
        }
        false
    }

    /// First check for decreasing debt.
    fn first_decrease_check(
        &self,
        debt_in_front: U256,
        maximum_redeemable_against_collateral: U256,
        target_percentage: U256,
    ) -> bool {
        if debt_in_front
            > (U256::from(SCALE) + tolerance_margin_up())
                * maximum_redeemable_against_collateral
                * target_percentage
        {
            return true;
        }
        false
    }

    /// Second check for decreasing debt based on update time, rate difference, and upfront fee.
    fn second_decrease_check(
        &self,
        time_since_last_update: U256,
        upfront_fee_period: U256,
        new_rate: U256,
        average_rate: U256,
    ) -> ManagerResult<bool> {
        let r = time_since_last_update
            .checked_div(upfront_fee_period)
            .ok_or(arithmetic_err("Upfront fee period was 0."))?;
        if (U256::from(1) - r) * (self.data.latest_rate - new_rate) > average_rate
            || time_since_last_update > upfront_fee_period
        {
            return Ok(true);
        }
        Ok(false)
    }
}

impl Drop for ExecutableStrategy {
    /// Unlocks the strategy when the instance goes out of scope
    /// Ensures that resources are freed and the strategy is no longer locked
    fn drop(&mut self) {
        self.unlock();
        JournalEntry::new(Ok(()))
            .note("Executable strategy is dropped.")
            .strategy(self.settings.key)
            .commit();
    }
}

/*
========================================
= May the rates be ever in your favor  =
========================================
*/
