// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBookTestBase } from "../OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../../src/interfaces/IOrderBook.sol";
import { SafeERC20 } from "../../../../src/libs/SafeERC20.sol";

/// @notice Tests demonstrating originRecipient validation behavior
/// @dev The contract validates originRecipient is not zero address and catches orderBook address via safeTransferExact
contract InvalidOriginRecipientTest is OrderBookTestBase {
    using TypeConverter for *;

    /// @notice Demonstrates that safeTransferExact catches self-transfer but with unclear error
    /// @dev Same-chain fill: originRecipient = orderBook causes revert in safeTransferExact
    ///      because balance doesn't increase when transferring to self
    function test_originRecipientIsOrderBook_revertsDueToSafeTransferExact() public {
        // Setup: Create a local order (same chain) for immediate fill
        params.destChainId = CHAIN_ID;

        // Alice creates order
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Solver fills order but mistakenly sets originRecipient to address(orderBook)
        vm.startPrank(users["solver"]);
        tokenOut.approve(address(orderBook), params.amountOut);

        // Reverts with SafeERC20 error - balance didn't increase
        // An explicit InvalidRecipient check would be clearer and fail earlier
        vm.expectRevert(
            abi.encodeWithSelector(
                SafeERC20.SafeERC20FeeOnTransfer.selector,
                address(tokenIn),
                address(orderBook),
                params.amountIn,
                0
            )
        );
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: order.fillDeadline,
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: params.amountOut,
                originRecipient: address(orderBook).toBytes32()
            })
        );
        vm.stopPrank();
    }

    /// @notice Verifies that zero address originRecipient reverts with InvalidRecipient
    /// @dev fillOrder with originRecipient = address(0) now properly reverts
    function test_originRecipientIsZero_reverts() public {
        // Setup: Create a local order (same chain) for immediate fill
        params.destChainId = CHAIN_ID;

        // Alice creates order
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        vm.startPrank(users["solver"]);
        tokenOut.approve(address(orderBook), params.amountOut);

        // Reverts with InvalidRecipient when originRecipient is zero address
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidRecipient.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: order.fillDeadline,
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: params.amountOut,
                originRecipient: bytes32(0) // address(0)
            })
        );
        vm.stopPrank();
    }
}
