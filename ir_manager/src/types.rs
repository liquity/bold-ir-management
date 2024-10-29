use crate::{state::CHAIN_ID, strategy::StrategyData};

use alloy_sol_types::sol;
use candid::{CandidType, Nat, Principal};
use evm_rpc_types::{RpcApi, RpcError, RpcService, RpcServices};
use ic_exports::ic_kit::RejectionCode;
use serde::{Deserialize, Serialize};

/// IR Manager Canister Result
pub type ManagerResult<T> = Result<T>;

/// IR Manager Canister Errors
#[derive(CandidType, Debug)]
pub enum ManagerError {
    /// `CallResult` error
    CallResult(RejectionCode, String),
    /// Unauthorized access
    Unauthorized,
    /// A requested value does not exist
    NonExistentValue,
    /// Wrapper for the RPC errors returned by the EVM RPC canister
    RpcResponseError(RpcError),
    /// Decoding issue
    DecodingError(String),
    /// Strategy is locked
    Locked,
    /// Unknown/Custom error
    Custom(String),
    /// The cycle balance is above the threshold.
    /// No arbitrage opportunity is available.
    CyclesBalanceAboveRechargingThreshold,
}

/// Derivation path for the tECDSA signatures
pub type DerivationPath = Vec<Vec<u8>>;

#[derive(CandidType, Deserialize)]
pub struct StrategyInput {
    pub key: u32,
    pub target_min: f64,
    pub manager: String,
    pub multi_trove_getter: String,
    pub collateral_index: Nat,
    pub rpc_principal: Principal,
    pub rpc_url: String,
    pub upfront_fee_period: Nat,
    pub collateral_registry: String,
    pub hint_helper: String,
}

#[derive(CandidType)]
pub struct StrategyQueryData {
    pub trove_manager: String,
    pub batch_manager: String,
    pub locked: bool,
    pub latest_rate: String,
    pub target_min: String,
    pub eoa_pk: Option<String>,
    pub last_update: u64,
}

impl From<StrategyData> for StrategyQueryData {
    fn from(value: StrategyData) -> Self {
        Self {
            latest_rate: value.latest_rate.to_string(),
            target_min: value.target_min.to_string(),
            eoa_pk: value.eoa_pk.map(|pk| pk.to_string()),
            last_update: value.last_update,
            trove_manager: value.manager.to_string(),
            batch_manager: value.batch_manager.to_string(),
            locked: value.lock,
        }
    }
}

#[derive(CandidType, Debug, Serialize, Deserialize)]
pub struct SwapResponse {
    pub accepted_cycles: Nat,
    pub returning_ether: Nat,
}

pub type Subaccount = [u8; 32];

// Account representation of ledgers supporting the ICRC1 standard
#[derive(Serialize, CandidType, Deserialize, Clone, Debug, Copy)]
pub struct Account {
    pub owner: Principal,
    pub subaccount: Option<Subaccount>,
}

/// RPC type to use
#[derive(Clone)]
pub enum ProviderSet {
    ManyProviders(RpcServices),
    CustomProvider(String),
}

impl Default for ProviderSet {
    fn default() -> Self {
        Self::CustomProvider("".to_string())
    }
}

impl Into<RpcServices> for ProviderSet {
    fn into(self) -> RpcServices {
        match self {
            ProviderSet::ManyProviders(rpc_services) => rpc_services,
            ProviderSet::CustomProvider(url) => RpcServices::Custom {
                chain_id: CHAIN_ID.with(|id| id.get()),
                services: vec![RpcApi { url, headers: None }],
            },
        }
    }
}

sol!(
    // Liquity types
    struct DebtPerInterestRate {
        address interestBatchManager;
        uint256 interestRate;
        uint256 debt;
    }

    // Liquity getters
    function getRedemptionRateWithDecay() public view override returns (uint256);
    function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);

    function getDebtPerInterestRateAscending(uint256 _collIndex, uint256 _startId, uint256 _maxIterations)
        external
        view
        returns (DebtPerInterestRate[] memory, uint256 currId);

    function getTroveAnnualInterestRate(uint256 _troveId) external view returns (uint256);
    function predictAdjustBatchInterestRateUpfrontFee(
        uint256 _collIndex,
        address _batchAddress,
        uint256 _newInterestRate
    ) external view returns (uint256);

    // Liquity externals
    function setNewRate(
        uint128 _newAnnualInterestRate,
        uint256 _upperHint,
        uint256 _lowerHint,
        uint256 _maxUpfrontFee
    );

    // ckETH Helper
    function deposit(bytes32 _principal) public payable;
);
