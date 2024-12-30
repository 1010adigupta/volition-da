// SPDX-License-Identifier: MIT
pragma solidity ^0.8.22;

import "forge-std/Script.sol";
import "../src/L1CelestiaVerification.sol";

contract DeployScript is Script {
    function run() external {
        // Read private key from environment variable
        uint256 deployerPrivateKey = 0x8167e51f2c57e08b6eabb2ab84a39169527289292d26f62310ee0572d519f97e;
        
        // Get the Blobstream Oracle address from environment
        address blobstreamAddress = 0xF0c6429ebAB2e7DC6e05DaFB61128bE21f13cb1e;
        
        vm.startBroadcast(deployerPrivateKey);

        // Deploy ZKRollupSettlement
        ZKRollupSettlement zkRollup = new ZKRollupSettlement(blobstreamAddress);
        
        console.log("ZKRollupSettlement deployed to:", address(zkRollup));
        console.log("Blobstream Oracle:", blobstreamAddress);

        vm.stopBroadcast();
    }
}