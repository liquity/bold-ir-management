//! Utility and helper functions needed for:
//! - Transaction signing, gas estimation, and submission
//! - Interacting with the EVM RPC and the exchange rate canisters
//! - Error handling
//! - Type casting

pub(crate) mod common;
pub(crate) mod error;
pub(crate) mod evm_rpc;
pub(crate) mod exchange;
pub(crate) mod gas;
pub(crate) mod signer;
pub(crate) mod transaction_builder;
