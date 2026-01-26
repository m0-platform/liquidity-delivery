// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Test } from "../../../../lib/forge-std/src/Test.sol";
import { ERC1967Proxy } from "../../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { TypeConverter } from "../../../../lib/common/src/libs/TypeConverter.sol";
import { UIntMath } from "../../../../lib/common/src/libs/UIntMath.sol";

import { OrderBook, IOrderBook } from "../../../../src/OrderBook.sol";
import { MockPortalV2 } from "../../../mock/MockPortalV2.t.sol";
import { MockERC20 } from "../../../mock/MockERC20.t.sol";
import { MockMEarnerToken } from "../../../mock/issue-pocs/MockMEarnerToken.t.sol";

/// @notice Tests for MEarnerManager yield accrual behavior
/// @dev Demonstrates that yield accrues to OrderBook during order lifetime and gets stuck
contract MEarnerYieldAccrualTest is Test {
    using TypeConverter for *;
    using UIntMath for uint256;

    OrderBook internal orderBook;
    MockPortalV2 internal portal;
    MockMEarnerToken internal mToken;
    MockERC20 internal tokenOut;

    uint32 internal CHAIN_ID = block.chainid.safe32();
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

        // Deploy M token (tokenIn) and regular token (tokenOut)
        mToken = new MockMEarnerToken("M Earner Token", "mEARN", 6);
        tokenOut = new MockERC20("Token Out", "OUT", 6);

        // Mint tokens
        mToken.mint(alice, MINT_AMOUNT);
        tokenOut.mint(solver, MINT_AMOUNT);

        // Deploy OrderBook
        portal = new MockPortalV2();
        vm.deal(admin, 1 ether);
        address implementation = address(new OrderBook(address(portal)));
        orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, admin))
            )
        );

        // Configure
        portal.setOrderBook(address(orderBook));
        vm.prank(admin);
        orderBook.setDestinationSupported(DEST_CHAIN_ID, true);

        // Setup order params
        params = IOrderBook.OrderParams({
            tokenIn: address(mToken),
            destChainId: DEST_CHAIN_ID,
            tokenOut: address(tokenOut).toBytes32(),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            recipient: alice.toBytes32(),
            fillDeadline: uint32(block.timestamp) + FILL_DURATION,
            solver: solver.toBytes32(),
            sender: alice
        });
    }

    /// @notice Demonstrates yield getting stuck in OrderBook after reportFill
    /// @dev Timeline:
    ///      1. Alice deposits 100e6 M tokens at index 1e12
    ///      2. Index increases by 10% (yield accrues)
    ///      3. OrderBook balance is now 110e6 (100e6 + 10% yield)
    ///      4. reportFill transfers 100e6 to solver
    ///      5. ~9.09e6 tokens remain stuck in OrderBook (the yield)
    function test_yieldAccrual_reportFill_yieldStuckInOrderBook() public {
        // Record initial index
        uint128 initialIndex = mToken.currentIndex();
        assertEq(initialIndex, 1e12);

        // 1. Alice creates order - OrderBook receives 100e6 M tokens at index 1e12
        vm.startPrank(alice);
        mToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // Verify OrderBook received exactly 100e6
        assertEq(mToken.balanceOf(address(orderBook)), AMOUNT_IN);

        // 2. Time passes, yield accrues - index increases by 10%
        mToken.accrueYield(1000); // 1000 bps = 10%

        // 3. OrderBook balance has increased due to yield
        uint256 orderBookBalanceAfterYield = mToken.balanceOf(address(orderBook));
        assertEq(orderBookBalanceAfterYield, 110e6, "OrderBook should have 110e6 after 10% yield");

        // 4. Solver fills order, reportFill transfers 100e6 to solver
        uint256 solverBalanceBefore = mToken.balanceOf(solver);

        vm.prank(address(portal));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: AMOUNT_OUT,
                amountInToRelease: AMOUNT_IN, // Only releases original 100e6
                originRecipient: solver.toBytes32(),
                tokenIn: address(mToken).toBytes32()
            })
        );

        // Solver received 100e6
        uint256 solverBalanceAfter = mToken.balanceOf(solver);
        assertEq(solverBalanceAfter - solverBalanceBefore, AMOUNT_IN, "solver should receive 100e6");

        // 5. Yield (~10e6) is stuck in OrderBook!
        uint256 orderBookBalanceAfterFill = mToken.balanceOf(address(orderBook));
        assertGt(orderBookBalanceAfterFill, 0, "yield should be stuck in OrderBook");

        // The stuck amount is approximately 10e6 (the yield)
        // Due to index math rounding, it's slightly less than 10e6
        emit log_named_uint("Stuck yield in OrderBook", orderBookBalanceAfterFill);

        // There's no mechanism to recover these funds
    }

    /// @notice Demonstrates yield getting stuck after reportCancel refund
    /// @dev Same issue occurs with reportCancel - user only gets original amount back
    function test_yieldAccrual_reportCancel_yieldStuckInOrderBook() public {
        // 1. Alice creates order
        vm.startPrank(alice);
        mToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // 2. Time passes, yield accrues - index increases by 10%
        mToken.accrueYield(1000); // 10%

        // OrderBook balance increased
        assertEq(mToken.balanceOf(address(orderBook)), 110e6);

        // 3. Simulate cancel report arriving from destination chain
        //    With the new design, cancellation originates on destination and sends
        //    a CancelReport to origin which triggers the refund
        uint256 aliceBalanceBefore = mToken.balanceOf(alice);
        vm.prank(address(portal));
        orderBook.reportCancel(
            DEST_CHAIN_ID,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: alice.toBytes32(),
                tokenIn: params.tokenIn.toBytes32(),
                amountInToRefund: params.amountIn
            })
        );
        uint256 aliceBalanceAfter = mToken.balanceOf(alice);

        // Alice only received ~100e6 (the recorded amountIn), not 110e6
        // Due to index math rounding, actual amount may be slightly less
        uint256 aliceReceived = aliceBalanceAfter - aliceBalanceBefore;
        assertApproxEqAbs(aliceReceived, AMOUNT_IN, 2, "alice only gets ~original amount");
        assertLt(aliceReceived, 110e6, "alice should NOT receive the yield");

        // 4. Yield is stuck in OrderBook
        uint256 orderBookBalanceAfterRefund = mToken.balanceOf(address(orderBook));
        assertGt(orderBookBalanceAfterRefund, 0, "yield should be stuck in OrderBook");

        emit log_named_uint("Stuck yield in OrderBook after refund", orderBookBalanceAfterRefund);
    }

    /* ========== MITIGATION TESTS ========== */

    /// @notice Demonstrates mitigation: feeRate = 100% routes all yield to feeRecipient
    /// @dev By configuring OrderBook with 100% feeRate in MEarnerManager,
    ///      yield is routed to a controlled feeRecipient instead of getting stuck
    function test_mitigation_feeRate100Percent_yieldRoutedToFeeRecipient() public {
        // Setup: Configure 100% fee rate for OrderBook address
        address treasury = makeAddr("treasury");
        mToken.setFeeRecipient(treasury);
        mToken.setFeeRate(address(orderBook), 10000); // 100% = 10000 bps

        // 1. Alice creates order
        vm.startPrank(alice);
        mToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // Verify OrderBook received 100e6
        assertEq(mToken.balanceOf(address(orderBook)), AMOUNT_IN);

        // 2. Yield accrues - 10%
        mToken.accrueYield(1000);

        // OrderBook balance increased to 110e6
        assertEq(mToken.balanceOf(address(orderBook)), 110e6);

        // 3. Claim excess yield - with 100% feeRate, all goes to treasury
        uint256 treasuryBalanceBefore = mToken.balanceOf(treasury);
        mToken.claimExcessYield(address(orderBook));
        uint256 treasuryBalanceAfter = mToken.balanceOf(treasury);

        // Treasury received the yield (~10e6)
        uint256 yieldToTreasury = treasuryBalanceAfter - treasuryBalanceBefore;
        emit log_named_uint("Yield routed to treasury", yieldToTreasury);
        assertGt(yieldToTreasury, 9e6, "treasury should receive yield");

        // 4. Now fill the order
        vm.prank(address(portal));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: AMOUNT_OUT,
                amountInToRelease: AMOUNT_IN,
                originRecipient: solver.toBytes32(),
                tokenIn: address(mToken).toBytes32()
            })
        );

        // 5. No yield stuck in OrderBook (or minimal due to rounding)
        uint256 orderBookFinalBalance = mToken.balanceOf(address(orderBook));
        emit log_named_uint("OrderBook final balance (should be ~0)", orderBookFinalBalance);

        // With the mitigation, yield went to treasury instead of getting stuck
        assertLt(orderBookFinalBalance, 1e6, "minimal or no funds stuck with mitigation");
    }
}
