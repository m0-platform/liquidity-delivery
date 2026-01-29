// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { ITransparentUpgradeableProxy } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/transparent/TransparentUpgradeableProxy.sol";
import { ProxyAdmin } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/transparent/ProxyAdmin.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { OrderBook } from "../../src/OrderBook.sol";

/// @title Upgrade
/// @notice Script to upgrade the OrderBook implementation via ProxyAdmin
/// @dev Usage: forge script script/deploy/Upgrade.s.sol --rpc-url <rpc> --broadcast
contract Upgrade is ScriptBase {
    /// @notice Upgrade the OrderBook implementation on the current chain
    /// @dev Reads deployment from deployments/{chainId}.json, deploys new implementation,
    ///      and calls ProxyAdmin.upgradeAndCall
    function run() external {
        address admin_ = vm.rememberKey(vm.envUint("ADMIN_PRIVATE_KEY"));
        address portal_ = vm.envAddress("PORTAL_ADDRESS");

        // Read existing deployment
        address proxy_ = _readDeployment(block.chainid);

        // Get the ProxyAdmin address from the proxy's ERC-1967 admin slot
        address proxyAdmin_ = _getProxyAdmin(proxy_);

        vm.startBroadcast(admin_);

        // Deploy new implementation
        address newImplementation_ = address(new OrderBook(portal_));

        // Upgrade via ProxyAdmin (empty data = no initialization call)
        ProxyAdmin(proxyAdmin_).upgradeAndCall(ITransparentUpgradeableProxy(proxy_), newImplementation_, bytes(""));

        vm.stopBroadcast();

        // Update deployment record with new implementation
        _serializeDeployment(block.chainid, proxy_, newImplementation_, proxyAdmin_);
    }
}
