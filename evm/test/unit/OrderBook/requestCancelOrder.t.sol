// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

contract RequestCancelOrderTest is OrderBookTestBase {
    // Test cases
    // [X] given the order does not exist
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but already cancelled
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but already filled (local)
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but has already filled (cross-chain)
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the fill deadline has passed
    //   [X] it reverts with an OrderExpired error
    // [X] given the caller is not the order sender
    //   [X] it reverts with an NotAuthorized error
    // [X] given the order can be cancelled
    //   [X] given the destination chain is different to the current chain (i.e. cross-chain order)
    //     [X] it updates the order status to CancelRequested
    //     [X] it sets the refund requested at timestamp to the current block timestamp
    //     [X] it emits an CancelRequest event
    //   [ ] given the destination chain is the current chain (i.e. local order)
    //     [ ] it immediately refunds the order amount in to the order sender
    //     [ ] it sets the order status to Completed
    //     [ ] it emits a CancelRequested event
    //     [ ] it emits a RefundClaimed event

    function setUp() public override {
        super.setUp();

        // open an order for alice
        _placeOrder(users["alice"], params);
    }

    function test_givenOrderDoesNotExist_reverts() public {
        bytes32 fakeOrderId = bytes32("fake order id");
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(fakeOrderId);
    }
    
    function test_givenOrderIsAlreadyCancelled_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // cancel the order
        vm.prank(users["alice"]);
        orderBook.requestCancelOrder(orderId);

        // try to cancel again
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(orderId);
    }

    function test_givenLocalOrderHasBeenFilled_reverts() public {
        // Open a local order for alice
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        // Fill the order
        _fillOrder(users["solver"], orderId, params.amountOut);

        // Try to cancel the order
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(orderId);
    }

    function test_givenCrosschainOrderHasBeenFilled_reverts() public {
        // Report fill from destination chain
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        _reportFill(users["solver"], orderId, params.amountOut, params.amountIn);

        // Try to cancel the order
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(orderId);
    }

    function test_givenCallerIsNotOrderSender_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        vm.prank(users["bob"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.requestCancelOrder(orderId);
    }


    function test_givenCrosschainOrder_success() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        vm.warp(block.timestamp + 1 minutes); // make the timestamp non-zero
        vm.prank(users["alice"]);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.CancelRequested(orderId, uint32(block.timestamp));
        orderBook.requestCancelOrder(orderId);
        
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.CancelRequested), "order status should be CancelRequested");
        assertEq(order.refundRequestedAt, uint32(block.timestamp), "refundRequestedAt should be updated to current block timestamp");
    }

    function test_givenLocalOrder_success() public {
        // Open a local order for alice
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        uint256 aliceStartingBalance = tokenIn.balanceOf(users["alice"]);

        vm.warp(block.timestamp + 1 minutes); // make the timestamp non-zero
        vm.prank(users["alice"]);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.CancelRequested(orderId, uint32(block.timestamp));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], params.amountIn);
        orderBook.requestCancelOrder(orderId);
        
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Completed), "order status should be Completed");
        assertEq(tokenIn.balanceOf(users["alice"]), aliceStartingBalance + params.amountIn, "alice should be refunded the amountIn");
    }
}