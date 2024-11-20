use candid::CandidType;
use evm_rpc_types::RpcError;
use ic_exports::ic_kit::RejectionCode;
use serde::Deserialize;

/// IR Manager Canister Result
pub type ManagerResult<T> = Result<T, ManagerError>;

/// IR Manager Canister Errors
#[derive(Clone, CandidType, Debug, Deserialize, PartialEq)]
pub enum ManagerError {
    /// `CallResult` error
    CallResult(RejectionCode, String),
    /// Unauthorized access
    Unauthorized,
    /// A requested value does not exist
    NonExistentValue,
    /// Wrapper for the RPC errors returned by the EVM RPC canister
    RpcResponseError(RpcError),
    /// Decoding issue
    DecodingError(String),
    /// Strategy is locked
    Locked,
    /// Unknown/Custom error
    Custom(String),
    /// The cycle balance is above the threshold.
    /// No arbitrage opportunity is available.
    CyclesBalanceAboveRechargingThreshold,
    /// No consensus was reached among RPC providers
    NoConsensus,
    /// Arithmetic error
    Arithmetic(String),
}

pub fn arithmetic_err<S: AsRef<str>>(s: S) -> ManagerError {
    ManagerError::Arithmetic(format!("{:#?}", s.as_ref()))
}
