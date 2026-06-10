// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { console2 } from "../../lib/forge-std/src/Script.sol";
import { ITransparentUpgradeableProxy } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/transparent/TransparentUpgradeableProxy.sol";
import { ProxyAdmin } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/transparent/ProxyAdmin.sol";

import { SafeProposalBase } from "./SafeProposalBase.s.sol";
import { OrderBook } from "../../src/OrderBook.sol";

/// @title ProposeUpgrade
/// @notice Script to deploy a new OrderBook implementation and propose the upgrade
///         (ProxyAdmin.upgradeAndCall) to the Safe multisig
/// @dev The Safe must own the ProxyAdmin. The new implementation is deployed and broadcast
///      by DEPLOYER_PRIVATE_KEY; only the upgrade call itself goes through the Safe.
///      Usage: forge script script/admin/ProposeUpgrade.s.sol --rpc-url <rpc> --broadcast --ffi
contract ProposeUpgrade is SafeProposalBase {
    function run() external {
        address deployer_ = vm.rememberKey(vm.envUint("DEPLOYER_PRIVATE_KEY"));
        address portal_ = vm.envAddress("PORTAL_ADDRESS");

        // Read existing deployment
        address proxy_ = _readDeployment(block.chainid);
        address proxyAdmin_ = _getProxyAdmin(proxy_);

        require(ProxyAdmin(proxyAdmin_).owner() == _safe(), "ProposeUpgrade: Safe does not own ProxyAdmin");

        console2.log("OrderBook proxy:", proxy_);
        console2.log("ProxyAdmin:", proxyAdmin_);
        console2.log("Deployer:", deployer_);
        console2.log("");

        // Deploy new implementation (regular transaction, not via Safe)
        vm.startBroadcast(deployer_);
        address newImplementation_ = address(new OrderBook(portal_));
        vm.stopBroadcast();

        console2.log("New implementation deployed:", newImplementation_);
        console2.log("");
        console2.log("Proposing ProxyAdmin.upgradeAndCall to the Safe...");
        console2.log("");

        _propose(
            proxyAdmin_,
            abi.encodeCall(
                ProxyAdmin.upgradeAndCall,
                (ITransparentUpgradeableProxy(proxy_), newImplementation_, bytes(""))
            )
        );

        console2.log("");
        console2.log("NOTE: deployments/<chainId>.json is NOT updated by this script.");
        console2.log("After the Safe transaction is executed, update the implementation field to:");
        console2.log("  ", newImplementation_);
    }
}
