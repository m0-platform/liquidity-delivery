// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";
import { PausableUpgradeable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

contract OpenOrderTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the contract is paused
    //    [X] it reverts with an EnforcedPause error
    // [X] given the fill deadline is before the current block timestamp
    //    [X] it reverts with an InvalidDeadline error
    // [X] given the amount in is zero
    //    [X] it reverts with an AmountInZero error
    // [X] given the amount out is zero
    //    [X] it reverts with an AmountOutZero error
    // [X] given the destination chain is invalid
    //   [X] it reverts with an InvalidDestinationChain error
    // [X] given the solver is the recipient
    //   [X] it reverts with an InvalidSolver error
    // [X] given a same-chain order with tokenOut equal to tokenIn
    //   [X] it reverts with a SameTokenOrder error
    // [X] given the sender has not approved the order book to spend their token in
    //   [X] it reverts with an ERC20 transfer error
    // [X] given the sender has not enough balance of the token in
    //   [X] it reverts with an ERC20 transfer error
    // [X] given the order is valid, the sender has approved the order book to spend their token in, and the sender has enough balance of the token in
    //   [X] given the token in and token out both have 6 decimals
    //     [X] it transfers the amount in of the token in from the sender to the order book
    //     [X] it stores the order against the correct order ID
    //     [X] it emits an OrderOpened event
    //     [X] it returns the order ID
    //  [X] given token in has 6 decimals and token out has 18 decimals
    //     [X] it transfers the amount in of the token in from the sender to the order book
    //     [X] it stores the order against the correct order ID
    //     [X] it emits an OrderOpened event
    //     [X] it returns the order ID
    //  [X] given token in has 18 decimals and token out has 6 decimals
    //     [X] it transfers the amount in of the token in from the sender to
    //     [X] it stores the order against the correct order ID
    //     [X] it emits an OrderOpened event
    //     [X] it returns the order ID
    //  [X] given both token in and token out have 18 decimals
    //     [X] it transfers the amount in of the token in from the sender to
    //     [X] it stores the order against the correct order ID
    //     [X] it emits an OrderOpened event
    //     [X] it returns the order ID

    function test_fillDeadlineBeforeCurrentTime_reverts() public {
        params.fillDeadline = uint32(block.timestamp - 1);
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDeadline.selector));
        orderBook.openOrder(params);
    }

    function test_amountInIsZero_reverts() public {
        params.amountIn = 0;
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.AmountInZero.selector));
        orderBook.openOrder(params);
    }

    function test_amountOutIsZero_reverts() public {
        params.amountOut = 0;
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.AmountOutZero.selector));
        orderBook.openOrder(params);
    }

    function test_destinationChainIsInvalid_reverts() public {
        params.destChainId = 100;
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDestinationChain.selector));
        orderBook.openOrder(params);
    }

    function test_solverIsRecipient_reverts() public {
        params.solver = params.recipient;
        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidSolver.selector));
        orderBook.openOrder(params);
    }

    function test_sameChainOrderWithSameToken_reverts() public {
        // Set to same-chain order
        params.destChainId = CHAIN_ID;
        // Set tokenOut to be the same as tokenIn
        params.tokenOut = address(tokenIn).toBytes32();

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.SameTokenOrder.selector));
        orderBook.openOrder(params);
    }

    function test_senderHasNotApprovedOrderBook_reverts() public {
        vm.prank(users["alice"]);
        vm.expectRevert();
        orderBook.openOrder(params);
    }

    function test_senderDoesNotHaveEnoughBalance_reverts() public {
        vm.prank(users["alice"]);
        tokenIn.approve(address(orderBook), params.amountIn);

        // Drain their balance
        uint256 balance = tokenIn.balanceOf(users["alice"]);
        vm.prank(users["alice"]);
        tokenIn.burn(balance);

        vm.prank(users["alice"]);
        vm.expectRevert();
        orderBook.openOrder(params);
    }

    function _test_success() internal {
        vm.prank(users["alice"]);
        tokenIn.approve(address(orderBook), params.amountIn);

        // Calculate the expected order ID before calling the method
        bytes32 expOrderId = _getOrderIdFromParams(users["alice"], 0, params);

        vm.prank(users["alice"]);
        vm.expectEmit(true, true, true, true);
        emit IOrderBook.OrderOpened(
            expOrderId,
            users["alice"],
            params.tokenIn,
            params.amountIn,
            params.destChainId,
            params.tokenOut,
            params.amountOut,
            params.solver
        );
        bytes32 orderId = orderBook.openOrder(params);

        assertEq(orderId, expOrderId);

        // It transfers the amount in of the token in from the sender to the order book
        assertEq(tokenIn.balanceOf(address(orderBook)), params.amountIn);
        assertEq(tokenIn.balanceOf(users["alice"]), MINT_AMOUNT * 10 ** tokenIn.decimals() - params.amountIn);

        // It stores the order against the correct order ID
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created));
        assertEq(order.version, uint16(1));
        assertEq(order.destChainId, params.destChainId);
        assertEq(order.fillDeadline, params.fillDeadline);
        assertEq(order.nonce, 0);
        assertEq(order.tokenIn, params.tokenIn);
        assertEq(order.tokenOut, params.tokenOut);
        assertEq(order.sender, users["alice"]);
        assertEq(order.recipient, params.recipient);
        assertEq(order.amountIn, params.amountIn);
        assertEq(order.amountOut, params.amountOut);
        assertEq(order.solver, params.solver);
    }

    function test_givenBothTokensHaveSixDecimals_success() public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _test_success();
    }

    function test_givenTokenInDecimalsSmallerThanTokenOut_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        _test_success();
    }

    function test_givenTokenInDecimalsLargerThanTokenOut_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(6)
    {
        _test_success();
    }

    function test_givenBothTokensHaveEighteenDecimals_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(18)
    {
        _test_success();
    }

    function test_whenPaused_reverts() public {
        vm.prank(users["alice"]);
        tokenIn.approve(address(orderBook), params.amountIn);

        vm.prank(pauser);
        orderBook.pause();

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.openOrder(params);
    }

    function test_openOrderWithPermit_whenPaused_reverts() public {
        vm.prank(pauser);
        orderBook.pause();

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.openOrderWithPermit(params, block.timestamp + 1 hours, 0, bytes32(0), bytes32(0));
    }
}
