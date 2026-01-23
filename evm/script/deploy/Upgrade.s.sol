// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { ERC1967Utils } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Utils.sol";
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
        address deployer_ = vm.rememberKey(vm.envUint("DEPLOYER_PRIVATE_KEY"));
        address portal_ = vm.envAddress("PORTAL_ADDRESS");

        // Read existing deployment
        address proxy_ = _readDeployment(block.chainid);

        // Get the ProxyAdmin address from the proxy's ERC-1967 admin slot
        address proxyAdmin_ = _getProxyAdmin(proxy_);

        vm.startBroadcast(deployer_);

        // Deploy new implementation
        address newImplementation_ = address(new OrderBook(portal_));

        // Upgrade via ProxyAdmin (empty data = no initialization call)
        ProxyAdmin(proxyAdmin_).upgradeAndCall(ITransparentUpgradeableProxy(proxy_), newImplementation_, bytes(""));

        vm.stopBroadcast();

        // Update deployment record with new implementation
        _serializeUpgrade(block.chainid, proxy_, newImplementation_, proxyAdmin_);
    }

    /// @notice Upgrade to a specific implementation address (for testing or custom implementations)
    /// @param newImplementation_ The new implementation address to upgrade to
    function run(address newImplementation_) external {
        address deployer_ = vm.rememberKey(vm.envUint("DEPLOYER_PRIVATE_KEY"));

        // Read existing deployment
        address proxy_ = _readDeployment(block.chainid);

        // Get the ProxyAdmin address
        address proxyAdmin_ = _getProxyAdmin(proxy_);

        vm.startBroadcast(deployer_);

        // Upgrade via ProxyAdmin
        ProxyAdmin(proxyAdmin_).upgradeAndCall(ITransparentUpgradeableProxy(proxy_), newImplementation_, bytes(""));

        vm.stopBroadcast();

        // Update deployment record
        _serializeUpgrade(block.chainid, proxy_, newImplementation_, proxyAdmin_);
    }

    /// @notice Get the ProxyAdmin address from the proxy's ERC-1967 admin slot
    /// @param proxy_ The proxy contract address
    /// @return The ProxyAdmin contract address
    function _getProxyAdmin(address proxy_) internal view returns (address) {
        // Read the admin slot (ERC-1967)
        // bytes32 ADMIN_SLOT = 0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103
        bytes32 adminSlot_ = vm.load(proxy_, ERC1967Utils.ADMIN_SLOT);
        return address(uint160(uint256(adminSlot_)));
    }

    /// @notice Serialize the upgrade information to the deployment file
    function _serializeUpgrade(
        uint256 chainId_,
        address proxy_,
        address implementation_,
        address proxyAdmin_
    ) internal {
        string memory root_ = "";
        root_ = vm.serializeAddress(root_, "orderBook", proxy_);
        root_ = vm.serializeAddress(root_, "implementation", implementation_);
        root_ = vm.serializeAddress(root_, "proxyAdmin", proxyAdmin_);
        root_ = vm.serializeUint(root_, "upgradedAt", block.timestamp);
        vm.writeJson(root_, _deployOutputPath(chainId_));
    }
}
