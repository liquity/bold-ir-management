use alloy_primitives::U256;
use alloy_sol_types::sol;
use candid::CandidType;

use crate::evm_rpc::RpcError;

#[derive(CandidType, Debug)]
pub enum ManagerError {
    NonExistentValue,
    RpcResponseError(RpcError),
    DecodingError(String),
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
    pub eoa_nonce: U256,
    pub eoa_pk: Option<String>,
}

#[derive(CandidType)]
pub struct StrategyInput {
    pub upfront_fee_period: String, // Because IC does not accept U256 params.
    pub target_min: String, // Because IC does not accept U256 params.
}

sol!(
    struct CombinedTroveData {
        uint256 id;
        uint256 debt;
        uint256 coll;
        uint256 stake;
        uint256 snapshotETH;
        uint256 snapshotBoldDebt;
    }
    function getRedemptionRateWithDecay() public view override returns (uint256);
    function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);
    function getMultipleSortedTroves(int256 _startIdx, uint256 _count) external view returns (CombinedTroveData[] memory _troves);

    // ckETH Helper
    function deposit(bytes32 _principal) public payable;
);
