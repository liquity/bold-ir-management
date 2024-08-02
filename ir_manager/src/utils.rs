use std::str::FromStr;

use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_sol_types::SolCall;
use candid::Nat;
use ic_exports::ic_cdk::{
    self,
    api::{
        call::CallResult,
        management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
    },
    call, id, print,
};
use serde_json::json;

use crate::{
    api::execute_strategy,
    evm_rpc::{
        MultiSendRawTransactionResult, RequestResult, RpcApi, RpcService, RpcServices, Service,
    },
    gas::{estimate_transaction_fees, FeeEstimates},
    signer::{
        get_canister_public_key, pubkey_bytes_to_address, sign_eip1559_transaction, SignRequest,
    },
    state::{CKETH_LEDGER, EXCHANGE_RATE_CANISTER, STRATEGY_DATA},
    types::{
        Account, Asset, AssetClass, DerivationPath, GetExchangeRateRequest, GetExchangeRateResult,
        ManagerError, StrategyData,
    },
};

pub async fn fetch_cketh_balance() -> Result<Nat, ManagerError> {
    let ledger_principal = CKETH_LEDGER.with(|ledger| ledger.get());
    let args = Account {
        owner: id(),
        subaccount: None,
    };

    let call_response: CallResult<(Nat,)> =
        call(ledger_principal, "icrc1_balance_of", (args,)).await;

    match call_response {
        Ok(response) => Ok(response.0),
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}

pub async fn fetch_ether_cycles_rate() -> Result<u64, ManagerError> {
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
            (Ok(response),) => Ok(response.rate),
            (Err(err),) => Err(ManagerError::Custom(format!(
                "Error from the exchange rate canister: {:#?}",
                err
            ))),
        },
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}

/// Logs the error, and sets off a zero second timer to re-run
pub async fn retry(
    key: u32,
    strategy: &StrategyData,
    error: ManagerError,
) -> Result<(), ManagerError> {
    // Attempt to retrieve and modify the strategy data, handling errors gracefully
    let result = STRATEGY_DATA.with(|strategies| {
        strategies
            .borrow_mut()
            .get_mut(&key)
            .map(|s| s.lock = false)
    });

    // Check if the above operation was successful
    if result.is_none() {
        // Log the error and return a ManagerError if the operation failed
        println!("[ERROR] Key not found for strategy data update: {}", key);
        return Err(ManagerError::NonExistentValue);
    }

    print(format!(
        "[ERROR] Dropping and Retrying error => {:#?}",
        error
    ));

    execute_strategy(key, strategy).await
}

pub fn unlock(key: u32) -> Result<(), ManagerError> {
    STRATEGY_DATA.with(|strategies| {
        match strategies.borrow_mut().get_mut(&key) {
            Some(strategy) => {
                // we shouldn't care if it's unlocked already or not
                strategy.lock = false;
                Ok(())
            }
            None => Err(ManagerError::NonExistentValue),
        }
    })
}

pub fn rpc_provider(rpc_url: &str) -> RpcService {
    RpcService::Custom({
        RpcApi {
            url: rpc_url.to_string(),
            headers: None,
        }
    })
}

pub fn decode_response<T, F: SolCall<Return = T>>(
    canister_response: CallResult<(RequestResult,)>,
) -> Result<T, ManagerError> {
    match canister_response {
        Ok((rpc_response,)) => handle_rpc_response::<T, F>(rpc_response),
        Err(e) => Err(ManagerError::Custom(e.1)),
    }
}

pub fn handle_rpc_response<T, F: SolCall<Return = T>>(
    rpc_response: RequestResult,
) -> Result<T, ManagerError> {
    match rpc_response {
        RequestResult::Ok(hex_data) => {
            let decoded_hex = hex::decode(hex_data)
                .map_err(|err| ManagerError::DecodingError(err.to_string()))?;
            F::abi_decode_returns(&decoded_hex, false)
                .map_err(|err| ManagerError::DecodingError(err.to_string()))
        }
        RequestResult::Err(e) => Err(ManagerError::RpcResponseError(e)),
    }
}

pub fn decode_request_response(
    canister_response: CallResult<(RequestResult,)>,
) -> Result<Vec<u8>, ManagerError> {
    match canister_response {
        Ok((rpc_response,)) => match rpc_response {
            RequestResult::Ok(hex_data) => {
                let decoded_hex = hex::decode(hex_data)
                    .map_err(|err| ManagerError::DecodingError(err.to_string()))?;
                Ok(decoded_hex)
            }
            RequestResult::Err(e) => Err(ManagerError::RpcResponseError(e)),
        },
        Err(e) => Err(ManagerError::Custom(e.1)),
    }
}

pub fn eth_call_args(to: String, data: Vec<u8>) -> String {
    json!({
        "id": 1,
        "jsonrpc": "2.0",
        "params": [ {
            "to": to,
            "data": format!("0x{}", hex::encode(data))
        }
        ],
        "method": "eth_call"
    })
    .to_string()
}

pub async fn set_public_keys() {
    let strategies =
        STRATEGY_DATA.with(|strategies_hashmap| strategies_hashmap.borrow_mut().clone());

    for (_id, strategy) in strategies {
        let derivation_path = strategy.derivation_path.clone();
        let key_id = EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: String::from("key_1"),
        };

        // Calculate the public key asynchronously
        let public_key_bytes = get_canister_public_key(key_id, None, Some(derivation_path)).await;
        let eoa_pk = pubkey_bytes_to_address(&public_key_bytes);

        // Update the strategy with the public key
        STRATEGY_DATA.with(|strategies_hashmap| {
            strategies_hashmap
                .borrow_mut()
                .get_mut(&_id)
                .unwrap()
                .eoa_pk = Some(eoa_pk);
        });
    }
}

pub async fn send_raw_transaction(
    to: String,
    data: Vec<u8>,
    value: U256,
    nonce: u64,
    derivation_path: DerivationPath,
    rpc_canister: &Service,
    rpc_url: &str,
    cycles: u128,
) -> Result<MultiSendRawTransactionResult, ManagerError> {
    let input = Bytes::from(data);
    let rpc = RpcServices::Custom {
        chainId: 1,
        services: vec![RpcApi {
            url: rpc_url.to_string(),
            headers: None,
        }],
    };

    let FeeEstimates {
        max_fee_per_gas,
        max_priority_fee_per_gas,
    } = estimate_transaction_fees(9, rpc.clone(), rpc_canister).await?;

    let key_id = EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: String::from("key_1"),
    };

    let request = SignRequest {
        chain_id: 1,
        from: None,
        to: TxKind::Call(
            Address::from_str(&to)
                .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?,
        ),
        max_fee_per_gas: max_fee_per_gas.to::<u128>(),
        max_priority_fee_per_gas: max_priority_fee_per_gas.to::<u128>(),
        value,
        nonce,
        data: input,
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
