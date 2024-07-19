use alloy_primitives::U256;
use candid::Nat;
use serde_bytes::ByteBuf;
use std::{ops::Add, str::FromStr};

use crate::{
    evm_rpc::{
        BlockTag, FeeHistory, FeeHistoryArgs, FeeHistoryResult, MultiFeeHistoryResult, RpcServices,
        Service,
    },
    types::ManagerError,
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

    let cycles = 10_000_000_000;

    match evm_rpc
        .eth_fee_history(rpc_services, None, fee_history_args, cycles)
        .await
    {
        Ok((res,)) => match res {
            MultiFeeHistoryResult::Consistent(fee_history) => match fee_history {
                FeeHistoryResult::Ok(fee_history) => fee_history.ok_or_else(|| ManagerError::NonExistentValue),
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
    let base_fee_per_gas = fee_history.baseFeePerGas.last().ok_or_else(|| ManagerError::NonExistentValue)?.clone();

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

pub fn nat_to_u256(n: &Nat) -> Result<U256, ManagerError> {
    let string_value = n.to_string();
    U256::from_str(&string_value).map_err(|err| ManagerError::Custom(format!("{:#?}", err)))
}
