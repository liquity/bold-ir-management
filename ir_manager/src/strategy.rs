use alloy_primitives::U256;

use crate::types::CombinedTroveData;

pub async fn run_strategy() {
    // Check if decrease/increase is valid
    // Update the rate if needed
    todo!()
}

pub fn calculate_new_ir(troves: Vec<CombinedTroveData>, target_amount: U256) -> f64 {
    for (index, trove) in troves.iter().enumerate() {
        if trove.debt > target_amount {}
    }
}

pub fn increase_check(
    front_debt: u64,
    min_target: u64,
    current_redemption_fee: u64,
    threshold: u64,
    days_since_last_update: u64,
) -> bool {
    let days_factor = 7.0 - days_since_last_update as f64;
    let redemption_factor = current_redemption_fee as f64 / 0.005;
    let effective_days_factor = if days_factor < 0.01 {
        0.01
    } else {
        days_factor
    };

    if front_debt < min_target && (days_since_last_update < 7 || current_redemption_fee > threshold)
    {
        return true;
    }
    if front_debt < min_target
        && (7.0 / effective_days_factor * redemption_factor > threshold as f64)
    {
        return true;
    }
    false
}

pub fn decrease_check(front_debt: u64, max_target: u64, days_since_last_update: u64) -> bool {
    if front_debt > max_target && days_since_last_update > 7 {
        return true;
    }
    false
}
