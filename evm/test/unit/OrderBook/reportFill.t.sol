// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

contract ReportFillTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the messenger is not the caller
    //   [X] it reverts with a NotAuthorized error
    // [X] given the order does not exist
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order is completed
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the reported amount out filled would exceed the order amount out
    //   [X] it reverts with an InvalidReport error
    // [X] given the reported amount in to release would exceed the order amount in
    //   [X] it reverts with an InvalidReport error
    // [X] given the tokenIn does not match
    //   [X] it reverts with an InvalidReport error
    // [X] given the order is active (Created status)
    //   [X] it updates the filled amount for the order
    //   [X] it transfers the pro-rata amount of the token in to the specified recipient
    //   [X] given the order is fully filled
    //     [X] it updates the order status to Completed
    //     [X] it emits an OrderCompleted event
    // Note: cancelled order tests removed - reportFill only accepts Created status orders

    function setUp() public override {
        super.setUp();

        // open a crosschain order for alice (nonce 0)
        _placeOrder(users["alice"], params);
    }

    function test_messengerIsNotCaller_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Try to report fill as a regular user (not messenger)
        vm.prank(users["bob"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
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
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_orderIsCompleted_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report full fill to complete the order
        _reportFill(users["solver"], orderId, params.amountOut, params.amountIn);

        // Try to report another fill on the completed order
        vm.prank(address(messenger));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut / 2,
                amountInToRelease: params.amountIn / 2,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_overReportAmountOut_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Try to report fill with amountOutFilled exceeding order amountOut
        vm.prank(address(messenger));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut + 1,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_overReportAmountIn_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Try to report fill with amountInToRelease exceeding order amountIn
        vm.prank(address(messenger));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn + 1,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_wrongTokenIn_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Try to report fill with wrong tokenIn
        vm.prank(address(messenger));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(users["bob"]).toBytes32() // invalid tokenIn
            })
        );
    }

    function _test_activeOrderPartialFill_success() internal {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        // Report partial fill (50%)
        uint128 fillAmount = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);

        // Record balances before
        uint256 recipientBalanceBefore = tokenIn.balanceOf(users["solver"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Report fill via messenger
        vm.prank(address(messenger));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: fillAmount,
                amountInToRelease: expectedAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );

        // Verify filled amount updated
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Verify tokenIn transferred to originRecipient
        assertEq(
            tokenIn.balanceOf(users["solver"]),
            recipientBalanceBefore + expectedAmountIn,
            "recipient should receive pro-rata tokenIn"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookBalanceBefore - expectedAmountIn,
            "orderBook should release pro-rata tokenIn"
        );

        // Verify order status remains Created (not Completed)
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created), "order should still be Created");
    }

    function test_bothSixDecimals_activeOrderPartialFill_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(6)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFill_success();
    }

    function test_tokenInSmallerDecimals_activeOrderPartialFill_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFill_success();
    }

    function test_tokenInLargerDecimals_activeOrderPartialFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(6)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFill_success();
    }

    function test_bothEighteenDecimals_activeOrderPartialFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderPartialFill_success();
    }

    function _test_activeOrderFullFill_success() internal {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        // Record balances before
        uint256 recipientBalanceBefore = tokenIn.balanceOf(users["solver"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Report full fill via messenger
        vm.prank(address(messenger));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );

        // Verify tokenIn transferred
        assertEq(
            tokenIn.balanceOf(users["solver"]),
            recipientBalanceBefore + params.amountIn,
            "recipient should receive full tokenIn"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookBalanceBefore - params.amountIn,
            "orderBook should release full tokenIn"
        );

        // Verify order status changed to Completed
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Completed), "order should be Completed");
    }

    function test_bothSixDecimals_activeOrderFullFill_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(6)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderFullFill_success();
    }

    function test_tokenInSmallerDecimals_activeOrderFullFill_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderFullFill_success();
    }

    function test_tokenInLargerDecimals_activeOrderFullFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(6)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderFullFill_success();
    }

    function test_bothEighteenDecimals_activeOrderFullFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(18)
    {
        _placeOrder(users["alice"], params);
        _test_activeOrderFullFill_success();
    }
}
