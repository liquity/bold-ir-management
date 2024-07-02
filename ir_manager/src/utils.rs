use std::str::FromStr;

use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_sol_types::SolCall;
use candid::Principal;
use ic_exports::ic_cdk::{
    self,
    api::{
        call::CallResult,
        management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
    },
};
use serde_json::json;

use crate::{
    evm_rpc::{
        MultiSendRawTransactionResult, RequestResult, RpcApi, RpcService, RpcServices, Service,
    },
    signer::{
        get_canister_public_key, pubkey_bytes_to_address, sign_eip1559_transaction, SignRequest,
    },
    state::STRATEGY_DATA,
    types::{DerivationPath, ManagerError, StrategyData},
};

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

    for (_id, mut strategy) in strategies {
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

    let key_id = EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: String::from("key_1"),
    };

    let request = SignRequest {
        chain_id: 1,
        from: None,
        to: TxKind::Call(Address::from_str(&to).unwrap()),
        max_fee_per_gas: todo!(),
        max_priority_fee_per_gas: todo!(),
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
