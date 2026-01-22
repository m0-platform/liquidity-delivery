// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";
import { PausableUpgradeable } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";

import { OrderBookTestBase } from "./OrderBookTestBase.t.sol";
import { IOrderBook } from "../../../src/interfaces/IOrderBook.sol";

contract FillOrderTest is OrderBookTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the contract is paused
    //    [X] it reverts with an EnforcedPause error
    // [X] given the destination chain ID of the order is not the current chain ID
    //   [X] it reverts with an InvalidDestinationChain error
    // [X] given the current timestamp is > fill deadline
    //   [X] it reverts with an OrderExpired error
    // [X] given the order version does not match the version of the order stored
    //   [X] it reverts with an InvalidOrderVersion error
    // [X] given the order has a specified solver and the caller is not the solver
    //   [X] it reverts with a NotAuthorized error
    // [X] given the hash of the order data does not match the order id
    //   [X] it reverts with an OrderIdMismatch error
    // [X] given the order has already been filled (local)
    //   [X] it reverts with an OrderFilled error
    // [X] given the fill amount is zero
    //   [X] it reverts with a FillAmountZero error
    // [X] given the order originated on the current chain (i.e. it is local)
    //   [X] given both tokens have 6 decimals
    //     [X] given the fill amount is equal to the amount out remaining to fill
    //       [X] it updates the order status to Completed
    //       [X] it emits an OrderCompleted event
    //       [X] it transfers the amount out remaining to be filled from the caller to the recipient
    //       [X] it transfers the amount in remaining to the caller
    //       [X] it emits a Fill event
    //     [X] given the fill amount is greater than the amount out remaining to fill
    //       [X] the fill amount is reduced to the remaining amount out to fill
    //       [X] it updates the order status to Completed
    //       [X] it emits an OrderCompleted event
    //       [X] it transfers the amount out remaining to be filled from the caller to the recipient
    //       [X] it transfers the amount in remaining to the caller
    //       [X] it emits a Fill event
    //     [X] given the fill amount is less than the amount out remaining to fill
    //       [X] it transfers the fill amount out from the caller to the recipient
    //       [X] it transfers a pro-rata amount in to the caller
    //       [X] it emits a Fill event
    //   [X] given token in has a smaller number of decimals than token out (6 vs. 18)
    //     [X] same cases as above
    //   [X] given token in has a larger number of decimals than token out (18 vs. 6)
    //     [X] same cases as above
    //   [X] given both tokens have 18 decimals
    //     [X] same cases as above
    // [X] given the order originated on a different chain (i.e. it is cross-chain)
    //    [X] given both tokens have 6 decimals
    //      [X] given the fill amount is equal to the amount out remaining
    //        [X] it transfers the amount out remaining to be filled from the caller to the recipient
    //        [X] it sends a fill report to the origin chain via the portal
    //        [X] it emits a Fill event
    //        [X] given the fill amount is greater than the amount out remaining to fill
    //     [X] the fill amount is reduced to the remaining amount out to fill
    //       [X] it transfers the amount out remaining to be filled from the caller to the
    //       [X] it sends a fill report to the origin chain via the portal
    //       [X] it emits a Fill event
    //     [X] given the fill amount is less than the amount out remaining to fill
    //       [X] it transfers the fill amount out from the caller to the recipient
    //       [X] it sends a fill report to the origin chain via the portal
    //       [X] it emits a Fill event
    //   [X] given token in has a smaller number of decimals than token out (6 vs. 18)
    //     [X] same cases as above
    //   [X] given token in has a larger number of decimals than token out (18 vs. 6)
    //     [X] same cases as above
    //   [X] given both tokens have 18 decimals
    //     [X] same cases as above
    // [X] given the order has no specified solver
    //     [X] anyone can fill the order

    function setUp() public override {
        super.setUp();

        // Approve the orderbook to spend tokenOut for the designated solver (params.solver.toAddress())
        vm.startPrank(params.solver.toAddress());
        tokens["token-out-6D"].approve(address(orderBook), type(uint256).max);
        tokens["token-out-18D"].approve(address(orderBook), type(uint256).max);
        vm.stopPrank();
    }

    function test_destinationChainIdNotCurrentChain_reverts() public {
        // Create an order destined for chain 2 (use default params with destChainId=2)
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Try to fill it on chain 1 (current chain) - should fail because destChainId is 2
        vm.prank(params.solver.toAddress());
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDestinationChain.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId, // This is DEST_CHAIN_ID (2), not CHAIN_ID (1)
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver })
        );
    }

    function test_fillDeadlineHasPassed_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp time past the fill deadline
        vm.warp(order.fillDeadline + 1);

        vm.prank(params.solver.toAddress());
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderExpired.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver })
        );
    }

    function test_orderVersionMismatch_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Calculate order ID with wrong version to check version mismatch
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 999, // Wrong version
            sender: order.sender.toBytes32(),
            nonce: order.nonce,
            originChainId: CHAIN_ID,
            destChainId: order.destChainId,
            createdAt: uint64(order.createdAt),
            fillDeadline: uint64(order.fillDeadline),
            tokenIn: order.tokenIn.toBytes32(),
            tokenOut: order.tokenOut,
            amountIn: order.amountIn,
            amountOut: order.amountOut,
            recipient: order.recipient,
            solver: order.solver
        });
        orderId = orderBook.getOrderId(orderData);

        vm.prank(params.solver.toAddress());
        tokenOut.approve(address(orderBook), type(uint256).max);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderVersion.selector));
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver })
        );
    }

    function test_callerNotDesignatedSolver_reverts() public {
        // Create a local order with a designated solver (params.solver.toAddress())
        params.destChainId = CHAIN_ID;
        params.solver = params.solver;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Try to fill as "bob" (not the designated solver)
        vm.prank(users["bob"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: users["bob"].toBytes32() })
        );
    }

    function test_orderIdMismatchWithOrderData_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        vm.prank(params.solver.toAddress());
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderIdMismatch.selector));
        orderBook.fillOrder(
            bytes32("wrong order id"), // Wrong order ID
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver })
        );
    }

    function test_orderAlreadyFilledLocal_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        params.solver = bytes32(0); // No designated solver to avoid NotAuthorized
        bytes32 orderId = _placeOrder(users["alice"], params);

        // Fill it completely
        _fillOrder(users["bob"], orderId, params.amountOut);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Try to fill it again
        vm.prank(users["solver"]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderStatus.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: users["bob"].toBytes32() })
        );
    }

    function test_fillAmountZero_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Try to fill the order with a zero fill amount, expect revert
        vm.prank(params.solver.toAddress());
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.FillAmountZero.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: 0, originRecipient: params.solver })
        );
    }

    function _test_localOrderFullFill_success() internal {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Record balances before fill
        uint256 solverTokenOutBefore = tokenOut.balanceOf(params.solver.toAddress());
        uint256 recipientTokenOutBefore = tokenOut.balanceOf(users["alice"]);
        uint256 solverTokenInBefore = tokenIn.balanceOf(params.solver.toAddress());
        uint256 orderBookTokenInBefore = tokenIn.balanceOf(address(orderBook));

        // Fill the order completely
        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), order.amountIn, order.amountOut);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver })
        );

        // Check order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");

        // Check token transfers
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - order.amountOut,
            "solver should have sent tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + order.amountOut,
            "recipient should have received tokenOut"
        );
        assertEq(
            tokenIn.balanceOf(params.solver.toAddress()),
            solverTokenInBefore + order.amountIn,
            "solver should have received tokenIn"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookTokenInBefore - order.amountIn,
            "orderBook should have released tokenIn"
        );
    }

    function test_bothSixDecimals_localOrderFullFill_success() public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _test_localOrderFullFill_success();
    }

    function test_tokenInSmallerDecimals_localOrderFullFill_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        _test_localOrderFullFill_success();
    }

    function test_tokenInLargerDecimals_localOrderFullFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(6)
    {
        _test_localOrderFullFill_success();
    }

    function test_bothEighteenDecimals_localOrderFullFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(18)
    {
        _test_localOrderFullFill_success();
    }

    function _testFuzz_localOrderOverfill_success(uint128 fillAmount) internal {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Cap the overfill to 2x the amountOut plus 1 (so we don't have 0)
        fillAmount = (fillAmount % (order.amountOut - 1)) + 1 + order.amountOut;

        // Record balances before fill
        uint256 solverTokenOutBefore = tokenOut.balanceOf(params.solver.toAddress());
        uint256 recipientTokenOutBefore = tokenOut.balanceOf(users["alice"]);
        uint256 solverTokenInBefore = tokenIn.balanceOf(params.solver.toAddress());
        uint256 orderBookTokenInBefore = tokenIn.balanceOf(address(orderBook));

        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), order.amountIn, order.amountOut);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: fillAmount, originRecipient: params.solver })
        );

        // Check order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");

        // Check token transfers
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - order.amountOut,
            "solver should have sent tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + order.amountOut,
            "recipient should have received tokenOut"
        );
        assertEq(
            tokenIn.balanceOf(params.solver.toAddress()),
            solverTokenInBefore + order.amountIn,
            "solver should have received tokenIn"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookTokenInBefore - order.amountIn,
            "order book should have sent tokenIn"
        );
    }

    function testFuzz_bothSixDecimals_localOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _testFuzz_localOrderOverfill_success(fillAmount);
    }

    function testFuzz_tokenInSmallerDecimals_localOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _testFuzz_localOrderOverfill_success(fillAmount);
    }

    function testFuzz_tokenInLargerDecimals_localOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _testFuzz_localOrderOverfill_success(fillAmount);
    }

    function testFuzz_bothEighteenDecimals_localOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _testFuzz_localOrderOverfill_success(fillAmount);
    }

    function _testFuzz_localOrderPartialFill_success(uint128 fillAmount) internal {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Set the fill amount to a number less than amount out, but not zero
        fillAmount = (fillAmount % (order.amountOut - 1)) + 1;
        uint128 expectedAmountIn = uint128((uint256(order.amountIn) * fillAmount) / order.amountOut);

        // Record balances before fill
        uint256 solverTokenOutBefore = tokenOut.balanceOf(params.solver.toAddress());
        uint256 recipientTokenOutBefore = tokenOut.balanceOf(users["alice"]);
        uint256 solverTokenInBefore = tokenIn.balanceOf(params.solver.toAddress());
        uint256 orderBookTokenInBefore = tokenIn.balanceOf(address(orderBook));

        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), expectedAmountIn, fillAmount);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: fillAmount, originRecipient: params.solver })
        );

        // Check order status - should still be Created, not Completed
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Created), "order should still be Created");

        // Check token transfers
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - fillAmount,
            "solver should have sent partial tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + fillAmount,
            "recipient should have received partial tokenOut"
        );
        assertEq(
            tokenIn.balanceOf(params.solver.toAddress()),
            solverTokenInBefore + expectedAmountIn,
            "solver should have received pro-rata tokenIn"
        );
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookTokenInBefore - expectedAmountIn,
            "orderBook should have released pro-rata tokenIn"
        );
    }

    function testFuzz_bothSixDecimals_localOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _testFuzz_localOrderPartialFill_success(fillAmount);
    }

    function testFuzz_tokenInSmallerDecimals_localOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _testFuzz_localOrderPartialFill_success(fillAmount);
    }

    function testFuzz_tokenInLargerDecimals_localOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _testFuzz_localOrderPartialFill_success(fillAmount);
    }

    function testFuzz_bothEighteenDecimals_localOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _testFuzz_localOrderPartialFill_success(fillAmount);
    }

    function _test_crossChainOrderFullFill_success() internal {
        // Create order data for a cross-chain order (originated on chain 2, destined for chain 1)
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order was created on chain 2
            sender: users["alice"].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // To be filled on chain 1 (current chain)
            createdAt: uint64(block.timestamp),
            fillDeadline: uint64(block.timestamp + FILL_DURATION),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: address(tokenOut).toBytes32(),
            recipient: users["alice"].toBytes32(),
            solver: params.solver
        });

        bytes32 orderId = orderBook.getOrderId(orderData);

        // Record balances before fill
        uint256 solverTokenOutBefore = tokenOut.balanceOf(params.solver.toAddress());
        uint256 recipientTokenOutBefore = tokenOut.balanceOf(users["alice"]);

        // Fill the order on the destination chain
        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), orderData.amountIn, orderData.amountOut);
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({ amountOutToFill: orderData.amountOut, originRecipient: params.solver })
        );

        // Check token transfers on destination chain
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - orderData.amountOut,
            "solver should have sent tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + orderData.amountOut,
            "recipient should have received tokenOut"
        );
        assertTrue(portal.isFillReported(orderId), "fill report should have been sent to origin chain");
    }

    function test_bothSixDecimals_crossChainOrderFullFill_success()
        public
        givenTokenOutDecimals(6)
        givenTokenOutDecimals(6)
    {
        _test_crossChainOrderFullFill_success();
    }

    function test_tokenInSmallerDecimals_crossChainOrderFullFill_success()
        public
        givenTokenInDecimals(6)
        givenTokenOutDecimals(18)
    {
        _test_crossChainOrderFullFill_success();
    }

    function test_tokenInLargerDecimals_crossChainOrderFullFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(6)
    {
        _test_crossChainOrderFullFill_success();
    }

    function test_bothEighteenDecimals_crossChainOrderFullFill_success()
        public
        givenTokenInDecimals(18)
        givenTokenOutDecimals(18)
    {
        _test_crossChainOrderFullFill_success();
    }

    function _testFuzz_crossChainOrderOverfill_success(uint128 fillAmount) internal {
        // Create order data for a cross-chain order (originated on chain 2, destined for chain 1)
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order was created on chain 2
            sender: users["alice"].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // To be filled on chain 1 (current chain)
            createdAt: uint64(block.timestamp),
            fillDeadline: uint64(block.timestamp + FILL_DURATION),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: address(tokenOut).toBytes32(),
            recipient: users["alice"].toBytes32(),
            solver: params.solver
        });

        bytes32 orderId = orderBook.getOrderId(orderData);

        // Set the fill amount to at most 2x the order amount
        fillAmount = (fillAmount % (AMOUNT_OUT - 1)) + AMOUNT_OUT + 1;

        // Record balances before fill
        uint256 solverTokenOutBefore = tokenOut.balanceOf(params.solver.toAddress());
        uint256 recipientTokenOutBefore = tokenOut.balanceOf(users["alice"]);

        // Fill the order on the destination chain
        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), orderData.amountIn, orderData.amountOut);
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({ amountOutToFill: fillAmount, originRecipient: params.solver })
        );

        // Check token transfers on destination chain
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - orderData.amountOut,
            "solver should have sent tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + orderData.amountOut,
            "recipient should have received tokenOut"
        );
        assertTrue(portal.isFillReported(orderId), "fill report should have been sent to origin chain");
    }

    function test_fuzz_bothSixDecimals_crossChainOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _testFuzz_crossChainOrderOverfill_success(fillAmount);
    }

    function testFuzz_tokenInSmallerDecimals_crossChainOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _testFuzz_crossChainOrderOverfill_success(fillAmount);
    }

    function testFuzz_tokenInLargerDecimals_crossChainOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _testFuzz_crossChainOrderOverfill_success(fillAmount);
    }

    function test_fuzz_bothEighteenDecimals_crossChainOrderOverfill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _testFuzz_crossChainOrderOverfill_success(fillAmount);
    }

    function _testFuzz_crossChainOrderPartialFill_success(uint128 fillAmount) public {
        // Create order data for a cross-chain order (originated on chain 2, destined for chain 1)
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order was created on chain 2
            sender: users["alice"].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // To be filled on chain 1 (current chain)
            createdAt: uint64(block.timestamp),
            fillDeadline: uint64(block.timestamp + FILL_DURATION),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: address(tokenOut).toBytes32(),
            recipient: users["alice"].toBytes32(),
            solver: params.solver
        });

        bytes32 orderId = orderBook.getOrderId(orderData);

        // Fill between 1 and the order amount - 1
        fillAmount = (fillAmount % (AMOUNT_OUT - 1)) + 1;
        uint128 expectedAmountIn = uint128((uint256(AMOUNT_IN) * fillAmount) / AMOUNT_OUT);

        // Record balances before fill
        uint256 solverTokenOutBefore = tokenOut.balanceOf(params.solver.toAddress());
        uint256 recipientTokenOutBefore = tokenOut.balanceOf(users["alice"]);

        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), expectedAmountIn, fillAmount);
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({ amountOutToFill: fillAmount, originRecipient: params.solver })
        );

        // Check token transfers on destination chain
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - fillAmount,
            "solver should have sent partial tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + fillAmount,
            "recipient should have received partial tokenOut"
        );
        assertTrue(portal.isFillReported(orderId), "fill report should have been sent to origin chain");
    }

    function testFuzz_bothSixDecimals_crossChainOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _testFuzz_crossChainOrderPartialFill_success(fillAmount);
    }

    function testFuzz_tokenInSmallerDecimals_crossChainOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _testFuzz_crossChainOrderPartialFill_success(fillAmount);
    }

    function testFuzz_tokenInLargerDecimals_crossChainOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _testFuzz_crossChainOrderPartialFill_success(fillAmount);
    }

    function testFuzz_bothEighteenDecimals_crossChainOrderPartialFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _testFuzz_crossChainOrderPartialFill_success(fillAmount);
    }

    function test_solverNotSpecified_anyoneCanFill_success(address solver) public {
        vm.assume(solver != address(0) && solver != address(orderBook) && solver != users["alice"]);

        vm.deal(solver, 1 ether);
        tokenOut.mint(solver, MINT_AMOUNT);

        // Create order data for a cross-chain order (originated on chain 2, destined for chain 1)
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order was created on chain 2
            sender: users["alice"].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // To be filled on chain 1 (current chain)
            createdAt: uint64(block.timestamp),
            fillDeadline: uint64(block.timestamp + FILL_DURATION),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            tokenIn: address(tokenIn).toBytes32(),
            tokenOut: address(tokenOut).toBytes32(),
            recipient: users["alice"].toBytes32(),
            solver: address(0).toBytes32() // No designated solver
        });

        bytes32 orderId = orderBook.getOrderId(orderData);

        vm.startPrank(solver);
        tokenOut.approve(address(orderBook), type(uint256).max);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, solver, orderData.amountIn, orderData.amountOut);
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({ amountOutToFill: orderData.amountOut, originRecipient: solver.toBytes32() })
        );
        vm.stopPrank();
    }

    function _testFuzz_multiPartFill_success(uint128 fillAmount) internal {
        // Use a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Set the fill amount for the first fill to be a random value between 1 and amount out - 1
        fillAmount = (fillAmount % (order.amountOut - 1)) + 1;
        uint128 expectedAmountIn = uint128((uint256(order.amountIn) * fillAmount) / order.amountOut);

        // Cache the starting balances
        uint256 orderBookTokenInBefore = tokenIn.balanceOf(address(orderBook));
        uint256 solverTokenInBefore = tokenIn.balanceOf(params.solver.toAddress());
        uint256 recipientTokenOutBefore = tokenOut.balanceOf(users["alice"]);
        uint256 solverTokenOutBefore = tokenOut.balanceOf(params.solver.toAddress());

        // Submit the initial fill
        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), expectedAmountIn, fillAmount);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: fillAmount, originRecipient: order.solver })
        );

        // Check balances after the first fill
        assertEq(
            tokenIn.balanceOf(address(orderBook)),
            orderBookTokenInBefore - expectedAmountIn,
            "order book should have sent pro-rata tokenIn"
        );
        assertEq(
            tokenIn.balanceOf(params.solver.toAddress()),
            solverTokenInBefore + expectedAmountIn,
            "solver should have received pro-rata tokenIn"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + fillAmount,
            "recipient should have received partial tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - fillAmount,
            "solver should have sent partial tokenOut"
        );

        // Submit the second fill to complete the order
        uint128 remainingAmountOut = order.amountOut - fillAmount;
        uint128 remainingAmountIn = order.amountIn - expectedAmountIn;
        vm.prank(params.solver.toAddress());
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderFilled(orderId, params.solver.toAddress(), remainingAmountIn, remainingAmountOut);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: remainingAmountOut, originRecipient: order.solver })
        );

        // Check final balances
        assertEq(tokenIn.balanceOf(address(orderBook)), 0, "order book should have sent all tokenIn");
        assertEq(
            tokenIn.balanceOf(params.solver.toAddress()),
            solverTokenInBefore + order.amountIn,
            "solver should have received all tokenIn"
        );
        assertEq(
            tokenOut.balanceOf(users["alice"]),
            recipientTokenOutBefore + order.amountOut,
            "recipient should have received all tokenOut"
        );
        assertEq(
            tokenOut.balanceOf(params.solver.toAddress()),
            solverTokenOutBefore - order.amountOut,
            "solver should have sent all tokenOut"
        );
    }

    function testFuzz_bothSixDecimals_multiPartFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(6) {
        _testFuzz_multiPartFill_success(fillAmount);
    }

    function testFuzz_tokenInSmallerDecimals_multiPartFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(6) givenTokenOutDecimals(18) {
        _testFuzz_multiPartFill_success(fillAmount);
    }

    function testFuzz_tokenInLargerDecimals_multiPartFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(6) {
        _testFuzz_multiPartFill_success(fillAmount);
    }

    function testFuzz_bothEighteenDecimals_multiPartFill_success(
        uint128 fillAmount
    ) public givenTokenInDecimals(18) givenTokenOutDecimals(18) {
        _testFuzz_multiPartFill_success(fillAmount);
    }

    function test_onFillDeadline_success() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);
        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Fast forward to fill deadline
        vm.warp(order.fillDeadline);

        vm.prank(users["solver"]);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: uint64(order.createdAt),
                fillDeadline: uint64(order.fillDeadline),
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver })
        );

        // Check order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");
    }

    function test_whenPaused_reverts() public {
        // Create a local order before pausing
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Pause the contract
        vm.prank(pauser);
        orderBook.pause();

        // Attempt to fill
        vm.prank(params.solver.toAddress());
        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: order.createdAt,
                fillDeadline: order.fillDeadline,
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver })
        );
    }

    function test_fillOrderWithMessageData_whenPaused_reverts() public {
        // Create a local order before pausing
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users["alice"], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Pause the contract
        vm.prank(pauser);
        orderBook.pause();

        // Attempt to fill with messageData
        vm.prank(params.solver.toAddress());
        vm.expectRevert(abi.encodeWithSelector(PausableUpgradeable.EnforcedPause.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                createdAt: order.createdAt,
                fillDeadline: order.fillDeadline,
                amountIn: order.amountIn,
                amountOut: order.amountOut,
                tokenIn: order.tokenIn.toBytes32(),
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({ amountOutToFill: order.amountOut, originRecipient: params.solver }),
            new bytes(0)
        );
    }
}
