use candid::{self, CandidType, Deserialize, Nat, Principal};
use evm_rpc_types::{MultiRpcResult, RpcConfig, RpcResult, RpcService, RpcServices};
use ic_exports::ic_cdk::{self, api::call::CallResult as Result};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Deserialize)]
pub struct GetTransactionCountArgs {
    pub address: String,
    pub block: BlockTag,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Deserialize, Default)]
pub enum BlockTag {
    #[default]
    Latest,
    Finalized,
    Safe,
    Earliest,
    Pending,
    Number(Nat),
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Deserialize)]
pub struct FeeHistoryArgs {
    /// Number of blocks in the requested range.
    /// Typically, providers request this to be between 1 and 1024.
    #[serde(rename = "blockCount")]
    pub block_count: Nat,

    /// Highest block of the requested range.
    /// Integer block number, or "latest" for the last mined block or "pending", "earliest" for not yet mined transactions.
    #[serde(rename = "newestBlock")]
    pub newest_block: BlockTag,

    /// A monotonically increasing list of percentile values between 0 and 100.
    /// For each block in the requested range, the transactions will be sorted in ascending order
    /// by effective tip per gas and the corresponding effective tip for the percentile
    /// will be determined, accounting for gas consumed.
    #[serde(rename = "rewardPercentiles")]
    pub reward_percentiles: Option<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, CandidType)]
pub struct FeeHistory {
    /// Lowest number block of the returned range.
    #[serde(rename = "oldestBlock")]
    pub oldest_block: Nat,

    /// An array of block base fees per gas.
    /// This includes the next block after the newest of the returned range,
    /// because this value can be derived from the newest block.
    /// Zeroes are returned for pre-EIP-1559 blocks.
    #[serde(rename = "baseFeePerGas")]
    pub base_fee_per_gas: Vec<Nat>,

    /// An array of block gas used ratios (gasUsed / gasLimit).
    #[serde(rename = "gasUsedRatio")]
    pub gas_used_ratio: Vec<f64>,

    /// A two-dimensional array of effective priority fees per gas at the requested block percentiles.
    #[serde(rename = "reward")]
    pub reward: Vec<Vec<Nat>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, CandidType)]
pub enum SendRawTransactionStatus {
    Ok(Option<String>),
    InsufficientFunds,
    NonceTooLow,
    NonceTooHigh,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Deserialize)]
pub struct CallArgs {
    pub transaction: TransactionRequest,
    /// Integer block number, or "latest" for the last mined block or "pending", "earliest" for not yet mined transactions.
    /// Default to "latest" if unspecified, see https://github.com/ethereum/execution-apis/issues/461.
    pub block: Option<BlockTag>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, CandidType, Deserialize)]
pub struct TransactionRequest {
    /// The type of the transaction:
    /// - "0x0" for legacy transactions (pre- EIP-2718)
    /// - "0x1" for access list transactions (EIP-2930)
    /// - "0x2" for EIP-1559 transactions
    #[serde(rename = "type")]
    pub tx_type: Option<String>,

    /// Transaction nonce
    pub nonce: Option<Nat>,

    /// Address of the receiver or `None` in a contract creation transaction.
    pub to: Option<String>,

    /// The address of the sender.
    pub from: Option<String>,

    /// Gas limit for the transaction.
    pub gas: Option<Nat>,

    /// Amount of ETH sent with this transaction.
    pub value: Option<Nat>,

    /// Transaction input data
    pub input: Option<String>,

    /// The legacy gas price willing to be paid by the sender in wei.
    #[serde(rename = "gasPrice")]
    pub gas_price: Option<Nat>,

    /// Maximum fee per gas the sender is willing to pay to miners in wei.
    #[serde(rename = "maxPriorityFeePerGas")]
    pub max_priority_fee_per_gas: Option<Nat>,

    /// The maximum total fee per gas the sender is willing to pay (includes the network / base fee and miner / priority fee) in wei.
    #[serde(rename = "maxFeePerGas")]
    pub max_fee_per_gas: Option<Nat>,

    /// The maximum total fee per gas the sender is willing to pay for blob gas in wei.
    #[serde(rename = "maxFeePerBlobGas")]
    pub max_fee_per_blob_gas: Option<Nat>,

    /// EIP-2930 access list
    #[serde(rename = "accessList")]
    pub access_list: Option<AccessList>,

    /// List of versioned blob hashes associated with the transaction's EIP-4844 data blobs.
    #[serde(rename = "blobVersionedHashes")]
    pub blob_versioned_hashes: Option<Vec<String>>,

    /// Raw blob data.
    pub blobs: Option<Vec<String>>,

    /// Chain ID that this transaction is valid on.
    #[serde(rename = "chainId")]
    pub chain_id: Option<Nat>,
}

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Deserialize)]
#[serde(transparent)]
pub struct AccessList(pub Vec<AccessListEntry>);

#[derive(Clone, Debug, PartialEq, Eq, CandidType, Deserialize)]
pub struct AccessListEntry {
    pub address: String,
    #[serde(rename = "storageKeys")]
    pub storage_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, CandidType)]
pub struct Block {
    /// Base fee per gas
    /// Only included for blocks after the London Upgrade / EIP-1559.
    #[serde(rename = "baseFeePerGas")]
    pub base_fee_per_gas: Option<Nat>,

    /// Block number
    pub number: Nat,

    /// Difficulty
    pub difficulty: Option<Nat>,

    /// Extra data
    #[serde(rename = "extraData")]
    pub extra_data: String,

    /// Maximum gas allowed in this block
    #[serde(rename = "gasLimit")]
    pub gas_limit: Nat,

    /// Gas used by all transactions in this block
    #[serde(rename = "gasUsed")]
    pub gas_used: Nat,

    /// Block hash
    pub hash: String,

    /// Bloom filter for the logs.
    #[serde(rename = "logsBloom")]
    pub logs_bloom: String,

    /// Miner
    pub miner: String,

    /// Mix hash
    #[serde(rename = "mixHash")]
    pub mix_hash: String,

    /// Nonce
    pub nonce: Nat,

    /// Parent block hash
    #[serde(rename = "parentHash")]
    pub parent_hash: String,

    /// Receipts root
    #[serde(rename = "receiptsRoot")]
    pub receipts_root: String,

    /// Ommers hash
    #[serde(rename = "sha3Uncles")]
    pub sha3_uncles: String,

    /// Block size
    pub size: Nat,

    /// State root
    #[serde(rename = "stateRoot")]
    pub state_root: String,

    /// Timestamp
    #[serde(rename = "timestamp")]
    pub timestamp: Nat,

    /// Total difficulty is the sum of all difficulty values up to and including this block.
    ///
    /// Note: this field was removed from the official JSON-RPC specification in
    /// https://github.com/ethereum/execution-apis/pull/570 and may no longer be served by providers.
    #[serde(rename = "totalDifficulty")]
    pub total_difficulty: Option<Nat>,

    /// Transaction hashes
    #[serde(default)]
    pub transactions: Vec<String>,

    /// Transactions root
    #[serde(rename = "transactionsRoot")]
    pub transactions_root: Option<String>,

    /// Uncles
    #[serde(default)]
    pub uncles: Vec<String>,
}

#[derive(Copy, Clone, Debug)]
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
    ) -> Result<(MultiRpcResult<Nat>,)> {
        ic_cdk::call(self.0, "eth_getTransactionCount", (arg0, arg1, arg2)).await
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

    pub async fn get_block_by_number(
        &self,
        arg0: RpcServices,
        arg1: Option<RpcConfig>,
        arg2: BlockTag,
    ) -> Result<(MultiRpcResult<Block>,)> {
        ic_cdk::api::call::call_with_payment128(
            self.0,
            "eth_getBlockByNumber",
            (arg0, arg1, arg2),
            1_000_000_000_u128,
        )
        .await
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
    ) -> Result<(RpcResult<Nat>,)> {
        ic_cdk::call(self.0, "requestCost", (arg0, arg1, arg2)).await
    }

    pub async fn eth_call(
        &self,
        source: RpcServices,
        config: Option<RpcConfig>,
        args: CallArgs,
    ) -> Result<(MultiRpcResult<String>,)> {
        ic_cdk::call(self.0, "eth_call", (source, config, args)).await
    }
}
