use std::{str::FromStr, sync::Arc, time::Duration};

use crate::{
    charger::{check_threshold, recharge_cketh, transfer_cketh},
    signer::{get_canister_public_key, pubkey_bytes_to_address},
    state::*,
    strategy::StrategyData,
    types::{InitArgs, ManagerError, Market, StrategyQueryData, SwapResponse},
    utils::{generate_strategies, only_controller, string_to_address},
};
use alloy_primitives::Address;
use ic_canister::{generate_idl, query, update, Canister, Idl, PreUpdate};
use ic_exports::{
    candid::Principal,
    ic_cdk::{
        api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId},
        caller, print, spawn,
    },
    ic_cdk_timers::{set_timer, set_timer_interval},
};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    /// Initializes the strategy data with placeholders based on the `strategies_count` provided.
    #[update]
    pub fn start(&mut self, strategies_count: u64) -> Result<(), ManagerError> {
        only_controller(caller())?;

        STRATEGY_DATA.with(|strategies| {
            let mut state = strategies.borrow_mut();
            let placeholder_data = vec![StrategyData::default(); strategies_count as usize];

            // Insert each placeholder strategy into the state HashMap.
            placeholder_data
                .into_iter()
                .enumerate()
                .for_each(|(index, strategy)| {
                    state.insert(index as u32, strategy);
                });
        });

        Ok(())
    }

    /// Generates and assigns derivation paths and public keys for each strategy.
    /// The `eoa_pk` and `derivation_path` fields in each strategy are updated.
    #[update]
    pub async fn assign_keys(&mut self) -> Result<(), ManagerError> {
        only_controller(caller())?;

        let strategies = STRATEGY_DATA.with(|strategies_hashmap| {
            let binding = strategies_hashmap.borrow();
            binding.clone().into_iter()
        });

        for (id, strategy) in strategies {
            if strategy.eoa_pk.is_some() {
                continue;
            }
            let derivation_path = vec![id.to_be_bytes().to_vec()];
            let key_id = EcdsaKeyId {
                curve: EcdsaCurve::Secp256k1,
                name: String::from("key_1"),
            };

            // Asynchronously calculate the public key for each strategy
            let public_key_bytes =
                get_canister_public_key(key_id, None, Some(derivation_path.clone())).await;
            let eoa_pk = Address::from_str(&pubkey_bytes_to_address(&public_key_bytes)).unwrap();
            STRATEGY_DATA.with(|strategies_hashmap| {
                let mut state_strategies = strategies_hashmap.borrow_mut();
                let state_strategy = state_strategies.get_mut(&(id as u32)).unwrap();
                state_strategy.eoa_pk = Some(eoa_pk);
                state_strategy.derivation_path = derivation_path;
            });
        }

        Ok(())
    }

    /// Starts timers for executing strategies and managing the canister's cycle balance.
    /// Each strategy executes on a 1-hour interval, and cycle balance checks happen every 24 hours.
    #[update]
    pub async fn start_timers(&self, init_args: InitArgs) -> Result<(), ManagerError> {
        only_controller(caller())?;

        let state_strategies_len = STRATEGY_DATA.with(|strategies| strategies.borrow().len());
        if state_strategies_len != init_args.markets.len() * init_args.strategies.len() {
            return Err(ManagerError::Custom("The original count of strategies does not correspond to the number of markets and strategies that is sent.".to_string()));
        }

        // Parse and assign initialization arguments
        let collateral_registry = string_to_address(init_args.collateral_registry)?;
        let hint_helper = string_to_address(init_args.hint_helper)?;

        let rpc_principal = init_args.rpc_principal;
        let strategies = init_args.strategies;
        let rpc_url = init_args.rpc_url;
        let markets = init_args.markets;
        let upfront_fee_period = init_args.upfront_fee_period;

        let mut managers = vec![];

        // Parse markets into usable data structures
        let parsed_markets: Vec<Market> = markets
            .into_iter()
            .map(|market| {
                managers.push(market.manager.clone());
                Market::try_from(market)
            })
            .collect::<Result<Vec<Market>, ManagerError>>()?;

        // Update the MANAGERS state with the market managers
        MANAGERS.with(|managers_vector| *managers_vector.borrow_mut() = managers);

        // Generate strategies from parsed market data and initialization arguments
        let generated_strategies = generate_strategies(
            parsed_markets,
            collateral_registry,
            hint_helper,
            strategies,
            rpc_principal,
            rpc_url,
            upfront_fee_period,
        )
        .await?;

        STRATEGY_DATA.with(|data| {
            *data.borrow_mut() = generated_strategies;
        });

        // Retrieve all strategies for setting up timers
        let strategies = STRATEGY_DATA.with(|vector_data| vector_data.borrow().clone());
        let max_retry_attempts = Arc::new(MAX_RETRY_ATTEMPTS.with(|attempts| attempts.get()));

        // Set timers for each strategy (execute every 1 hour)
        strategies.into_iter().for_each(|(_, strategy)| {
            let max_retry_attempts = Arc::clone(&max_retry_attempts);

            set_timer(Duration::ZERO, move || {
                let mut strategy = strategy.clone();
                let max_retry_attempts = Arc::clone(&max_retry_attempts);
                spawn(async move {
                    print(format!(
                        "[INIT] Running strategy number {} with EOA {:#?}",
                        strategy.key, strategy.eoa_pk.unwrap()
                    ));
                    for turn in 1..=*max_retry_attempts {
                        let result = strategy.execute().await;

                        // Handle success or failure for each strategy execution attempt
                        match result {
                            Ok(()) => {
                                print(format!("[FINISH] Strategy number {} ran successfully on attempt number {}.", strategy.key, turn));
                                break;
                            }, // Exit on success
                            Err(err) => {
                                let _ = strategy.unlock(); // Unlock on failure
                                print(format!(
                                    "[ERROR] Error running strategy number {}, attempt {} => {:#?}",
                                    strategy.key, turn, err
                                ));
                            }
                        }
                    }
                });
            });

            // set_timer_interval(Duration::from_secs(3600), move || {
            //     let mut strategy = strategy.clone();
            //     let max_retry_attempts = Arc::clone(&max_retry_attempts);
            //     spawn(async move {
            //         for turn in 1..=*max_retry_attempts {
            //             let result = strategy.execute().await;

            //             // Handle success or failure for each strategy execution attempt
            //             match result {
            //                 Ok(()) => break, // Exit on success
            //                 Err(err) => {
            //                     let _ = strategy.unlock(); // Unlock on failure
            //                     print(format!(
            //                         "[ERROR] Error running strategy number {}, attempt {} => {:#?}",
            //                         strategy.key, turn, err
            //                     ));
            //                 }
            //             }
            //         }
            //     });
            // });
        });

        // Set a recurring timer for recharging ckETH balance (execute every 24 hours)
        set_timer_interval(Duration::from_secs(86_400), move || {
            let max_retry_attempts = Arc::clone(&max_retry_attempts);
            spawn(async move {
                let mut turn = 0;

                while turn <= *max_retry_attempts {
                    let result = recharge_cketh().await;

                    match result {
                        Ok(()) => break, // Exit on success
                        Err(err) => {
                            print(format!(
                                "[ERROR] Error running the daily ckETH recharge cycle, attempt {} => {:#?}",
                                turn, err
                            ));
                            if turn == *max_retry_attempts {
                                break; // Stop retrying after max attempts
                            }
                        }
                    }

                    turn += 1;
                }
            });
        });

        Ok(())
    }

    /// Retrieves a list of strategies currently stored in the state.
    #[query]
    pub fn get_strategies(&self) -> Vec<StrategyQueryData> {
        STRATEGY_DATA.with(|vector_data| {
            let binding = vector_data.borrow();
            let values = binding.values();
            if values.len() == 0 {
                return vec![];
            }
            values
                .map(|strategy| StrategyQueryData::from(strategy.clone()))
                .collect()
        })
    }

    /// Returns the strategy EOA
    #[query]
    pub fn get_strategy_address(&self, index: u32) -> Option<String> {
        STRATEGY_DATA.with(|data| {
            let binding = data.borrow();
            match binding.get(&index) {
                Some(strategy) => {
                    if let Some(pk) = strategy.eoa_pk {
                        return Some(pk.to_string());
                    }
                    None
                }
                None => None,
            }
        })
    }

    /// Swaps ckETH by first checking the cycle balance, then transferring ckETH to the caller.
    #[update]
    pub async fn swap_cketh(&self) -> Result<SwapResponse, ManagerError> {
        // Ensure the cycle balance is above a certain threshold before proceeding
        check_threshold().await?;
        transfer_cketh(caller()).await
    }

    /// Generates the IDL for the canister interface.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}
