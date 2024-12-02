//! Makes gas estimations and is used to submit a transaction through the TransactionBuilder

use alloy_primitives::U256;
use candid::Nat;
use evm_rpc_types::RpcServices;
use ic_exports::ic_cdk::print;
use serde_json::json;

use crate::providers::extract_multi_rpc_result;
use crate::types::*;

use super::common::{extract_call_result, request_with_dynamic_retries};
use super::error::{ManagerError, ManagerResult};
use super::evm_rpc::{BlockTag, FeeHistory, FeeHistoryArgs, Service};

/// The minimum suggested maximum priority fee per gas.
const MIN_SUGGEST_MAX_PRIORITY_FEE_PER_GAS: u64 = 1_500_000_000;

pub struct FeeEstimates {
    pub max_fee_per_gas: u128,
    pub max_priority_fee_per_gas: u128,
}

pub async fn fee_history(
    block_count: Nat,
    newest_block: BlockTag,
    reward_percentiles: Option<Vec<u8>>,
    rpc_services: RpcServices,
    evm_rpc: &Service,
) -> ManagerResult<FeeHistory> {
    let fee_history_args = FeeHistoryArgs {
        block_count,
        newest_block,
        reward_percentiles,
    };

    let cycles = 25_000_000_000;

    let call_result = evm_rpc
        .eth_fee_history(rpc_services.clone(), None, fee_history_args, cycles)
        .await;

    let canister_response = extract_call_result(call_result)?;

    extract_multi_rpc_result(rpc_services, canister_response)
}

fn median_index(length: usize) -> usize {
    if length == 0 {
        panic!("Cannot find a median index for an array of length zero.");
    }
    (length - 1) / 2
}

pub async fn estimate_transaction_fees(
    block_count: u8,
    rpc_services: RpcServices,
    evm_rpc: &Service,
    block_tag: BlockTag,
) -> ManagerResult<FeeEstimates> {
    let fee_history = fee_history(
        Nat::from(block_count),
        block_tag,
        Some(vec![95]),
        rpc_services,
        evm_rpc,
    )
    .await?;

    let median_index = median_index(block_count.into());

    // Convert baseFeePerGas to u128
    let base_fee_per_gas = fee_history
        .base_fee_per_gas
        .last()
        .ok_or(ManagerError::NonExistentValue)?;
    let base_fee_per_gas_u128 = u128::try_from(base_fee_per_gas.0.clone())
        .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;

    // obtain the 95th percentile of the tips for the past blocks
    let mut percentiles: Vec<Nat> = fee_history
        .reward
        .into_iter()
        .flat_map(|rewards| rewards.into_iter())
        .collect();

    // sort and retrieve the median reward
    percentiles.sort_unstable();
    let zero_nat = Nat::from(0_u32);
    let median_reward = percentiles.get(median_index).unwrap_or(&zero_nat);
    let median_reward_u128 = u128::try_from(median_reward.0.clone())
        .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;

    let max_priority_fee_per_gas = median_reward_u128
        .saturating_add(base_fee_per_gas_u128)
        .max(MIN_SUGGEST_MAX_PRIORITY_FEE_PER_GAS as u128);

    Ok(FeeEstimates {
        max_fee_per_gas: max_priority_fee_per_gas,
        max_priority_fee_per_gas: median_reward_u128,
    })
}

pub async fn get_estimate_gas(
    rpc_canister: &Service,
    data: Vec<u8>,
    to: String,
    from: String,
) -> ManagerResult<U256> {
    let args = json!({
        "id": 1,
        "jsonrpc": "2.0",
        "params": [{
            "from": from,
            "to": to,
            "data": format!("0x{}", hex::encode(data))
        },
        "latest"],
        "method": "eth_estimateGas"
    })
    .to_string();
    print(&args);
    let rpc_canister_response: String = request_with_dynamic_retries(rpc_canister, args).await?;

    let decoded_response: EthCallResponse =
        serde_json::from_str(&rpc_canister_response).map_err(|err| {
            ManagerError::DecodingError(format!(
                "Could not decode eth_estimateGas response: {} error: {}",
                &rpc_canister_response, err
            ))
        })?;

    if decoded_response.result.len() <= 2 {
        return Err(ManagerError::DecodingError(
            "The result field of the RPC's response is empty".to_string(),
        ));
    }

    let hex_string = if decoded_response.result[2..].len() % 2 == 1 {
        format!("0{}", &decoded_response.result[2..])
    } else {
        decoded_response.result[2..].to_string()
    };

    let hex_decoded_response = hex::decode(hex_string)
        .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;

    Ok(U256::from_be_slice(&hex_decoded_response))
}
