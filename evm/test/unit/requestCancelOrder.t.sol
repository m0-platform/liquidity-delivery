// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { UnitTestBase } from "./UnitTestBase.t.sol";
import { IOrderBook } from "../../src/interfaces/IOrderBook.sol";

contract RequestCancelOrderTest is UnitTestBase {
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
    //   [X] it updates the order status to CancelRequested
    //   [X] it sets the refund requested at timestamp to the current block timestamp
    //   [X] it emits an CancelRequest event

    function setUp() public override {
        super.setUp();

        // open an order for user 0
        _placeOrder(users[0], params);
    }

    function test_givenOrderDoesNotExist_reverts() public {
        bytes32 fakeOrderId = bytes32("fake order id");
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(fakeOrderId);
    }
    
    function test_givenOrderIsAlreadyCancelled_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // cancel the order
        vm.prank(users[0]);
        orderBook.requestCancelOrder(orderId);

        // try to cancel again
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(orderId);
    }

    function test_givenLocalOrderHasBeenFilled_reverts() public {
        // Open a local order for user 0
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users[0], params);

        // Fill the order
        _fillOrder(users[2], orderId, params.amountOut);

        // Try to cancel the order
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(orderId);
    }

    function test_givenXchainOrderHasBeenFilled_reverts() public {
        // Report fill from destination chain
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);
        _reportFill(users[2], orderId, params.amountOut);

        // Try to cancel the order
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrder(orderId);
    }

    function test_givenCallerIsNotOrderSender_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        vm.prank(users[1]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.requestCancelOrder(orderId);
    }


    function test_success() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        vm.warp(block.timestamp + 1 minutes); // make the timestamp non-zero
        vm.prank(users[0]);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.CancelRequested(orderId, uint40(block.timestamp));
        orderBook.requestCancelOrder(orderId);
        
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.CancelRequested), "order status should be CancelRequested");
        assertEq(order.refundRequestedAt, uint40(block.timestamp), "refundRequestedAt should be updated to current block timestamp");
    }
}