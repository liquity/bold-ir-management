use candid::Principal;

use crate::{evm_rpc::Service, types::ManagerError};

pub fn evm_rpc(id: Option<Principal>) -> Result<Service, ManagerError> {
    if let Some(rpc_id) = id {
        return Ok(Service(rpc_id))
    }
    Err(ManagerError::NonExistentValue)
}