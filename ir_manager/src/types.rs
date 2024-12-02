//! Commonly used types

use crate::strategy::stale::StableStrategy;

use alloy_sol_types::sol;
use candid::{CandidType, Nat, Principal};
use evm_rpc_types::EthSepoliaService;
use serde::{Deserialize, Serialize};

/// Derivation path for the tECDSA signatures
pub type DerivationPath = Vec<Vec<u8>>;

/// Provider service to use
// AUDIT: The following enums will be replaced by the Ethereum main-net providers.
// AUDIT: Misconfiguration due to Sepolia types is out of scope.
pub type ProviderService = EthSepoliaService;

#[derive(CandidType, Deserialize)]
pub struct StrategyInput {
    /// Key in the Hashmap<u32, StrategyData> that is `STRATEGY_DATA`
    pub key: u32,
    /// Minimum target for this strategy
    pub target_min: Nat,
    /// Manager contract address for this strategy
    pub manager: String,
    /// Multi trove getter contract address for this strategy
    pub multi_trove_getter: String,
    /// Collateral index
    pub collateral_index: Nat,
    /// EVM RPC Canister's principal
    pub rpc_principal: Principal,
    /// Upfront fee period constant denominated in seconds
    pub upfront_fee_period: Nat,
    /// Collateral registry contract address
    pub collateral_registry: String,
    /// Hint helper contract address.
    pub hint_helper: String,
}

#[derive(CandidType)]
pub struct StrategyQueryData {
    pub trove_manager: String,
    /// Batch manager contract address for this strategy
    pub batch_manager: String,
    /// Lock for the strategy. Determines if the strategy is currently being executed.
    pub locked: bool,
    /// Latest rate determined by the canister in the previous cycle
    pub latest_rate: String,
    /// Minimum target for this strategy
    pub target_min: String,
    /// The EOA's public key
    pub eoa_pk: Option<String>,
    /// Timestamp of the last time the strategy had updated the batch's interest rate.
    /// Denominated in seconds.
    pub last_update: u64,
}

impl From<StableStrategy> for StrategyQueryData {
    fn from(value: StableStrategy) -> Self {
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
