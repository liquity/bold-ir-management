//! Common utility and helper functions that are used across the project

use std::str::FromStr;

use alloy_primitives::{Address, U256};
use alloy_sol_types::SolCall;
use candid::{Nat, Principal};
use evm_rpc_types::{
    HttpOutcallError, MultiRpcResult, RpcConfig, RpcError, RpcService, RpcServices,
};
use ic_exports::ic_cdk::{
    self,
    api::{call::CallResult, is_controller},
    call, id, print,
};

use super::{error::*, evm_rpc::*, exchange::*};

use crate::{
    constants::{
        cketh_ledger, exchange_rate_canister, DEFAULT_MAX_RESPONSE_BYTES, PROVIDER_COUNT,
        PROVIDER_THRESHOLD, SCALE,
    },
    providers::{extract_multi_rpc_result, get_ranked_rpc_provider, get_ranked_rpc_providers},
    state::RPC_SERVICE,
    types::Account,
};

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
        return Err(ManagerError::DecodingError("The `Nat` input length exceedes 32 bytes when converted to big-endian bytes representation.".to_string()));
    }
    // Ensure the byte array is exactly 32 bytes long
    let mut padded_bytes = [0u8; 32];
    let start_pos = 32 - be_bytes.len();
    padded_bytes[start_pos..].copy_from_slice(&be_bytes);

    Ok(U256::from_be_bytes(padded_bytes))
}

pub async fn fetch_cketh_balance() -> ManagerResult<Nat> {
    let ledger_principal = cketh_ledger();
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
    let exchange_rate_canister = exchange_rate_canister();
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
        Ok(response) => {
            let decimals = 10_u64
                .checked_pow(response.metadata.decimals)
                .ok_or(arithmetic_err(
                    "The ETH/CXDR decimals calculation overflowed.",
                ))?;
            let rate = response
                .rate
                .checked_div(decimals)
                .ok_or(arithmetic_err("ETH/CXDR decimals value was zero."))?;
            Ok(rate)
        }
        Err(err) => Err(ManagerError::Custom(format!(
            "Error from the exchange rate canister: {:#?}",
            err
        ))),
    }
}

/// Returns `T` from Solidity struct.
pub fn decode_abi_response<T, F: SolCall<Return = T>>(hex_data: String) -> ManagerResult<T> {
    let stripped_hex = if hex_data.starts_with("0x") {
        hex_data[2..].to_string()
    } else {
        hex_data
    };
    let hex_bytes =
        hex::decode(stripped_hex).map_err(|err| ManagerError::DecodingError(err.to_string()))?;
    F::abi_decode_returns(&hex_bytes, false)
        .map_err(|err| ManagerError::DecodingError(err.to_string()))
}

pub async fn get_block_tag(rpc_canister: &Service, latest: bool) -> ManagerResult<BlockTag> {
    let rpc = get_ranked_rpc_provider();
    let rpc_config = RpcConfig {
        response_size_estimate: Some(2000),
        response_consensus: Some(evm_rpc_types::ConsensusStrategy::Threshold {
            total: Some(1),
            min: 1,
        }),
    };

    let tag = if latest {
        BlockTag::Latest
    } else {
        BlockTag::Safe
    };

    let call_result = rpc_canister
        .get_block_by_number(rpc.clone(), Some(rpc_config), tag)
        .await;
    let rpc_result = extract_call_result(call_result)?;
    let result = extract_multi_rpc_result(rpc, rpc_result)?;

    Ok(BlockTag::Number(result.number))
}

fn is_response_size_error(err: &RpcError) -> bool {
    if let RpcError::HttpOutcallError(HttpOutcallError::IcError { code, message }) = err {
        *code == ic_cdk::api::call::RejectionCode::SysFatal
            && (message.contains("size limit") || message.contains("length limit"))
    } else {
        false
    }
}

pub fn get_rpc_config(max_response_bytes: Option<u64>) -> RpcConfig {
    RpcConfig {
        response_size_estimate: max_response_bytes,
        response_consensus: Some(evm_rpc_types::ConsensusStrategy::Threshold {
            total: Some(PROVIDER_COUNT),
            min: PROVIDER_THRESHOLD,
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
    let mut max_response_bytes = DEFAULT_MAX_RESPONSE_BYTES;
    let provider_set: RpcServices = get_ranked_rpc_providers();
    let data_string = format!("0x{}", hex::encode(data));
    // There is a 2 MB limit on the response size, an ICP limitation.
    while max_response_bytes < 2_000_000 {
        // Perform the request using the provided function
        let transaction = TransactionRequest {
            to: Some(to.to_string()),
            input: Some(data_string.to_string()),
            ..Default::default()
        };

        let args = CallArgs {
            transaction,
            block: Some(block.clone()),
        };

        print(format!("{:#?}", args));

        let config = get_rpc_config(Some(max_response_bytes));
        let response = rpc_canister
            .eth_call(provider_set.clone(), Some(config), args)
            .await;

        let extracted_response = extract_call_result(response)?;
        let extracted_rpc_result =
            extract_multi_rpc_result(provider_set.clone(), extracted_response);

        if let Err(ManagerError::RpcResponseError(err)) = extracted_rpc_result.clone() {
            if is_response_size_error(&err) {
                max_response_bytes *= 2;
                continue;
            }
        }

        // note: if the code has reached this line, it means that a response unrelated to the size was received.
        return extracted_rpc_result;
    }

    Err(ManagerError::Custom(
        "Request with dynamic retries reached its ceiling of 2 Megabytes.".to_string(),
    ))
}

pub fn get_rpc_service() -> RpcService {
    RPC_SERVICE.with(|rpc| {
        let mut state = rpc.borrow_mut();
        let rpc = match state.front() {
            Some(inner) => inner.clone(),
            None => unreachable!(),
        };
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
    let mut max_response_bytes = DEFAULT_MAX_RESPONSE_BYTES;
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

        let extracted_response =
            extract_call_result(call_result)?.map_err(ManagerError::RpcResponseError);

        if let Err(ManagerError::RpcResponseError(err)) = extracted_response.clone() {
            print(format!(
                "RPC error in request with dynamic retries: {:#?}",
                err
            ));
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

    if max_response_bytes >= 2_000_000 {
        return Err(ManagerError::Custom(
            "Request with dynamic retries reached its ceiling of 2 MB.".to_string(),
        ));
    } else if rpc_changes >= 3 {
        return Err(ManagerError::Custom(
            "Request with dynamic retries reached its ceiling of 3 provider rotations.".to_string(),
        ));
    }
    unreachable!()
}

/// On success, returns the nonce associated with the given address
pub async fn get_nonce(rpc_canister: &Service, address: Address) -> ManagerResult<U256> {
    let account = address.to_string();
    let rpc: RpcServices = get_ranked_rpc_providers();
    let args = GetTransactionCountArgs {
        address: account,
        block: BlockTag::Latest,
    };

    let config = RpcConfig {
        response_size_estimate: Some(10_000),
        response_consensus: Some(evm_rpc_types::ConsensusStrategy::Threshold {
            total: Some(PROVIDER_COUNT),
            min: PROVIDER_THRESHOLD,
        }),
    };

    let result = rpc_canister
        .eth_get_transaction_count(rpc.clone(), Some(config), args)
        .await;

    let wrapped_number = extract_call_result::<MultiRpcResult<Nat>>(result)?;
    let number = extract_multi_rpc_result(rpc, wrapped_number)?;
    nat_to_u256(&number)
}

/// Extracts the Ok or Err values of a canister call and returns them.
pub fn extract_call_result<T>(result: CallResult<(T,)>) -> ManagerResult<T> {
    result
        .map(|(success_value,)| success_value)
        .map_err(|(rejection_code, error_message)| {
            ManagerError::CallResult(rejection_code, error_message)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::{Address, U256};
    use evm_rpc_types::{EthMainnetService, HttpOutcallError, RpcError};
    use ic_cdk::api::call::RejectionCode;
    use std::str::FromStr;

    #[test]
    fn test_string_to_address_valid() {
        // Valid Ethereum address
        let input = "0x0123456789abcdef0123456789abcdef01234567".to_string();
        let result = string_to_address(input.clone());
        assert!(result.is_ok());
        let address = result.unwrap();
        assert_eq!(address, Address::from_str(&input).unwrap());
    }

    #[test]
    fn test_string_to_address_invalid() {
        // Invalid Ethereum address
        let input = "invalid_address".to_string();
        let result = string_to_address(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_nat_to_u256_valid() {
        // Nat that fits into U256
        let value = 1234567890_u64;
        let nat = Nat::from(value);
        let result = nat_to_u256(&nat);
        assert!(result.is_ok());
        let u256 = result.unwrap();
        assert_eq!(
            u256,
            U256::from_be_bytes({
                let mut bytes = [0u8; 32];
                let be_bytes = value.to_be_bytes();
                bytes[32 - be_bytes.len()..].copy_from_slice(&be_bytes);
                bytes
            })
        );
    }

    #[test]
    fn test_is_response_size_error_true() {
        // Create an RpcError that represents a response size error
        let err = RpcError::HttpOutcallError(HttpOutcallError::IcError {
            code: RejectionCode::SysFatal,
            message: "size limit exceeded".to_string(),
        });
        assert!(is_response_size_error(&err));
    }

    #[test]
    fn test_is_response_size_error_false() {
        // Create an RpcError that does not represent a response size error
        let err = RpcError::HttpOutcallError(HttpOutcallError::IcError {
            code: RejectionCode::CanisterReject,
            message: "some other error".to_string(),
        });
        assert!(!is_response_size_error(&err));
    }

    #[test]
    fn test_extract_call_result_ok() {
        let call_result: CallResult<(String,)> = Ok(("success".to_string(),));
        let extracted = extract_call_result(call_result);
        assert!(extracted.is_ok());
        assert_eq!(extracted.unwrap(), "success".to_string());
    }

    #[test]
    fn test_extract_call_result_err() {
        let call_result: CallResult<(String,)> =
            Err((RejectionCode::CanisterReject, "error message".to_string()));
        let extracted = extract_call_result(call_result);
        assert!(extracted.is_err());
        match extracted.unwrap_err() {
            ManagerError::CallResult(code, message) => {
                assert_eq!(code, RejectionCode::CanisterReject);
                assert_eq!(message, "error message".to_string());
            }
            _ => panic!("Expected CallResult error"),
        }
    }

    #[test]
    fn test_get_rpc_config() {
        let max_response_bytes = Some(5000);
        let config = get_rpc_config(max_response_bytes);
        assert_eq!(config.response_size_estimate, Some(5000));
        assert!(config.response_consensus.is_some());
        if let Some(evm_rpc_types::ConsensusStrategy::Threshold { total, min }) =
            config.response_consensus
        {
            assert_eq!(total, Some(3));
            assert_eq!(min, 2);
        } else {
            panic!("Expected Threshold consensus strategy");
        }
    }

    #[test]
    fn test_get_rpc_service() {
        // Since `get_rpc_service` uses a thread-local variable, we need to set it up
        // For testing purposes, we can assume that it returns an RpcService
        // We cannot directly test the rotation without setting up the thread-local state
        // So we'll just call it and check that it returns a value
        let rpc_service = get_rpc_service();
        // Since we don't know the exact value, we can check that it's of type RpcService
        // For example:
        assert!(matches!(
            rpc_service,
            RpcService::EthMainnet(EthMainnetService::Alchemy)
        ));
    }
}
