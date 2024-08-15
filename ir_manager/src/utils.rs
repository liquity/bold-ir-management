use std::{collections::HashMap, str::FromStr};

use alloy_primitives::{Address, Bytes, TxKind, U256};
use alloy_sol_types::SolCall;
use candid::{Nat, Principal};
use ic_exports::ic_cdk::{
    self,
    api::{
        call::CallResult,
        is_controller,
        management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
    },
    call, id,
};
use serde_json::json;

use crate::{
    evm_rpc::{
        EthCallResponse, MultiSendRawTransactionResult, RequestResult, RpcApi, RpcService,
        RpcServices, Service,
    },
    exchange::*,
    gas::{estimate_transaction_fees, FeeEstimates},
    signer::{sign_eip1559_transaction, SignRequest},
    state::{CKETH_LEDGER, EXCHANGE_RATE_CANISTER, STRATEGY_DATA},
    strategy::StrategyData,
    types::{Account, DerivationPath, ManagerError, Market, StrategyInput},
};

pub fn only_controller(caller: Principal) -> Result<(), ManagerError> {
    if !is_controller(&caller) {
        // only the controller should be able to call this function
        return Err(ManagerError::Unauthorized);
    }
    Ok(())
}

/// Generates strategies for each market. Returns a HashMap<u32, StrategyData>.
pub fn generate_strategies(
    markets: Vec<Market>,
    collateral_registry: Address,
    hint_helper: Address,
    strategies: Vec<StrategyInput>,
    rpc_principal: Principal,
    rpc_url: String,
    upfront_fee_period: Nat,
) -> HashMap<u32, StrategyData> {
    let mut strategies_data: HashMap<u32, StrategyData> = HashMap::new();
    let mut strategy_id = 0;
    let mut state_strategies = STRATEGY_DATA
        .with(|strategies| strategies.borrow().clone())
        .into_iter();
    markets.into_iter().for_each(|market| {
        strategies.iter().enumerate().for_each(|(index, strategy)| {
            let state_strategy_data = state_strategies.next().unwrap().1;
            let strategy_data = StrategyData::new(
                strategy_id,
                market.manager.clone(),
                collateral_registry.clone(),
                market.multi_trove_getter.clone(),
                nat_to_u256(&strategy.target_min),
                Service(rpc_principal.clone()),
                rpc_url.clone(),
                nat_to_u256(&upfront_fee_period),
                nat_to_u256(&market.collateral_index),
                hint_helper.clone(),
                market.batch_managers[index],
                state_strategy_data.eoa_pk.unwrap(),
                state_strategy_data.derivation_path,
            );
            strategies_data.insert(strategy_id, strategy_data);
            strategy_id += 1;
        });
    });

    strategies_data
}

/// Converts values of type `Nat` to `U256`
pub fn nat_to_u256(n: &Nat) -> U256 {
    let be_bytes = n.0.to_bytes_be();
    // Ensure the byte array is exactly 32 bytes long
    let mut padded_bytes = [0u8; 32];
    let start_pos = 32 - be_bytes.len();
    padded_bytes[start_pos..].copy_from_slice(&be_bytes);

    U256::from_be_bytes(padded_bytes)
}

pub async fn fetch_cketh_balance() -> Result<Nat, ManagerError> {
    let ledger_principal = CKETH_LEDGER.with(|ledger| ledger.get());
    let args = Account {
        owner: id(),
        subaccount: None,
    };

    let call_response: CallResult<(Nat,)> =
        call(ledger_principal, "icrc1_balance_of", (args,)).await;

    match call_response {
        Ok(response) => Ok(response.0),
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}

pub async fn fetch_ether_cycles_rate() -> Result<u64, ManagerError> {
    let exchange_rate_canister = EXCHANGE_RATE_CANISTER.with(|principal_id| principal_id.get());
    let fetch_args = GetExchangeRateRequest {
        base_asset: Asset {
            symbol: "ETH".to_string(),
            class: AssetClass::Cryptocurrency,
        },
        quote_asset: Asset {
            symbol: "CXDR".to_string(),
            class: AssetClass::FiatCurrency,
        },
        timestamp: None,
    };

    let fetch_result: CallResult<(GetExchangeRateResult,)> =
        ic_cdk::api::call::call_with_payment128(
            exchange_rate_canister,
            "get_exchange_rate",
            (fetch_args,),
            1_000_000_000,
        )
        .await;
    match fetch_result {
        Ok(result) => match result {
            (Ok(response),) => Ok(response.rate),
            (Err(err),) => Err(ManagerError::Custom(format!(
                "Error from the exchange rate canister: {:#?}",
                err
            ))),
        },
        Err(err) => Err(ManagerError::Custom(err.1)),
    }
}

pub fn unlock(key: u32) -> Result<(), ManagerError> {
    STRATEGY_DATA.with(|strategies| {
        match strategies.borrow_mut().get_mut(&key) {
            Some(strategy) => {
                // we shouldn't care if it's unlocked already or not
                strategy.lock = false;
                Ok(())
            }
            None => Err(ManagerError::NonExistentValue),
        }
    })
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

pub fn decode_request_response_encoded(
    canister_response: CallResult<(RequestResult,)>,
) -> Result<String, ManagerError> {
    match canister_response {
        Ok((rpc_response,)) => match rpc_response {
            RequestResult::Ok(hex_data) => Ok(hex_data),
            RequestResult::Err(e) => Err(ManagerError::RpcResponseError(e)),
        },
        Err(e) => Err(ManagerError::Custom(e.1)),
    }
}

pub fn eth_call_args(to: String, data: Vec<u8>, hex_block_number: &str) -> String {
    json!({
        "id": 1,
        "jsonrpc": "2.0",
        "params": [ {
            "to": to,
            "data": format!("0x{}", hex::encode(data))
        },
        hex_block_number
        ],
        "method": "eth_call"
    })
    .to_string()
}

pub async fn get_block_number(
    rpc_canister: &Service,
    rpc_url: &str,
) -> Result<String, ManagerError> {
    let rpc: RpcService = rpc_provider(rpc_url);

    let args = json!({
      "id": 1,
      "jsonrpc": "2.0",
      "method": "eth_blockNumber"
    })
    .to_string();

    let rpc_canister_response = rpc_canister
        .request(rpc, args, 500000, 10_000_000_000)
        .await;

    let encoded_response = decode_request_response_encoded(rpc_canister_response)?;
    let decoded_response: EthCallResponse = serde_json::from_str(&encoded_response)
        .map_err(|err| ManagerError::DecodingError(format!("{}", err)))?;
    Ok(decoded_response.result)
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

    let FeeEstimates {
        max_fee_per_gas,
        max_priority_fee_per_gas,
    } = estimate_transaction_fees(9, rpc.clone(), rpc_canister).await?;

    let key_id = EcdsaKeyId {
        curve: EcdsaCurve::Secp256k1,
        name: String::from("key_1"),
    };

    let request = SignRequest {
        chain_id: 1,
        from: None,
        to: TxKind::Call(
            Address::from_str(&to)
                .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?,
        ),
        max_fee_per_gas: max_fee_per_gas.to::<u128>(),
        max_priority_fee_per_gas: max_priority_fee_per_gas.to::<u128>(),
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
