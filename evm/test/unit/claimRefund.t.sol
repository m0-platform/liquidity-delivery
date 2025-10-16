// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { UnitTestBase } from "./UnitTestBase.t.sol";
import { IOrderBook } from "../../src/interfaces/IOrderBook.sol";
import { TypeConverter } from "../../src/libs/TypeConverter.sol";

contract ClaimRefundTest is UnitTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the order does not exist
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order has been completed
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order has been cancelled
    //   [X] given refund requested timestamp + finality buffer has not passed
    //     [X] it reverts with a FinalityPending error
    //   [X] given refund requested timestamp + finality buffer has passed
    //     [X] given the order has not been filled at all
    //       [X] it transfers the full amount in of the token in from the order book to the order sender
    //     [X] given the order has been partially filled
    //       [X] it transfers the remaining amount in of the token in from the order book to the order sender
    //     [X] it emits a RefundClaimed event
    //     [X] it updates the order status to Completed
    // [X] given the order exists but is not cancelled or filled
    //   [X] given the fill deadline + finality buffer has not passed
    //     [X] it reverts with a FinalityPending error
    //   [X] given the fill deadline + finality buffer has passed
    //     [X] given the order has not been filled at all
    //       [X] it transfers the full amount in of the token in from the order book to the order sender
    //     [X] given the order has been partially filled
    //       [X] it transfers the remaining amount in of the token in from the order book to the order sender
    //     [X] it emits a RefundClaimed event
    //     [X] it updates the order status to Completed
    // [X] given anyone can call claimRefund
    //   [X] it transfers the refund to the original sender

    function setUp() public override {
        super.setUp();

        // open an order for user 0
        _placeOrder(users[0], params);
    }

    function test_givenOrderDoesNotExist_reverts() public {
        bytes32 fakeOrderId = bytes32("fake order id");

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.claimRefund(fakeOrderId);
    }

    function test_givenOrderCompleted_reverts() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Fill it completely
        _reportFill(users[2], orderId, params.amountOut);

        // Try to claim refund
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.claimRefund(orderId);
    }

    function test_givenCancelledOrderFinalityNotPassed_reverts() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Request cancellation
        vm.prank(users[0]);
        orderBook.requestCancelOrder(orderId);

        // Try to claim refund immediately (finality buffer is 10 minutes)
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.FinalityPending.selector));
        orderBook.claimRefund(orderId);
    }

    function test_givenOrderExistsFinalityNotPassed_reverts() public {
        // Get the order
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp to just before fillDeadline + finalityBuffer
        uint40 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer - 1);

        // Try to claim refund
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.FinalityPending.selector));
        orderBook.claimRefund(orderId);
    }

    function test_givenCancelledOrderNoFills_success() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Request cancellation
        vm.prank(users[0]);
        orderBook.requestCancelOrder(orderId);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp past refundRequestedAt + finalityBuffer
        uint40 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.refundRequestedAt + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokens[0].balanceOf(users[0]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users[0], order.amountIn);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokens[0].balanceOf(users[0]), senderBalanceBefore + order.amountIn, "sender should receive full refund");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - order.amountIn, "orderBook should release full amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_givenCancelledOrderPartialFills_success() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Report the order is partially filled (50%) 
        uint128 fillAmount = params.amountOut / 2;
        _reportFill(users[2], orderId, fillAmount);

        // Request cancellation
        vm.prank(users[0]);
        orderBook.requestCancelOrder(orderId);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Calculate expected refund (pro-rata)
        uint128 expectedRefund = uint128((uint256(params.amountIn) * (params.amountOut - fillAmount)) / params.amountOut);

        // Warp past refundRequestedAt + finalityBuffer
        uint40 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.refundRequestedAt + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokens[0].balanceOf(users[0]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users[0], expectedRefund);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokens[0].balanceOf(users[0]), senderBalanceBefore + expectedRefund, "sender should receive partial refund");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - expectedRefund, "orderBook should release partial amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_givenOrderExistsNoFills_success() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp past fillDeadline + finalityBuffer
        uint40 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokens[0].balanceOf(users[0]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users[0], order.amountIn);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokens[0].balanceOf(users[0]), senderBalanceBefore + order.amountIn, "sender should receive full refund");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - order.amountIn, "orderBook should release full amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_givenOrderExistsPartialFills_success() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Report the order is partially filled (50%)
        uint128 fillAmount = params.amountOut / 2;
        _reportFill(users[2], orderId, fillAmount);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Calculate expected refund (pro-rata)
        uint128 expectedRefund = uint128((uint256(params.amountIn) * (params.amountOut - fillAmount)) / params.amountOut);

        // Warp past fillDeadline + finalityBuffer
        uint40 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokens[0].balanceOf(users[0]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users[0], expectedRefund);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokens[0].balanceOf(users[0]), senderBalanceBefore + expectedRefund, "sender should receive partial refund");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - expectedRefund, "orderBook should release partial amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_claimRefundCanBeCalledByAnyone_success(address caller) public {
        vm.assume(caller != address(orderBook));
        vm.deal(caller, 1 ether); // ensure caller has some ETH in case it's needed for gas

        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp past fillDeadline + finalityBuffer
        uint40 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokens[0].balanceOf(users[0]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Claim refund
        vm.prank(caller);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users[0], order.amountIn);
        orderBook.claimRefund(orderId);

        // Verify refund still goes to original sender (users[0])
        assertEq(tokens[0].balanceOf(users[0]), senderBalanceBefore + order.amountIn, "sender should receive full refund");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - order.amountIn, "orderBook should release full amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }
}