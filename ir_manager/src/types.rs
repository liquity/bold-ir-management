use alloy_primitives::U256;
use alloy_sol_types::sol;
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};

use crate::evm_rpc::RpcError;

#[derive(CandidType, Debug)]
pub enum ManagerError {
    NonExistentValue,
    RpcResponseError(RpcError),
    DecodingError(String),
    Locked,
    Custom(String),
    CyclesBalanceAboveRechargingThreshold
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


/// The enum defining the different asset classes.
#[derive(CandidType, Clone, Debug, PartialEq, Deserialize)]
pub enum AssetClass {
    /// The cryptocurrency asset class.
    Cryptocurrency,
    /// The fiat currency asset class.
    FiatCurrency,
}

/// Exchange rates are derived for pairs of assets captured in this struct.
#[derive(CandidType, Clone, Debug, PartialEq, Deserialize)]
pub struct Asset {
    /// The symbol/code of the asset.
    pub symbol: String,
    /// The asset class.
    pub class: AssetClass,
}

/// Exchange Rate Canister's Fetch API Type
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct GetExchangeRateRequest {
    /// The asset to be used as the resulting asset. For example, using
    /// ICP/USD, ICP would be the base asset.
    pub base_asset: Asset,
    /// The asset to be used as the starting asset. For example, using
    /// ICP/USD, USD would be the quote asset.
    pub quote_asset: Asset,
    /// An optional parameter used to find a rate at a specific time.
    pub timestamp: Option<u64>,
}


/// Metadata information to give background on how the rate was determined.
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq)]
pub struct ExchangeRateMetadata {
    /// The scaling factor for the exchange rate and the standard deviation.
    pub decimals: u32,
    /// The number of queried exchanges for the base asset.
    pub base_asset_num_queried_sources: usize,
    /// The number of rates successfully received from the queried sources for the base asset.
    pub base_asset_num_received_rates: usize,
    /// The number of queried exchanges for the quote asset.
    pub quote_asset_num_queried_sources: usize,
    /// The number of rates successfully received from the queried sources for the quote asset.
    pub quote_asset_num_received_rates: usize,
    /// The standard deviation of the received rates, scaled by the factor `10^decimals`.
    pub standard_deviation: u64,
    /// The timestamp of the beginning of the day for which the forex rates were retrieved, if any.
    pub forex_timestamp: Option<u64>,
}

/// When a rate is determined, this struct is used to present the information
/// to the user.
#[derive(CandidType, Clone, Debug, Deserialize, PartialEq)]
pub struct ExchangeRate {
    /// The base asset.
    pub base_asset: Asset,
    /// The quote asset.
    pub quote_asset: Asset,
    /// The timestamp associated with the returned rate.
    pub timestamp: u64,
    /// The median rate from the received rates, scaled by the factor `10^decimals` in the metadata.
    pub rate: u64,
    /// Metadata providing additional information about the exchange rate calculation.
    pub metadata: ExchangeRateMetadata,
}

/// Returned to the user when something goes wrong retrieving the exchange rate.
#[derive(CandidType, Clone, Debug, Deserialize)]
pub enum ExchangeRateError {
    /// Returned when the canister receives a call from the anonymous principal.
    AnonymousPrincipalNotAllowed,
    /// Returned when the canister is in process of retrieving a rate from an exchange.
    Pending,
    /// Returned when the base asset rates are not found from the exchanges HTTP outcalls.
    CryptoBaseAssetNotFound,
    /// Returned when the quote asset rates are not found from the exchanges HTTP outcalls.
    CryptoQuoteAssetNotFound,
    /// Returned when the stablecoin rates are not found from the exchanges HTTP outcalls needed for computing a crypto/fiat pair.
    StablecoinRateNotFound,
    /// Returned when there are not enough stablecoin rates to determine the forex/USDT rate.
    StablecoinRateTooFewRates,
    /// Returned when the stablecoin rate is zero.
    StablecoinRateZeroRate,
    /// Returned when a rate for the provided forex asset could not be found at the provided timestamp.
    ForexInvalidTimestamp,
    /// Returned when the forex base asset is found.
    ForexBaseAssetNotFound,
    /// Returned when the forex quote asset is found.
    ForexQuoteAssetNotFound,
    /// Returned when neither forex asset is found.
    ForexAssetsNotFound,
    /// Returned when the caller is not the CMC and there are too many active requests.
    RateLimited,
    /// Returned when the caller does not send enough cycles to make a request.
    NotEnoughCycles,
    /// Returned if too many collected rates deviate substantially.
    InconsistentRatesReceived,
    /// Until candid bug is fixed, new errors after launch will be placed here.
    Other(OtherError),
}

/// Used to provide details for the [ExchangeRateError::Other] variant field.
#[derive(CandidType, Clone, Debug, Deserialize)]
pub struct OtherError {
    /// The identifier for the error that occurred.
    pub code: u32,
    /// A description of the error that occurred.
    pub description: String,
}

#[derive(CandidType, Debug, Serialize, Deserialize)]
pub struct SwapResponse {
    pub accepted_cycles: u64,
    pub returning_ether: u64
}

/// Short-hand for returning the result of a `get_exchange_rate` request.
pub type GetExchangeRateResult = Result<ExchangeRate, ExchangeRateError>;

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
