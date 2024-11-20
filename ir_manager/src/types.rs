use crate::strategy::executable::ExecutableStrategy;

use alloy_sol_types::sol;
use candid::{CandidType, Nat, Principal};
use serde::{Deserialize, Serialize};

/// Derivation path for the tECDSA signatures
pub type DerivationPath = Vec<Vec<u8>>;

#[derive(CandidType, Deserialize)]
pub struct StrategyInput {
    pub key: u32,
    pub target_min: Nat,
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

impl From<ExecutableStrategy> for StrategyQueryData {
    fn from(value: ExecutableStrategy) -> Self {
        Self {
            latest_rate: value.data.latest_rate.to_string(),
            target_min: value.settings.target_min.to_string(),
            eoa_pk: value.settings.eoa_pk.map(|pk| pk.to_string()),
            last_update: value.data.last_update,
            trove_manager: value.settings.manager.to_string(),
            batch_manager: value.settings.batch_manager.to_string(),
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

#[allow(dead_code)]
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EthCallResponse {
    pub id: u64,
    pub jsonrpc: String,
    pub result: String,
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
