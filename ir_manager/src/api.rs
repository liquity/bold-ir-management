use alloy::sol_types::SolCall;
use alloy::{primitives::keccak256, sol};
use ic_canister::{generate_idl, init, query, update, Canister, Idl, PreUpdate};
use ic_exports::{candid::Principal, ic_cdk_timers::set_timer_interval, ic_kit::ic::spawn};
use serde_json::json;
use std::{
    cell::{Ref, RefCell, RefMut},
    rc::Rc,
    time::Duration,
};

use crate::{
    evm_rpc::{RpcService, Service},
    state::IrState,
    strategy::run_strategy,
    types::ManagerError,
    utils::{rpc_canister, rpc_provider},
};

#[derive(Canister)]
pub struct IrManager {
    #[id]
    id: Principal,

    #[state]
    pub state: Rc<RefCell<IrState>>,
}

impl PreUpdate for IrManager {}

impl IrManager {
    // STATE FUNCTIONS
    fn state(&self) -> Ref<IrState> {
        RefCell::borrow(&self.state)
    }

    fn mut_state(&mut self) -> RefMut<IrState> {
        RefCell::borrow_mut(&self.state)
    }

    // INITIALIZATION
    #[init]
    pub fn init(
        &mut self,
        rpc_canister: Principal,
        weth_manager: String,
        reth_manager: String,
        wsteth_manager: String,
    ) {
        self.mut_state().rpc_canister = Some(rpc_canister);
        self.mut_state().weth_manager = weth_manager;
        self.mut_state().reth_manager = reth_manager;
        self.mut_state().wsteth_manager = wsteth_manager;

        // Timers need to start here
        set_timer_interval(Duration::from_secs(3600), || spawn(run_strategy()));
        set_timer_interval(Duration::from_secs(3600), || spawn(run_strategy()));
        set_timer_interval(Duration::from_secs(3600), || spawn(run_strategy()));
    }

    // UPDATE FUNCTIONS
    #[update]
    pub async fn execute_strategy(&mut self, trove_manager: Principal) -> Result<(), ManagerError> {
        let rpc_canister: Service = rpc_canister(self.state().rpc_canister)?;
        let rpc: RpcService = rpc_provider(&self.state().rpc_url);

        // Fetch data
        self.fetch_entire_system_debt(rpc_canister, rpc_url);

        Ok(())
    }

    async fn fetch_entire_system_debt(&self, rpc_canister: Service, rpc: RpcService) {
        sol!(
            function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
        );

        let liquity_base_address = self.state().liquity_base;
        let function_signature = "getEntireSystemDebt()";
        let selector = &keccak256(function_signature.as_bytes())[0..4];
        let mut data: Vec<u8> = Vec::from(selector);

        let json_data: String = json!({
                "id": 1,
                "jsonrpc": "2.0",
                "params": [ {
                    "to": liquity_base_address,
                    "data": format!("0x{}", hex::encode(data))
                }
                ],
                "method": "eth_call"
        })
        .to_string();

        let returned_data = match rpc_canister
            .request(
                rpc_provider(&self.state().rpc_url),
                json_data,
                500000,
                10_000_000_000,
            )
            .await
        {
            Ok(_) => todo!(),
            Err(_) => todo!(),
        };

        let dec = getEntireSystemDebtCall::abi_decode_returns(return_data, false);
    }

    // QUERY FUNCTIONS
    #[query]
    pub fn get_rpc_canister(&self) -> Option<Principal> {
        self.state().rpc_canister
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}
