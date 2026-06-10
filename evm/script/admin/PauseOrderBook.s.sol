// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { console2 } from "../../lib/forge-std/src/Script.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { OrderBook } from "../../src/OrderBook.sol";

/// @title PauseOrderBook
/// @notice Script to pause or unpause the OrderBook using the pauser key
/// @dev Broadcasts from PAUSER_PRIVATE_KEY, which must match PAUSER_ADDRESS and hold PAUSER_ROLE.
///      Idempotent: checks current on-chain state and skips if already in the desired state.
///      Usage: forge script script/admin/PauseOrderBook.s.sol --rpc-url <rpc> --broadcast \
///             --sig "pause()" (or --sig "unpause()")
contract PauseOrderBook is ScriptBase {
    /// @notice Pause the OrderBook
    function pause() external {
        _setPaused(true);
    }

    /// @notice Unpause the OrderBook
    function unpause() external {
        _setPaused(false);
    }

    function _setPaused(bool paused_) internal {
        address signer_ = vm.rememberKey(vm.envUint("PAUSER_PRIVATE_KEY"));
        address pauser_ = vm.envAddress("PAUSER_ADDRESS");

        require(signer_ == pauser_, "PauseOrderBook: signer does not match PAUSER_ADDRESS");

        // Read deployment
        address proxy_ = _readDeployment(block.chainid);

        OrderBook orderBook_ = OrderBook(proxy_);

        // Read current state
        bool signerHasRole_ = orderBook_.hasRole(orderBook_.PAUSER_ROLE(), signer_);
        bool isPaused_ = orderBook_.paused();

        // Log current state
        console2.log("OrderBook proxy:", proxy_);
        console2.log("Pauser:", signer_);
        console2.log("");
        console2.log("Current state:");
        console2.log("  Pauser has PAUSER_ROLE:", signerHasRole_);
        console2.log("  OrderBook paused:", isPaused_);
        console2.log("");

        require(signerHasRole_, "PauseOrderBook: pauser does not have PAUSER_ROLE");

        if (isPaused_ == paused_) {
            console2.log("OrderBook already in desired state, skipping");
            return;
        }

        vm.startBroadcast(signer_);

        if (paused_) {
            console2.log("Pausing OrderBook...");
            orderBook_.pause();
        } else {
            console2.log("Unpausing OrderBook...");
            orderBook_.unpause();
        }

        vm.stopBroadcast();

        console2.log("");
        console2.log(paused_ ? "OrderBook paused!" : "OrderBook unpaused!");
    }
}
