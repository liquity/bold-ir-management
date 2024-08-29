// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.0;

import "forge-std/Script.sol";
import "../src/BatchManager.sol";
import "../src/Interfaces/ITroveManager.sol";
import "../src/Interfaces/ISortedTroves.sol";
import "../src/Interfaces/IBoldToken.sol";
import "../src/Interfaces/IBorrowerOperations.sol";
import "../src/Interfaces/IWETHPriceFeed.sol";

contract BatchManagerDeployer is Script {
    function run(
        address strategyEOA,
        ITroveManager troveManager,
        IBorrowerOperations borrowerOperations,
        IBoldToken boldToken,
        IWETHPriceFeed wethPriceFeed,
        ISortedTroves sortedTroves,
        uint128 minInterestRate,
        uint128 maxInterestRate,
        uint128 currentInterestRate,
        uint128 fee,
        uint128 minInterestRateChangePeriod
    ) external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(deployerPrivateKey);

        BatchManager batchManagerContract = new BatchManager(
            strategyEOA,
            troveManager,
            borrowerOperations,
            boldToken,
            wethPriceFeed,
            sortedTroves,
            minInterestRate,
            maxInterestRate,
            currentInterestRate,
            fee,
            minInterestRateChangePeriod,
            0.05 ether // discount rate
        );

        vm.stopBroadcast();
    }
}
