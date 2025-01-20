// SPDX-License-Identifier: GPL-3.0

pragma solidity ^0.8.0;
import "./Interfaces/IBoldToken.sol";
import "./Interfaces/ITroveManager.sol";
import "./Interfaces/IWETHPriceFeed.sol";
import "./Interfaces/IBorrowerOperations.sol";
import "./Interfaces/ISortedTroves.sol";
import {BatchId} from "./Types/BatchId.sol";

/**
 * @title Liquity V2 Autonomous Interest Rate Manager
 * @dev Enables BOLD<>ETH conversions at a discounted rate and manages the redistribution of collected Ether.
 * All calculations, including fees and rates, utilize fixed-point arithmetic with 18 decimal places (e18).
 */
contract BatchManager {
    uint256 immutable discountRate; // Discount rate applied to BOLD conversions, expressed in e18.
    address immutable batchManagerEOA; // Address of the entity authorized to manage batches.

    IBorrowerOperations immutable borrowerOperations; // Interface for borrower-related operations.
    ITroveManager immutable troveManager; // Interface for managing troves.
    IBoldToken immutable boldToken; // Interface for the BOLD token.
    IWETHPriceFeed immutable wethPriceFeed; // Interface for fetching the WETH/USD price.
    ISortedTroves immutable sortedTroves; // Interface for managing sorted troves.

    /**
     * @notice Emitted when the BatchManager is initialized.
     * @param batchManagerEOA Address of the batch manager EOA.
     * @param boldToken Address of the BOLD token contract.
     * @param troveManager Address of the TroveManager contract.
     * @param borrowerOperations Address of the BorrowerOperations contract.
     * @param wethPriceFeed Address of the WETH price feed contract.
     */
    event Initialized(
        address batchManagerEOA,
        address boldToken,
        address troveManager,
        address borrowerOperations,
        address wethPriceFeed
    );

    /**
     * @notice Restricts function access to the batch manager EOA.
     */
    modifier onlyBatchManagerEOA() {
        require(msg.sender == batchManagerEOA, "Caller is not batch manager.");
        _;
    }

    /**
     * @notice Initializes the BatchManager contract and registers it with BorrowerOperations.
     * @param _batchManagerEOA Address of the batch manager EOA.
     * @param _troveManager Address of the TroveManager contract.
     * @param _borrowerOperations Address of the BorrowerOperations contract.
     * @param _boldToken Address of the BOLD token contract.
     * @param _wethPricefeed Address of the WETH price feed contract.
     * @param _sortedTroves Address of the SortedTroves contract.
     * @param minInterestRate Minimum interest rate (e18) the batch manager can set.
     * @param maxInterestRate Maximum interest rate (e18) the batch manager can set.
     * @param currentInterestRate Current interest rate (e18) to initialize the batch.
     * @param fee Fee applied to operations (e18).
     * @param minInterestRateChangePeriod Minimum period for interest rate changes.
     * @param _discountRate Discount rate for BOLD conversions (e18).
     */
    constructor(
        address _batchManagerEOA,
        ITroveManager _troveManager,
        IBorrowerOperations _borrowerOperations,
        IBoldToken _boldToken,
        IWETHPriceFeed _wethPricefeed,
        ISortedTroves _sortedTroves,
        uint128 minInterestRate,
        uint128 maxInterestRate,
        uint128 currentInterestRate,
        uint128 fee,
        uint128 minInterestRateChangePeriod,
        uint128 _discountRate
    ) {
        batchManagerEOA = _batchManagerEOA;
        troveManager = _troveManager;
        borrowerOperations = _borrowerOperations;
        boldToken = _boldToken;
        wethPriceFeed = _wethPricefeed;
        sortedTroves = _sortedTroves;
        discountRate = _discountRate;

        borrowerOperations.registerBatchManager(
            minInterestRate,
            maxInterestRate,
            currentInterestRate,
            fee,
            minInterestRateChangePeriod
        );

        emit Initialized(
            batchManagerEOA,
            address(boldToken),
            address(troveManager),
            address(borrowerOperations),
            address(wethPriceFeed)
        );
    }

    /**
     * @notice Claims discounted BOLD in exchange for Ether.
     * The amount of BOLD is calculated based on the WETH/USD price and the discount rate.
     */
    function claimBOLD() external payable {
        uint256 rate = wethPriceFeed.fetchPrice(); // Fetch the current WETH/USD rate.
        uint256 boldHoldings = boldToken.balanceOf(address(this)); // Check the contract's current BOLD balance.
        uint256 expectedBold = (msg.value * rate) / (1 ether - discountRate); // Calculate the required BOLD amount.

        require(
            boldHoldings + troveManager.getLatestBatchData(address(this)).accruedManagementFee >= expectedBold,
            "Insufficient BOLD for the given Ether."
        );

        if (boldHoldings < expectedBold) {
            (uint256 head, ) = sortedTroves.batches(BatchId.wrap(address(this)));
            borrowerOperations.applyPendingDebt(head, 0, 0);
        }

        boldToken.transfer(msg.sender, expectedBold); // Transfer BOLD to the sender.
    }

    /**
     * @notice Updates the annual interest rate for the batch manager.
     * @param _newAnnualInterestRate New interest rate (e18).
     * @param _upperHint Upper hint for batch placement.
     * @param _lowerHint Lower hint for batch placement.
     * @param _maxUpfrontFee Maximum fee allowed for upfront operations.
     */
    function setNewRate(
        uint128 _newAnnualInterestRate,
        uint256 _upperHint,
        uint256 _lowerHint,
        uint256 _maxUpfrontFee
    ) external onlyBatchManagerEOA {
        borrowerOperations.setBatchManagerAnnualInterestRate(
            _newAnnualInterestRate,
            _upperHint,
            _lowerHint,
            _maxUpfrontFee
        );
    }

    /**
     * @notice Returns the address of the batch manager EOA.
     * @return Address of the batch manager EOA.
     */
    function ManagerEOA() external view returns (address) {
        return batchManagerEOA;
    }
}
