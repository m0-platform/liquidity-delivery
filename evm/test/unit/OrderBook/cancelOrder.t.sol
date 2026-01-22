// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";
import { console } from "../../../lib/forge-std/src/console.sol";
import { PausableUpgradeable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";
import { MockERC20 } from "../../mock/MockERC20.t.sol";

contract CancelOrderTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the contract is paused
    //    [X] it reverts with an EnforcedPause error
    // [X] given the order does not exist
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but already cancelled
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but already filled (local)
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but has already filled (cross-chain)
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the orderId does not match computed OrderData hash
    //   [X] it reverts with an OrderIdMismatch error
    // [X] given the createdAt timestamp is in the future
    //   [X] it reverts with an InvalidTimestamp error
    // [X] given the caller is not the recipient AND current time <= fillDeadline
    //   [X] it reverts with a NotAuthorized error
    // [X] given the current time > fillDeadline
    //   [X] anyone can cancel and trigger refund
    // [X] given the current chain is not the destination chain (cross-chain order)
    //   [X] it reverts with an InvalidDestinationChain error
    // [X] given the current chain is not the destination chain (local order)
    //   [X] it reverts with an InvalidDestinationChain error
    // [X] given the order can be cancelled by recipient
    //   [X] given the destination chain is different than the origin chain (i.e. cross-chain order)
    //     [X] given no fills have occurred
    //       [X] it updates the order status to Cancelled
    //       [X] it sends a CancelReport to the origin chain via portal
    //       [X] it emits an OrderCancelled event
    //       [X] it increments the amountInRefunded by the full amountIn
    //     [X] given partial fills have occurred
    //       [X] it updates the order status to Cancelled
    //       [X] it sends a CancelReport to the origin chain via portal
    //       [X] it emits an OrderCancelled event
    //       [X] it increments the amountInRefunded by the unfilled amountIn
    //   [X] given the destination chain is the origin chain (i.e. local order)
    //     [X] given the msg.value is not zero
    //       [X] it reverts with an InvalidMsgValue error
    //     [X] given no fills have occurred
    //       [X] it immediately refunds the order amount in to the order sender
    //       [X] it sets the order status to Cancelled
    //       [X] it emits an OrderCancelled event
    //       [X] it emits a RefundClaimed event
    //       [X] it increments the amountInRefunded by the full amountIn
    //     [X] given partial fills have occurred
    //       [X] it refunds the unfilled amountIn to the order sender
    //       [X] it sets the order status to Cancelled
    //       [X] it emits an OrderCancelled event
    //       [X] it emits a RefundClaimed event
    //       [X] it increments the amountInRefunded by the unfilled amountIn

    IOrderBook.OrderData internal xchainOrderData;
    bytes32 internal xchainOrderId;

    function setUp() public override {
        super.setUp();

        // open a local order for alice with recipient = alice
        params.destChainId = CHAIN_ID;
        _placeOrder(users["alice"], params);

        // create order data for cross-chain order that originates on another chain and is destined for this chain
        xchainOrderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID,
            sender: users["alice"].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // current chain ID
            createdAt: uint64(block.timestamp),
            fillDeadline: params.fillDeadline,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: params.tokenOut,
            recipient: users["alice"].toBytes32(),
            solver: params.solver
        });
        xchainOrderId = orderBook.getOrderId(xchainOrderData);
    }

    function test_whenPaused_localOrder_reverts() public {
        vm.prank(pauser);
        orderBook.pause();

        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.cancelOrder(orderId, orderData, new bytes(0));
    }

    function test_whenPaused_xchainOrder_reverts() public {
        vm.prank(pauser);
        orderBook.pause();

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));
    }

    function test_givenOrderDoesNotExist_reverts() public {
        IOrderBook.Order memory order = orderBook.getOrder(_getOrderIdFromParams(users["alice"], 0, params));
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(bytes32(0), order);
        orderData.nonce = 999; // Wrong nonce to create a non-existent order ID
        bytes32 fakeOrderId = orderBook.getOrderId(orderData);

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrder(fakeOrderId, orderData, new bytes(0));
    }

    function test_givenOrderIsAlreadyCancelled_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Cancel the order first (recipient can cancel)
        vm.prank(users["alice"]);
        orderBook.cancelOrder(orderId, orderData, new bytes(0));

        // Try to cancel again
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrder(orderId, orderData, new bytes(0));
    }

    function test_givenLocalOrderHasBeenFilled_reverts() public {
        // Use the local order created in setUp
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);

        // Fill the order
        _fillOrder(users["solver"], orderId, params.amountOut);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Try to cancel the order
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrder(orderId, orderData, new bytes(0));
    }

    function test_givenXchainOrderHasBeenFilled_reverts() public {
        // Fill the order
        vm.startPrank(users["solver"]);
        MockERC20(xchainOrderData.tokenOut.toAddress()).approve(address(orderBook), params.amountOut);
        orderBook.fillOrder(
            xchainOrderId,
            xchainOrderData,
            IOrderBook.FillParams({
                amountOutToFill: params.amountOut,
                originRecipient: xchainOrderData.solver,
                refundAddress: bytes32(0)
            })
        );
        vm.stopPrank();

        // Try to cancel the order
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));
    }

    function test_givenOrderIdMismatch_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Use wrong orderId
        bytes32 wrongOrderId = bytes32("wrong order id");

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderIdMismatch.selector));
        orderBook.cancelOrder(wrongOrderId, orderData, new bytes(0));
    }

    function test_givenCreatedAtInFuture_reverts() public {
        IOrderBook.OrderData memory orderData = xchainOrderData;

        // Set createdAt to future timestamp
        orderData.createdAt = uint64(block.timestamp + 1 hours);
        bytes32 futureOrderId = orderBook.getOrderId(orderData);

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidTimestamp.selector));
        orderBook.cancelOrder(futureOrderId, orderData, new bytes(0));
    }

    function test_givenLocalOrder_givenCallerNotRecipientBeforeDeadline_reverts() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Bob tries to cancel before deadline (bob is not the recipient)
        vm.prank(users["bob"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.cancelOrder(orderId, orderData, new bytes(0));
    }

    function test_givenXchainOrder_givenCallerNotRecipientBeforeDeadline_reverts() public {
        // Bob tries to cancel before deadline (bob is not the recipient)
        vm.prank(users["bob"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));
    }

    function test_givenLocalOrder_givenChainIsNotDestinationChain_reverts() public {
        // Create order data for a local order on another chain
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID,
            sender: users["alice"].toBytes32(),
            nonce: 0,
            destChainId: DEST_CHAIN_ID, // Local order on another chain
            createdAt: uint64(block.timestamp),
            fillDeadline: params.fillDeadline,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: params.tokenOut,
            recipient: users["alice"].toBytes32(),
            solver: params.solver
        });
        bytes32 orderId = orderBook.getOrderId(orderData);

        // Try to cancel on the current chain when it's not the destination chain
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDestinationChain.selector));
        orderBook.cancelOrder(orderId, orderData, new bytes(0));
    }

    function test_givenLocalOrder_givenMsgValueNotZero_reverts() public {
        // User local order
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        uint256 aliceStartingBalance = tokenIn.balanceOf(users["alice"]);

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidMsgValue.selector));
        orderBook.cancelOrder{ value: 1 }(orderId, orderData, new bytes(0));
    }

    function test_givenXchainOrder_givenChainIsNotDestinationChain_reverts() public {
        // Create an order that originates from this chain but is destined for another chain
        params.destChainId = DEST_CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Try to cancel on the current chain when it's not the destination chain
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDestinationChain.selector));
        orderBook.cancelOrder(orderId, orderData, new bytes(0));
    }

    function test_givenLocalOrder_givenAfterFillDeadline_anyoneCanCancel() public {
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Warp past fill deadline
        vm.warp(order.fillDeadline + 1);

        // Bob (not recipient) can now cancel
        vm.prank(users["bob"]);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.OrderCancelled(orderId, bytes32(0));
        bytes32 messageId = orderBook.cancelOrder(orderId, orderData, new bytes(0));

        // Check messageId is zero for local orders
        assertEq(messageId, bytes32(0), "messageId should be zero for local orders");

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
    }

    function test_givenXchainOrder_givenAfterFillDeadline_anyoneCanCancel() public {
        // Warp past fill deadline
        vm.warp(xchainOrderData.fillDeadline + 1);

        bytes32 expectedMessageId = keccak256(abi.encodePacked("cancel", xchainOrderId));

        // Bob (not recipient) can now cancel
        vm.prank(users["bob"]);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.OrderCancelled(xchainOrderId, expectedMessageId);
        bytes32 messageId = orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));

        // Check messageId is non-zero for cross-chain orders
        assertEq(messageId, expectedMessageId, "messageId should match expected value for cross-chain orders");

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(xchainOrderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
    }

    function test_givenXchainOrder_success() public {
        // For this test, we simulate being on the DESTINATION chain canceling an order
        // that originated from a different chain (DEST_CHAIN_ID).
        // We construct orderData with originChainId = DEST_CHAIN_ID (not current chain)
        // and the order doesn't exist on this chain yet (DoesNotExist status is allowed for xchain)

        // Order doesn't exist on this chain (DoesNotExist status) - this is valid for cross-chain cancel
        IOrderBook.Order memory order = orderBook.getOrder(xchainOrderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.DoesNotExist));

        bytes32 expectedMessageId = keccak256(abi.encodePacked("cancel", xchainOrderId));

        vm.prank(users["alice"]);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.OrderCancelled(xchainOrderId, expectedMessageId);
        bytes32 messageId = orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));

        // Check messageId is non-zero for cross-chain orders
        assertEq(messageId, expectedMessageId, "messageId should match expected value for cross-chain orders");

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(xchainOrderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
        IOrderBook.FilledAmounts memory filledAmounts = orderBook.getFilledAmounts(xchainOrderId);
        assertEq(filledAmounts.amountInReleased, 0, "amountInReleased should be zero");
        assertEq(filledAmounts.amountOutFilled, 0, "amountOutFilled should be zero");
        assertEq(filledAmounts.amountInRefunded, params.amountIn, "amountInRefunded should be the initial amount in");

        // Verify cancel report was sent to portal
        assertTrue(portal.isCancelReported(xchainOrderId), "cancel report should have been sent");
    }

    function test_givenLocalOrder_success() public {
        // Open a local order for alice
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        uint256 aliceStartingBalance = tokenIn.balanceOf(users["alice"]);

        vm.prank(users["alice"]);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], params.amountIn);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.OrderCancelled(orderId, bytes32(0));
        bytes32 messageId = orderBook.cancelOrder(orderId, orderData, new bytes(0));

        // Check messageId is zero for local orders
        assertEq(messageId, bytes32(0), "messageId should be zero for local orders");

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
        assertEq(
            tokenIn.balanceOf(users["alice"]),
            aliceStartingBalance + params.amountIn,
            "alice should be refunded the amountIn"
        );

        IOrderBook.FilledAmounts memory filledAmounts = orderBook.getFilledAmounts(orderId);
        assertEq(filledAmounts.amountInReleased, 0, "amountInReleased should be zero");
        assertEq(filledAmounts.amountOutFilled, 0, "amountOutFilled should be zero");
        assertEq(filledAmounts.amountInRefunded, params.amountIn, "amountInRefunded should be the initial amount in");
    }

    function test_givenXchainOrder_senderCalls_reverts() public {
        // For this test, we simulate being on the DESTINATION chain canceling an order
        // that originated from a different chain (DEST_CHAIN_ID).
        // We construct orderData with originChainId = DEST_CHAIN_ID (not current chain)
        // and the order doesn't exist on this chain yet (DoesNotExist status is allowed for xchain)
        // Set the recipient to be different from the sender

        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order originated from another chain
            sender: users["alice"].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // This chain is the destination
            createdAt: uint64(block.timestamp),
            fillDeadline: params.fillDeadline,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: params.tokenOut,
            recipient: users["bob"].toBytes32(),
            solver: params.solver
        });
        bytes32 orderId = orderBook.getOrderId(orderData);

        // Order doesn't exist on this chain (DoesNotExist status) - this is valid for cross-chain cancel
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.DoesNotExist));

        vm.prank(users["alice"]);
        vm.expectRevert(IOrderBook.NotAuthorized.selector);
        orderBook.cancelOrder(orderId, orderData, new bytes(0));
    }

    function test_givenLocalOrder_senderCalls_success() public {
        // Open a local order for alice with bob as the recipient
        params.recipient = users["bob"].toBytes32();
        params.destChainId = CHAIN_ID;

        bytes32 orderId = _placeOrder(users["alice"], params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        uint256 aliceStartingBalance = tokenIn.balanceOf(users["alice"]);

        vm.prank(users["alice"]);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], params.amountIn);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.OrderCancelled(orderId, bytes32(0));
        bytes32 messageId = orderBook.cancelOrder(orderId, orderData, new bytes(0));

        // Check messageId is zero for local orders
        assertEq(messageId, bytes32(0), "messageId should be zero for local orders");

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
        assertEq(
            tokenIn.balanceOf(users["alice"]),
            aliceStartingBalance + params.amountIn,
            "alice should be refunded the amountIn"
        );
    }

    function test_givenLocalOrder_givenPartialFill_success() public {
        // Open a local order for alice
        bytes32 orderId = _getOrderIdFromParams(users["alice"], 0, params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        uint256 aliceStartingBalance = tokenIn.balanceOf(users["alice"]);
        uint256 solverStartingBalance = tokenIn.balanceOf(users["solver"]);
        uint256 solverOutStartingBalance = tokenOut.balanceOf(users["solver"]);

        // Partially fill the order (50%)
        uint128 partialFillAmountOut = params.amountOut / 2;
        _fillOrder(users["solver"], orderId, partialFillAmountOut);

        // Cancel the order and expect refund of remaining amountIn
        uint128 expectedRefundAmountIn = params.amountIn / 2;
        vm.prank(users["alice"]);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], expectedRefundAmountIn);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.OrderCancelled(orderId, bytes32(0));
        bytes32 messageId = orderBook.cancelOrder(orderId, orderData, new bytes(0));

        // Check messageId is zero for local orders
        assertEq(messageId, bytes32(0), "messageId should be zero for local orders");

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
        assertEq(
            tokenIn.balanceOf(users["alice"]),
            aliceStartingBalance + expectedRefundAmountIn,
            "alice should be refunded the amountIn"
        );
        assertEq(
            tokenIn.balanceOf(users["solver"]),
            solverStartingBalance + (params.amountIn - expectedRefundAmountIn),
            "solver should keep the filled amountIn"
        );
        assertEq(
            tokenOut.balanceOf(users["solver"]),
            solverOutStartingBalance - partialFillAmountOut,
            "solver should have sent the filled amountOut"
        );

        IOrderBook.FilledAmounts memory filledAmounts = orderBook.getFilledAmounts(orderId);
        assertEq(
            filledAmounts.amountInReleased,
            params.amountIn - expectedRefundAmountIn,
            "amountInReleased should be the released amount in"
        );
        assertEq(
            filledAmounts.amountOutFilled,
            partialFillAmountOut,
            "amountOutFilled should be the filled amount out"
        );
        assertEq(
            filledAmounts.amountInRefunded,
            expectedRefundAmountIn,
            "amountInRefunded should be the refunded amount in"
        );
    }

    function test_givenXchainOrder_givenPartialFill_success() public {
        // For this test, we simulate being on the DESTINATION chain canceling an order
        // that originated from a different chain (DEST_CHAIN_ID).
        // We construct orderData with originChainId = DEST_CHAIN_ID (not current chain)
        // and the order doesn't exist on this chain yet (DoesNotExist status is allowed for xchain)

        uint256 aliceStartingBalance = tokenIn.balanceOf(users["alice"]);
        uint256 solverStartingBalance = tokenIn.balanceOf(users["solver"]);
        uint256 solverOutStartingBalance = tokenOut.balanceOf(users["solver"]);

        // Partially fill the order (50%)
        uint128 partialFillAmountOut = params.amountOut / 2;
        vm.startPrank(users["solver"]);
        MockERC20(xchainOrderData.tokenOut.toAddress()).approve(address(orderBook), params.amountOut);
        orderBook.fillOrder(
            xchainOrderId,
            xchainOrderData,
            IOrderBook.FillParams({
                amountOutToFill: partialFillAmountOut,
                originRecipient: xchainOrderData.solver,
                refundAddress: bytes32(0)
            })
        );
        vm.stopPrank();

        // Cancel the order and expect refund of remaining amountIn
        uint128 expectedRefundAmountIn = params.amountIn / 2;
        bytes32 expectedMessageId = keccak256(abi.encodePacked("cancel", xchainOrderId));
        vm.prank(users["alice"]);
        vm.expectEmit(true, true, false, true);
        emit IOrderBook.OrderCancelled(xchainOrderId, expectedMessageId);
        bytes32 messageId = orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));

        // Check messageId is non-zero for cross-chain orders
        assertEq(messageId, expectedMessageId, "messageId should match expected value for cross-chain orders");

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(xchainOrderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
        IOrderBook.FilledAmounts memory filledAmounts = orderBook.getFilledAmounts(xchainOrderId);
        assertEq(
            filledAmounts.amountInReleased,
            params.amountIn / 2,
            "amountInReleased should be the released amount in"
        );
        assertEq(
            filledAmounts.amountOutFilled,
            partialFillAmountOut,
            "amountOutFilled should be the filled amount out"
        );
        assertEq(
            filledAmounts.amountInRefunded,
            expectedRefundAmountIn,
            "amountInRefunded should be the refunded amount in"
        );
    }

    function test_givenXchainOrder_msgValueForwardedToPortal() public {
        // Create a new cross-chain order to test msg.value forwarding
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID,
            sender: users["alice"].toBytes32(),
            nonce: 1, // Use different nonce than setUp
            destChainId: CHAIN_ID,
            createdAt: uint64(block.timestamp),
            fillDeadline: params.fillDeadline,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: params.tokenOut,
            recipient: users["alice"].toBytes32(),
            solver: params.solver
        });
        bytes32 orderId = orderBook.getOrderId(orderData);

        uint256 msgValue = 0.1 ether;

        // Cancel the order with msg.value
        vm.prank(users["alice"]);
        orderBook.cancelOrder{ value: msgValue }(orderId, orderData, new bytes(0));

        // Verify the msg.value was forwarded to the portal
        assertEq(portal.getCancelReportValue(orderId), msgValue, "msg.value should be forwarded to portal");
    }

    function test_givenXchainOrder_withBridgeAdapter_msgValueForwardedToPortal() public {
        // Create a new cross-chain order to test msg.value forwarding with bridge adapter
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID,
            sender: users["alice"].toBytes32(),
            nonce: 2, // Use different nonce than other tests
            destChainId: CHAIN_ID,
            createdAt: uint64(block.timestamp),
            fillDeadline: params.fillDeadline,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: params.tokenOut,
            recipient: users["alice"].toBytes32(),
            solver: params.solver
        });
        bytes32 orderId = orderBook.getOrderId(orderData);

        uint256 msgValue = 0.2 ether;
        address bridgeAdapter = address(0x1234);

        // Cancel the order with msg.value and bridge adapter
        vm.prank(users["alice"]);
        orderBook.cancelOrder{ value: msgValue }(orderId, orderData, bridgeAdapter, new bytes(0));

        // Verify the msg.value was forwarded to the portal
        assertEq(
            portal.getCancelReportValue(orderId),
            msgValue,
            "msg.value should be forwarded to portal with bridge adapter"
        );
    }
}
