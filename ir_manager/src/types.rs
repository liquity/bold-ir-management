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

pub struct StrategyData {
    pub manager: String,
    pub latest_rate: u32,
    pub derivation_path: DerivationPath,
    pub target_min: u32,
    pub upfront_fee_period: u32,
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
    function getEntireSystemDebt() public view returns (uint256 entireSystemDebt);
    function getUnbackedPortionPriceAndRedeemability() external returns (uint256, uint256, bool);
    function getMultipleSortedTroves(int256 _startIdx, uint256 _count) external view returns (CombinedTroveData[] memory _troves);
);
