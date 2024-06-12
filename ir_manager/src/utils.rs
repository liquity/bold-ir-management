use alloy::sol_types::SolCall;
use candid::Principal;
use ic_exports::ic_cdk::{self, api::call::CallResult};

use crate::{
    evm_rpc::{RequestResult, RpcApi, RpcService, Service},
    types::ManagerError,
};

pub fn rpc_canister(id: Option<Principal>) -> Result<Service, ManagerError> {
    if let Some(rpc_id) = id {
        return Ok(Service(rpc_id));
    }
    Err(ManagerError::NonExistentValue)
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
        Err(e) => Err(ManagerError::Custom(e.1)), // Assuming e is an error type that can be converted to a String
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
