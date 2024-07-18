// SPDX-License-Identifier: GPL-3.0

pragma solidity 0.8.26;

/**
 * @title Liquity V2 Autonomous Interest Rate Manager
 * @dev Allows for BOLD<>ETH conversions with a discounted rate and the distribution of the collected ether to the corresponding EOA.
 */
contract IrManager {

    address immutable batchManager;

    // event for EVM logging
    event batchManagerSet(address indexed batchManager);

    // modifier to check if caller is the batch manaager
    modifier isBatchManager() {
        require(msg.sender == batchManager, "Caller is not batch manager.");
        _;
    }

    /**
     * @dev Set contract deployer as owner
     */
    constructor(address batchManagerAddress) {
        batchManager = batchManagerAddress;
        emit batchManagerSet(batchManager);
    }

    /**
     * @dev Claim discounted BOLD in exchange for Ether
     */
    function claimBOLD() external {
        uint256 sentEther = msg.value;
    }
} 