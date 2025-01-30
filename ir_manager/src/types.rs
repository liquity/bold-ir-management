//! Commonly used types

// This is allowed in this module because the sol! macro doesn't provide its own docs.
#![allow(missing_docs)]

use alloy_sol_types::sol;
use candid::{CandidType, Nat, Principal};
#[cfg(feature = "mainnet")]
use evm_rpc_types::EthMainnetService;
#[cfg(feature = "sepolia")]
use evm_rpc_types::EthSepoliaService;
use serde::{Deserialize, Serialize};

/// Derivation path for the tECDSA signatures
pub type DerivationPath = Vec<Vec<u8>>;

/// Provider service to use
#[cfg(feature = "sepolia")]
pub type ProviderService = EthSepoliaService;

/// Provider service to use
#[cfg(feature = "mainnet")]
pub type ProviderService = EthMainnetService;

/// Strategy input provided by the caller during the initialization phase
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
    /// Sorted troves contract address for this strategy
    pub sorted_troves: String,
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

/// Response for the ckETH<>Cycles swaps
#[derive(CandidType, Debug, Serialize, Deserialize)]
pub struct SwapResponse {
    /// The amount of accepted cycles
    /// The canister will only accept the amount of cycles that it can compensate for with ckETH.
    pub accepted_cycles: Nat,
    /// The amount of ckETH that is returned to the caller.
    pub returning_ether: Nat,
}

/// ICRC-1 subaccount type
pub type Subaccount = [u8; 32];

/// Account representation of ledgers supporting the ICRC1 standard
#[derive(Serialize, CandidType, Deserialize, Clone, Debug, Copy)]
pub struct Account {
    /// Principal ID of the account owner
    pub owner: Principal,
    /// Optional subaccount for the owner principal
    pub subaccount: Option<Subaccount>,
}

/// The HTTPS response format for RPC requests.
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
    function getApproxHint(uint256 _collIndex, uint256 _interestRate, uint256 _numTrials, uint256 _inputRandomSeed)
        external
        view
        returns (uint256 hintId, uint256 diff, uint256 latestRandomSeed);

    function findInsertPosition(uint256 _annualInterestRate, uint256 _prevId, uint256 _nextId)
        external
        view
        override
        returns (uint256, uint256);


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
