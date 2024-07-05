use alloy_primitives::U256;
use alloy_sol_types::sol;
use candid::{CandidType, Principal};

use crate::evm_rpc::RpcError;

#[derive(CandidType, Debug)]
pub enum ManagerError {
    NonExistentValue,
    RpcResponseError(RpcError),
    DecodingError(String),
    Locked,
    Custom(String),
}

pub type DerivationPath = Vec<Vec<u8>>;

#[derive(Clone)]
pub struct StrategyData {
    pub manager: String,
    pub multi_trove_getter: String,
    pub latest_rate: U256,
    pub derivation_path: DerivationPath,
    pub target_min: U256,
    pub upfront_fee_period: U256,
    pub eoa_nonce: u64,
    pub eoa_pk: Option<String>,
    pub last_update: u64,
    pub lock: bool,
}

#[derive(CandidType)]
pub struct StrategyInput {
    pub upfront_fee_period: String, // Because IC does not accept U256 params.
    pub target_min: String,         // Because IC does not accept U256 params.
}

#[derive(CandidType)]
pub struct StrategyQueryData {
    pub manager: String,
    pub latest_rate: String,
    pub target_min: String,
    pub eoa_pk: Option<String>,
    pub last_update: u64,
}

impl From<StrategyData> for StrategyQueryData {
    fn from(value: StrategyData) -> Self {
        Self {
            manager: value.manager,
            latest_rate: value.latest_rate.to_string(),
            target_min: value.target_min.to_string(),
            eoa_pk: value.eoa_pk,
            last_update: value.last_update,
        }
    }
}

#[derive(CandidType)]
pub struct InitArgs {
    pub rpc_principal: Principal,
    pub rpc_url: String,
    pub managers: Vec<String>,
    pub multi_trove_getters: Vec<String>,
    pub collateral_registry: String,
    pub strategies: Vec<StrategyInput>,
}

sol!(
    // Liquity types
    struct CombinedTroveData {
        uint256 id;
        uint256 debt;
        uint256 coll;
        uint256 stake;
        uint256 snapshotETH;
        uint256 snapshotBoldDebt;
    }

    // Liquity getters
    function getRedemptionRateWithDecay() public view override returns (uint256);
    function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);
    function getMultipleSortedTroves(int256 _startIdx, uint256 _count) external view returns (CombinedTroveData[] memory _troves);
    function getTroveAnnualInterestRate(uint256 _troveId) external view returns (uint256);

    // Liquity externals
    function setBatchManagerAnnualInterestRate(
        uint128 _newAnnualInterestRate,
        uint256 _upperHint,
        uint256 _lowerHint,
        uint256 _maxUpfrontFee
    ) external;

    // ckETH Helper
    function deposit(bytes32 _principal) public payable;
);
