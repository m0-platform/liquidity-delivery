// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBookTestBase } from "../OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../../src/interfaces/IOrderBook.sol";

/// @notice Tests demonstrating rounding edge cases with mismatched decimals
contract RoundingEdgeCaseTest is OrderBookTestBase {
    using TypeConverter for *;

    /// @notice Demonstrates that dust fills round to zero when tokenIn decimals < tokenOut decimals
    /// @dev Order: 1 TOKEN6 (1e6) → 1 TOKEN18 (1e18)
    ///      Solver fills 1 wei of TOKEN18
    ///      amountInToRelease = (1e6 * 1) / 1e18 = 0
    function test_dustFill_roundsToZero_solverReceivesNothing()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        // Setup: Create a local order (same chain) for immediate fill
        params.destChainId = CHAIN_ID;
        params.amountIn = 1e6; // 1 TOKEN6
        params.amountOut = 1e18; // 1 TOKEN18

        // Alice creates order
        bytes32 orderId = _placeOrder(users["alice"], params);

        // Record balances before fill
        uint256 solverTokenInBefore = tokenIn.balanceOf(users["solver"]);
        uint256 aliceTokenOutBefore = tokenOut.balanceOf(users["alice"]);

        // Solver fills with minimum amount: 1 wei of TOKEN18
        uint128 dustFillAmount = 1;

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        vm.startPrank(users["solver"]);
        tokenOut.approve(address(orderBook), dustFillAmount);
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
            IOrderBook.FillParams({ amountOutToFill: dustFillAmount, originRecipient: users["solver"].toBytes32() })
        );
        vm.stopPrank();

        // Verify: Solver received 0 TOKEN6 (rounded to zero!)
        uint256 solverTokenInAfter = tokenIn.balanceOf(users["solver"]);
        assertEq(solverTokenInAfter - solverTokenInBefore, 0, "solver should receive 0 tokenIn due to rounding");

        // Verify: Alice received 1 wei of TOKEN18
        uint256 aliceTokenOutAfter = tokenOut.balanceOf(users["alice"]);
        assertEq(aliceTokenOutAfter - aliceTokenOutBefore, dustFillAmount, "alice should receive 1 wei of tokenOut");

        // Calculate expected: (1e6 * 1) / 1e18 = 0
        uint256 expectedAmountIn = (uint256(params.amountIn) * dustFillAmount) / params.amountOut;
        assertEq(expectedAmountIn, 0, "calculation should round to zero");
    }
}
