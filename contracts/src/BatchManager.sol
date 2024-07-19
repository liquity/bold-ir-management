// SPDX-License-Identifier: GPL-3.0

pragma solidity 0.8.20;

import "./Interfaces/IBoldToken.sol";
import "./Interfaces/ITroveManager.sol";

/**
 * @title Liquity V2 Autonomous Interest Rate Manager
 * @dev Allows for BOLD<>ETH conversions with a discounted rate and the distribution of the collected ether to the corresponding EOA.
 */
contract BatchManager {
    address immutable batchManagerEOA;

    IBorrowerOperations immutable borrowerOperations;
    ITroveManager immutable troveManager;
    IBoldToken immutable boldToken;

    // event for EVM logging
    event initialized(
        address batchManagerEOA,
        address boldToken,
        address troveManager,
        address borrowerOperations
    );

    // modifier to check if caller is the batch manaager
    modifier isBatchManagerEOA() {
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
        IBoldToken _boldToken
    ) {
        batchManagerEOA = _batchManagerEOA;
        troveManager = _troveManager;
        borrowerOperations = _borrowerOperations;
        boldToken = _boldToken;

        emit initialized(
            batchManagerEOA,
            address(boldToken),
            address(troveManager),
            address(borrowerOperations)
        );
    }

    /**
     * @dev Claim discounted BOLD in exchange for Ether
     */
    function claimBOLD() external payable {
        uint256 sentEther = msg.value;
        // check current weth/bold rate

        // check current bold holdings
        uint256 boldHoldings = boldToken.balanceOf(address(this));
        uint256 expectedBold = 1; // TODO: Query the WETH rate from the oracle contract and calculate the expected bold
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
}
