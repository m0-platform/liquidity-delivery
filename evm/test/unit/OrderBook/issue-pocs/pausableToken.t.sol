// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Test } from "../../../../lib/forge-std/src/Test.sol";
import { ERC1967Proxy } from "../../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { TypeConverter } from "../../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBook, IOrderBook } from "../../../../src/OrderBook.sol";
import { MockPortalV2 } from "../../../mock/MockPortalV2.t.sol";
import { MockERC20 } from "../../../mock/MockERC20.t.sol";
import { MockPausableToken } from "../../../mock/issue-pocs/MockPausableToken.t.sol";

/// @notice Tests for pausable token behavior (USDC, USDT)
/// @dev Demonstrates delayed double-spend when token is paused during reportFill
contract PausableTokenTest is Test {
    using TypeConverter for *;

    OrderBook internal orderBook;
    MockPortalV2 internal messenger;
    MockPausableToken internal pausableToken;
    MockERC20 internal tokenOut;

    uint32 internal constant CHAIN_ID = 1;
    uint32 internal constant DEST_CHAIN_ID = 2;
    uint256 internal constant MINT_AMOUNT = 1000e6;
    uint128 internal constant AMOUNT_IN = 100e6;
    uint128 internal constant AMOUNT_OUT = 99e6;
    uint32 internal constant FILL_DURATION = 1 hours;
    uint32 internal constant FINALITY_BUFFER = 10 minutes;

    address internal admin;
    address internal alice;
    address internal solver;

    IOrderBook.OrderParams internal params;

    function setUp() public {
        // Create users
        admin = makeAddr("admin");
        alice = makeAddr("alice");
        solver = makeAddr("solver");

        // Deploy pausable token (tokenIn) and regular token (tokenOut)
        pausableToken = new MockPausableToken("Pausable Token", "PAUSE", 6);
        tokenOut = new MockERC20("Token Out", "OUT", 6);

        // Mint tokens
        pausableToken.mint(alice, MINT_AMOUNT);
        tokenOut.mint(solver, MINT_AMOUNT);

        // Deploy OrderBook
        messenger = new MockPortalV2();
        vm.deal(admin, 1 ether);
        address implementation = address(new OrderBook(CHAIN_ID, address(messenger)));
        orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, admin))
            )
        );

        // Configure
        messenger.setOrderBook(address(orderBook));
        vm.prank(admin);
        orderBook.setDestinationSupported(DEST_CHAIN_ID, true);

        // Setup order params
        params = IOrderBook.OrderParams({
            tokenIn: address(pausableToken),
            destChainId: DEST_CHAIN_ID,
            tokenOut: address(tokenOut).toBytes32(),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            recipient: alice.toBytes32(),
            fillDeadline: uint32(block.timestamp) + FILL_DURATION,
            solver: solver.toBytes32()
        });
    }

    /// @notice Demonstrates delayed double-spend when token is paused during reportFill
    /// @dev Timeline:
    ///      1. Alice creates order with pausable tokenIn (e.g., USDC)
    ///      2. Solver fills on destination - Alice receives tokenOut
    ///      3. Token gets paused before reportFill executes on origin
    ///      4. reportFill reverts - solver can't claim tokenIn
    ///      5. Token gets unpaused
    ///      6. Alice claims refund - gets tokenIn back
    ///      Result: Alice received BOTH tokenOut (dest) AND tokenIn refund (origin)
    function test_pausedToken_delayedDoubleSpend() public {
        // 1. Alice creates order - OrderBook receives 100e6 pausable tokens
        vm.startPrank(alice);
        pausableToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // Verify OrderBook received the tokens
        assertEq(pausableToken.balanceOf(address(orderBook)), AMOUNT_IN);

        // 2. At this point, solver fills on destination chain
        //    Alice receives tokenOut on destination (simulated - not shown here)
        //    Solver sends reportFill message back to origin...

        // 3. Token gets paused (e.g., USDC blackswan event, regulatory action)
        pausableToken.pause();

        // 4. reportFill arrives but reverts because token is paused
        vm.prank(address(messenger));
        vm.expectRevert(abi.encodeWithSelector(MockPausableToken.EnforcedPause.selector));
        orderBook.reportFill(
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: AMOUNT_OUT,
                amountInToRelease: AMOUNT_IN,
                originRecipient: solver.toBytes32(),
                tokenIn: address(pausableToken).toBytes32()
            })
        );

        // Order is still in Created status - solver couldn't complete the fill
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created));

        // 5. Time passes, token gets unpaused
        vm.warp(block.timestamp + 1 days);
        pausableToken.unpause();

        // 6. Simulate cancel report arriving from destination chain
        //    With the new design, cancellation originates on destination and sends
        //    a CancelReport to origin which triggers the refund

        uint256 aliceBalanceBefore = pausableToken.balanceOf(alice);
        vm.prank(address(messenger));
        orderBook.reportCancel(
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: alice.toBytes32(),
                tokenIn: params.tokenIn.toBytes32()
            })
        );
        uint256 aliceBalanceAfter = pausableToken.balanceOf(alice);

        // Alice got her tokenIn back!
        assertEq(aliceBalanceAfter - aliceBalanceBefore, AMOUNT_IN, "alice got full refund");

        // RESULT: Alice received BOTH:
        // - tokenOut on destination chain (from solver's fill)
        // - tokenIn refund on origin chain (after unpause)
        // This is a delayed double-spend - solver loses funds
    }
}
