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
    //     [X] it updates the order status to Cancelled
    //     [X] it sends a CancelReport to the origin chain via portal
    //     [X] it emits an OrderCancelled event
    //   [X] given the destination chain is the origin chain (i.e. local order)
    //     [X] given the msg.value is not zero
    //       [X] it reverts with an InvalidMsgValue error
    //     [X] it immediately refunds the order amount in to the order sender
    //     [X] it sets the order status to Cancelled
    //     [X] it emits an OrderCancelled event
    //     [X] it emits a RefundClaimed event

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

    function test_givenCrosschainOrderHasBeenFilled_reverts() public {
        // Fill the order
        vm.startPrank(users["solver"]);
        MockERC20(xchainOrderData.tokenOut.toAddress()).approve(address(orderBook), params.amountOut);
        orderBook.fillOrder(
            xchainOrderId,
            xchainOrderData,
            IOrderBook.FillParams({ amountOutToFill: params.amountOut, originRecipient: xchainOrderData.solver })
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

    function test_givenXChainOrder_givenChainIsNotDestinationChain_reverts() public {
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
        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(orderId);
        orderBook.cancelOrder(orderId, orderData, new bytes(0));

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

        // Bob (not recipient) can now cancel
        vm.prank(users["bob"]);
        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(xchainOrderId);
        orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(xchainOrderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );
    }

    function test_givenXChainOrder_success() public {
        // For this test, we simulate being on the DESTINATION chain canceling an order
        // that originated from a different chain (DEST_CHAIN_ID).
        // We construct orderData with originChainId = DEST_CHAIN_ID (not current chain)
        // and the order doesn't exist on this chain yet (DoesNotExist status is allowed for xchain)

        // Order doesn't exist on this chain (DoesNotExist status) - this is valid for cross-chain cancel
        IOrderBook.Order memory order = orderBook.getOrder(xchainOrderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.DoesNotExist));

        vm.prank(users["alice"]);
        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(xchainOrderId);
        orderBook.cancelOrder(xchainOrderId, xchainOrderData, new bytes(0));

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(xchainOrderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );

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
        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(orderId);
        orderBook.cancelOrder(orderId, orderData, new bytes(0));

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

    function test_givenXChainOrder_senderCalls_reverts() public {
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
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.RefundClaimed(orderId, users["alice"], params.amountIn);
        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(orderId);
        orderBook.cancelOrder(orderId, orderData, new bytes(0));

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
}
