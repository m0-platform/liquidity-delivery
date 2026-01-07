// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IAccessControl } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/access/IAccessControl.sol";
import { PausableUpgradeable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";

contract PauseTest is OrderBookTestBase {
    // Test cases
    // [X] given caller does not have PAUSER_ROLE
    //   [X] pause() reverts with AccessControlUnauthorizedAccount
    //   [X] unpause() reverts with AccessControlUnauthorizedAccount
    // [X] given caller has PAUSER_ROLE
    //   [X] pause() sets paused to true and emits Paused event
    //   [X] unpause() sets paused to false and emits Unpaused event

    /* ========== Tests ========== */

    function test_pause_callerWithoutPauserRole_reverts() public {
        bytes32 pauserRole = orderBook.PAUSER_ROLE();

        vm.prank(users["alice"]);
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, users["alice"], pauserRole)
        );
        orderBook.pause();
    }

    function test_unpause_callerWithoutPauserRole_reverts() public {
        bytes32 pauserRole = orderBook.PAUSER_ROLE();

        // First pause the contract
        vm.prank(pauser);
        orderBook.pause();

        // Try to unpause as non-pauser
        vm.prank(users["alice"]);
        vm.expectRevert(
            abi.encodeWithSelector(IAccessControl.AccessControlUnauthorizedAccount.selector, users["alice"], pauserRole)
        );
        orderBook.unpause();
    }

    function test_pause_callerWithPauserRole_success() public {
        assertFalse(orderBook.paused(), "should not be paused initially");

        vm.prank(pauser);
        vm.expectEmit(true, false, false, false);
        emit PausableUpgradeable.Paused(pauser);
        orderBook.pause();

        assertTrue(orderBook.paused(), "should be paused");
    }

    function test_unpause_callerWithPauserRole_success() public {
        // First pause
        vm.prank(pauser);
        orderBook.pause();
        assertTrue(orderBook.paused(), "should be paused");

        // Unpause
        vm.prank(pauser);
        vm.expectEmit(true, false, false, false);
        emit PausableUpgradeable.Unpaused(pauser);
        orderBook.unpause();

        assertFalse(orderBook.paused(), "should not be paused");
    }
}
