//! # Interest Rate Manager's Constants
//!
//! This module defines various constants and helper functions used by the
//! Interest Rate Manager, including:
//! - Scaling factors for fixed-point arithmetic.
//! - Threshold values for `ckETH` management and cycle balances.
//! - Configuration values for retry attempts, providers, and response limits.
//! - Principal IDs for interacting with external canisters.
//! - Ethereum contract addresses.

use alloy_primitives::U256;
use candid::{Nat, Principal};

/// Scale used for fixed point arithmetic
pub const SCALE: u128 = 1_000_000_000_000_000_000; // e18

/// Returns the scale as a `U256` for fixed-point arithmetic.
pub fn scale() -> U256 {
    U256::from(SCALE)
}

/// Chain ID
#[cfg(feature = "sepolia")]
pub const CHAIN_ID: u64 = 11155111;
/// Chain ID
#[cfg(feature = "mainnet")]
pub const CHAIN_ID: u64 = 1;

/// Tolerance margin up formula constant
const TOLERANCE_MARGIN_UP_RAW: u128 = 15 * SCALE / 100; // 15*10^16 => 15%
                                                        // const TOLERANCE_MARGIN_UP_RAW: u128 = SCALE / 100; // 1%

/// Returns the tolerance margin for upward adjustments as a `U256`.
pub fn tolerance_margin_up() -> U256 {
    U256::from(TOLERANCE_MARGIN_UP_RAW)
}

/// Tolerance margin down formula constant
const TOLERANCE_MARGIN_DOWN_RAW: u128 = 15 * SCALE / 100; // 15*10^16 => 15%
                                                          // const TOLERANCE_MARGIN_DOWN_RAW: u128 = SCALE / 100; // 1%

/// Returns the tolerance margin for downward adjustments as a `U256`.
pub fn tolerance_margin_down() -> U256 {
    U256::from(TOLERANCE_MARGIN_DOWN_RAW)
}

/// Max number of retry attempts
pub const MAX_RETRY_ATTEMPTS: u8 = 2;

/// Max number of troves to fetch in one call
pub const MAX_NUMBER_OF_TROVES: u128 = 75;

/// Returns the maximum number of troves as a `U256`.
pub fn max_number_of_troves() -> U256 {
    U256::from(MAX_NUMBER_OF_TROVES)
}

/// Cycles balance threshold of the canister
pub const CYCLES_THRESHOLD: u64 = 30_000_000_000_000;

/// ckETH token transfer fee
const CKETH_FEE_RAW: u64 = 2_000_000_000_000;

/// Returns the ckETH transfer fee as a `Nat`.
pub fn cketh_fee() -> Nat {
    Nat::from(CKETH_FEE_RAW)
}

/// ckETH mint value
/// The amount of Ether that will be used to mint new ckETH tokens when the balance is below the threshold
const ETHER_RECHARGE_VALUE_RAW: u64 = 20_000_000_000_000_000; // 0.02 ETH in WEI

/// Returns the Ether recharge value as a `U256`.
pub fn ether_recharge_value() -> U256 {
    U256::from(ETHER_RECHARGE_VALUE_RAW)
}

/// Cycles discount percentage
pub const CYCLES_DISCOUNT_PERCENTAGE: u64 = 97; // 3% discount is provided

/// ckETH balance threshold of the canister.
/// The recharging cycle will mint more ckETH if the balance falls below this number
const CKETH_THRESHOLD_RAW: u64 = 30_000_000_000_000_000; // 0.03 ckETH

/// Returns the ckETH balance threshold as a `Nat`.
pub fn cketh_threshold() -> Nat {
    Nat::from(CKETH_THRESHOLD_RAW)
}

/// Default max response bytes
pub const DEFAULT_MAX_RESPONSE_BYTES: u64 = 8_000;

/// Exchange rate canister's principal ID as a constant string slice.
const EXCHANGE_RATE_CANISTER_RAW: &str = "uf6dk-hyaaa-aaaaq-qaaaq-cai";

/// Returns the Principal for the exchange rate canister.
///
/// # Panics
/// This function will panic if the hardcoded principal string is invalid.
/// The panic should be caught by the unit tests.
pub fn exchange_rate_canister() -> Principal {
    Principal::from_text(EXCHANGE_RATE_CANISTER_RAW)
        .expect("Invalid principal ID for the exchange rate canister.")
}

/// ckETH smart contract on Ethereum mainnet
#[cfg(feature = "mainnet")]
pub const CKETH_HELPER: &str = "0x18901044688D3756C35Ed2b36D93e6a5B8e00E68";

/// ckETH ledger canister's principal ID
#[cfg(feature = "mainnet")]
const CKETH_LEDGER_RAW: &str = "ss2fx-dyaaa-aaaar-qacoq-cai";

/// Returns the Principal for the ckETH ledger canister.
///
/// # Panics
/// This function will panic if the hardcoded principal string is invalid.
/// The panic should be caught by the unit tests.
pub fn cketh_ledger() -> Principal {
    Principal::from_text(CKETH_LEDGER_RAW)
        .expect("Invalid principal ID for the exchange rate canister.")
}

/// Number of providers to use
pub const PROVIDER_COUNT: u8 = 3;

/// Number of providers needed to reach consensus
pub const PROVIDER_THRESHOLD: u8 = 2;

/// Timeout in milliseconds for strategy locks
pub const STRATEGY_LOCK_TIMEOUT: u64 = 3_600_000; // one hour

/// Sepolia providers
#[cfg(feature = "sepolia")]
pub const PROVIDERS: [evm_rpc_types::EthSepoliaService; 5] = [
    evm_rpc_types::EthSepoliaService::BlockPi,
    evm_rpc_types::EthSepoliaService::PublicNode,
    evm_rpc_types::EthSepoliaService::Sepolia,
    evm_rpc_types::EthSepoliaService::Alchemy,
    evm_rpc_types::EthSepoliaService::Ankr,
];

/// Ethereum main-net providers
#[cfg(feature = "mainnet")]
pub const PROVIDERS: [evm_rpc_types::EthMainnetService; 4] = [
    evm_rpc_types::EthMainnetService::BlockPi,
    evm_rpc_types::EthMainnetService::PublicNode,
    evm_rpc_types::EthMainnetService::Alchemy,
    evm_rpc_types::EthMainnetService::Ankr,
];

/// Minimum expected cycles for the ckETH<>Cycles arbitrage opportunity
pub const MINIMUM_ATTACHED_CYCLES: u64 = 1_000_000_000_000; // 1 Trillion Cycles

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cketh_ledger_is_correct() {
        assert_eq!(
            cketh_ledger().to_text(),
            "ss2fx-dyaaa-aaaar-qacoq-cai".to_string()
        );
    }

    #[test]
    fn exchange_rate_canister_is_correct() {
        assert_eq!(
            exchange_rate_canister().to_text(),
            "uf6dk-hyaaa-aaaaq-qaaaq-cai".to_string()
        );
    }

    #[test]
    fn scale_is_e18() {
        assert_eq!(SCALE, 10_u128.pow(18));
    }
}
