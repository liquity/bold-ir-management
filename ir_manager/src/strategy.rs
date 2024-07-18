use alloy_primitives::U256;
use alloy_sol_types::SolCall;

use crate::{
    evm_rpc::{RpcService, Service},
    state::{TOLERANCE_MARGIN_DOWN, TOLERANCE_MARGIN_UP},
    types::{getTroveAnnualInterestRateCall, getTroveAnnualInterestRateReturn, CombinedTroveData},
    utils::{decode_response, eth_call_args, rpc_provider},
};

pub async fn run_strategy(
    rpc_canister: &Service,
    rpc_url: &str,
    manager: &str,
    troves: Vec<CombinedTroveData>,
    time_since_last_update: U256,
    latest_rate: U256,
    average_rate: U256,
    upfront_fee_period: U256,
    debt_in_front: U256,
    target_amount: U256,
    redemption_fee: U256,
    target_min: U256,
) -> Option<U256> {
    // Check if decrease/increase is valid
    if increase_check(debt_in_front, target_amount, redemption_fee, target_min) {
        // calculate new rate and return it.
        return Some(
            calculate_new_rate(rpc_canister, rpc_url, manager, troves, target_amount).await,
        );
    } else if first_decrease_check(debt_in_front, target_amount, redemption_fee, target_min) {
        // calculate new rate
        let new_rate =
            calculate_new_rate(rpc_canister, rpc_url, manager, troves, target_amount).await;
        if second_decrease_check(
            time_since_last_update,
            upfront_fee_period,
            latest_rate,
            new_rate,
            average_rate,
        ) {
            // return the new rate;
            return Some(new_rate);
        }
    }
    None
}

async fn calculate_new_rate(
    rpc_canister: &Service,
    rpc_url: &str,
    manager: &str,
    troves: Vec<CombinedTroveData>,
    target_amount: U256,
) -> U256 {
    let mut counted_debt = U256::from(0);
    let mut new_rate = U256::from(0);
    for (_, trove) in troves.iter().enumerate() {
        if counted_debt > target_amount {
            // get trove current interest rate
            let rpc: RpcService = rpc_provider(rpc_url);

            let json_data = eth_call_args(
                manager.to_string(),
                getTroveAnnualInterestRateCall { _troveId: trove.id }.abi_encode(),
            );

            let rpc_canister_response = rpc_canister
                .request(rpc, json_data, 500000, 10_000_000_000)
                .await;

            let interest_rate = decode_response::<
                getTroveAnnualInterestRateReturn,
                getTroveAnnualInterestRateCall,
            >(rpc_canister_response)
            .map(|data| Ok(data))
            .unwrap_or_else(|e| Err(e))
            .unwrap()
            ._0;

            new_rate = interest_rate + U256::from(10000000000000000);
            break;
        }
        counted_debt += trove.debt;
    }
    new_rate
}

fn increase_check(
    debt_in_front: U256,
    target_amount: U256,
    redemption_fee: U256,
    target_min: U256,
) -> bool {
    let tolerance_margin_down = TOLERANCE_MARGIN_DOWN.with(|tolerance_margin_down| tolerance_margin_down.clone());

    if debt_in_front
        < (U256::from(1) - tolerance_margin_down)
            * (((target_amount * redemption_fee * target_min) / U256::from(5)) / U256::from(1000))
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
    let tolerance_margin_up = TOLERANCE_MARGIN_UP.with(|tolerance_margin_up| tolerance_margin_up.clone());

    if debt_in_front
        > (U256::from(1) + tolerance_margin_up)
            * (((target_amount * redemption_fee * target_min) / U256::from(5)) / U256::from(1000))
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
