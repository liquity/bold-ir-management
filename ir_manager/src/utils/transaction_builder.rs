//! Transaction builder (and sender) that interacts with the EVM RPC canister

use std::str::FromStr;

use alloy::consensus::TxEip1559;
use alloy_primitives::{Address, Bytes, TxKind, U256};
use evm_rpc_types::RpcServices;
use ic_exports::ic_cdk::api::management_canister::ecdsa::{EcdsaCurve, EcdsaKeyId};

use crate::{
    constants::CHAIN_ID,
    providers::{extract_multi_rpc_send_raw_transaction_status, get_ranked_rpc_providers},
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

    /// Builds the TransactionBuilder into a Transaction and sends it.
    /// Makes async calls to estimate the gas limit, priority fee per gas unit, and fee per gas.
    /// Handles the signing internally.
    pub async fn send(self, rpc_canister: &Service) -> ManagerResult<SendRawTransactionStatus> {
        let chain_id = CHAIN_ID;
        let input = Bytes::from(self.data.clone());
        let rpc: RpcServices = get_ranked_rpc_providers();
        let block_tag = get_block_tag(rpc_canister, true).await?;
        let FeeEstimates {
            max_fee_per_gas,
            max_priority_fee_per_gas,
        } = estimate_transaction_fees(9, rpc_canister, block_tag.clone()).await?;

        let estimated_gas =
            super::gas::get_estimate_gas(rpc_canister, self.data, self.to.clone(), self.from)
                .await?;

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
            gas_limit: estimated_gas.to::<u128>(),
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
                let extracted_response =
                    extract_multi_rpc_send_raw_transaction_status(rpc, response)?;
                Ok(extracted_response)
            }
            Err(e) => Err(ManagerError::Custom(e.1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DerivationPath;
    use alloy_primitives::U256;

    #[test]
    fn test_default_transaction_builder() {
        let builder = TransactionBuilder::default();
        assert_eq!(builder.to, "");
        assert_eq!(builder.from, "");
        assert_eq!(builder.data, Vec::<u8>::new());
        assert_eq!(builder.value, U256::ZERO);
        assert_eq!(builder.nonce, 0);
        assert_eq!(builder.derivation_path, DerivationPath::default());
        assert_eq!(builder.cycles, 0);
    }

    #[test]
    fn test_set_to() {
        let to_address = "0x0123456789abcdef0123456789abcdef01234567".to_string();
        let builder = TransactionBuilder::default().to(to_address.clone());
        assert_eq!(builder.to, to_address);
    }

    #[test]
    fn test_set_from() {
        let from_address = "0xabcdef0123456789abcdef0123456789abcdef01".to_string();
        let builder = TransactionBuilder::default().from(from_address.clone());
        assert_eq!(builder.from, from_address);
    }

    #[test]
    fn test_set_data() {
        let data = vec![0xde, 0xad, 0xbe, 0xef];
        let builder = TransactionBuilder::default().data(data.clone());
        assert_eq!(builder.data, data);
    }

    #[test]
    fn test_set_value() {
        let value = U256::from(1000);
        let builder = TransactionBuilder::default().value(value);
        assert_eq!(builder.value, value);
    }

    #[test]
    fn test_set_nonce() {
        let nonce = 42;
        let builder = TransactionBuilder::default().nonce(nonce);
        assert_eq!(builder.nonce, nonce);
    }

    #[test]
    fn test_set_derivation_path() {
        let derivation_path = vec![vec![44, 60, 0, 0, 0]];
        let builder = TransactionBuilder::default().derivation_path(derivation_path.clone());
        assert_eq!(builder.derivation_path, derivation_path);
    }

    #[test]
    fn test_set_cycles() {
        let cycles = 1_000_000;
        let builder = TransactionBuilder::default().cycles(cycles);
        assert_eq!(builder.cycles, cycles);
    }

    #[test]
    fn test_chained_setters() {
        let to_address = "0x0123456789abcdef0123456789abcdef01234567".to_string();
        let from_address = "0xabcdef0123456789abcdef0123456789abcdef01".to_string();
        let data = vec![0xde, 0xad, 0xbe, 0xef];
        let value = U256::from(1000);
        let nonce = 42;
        let derivation_path = vec![vec![44, 60, 0, 0, 0]];
        let cycles = 1_000_000;

        let builder = TransactionBuilder::default()
            .to(to_address.clone())
            .from(from_address.clone())
            .data(data.clone())
            .value(value)
            .nonce(nonce)
            .derivation_path(derivation_path.clone())
            .cycles(cycles);

        assert_eq!(builder.to, to_address);
        assert_eq!(builder.from, from_address);
        assert_eq!(builder.data, data);
        assert_eq!(builder.value, value);
        assert_eq!(builder.nonce, nonce);
        assert_eq!(builder.derivation_path, derivation_path);
        assert_eq!(builder.cycles, cycles);
    }
}
