// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { VmSafe } from "../../../lib/forge-std/src/Vm.sol";
import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";

import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { MockERC20 } from "../../mock/MockERC20.t.sol";

contract CancelOrderForTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the signature is invalid (not from recipient)
    //   [X] it reverts
    // [X] given the signature is a valid standard ECDSA signature from recipient
    //   [X] it sets the order status to Cancelled
    //   [X] it emits an OrderCancelled event
    // [X] given the signature is a valid compact ECDSA signature from recipient
    //   [X] it sets the order status to Cancelled
    //   [X] it emits an OrderCancelled event
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
    // [X] given the order can be cancelled
    //   [X] given the destination chain is different to the current chain (i.e. cross-chain order)
    //     [X] it updates the order status to Cancelled
    //     [X] it sends a CancelReport to the origin chain via messenger
    //     [X] it emits an OrderCancelled event
    //   [X] given the destination chain is the current chain (i.e. local order)
    //     [X] it immediately refunds the order amount in to the order sender
    //     [X] it sets the order status to Cancelled
    //     [X] it emits an OrderCancelled event
    //     [X] it emits a RefundClaimed event

    VmSafe.Wallet internal recipient;
    VmSafe.Wallet internal sender;
    bytes32 internal orderId;
    IOrderBook.OrderData internal xchainOrderData;
    bytes32 internal xchainOrderId;

    function setUp() public override {
        super.setUp();

        // Create recipient wallet (signatures come from recipient now)
        recipient = vm.createWallet("recipient");
        vm.deal(recipient.addr, 1 ether);

        // Create sender wallet
        sender = vm.createWallet("sender");
        vm.deal(sender.addr, 1 ether);
        tokenIn.mint(sender.addr, MINT_AMOUNT * (10 ** tokenIn.decimals()));

        vm.prank(sender.addr);
        tokenIn.approve(address(orderBook), type(uint256).max);

        // Set recipient in params
        params.recipient = recipient.addr.toBytes32();

        // Create local order for tests
        params.destChainId = CHAIN_ID;
        orderId = _placeOrder(sender.addr, params);

        // Create xchain order data that originates on another chain and is destined for this chain
        xchainOrderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order originated from another chain
            sender: sender.addr.toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // This chain is the destination
            createdAt: uint64(block.timestamp),
            fillDeadline: params.fillDeadline,
            amountIn: params.amountIn,
            amountOut: params.amountOut,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: params.tokenOut,
            recipient: recipient.addr.toBytes32(),
            solver: params.solver
        });
        xchainOrderId = orderBook.getOrderId(xchainOrderData);
    }

    function _signStandardECDSA(VmSafe.Wallet memory wallet_, bytes32 orderId_) internal returns (bytes memory) {
        bytes32 digest = orderBook.getCancelOrderDigest(orderId_);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(wallet_, digest);
        return abi.encodePacked(r, s, v);
    }

    function _signCompactECDSA(VmSafe.Wallet memory wallet_, bytes32 orderId_) internal returns (bytes memory) {
        bytes32 digest = orderBook.getCancelOrderDigest(orderId_);
        (bytes32 r, bytes32 vs) = vm.signCompact(wallet_, digest);
        return abi.encodePacked(r, vs);
    }

    /* ========== Tests ========= */

    function test_givenSignatureNotFromRecipient_reverts() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        bytes memory signature = _signStandardECDSA(vm.createWallet("attacker"), orderId);

        vm.expectRevert();
        orderBook.cancelOrderFor{ value: 0 }(orderId, orderData, signature);
    }

    function test_givenCompactECDSASignature_succeeds() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        bytes memory signature = _signCompactECDSA(recipient, orderId);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(orderId);

        orderBook.cancelOrderFor{ value: 0 }(orderId, orderData, signature);

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Cancelled));
    }

    function test_givenStandardECDSASignature_succeeds() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        bytes memory signature = _signStandardECDSA(recipient, orderId);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(orderId);

        orderBook.cancelOrderFor{ value: 0 }(orderId, orderData, signature);

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Cancelled));
    }

    function test_givenOrderDoesNotExist_reverts() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);
        orderData.nonce = 999; // Wrong nonce to create a non-existent order ID
        bytes32 fakeOrderId = orderBook.getOrderId(orderData);

        bytes memory signature = _signStandardECDSA(recipient, fakeOrderId);

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrderFor{ value: 0 }(fakeOrderId, orderData, signature);
    }

    function test_givenOrderAlreadyCancelled_reverts() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // First, cancel the order (recipient can cancel directly)
        vm.prank(recipient.addr);
        orderBook.cancelOrder{ value: 0 }(orderId, orderData);

        // Attempt to cancel again via cancelOrderFor
        bytes memory signature = _signStandardECDSA(recipient, orderId);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrderFor{ value: 0 }(orderId, orderData, signature);
    }

    function test_givenOrderAlreadyFilledLocal_reverts() public {
        // First, fill the order
        _fillOrder(users["solver"], orderId, params.amountOut);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Attempt to cancel
        bytes memory signature = _signStandardECDSA(recipient, orderId);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrderFor{ value: 0 }(orderId, orderData, signature);
    }

    function test_givenOrderAlreadyFilledXchain_reverts() public {
        // First, fill on cross-chain order
        vm.startPrank(users["solver"]);
        MockERC20(xchainOrderData.tokenOut.toAddress()).approve(address(orderBook), params.amountOut);
        orderBook.fillOrder(
            xchainOrderId,
            xchainOrderData,
            IOrderBook.FillParams({ amountOutToFill: params.amountOut, originRecipient: xchainOrderData.solver })
        );
        vm.stopPrank();

        // Attempt to cancel
        bytes memory signature = _signStandardECDSA(recipient, xchainOrderId);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.cancelOrderFor{ value: 1 }(xchainOrderId, xchainOrderData, signature);
    }

    function test_givenOrderIdMismatch_reverts() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Use wrong orderId
        bytes32 wrongOrderId = bytes32("wrong order id");

        bytes memory signature = _signStandardECDSA(recipient, wrongOrderId);

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderIdMismatch.selector));
        orderBook.cancelOrderFor{ value: 0 }(wrongOrderId, orderData, signature);
    }

    function test_givenCreatedAtInFuture_reverts() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        // Set createdAt to future timestamp
        orderData.createdAt = uint64(block.timestamp + 1 hours);
        bytes32 futureOrderId = orderBook.getOrderId(orderData);

        bytes memory signature = _signStandardECDSA(recipient, futureOrderId);

        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidTimestamp.selector));
        orderBook.cancelOrderFor{ value: 0 }(futureOrderId, orderData, signature);
    }

    function test_givenLocalOrder_success() public {
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        IOrderBook.OrderData memory orderData = _getOrderDataFromOrder(orderId, order);

        bytes memory signature = _signStandardECDSA(recipient, orderId);

        vm.expectEmit(true, true, false, true);
        emit IOrderBook.RefundClaimed(orderId, sender.addr, params.amountIn);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(orderId);

        uint256 senderBalanceBefore = tokenIn.balanceOf(sender.addr);

        orderBook.cancelOrderFor{ value: 0 }(orderId, orderData, signature);

        uint256 senderBalanceAfter = tokenIn.balanceOf(sender.addr);
        assertEq(senderBalanceAfter - senderBalanceBefore, params.amountIn);

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Cancelled));
    }

    function test_givenCrosschainOrder_success() public {
        // For this test, we simulate being on the DESTINATION chain canceling an order
        // that originated from a different chain (DEST_CHAIN_ID).
        // We construct orderData with originChainId = DEST_CHAIN_ID (not current chain)
        // and the order doesn't exist on this chain yet (DoesNotExist status is allowed for xchain)

        // Order doesn't exist on this chain (DoesNotExist status) - this is valid for cross-chain cancel
        IOrderBook.Order memory order = orderBook.getOrder(xchainOrderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.DoesNotExist));

        bytes memory signature = _signStandardECDSA(recipient, xchainOrderId);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.OrderCancelled(xchainOrderId);

        orderBook.cancelOrderFor{ value: 1 }(xchainOrderId, xchainOrderData, signature);

        IOrderBook.Order memory updatedOrder = orderBook.getOrder(xchainOrderId);
        assertEq(
            uint8(updatedOrder.status),
            uint8(IOrderBook.OrderStatus.Cancelled),
            "order status should be Cancelled"
        );

        // Verify cancel report was sent to messenger
        assertTrue(messenger.isCancelReported(xchainOrderId), "cancel report should have been sent");
    }
}
