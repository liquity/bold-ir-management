use alloy_primitives::U256;

use crate::{
    state::{TOLERANCE_MARGIN_DOWN, TOLERANCE_MARGIN_UP},
    types::CombinedTroveData,
};

pub async fn run_strategy(
    troves: Vec<CombinedTroveData>,
    time_since_last_update: U256,
    latest_rate: U256,
    average_rate: U256,
    upfront_fee_period: U256,
    debt_in_front: U256,
    target_amount: U256,
    redemption_fee: U256,
    target_min: U256,
) -> Option<(U256, U256, U256)> {
    // Check if decrease/increase is valid
    if increase_check(debt_in_front, target_amount, redemption_fee, target_min) {
        // calculate new rate and return it.
        return Some(calculate_new_rate(troves, target_amount));
    } else if first_decrease_check(debt_in_front, target_amount, redemption_fee, target_min) {
        // calculate new rate
        let new_rate = calculate_new_rate(troves, target_amount);
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

fn calculate_new_rate(troves: Vec<CombinedTroveData>, target_amount: U256) -> (U256, U256, U256) {
    for (index, trove) in troves.iter().enumerate() {
        if trove.debt > target_amount {}
    }
}

fn increase_check(
    debt_in_front: U256,
    target_amount: U256,
    redemption_fee: U256,
    target_min: U256,
) -> bool {
    let tolerance_margin_down = TOLERANCE_MARGIN_DOWN.get();

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
    let tolerance_margin_up = TOLERANCE_MARGIN_UP.get();

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
