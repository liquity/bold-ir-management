// SPDX-License-Identifier: GPL-3.0

pragma solidity 0.8.20;

import "./Interfaces/IBoldToken.sol";
import "./Interfaces/ITroveManager.sol";
import "./Interfaces/IWETHPriceFeed.sol";

/**
 * @title Liquity V2 Autonomous Interest Rate Manager
 * @dev Allows for BOLD<>ETH conversions with a discounted rate and the distribution of the collected ether to the corresponding EOA.
 */
contract BatchManager {
    address immutable batchManagerEOA;

    IBorrowerOperations immutable borrowerOperations;
    ITroveManager immutable troveManager;
    IBoldToken immutable boldToken;
    IWETHPriceFeed immutable wethPriceFeed;

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
        IWETHPriceFeed _wethPricefeed
    ) {
        batchManagerEOA = _batchManagerEOA;
        troveManager = _troveManager;
        borrowerOperations = _borrowerOperations;
        boldToken = _boldToken;
        wethPriceFeed = _wethPricefeed;

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
        uint256 expectedBold = msg.value * rate;

        if (boldHoldings >= expectedBold) {
            // we have enough bold
            boldToken.transfer(msg.sender, expectedBold);
            return;
        }
        uint256 accruedBold = troveManager
            .getLatestBatchData(address(this))
            .accruedManagementFee;

        if (accruedBold + boldHoldings >= expectedBold) {
            borrowerOperations.applyBatchInterestAndFeePermissionless(
                address(this)
            );
            boldToken.transfer(msg.sender, expectedBold);
            return;
        }
    }

    /**
     * @dev Proxy for setting the new rate
     */
    function setNewRate(
        address _batchAddress,
        uint256 _newColl,
        uint256 _newDebt,
        uint256 _newAnnualInterestRate
    ) external onlyBatchManagerEOA {
        troveManager.onSetBatchManagerAnnualInterestRate(
            _batchAddress,
            _newColl,
            _newDebt,
            _newAnnualInterestRate
        );
    }
}
