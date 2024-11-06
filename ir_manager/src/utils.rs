#![allow(dead_code)]

use std::{io::Read, str::FromStr};

use alloy::consensus::TxEip1559;
use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_sol_types::SolCall;
use candid::{Nat, Principal};
use evm_rpc_types::{
    BlockTag, CallArgs, GetTransactionCountArgs, Hex, Hex20, HttpOutcallError, MultiRpcResult, Nat256, RpcApi, RpcConfig, RpcError, RpcResult, RpcService, RpcServices, SendRawTransactionStatus, TransactionRequest
};
use ic_exports::{ic_cdk::{
    self,
    api::{
        call::CallResult,
        is_controller,
        management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
    },
    call, id,
}, ic_kit::RejectionCode};
use serde_json::json;

use crate::{
    evm_rpc::Service,
    exchange::*,
    gas::{estimate_transaction_fees, get_estimate_gas, FeeEstimates},
    signer::sign_eip1559_transaction,
    state::{
        get_provider_set, CHAIN_ID, CKETH_LEDGER, DEFAULT_MAX_RESPONSE_BYTES,
        EXCHANGE_RATE_CANISTER,
    },
    types::{Account, DerivationPath, EthCallResponse, ManagerError, ManagerResult, ProviderSet},
};
use num_traits::ToPrimitive;

/// Returns the estimated cycles cost of performing the RPC call if successful
pub async fn estimate_cycles(
    rpc_canister: &Service,
    rpc: RpcService,
    json_data: String,
    max_response_bytes: u64,
) -> ManagerResult<u128> {
    let canister_response = rpc_canister
        .request_cost(rpc, json_data, max_response_bytes)
        .await;
    match canister_response {
        Ok((request_cost_result,)) => match request_cost_result {
            RequestCostResult::Ok(amount) => {
                let cycles = amount.0.to_u128();
                if let Some(cycles_u128) = cycles {
                    return Ok(cycles_u128);
                }
                Err(ManagerError::DecodingError(String::from(
                    "Could not convert Nat response of request_cost to u128.",
                )))
            }
            RequestCostResult::Err(rpc_err) => Err(ManagerError::RpcResponseError(rpc_err)),
        },
        Err((_, err)) => Err(ManagerError::Custom(err)),
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
        Ok(response) => Ok(response.0 / 10_u64.pow(18)),
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

    let fetch_result: CallResult<(GetExchangeRateResult,)> =
        ic_cdk::api::call::call_with_payment128(
            exchange_rate_canister,
            "get_exchange_rate",
            (fetch_args,),
            1_000_000_000,
        )
        .await;
    match fetch_result {
        Ok(result) => match result {
            (Ok(response),) => Ok(response.rate / response.metadata.decimals as u64),
            (Err(err),) => Err(ManagerError::Custom(format!(
                "Error from the exchange rate canister: {:#?}",
                err
            ))),
        },
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}

pub fn rpc_provider(rpc_url: &str) -> RpcService {
    RpcService::Custom({
        RpcApi {
            url: rpc_url.to_string(),
            headers: None,
        }
    })
}

/// Returns `T` from Solidity struct.
pub fn decode_abi_response<T, F: SolCall<Return = T>>(
    hex_data: Hex,
) -> ManagerResult<T> {
    let data = hex_data.as_ref();
    F::abi_decode_returns(data, false)
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
    let args = json!({
      "id": 1,
      "jsonrpc": "2.0",
      "method": "eth_blockNumber"
    })
    .to_string();

    let rpc_canister_response = request_with_dynamic_retries(rpc_canister, args).await?;

    let decoded_response: EthCallResponse = serde_json::from_str(&rpc_canister_response)
        .map_err(|err| ManagerError::DecodingError(format!("{}", err)))?;
    
    let result = &decoded_response.result;
    let block_number = result.parse::<Nat>().map_err(|err| {
        ManagerError::DecodingError(format!(
            "Could not convert current block number to a Nat256: {:#?}", 
            err
        ))
    })?;

    let safe_block = block_number - Nat::from(32_u8);

    let safe_block_converted = Nat256::try_from(safe_block).map_err(|err| {
        ManagerError::DecodingError(format!(
            "Could not convert current block number to a Nat256: {:#?}", 
            err
        ))
    })?;

    Ok(BlockTag::Number(safe_block_converted))
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
    let rpc = get_provider_set().into();

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
        max_fee_per_gas: max_fee_per_gas.to::<u128>(),
        max_priority_fee_per_gas: max_priority_fee_per_gas.to::<u128>(),
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
    if let RpcError::HttpOutcallError(HttpOutcallError::IcError {
        code,
        message,
    }) = err
    {
        *code == ic_cdk::api::call::RejectionCode::SysFatal
            && (message.contains("size limit") || message.contains("length limit"))
    } else {
        false
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
) -> ManagerResult<Hex> {
    let mut max_response_bytes = DEFAULT_MAX_RESPONSE_BYTES.with(|value| value.get());
    let provider_set : RpcServices = get_provider_set().into();
    // There is a 2 MB limit on the response size, an ICP limitation.
    while max_response_bytes < 2_000_000 {
        // Perform the request using the provided function
        let mut transaction = TransactionRequest::default();
        transaction.to = Some(Hex20::from(to.into_array()));
        transaction.input = Some(Hex::from(data.clone()));

        let args = CallArgs {
            transaction,
            block: Some(block.clone()),
        };

        let config = RpcConfig {
            response_size_estimate: Some(max_response_bytes),
            response_consensus: None,
        };

        let response = rpc_canister
            .eth_call(
                provider_set.clone(),
                Some(config),
                args,
            )
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
    let rpc : RpcService = get_provider_set();
    while max_response_bytes < 2_000_000 {
        // Estimate the cycles based on the current max response bytes
        let cycles = estimate_cycles(
            rpc_canister,
            ,
            json_data.clone(),
            max_response_bytes,
        )
        .await?;

        // Perform the request using the provided function
        let response = rpc_canister
            .request(
                rpc_provider(rpc_url),
                json_data.clone(),
                max_response_bytes,
                cycles,
            )
            .await;

            let extracted_response = extract_call_result(response)?;
            let extracted_rpc_result = extract_multi_rpc_result(extracted_response);
    
            if let Err(ManagerError::RpcResponseError(err)) = extracted_rpc_result.clone() {
                if is_response_size_error(&err) {
                    max_response_bytes *= 2;
                    continue;
                }
            }
        return extracted_rpc_result;
    }

    Err(ManagerError::Custom(format!(
        "Request with dynamic retries reached its ceiling of 2 Megabytes."
    )))
}

/// On success, returns the nonce associated with the given address
pub async fn get_nonce(rpc_canister: &Service, address: Address) -> ManagerResult<U256> {
    let account = Hex20::from(address.into_array());
    let rpc: RpcServices = get_provider_set().into();
    let args = GetTransactionCountArgs {
        address: account,
        block: evm_rpc_types::BlockTag::Latest,
    };

    let result = rpc_canister
        .eth_get_transaction_count(rpc, None, args)
        .await;
    
    let wrapped_number = extract_call_result::<MultiRpcResult<Nat256>>(result)?;
    let number = extract_multi_rpc_result(wrapped_number)?;
    Ok(U256::from_be_bytes(number.into_be_bytes()))
}

/// Extracts result from `MultiRpcResult`, if the threshold is met.
pub fn extract_multi_rpc_result<T>(result: MultiRpcResult<T>) -> ManagerResult<T> {
    match result {
        MultiRpcResult::Consistent(response) => response.map_err(|rpc_err| ManagerError::RpcResponseError(rpc_err)),
        MultiRpcResult::Inconsistent(vec) => todo!(),
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
