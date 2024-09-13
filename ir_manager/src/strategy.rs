use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use candid::Principal;
use ic_exports::ic_cdk::{api::time, print};
use serde_json::json;

use crate::{
    evm_rpc::{EthCallResponse, SendRawTransactionResult, Service},
    state::{
        MANAGERS, MAX_NUMBER_OF_TROVES, SCALE, STRATEGY_DATA, TOLERANCE_MARGIN_DOWN,
        TOLERANCE_MARGIN_UP,
    },
    types::*,
    utils::{
        decode_request_response_encoded, decode_response, eth_call_args, get_block_number,
        request_with_dynamic_retries, send_raw_transaction, string_to_address,
    },
};

/// Struct containing all information necessary to execute a strategy
#[derive(Clone)]
pub struct StrategyData {
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
    /// Latest rate determined by the canister in the previous cycle
    pub latest_rate: U256,
    /// Derivation path of the ECDSA signature
    pub derivation_path: DerivationPath,
    /// Minimum target for this strategy
    pub target_min: f64,
    /// Upfront fee period constant denominated in seconds
    pub upfront_fee_period: U256,
    /// Timestamp of the last time the strategy had updated the batch's interest rate.
    /// Denominated in seconds.
    pub last_update: u64,
    /// Lock for the strategy. Determins if the strategy is currently being executed.
    pub lock: bool,
    /// The EOA's nonce
    pub eoa_nonce: u64,
    /// The EOA's public key
    pub eoa_pk: Option<Address>,
    /// RPC canister service
    pub rpc_canister: Service,
    /// RPC URL for the strategy.
    pub rpc_url: String,
}

impl Default for StrategyData {
    fn default() -> Self {
        Self {
            key: 0,
            batch_manager: Address::ZERO,
            hint_helper: Address::ZERO,
            manager: Address::ZERO,
            collateral_registry: Address::ZERO,
            multi_trove_getter: Address::ZERO,
            collateral_index: U256::ZERO,
            latest_rate: U256::ZERO,
            derivation_path: vec![],
            target_min: 0.0,
            upfront_fee_period: U256::ZERO,
            last_update: 0,
            lock: false,
            eoa_nonce: 0,
            eoa_pk: None,
            rpc_canister: Service(Principal::anonymous()),
            rpc_url: String::default(),
        }
    }
}

impl StrategyData {
    /// Generates a new strategy
    pub fn new(
        key: u32,
        manager: String,
        collateral_registry: String,
        multi_trove_getter: String,
        target_min: f64,
        rpc_canister: Service,
        rpc_url: String,
        upfront_fee_period: U256,
        collateral_index: U256,
        hint_helper: String,
        eoa_pk: Option<Address>,
        derivation_path: DerivationPath,
    ) -> Result<Self, ManagerError> {
        let result = Self {
            key,
            batch_manager: Address::ZERO,
            hint_helper: string_to_address(hint_helper)?,
            manager: string_to_address(manager)?,
            collateral_registry: string_to_address(collateral_registry)?,
            multi_trove_getter: string_to_address(multi_trove_getter)?,
            collateral_index,
            latest_rate: U256::from(0),
            derivation_path,
            target_min,
            upfront_fee_period,
            last_update: 0,
            lock: false,
            eoa_nonce: 0,
            eoa_pk,
            rpc_canister,
            rpc_url,
        };
        Ok(result)
    }

    /// Sets batch manager address for a certain strategy, if the address is not already set.
    pub fn set_batch_manager(key: u32, batch_manager: Address) -> Result<(), ManagerError> {
        STRATEGY_DATA.with(|strategies| {
            let mut binding = strategies.borrow_mut();
            let strategy = binding.get_mut(&key);

            if let Some(strategy_inner) = strategy {
                return match strategy_inner.batch_manager {
                    Address::ZERO => {
                        strategy_inner.batch_manager = batch_manager;
                        Ok(())
                    }
                    _ => Err(ManagerError::Custom(
                        "Batch manager is already set.".to_string(),
                    )),
                };
            }

            Err(ManagerError::NonExistentValue)
        })
    }

    /// Replaces the strategy data in the HashMap
    /// Mutably accesses the strategy data in the HashMap.
    fn apply_change(&self) {
        STRATEGY_DATA.with(|strategies| {
            strategies.borrow_mut().insert(self.key, self.clone());
        });
    }

    /// Locks the strategy.
    /// Mutably accesses the strategy data in the HashMap.
    fn lock(&mut self) -> Result<(), ManagerError> {
        if self.lock {
            // already processing
            return Err(ManagerError::Locked);
        }
        self.lock = true;
        self.apply_change();
        Ok(())
    }

    /// Unlocks the strategy.
    /// Mutably accesses the strategy data in the HashMap.
    pub fn unlock(&mut self) -> Result<(), ManagerError> {
        if !self.lock {
            // already unlocked
            return Err(ManagerError::Locked);
        }
        self.lock = false;
        self.apply_change();
        Ok(())
    }

    /// The only public function for this struct implementation. It runs the strategy and returns `Err` in case of failure.
    /// Mutably accesses the strategy data in the HashMap.
    pub async fn execute(&mut self) -> Result<(), ManagerError> {
        // Lock the strategy
        self.lock()?;

        let block_number = get_block_number(&self.rpc_canister, &self.rpc_url).await?;
        let time_since_last_update = U256::from(time() - self.last_update);

        let entire_system_debt: U256 = self.fetch_entire_system_debt(&block_number).await?;
        let unbacked_portion_price_and_redeemability = self
            .fetch_unbacked_portion_price_and_redeemablity(None, &block_number)
            .await?;

        let mut troves: Vec<DebtPerInterestRate> = vec![];
        let mut troves_index = U256::from(0);
        let max_count = U256::from(MAX_NUMBER_OF_TROVES.with(|number| number.get()));
        loop {
            let fetched_troves = self
                .fetch_multiple_sorted_troves(troves_index, max_count, &block_number)
                .await?;
            let last_trove = fetched_troves.last().unwrap().clone();
            troves.extend(fetched_troves);
            if last_trove.debt == U256::ZERO && last_trove.interestRate == U256::ZERO {
                break;
            }
            troves_index += max_count;
        }

        let redemption_fee = self.fetch_redemption_rate(&block_number).await?;
        let total_unbacked = self
            .fetch_total_unbacked(unbacked_portion_price_and_redeemability._0, &block_number)
            .await?;
        let redemption_split = unbacked_portion_price_and_redeemability._0 / total_unbacked;
        let maximum_redeemable_against_collateral = redemption_split * entire_system_debt;

        let exponent: f64 = (0.005 * SCALE) / (redemption_fee.to::<u64>() as f64);
        let target_amount = self.target_min.powf(exponent) * SCALE;

        print(format!(
            "Target amount = {}, exponent = {}, redemption_fee as f64 = {}, self.target_min = {}",
            target_amount,
            exponent,
            redemption_fee.to::<u64>() as f64,
            self.target_min
        ));

        let strategy_result = self
            .run_strategy(
                troves,
                time_since_last_update,
                self.upfront_fee_period,
                maximum_redeemable_against_collateral,
                U256::from(target_amount),
                &block_number,
            )
            .await?;

        if let Some((new_rate, max_upfront_fee)) = strategy_result {
            // send a signed transaction to update the rate for the batch
            // get hints

            let upper_hint = U256::from(0);
            let lower_hint = U256::from(0);

            // update strategy data
            let payload = setNewRateCall {
                _newAnnualInterestRate: new_rate.to::<u128>(),
                _upperHint: upper_hint,
                _lowerHint: lower_hint,
                _maxUpfrontFee: max_upfront_fee + U256::from(1_000_000_000_000_000_u128), // + %0.001 ,
            };

            print(format!(
                "[TRANSACTION] Sending a new rate transaction with rate {} to batch manager {}...",
                new_rate, self.batch_manager
            ));

            let tx_response = send_raw_transaction(
                self.batch_manager.to_string(),
                self.eoa_pk.unwrap().to_string(),
                payload.abi_encode(),
                U256::ZERO,
                self.eoa_nonce,
                self.derivation_path.clone(),
                &self.rpc_canister,
                &self.rpc_url,
                1_000_000_000,
            )
            .await?;

            match tx_response {
                crate::evm_rpc::MultiSendRawTransactionResult::Consistent(tx_result) => {
                    return match tx_result {
                        crate::evm_rpc::SendRawTransactionResult::Ok(status) => match status {
                            crate::evm_rpc::SendRawTransactionStatus::Ok(_) => {
                                self.eoa_nonce += 1;
                                self.last_update = time();
                                self.latest_rate = new_rate;
                                self.apply_change();
                                print(format!("[TRANSACTION] Strategy number {}: New rate transaction was submitted successfully for batch manager {}.", self.key, self.batch_manager));
                                self.unlock()?;
                                Ok(())
                            }
                            crate::evm_rpc::SendRawTransactionStatus::NonceTooLow
                            | crate::evm_rpc::SendRawTransactionStatus::TooHigh => {
                                self.update_rate_with_nonce(
                                    new_rate,
                                    upper_hint,
                                    lower_hint,
                                    max_upfront_fee + U256::from(1_000_000_000_000_000_u128), // + %0.001
                                )
                                .await
                            }
                            crate::evm_rpc::SendRawTransactionStatus::InsufficientFunds => {
                                Err(ManagerError::Custom(
                                    format!("[GAS] Strategy {}: Not enough Ether balance to cover the gas fee.", self.key)
                                ))
                            }
                        },
                        crate::evm_rpc::SendRawTransactionResult::Err(err) => {
                            Err(ManagerError::RpcResponseError(err))
                        }
                    };
                }
                crate::evm_rpc::MultiSendRawTransactionResult::Inconsistent(
                    inconsistent_responses,
                ) => {
                    print(format!("[INCONSISTENCY DETECTED] Inconsistent RPC responses were received for strategy number {}. Investigating...", self.key));
                    for (_, response) in inconsistent_responses {
                        if let SendRawTransactionResult::Ok(
                            crate::evm_rpc::SendRawTransactionStatus::Ok(_),
                        ) = response
                        {
                            self.eoa_nonce += 1;
                            self.last_update = time();
                            self.latest_rate = new_rate;
                            self.apply_change();
                            print(format!("[TRANSACTION] Inconsistency ignored for strategy {}, as at least one RPC response was ok. New rate transaction was submitted successfully for batch manager {}.", self.key, self.batch_manager));
                            self.unlock()?;
                            return Ok(());
                        }
                    }
                    return Err(ManagerError::Custom(
                        "None of the RPC responses were OK.".to_string(),
                    ));
                }
            }
        }
        print(format!("[NO TRANSACTION] Strategy number {} finished its run successfully without submitting a transaction.", self.key));
        self.unlock()?;
        Ok(())
    }

    async fn update_rate_with_nonce(
        &mut self,
        rate: U256,
        upper_hint: U256,
        lower_hint: U256,
        max_upfront_fee: U256,
    ) -> Result<(), ManagerError> {
        // fetch nonce
        self.eoa_nonce = self.get_nonce().await?.to::<u64>();
        self.apply_change();

        // send tx with new nonce
        let payload = setNewRateCall {
            _newAnnualInterestRate: rate.to::<u128>(),
            _upperHint: upper_hint,
            _lowerHint: lower_hint,
            _maxUpfrontFee: max_upfront_fee,
        };

        let tx_response = send_raw_transaction(
            self.batch_manager.to_string(),
            self.eoa_pk.unwrap().to_string(),
            payload.abi_encode(),
            U256::ZERO,
            self.eoa_nonce,
            self.derivation_path.clone(),
            &self.rpc_canister,
            &self.rpc_url,
            1_000_000_000,
        )
        .await?;

        match tx_response {
            crate::evm_rpc::MultiSendRawTransactionResult::Consistent(tx_result) => match tx_result
            {
                crate::evm_rpc::SendRawTransactionResult::Ok(status) => match status {
                    crate::evm_rpc::SendRawTransactionStatus::Ok(_) => {
                        self.eoa_nonce += 1;
                        self.apply_change();
                        self.unlock()?;
                        Ok(())
                    }
                    crate::evm_rpc::SendRawTransactionStatus::NonceTooLow => Err(
                        ManagerError::Custom("Could not detect the right nonce.".to_string()),
                    ),
                    crate::evm_rpc::SendRawTransactionStatus::TooHigh => Err(ManagerError::Custom(
                        "Could not detect the right nonce.".to_string(),
                    )),
                    crate::evm_rpc::SendRawTransactionStatus::InsufficientFunds => {
                        Err(ManagerError::Custom(
                            "Not enough Ether balance to cover the gas fee.".to_string(),
                        ))
                    }
                },
                crate::evm_rpc::SendRawTransactionResult::Err(err) => {
                    Err(ManagerError::RpcResponseError(err))
                }
            },
            crate::evm_rpc::MultiSendRawTransactionResult::Inconsistent(inconsistent_responses) => {
                for (_, response) in inconsistent_responses {
                    if let SendRawTransactionResult::Ok(
                        crate::evm_rpc::SendRawTransactionStatus::Ok(_),
                    ) = response
                    {
                        return Ok(());
                    }
                }
                Err(ManagerError::Custom(
                    "None of the RPC responses were OK.".to_string(),
                ))
            }
        }
    }

    pub async fn get_nonce(&self) -> Result<U256, ManagerError> {
        let request_json = json!({
            "id": 1,
            "jsonrpc": "2.0",
            "params": [
            self.eoa_pk,
            "latest"
            ],
            "method": "eth_getTransactionCount"
        })
        .to_string();

        let rpc_canister_response =
            request_with_dynamic_retries(&self.rpc_canister, &self.rpc_url, request_json).await?;

        let encoded_response = decode_request_response_encoded(rpc_canister_response)?;

        let decoded_response: EthCallResponse =
            serde_json::from_str(&encoded_response).map_err(|err| {
                ManagerError::DecodingError(format!(
                    "Could not decode eth_getTransactionCount reponse: {} error: {}",
                    &encoded_response, err
                ))
            })?;

        let hex_string = if decoded_response.result[2..].len() % 2 == 1 {
            format!("0{}", &decoded_response.result[2..])
        } else {
            decoded_response.result[2..].to_string()
        };

        let hex_decoded_response = hex::decode(hex_string)
            .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;

        Ok(U256::from_be_slice(&hex_decoded_response))
    }

    async fn predict_upfront_fee(
        &self,
        new_rate: U256,
        block_number: &str,
    ) -> Result<U256, ManagerError> {
        let arguments = predictAdjustBatchInterestRateUpfrontFeeCall {
            _collIndex: self.collateral_index,
            _batchAddress: self.batch_manager,
            _newInterestRate: new_rate,
        };

        let json_data = eth_call_args(
            self.hint_helper.to_string(),
            predictAdjustBatchInterestRateUpfrontFeeCall::abi_encode(&arguments),
            block_number,
        );

        let rpc_canister_response =
            request_with_dynamic_retries(&self.rpc_canister, &self.rpc_url, json_data).await?;

        decode_response::<
            predictAdjustBatchInterestRateUpfrontFeeReturn,
            predictAdjustBatchInterestRateUpfrontFeeCall,
        >(rpc_canister_response)
        .map(|data| Ok(data._0))?
    }

    /// Returns the debt of the entire system across all markets if successful.
    async fn fetch_entire_system_debt(&self, block_number: &str) -> Result<U256, ManagerError> {
        let json_data = eth_call_args(
            self.manager.to_string(),
            getEntireSystemDebtCall::SELECTOR.to_vec(),
            block_number,
        );

        let rpc_canister_response =
            request_with_dynamic_retries(&self.rpc_canister, &self.rpc_url, json_data).await?;

        decode_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(rpc_canister_response)
            .map(|data| Ok(data.entireSystemDebt))?
    }

    async fn fetch_redemption_rate(&self, block_number: &str) -> Result<U256, ManagerError> {
        let json_data = eth_call_args(
            self.collateral_registry.to_string(),
            getRedemptionRateWithDecayCall::SELECTOR.to_vec(),
            block_number,
        );

        let rpc_canister_response =
            request_with_dynamic_retries(&self.rpc_canister, &self.rpc_url, json_data).await?;

        decode_response::<getRedemptionRateWithDecayReturn, getRedemptionRateWithDecayCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data._0))?
    }

    async fn fetch_unbacked_portion_price_and_redeemablity(
        &self,
        manager: Option<String>,
        block_number: &str,
    ) -> Result<getUnbackedPortionPriceAndRedeemabilityReturn, ManagerError> {
        let call_manager = match manager {
            Some(value) => value,
            None => self.manager.to_string(),
        };

        let json_data = eth_call_args(
            call_manager,
            getUnbackedPortionPriceAndRedeemabilityCall::SELECTOR.to_vec(),
            block_number,
        );

        let rpc_canister_response =
            request_with_dynamic_retries(&self.rpc_canister, &self.rpc_url, json_data).await?;

        decode_response::<
            getUnbackedPortionPriceAndRedeemabilityReturn,
            getUnbackedPortionPriceAndRedeemabilityCall,
        >(rpc_canister_response)
    }

    async fn fetch_multiple_sorted_troves(
        &self,
        index: U256,
        count: U256,
        block_number: &str,
    ) -> Result<Vec<DebtPerInterestRate>, ManagerError> {
        let parameters = getDebtPerInterestRateAscendingCall {
            _collIndex: self.collateral_index,
            _startId: index,
            _maxIterations: count,
        };

        let json_data = eth_call_args(
            self.multi_trove_getter.to_string(),
            getDebtPerInterestRateAscendingCall::abi_encode(&parameters),
            block_number,
        );

        let rpc_canister_response =
            request_with_dynamic_retries(&self.rpc_canister, &self.rpc_url, json_data).await?;

        decode_response::<getDebtPerInterestRateAscendingReturn, getDebtPerInterestRateAscendingCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data._0))?
    }

    /// Fetches the total unbacked amount across all collateral markets excluding the ones defined in the parameter.
    async fn fetch_total_unbacked(
        &self,
        initial_value: U256,
        block_number: &str,
    ) -> Result<U256, ManagerError> {
        let managers: Vec<String> =
            MANAGERS.with(|managers_vector| managers_vector.borrow().clone());

        let mut total_unbacked = initial_value;

        for manager in managers {
            total_unbacked += self
                .fetch_unbacked_portion_price_and_redeemablity(Some(manager), block_number)
                .await?
                ._0;
        }

        Ok(total_unbacked)
    }

    fn get_current_debt_in_front(&self, troves: Vec<DebtPerInterestRate>) -> Option<U256> {
        let mut counted_debt = U256::from(0);

        for trove in troves.iter() {
            if trove.interestBatchManager == self.batch_manager {
                return Some(trove.debt);
            }
            counted_debt += trove.debt;
        }
        None
    }

    async fn run_strategy(
        &self,
        troves: Vec<DebtPerInterestRate>,
        time_since_last_update: U256,
        upfront_fee_period: U256,
        maximum_redeemable_against_collateral: U256,
        target_amount: U256,
        block_number: &str,
    ) -> Result<Option<(U256, U256)>, ManagerError> {
        if let Some(current_debt_in_front) = self.get_current_debt_in_front(troves.clone()) {
            // Check if decrease/increase is valid
            let new_rate = self.calculate_new_rate(troves, target_amount).await?;
            let upfront_fee = self.predict_upfront_fee(new_rate, block_number).await?;
            // return Ok(Some((new_rate, upfront_fee))); // You can uncomment this line to test the canister without waiting for an update condition to be satisfied.
            if self.increase_check(
                current_debt_in_front,
                maximum_redeemable_against_collateral,
                target_amount,
            ) {
                return Ok(Some((new_rate, upfront_fee)));
            } else if self.first_decrease_check(
                current_debt_in_front,
                maximum_redeemable_against_collateral,
                target_amount,
            ) && self.second_decrease_check(
                time_since_last_update,
                upfront_fee_period,
                new_rate,
                upfront_fee,
            ) {
                return Ok(Some((new_rate, upfront_fee)));
            }
        }

        Ok(None)
    }

    async fn calculate_new_rate(
        &self,
        troves: Vec<DebtPerInterestRate>,
        target_amount: U256,
    ) -> Result<U256, ManagerError> {
        let mut counted_debt = U256::from(0);
        let mut new_rate = U256::from(0);
        for trove in troves.iter() {
            if counted_debt > target_amount {
                new_rate = trove.interestRate + U256::from(100_000_000_000_000_u64); // 1 bps = 0.01%
                break;
            }
            counted_debt += trove.debt;
        }
        Ok(new_rate)
    }

    fn increase_check(
        &self,
        debt_in_front: U256,
        maximum_redeemable_against_collateral: U256,
        target_amount: U256,
    ) -> bool {
        let tolerance_margin_down =
            TOLERANCE_MARGIN_DOWN.with(|tolerance_margin_down| tolerance_margin_down.get());

        if debt_in_front
            < (U256::from(1) - tolerance_margin_down)
                * target_amount
                * maximum_redeemable_against_collateral
        {
            return true;
        }
        false
    }

    fn first_decrease_check(
        &self,
        debt_in_front: U256,
        maximum_redeemable_against_collateral: U256,
        target_amount: U256,
    ) -> bool {
        let tolerance_margin_up =
            TOLERANCE_MARGIN_UP.with(|tolerance_margin_up| tolerance_margin_up.get());

        if debt_in_front
            > (U256::from(1) + tolerance_margin_up)
                * maximum_redeemable_against_collateral
                * target_amount
        {
            return true;
        }
        false
    }

    fn second_decrease_check(
        &self,
        time_since_last_update: U256,
        upfront_fee_period: U256,
        new_rate: U256,
        average_rate: U256,
    ) -> bool {
        if (U256::from(1) - time_since_last_update / upfront_fee_period)
            * (self.latest_rate - new_rate)
            > average_rate
        {
            return true;
        } else if time_since_last_update > upfront_fee_period {
            return true;
        }
        false
    }
}
