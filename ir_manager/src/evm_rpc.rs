// This is an experimental feature to generate Rust binding from Candid.
// You may want to manually adjust some of the types.
#![allow(
    dead_code,
    unused_imports,
    non_snake_case,
    clippy::large_enum_variant,
    clippy::enum_variant_names
)]
use std::str::FromStr;

use candid::{self, CandidType, Decode, Deserialize, Encode, Principal};
use evm_rpc_types::*;
use ic_exports::ic_cdk::{self, api::call::CallResult as Result};

#[derive(Copy, Clone)]
pub struct Service(pub Principal);

impl Default for Service {
    fn default() -> Self {
        Self(Principal::anonymous())
    }
}

impl Service {
    pub async fn eth_fee_history(
        &self,
        arg0: RpcServices,
        arg1: Option<RpcConfig>,
        arg2: FeeHistoryArgs,
        cycles: u128,
    ) -> Result<(MultiRpcResult<FeeHistory>,)> {
        ic_cdk::api::call::call_with_payment128(
            self.0,
            "eth_feeHistory",
            (arg0, arg1, arg2),
            cycles,
        )
        .await
    }

    pub async fn eth_get_transaction_count(
        &self,
        arg0: RpcServices,
        arg1: Option<RpcConfig>,
        arg2: GetTransactionCountArgs,
    ) -> Result<(MultiRpcResult<Nat256>,)> {
        ic_cdk::call(self.0, "eth_getTransactionCount", (arg0, arg1, arg2)).await
    }

    pub async fn eth_get_transaction_receipt(
        &self,
        arg0: RpcServices,
        arg1: Option<RpcConfig>,
        arg2: Hex32,
    ) -> Result<(MultiRpcResult<Option<TransactionReceipt>>,)> {
        ic_cdk::call(self.0, "eth_getTransactionReceipt", (arg0, arg1, arg2)).await
    }

    pub async fn eth_send_raw_transaction(
        &self,
        arg0: RpcServices,
        arg1: Option<RpcConfig>,
        arg2: String,
        cycles: u128,
    ) -> Result<(MultiRpcResult<SendRawTransactionStatus>,)> {
        ic_cdk::api::call::call_with_payment128(
            self.0,
            "eth_sendRawTransaction",
            (arg0, arg1, arg2),
            cycles,
        )
        .await
    }
    
    pub async fn get_providers(&self) -> Result<(Vec<Provider>,)> {
        ic_cdk::call(self.0, "getProviders", ()).await
    }
    
    pub async fn get_service_provider_map(&self) -> Result<(Vec<(RpcService, u64)>,)> {
        ic_cdk::call(self.0, "getServiceProviderMap", ()).await
    }
    
    pub async fn request(
        &self,
        arg0: RpcService,
        arg1: String,
        arg2: u64,
        cycles: u128,
    ) -> Result<(RpcResult<String>,)> {
        ic_cdk::api::call::call_with_payment128(self.0, "request", (arg0, arg1, arg2), cycles).await
    }
    
    pub async fn request_cost(
        &self,
        arg0: RpcService,
        arg1: String,
        arg2: u64,
    ) -> Result<(RpcResult<u128>,)> {
        ic_cdk::call(self.0, "requestCost", (arg0, arg1, arg2)).await
    }

    pub async fn eth_call(&self, source: RpcServices, config: Option<RpcConfig>, args: CallArgs) -> Result<(MultiRpcResult<Hex>,)> {
        ic_cdk::call(self.0, "eth_call", (source, config, args)).await
    }
}
