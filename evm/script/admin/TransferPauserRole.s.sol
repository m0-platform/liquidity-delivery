// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { console2 } from "../../lib/forge-std/src/Script.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { OrderBook } from "../../src/OrderBook.sol";

/// @title TransferPauserRole
/// @notice Script to transfer the PAUSER_ROLE from the current pauser to a new pauser
/// @dev Grants PAUSER_ROLE to a new address and revokes it from the signer in a single broadcast.
///      Idempotent: checks current on-chain state and skips already-completed operations.
///      Usage: forge script script/admin/TransferPauserRole.s.sol --rpc-url <rpc> --broadcast \
///             --sig "run(address)" <newPauser>
contract TransferPauserRole is ScriptBase {
    /// @notice Transfer pauser role to a new address
    /// @param newPauser_ The address to receive the PAUSER_ROLE
    function run(address newPauser_) external {
        require(newPauser_ != address(0), "TransferPauserRole: zero address");

        address signer_ = vm.rememberKey(vm.envUint("ADMIN_PRIVATE_KEY"));

        // Read deployment
        address proxy_ = _readDeployment(block.chainid);

        OrderBook orderBook_ = OrderBook(proxy_);

        // Read current state
        bytes32 pauserRole_ = orderBook_.PAUSER_ROLE();

        bool newPauserHasRole_ = orderBook_.hasRole(pauserRole_, newPauser_);
        bool signerHasRole_ = orderBook_.hasRole(pauserRole_, signer_);

        // Log current state
        console2.log("OrderBook proxy:", proxy_);
        console2.log("New pauser:", newPauser_);
        console2.log("Signer:", signer_);
        console2.log("");
        console2.log("Current state:");
        console2.log("  New pauser has PAUSER_ROLE:", newPauserHasRole_);
        console2.log("  Signer has PAUSER_ROLE:", signerHasRole_);
        console2.log("");

        vm.startBroadcast(signer_);

        // 1. Grant PAUSER_ROLE to new pauser
        if (!newPauserHasRole_) {
            console2.log("Granting PAUSER_ROLE to new pauser...");
            orderBook_.grantRole(pauserRole_, newPauser_);
        } else {
            console2.log("New pauser already has PAUSER_ROLE, skipping");
        }

        // 2. Renounce PAUSER_ROLE from signer
        if (signerHasRole_) {
            console2.log("Renouncing PAUSER_ROLE from signer...");
            orderBook_.renounceRole(pauserRole_, signer_);
        } else {
            console2.log("Signer does not have PAUSER_ROLE, skipping");
        }

        vm.stopBroadcast();

        console2.log("");
        console2.log("Transfer complete!");
    }
}
