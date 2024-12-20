//! Interest Rate Manager's Constants
// AUDIT: The following constants and parameters are subject to change and are not finalized.
// AUDIT: The thresholds, margins, chain id, etc

use alloy_primitives::U256;
use candid::{Nat, Principal};

/// Scale used for fixed point arithmetic
pub const SCALE: u128 = 1_000_000_000_000_000_000; // e18
pub fn scale() -> U256 {
    U256::from(SCALE)
}

/// Chain ID
pub const CHAIN_ID: u64 = 11155111; // Sepolia testnet

/// Tolerance margin up formula constant
const TOLERANCE_MARGIN_UP_RAW: u128 = 15 * SCALE / 100; // 15*10^16 => 15%
pub fn tolerance_margin_up() -> U256 {
    U256::from(TOLERANCE_MARGIN_UP_RAW)
}

/// Tolerance margin down formula constant
const TOLERANCE_MARGIN_DOWN_RAW: u128 = 15 * SCALE / 100; // 15*10^16 => 15%
pub fn tolerance_margin_down() -> U256 {
    U256::from(TOLERANCE_MARGIN_DOWN_RAW)
}

/// Max number of retry attempts
pub const MAX_RETRY_ATTEMPTS: u8 = 2;

/// Max number of troves to fetch in one call
pub const MAX_NUMBER_OF_TROVES: u128 = 50;
pub fn max_number_of_troves() -> U256 {
    U256::from(MAX_NUMBER_OF_TROVES)
}

/// Cycles balance threshold of the canister
pub const CYCLES_THRESHOLD: u64 = 50_000_000_000;

/// ckETH token transfer fee
const CKETH_FEE_RAW: u64 = 2_000_000_000_000;
pub fn cketh_fee() -> Nat {
    Nat::from(CKETH_FEE_RAW)
}

/// ckETH mint value
/// The amount of Ether that will be used to mint new ckETH tokens when the balance is below the threshold
const ETHER_RECHARGE_VALUE_RAW: u64 = 30_000_000_000_000_000; // 0.03 ETH in WEI
pub fn ether_recharge_value() -> U256 {
    U256::from(ETHER_RECHARGE_VALUE_RAW)
}

/// Cycles discount percentage
pub const CYCLES_DISCOUNT_PERCENTAGE: u64 = 97; // 3% discount is provided

/// ckETH balance threshold of the canister.
/// The recharging cycle will mint more ckETH if the balance falls below this number
const CKETH_THRESHOLD_RAW: u64 = 100_000_000_000_000; // 100 Trillion Cycles
pub fn cketh_threshold() -> Nat {
    Nat::from(CKETH_THRESHOLD_RAW)
}

/// Default max response bytes
pub const DEFAULT_MAX_RESPONSE_BYTES: u64 = 8_000;

/// Exchange rate canister's principal ID
const EXCHANGE_RATE_CANISTER_RAW: &[u8] = b"uf6dk-hyaaa-aaaaq-qaaaq-cai";
pub fn exchange_rate_canister() -> Principal {
    Principal::from_slice(EXCHANGE_RATE_CANISTER_RAW)
}

/// ckETH smart contract on Ethereum mainnet
pub const CKETH_HELPER: &str = "0x7574eB42cA208A4f6960ECCAfDF186D627dCC175";

/// ckETH ledger canister's principal ID
const CKETH_LEDGER_RAW: &[u8] = b"ss2fx-dyaaa-aaaar-qacoq-cai";
pub fn cketh_ledger() -> Principal {
    Principal::from_slice(CKETH_LEDGER_RAW)
}

/// Number of providers to use
pub const PROVIDER_COUNT: u8 = 3;

/// Number of providers needed to reach consensus
pub const PROVIDER_THRESHOLD: u8 = 2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cketh_ledger_is_correct() {
        assert_eq!(cketh_ledger().to_text(), "ss2fx-dyaaa-aaaar-qacoq-cai".to_string());
    }

    #[test]
    fn exchange_rate_canister_is_correct() {
        assert_eq!(exchange_rate_canister().to_text(), "uf6dk-hyaaa-aaaaq-qaaaq-cai".to_string());
    }

    #[test]
    fn scale_is_e18() {
        assert_eq!(SCALE, 10_u128.pow(18));
    }
}