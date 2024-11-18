#![allow(dead_code)]

use std::str::FromStr;

use alloy::consensus::TxEip1559;
use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_sol_types::SolCall;
use candid::{Nat, Principal};
use evm_rpc_types::{
    HttpOutcallError, MultiRpcResult, RpcApi, RpcConfig, RpcError, RpcService, RpcServices,
};
use ic_exports::ic_cdk::{
    self,
    api::{
        call::CallResult,
        is_controller,
        management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
    },
    call, id,
};
use serde_json::json;

use crate::{
    error::*,
    evm_rpc::*,
    exchange::*,
    gas::{estimate_transaction_fees, get_estimate_gas, FeeEstimates},
    signer::sign_eip1559_transaction,
    state::{
        CHAIN_ID, CKETH_LEDGER, DEFAULT_MAX_RESPONSE_BYTES, EXCHANGE_RATE_CANISTER, RPC_SERVICE, SCALE,
    },
    types::{Account, DerivationPath},
};
use num_traits::ToPrimitive;

/// Returns the estimated cycles cost of performing the RPC call if successful
pub async fn estimate_cycles(
    rpc_canister: &Service,
    json_data: String,
    max_response_bytes: u64,
) -> ManagerResult<u128> {
    let rpc = get_rpc_service();
    let call_result = rpc_canister
        .request_cost(rpc, json_data, max_response_bytes)
        .await;

    let extracted_call_result = extract_call_result(call_result)?;

    match extracted_call_result {
        Ok(cost) => {
            let cost_u128 = u128::try_from(cost.0).map_err(|err| {
                ManagerError::DecodingError(format!("Error converting Nat to u128: {:#?}", err))
            })?;
            Ok(cost_u128)
        }
        Err(rpc_err) => Err(ManagerError::RpcResponseError(rpc_err)),
    }
}

/// Returns Err if the `caller` is not a controller of the canister
pub fn only_controller(caller: Principal) -> ManagerResult<()> {
    if !is_controller(&caller) {
        // only the controller should be able to call this function
        return Err(ManagerError::Unauthorized);
    }
    Ok(())
}

/// Converts String to Address and returns ManagerError on failure
pub fn string_to_address(input: String) -> ManagerResult<Address> {
    Address::from_str(&input).map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))
}

/// Converts values of type `Nat` to `U256`
pub fn nat_to_u256(n: &Nat) -> ManagerResult<U256> {
    let be_bytes = n.0.to_bytes_be();
    if be_bytes.len() > 32 {
        return Err(ManagerError::DecodingError(format!("The `Nat` input length exceedes 32 bytes when converted to big-endian bytes representation.")));
    }
    // Ensure the byte array is exactly 32 bytes long
    let mut padded_bytes = [0u8; 32];
    let start_pos = 32 - be_bytes.len();
    padded_bytes[start_pos..].copy_from_slice(&be_bytes);

    Ok(U256::from_be_bytes(padded_bytes))
}

pub async fn fetch_cketh_balance() -> ManagerResult<Nat> {
    let ledger_principal = CKETH_LEDGER.with(|ledger| ledger.get());
    let args = Account {
        owner: id(),
        subaccount: None,
    };

    let call_response: CallResult<(Nat,)> =
        call(ledger_principal, "icrc1_balance_of", (args,)).await;

    match call_response {
        // We are hardcoding 18 decimals points for ckETH, as it will always reflect the original Ether token's metadata (and that is immutable).
        Ok(response) => Ok(response.0 / SCALE),
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}

pub async fn fetch_ether_cycles_rate() -> ManagerResult<u64> {
    let exchange_rate_canister = EXCHANGE_RATE_CANISTER.with(|principal_id| principal_id.get());
    let fetch_args = GetExchangeRateRequest {
        base_asset: Asset {
            symbol: "ETH".to_string(),
            class: AssetClass::Cryptocurrency,
        },
        quote_asset: Asset {
            symbol: "CXDR".to_string(),
            class: AssetClass::FiatCurrency,
        },
        timestamp: None,
    };

    let call_result: CallResult<(GetExchangeRateResult,)> =
        ic_cdk::api::call::call_with_payment128(
            exchange_rate_canister,
            "get_exchange_rate",
            (fetch_args,),
            1_000_000_000,
        )
        .await;
    let canister_response = extract_call_result(call_result)?;
    match canister_response {
        Ok(response) => Ok(response
            .rate
            .checked_div(response.metadata.decimals as u64)
            .ok_or(arithmetic_err("ETH/CXDR decimals value was zero."))?),
        Err(err) => Err(ManagerError::Custom(format!(
            "Error from the exchange rate canister: {:#?}",
            err
        ))),
    }
}

/// Returns `T` from Solidity struct.
pub fn decode_abi_response<T, F: SolCall<Return = T>>(hex_data: String) -> ManagerResult<T> {
    F::abi_decode_returns(hex_data.as_bytes(), false)
        .map_err(|err| ManagerError::DecodingError(err.to_string()))
}

pub fn eth_call_args(to: String, data: Vec<u8>, hex_block_number: &str) -> String {
    json!({
        "id": 1,
        "jsonrpc": "2.0",
        "params": [ {
            "to": to,
            "data": format!("0x{}", hex::encode(data))
        },
        hex_block_number
        ],
        "method": "eth_call"
    })
    .to_string()
}

pub async fn get_block_tag(rpc_canister: &Service) -> ManagerResult<BlockTag> {
    let rpc = get_rpc_services();
    let rpc_config = get_rpc_config(Some(2_000));

    let call_result = rpc_canister
        .get_block_by_number(rpc, Some(rpc_config), BlockTag::Latest)
        .await;
    let rpc_result = extract_call_result(call_result)?;
    let result = extract_multi_rpc_result(rpc_result)?;

    let safe_block = result.number - Nat::from(32_u8);

    // let safe_block_converted = Nat256::try_from(safe_block).map_err(|err| {
    //     ManagerError::DecodingError(format!(
    //         "Could not convert current block number to a Nat256: {:#?}",
    //         err
    //     ))
    // })?;

    Ok(BlockTag::Number(safe_block))
}

pub async fn send_raw_transaction(
    to: String,
    from: String,
    data: Vec<u8>,
    value: U256,
    nonce: u64,
    derivation_path: DerivationPath,
    rpc_canister: &Service,
    cycles: u128,
) -> ManagerResult<MultiRpcResult<SendRawTransactionStatus>> {
    let chain_id = CHAIN_ID.with(|id| id.get());
    let input = Bytes::from(data.clone());
    let rpc: RpcServices = get_rpc_services();

    let FeeEstimates {
        max_fee_per_gas,
        max_priority_fee_per_gas,
    } = estimate_transaction_fees(9, rpc.clone(), rpc_canister).await?;

    let estimated_gas = get_estimate_gas(rpc_canister, data, to.clone(), from).await?;

    let key_id = EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: String::from("key_1"),
    };

    let request = TxEip1559 {
        chain_id,
        to: TxKind::Call(
            Address::from_str(&to)
                .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?,
        ),
        max_fee_per_gas,
        max_priority_fee_per_gas,
        value,
        nonce,
        gas_limit: estimated_gas.to::<u128>(),
        access_list: Default::default(),
        input,
    };

    let signed_transaction = sign_eip1559_transaction(request, key_id, derivation_path).await;

    match rpc_canister
        .eth_send_raw_transaction(rpc, None, signed_transaction, cycles)
        .await
    {
        Ok((response,)) => Ok(response),
        Err(e) => Err(ManagerError::Custom(e.1)),
    }
}

fn is_response_size_error(err: &RpcError) -> bool {
    if let RpcError::HttpOutcallError(HttpOutcallError::IcError { code, message }) = err {
        *code == ic_cdk::api::call::RejectionCode::SysFatal
            && (message.contains("size limit") || message.contains("length limit"))
    } else {
        false
    }
}

pub fn get_rpc_services() -> RpcServices {
    RpcServices::EthMainnet(None)
}

pub fn get_rpc_config(max_response_bytes: Option<u64>) -> RpcConfig {
    RpcConfig {
        response_size_estimate: max_response_bytes,
        response_consensus: Some(evm_rpc_types::ConsensusStrategy::Threshold {
            total: Some(3),
            min: 2,
        }),
    }
}

/// Performs `eth_call` calls to the EVM RPC canister and doubles the max response bytes argument, if insufficient
/// Exits the loop if either of the following are satisfied:
/// A) The EVM RPC canister responds with Ok() or an error that is not related to the response size
/// B) The limit of 2MB is reached.
/// NOTE: Use the `request_with_dynamic_retries` to make requests
pub async fn call_with_dynamic_retries(
    rpc_canister: &Service,
    block: BlockTag,
    to: Address,
    data: Vec<u8>,
) -> ManagerResult<String> {
    let mut max_response_bytes = DEFAULT_MAX_RESPONSE_BYTES.with(|value| value.get());
    let provider_set: RpcServices = get_rpc_services();

    // There is a 2 MB limit on the response size, an ICP limitation.
    while max_response_bytes < 2_000_000 {
        // Perform the request using the provided function
        let mut transaction = TransactionRequest::default();
        transaction.to = Some(to.to_string());
        transaction.input = Some(format!("{:?}", data));

        let args = CallArgs {
            transaction,
            block: Some(block.clone()),
        };

        let config = get_rpc_config(Some(max_response_bytes));

        let response = rpc_canister
            .eth_call(provider_set.clone(), Some(config), args)
            .await;

        let extracted_response = extract_call_result(response)?;
        let extracted_rpc_result = extract_multi_rpc_result(extracted_response);

        if let Err(ManagerError::RpcResponseError(err)) = extracted_rpc_result.clone() {
            if is_response_size_error(&err) {
                max_response_bytes *= 2;
                continue;
            }
        }

        // note: if the code has reached this line, it means that a response unrelated to the size was received.
        return extracted_rpc_result;
    }

    Err(ManagerError::Custom(format!(
        "Request with dynamic retries reached its ceiling of 2 Megabytes."
    )))
}

pub fn get_rpc_service() -> RpcService {
    RPC_SERVICE.with(|rpc| {
        let mut state = rpc.borrow_mut();
        // we can safely unwrap, because the RPC services are never deleted, just rotated.
        let rpc = state.front().unwrap().clone();
        state.rotate_left(1);
        rpc
    })
}

/// Performs `request` calls to the EVM RPC canister and doubles the max response bytes argument, if insufficient
/// Exits the loop if either of the following are satisfied:
/// A) The EVM RPC canister responds with Ok() or an error that is not related to the response size
/// B) The limit of 2MB is reached.
/// NOTE: Use the `call_with_dynamic_retries` for making `eth_call` queries
pub async fn request_with_dynamic_retries(
    rpc_canister: &Service,
    json_data: String,
) -> ManagerResult<String> {
    let mut max_response_bytes = DEFAULT_MAX_RESPONSE_BYTES.with(|value| value.get());
    let mut rpc = get_rpc_service();
    let mut rpc_changes = 0;

    // There is a 2 MB limit on the response size, an ICP limitation.
    while max_response_bytes < 2_000_000 && rpc_changes < 3 {
        // Estimate the cycles based on the current max response bytes
        let cycles = estimate_cycles(rpc_canister, json_data.clone(), max_response_bytes).await?;

        // Perform the request using the provided function
        let call_result = rpc_canister
            .request(rpc.clone(), json_data.clone(), max_response_bytes, cycles)
            .await;

        let extracted_response = extract_call_result(call_result)?
            .map_err(|rpc_err| ManagerError::RpcResponseError(rpc_err));

        if let Err(ManagerError::RpcResponseError(err)) = extracted_response.clone() {
            if is_response_size_error(&err) {
                max_response_bytes *= 2;
                continue;
            }
            rpc = get_rpc_service();
            rpc_changes += 1;
            continue;
        }
        return extracted_response;
    }

    Err(ManagerError::Custom(format!(
        "Request with dynamic retries reached its ceiling of 2 Megabytes."
    )))
}

/// On success, returns the nonce associated with the given address
pub async fn get_nonce(rpc_canister: &Service, address: Address) -> ManagerResult<U256> {
    let account = address.to_string();
    let rpc: RpcServices = get_rpc_services();
    let args = GetTransactionCountArgs {
        address: account,
        block: BlockTag::Latest,
    };

    let config = RpcConfig {
        response_size_estimate: Some(10_000),
        response_consensus: Some(evm_rpc_types::ConsensusStrategy::Threshold {
            total: Some(3),
            min: 2,
        }),
    };

    let result = rpc_canister
        .eth_get_transaction_count(rpc, Some(config), args)
        .await;

    let wrapped_number = extract_call_result::<MultiRpcResult<Nat>>(result)?;
    let number = extract_multi_rpc_result(wrapped_number)?;
    nat_to_u256(&number)
}

/// Extracts result from `MultiRpcResult`, if the threshold is met.
pub fn extract_multi_rpc_result<T>(result: MultiRpcResult<T>) -> ManagerResult<T> {
    match result {
        MultiRpcResult::Consistent(response) => {
            response.map_err(|rpc_err| ManagerError::RpcResponseError(rpc_err))
        }
        MultiRpcResult::Inconsistent(_) => Err(ManagerError::NoConsensus),
    }
}

/// Extracts the Ok or Err values of a canister call and returns them.
pub fn extract_call_result<T>(result: CallResult<(T,)>) -> ManagerResult<T> {
    result
        .map(|(success_value,)| success_value)
        .map_err(|(rejection_code, error_message)| {
            ManagerError::CallResult(rejection_code, error_message)
        })
}
