// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";
import { TypeConverter } from "../../../src/libs/TypeConverter.sol";

contract ReportFillTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the messenger is not the caller
    //   [X] it reverts with a NotAuthorized error
    // [X] given the order does not exist
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order is completed
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order is active or cancelled
    //   [X] it updates the filled amount for the order
    //   [X] it transfers the pro-rata amount of the token in to the specified recipient
    //   [X] given the order is fully filled
    //     [X] it updates the order status to Completed
    //     [X] it emits an OrderCompleted event

    function setUp() public override {
        super.setUp();

        // open a crosschain order for user 0
        _placeOrder(users[0], params);
    }

    function test_messengerIsNotCaller_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Try to report fill as a regular user (not messenger)
        vm.prank(users[1]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_orderDoesNotExist_reverts() public {
        bytes32 fakeOrderId = bytes32("fake order id");

        // Try to report fill on non-existent order
        vm.prank(address(messenger));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: fakeOrderId,
                amountOutFilled: params.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_orderIsCompleted_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Report full fill to complete the order
        _reportFill(users[2], orderId, params.amountOut);

        // Try to report another fill on the completed order
        vm.prank(address(messenger));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut / 2,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_activeOrderPartialFill_success() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Report partial fill (50%)
        uint128 fillAmount = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);

        // Record balances before
        uint256 recipientBalanceBefore = tokens[0].balanceOf(users[2]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Report fill via messenger
        vm.prank(address(messenger));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: fillAmount,
                originRecipient: users[2].toBytes32()
            })
        );

        // Verify filled amount updated
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Verify tokenIn transferred to originRecipient
        assertEq(tokens[0].balanceOf(users[2]), recipientBalanceBefore + expectedAmountIn, "recipient should receive pro-rata tokenIn");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - expectedAmountIn, "orderBook should release pro-rata tokenIn");

        // Verify order status remains Created (not Completed)
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created), "order should still be Created");
    }

    function test_activeOrderFullFill_success() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Record balances before
        uint256 recipientBalanceBefore = tokens[0].balanceOf(users[2]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Report full fill via messenger
        vm.prank(address(messenger));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );

        // Verify tokenIn transferred
        assertEq(tokens[0].balanceOf(users[2]), recipientBalanceBefore + params.amountIn, "recipient should receive full tokenIn");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - params.amountIn, "orderBook should release full tokenIn");

        // Verify order status changed to Completed
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Completed), "order should be Completed");
    }

    function test_cancelledOrderPartialFill_success() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Request cancellation
        vm.prank(users[0]);
        orderBook.requestCancelOrder(orderId);

        // Report partial fill (50%)
        uint128 fillAmount = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);

        // Record balances before
        uint256 recipientBalanceBefore = tokens[0].balanceOf(users[2]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Report fill via messenger
        vm.prank(address(messenger));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: fillAmount,
                originRecipient: users[2].toBytes32()
            })
        );

        // Verify tokenIn transferred
        assertEq(tokens[0].balanceOf(users[2]), recipientBalanceBefore + expectedAmountIn, "recipient should receive pro-rata tokenIn");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - expectedAmountIn, "orderBook should release pro-rata tokenIn");

        // Verify order status remains CancelRequested (not Completed)
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.CancelRequested), "order should still be CancelRequested");
    }

    function test_cancelledOrderFullFill_success() public {
        bytes32 orderId = _getOrderIdFromParams(users[0], 0, params);

        // Request cancellation
        vm.prank(users[0]);
        orderBook.requestCancelOrder(orderId);

        // Record balances before
        uint256 recipientBalanceBefore = tokens[0].balanceOf(users[2]);
        uint256 orderBookBalanceBefore = tokens[0].balanceOf(address(orderBook));

        // Report full fill via messenger
        vm.prank(address(messenger));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );

        // Verify tokenIn transferred
        assertEq(tokens[0].balanceOf(users[2]), recipientBalanceBefore + params.amountIn, "recipient should receive full tokenIn");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookBalanceBefore - params.amountIn, "orderBook should release full tokenIn");

        // Verify order status changed to Completed
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Completed), "order should be Completed");
    }
}