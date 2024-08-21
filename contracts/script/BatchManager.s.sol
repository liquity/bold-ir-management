// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

import "forge-std/Script.sol";
import "../src/BatchManager.sol";
import "../src/Interfaces/ITroveManager.sol";
import "../src/Interfaces/IBoldToken.sol";
import "../src/Interfaces/IBorrowerOperations.sol";
import "../src/Interfaces/IWETHPriceFeed.sol";

contract BatchManagerDeployer is Script {
    function run(address strategyEOA, ITroveManager troveManager, IBorrowerOperations borrowerOperations, IBoldToken boldToken, IWETHPriceFeed wethPriceFeed) external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(deployerPrivateKey);

        BatchManager batchManagerContract = new BatchManager(strategyEOA, troveManager, borrowerOperations, boldToken, wethPriceFeed);

        vm.stopBroadcast();
    }
}
