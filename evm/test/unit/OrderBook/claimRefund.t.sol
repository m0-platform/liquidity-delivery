// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";
import { TypeConverter } from "../../../src/libs/TypeConverter.sol";

contract ClaimRefundTest is OrderBookTestBase {
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

        // open an order for alice
        _placeOrder(users["alice"], params);
    }

    function test_givenOrderDoesNotExist_reverts() public {
        bytes32 fakeOrderId = bytes32("fake order id");

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.claimRefund(fakeOrderId);
    }

    function test_givenOrderCompleted_reverts() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Fill it completely
        _reportFill(users["solver"], orderId, params.amountOut, params.amountIn);

        // Try to claim refund
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.claimRefund(orderId);
    }

    function test_givenCancelledOrderFinalityNotPassed_reverts() public {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Request cancellation
        vm.prank(users["alice"]);
        orderBook.requestCancelOrder(orderId);

        // Try to claim refund immediately (finality buffer is 10 minutes)
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.FinalityPending.selector));
        orderBook.claimRefund(orderId);
    }

    function test_givenOrderExistsFinalityNotPassed_reverts() public {
        // Get the order
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp to just before fillDeadline + finalityBuffer
        uint32 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer - 1);

        // Try to claim refund
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.FinalityPending.selector));
        orderBook.claimRefund(orderId);
    }

    function _test_givenCancelledOrderNoFills_success() internal {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        // Request cancellation
        vm.prank(users["alice"]);
        orderBook.requestCancelOrder(orderId);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp past cancelRequestedAt + finalityBuffer
        uint32 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.cancelRequestedAt + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokenIn.balanceOf(users["alice"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], order.amountIn);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokenIn.balanceOf(users["alice"]), senderBalanceBefore + order.amountIn, "sender should receive full refund");
        assertEq(tokenIn.balanceOf(address(orderBook)), orderBookBalanceBefore - order.amountIn, "orderBook should release full amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_bothSixDecimals_givenCancelledOrderNoFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderNoFills_success();
    }

    function test_tokenInSmallerDecimals_givenCancelledOrderNoFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderNoFills_success();
    }

    function test_tokenInLargerDecimals_givenCancelledOrderNoFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderNoFills_success();
    }

    function test_bothEighteenDecimals_givenCancelledOrderNoFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderNoFills_success();
    }

    function _test_givenCancelledOrderPartialFills_success() internal {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        // Report the order is partially filled (50%) 
        uint128 fillAmount = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);
        _reportFill(users["solver"], orderId, fillAmount, expectedAmountIn);

        // Request cancellation
        vm.prank(users["alice"]);
        orderBook.requestCancelOrder(orderId);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Calculate expected refund (pro-rata)
        uint128 expectedRefund = uint128((uint256(params.amountIn) * (params.amountOut - fillAmount)) / params.amountOut);

        // Warp past cancelRequestedAt + finalityBuffer
        uint32 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.cancelRequestedAt + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokenIn.balanceOf(users["alice"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], expectedRefund);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokenIn.balanceOf(users["alice"]), senderBalanceBefore + expectedRefund, "sender should receive partial refund");
        assertEq(tokenIn.balanceOf(address(orderBook)), orderBookBalanceBefore - expectedRefund, "orderBook should release partial amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_bothSixDecimals_givenCancelledOrderPartialFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderPartialFills_success();
    }

    function test_tokenInSmallerDecimals_givenCancelledOrderPartialFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderPartialFills_success();
    }

    function test_tokenInLargerDecimals_givenCancelledOrderPartialFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderPartialFills_success();
    }

    function test_bothEighteenDecimals_givenCancelledOrderPartialFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenCancelledOrderPartialFills_success();
    }

    function _test_givenOrderExistsNoFills_success() internal {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp past fillDeadline + finalityBuffer
        uint32 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokenIn.balanceOf(users["alice"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], order.amountIn);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokenIn.balanceOf(users["alice"]), senderBalanceBefore + order.amountIn, "sender should receive full refund");
        assertEq(tokenIn.balanceOf(address(orderBook)), orderBookBalanceBefore - order.amountIn, "orderBook should release full amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_bothSixDecimals_givenOrderExistsNoFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsNoFills_success();
    }

    function test_tokenInSmallerDecimals_givenOrderExistsNoFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsNoFills_success();
    }

    function test_tokenInLargerDecimals_givenOrderExistsNoFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsNoFills_success();
    }

    function test_bothEighteenDecimals_givenOrderExistsNoFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsNoFills_success();
    }

    function _test_givenOrderExistsPartialFills_success() internal {
        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        // Report the order is partially filled (50%)
        uint128 fillAmount = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);
        _reportFill(users["solver"], orderId, fillAmount, expectedAmountIn);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Calculate expected refund (pro-rata)
        uint128 expectedRefund = uint128((uint256(params.amountIn) * (params.amountOut - fillAmount)) / params.amountOut);

        // Warp past fillDeadline + finalityBuffer
        uint32 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokenIn.balanceOf(users["alice"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Claim refund
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], expectedRefund);
        orderBook.claimRefund(orderId);

        // Verify refund
        assertEq(tokenIn.balanceOf(users["alice"]), senderBalanceBefore + expectedRefund, "sender should receive partial refund");
        assertEq(tokenIn.balanceOf(address(orderBook)), orderBookBalanceBefore - expectedRefund, "orderBook should release partial amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_bothSixDecimals_givenOrderExistsPartialFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsPartialFills_success();
    }

    function test_tokenInSmallerDecimals_givenOrderExistsPartialFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsPartialFills_success();
    }

    function test_tokenInLargerDecimals_givenOrderExistsPartialFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsPartialFills_success();
    }

    function test_bothEighteenDecimals_givenOrderExistsPartialFills_success() public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _placeOrder(users["alice"], params);
        _test_givenOrderExistsPartialFills_success();
    }

    function test_claimRefundCanBeCalledByAnyone_success(address caller) public {
        vm.assume(caller != address(orderBook));
        vm.deal(caller, 1 ether); // ensure caller has some ETH in case it's needed for gas

        // Get the order ID
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp past fillDeadline + finalityBuffer
        uint32 finalityBuffer = orderBook.getDestinationFinalityBuffer(order.destChainId);
        vm.warp(order.fillDeadline + finalityBuffer + 1);

        // Record balances
        uint256 senderBalanceBefore = tokenIn.balanceOf(users["alice"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Claim refund
        vm.prank(caller);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], order.amountIn);
        orderBook.claimRefund(orderId);

        // Verify refund still goes to original sender (users["alice"])
        assertEq(tokenIn.balanceOf(users["alice"]), senderBalanceBefore + order.amountIn, "sender should receive full refund");
        assertEq(tokenIn.balanceOf(address(orderBook)), orderBookBalanceBefore - order.amountIn, "orderBook should release full amount");

        // Verify order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }
}