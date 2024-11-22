//! Transaction builder (and sender) that interacts with the EVM RPC canister

use std::str::FromStr;

use alloy::consensus::TxEip1559;
use alloy_primitives::{Address, Bytes, TxKind, U256};
use evm_rpc_types::RpcServices;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId};

use crate::{
    constants::CHAIN_ID,
    providers::{extract_multi_rpc_result, get_ranked_rpc_providers},
    types::DerivationPath,
};

use super::{
    common::get_block_tag,
    error::{ManagerError, ManagerResult},
    evm_rpc::{SendRawTransactionStatus, Service},
    gas::{estimate_transaction_fees, FeeEstimates},
    signer::sign_eip1559_transaction,
};

/// Transaction builder struct
#[derive(Default)]
pub struct TransactionBuilder {
    to: String,
    from: String,
    data: Vec<u8>,
    value: U256,
    nonce: u64,
    derivation_path: DerivationPath,
    cycles: u128,
}

impl TransactionBuilder {
    /// Sets the `to` field
    pub fn to(mut self, to: String) -> Self {
        self.to = to;
        self
    }

    /// Sets the `from` field
    pub fn from(mut self, from: String) -> Self {
        self.from = from;
        self
    }

    /// Sets the `data` field
    pub fn data(mut self, data: Vec<u8>) -> Self {
        self.data = data;
        self
    }

    /// Sets the `value` field
    pub fn value(mut self, value: U256) -> Self {
        self.value = value;
        self
    }

    /// Sets the `nonce` field
    pub fn nonce(mut self, nonce: u64) -> Self {
        self.nonce = nonce;
        self
    }

    /// Sets the `derivation_path` field
    pub fn derivation_path(mut self, derivation_path: DerivationPath) -> Self {
        self.derivation_path = derivation_path;
        self
    }

    /// Sets the `cycles` field
    pub fn cycles(mut self, cycles: u128) -> Self {
        self.cycles = cycles;
        self
    }

    /// Builds the TransactionBuilder into a Transaction and sends it
    pub async fn send(self, rpc_canister: &Service) -> ManagerResult<SendRawTransactionStatus> {
        let chain_id = CHAIN_ID;
        let input = Bytes::from(self.data.clone());
        let rpc: RpcServices = get_ranked_rpc_providers();
        let block_tag = get_block_tag(rpc_canister, true).await?;
        let FeeEstimates {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } = estimate_transaction_fees(9, rpc.clone(), rpc_canister, block_tag.clone()).await?;

        // let block_number = if let BlockTag::Number(num) = block_tag {
        //     num
        // } else {
        //     unreachable!()
        // };
        // let estimated_gas = get_estimate_gas(
        //     rpc_canister,
        //     self.data,
        //     self.to.clone(),
        //     self.from,
        //     block_number,
        // )
        // .await?;

        let key_id = EcdsaKeyId {
            curve: EcdsaCurve::Secp256k1,
            name: String::from("key_1"),
        };

        let request = TxEip1559 {
            chain_id,
            to: TxKind::Call(
                Address::from_str(&self.to)
                    .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?,
            ),
            max_fee_per_gas,
            max_priority_fee_per_gas,
            value: self.value,
            nonce: self.nonce,
            gas_limit: 450_000, // TODO: THIS NEEDS TO CHANGE. THE GET_ESTIMATE_GAS FN FAILS AT CONSENSUS
            access_list: Default::default(),
            input,
        };

        let signed_transaction =
            sign_eip1559_transaction(request, key_id, self.derivation_path).await?;

        match rpc_canister
            .eth_send_raw_transaction(rpc.clone(), None, signed_transaction, self.cycles)
            .await
        {
            Ok((response,)) => {
                let extracted_response = extract_multi_rpc_result(rpc, response)?;
                Ok(extracted_response)
            }
            Err(e) => Err(ManagerError::Custom(e.1)),
        }
    }
}
