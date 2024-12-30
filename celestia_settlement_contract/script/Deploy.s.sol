// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "forge-std/Script.sol";
import "../src/ZKRollupSettlement.sol";

contract DeployScript is Script {
    function run() external {
        uint256 deployerPrivateKey = vm.envUint("PRIVATE_KEY");
        vm.startBroadcast(deployerPrivateKey);

        // Deploy mock contracts first
        MockDAOracle blobstream = new MockDAOracle();
        MockZKVerifier verifier = new MockZKVerifier();

        // Deploy main contract
        ZKRollupSettlement zkRollup = new ZKRollupSettlement(
            address(blobstream),
            address(verifier)
        );

        vm.stopBroadcast();
    }
}