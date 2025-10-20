// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { UnitTestBase } from "./UnitTestBase.t.sol";
import { IOrderBook } from "../../src/interfaces/IOrderBook.sol";
import { TypeConverter } from "../../src/libs/TypeConverter.sol";

contract FillOrderTest is UnitTestBase {
    using TypeConverter for *;

    // Test cases
    // [X] given the destination chain ID of the order is not the current chain ID
    //   [X] it reverts with an InvalidDestinationChain error
    // [X] given the fill deadline has passed
    //   [X] it reverts with an OrderExpired error
    // [X] given the order version does not match the version of the order stored
    //   [X] it reverts with an InvalidOrderVersion error
    // [X] given the order has a specified solver and the caller is not the solver
    //   [X] it reverts with a NotAuthorized error
    // [X] given the hash of the order data does not match the order id
    //   [X] it reverts with an OrderIdMismatch error
    // [X] given the order has already been filled (local)
    //   [X] it reverts with an OrderFilled error
    // [X] given the order originated on the current chain (i.e. it is local)
    //   [X] given the fill amount is greater than or equal to the amount out remaining to fill
    //     [X] it updates the order status to Completed
    //     [X] it emits an OrderCompleted event
    //     [X] it transfers the amount out remaining to be filled from the caller to the recipient
    //     [X] it transfers the amount in remaining to the caller
    //     [X] it emits a Fill event
    //   [X] given the fill amount is less than the amount out remaining to fill
    //     [X] it transfers the fill amount out from the caller to the recipient
    //     [X] it transfers a pro-rata amount in to the caller
    //     [X] it emits a Fill event
    // [X] given the order originated on a different chain (i.e. it is cross-chain)
    //   [X] given the fill amount is equal to the amount out remaining
    //     [X] it transfers the amount out remaining to be filled from the caller to the recipient
    //     [X] it sends a fill report to the origin chain via the messenger
    //     [X] it emits a Fill event
    //   [ ] given the fill amount is greater than the amount out remaining to fill
    //     [ ] the fill amount is reduced to the remaining amount out to fill
    //     [ ] it transfers the amount out remaining to be filled from the caller to the
    //     [ ] it sends a fill report to the origin chain via the messenger
    //     [ ] it emits a Fill event
    //   [X] given the fill amount is less than the amount out remaining to fill
    //     [X] it transfers the fill amount out from the caller to the recipient
    //     [X] it sends a fill report to the origin chain via the messenger
    //     [X] it emits a Fill event
    // [X] given the order has no specified solver
    //     [X] anyone can fill the order

    function setUp() public override {
        super.setUp();

        // Approve the orderbook to spend tokenOut for the designated solver (users[2])
        vm.prank(users[2]);
        tokens[1].approve(address(orderBook), type(uint256).max);
    }

    function test_destinationChainIdNotCurrentChain_reverts() public {
        // Create an order destined for chain 2 (use default params with destChainId=2)
        params.solver = bytes32(0); // No designated solver to avoid NotAuthorized
        bytes32 orderId = _placeOrder(users[0], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Try to fill it on chain 1 (current chain) - should fail because destChainId is 2
        vm.prank(users[2]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidDestinationChain.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId, // This is DEST_CHAIN_ID (2), not CHAIN_ID (1)
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: order.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_fillDeadlineHasPassed_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        params.solver = bytes32(0); // No designated solver to avoid NotAuthorized
        bytes32 orderId = _placeOrder(users[0], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Warp time past the fill deadline
        vm.warp(order.fillDeadline + 1);

        vm.prank(users[2]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderExpired.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: order.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_orderVersionMismatch_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        params.solver = bytes32(0); // No designated solver to avoid NotAuthorized
        bytes32 orderId = _placeOrder(users[0], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        vm.prank(users[2]);
        tokens[1].approve(address(orderBook), type(uint256).max);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.InvalidOrderVersion.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: 999, // Wrong version
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: order.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_callerNotDesignatedSolver_reverts() public {
        // Create a local order with a designated solver (users[2])
        params.destChainId = CHAIN_ID;
        params.solver = users[2].toBytes32();
        bytes32 orderId = _placeOrder(users[0], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Try to fill as users[1] (not the designated solver)
        vm.prank(users[1]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.NotAuthorized.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: order.amountOut,
                originRecipient: users[1].toBytes32()
            })
        );
    }

    function test_orderIdMismatchWithOrderData_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        params.solver = bytes32(0); // No designated solver to avoid NotAuthorized
        bytes32 orderId = _placeOrder(users[0], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        vm.prank(users[2]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderIdMismatch.selector));
        orderBook.fillOrder(
            bytes32("wrong order id"), // Wrong order ID
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: order.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_orderAlreadyFilledLocal_reverts() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        params.solver = bytes32(0); // No designated solver to avoid NotAuthorized
        bytes32 orderId = _placeOrder(users[0], params);

        // Fill it completely
        _fillOrder(users[2], orderId, params.amountOut);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Try to fill it again
        vm.prank(users[2]);
        vm.expectRevert(abi.encodeWithSelector(IOrderBook.OrderFilled.selector));
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: order.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );
    }

    function test_localOrderFullFill_success() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users[0], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Record balances before fill
        uint256 solverTokenOutBefore = tokens[1].balanceOf(users[2]);
        uint256 recipientTokenOutBefore = tokens[1].balanceOf(users[0]);
        uint256 solverTokenInBefore = tokens[0].balanceOf(users[2]);
        uint256 orderBookTokenInBefore = tokens[0].balanceOf(address(orderBook));

        // Fill the order completely
        vm.prank(users[2]);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.OrderCompleted(orderId);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.Fill(orderId, users[2], order.amountOut);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: order.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );

        // Check order status
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Completed), "order should be completed");

        // Check token transfers
        assertEq(tokens[1].balanceOf(users[2]), solverTokenOutBefore - order.amountOut, "solver should have sent tokenOut");
        assertEq(tokens[1].balanceOf(users[0]), recipientTokenOutBefore + order.amountOut, "recipient should have received tokenOut");
        assertEq(tokens[0].balanceOf(users[2]), solverTokenInBefore + order.amountIn, "solver should have received tokenIn");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookTokenInBefore - order.amountIn, "orderBook should have released tokenIn");
    }

    function test_localOrderPartialFill_success() public {
        // Create a local order
        params.destChainId = CHAIN_ID;
        bytes32 orderId = _placeOrder(users[0], params);

        IOrderBook.Order memory order = orderBook.getOrder(orderId);

        // Fill half of the order
        uint128 fillAmount = order.amountOut / 2;
        uint128 expectedAmountIn = uint128((uint256(order.amountIn) * fillAmount) / order.amountOut);

        // Record balances before fill
        uint256 solverTokenOutBefore = tokens[1].balanceOf(users[2]);
        uint256 recipientTokenOutBefore = tokens[1].balanceOf(users[0]);
        uint256 solverTokenInBefore = tokens[0].balanceOf(users[2]);
        uint256 orderBookTokenInBefore = tokens[0].balanceOf(address(orderBook));

        vm.prank(users[2]);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.Fill(orderId, users[2], fillAmount);
        orderBook.fillOrder(
            orderId,
            IOrderBook.OrderData({
                version: order.version,
                originChainId: CHAIN_ID,
                sender: order.sender.toBytes32(),
                nonce: order.nonce,
                destChainId: order.destChainId,
                fillDeadline: order.fillDeadline,
                amountOut: order.amountOut,
                tokenOut: order.tokenOut,
                recipient: order.recipient,
                solver: order.solver
            }),
            IOrderBook.FillParams({
                amountOutToFill: fillAmount,
                originRecipient: users[2].toBytes32()
            })
        );

        // Check order status - should still be Created, not Completed
        IOrderBook.Order memory updatedOrder = orderBook.getOrder(orderId);
        assertEq(uint8(updatedOrder.status), uint8(IOrderBook.OrderStatus.Created), "order should still be Created");

        // Check token transfers
        assertEq(tokens[1].balanceOf(users[2]), solverTokenOutBefore - fillAmount, "solver should have sent partial tokenOut");
        assertEq(tokens[1].balanceOf(users[0]), recipientTokenOutBefore + fillAmount, "recipient should have received partial tokenOut");
        assertEq(tokens[0].balanceOf(users[2]), solverTokenInBefore + expectedAmountIn, "solver should have received pro-rata tokenIn");
        assertEq(tokens[0].balanceOf(address(orderBook)), orderBookTokenInBefore - expectedAmountIn, "orderBook should have released pro-rata tokenIn");
    }

    function test_crossChainOrderFullFill_success() public {
        // Create order data for a cross-chain order (originated on chain 2, destined for chain 1)
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order was created on chain 2
            sender: users[0].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // To be filled on chain 1 (current chain)
            fillDeadline: uint64(block.timestamp + FILL_DURATION),
            amountOut: AMOUNT_OUT,
            tokenOut: address(tokens[1]).toBytes32(),
            recipient: users[0].toBytes32(),
            solver: users[2].toBytes32()
        });

        bytes32 orderId = orderBook.getOrderId(orderData);

        // Record balances before fill
        uint256 solverTokenOutBefore = tokens[1].balanceOf(users[2]);
        uint256 recipientTokenOutBefore = tokens[1].balanceOf(users[0]);

        // Fill the order on the destination chain
        vm.prank(users[2]);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.Fill(orderId, users[2], orderData.amountOut);
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({
                amountOutToFill: orderData.amountOut,
                originRecipient: users[2].toBytes32()
            })
        );

        // Check token transfers on destination chain
        assertEq(tokens[1].balanceOf(users[2]), solverTokenOutBefore - orderData.amountOut, "solver should have sent tokenOut");
        assertEq(tokens[1].balanceOf(users[0]), recipientTokenOutBefore + orderData.amountOut, "recipient should have received tokenOut");
        assertTrue(messenger.isFillReported(orderId), "fill report should have been sent to origin chain");
    }

    function test_crossChainOrderPartialFill_success() public {
        // Create order data for a cross-chain order (originated on chain 2, destined for chain 1)
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order was created on chain 2
            sender: users[0].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // To be filled on chain 1 (current chain)
            fillDeadline: uint64(block.timestamp + FILL_DURATION),
            amountOut: AMOUNT_OUT,
            tokenOut: address(tokens[1]).toBytes32(),
            recipient: users[0].toBytes32(),
            solver: users[2].toBytes32()
        });

        bytes32 orderId = orderBook.getOrderId(orderData);

        // Fill half of the order
        uint128 fillAmount = orderData.amountOut / 2;

        // Record balances before fill
        uint256 solverTokenOutBefore = tokens[1].balanceOf(users[2]);
        uint256 recipientTokenOutBefore = tokens[1].balanceOf(users[0]);

        vm.prank(users[2]);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.Fill(orderId, users[2], fillAmount);
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({
                amountOutToFill: fillAmount,
                originRecipient: users[2].toBytes32()
            })
        );

        // Check token transfers on destination chain
        assertEq(tokens[1].balanceOf(users[2]), solverTokenOutBefore - fillAmount, "solver should have sent partial tokenOut");
        assertEq(tokens[1].balanceOf(users[0]), recipientTokenOutBefore + fillAmount, "recipient should have received partial tokenOut");
        assertTrue(messenger.isFillReported(orderId), "fill report should have been sent to origin chain");
    }

    function test_solverNotSpecified_anyoneCanFill_success(address solver) public {
        vm.assume(solver != address(orderBook) && solver != users[0]);

        vm.deal(solver, 1 ether);
        tokens[1].mint(solver, MINT_AMOUNT);

        // Create order data for a cross-chain order (originated on chain 2, destined for chain 1)
        IOrderBook.OrderData memory orderData = IOrderBook.OrderData({
            version: 1,
            originChainId: DEST_CHAIN_ID, // Order was created on chain 2
            sender: users[0].toBytes32(),
            nonce: 0,
            destChainId: CHAIN_ID, // To be filled on chain 1 (current chain)
            fillDeadline: uint64(block.timestamp + FILL_DURATION),
            amountOut: AMOUNT_OUT,
            tokenOut: address(tokens[1]).toBytes32(),
            recipient: users[0].toBytes32(),
            solver: address(0).toBytes32() // No designated solver
        });

        bytes32 orderId = orderBook.getOrderId(orderData);

        vm.startPrank(solver);
        tokens[1].approve(address(orderBook), type(uint256).max);
        vm.expectEmit(true, false, false, true);
        emit IOrderBook.Fill(orderId, solver, orderData.amountOut);
        orderBook.fillOrder(
            orderId,
            orderData,
            IOrderBook.FillParams({
                amountOutToFill: orderData.amountOut,
                originRecipient: solver.toBytes32()
            })
        );
        vm.stopPrank();
    }
}