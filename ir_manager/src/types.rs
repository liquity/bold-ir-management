use std::str::FromStr;

use crate::strategy::StrategyData;
use alloy_primitives::Address;
use alloy_sol_types::sol;
use candid::{CandidType, Nat, Principal};
use serde::{Deserialize, Serialize};

use crate::evm_rpc::RpcError;

#[derive(CandidType, Debug)]
pub enum ManagerError {
    NonExistentValue,
    RpcResponseError(RpcError),
    DecodingError(String),
    Locked,
    Custom(String),
    CyclesBalanceAboveRechargingThreshold,
}

pub type DerivationPath = Vec<Vec<u8>>;

#[derive(CandidType)]
pub struct StrategyInput {
    pub target_min: Nat,
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
            manager: value.manager.to_string(),
            latest_rate: value.latest_rate.to_string(),
            target_min: value.target_min.to_string(),
            eoa_pk: value.eoa_pk.map(|pk| pk.to_string()),
            last_update: value.last_update,
        }
    }
}

#[derive(CandidType)]
pub struct MarketInput {
    pub manager: String,
    pub multi_trove_getter: String,
    pub collateral_index: Nat,
    pub batch_managers: Vec<String>,
}

impl TryFrom<MarketInput> for Market {
    type Error = ManagerError;

    fn try_from(value: MarketInput) -> Result<Self, Self::Error> {
        let manager = Address::from_str(&value.manager)
            .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;
        let multi_trove_getter = Address::from_str(&value.multi_trove_getter)
            .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))?;
        let batch_managers: Vec<Address> = value
            .batch_managers
            .into_iter()
            .map(|batch_manager| {
                Address::from_str(&batch_manager)
                    .map_err(|err| ManagerError::DecodingError(format!("{:#?}", err)))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self {
            manager,
            multi_trove_getter,
            collateral_index: value.collateral_index,
            batch_managers,
        })
    }
}

pub struct Market {
    pub manager: Address,
    pub multi_trove_getter: Address,
    pub collateral_index: Nat,
    pub batch_managers: Vec<Address>,
}

#[derive(CandidType)]
pub struct InitArgs {
    pub rpc_principal: Principal,
    pub rpc_url: String,
    pub markets: Vec<MarketInput>,
    pub collateral_registry: String,
    pub hint_helper: String,
    pub strategies: Vec<StrategyInput>,
    pub upfront_fee_period: Nat
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

sol!(
    // Liquity types
    struct CombinedTroveData {
        uint256 id;
        uint256 debt;
        uint256 coll;
        uint256 stake;
        uint256 annualInterestRate;
        uint256 lastDebtUpdateTime;
        uint256 lastInterestRateAdjTime;
        address interestBatchManager;
        uint256 batchDebtShares;
        uint256 batchCollShares;
        uint256 snapshotETH;
        uint256 snapshotBoldDebt;
    }

    // Liquity getters
    function getRedemptionRateWithDecay() public view override returns (uint256);
    function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);
    function getMultipleSortedTroves(int256 _startIdx, uint256 _count) external view returns (CombinedTroveData[] memory _troves);
    function getTroveAnnualInterestRate(uint256 _troveId) external view returns (uint256);
    function predictAdjustBatchInterestRateUpfrontFee(
        uint256 _collIndex,
        address _batchAddress,
        uint256 _newInterestRate
    ) external view returns (uint256);

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
