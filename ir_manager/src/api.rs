use std::str::FromStr;

use alloy_primitives::U256;
use ic_exports::ic_kit::ic::time;

use crate::process::LiquityProcess;
use crate::types::*;
use crate::utils::{lock, unlock};
use crate::{strategy::run_strategy, types::ManagerError};

pub async fn execute_strategy(key: u32, strategy: &StrategyData) -> Result<(), ManagerError> {
    // Lock the strategy
    lock(key)?;

    let time_since_last_update = U256::from(time() - strategy.last_update);

    // Fetch data
    let exec = LiquityProcess::new(strategy);
    let entire_system_debt: U256 = exec.fetch_entire_system_debt().await?;

    let unbacked_portion_price_and_redeemability = exec
        .fetch_unbacked_portion_price_and_redeemablity(None)
        .await?;

    let troves = exec
        .fetch_multiple_sorted_troves(U256::from_str("1000").unwrap())
        .await?;

    // Calculate
    let redemption_fee = exec.fetch_redemption_rate().await?;
    let redemption_split = unbacked_portion_price_and_redeemability._0
        / exec.fetch_total_unbacked(vec![&exec.manager]).await?;
    let target_amount = redemption_split
        * entire_system_debt
        * ((redemption_fee * strategy.target_min) / U256::from(5))
        / U256::from(1000);

    let new_rate = run_strategy(
        &exec.rpc_canister,
        &exec.rpc_url,
        &strategy.manager,
        troves,
        time_since_last_update,
        strategy.latest_rate,
        average_rate,
        strategy.upfront_fee_period,
        debt_in_front,
        target_amount,
        redemption_fee,
        strategy.target_min,
    )
    .await;

    if let Some(rate) = new_rate {
        // send a signed transaction to update the rate for the batch
        // get hints

        // update strategy data
    }

    unlock(key)?;
    Ok(())
}
