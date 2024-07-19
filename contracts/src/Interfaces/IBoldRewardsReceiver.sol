// SPDX-License-Identifier: MIT

pragma solidity 0.8.20;

interface IBoldRewardsReceiver {
    function triggerBoldRewards(uint256 _boldYield) external;
}
