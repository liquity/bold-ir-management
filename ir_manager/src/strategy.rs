use alloy_primitives::U256;
use alloy_sol_types::SolCall;
use ic_exports::ic_cdk::api::time;

use crate::{
    evm_rpc::{RpcService, Service},
    state::{TOLERANCE_MARGIN_DOWN, TOLERANCE_MARGIN_UP},
    types::*,
    utils::{decode_response, eth_call_args, rpc_provider},
};

#[derive(Clone)]
pub struct StrategyData {
    /// Manager contract address for this strategy
    pub manager: String,
    /// Multi trove getter contract address for this strategy
    pub multi_trove_getter: String,
    /// Latest rate determined by the canister in the previous cycle
    pub latest_rate: U256,
    /// Derivation path of the ECDSA signature
    pub derivation_path: DerivationPath,
    /// Minimum target for this strategy
    pub target_min: U256,
    /// Collateral registry contract address
    pub collateral_registry: String,
    /// RPC canister service
    pub rpc_canister: Service,
    pub upfront_fee_period: U256,
    pub eoa_nonce: u64,
    pub eoa_pk: Option<String>,
    pub last_update: u64,
    pub lock: bool,
    pub rpc_url: String,
}

impl StrategyData {
    /// Locks the strategy.
    fn lock(&mut self) -> Result<(), ManagerError> {
        if self.lock {
            // already processing
            return Err(ManagerError::Locked);
        }
        self.lock = true;
        Ok(())
    }

    /// Unlocks the strategy.
    fn unlock(&mut self) -> Result<(), ManagerError> {
        if !self.lock {
            // already unlocked
            return Err(ManagerError::Locked);
        }
        self.lock = false;
        Ok(())
    }

    /// The only public function for this struct implementation. It runs the strategy and returns `Err` in case of failure.
    pub async fn execute(&mut self) -> Result<(), ManagerError> {
        // Lock the strategy
        self.lock()?;

        let time_since_last_update = U256::from(time() - self.last_update); // what unit should this be in? millis? secs?

        let entire_system_debt: U256 = self.fetch_entire_system_debt().await?;
        let unbacked_portion_price_and_redeemability = self
            .fetch_unbacked_portion_price_and_redeemablity(None)
            .await?;

        let troves = self.fetch_multiple_sorted_troves(U256::from(1000)).await?; // TODO change fixed number 1000

        // let upfront_fee =

        let redemption_fee = self.fetch_redemption_rate().await?;
        let redemption_split = unbacked_portion_price_and_redeemability._0
            / self.fetch_total_unbacked(vec![&self.manager]).await?;
        let target_amount = redemption_split
            * entire_system_debt
            * ((redemption_fee * self.target_min) / U256::from(5))
            / U256::from(1000);

        let new_rate = self
            .run_strategy(
                troves,
                time_since_last_update,
                average_rate,
                strategy.upfront_fee_period,
                debt_in_front,
                target_amount,
                redemption_fee,
                strategy.target_min,
            )
            .await?;

        if let Some(rate) = new_rate {
            // send a signed transaction to update the rate for the batch
            // get hints

            // update strategy data
        }

        self.unlock()?;
        Ok(())
    }

    async fn predict_upfront_fee(&mut self) -> Result<U256, ManagerError> {
        let arguments = predictAdjustTroveUpfrontFeeCall {
            _collIndex: todo!(),
            _troveId: todo!(),
            _debtIncrease: todo!(),
        };
        let json_data = eth_call_args(
            self.hint_helper,
            predictAdjustTroveUpfrontFee::abi_encode(&arguments),
        );
    }

    /// Returns the debt of the entire system across all markets if successful.
    async fn fetch_entire_system_debt(&mut self) -> Result<U256, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let json_data = eth_call_args(
            self.manager.to_string(),
            getEntireSystemDebtCall::SELECTOR.to_vec(),
        );

        let rpc_canister_response = self
            .rpc_canister
            .request(rpc, json_data, 500000, 10_000_000_000)
            .await;

        decode_response::<getEntireSystemDebtReturn, getEntireSystemDebtCall>(rpc_canister_response)
            .map(|data| Ok(data.entireSystemDebt))
            .unwrap_or_else(|e| Err(e))
    }

    async fn fetch_redemption_rate(&mut self) -> Result<U256, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let json_data = eth_call_args(
            self.collateral_registry.to_string(),
            getRedemptionRateWithDecayCall::SELECTOR.to_vec(),
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
        &mut self,
        manager: Option<String>,
    ) -> Result<getUnbackedPortionPriceAndRedeemabilityReturn, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let call_manager = match manager {
            Some(value) => value,
            None => self.manager.clone(),
        };

        let json_data = eth_call_args(
            call_manager,
            getUnbackedPortionPriceAndRedeemabilityCall::SELECTOR.to_vec(),
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
        &mut self,
        count: U256,
    ) -> Result<Vec<CombinedTroveData>, ManagerError> {
        let rpc: RpcService = rpc_provider(&self.rpc_url);

        let start_index =
            I256::from_str("0").map_err(|err| ManagerError::Custom(format!("{:#?}", err)))?;

        let parameters = getMultipleSortedTrovesCall {
            _startIdx: start_index,
            _count: count,
        };

        let json_data = eth_call_args(
            self.multi_trove_getter.to_string(),
            getMultipleSortedTrovesCall::abi_encode(&parameters),
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
        &mut self,
        excluded_managers: Vec<&str>,
    ) -> Result<U256, ManagerError> {
        let managers: Vec<String> = MANAGERS.with(|managers_vector| {
            let mut filtered_managers = managers_vector.borrow().clone(); // Clone the vector
            filtered_managers.retain(|x| !excluded_managers.contains(&x.as_str())); // Then retain elements
            filtered_managers // Return the filtered vector
        });

        let mut total_unbacked = U256::from(0);

        for manager in managers {
            total_unbacked += self
                .fetch_unbacked_portion_price_and_redeemablity(Some(manager))
                .await?
                ._0;
        }

        Ok(total_unbacked)
    }

    async fn run_strategy(
        &mut self,
        troves: Vec<CombinedTroveData>,
        time_since_last_update: U256,
        average_rate: U256,
        upfront_fee_period: U256,
        debt_in_front: U256,
        target_amount: U256,
        redemption_fee: U256,
    ) -> Result<Option<U256>, ManagerError> {
        // Check if decrease/increase is valid
        if Self::increase_check(debt_in_front, target_amount, redemption_fee, target_min) {
            // calculate new rate and return it.
            return Ok(Some(
                self.calculate_new_rate(troves, target_amount)
                    .await?,
            ));
        } else if Self::first_decrease_check(
            debt_in_front,
            target_amount,
            redemption_fee,
            target_min,
        ) {
            // calculate new rate
            let new_rate =
                self.calculate_new_rate(troves, target_amount).await?;
            if second_decrease_check(
                time_since_last_update,
                upfront_fee_period,
                latest_rate,
                new_rate,
                average_rate,
            ) {
                // return the new rate;
                return Ok(Some(new_rate));
            }
        }
        Ok(None)
    }

    async fn calculate_new_rate(
        &mut self,
        troves: Vec<CombinedTroveData>,
        target_amount: U256,
    ) -> Result<U256, ManagerError> {
        let mut counted_debt = U256::from(0);
        let mut new_rate = U256::from(0);
        for (_, trove) in troves.iter().enumerate() {
            if counted_debt > target_amount {
                // get trove current interest rate
                let rpc: RpcService = rpc_provider(self.rpc_url);

                let json_data = eth_call_args(
                    manager.to_string(),
                    getTroveAnnualInterestRateCall { _troveId: trove.id }.abi_encode(),
                );

                let rpc_canister_response = self.rpc_canister
                    .request(rpc, json_data, 500000, 10_000_000_000)
                    .await;

                let interest_rate = decode_response::<
                    getTroveAnnualInterestRateReturn,
                    getTroveAnnualInterestRateCall,
                >(rpc_canister_response)?
                ._0;

                new_rate = interest_rate + U256::from(10000000000000000);
                break;
            }
            counted_debt += trove.debt;
        }
        Ok(new_rate)
    }

    fn increase_check(
        &mut self,
        debt_in_front: U256,
        target_amount: U256,
        redemption_fee: U256,
    ) -> bool {
        let tolerance_margin_down =
            TOLERANCE_MARGIN_DOWN.with(|tolerance_margin_down| tolerance_margin_down.get());

        if debt_in_front
            < (U256::from(1) - tolerance_margin_down)
                * (((target_amount * redemption_fee * target_min) / U256::from(5))
                    / U256::from(1000))
        {
            return true;
        }
        false
    }

    fn first_decrease_check(
        debt_in_front: U256,
        target_amount: U256,
        redemption_fee: U256,
        target_min: U256,
    ) -> bool {
        let tolerance_margin_up =
            TOLERANCE_MARGIN_UP.with(|tolerance_margin_up| tolerance_margin_up.get());

        if debt_in_front
            > (U256::from(1) + tolerance_margin_up)
                * (((target_amount * redemption_fee * target_min) / U256::from(5))
                    / U256::from(1000))
        {
            return true;
        }
        false
    }

    fn second_decrease_check(
        time_since_last_update: U256,
        upfront_fee_period: U256,
        latest_rate: U256,
        new_rate: U256,
        average_rate: U256,
    ) -> bool {
        if (U256::from(1) - time_since_last_update / upfront_fee_period) * (latest_rate - new_rate)
            > average_rate
        {
            return true;
        } else if time_since_last_update > upfront_fee_period {
            return true;
        }
        false
    }
}
