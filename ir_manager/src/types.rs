use candid::CandidType;

use crate::evm_rpc::RpcError;

#[derive(CandidType)]
pub enum ManagerError {
    NonExistentValue,
    RpcResponseError(RpcError),
    Custom(String),
}
