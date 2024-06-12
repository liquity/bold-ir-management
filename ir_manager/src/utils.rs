use candid::Principal;

use crate::{
    evm_rpc::{RpcApi, RpcService, Service},
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
