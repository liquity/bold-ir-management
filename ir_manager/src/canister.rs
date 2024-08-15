use std::{str::FromStr, sync::Arc, time::Duration};

use crate::{
    charger::{check_threshold, recharge_cketh, transfer_cketh},
    signer::{get_canister_public_key, pubkey_bytes_to_address},
    state::*,
    strategy::StrategyData,
    types::{InitArgs, ManagerError, Market, StrategyQueryData, SwapResponse},
    utils::{generate_strategies, only_controller, retry},
};
use alloy_primitives::Address;
use ic_canister::{generate_idl, query, update, Canister, Idl, PreUpdate};
use ic_exports::{
    candid::Principal,
    ic_cdk::{
        api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
        caller, spawn,
    },
    ic_cdk_timers::set_timer_interval,
};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    // INITIALIZATION
    #[update]
    pub fn start(&mut self, strategies_count: u64) -> Result<(), ManagerError> {
        only_controller(caller())?;
        STRATEGY_DATA.with(|strategies| {
            let mut state = strategies.borrow_mut();
            let placeholder_data = vec![StrategyData::default(); strategies_count as usize];
            placeholder_data
                .into_iter()
                .enumerate()
                .for_each(|(index, strategy)| {
                    state.insert(index as u32, strategy);
                });
        });
        Ok(())
    }

    /// Generates derivation paths and public keys for each strategy.
    /// Updates the `eoa_pk` and `derivation_path` fields of each strategy in the HashMap.
    #[update]
    pub async fn assign_keys(&mut self) -> Result<(), ManagerError> {
        only_controller(caller())?;

        let strategies_len =
            STRATEGY_DATA.with(|strategies_hashmap| strategies_hashmap.borrow().len());

        for id in 0..strategies_len {
            let derivation_path = vec![id.to_be_bytes().to_vec()];
            let key_id = EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: String::from("key_1"),
            };

            // Calculate the public key asynchronously
            let public_key_bytes =
                get_canister_public_key(key_id, None, Some(derivation_path.clone())).await;
            let eoa_pk = Address::from_str(&pubkey_bytes_to_address(&public_key_bytes)).unwrap();

            // Update the strategy with the public key
            STRATEGY_DATA.with(|strategies_hashmap| {
                let mut state_strategies = strategies_hashmap.borrow_mut();
                let state_strategy = state_strategies.get_mut(&(id as u32)).unwrap();
                state_strategy.eoa_pk = Some(eoa_pk);
                state_strategy.derivation_path = derivation_path;
            });
        }

        Ok(())
    }

    /// Starts timers for all strategies, and a recurring timer for cycle balance checks.
    ///
    /// Workflow:
    /// 1. Start the canister:
    /// 2. Start strategy execution timers:
    ///    - Each strategy has its own timer, triggering every 1 hour.
    /// 3. Start a 24-hour recurring timer:
    ///    - Checks the ckETH balance and recharges if needed.
    pub fn start_timers(&self, init_args: InitArgs) -> Result<(), ManagerError> {
        only_controller(caller())?;
        let state_strategies_len = STRATEGY_DATA.with(|strategies| strategies.borrow().len());
        if state_strategies_len != init_args.markets.len() * init_args.strategies.len() {
            return Err(ManagerError::Custom("The original count of strategies does not correspond to the number of markets and strategies that is sent.".to_string()));
        }

        // Assigning init_args field values to variables
        let collateral_registry = Address::from_str(&init_args.collateral_registry)
            .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;
        let hint_helper = Address::from_str(&init_args.hint_helper)
            .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;

        let rpc_principal = init_args.rpc_principal;
        let strategies = init_args.strategies;
        let rpc_url = init_args.rpc_url;
        let markets = init_args.markets;
        let upfront_fee_period = init_args.upfront_fee_period;

        let mut managers = vec![];

        let parsed_markets: Vec<Market> = markets
            .into_iter()
            .map(|market| {
                managers.push(market.manager.clone());
                Market::try_from(market)
            })
            .collect::<Result<Vec<Market>, ManagerError>>()?;

        MANAGERS.with(|managers_vector| *managers_vector.borrow_mut() = managers);

        STRATEGY_DATA.with(|data| {
            let generated_strategies = generate_strategies(
                parsed_markets,
                collateral_registry,
                hint_helper,
                strategies,
                rpc_principal,
                rpc_url,
                upfront_fee_period,
            );
            *data.borrow_mut() = generated_strategies
        });

        // assign a separate timer for each strategy
        let strategies = STRATEGY_DATA.with(|vector_data| vector_data.borrow().clone());

        let max_retry_attempts = Arc::new(MAX_RETRY_ATTEMPTS.with(|attempts| attempts.get()));

        // STRATEGY TIMER | EVERY 1 HOUR
        strategies.into_iter().for_each(|(key, strategy)| {
            let max_retry_attempts = Arc::clone(&max_retry_attempts);
            set_timer_interval(Duration::from_secs(3600), move || {
                let mut strategy = strategy.clone();
                let max_retry_attempts = Arc::clone(&max_retry_attempts);
                spawn(async move {
                    for turn in 0..=*max_retry_attempts {
                        let result = match strategy.execute().await {
                            Ok(()) => Ok(()),
                            Err(error) => retry(key, &mut strategy.clone(), error).await,
                        };

                        if result.is_ok() {
                            break;
                        } else if turn == *max_retry_attempts && result.is_err() {
                            let _ = strategy.unlock();
                        }
                    }
                });
            });
        });

        // CKETH RECHARGER | EVERY 24 HOURS
        set_timer_interval(Duration::from_secs(86_400), move || {
            let max_retry_attempts = Arc::clone(&max_retry_attempts);
            spawn(async move {
                for _ in 0..=*max_retry_attempts {
                    let result = match recharge_cketh().await {
                        Ok(()) => Ok(()),
                        Err(_error) => recharge_cketh().await,
                    };

                    if result.is_ok() {
                        break;
                    }
                }
            });
        });

        Ok(())
    }

    #[query]
    pub fn get_strategies(&self) -> Vec<StrategyQueryData> {
        STRATEGY_DATA.with(|vector_data| {
            vector_data
                .borrow()
                .values()
                .map(|strategy| StrategyQueryData::from(strategy.clone()))
                .collect()
        })
    }

    #[update]
    pub async fn swap_cketh(&self) -> Result<SwapResponse, ManagerError> {
        // lock / unlock based on the current cycles balance of the canister
        check_threshold().await?;
        transfer_cketh(caller()).await
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
