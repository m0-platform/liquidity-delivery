// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { TypeConverter } from "../../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBookTestBase } from "../OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../../src/interfaces/IOrderBook.sol";

/// @notice Tests demonstrating race condition when admin decreases finalityBuffer
/// @dev If admin decreases finalityBuffer while orders are in CancelRequested state,
///      solvers' in-flight reportFill messages can be front-run by claimRefund
contract FinalityBufferRaceConditionTest is OrderBookTestBase {
    using TypeConverter for *;

    uint32 constant INITIAL_FINALITY_BUFFER = 1 hours;
    uint32 constant REDUCED_FINALITY_BUFFER = 10 minutes;

    function setUp() public override {
        super.setUp();

        // Set a longer finality buffer for testing
        vm.prank(users["admin"]);
        orderBook.setDestinationConfig(DEST_CHAIN_ID, true, INITIAL_FINALITY_BUFFER);
    }

    /// @notice Demonstrates solver loss when admin decreases finalityBuffer mid-flight
    /// @dev Timeline:
    ///      T=0: Order created, user requests cancel (cancelRequestedAt = 0)
    ///      T=0: Solver fills on destination, reportFill in flight (~30min latency)
    ///      T=15min: Admin reduces finalityBuffer from 1hr to 10min
    ///      T=25min: User claims refund (cancelRequestedAt + 10min < now)
    ///      T=30min: Solver's reportFill arrives but reverts - order already Refunded
    ///      Result: Solver paid recipient on dest chain but can't claim tokenIn on origin
    function test_reducedFinalityBuffer_solverLosesFunds() public {
        // Setup: Cross-chain order
        params.destChainId = DEST_CHAIN_ID;

        // T=0: Alice creates order
        bytes32 orderId = _placeOrder(users["alice"], params);

        // T=0: Alice requests cancellation
        vm.prank(users["alice"]);
        orderBook.requestCancelOrder(orderId);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.CancelRequested));
        uint32 cancelRequestedAt = order.cancelRequestedAt;

        // At this point, solver sees the order is still CancelRequested (not Refunded yet)
        // Solver fills on destination chain, reportFill message starts cross-chain journey
        // Cross-chain message takes ~30 minutes to arrive

        // T=15min: Admin reduces finalityBuffer (perhaps for operational reasons)
        vm.warp(block.timestamp + 15 minutes);
        vm.prank(users["admin"]);
        orderBook.setDestinationConfig(DEST_CHAIN_ID, true, REDUCED_FINALITY_BUFFER);

        // T=25min: Alice can now claim refund (cancelRequestedAt + 10min has passed)
        vm.warp(cancelRequestedAt + REDUCED_FINALITY_BUFFER + 1);

        // Alice front-runs the incoming reportFill by claiming refund
        uint256 aliceBalanceBefore = tokenIn.balanceOf(users["alice"]);
        orderBook.claimRefund(orderId);
        uint256 aliceBalanceAfter = tokenIn.balanceOf(users["alice"]);

        // Alice got her tokens back
        assertEq(aliceBalanceAfter - aliceBalanceBefore, params.amountIn, "alice got refund");

        // Order is now Completed (refund sets status to Completed)
        order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Completed));

        // T=30min: Solver's reportFill finally arrives
        vm.warp(block.timestamp + 5 minutes);

        // reportFill reverts because order is no longer Created or CancelRequested
        vm.prank(address(messenger));
        vm.expectRevert(IOrderBook.InvalidOrderStatus.selector);
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32()
            })
        );

        // Result: Solver already paid Alice on destination chain (amountOut)
        // but cannot claim their tokenIn on origin chain
        // Solver loses amountOut worth of funds
    }
}
