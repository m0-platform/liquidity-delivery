// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

contract OpenOrderTest is OrderBookTestBase {
    // Test cases
    // [X] given the fill deadline is before the current block timestamp
    //    [X] it reverts with an InvalidDeadline error
    // [X] given the amount in is zero
    //    [X] it reverts with an AmountInZero error
    // [X] given the amount out is zero
    //    [X] it reverts with an AmountOutZero error
    // [ ] given the destination chain is invalid
    //   [ ] it reverts with an InvalidDestinationChain error
    // [X] given the sender has not approved the order book to spend their token in
    //   [X] it reverts with an ERC20 transfer error
    // [X] given the sender has not enough balance of the token in
    //   [X] it reverts with an ERC20 transfer error
    // [X] given the order is valid, the sender has approved the order book to spend their token in, and the sender has enough balance of the token in
    //   [X] it transfers the amount in of the token in from the sender to the order book
    //   [X] it stores the order against the correct order ID
    //   [X] it emits an OrderOpened event
    //   [X] it returns the order ID

    function test_fillDeadlineBeforeCurrentTime_reverts() public {
        params.fillDeadline = uint40(block.timestamp - 1);
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDeadline.selector));
        orderBook.openOrder(params);
    }

    function test_amountInIsZero_reverts() public {
        params.amountIn = 0;
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.AmountInZero.selector));
        orderBook.openOrder(params);
    }

    function test_amountOutIsZero_reverts() public {
        params.amountOut = 0;
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.AmountOutZero.selector));
        orderBook.openOrder(params);
    }

    function test_destinationChainIsInvalid_reverts() public {
        params.destChainId = 100;
        vm.prank(users[0]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDestinationChain.selector));
        orderBook.openOrder(params);
    }

    function test_senderHasNotApprovedOrderBook_reverts() public {
        vm.prank(users[0]);
        vm.expectRevert();
        orderBook.openOrder(params);
    }

    function test_senderDoesNotHaveEnoughBalance_reverts() public {
        vm.prank(users[0]);
        tokens[0].approve(address(orderBook), params.amountIn);

        // Drain their balance
        vm.prank(users[0]);
        tokens[0].burn(MINT_AMOUNT);

        vm.prank(users[0]);
        vm.expectRevert();
        orderBook.openOrder(params);
    }

    function test_success() public {
        vm.prank(users[0]);
        tokens[0].approve(address(orderBook), params.amountIn);

        // Calculate the expected order ID before calling the method
        bytes32 expOrderId = orderBook.getOrderId(
            IOrderBook.OrderData({
                version: 1,
                originChainId: CHAIN_ID,
                sender: bytes32(uint256(uint160(users[0]))),
                nonce: 0,
                destChainId: params.destChainId,
                fillDeadline: params.fillDeadline,
                amountIn: params.amountIn,
                amountOut: params.amountOut,
                tokenOut: params.tokenOut,
                recipient: params.recipient,
                solver: params.solver
            })
        );

        vm.prank(users[0]);
        vm.expectEmit(true, true, true, true);
        emit IOrderBook.OrderOpen(expOrderId, params.tokenIn, params.amountIn, params.destChainId, params.tokenOut, params.amountOut, params.solver);
        bytes32 orderId = orderBook.openOrder(params);

        assertEq(orderId, expOrderId);

        // It transfers the amount in of the token in from the sender to the order book
        assertEq(tokens[0].balanceOf(address(orderBook)), params.amountIn);
        assertEq(tokens[0].balanceOf(users[0]), MINT_AMOUNT - params.amountIn);

        // It stores the order against the correct order ID
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created));
        assertEq(order.version, uint16(1));
        assertEq(order.destChainId, params.destChainId);
        assertEq(order.fillDeadline, params.fillDeadline);
        assertEq(order.nonce, 0);
        assertEq(order.tokenIn, params.tokenIn);
        assertEq(order.tokenOut, params.tokenOut);
        assertEq(order.sender, users[0]);
        assertEq(order.recipient, params.recipient);
        assertEq(order.amountIn, params.amountIn);
        assertEq(order.amountOut, params.amountOut);
        assertEq(order.solver, params.solver);
    }


}

