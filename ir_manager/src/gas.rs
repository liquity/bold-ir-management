use alloy_primitives::U256;
use candid::Nat;

use serde_bytes::ByteBuf;
use serde_json::json;
use std::{ops::Add, str::FromStr};

use crate::{
    evm_rpc::{
        BlockTag, EthCallResponse, FeeHistory, FeeHistoryArgs, FeeHistoryResult,
        MultiFeeHistoryResult, RpcServices, Service,
    },
    types::ManagerError,
    utils::{decode_request_response_encoded, nat_to_u256, request_with_dynamic_retries},
};

/// The minimum suggested maximum priority fee per gas.
const MIN_SUGGEST_MAX_PRIORITY_FEE_PER_GAS: u32 = 1_500_000_000;

pub struct FeeEstimates {
    pub max_fee_per_gas: U256,
    pub max_priority_fee_per_gas: U256,
}

pub async fn fee_history(
    block_count: Nat,
    newest_block: BlockTag,
    reward_percentiles: Option<Vec<u8>>,
    rpc_services: RpcServices,
    evm_rpc: &Service,
) -> Result<FeeHistory, ManagerError> {
    let fee_history_args: FeeHistoryArgs = FeeHistoryArgs {
        blockCount: block_count,
        newestBlock: newest_block,
        rewardPercentiles: reward_percentiles.map(ByteBuf::from),
    };

    let cycles = 25_000_000_000;

    match evm_rpc
        .eth_fee_history(rpc_services, None, fee_history_args, cycles)
        .await
    {
        Ok((res,)) => match res {
            MultiFeeHistoryResult::Consistent(fee_history) => match fee_history {
                FeeHistoryResult::Ok(fee_history) => {
                    fee_history.ok_or(ManagerError::NonExistentValue)
                }
                FeeHistoryResult::Err(e) => Err(ManagerError::RpcResponseError(e)),
            },
            MultiFeeHistoryResult::Inconsistent(_) => Err(ManagerError::Custom(
                "Fee history is inconsistent".to_string(),
            )),
        },
        Err(e) => Err(ManagerError::Custom(e.1)),
    }
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
) -> Result<FeeEstimates, ManagerError> {
    let fee_history = fee_history(
        Nat::from(block_count),
        BlockTag::Latest,
        Some(vec![95]),
        rpc_services,
        evm_rpc,
    )
    .await?;

    let median_index = median_index(block_count.into());

    // baseFeePerGas
    let base_fee_per_gas = fee_history
        .baseFeePerGas
        .last()
        .ok_or(ManagerError::NonExistentValue)?
        .clone();

    // obtain the 95th percentile of the tips for the past 9 blocks
    let mut percentile_95: Vec<Nat> = fee_history
        .reward
        .into_iter()
        .flat_map(|x| x.into_iter())
        .collect();
    // sort the tips in ascending order
    percentile_95.sort_unstable();
    // get the median by accessing the element in the middle
    // set tip to 0 if there are not enough blocks in case of a local testnet
    let median_reward = percentile_95
        .get(median_index)
        .unwrap_or(&Nat::from(0_u8))
        .clone();

    let max_priority_fee_per_gas = median_reward
        .clone()
        .add(base_fee_per_gas)
        .max(Nat::from(MIN_SUGGEST_MAX_PRIORITY_FEE_PER_GAS));

    Ok(FeeEstimates {
        max_fee_per_gas: nat_to_u256(&max_priority_fee_per_gas)?,
        max_priority_fee_per_gas: nat_to_u256(&median_reward)?,
    })
}

pub async fn get_estimate_gas(
    rpc_canister: &Service,
    data: Vec<u8>,
    to: String,
    from: String,
) -> Result<U256, ManagerError> {
    let args = json!({
        "id": 1,
        "jsonrpc": "2.0",
        "params": [ {
            "from": from,
            "to": to,
            "data": format!("0x{}", hex::encode(data))
        },
        "latest"
        ],
        "method": "eth_estimateGas"
    })
    .to_string();

    let rpc_canister_response = request_with_dynamic_retries(rpc_canister, rpc_url, args).await?;

    let encoded_response = decode_request_response_encoded(rpc_canister_response)?;

    let decoded_response: EthCallResponse =
        serde_json::from_str(&encoded_response).map_err(|err| {
            ManagerError::DecodingError(format!(
                "Could not decode eth_estimateGas response: {} error: {}",
                &encoded_response, err
            ))
        })?;

    if decoded_response.result.len() <= 2 {
        return Err(ManagerError::DecodingError(format!(
            "The result field of the RPC's response is empty"
        )));
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
