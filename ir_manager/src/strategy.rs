use alloy_primitives::{Address, I256, U256};
use alloy_sol_types::SolCall;
use candid::Principal;
use ic_exports::ic_cdk::api::time;

use crate::{
    evm_rpc::{RpcService, Service},
    state::{MANAGERS, STRATEGY_DATA, TOLERANCE_MARGIN_DOWN, TOLERANCE_MARGIN_UP},
    types::*,
    utils::{decode_response, eth_call_args, get_block_number, rpc_provider},
};

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
    pub target_min: U256,
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
            target_min: U256::ZERO,
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
        manager: Address,
        collateral_registry: Address,
        multi_trove_getter: Address,
        target_min: U256,
        rpc_canister: Service,
        rpc_url: String,
        upfront_fee_period: U256,
        collateral_index: U256,
        hint_helper: Address,
        batch_manager: Address,
        eoa_pk: Option<Address>,
        derivation_path: DerivationPath,
    ) -> Self {
        Self {
            key,
            batch_manager,
            hint_helper,
            manager,
            collateral_registry,
            multi_trove_getter,
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
        }
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

        let mut troves: Vec<CombinedTroveData> = vec![];
        let mut troves_index = U256::from(0);
        loop {
            let fetched_troves = self
                .fetch_multiple_sorted_troves(troves_index, U256::from(1500), &block_number)
                .await?;
            let fetched_troves_count = fetched_troves.len();
            troves.extend(fetched_troves);
            if fetched_troves_count != 1500 {
                break;
            }
            troves_index += U256::from(1500);
        }

        let redemption_fee = self.fetch_redemption_rate(&block_number).await?;
        let redemption_split = unbacked_portion_price_and_redeemability._0
            / self
                .fetch_total_unbacked(unbacked_portion_price_and_redeemability._0, &block_number)
                .await?;
        let target_amount = redemption_split
            * entire_system_debt
            * ((redemption_fee * self.target_min) / U256::from(5))
            / U256::from(1000);

        let new_rate = self
            .run_strategy(
                troves,
                time_since_last_update,
                self.upfront_fee_period,
                target_amount,
                redemption_fee,
                &block_number,
            )
            .await?;

        if let Some(_rate) = new_rate {
            // send a signed transaction to update the rate for the batch
            // get hints

            // update strategy data
        }

        self.unlock()?;
        Ok(())
    }

    async fn predict_upfront_fee(
        &self,
        new_rate: U256,
        block_number: &str,
    ) -> Result<U256, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let arguments = predictAdjustBatchInterestRateUpfrontFeeCall {
            _collIndex: self.collateral_index,
            _batchAddress: self.batch_manager.clone(),
            _newInterestRate: new_rate,
        };

        let json_data = eth_call_args(
            self.hint_helper.to_string(),
            predictAdjustBatchInterestRateUpfrontFeeCall::abi_encode(&arguments),
            block_number,
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<
            predictAdjustBatchInterestRateUpfrontFeeReturn,
            predictAdjustBatchInterestRateUpfrontFeeCall,
        >(rpc_canister_response)
        .map(|data| Ok(data._0))
        .unwrap_or_else(|e| Err(e))
    }

    /// Returns the debt of the entire system across all markets if successful.
    async fn fetch_entire_system_debt(&self, block_number: &str) -> Result<U256, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let json_data = eth_call_args(
            self.manager.to_string(),
            getEntireSystemDebtCall::SELECTOR.to_vec(),
            block_number,
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(rpc_canister_response)
            .map(|data| Ok(data.entireSystemDebt))
            .unwrap_or_else(|e| Err(e))
    }

    async fn fetch_redemption_rate(&self, block_number: &str) -> Result<U256, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let json_data = eth_call_args(
            self.collateral_registry.to_string(),
            getRedemptionRateWithDecayCall::SELECTOR.to_vec(),
            block_number,
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getRedemptionRateWithDecayReturn, getRedemptionRateWithDecayCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data._0))
        .unwrap_or_else(|e| Err(e))
    }

    async fn fetch_unbacked_portion_price_and_redeemablity(
        &self,
        manager: Option<String>,
        block_number: &str,
    ) -> Result<getUnbackedPortionPriceAndRedeemabilityReturn, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let call_manager = match manager {
            Some(value) => value,
            None => self.manager.to_string(),
        };

        let json_data = eth_call_args(
            call_manager,
            getUnbackedPortionPriceAndRedeemabilityCall::SELECTOR.to_vec(),
            block_number,
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

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
    ) -> Result<Vec<CombinedTroveData>, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let parameters = getMultipleSortedTrovesCall {
            _startIdx: I256::from_raw(index),
            _count: count,
        };

        let json_data = eth_call_args(
            self.multi_trove_getter.to_string(),
            getMultipleSortedTrovesCall::abi_encode(&parameters),
            block_number,
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getMultipleSortedTrovesReturn, getMultipleSortedTrovesCall>(
            rpc_canister_response,
        )
        .map(|data| Ok(data._troves))
        .unwrap_or_else(|e| Err(e))
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

    fn get_current_debt_in_front(&self, troves: Vec<CombinedTroveData>) -> Option<U256> {
        let mut counted_debt = U256::from(0);

        for (_, trove) in troves.iter().enumerate() {
            if trove.interestBatchManager == self.batch_manager {
                return Some(trove.debt);
            }
            counted_debt += trove.debt;
        }
        None
    }

    async fn run_strategy(
        &self,
        troves: Vec<CombinedTroveData>,
        time_since_last_update: U256,
        upfront_fee_period: U256,
        target_amount: U256,
        redemption_fee: U256,
        block_number: &str,
    ) -> Result<Option<U256>, ManagerError> {
        if let Some(current_debt_in_front) = self.get_current_debt_in_front(troves.clone()) {
            // Check if decrease/increase is valid
            let new_rate = self
                .calculate_new_rate(troves, target_amount, block_number)
                .await?;
            if self.increase_check(current_debt_in_front, target_amount, redemption_fee) {
                return Ok(Some(new_rate));
            } else if self.first_decrease_check(
                current_debt_in_front,
                target_amount,
                redemption_fee,
            ) {
                let upfront_fee = self.predict_upfront_fee(new_rate, block_number).await?;
                if self.second_decrease_check(
                    time_since_last_update,
                    upfront_fee_period,
                    new_rate,
                    upfront_fee,
                ) {
                    return Ok(Some(new_rate));
                }
            }
        }

        Ok(None)
    }

    async fn calculate_new_rate(
        &self,
        troves: Vec<CombinedTroveData>,
        target_amount: U256,
        block_number: &str,
    ) -> Result<U256, ManagerError> {
        let mut counted_debt = U256::from(0);
        let mut new_rate = U256::from(0);
        for (_, trove) in troves.iter().enumerate() {
            if counted_debt > target_amount {
                // get trove current interest rate
                let rpc: RpcService = rpc_provider(&self.rpc_url);

                let json_data = eth_call_args(
                    self.manager.to_string(),
                    getTroveAnnualInterestRateCall { _troveId: trove.id }.abi_encode(),
                    block_number,
                );

                let rpc_canister_response = self
                    .rpc_canister
                    .request(rpc, json_data, 500000, 10_000_000_000)
                    .await;

                let interest_rate = decode_response::<
                    getTroveAnnualInterestRateReturn,
                    getTroveAnnualInterestRateCall,
                >(rpc_canister_response)?
                ._0;

                new_rate = interest_rate + U256::from(10000000000000000 as u64);
                break;
            }
            counted_debt += trove.debt;
        }
        Ok(new_rate)
    }

    fn increase_check(
        &self,
        debt_in_front: U256,
        target_amount: U256,
        redemption_fee: U256,
    ) -> bool {
        let tolerance_margin_down =
            TOLERANCE_MARGIN_DOWN.with(|tolerance_margin_down| tolerance_margin_down.get());

        if debt_in_front
            < (U256::from(1) - tolerance_margin_down)
                * (((target_amount * redemption_fee * self.target_min) / U256::from(5))
                    / U256::from(1000))
        {
            return true;
        }
        false
    }

    fn first_decrease_check(
        &self,
        debt_in_front: U256,
        target_amount: U256,
        redemption_fee: U256,
    ) -> bool {
        let tolerance_margin_up =
            TOLERANCE_MARGIN_UP.with(|tolerance_margin_up| tolerance_margin_up.get());

        if debt_in_front
            > (U256::from(1) + tolerance_margin_up)
                * (((target_amount * redemption_fee * self.target_min) / U256::from(5))
                    / U256::from(1000))
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
