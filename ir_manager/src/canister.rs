use std::str::FromStr;

use crate::{
    charger::{check_threshold, transfer_cketh},
    state::*,
    types::{InitArgs, ManagerError, Market, StrategyQueryData, SwapResponse},
    utils::generate_strategies,
};
use alloy_primitives::Address;
use ic_canister::{generate_idl, query, update, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_cdk::caller};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,
}

impl PreUpdate for IrManager {}

impl IrManager {
    // INITIALIZATION
    #[update]
    pub fn start(&mut self, init_args: InitArgs) -> Result<(), ManagerError> {
        // Assigning init_args field values to variables
        let collateral_registry = Address::from_str(&init_args.collateral_registry)
            .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;
        let hint_helper = Address::from_str(&init_args.hint_helper)
            .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;

        let rpc_principal = init_args.rpc_principal;
        let strategies = init_args.strategies;
        let rpc_url = init_args.rpc_url;
        let markets = init_args.markets;

        let parsed_markets: Vec<Market> = markets
            .into_iter()
            .map(|market| Market::try_from(market))
            .collect::<Result<Vec<Market>, ManagerError>>()?;

        STRATEGY_DATA.with(|data| {
            let generated_strategies = generate_strategies(
                parsed_markets,
                collateral_registry,
                hint_helper,
                strategies,
                rpc_principal,
                rpc_url,
            );
            *data.borrow_mut() = generated_strategies
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
