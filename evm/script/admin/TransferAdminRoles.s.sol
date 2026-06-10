// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { console2 } from "../../lib/forge-std/src/Script.sol";
import { ProxyAdmin } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/transparent/ProxyAdmin.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { OrderBook } from "../../src/OrderBook.sol";

/// @title TransferAdminRoles
/// @notice Script to transfer admin roles from the current admin to a new admin
/// @dev Transfers ProxyAdmin ownership and DEFAULT_ADMIN_ROLE in a single broadcast.
///      Does NOT transfer PAUSER_ROLE — use TransferPauserRole.s.sol for that.
///      Idempotent: checks current on-chain state and skips already-completed operations.
///      Usage: forge script script/admin/TransferAdminRoles.s.sol --rpc-url <rpc> --broadcast \
///             --sig "run(address)" <newAdmin>
contract TransferAdminRoles is ScriptBase {
    /// @notice Transfer admin roles to a new admin address
    /// @param newAdmin_ The address to receive admin roles
    function run(address newAdmin_) external {
        require(newAdmin_ != address(0), "TransferAdminRoles: zero address");

        address signer_ = vm.rememberKey(vm.envUint("ADMIN_PRIVATE_KEY"));

        // Read deployment
        address proxy_ = _readDeployment(block.chainid);
        address proxyAdmin_ = _getProxyAdmin(proxy_);

        OrderBook orderBook_ = OrderBook(proxy_);
        ProxyAdmin proxyAdminContract_ = ProxyAdmin(proxyAdmin_);

        // Read current state
        bytes32 defaultAdminRole_ = orderBook_.DEFAULT_ADMIN_ROLE();

        bool newAdminHasAdmin_ = orderBook_.hasRole(defaultAdminRole_, newAdmin_);
        bool signerHasAdmin_ = orderBook_.hasRole(defaultAdminRole_, signer_);
        address currentProxyOwner_ = proxyAdminContract_.owner();

        // Log current state
        console2.log("OrderBook proxy:", proxy_);
        console2.log("ProxyAdmin:", proxyAdmin_);
        console2.log("New admin:", newAdmin_);
        console2.log("Signer:", signer_);
        console2.log("");
        console2.log("Current state:");
        console2.log("  New admin has DEFAULT_ADMIN_ROLE:", newAdminHasAdmin_);
        console2.log("  Signer has DEFAULT_ADMIN_ROLE:", signerHasAdmin_);
        console2.log("  ProxyAdmin owner:", currentProxyOwner_);
        console2.log("");

        vm.startBroadcast(signer_);

        // 1. Grant DEFAULT_ADMIN_ROLE to new admin (MUST happen before renouncing)
        if (!newAdminHasAdmin_) {
            console2.log("Granting DEFAULT_ADMIN_ROLE to new admin...");
            orderBook_.grantRole(defaultAdminRole_, newAdmin_);
        } else {
            console2.log("New admin already has DEFAULT_ADMIN_ROLE, skipping");
        }

        // 2. Renounce DEFAULT_ADMIN_ROLE from signer (MUST be last role operation)
        if (signerHasAdmin_) {
            console2.log("Renouncing DEFAULT_ADMIN_ROLE from signer...");
            orderBook_.renounceRole(defaultAdminRole_, signer_);
        } else {
            console2.log("Signer does not have DEFAULT_ADMIN_ROLE, skipping");
        }

        // 3. Transfer ProxyAdmin ownership to new admin
        if (currentProxyOwner_ == signer_) {
            console2.log("Transferring ProxyAdmin ownership to new admin...");
            proxyAdminContract_.transferOwnership(newAdmin_);
        } else if (currentProxyOwner_ == newAdmin_) {
            console2.log("ProxyAdmin already owned by new admin, skipping");
        } else {
            console2.log("WARNING: ProxyAdmin owned by unexpected address:", currentProxyOwner_);
        }

        vm.stopBroadcast();

        console2.log("");
        console2.log("Transfer complete!");
    }
}
