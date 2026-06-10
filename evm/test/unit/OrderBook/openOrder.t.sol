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
    // [X] given the sender field is zero address
    //   [X] it reverts with a ZeroSender error
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
    // [X] given the funder (msg.sender) is different from the sender field
    //   [X] it pulls tokens from the funder
    //   [X] it stores the order with sender as the owner
    //   [X] it increments the nonce for sender (not funder)
    //   [X] it emits OrderOpened with both funder and sender
    //   [X] the sender can cancel before deadline
    //   [X] the funder cannot cancel before deadline (if not sender or recipient)
    //   [X] the sender receives refunds on cancellation (not funder)

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
            users["alice"],
            params.tokenIn,
            params.amountIn,
            params.destChainId,
            params.tokenOut,
            params.amountOut,
            params.solver,
            params.fillDeadline
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

    // =========== Sender Field Tests ========== //

    function test_senderIsZeroAddress_reverts() public {
        params.sender = address(0);
        vm.prank(users["alice"]);
        tokenIn.approve(address(orderBook), params.amountIn);

        vm.prank(users["alice"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.ZeroSender.selector));
        orderBook.openOrder(params);
    }

    function test_funderDifferentFromSender_success() public {
        // Bob (funder) opens an order with Alice as sender
        address funder = users["bob"];
        address sender = users["alice"];
        params.sender = sender;

        // Calculate expected order ID (using Alice as sender, nonce 0)
        bytes32 expOrderId = _getOrderIdFromParams(sender, 0, params);

        // Bob approves and opens the order
        vm.prank(funder);
        tokenIn.approve(address(orderBook), params.amountIn);

        uint256 funderBalanceBefore = tokenIn.balanceOf(funder);
        uint256 senderBalanceBefore = tokenIn.balanceOf(sender);

        vm.prank(funder);
        vm.expectEmit(true, true, true, true);
        emit IOrderBook.OrderOpened(
            expOrderId,
            funder, // funder is Bob
            sender, // sender is Alice
            params.tokenIn,
            params.amountIn,
            params.destChainId,
            params.tokenOut,
            params.amountOut,
            params.solver,
            params.fillDeadline
        );
        bytes32 orderId = orderBook.openOrder(params);

        assertEq(orderId, expOrderId);

        // Tokens pulled from funder (Bob), not sender (Alice)
        assertEq(tokenIn.balanceOf(funder), funderBalanceBefore - params.amountIn);
        assertEq(tokenIn.balanceOf(sender), senderBalanceBefore); // unchanged

        // Order stored with Alice as sender
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(order.sender, sender);

        // Nonce incremented for sender (Alice), not funder (Bob)
        assertEq(orderBook.getSenderNonce(sender), 1);
        assertEq(orderBook.getSenderNonce(funder), 0);
    }

    function test_funderDifferentFromSender_nonceIncrementsForSender() public {
        address sender = users["alice"];
        params.sender = sender;

        // Initial nonces should be 0
        assertEq(orderBook.getSenderNonce(sender), 0);
        assertEq(orderBook.getSenderNonce(users["bob"]), 0);
        assertEq(orderBook.getSenderNonce(users["carol"]), 0);

        // Bob opens an order for Alice (Alice's nonce: 0 -> 1)
        vm.prank(users["bob"]);
        tokenIn.approve(address(orderBook), params.amountIn);
        vm.prank(users["bob"]);
        orderBook.openOrder(params);

        assertEq(orderBook.getSenderNonce(sender), 1);
        assertEq(orderBook.getSenderNonce(users["bob"]), 0);

        // Carol opens an order for Alice (Alice's nonce: 1 -> 2)
        vm.prank(users["carol"]);
        tokenIn.approve(address(orderBook), params.amountIn);
        vm.prank(users["carol"]);
        orderBook.openOrder(params);

        assertEq(orderBook.getSenderNonce(sender), 2);
        assertEq(orderBook.getSenderNonce(users["bob"]), 0);
        assertEq(orderBook.getSenderNonce(users["carol"]), 0);
    }

    function test_funderDifferentFromSender_senderReceivesRefund() public {
        // Setup same-chain order so refund happens immediately
        params.destChainId = CHAIN_ID;
        params.tokenOut = address(tokens["token-out-6D"]).toBytes32();

        address funder = users["bob"];
        address sender = users["alice"];
        params.sender = sender;

        // Bob opens the order
        vm.prank(funder);
        tokenIn.approve(address(orderBook), params.amountIn);
        vm.prank(funder);
        bytes32 orderId = orderBook.openOrder(params);

        uint256 funderBalanceBefore = tokenIn.balanceOf(funder);
        uint256 senderBalanceBefore = tokenIn.balanceOf(sender);

        // Alice (sender) cancels the order
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        vm.prank(sender);
        orderBook.cancelOrder(orderId, _getOrderDataFromOrder(orderId, order), new bytes(0));

        // Refund goes to sender (Alice), not funder (Bob)
        assertEq(tokenIn.balanceOf(sender), senderBalanceBefore + params.amountIn);
        assertEq(tokenIn.balanceOf(funder), funderBalanceBefore); // unchanged
    }

    function test_funderDifferentFromSender_funderCannotCancelBeforeDeadline_reverts() public {
        // Setup same-chain order
        params.destChainId = CHAIN_ID;
        params.tokenOut = address(tokens["token-out-6D"]).toBytes32();

        address funder = users["bob"];
        address sender = users["alice"];
        params.recipient = users["carol"].toBytes32(); // recipient is Carol
        params.sender = sender;

        // Bob opens the order
        vm.prank(funder);
        tokenIn.approve(address(orderBook), params.amountIn);
        vm.prank(funder);
        bytes32 orderId = orderBook.openOrder(params);

        // Bob (funder) attempts to cancel before deadline - should fail
        // Bob is neither sender (Alice) nor recipient (Carol)
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        vm.prank(funder);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.cancelOrder(orderId, _getOrderDataFromOrder(orderId, order), new bytes(0));
    }

    function test_funderDifferentFromSender_senderCanCancelBeforeDeadline_success() public {
        // Setup same-chain order
        params.destChainId = CHAIN_ID;
        params.tokenOut = address(tokens["token-out-6D"]).toBytes32();

        address funder = users["bob"];
        address sender = users["alice"];
        params.sender = sender;

        // Bob opens the order
        vm.prank(funder);
        tokenIn.approve(address(orderBook), params.amountIn);
        vm.prank(funder);
        bytes32 orderId = orderBook.openOrder(params);

        // Verify order is created
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created));

        // Alice (sender) cancels the order - should succeed
        vm.prank(sender);
        orderBook.cancelOrder(orderId, _getOrderDataFromOrder(orderId, order), new bytes(0));

        // Verify order is cancelled
        order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Cancelled));
    }
}
