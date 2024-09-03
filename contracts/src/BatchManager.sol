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
 * @dev Allows for BOLD<>ETH conversions with a discounted rate and the distribution of the collected ether to the corresponding EOA.
 */
contract BatchManager {
    uint256 immutable discountRate;
    address immutable batchManagerEOA;

    IBorrowerOperations immutable borrowerOperations;
    ITroveManager immutable troveManager;
    IBoldToken immutable boldToken;
    IWETHPriceFeed immutable wethPriceFeed;
    ISortedTroves immutable sortedTroves;

    // event for EVM logging
    event initialized(
        address batchManagerEOA,
        address boldToken,
        address troveManager,
        address borrowerOperations,
        address wethPriceFeed
    );

    // modifier to check if caller is the batch manaager
    modifier onlyBatchManagerEOA() {
        require(msg.sender == batchManagerEOA, "Caller is not batch manager.");
        _;
    }

    /**
     * @dev Set contract deployer as owner
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

        // The contract needs to register itself as a batch manager
        borrowerOperations.registerBatchManager(
            minInterestRate,
            maxInterestRate,
            currentInterestRate,
            fee,
            minInterestRateChangePeriod
        );

        emit initialized(
            batchManagerEOA,
            address(boldToken),
            address(troveManager),
            address(borrowerOperations),
            address(wethPriceFeed)
        );
    }

    /**
     * @dev Claim discounted BOLD in exchange for Ether
     */
    function claimBOLD() external payable {
        // check current weth/usd rate
        uint256 rate = wethPriceFeed.fetchPrice();

        // check current bold holdings
        uint256 boldHoldings = boldToken.balanceOf(address(this));
        uint256 expectedBold = (msg.value * rate) / (1 ether - discountRate);

        if (boldHoldings >= expectedBold) {
            // we have enough bold
            boldToken.transfer(msg.sender, expectedBold);
            return;
        }
        
        uint256 accruedBold = troveManager
            .getLatestBatchData(address(this))
            .accruedManagementFee;
        
        require(accruedBold + boldHoldings >= expectedBold, "The contract doesn't have enough BOLD for this amount of Ether.");
        
        if (accruedBold + boldHoldings >= expectedBold) {
            (uint256 head, ) = sortedTroves.batches(
                BatchId.wrap(address(this))
            );
            borrowerOperations.applyPendingDebt(head, 0, 0);
            boldToken.transfer(msg.sender, expectedBold);
            return;
        }
    }

    /**
     * @dev Proxy for setting the new rate
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
     * @dev Returns the batch manager EOA
     */
    function ManagerEOA() external view returns (address) {
        return batchManagerEOA;
    }
}
