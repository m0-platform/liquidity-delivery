// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";
import { PausableUpgradeable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

contract ReportCancelTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the contract is paused
    //   [X] it completes successfully
    // [X] given the portal is not the caller
    //   [X] it reverts with a NotAuthorized error
    // [X] given the order does not exist
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order is already completed
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order is already cancelled
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the reported order sender is wrong
    //   [X] it reverts with an InvalidReport error
    // [X] given the reported token in is wrong
    //   [X] it reverts with an InvalidReport error
    // [X] given the order is active (Created status)
    //   [X] given no prior fills
    //     [X] given the refund amount exceeds available amount
    //       [X] it reverts with an InvalidReport error
    //     [X] given valid refund amount
    //       [X] it transfers full amountIn to order sender
    //       [X] it sets order status to Cancelled
    //       [X] it emits RefundClaimed event
    //   [X] given partial fills
    //     [X] given the refund amount exceeds available amount
    //       [X] it reverts with an InvalidReport error
    //     [X] given valid refund amount
    //       [X] it transfers remaining amountIn to order sender (amountIn - amountInReleased)
    //       [X] it sets order status to Cancelled
    //       [X] it emits RefundClaimed event

    function setUp() public override {
        super.setUp();

        // open a crosschain order for alice (nonce 0)
        _placeOrder(users["alice"], params);
    }

    function test_portalIsNotCaller_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Try to report cancel as a regular user (not portal)
        vm.prank(users["bob"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
    }

    function test_orderDoesNotExist_reverts() public {
        bytes32 fakeOrderId = bytes32("fake order id");

        // Try to report cancel on non-existent order
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: fakeOrderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
    }

    function test_orderIsCompleted_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report full fill to complete the order
        _reportFill(users["solver"], orderId, params.amountOut, params.amountIn);

        // Try to report cancel on the completed order
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
    }

    function test_orderIsAlreadyCancelled_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report cancel once
        _reportCancel(orderId, users["alice"], params.tokenIn, params.amountIn);

        // Try to report cancel again
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
    }

    function test_orderSenderInvalid_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report with wrong order sender, reverts
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["bob"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
    }

    function test_tokenInInvalid_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report with wrong token in, reverts
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenOut,
                amountInToRefund: params.amountIn
            })
        );
    }

    function test_invalidSourceChainId_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report with wrong source chain ID, reverts
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReportSource.selector));
        orderBook.reportCancel(
            params.destChainId + 1, // should be params.destChainId
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
    }

    function test_activeOrderNoFills_refundExceedsAvailable_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report cancel with refund amount greater than available (amountIn)
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn + 1 // exceeds amountIn
            })
        );
    }

    function _test_activeOrderNoFills_success() internal {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Record balances before
        uint256 senderBalanceBefore = tokenIn.balanceOf(users["alice"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Report cancel via portal
        vm.prank(address(portal));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], order.amountIn);
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: order.amountIn
            })
        );

        // Verify tokenIn transferred to sender
        assertEq(
            tokenIn.balanceOf(users["alice"]),
            senderBalanceBefore + order.amountIn,
            "sender should receive full refund"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookBalanceBefore - order.amountIn,
            "orderBook should release full amount"
        );

        // Verify order status changed to Cancelled
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Cancelled), "order should be Cancelled");
    }

    function test_bothSixDecimals_activeOrderNoFills_success() public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _placeOrder(users["alice"], params);
        _test_activeOrderNoFills_success();
    }

    function test_tokenInSmallerDecimals_activeOrderNoFills_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderNoFills_success();
    }

    function test_tokenInLargerDecimals_activeOrderNoFills_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(6)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderNoFills_success();
    }

    function test_bothEighteenDecimals_activeOrderNoFills_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderNoFills_success();
    }

    function test_activeOrderPartialFills_refundExceedsAvailable_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report partial fill (50%)
        uint128 fillAmount = params.amountOut / 2;
        uint128 amountInReleased = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);
        _reportFill(users["solver"], orderId, fillAmount, amountInReleased);

        // Attempt to report cancel with refund amount greater than available
        uint128 amountInRemaining = params.amountIn - amountInReleased;
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: amountInRemaining + 1 // exceeds remaining amount
            })
        );
    }

    function _test_activeOrderPartialFills_success() internal {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        // Report partial fill (50%)
        uint128 fillAmount = params.amountOut / 2;
        uint128 amountInReleased = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);
        _reportFill(users["solver"], orderId, fillAmount, amountInReleased);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Calculate expected refund
        uint128 expectedRefund = order.amountIn - amountInReleased;

        // Record balances before
        uint256 senderBalanceBefore = tokenIn.balanceOf(users["alice"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Report cancel via portal
        vm.prank(address(portal));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], expectedRefund);
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: expectedRefund
            })
        );

        // Verify tokenIn transferred to sender (remaining amount)
        assertEq(
            tokenIn.balanceOf(users["alice"]),
            senderBalanceBefore + expectedRefund,
            "sender should receive partial refund"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookBalanceBefore - expectedRefund,
            "orderBook should release remaining amount"
        );

        // Verify order status changed to Cancelled
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Cancelled), "order should be Cancelled");
    }

    function test_bothSixDecimals_activeOrderPartialFills_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(6)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFills_success();
    }

    function test_tokenInSmallerDecimals_activeOrderPartialFills_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFills_success();
    }

    function test_tokenInLargerDecimals_activeOrderPartialFills_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(6)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFills_success();
    }

    function test_bothEighteenDecimals_activeOrderPartialFills_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFills_success();
    }

    function test_whenPaused_success() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Pause the contract
        vm.prank(pauser);
        orderBook.pause();

        vm.prank(address(portal));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
    }
}
