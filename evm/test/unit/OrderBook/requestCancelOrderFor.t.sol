// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { VmSafe } from "../../../lib/forge-std/src/Vm.sol";
import { console } from "../../../lib/forge-std/src/console.sol";
import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";

import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";

contract requestCancelOrderForTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the signature is invalid
    //   [X] it reverts
    // [X] given the signature is a valid standard ECDSA signature
    //   [X] it sets to order status to CancelRequested
    //   [X] it emits an CancelRequested event
    // [X] given the signature is a valid compact ECDSA signature
    //   [X] it sets to order status to CancelRequested
    //   [X] it emits an CancelRequested event
    // [X] given the order does not exist
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but already cancelled
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but already filled (local)
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the order exists but has already filled (cross-chain)
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the current timestamp is > the fill deadline
    //   [X] it reverts with an OrderExpired error
    // [X] given the order can be cancelled
    //   [X] given the destination chain is different to the current chain (i.e. cross-chain order)
    //     [X] it updates the order status to CancelRequested
    //     [X] it sets the refund requested at timestamp to the current block timestamp
    //     [X] it emits an CancelRequest event
    //   [X] given the destination chain is the current chain (i.e. local order)
    //     [X] it immediately refunds the order amount in to the order sender
    //     [X] it sets the order status to Completed
    //     [X] it emits a CancelRequested event
    //     [X] it emits a RefundClaimed event

    VmSafe.Wallet internal sender;
    bytes32 internal orderId;

    function setUp() public override {
        super.setUp();

        sender = vm.createWallet("sender");
        vm.deal(sender.addr, 1 ether);
        tokenIn.mint(sender.addr, MINT_AMOUNT * (10 ** tokenIn.decimals()));

        vm.prank(sender.addr);
        tokenIn.approve(address(orderBook), type(uint256).max);

        orderId = _placeOrder(sender.addr, params);
    }

    function _signStandardECDSA(VmSafe.Wallet memory wallet_, bytes32 orderId_) internal returns (bytes memory) {
        bytes32 digest = orderBook.getCancelRequestDigest(orderId_);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(wallet_, digest);
        return abi.encodePacked(r, s, v);
    }

    function _signCompactECDSA(VmSafe.Wallet memory wallet_, bytes32 orderId_) internal returns (bytes memory) {
        bytes32 digest = orderBook.getCancelRequestDigest(orderId_);
        (bytes32 r, bytes32 vs) = vm.signCompact(wallet_, digest);
        return abi.encodePacked(r, vs);
    }

    /* ========== Tests ========= */

    function test_givenSignatureNotFromSender_reverts() public {
        bytes memory signature = _signStandardECDSA(vm.createWallet("attacker"), orderId);

        vm.expectRevert();
        orderBook.requestCancelOrderFor(orderId, signature);
    }

    function test_givenCompactECDSASignature_succeeds() public {
        bytes memory signature = _signCompactECDSA(sender, orderId);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.CancelRequested(orderId, uint32(block.timestamp));

        orderBook.requestCancelOrderFor(orderId, signature);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.CancelRequested));
    }

    function test_givenStandardECDSASignature_succeeds() public {
        bytes memory signature = _signStandardECDSA(sender, orderId);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.CancelRequested(orderId, uint32(block.timestamp));

        orderBook.requestCancelOrderFor(orderId, signature);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.CancelRequested));
    }

    function test_givenOrderDoesNotExist_reverts() public {
        bytes32 orderId_ = ~orderId;
        bytes memory signature = _signStandardECDSA(sender, orderId_);

        vm.expectRevert();
        orderBook.requestCancelOrderFor(orderId_, signature);
    }

    function test_givenOrderAlreadyCancelled_reverts() public {
        // First, cancel the order
        vm.prank(sender.addr);
        orderBook.requestCancelOrder(orderId);

        // Attempt to cancel again
        bytes memory signature = _signStandardECDSA(sender, orderId);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrderFor(orderId, signature);
    }

    function test_givenOrderAlreadyFilledLocal_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId_ = _placeOrder(sender.addr, params);

        // First, fill the order
        _fillOrder(users["solver"], orderId_, params.amountOut);

        // Attempt to cancel
        bytes memory signature = _signStandardECDSA(sender, orderId_);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrderFor(orderId_, signature);
    }

    function test_givenOrderAlreadyFilledXchain_reverts() public {
        // First, fill the order
        _reportFill(users["solver"], orderId, params.amountOut, params.amountIn);

        // Attempt to cancel
        bytes memory signature = _signStandardECDSA(sender, orderId);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.requestCancelOrderFor(orderId, signature);
    }

    function test_givenPastFillDeadline_reverts() public {
        // Warp past fill deadline
        vm.warp(params.fillDeadline + 1);

        bytes memory signature = _signStandardECDSA(sender, orderId);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderExpired.selector));
        orderBook.requestCancelOrderFor(orderId, signature);
    }

    function test_givenLocalOrder_success() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId_ = _placeOrder(sender.addr, params);

        bytes memory signature = _signStandardECDSA(sender, orderId_);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.CancelRequested(orderId_, uint32(block.timestamp));

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.RefundClaimed(orderId_, sender.addr, params.amountIn);

        uint256 senderBalanceBefore = tokenIn.balanceOf(sender.addr);

        orderBook.requestCancelOrderFor(orderId_, signature);

        uint256 senderBalanceAfter = tokenIn.balanceOf(sender.addr);
        assertEq(senderBalanceAfter - senderBalanceBefore, params.amountIn);

        IOrderBook.Order memory order = orderBook.getOrder(orderId_);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Completed));
    }

    function test_givenCrosschainOrder_success() public {
        bytes memory signature = _signStandardECDSA(sender, orderId);

        vm.expectEmit(true, false, false, false);
        emit IOrderBook.CancelRequested(orderId, uint32(block.timestamp));

        orderBook.requestCancelOrderFor(orderId, signature);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.CancelRequested));
        assertEq(
            order.cancelRequestedAt,
            uint32(block.timestamp),
            "cancelRequestedAt should be updated to current block timestamp"
        );
    }
}
