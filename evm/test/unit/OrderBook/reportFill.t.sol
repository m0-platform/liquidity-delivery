// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";
import { PausableUpgradeable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

contract ReportFillTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the contract is paused
    //   [X] it completes successfully
    // [X] given the portal is not the caller
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
    // [x] given the source chain ID does not match the order's dest chain ID
    //   [x] it reverts with an InvalidReportSource error
    // [X] given the order is active (Created status)
    //   [X] it updates the filled amount for the order
    //   [X] it transfers the pro-rata amount of the token in to the specified recipient
    //   [X] given the order is fully filled
    //     [X] it updates the order status to Completed
    //     [X] it emits an OrderCompleted event
    // [X] given the reported amount in does not match the expected pro-rata amount
    //   [X] it reverts with an InvalidReport error (too high)
    //   [X] it reverts with an InvalidReport error (too low)
    // [X] given there was a prior partial fill
    //   [X] given the reported amount out would exceed the remaining unfilled amount
    //     [X] it reverts with an InvalidReport error
    //   [X] given correct amounts are reported for a second partial fill
    //     [X] it updates filled amounts and transfers tokens
    //   [X] given the order is now fully filled after a partial fill
    //     [X] it marks the order as Completed

    function setUp() public override {
        super.setUp();

        // open a crosschain order for alice (nonce 0)
        _placeOrder(users["alice"], params);
    }

    function test_whenPaused_success() public {
        vm.prank(pauser);
        orderBook.pause();

        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        vm.prank(address(portal));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_portalIsNotCaller_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Try to report fill as a regular user (not portal)
        vm.prank(users["bob"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.reportFill(
            params.destChainId,
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
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportFill(
            params.destChainId,
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
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.reportFill(
            params.destChainId,
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
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            params.destChainId,
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
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            params.destChainId,
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
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(users["bob"]).toBytes32() // invalid tokenIn
            })
        );
    }

    function test_sourceChainIdMismatch_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Try to report fill with wrong source chain ID
        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReportSource.selector));
        orderBook.reportFill(
            params.destChainId + 1, // incorrect source chain ID
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: params.amountOut,
                amountInToRelease: params.amountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_cancelledOrder_partialFill_success() public {
        // Here we simulate a situation where an order is partially filled on the destination chain,
        // and then cancelled on the destination chain (triggering a partial refund on origin).
        // Because crosschain messages do not have to be delivered in order, we deliver
        // the cancel report first, then the fill report to check that the fill report still works.
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // 1. Report cancel first that refunds half the order
        uint128 expectedRefund = params.amountIn / 2;

        uint256 aliceBalanceBefore = tokenIn.balanceOf(users["alice"]);

        vm.prank(address(portal));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: expectedRefund
            })
        );

        // Verify the order status is Cancelled and alice got refunded half
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Cancelled), "order should be Cancelled");
        uint256 aliceBalanceAfter = tokenIn.balanceOf(users["alice"]);
        assertEq(
            aliceBalanceAfter - aliceBalanceBefore,
            expectedRefund,
            "alice should receive partial refund on cancel"
        );

        // 2. Now report fill for the remaining half (which would have been filled before cancel on the destination)
        uint128 fillAmountOut = params.amountOut / 2;
        uint128 fillAmountIn = params.amountIn / 2;

        uint256 solverBalanceBefore = tokenIn.balanceOf(users["solver"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        vm.prank(address(portal));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: fillAmountOut,
                amountInToRelease: fillAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );

        // Verify the solver received the filled amountIn
        uint256 solverBalanceAfter = tokenIn.balanceOf(users["solver"]);
        assertEq(solverBalanceAfter - solverBalanceBefore, fillAmountIn, "solver should receive filled amountIn");
        // Verify the orderBook released the filled amountIn
        uint256 orderBookBalanceAfter = tokenIn.balanceOf(address(orderBook));
        assertEq(
            orderBookBalanceBefore - orderBookBalanceAfter,
            fillAmountIn,
            "orderBook should release filled amountIn"
        );

        // Verify order status remains Cancelled
        order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Cancelled), "order should still be Cancelled");
    }

    function test_cancelledOrder_fillAmountInExceedsAvailable_revert() public {
        // Here we simulate a situation where an order is cancelled first on the destination and receives a full refund.
        // Then, an invalid fill report arrives that tries to release more amountIn than is available (should revert).
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // 1. Report cancel first that refunds the full order
        uint128 expectedRefund = params.amountIn;

        vm.prank(address(portal));
        orderBook.reportCancel(
            params.destChainId,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: users["alice"].toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: expectedRefund
            })
        );

        // Verify the order status is Cancelled
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Cancelled), "order should be Cancelled");

        // 2. Now report fill that tries to release more than available (should revert)
        uint128 fillAmountOut = 1;
        uint128 fillAmountIn = 1; // exceed available

        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: fillAmountOut,
                amountInToRelease: fillAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_partiallyFilledOrder_overfillRemaining_reverts() public {
        // Setup: Place order and report a 50% partial fill
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        uint128 firstFillAmountOut = params.amountOut / 2;
        uint128 firstFillAmountIn = uint128((uint256(params.amountIn) * firstFillAmountOut) / params.amountOut);

        _reportFill(users["solver"], orderId, firstFillAmountOut, firstFillAmountIn);

        // Try to report another fill that exceeds the remaining 50%
        uint128 remainingAmountOut = params.amountOut - firstFillAmountOut;
        uint128 overfillAmountOut = remainingAmountOut + 1;
        uint128 overfillAmountIn = uint128((uint256(params.amountIn) * overfillAmountOut) / params.amountOut);

        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: overfillAmountOut,
                amountInToRelease: overfillAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_wrongAmountInRatio_tooHigh_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report partial fill with correct amountOut but amountIn too high
        uint128 fillAmountOut = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmountOut) / params.amountOut);
        uint128 wrongAmountIn = expectedAmountIn + 1; // Too high

        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: fillAmountOut,
                amountInToRelease: wrongAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_wrongAmountInRatio_tooLow_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Report partial fill with correct amountOut but amountIn too low
        uint128 fillAmountOut = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmountOut) / params.amountOut);
        uint128 wrongAmountIn = expectedAmountIn - 1; // Too low

        vm.prank(address(portal));
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidReport.selector));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: fillAmountOut,
                amountInToRelease: wrongAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function test_partiallyFilledOrder_secondPartialFill_success() public {
        // Setup: Place order and report a 25% partial fill
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        uint128 firstFillAmountOut = params.amountOut / 4;
        uint128 firstFillAmountIn = uint128((uint256(params.amountIn) * firstFillAmountOut) / params.amountOut);

        _reportFill(users["solver"], orderId, firstFillAmountOut, firstFillAmountIn);

        // Report another 25% partial fill with correct amounts
        uint128 secondFillAmountOut = params.amountOut / 4;
        uint128 secondFillAmountIn = uint128((uint256(params.amountIn) * secondFillAmountOut) / params.amountOut);

        uint256 solverBalanceBefore = tokenIn.balanceOf(users["solver"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        vm.prank(address(portal));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: secondFillAmountOut,
                amountInToRelease: secondFillAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );

        // Verify balances updated correctly
        assertEq(
            tokenIn.balanceOf(users["solver"]),
            solverBalanceBefore + secondFillAmountIn,
            "solver should receive second fill amountIn"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookBalanceBefore - secondFillAmountIn,
            "orderBook should release second fill amountIn"
        );

        // Verify order is still Created (not fully filled yet)
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created), "order should still be Created");
    }

    function test_partiallyFilledOrder_fullFillRemaining_success() public {
        // Setup: Place order and report a 50% partial fill
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        uint128 firstFillAmountOut = params.amountOut / 2;
        uint128 firstFillAmountIn = uint128((uint256(params.amountIn) * firstFillAmountOut) / params.amountOut);

        _reportFill(users["solver"], orderId, firstFillAmountOut, firstFillAmountIn);

        // Fill the remaining 50% - for full fill, use remaining amounts
        uint128 remainingAmountOut = params.amountOut - firstFillAmountOut;
        uint128 remainingAmountIn = params.amountIn - firstFillAmountIn;

        uint256 solverBalanceBefore = tokenIn.balanceOf(users["solver"]);

        vm.prank(address(portal));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: remainingAmountOut,
                amountInToRelease: remainingAmountIn,
                originRecipient: users["solver"].toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );

        // Verify balance updated correctly
        assertEq(
            tokenIn.balanceOf(users["solver"]),
            solverBalanceBefore + remainingAmountIn,
            "solver should receive remaining amountIn"
        );

        // Verify order is now Completed
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Completed), "order should be Completed");
    }

    function _test_activeOrderPartialFill_success() internal {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 1, params);

        // Report partial fill (50%)
        uint128 fillAmount = params.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(params.amountIn) * fillAmount) / params.amountOut);

        // Record balances before
        uint256 recipientBalanceBefore = tokenIn.balanceOf(users["solver"]);
        uint256 orderBookBalanceBefore = tokenIn.balanceOf(address(orderBook));

        // Report fill via portal
        vm.prank(address(portal));
        orderBook.reportFill(
            params.destChainId,
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

        // Report full fill via portal
        vm.prank(address(portal));
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        orderBook.reportFill(
            params.destChainId,
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
