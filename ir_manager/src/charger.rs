//! Responsible for managing the ckETH<>Cycles arbitrage process.
//! This module facilitates recharging ckETH, ensuring the canister's cycle balance is maintained,
//! and handling ETH deposits for minting ckETH tokens on ICP.
//!
//! Key functionalities include:
//! - Monitoring and recharging canister cycle balance when it falls below a defined threshold.
//! - Monitoring ckETH balance and minting ckETH when it is below a specified threshold.
//! - Sending ETH to the ckETH helper contract to mint ckETH tokens.
//! - Facilitating transfers of ckETH to arbitrageurs at a discounted rate.
//! - Providing locking mechanisms to ensure atomicity for ckETH<>Cycles arbitrage operations.
//!
//! Dependencies:
//! - EVM RPC for querying ETH balances and submitting transactions.
//! - ICRC-1 ledger for transferring ckETH tokens.
//! - Stable strategies for managing multiple EOAs (Externally Owned Accounts).

use crate::{
    constants::{
        cketh_fee, cketh_ledger, cketh_threshold, ether_recharge_value, scale, CKETH_HELPER,
        CYCLES_DISCOUNT_PERCENTAGE, CYCLES_THRESHOLD, SCALE,
    },
    journal::{JournalCollection, LogType},
    strategy::stable::StableStrategy,
    types::{depositEthCall, EthCallResponse},
    utils::{
        common::{
            fetch_cketh_balance, fetch_ether_cycles_rate, request_with_dynamic_retries, u256_to_nat,
        },
        error::*,
        evm_rpc::{SendRawTransactionStatus, Service},
        transaction_builder::TransactionBuilder,
    },
};
use crate::{state::*, types::SwapResponse};
use alloy_primitives::{FixedBytes, U256};
use alloy_sol_types::SolCall;
use candid::Principal;
use ic_exports::ic_cdk::{
    api::{
        self,
        call::{msg_cycles_accept, msg_cycles_available},
        canister_balance,
    },
    call, print,
};
use ic_exports::{candid::Nat, ic_kit::CallResult};
use icrc_ledger_types::icrc1::transfer::{TransferArg, TransferError};
use num_traits::ToPrimitive;
use serde_json::json;

/// Monitors the canister's cycle balance and ensures it does not exceed the recharge threshold.
///
/// Returns:
/// - `Ok(())` if the cycle balance is below the threshold.
/// - `Err(ManagerError::CyclesBalanceAboveRechargingThreshold)` if the cycle balance exceeds the threshold.
pub async fn check_threshold() -> ManagerResult<()> {
    let threshold = CYCLES_THRESHOLD;
    if canister_balance() <= threshold {
        return Ok(());
    }
    Err(ManagerError::CyclesBalanceAboveRechargingThreshold)
}

/// Monitors the canister's ckETH balance and triggers minting (recharging) if below the threshold.
///
/// Returns:
/// - `Ok(())` if the ckETH balance is sufficient.
/// - Triggers `ether_deposit` if the ckETH balance is below the threshold.
pub async fn recharge_cketh(journal: &mut JournalCollection) -> ManagerResult<()> {
    let current_balance = fetch_cketh_balance().await?;
    journal.append_note(
        Ok(()),
        LogType::Recharge,
        format!("The current ckETH balance is at {}", current_balance),
    );
    let cketh_threshold = cketh_threshold();

    if current_balance < cketh_threshold {
        return ether_deposit(journal).await;
    }

    journal.append_note(
        Ok(()),
        LogType::Recharge,
        format!(
            "The current ckETH balance is larger than the threshold {}",
            cketh_threshold
        ),
    );
    Ok(())
}

/// Deposits ETH into the ckETH helper contract to mint ckETH tokens on the Internet Computer.
///
/// This function rotates through available EOAs (Externally Owned Accounts) to select one
/// with sufficient balance for the deposit operation.
///
/// Returns:
/// - `Ok(())` if the deposit succeeds.
/// - `Err(ManagerError::Custom)` if no EOA has enough balance or an error occurs.
async fn ether_deposit(journal: &mut JournalCollection) -> ManagerResult<()> {
    let ether_value = ether_recharge_value();
    let cketh_helper: String = CKETH_HELPER.to_string();
    let mut strategies: Vec<StableStrategy> = STRATEGY_STATE
        .with(|strategies_hashmap| strategies_hashmap.borrow().clone().into_values().collect());

    let turn = CKETH_EOA_TURN_COUNTER.with(|counter| counter.get());
    strategies.rotate_left(turn as usize);

    for (index, strategy) in strategies.iter().enumerate() {
        let eoa = match strategy.settings.eoa_pk {
            Some(pk) => pk,
            None => continue, // Skip if eoa_pk is None
        };

        let balance = match fetch_balance(&strategy.settings.rpc_canister, eoa.to_string()).await {
            Ok(balance) => {
                journal.append_note(
                    Ok(()),
                    LogType::Recharge,
                    format!(
                        "Queried the ETH balance of address {}. The current balance is {}",
                        eoa, balance
                    ),
                );
                balance
            }
            Err(err) => {
                journal.append_note(
                    Ok(()),
                    LogType::Recharge,
                    format!(
                        "Tried to queried the ETH balance of address {}. Got error: {:#?}",
                        eoa, err
                    ),
                );
                continue;
            } // Skip on error
        };

        if balance > ether_value {
            journal.append_note(
                Ok(()),
                LogType::Recharge,
                "The balance is larger than the required ETH value. Proceeding with minting ckETH.",
            );

            let principal = api::id();
            let principal_bytes = principal.as_slice();
            let n = principal_bytes.len();

            let mut bytes = [0u8; 32];
            bytes[0] = n as u8;
            bytes[1..=n].copy_from_slice(principal_bytes);

            let encoded_canister_id = FixedBytes::<32>::from(bytes);

            let deposit_call = depositEthCall {
                principal: encoded_canister_id,
                subaccount: FixedBytes::<32>::ZERO,
            };

            let transaction_data = deposit_call.abi_encode();

            let new_counter = (index as u8 + turn + 1) % strategies.len() as u8;
            CKETH_EOA_TURN_COUNTER.with(|counter| counter.set(new_counter));

            let transaction_response = TransactionBuilder::default()
                .to(cketh_helper.clone())
                .from(eoa.to_string())
                .data(transaction_data)
                .value(ether_value)
                .nonce(strategy.data.eoa_nonce)
                .derivation_path(strategy.settings.derivation_path.clone())
                .cycles(40_000_000_000)
                .send(&strategy.settings.rpc_canister)
                .await?;

            match transaction_response {
                SendRawTransactionStatus::Ok(tx_hash) => {
                    journal.append_note(
                        Ok(()),
                        LogType::Recharge,
                        format!(
                            "The mint transaction was sent successful with hash: {:#?}",
                            tx_hash
                        ),
                    );
                    return Ok(());
                }
                SendRawTransactionStatus::InsufficientFunds => {
                    journal.append_note(
                        Ok(()),
                        LogType::Recharge,
                        "Not enough funds to cover the mint value and the gas costs.",
                    );
                    continue;
                }
                SendRawTransactionStatus::NonceTooHigh | SendRawTransactionStatus::NonceTooLow => {
                    journal.append_note(
                        Ok(()),
                        LogType::Recharge,
                        format!("The nonce needs adjusting: {:#?}", transaction_response),
                    );
                    continue;
                }
            }
        }
    }

    Err(ManagerError::Custom(
        "No EOA had enough balance and proper nonce.".to_string(),
    ))
}

/// Queries the ETH balance for a given public key using the EVM RPC canister.
///
/// Arguments:
/// - `rpc_canister`: Reference to the RPC service canister.
/// - `public_key`: The public key to check the ETH balance for.
///
/// Returns:
/// - `Ok(U256)` representing the balance.
/// - `Err(ManagerError)` if the RPC call or balance parsing fails.
async fn fetch_balance(rpc_canister: &Service, public_key: String) -> ManagerResult<U256> {
    let json_args = json!({
        "id": 1,
        "jsonrpc": "2.0",
        "params": [
            public_key,
            "latest"
        ],
        "method": "eth_getBalance"
    })
    .to_string();

    let rpc_canister_response = request_with_dynamic_retries(rpc_canister, json_args).await?;

    let decoded_response: EthCallResponse =
        serde_json::from_str(&rpc_canister_response).map_err(|err| {
            ManagerError::DecodingError(format!(
                "Could not decode eth_estimateGas response: {} error: {}",
                &rpc_canister_response, err
            ))
        })?;

    if decoded_response.result.len() <= 2 {
        return Err(ManagerError::DecodingError(
            "The result field of the RPC's response is empty".to_string(),
        ));
    }

    let hex_string = if decoded_response.result[2..].len() % 2 == 1 {
        format!("0{}", &decoded_response.result[2..])
    } else {
        decoded_response.result[2..].to_string()
    };

    let hex_decoded_response = hex::decode(hex_string)
        .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;

    Ok(U256::from_be_slice(&hex_decoded_response))
}

/// Calculates the maximum amount of ckETH that can be transferred
/// to the specified `receiver`, considering available cycles and conversion rates.
///
/// This function performs the following steps:
/// 1. **Rate Calculation**: Fetches the current Ether-to-Cycles conversion rate and applies a
///    predefined discount percentage (`CYCLES_DISCOUNT_PERCENTAGE`).
/// 2. **Cycle Validation**: Verifies that the conversion rate is non-zero.
/// 3. **Maximum ckETH Transfer Calculation**:
///    - Calculates the maximum amount of ckETH that can be transferred based on available cycles.
///    - If the account balance is less than the maximum, it adjusts the cycles accepted.
/// 4. **Cycles Acceptance**: Accepts the necessary cycles for the transfer.
/// 5. **Transfer Execution**:
///    - Constructs a transfer argument (`TransferArg`) for the ckETH ledger.
///    - Sends the transfer request using the ICRC1 transfer method.
///
/// # Arguments
/// * `receiver` - The principal identifier of the arbitrageur (the recipient).
///
/// # Returns
/// A `SwapResponse` struct containing:
/// - `accepted_cycles`: The number of accepted cycles.
/// - `returning_ether`: The amount of ckETH transferred.
///
/// # Errors
/// Returns a `ManagerError` in cases where:
/// - The calculated conversion rate is zero.
/// - Decoding issues occur during cycle-to-amount conversion.
/// - Transfer fails due to ledger errors.
///
/// # Example
/// ```rust
/// let receiver = Principal::from_text("aaaaa-aa").unwrap();
/// let response = transfer_cketh(receiver).await?;
/// println!("Transferred: {} ckETH, Accepted Cycles: {}", response.returning_ether, response.accepted_cycles);
/// ```
pub async fn transfer_cketh(receiver: Principal) -> ManagerResult<SwapResponse> {
    let discount_percentage = CYCLES_DISCOUNT_PERCENTAGE;
    let rate = fetch_ether_cycles_rate().await? * discount_percentage / 100;

    if rate == 0 {
        return Err(arithmetic_err("The calculated ETH/CXDR rate is zero."));
    }

    let trillion = U256::from(1_000_000_000_000_u64);
    let attached_cycles = U256::from(msg_cycles_available());
    let scaled_rate = U256::from(rate)
        .checked_mul(trillion)
        .ok_or(arithmetic_err(
            "Overflow occurred when calculating the scaled rate.",
        ))?;

    let max_returned_ether_amount_u256 = &attached_cycles
        .checked_mul(scale()) // SCALE here is the decimals ckETH tokens have (10^18)
        .and_then(|r| r.checked_div(scaled_rate))
        .ok_or(arithmetic_err(
            "Overflow occurred when calculating the maximum possible Ether to return.",
        ))?;

    let maximum_returned_ether_amount = u256_to_nat(max_returned_ether_amount_u256)?;

    // Check the current balance of ckETH.
    let cketh_balance = fetch_cketh_balance().await?;

    // Determine the amount to transfer and cycles to accept.
    let (transfer_amount, cycles_to_accept) = if cketh_balance > maximum_returned_ether_amount {
        // we are not worried about casting like this as `msg_cycles_available()` had returned a u64 before
        (maximum_returned_ether_amount, attached_cycles.to::<u64>())
    } else {
        let cycles_to_accept = (cketh_balance.clone() / SCALE / rate)
            .0
            .to_u64()
            .ok_or_else(|| {
                ManagerError::DecodingError(
                    "Error while decoding the amount of cycles to accept to u64".to_string(),
                )
            })?;
        (cketh_balance, cycles_to_accept)
    };

    msg_cycles_accept(cycles_to_accept);

    // Send ckETH to the receiver via the ledger.
    let ledger_principal = cketh_ledger();
    let args = TransferArg {
        from_subaccount: None,
        to: receiver.into(),
        fee: Some(cketh_fee()),
        created_at_time: None,
        memo: None,
        amount: transfer_amount.clone(),
    };

    let call_response: CallResult<(Result<Nat, TransferError>,)> =
        call(ledger_principal, "icrc1_transfer", (args,)).await;

    match call_response {
        Ok(_) => Ok(SwapResponse {
            accepted_cycles: Nat::from(cycles_to_accept),
            returning_ether: transfer_amount,
        }),
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}

/// A structure to manage locking and unlocking of the ckETH<>Cycles arbitrage opportunity.
///
/// `SwapLock` ensures that only one arbitrage operation is executed at a time.
/// It prevents concurrent access to the swap functionality by providing a
/// locking mechanism.
///
/// # Methods
/// - `lock`: Acquires the lock, preventing further arbitrage operations until it is released.
/// - `unlock`: Releases the lock, allowing new arbitrage operations.
/// - `apply`: Updates the shared `SWAP_LOCK` state.
///
/// The lock is automatically released when the `SwapLock` instance is dropped, ensuring safety.
///
/// # Example
/// ```rust
/// let mut lock = SwapLock::default();
/// lock.lock()?; // Acquire the lock
/// // Perform swap operations here...
/// drop(lock); // Automatically releases the lock
/// ```
#[derive(Default)]
pub struct SwapLock(bool);

impl SwapLock {
    /// Applies the current lock state to the shared `SWAP_LOCK`.
    fn apply(&mut self) {
        SWAP_LOCK.with(|lock| lock.set(self.0));
    }

    /// Acquires the lock for the ckETH<>Cycles arbitrage opportunity.
    ///
    /// # Errors
    /// Returns `ManagerError::Locked` if the lock is already held.
    pub fn lock(&mut self) -> ManagerResult<()> {
        if self.0 || SWAP_LOCK.with(|lock| lock.get()) {
            return Err(ManagerError::Locked);
        }
        self.0 = true;
        self.apply();
        Ok(())
    }

    /// Releases the lock for the ckETH<>Cycles arbitrage opportunity.
    ///
    /// This method is called automatically when the `SwapLock` instance is dropped.
    pub fn unlock(&mut self) {
        self.0 = false;
        self.apply();
    }
}

impl Drop for SwapLock {
    /// Ensures the lock is released when the `SwapLock` instance goes out of scope.
    fn drop(&mut self) {
        self.unlock();
    }
}
