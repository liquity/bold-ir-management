// SPDX-License-Identifier: GPL-3.0

pragma solidity 0.8.20;

import "./Interfaces/IBoldToken.sol";

/**
 * @title Liquity V2 Autonomous Interest Rate Manager
 * @dev Allows for BOLD<>ETH conversions with a discounted rate and the distribution of the collected ether to the corresponding EOA.
 */
contract BatchManager {
    address immutable batchManager;
    address immutable boldERC20;
    address immutable troveManager;
    // event for EVM logging
    event initialized(address batchManager, address boldERC20, address troveManager);

    // modifier to check if caller is the batch manaager
    modifier isBatchManager() {
        require(msg.sender == batchManager, "Caller is not batch manager.");
        _;
    }

    /**
     * @dev Set contract deployer as owner
     */
    constructor(
        address batchManagerArg,
        address boldERC20Arg,
        address troveManagerArg
    ) {
        batchManager = batchManagerArg;
        boldERC20 = boldERC20Arg;
        troveManager = troveManagerArg;
        emit initialized(batchManager, boldERC20, troveManager);
    }

    /**
     * @dev Claim discounted BOLD in exchange for Ether
     */
    function claimBOLD() external payable {
        uint256 sentEther = msg.value;
        
    }
}
