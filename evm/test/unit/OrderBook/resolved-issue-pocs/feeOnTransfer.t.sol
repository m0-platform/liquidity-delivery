// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Test } from "../../../../lib/forge-std/src/Test.sol";
import { ERC1967Proxy } from "../../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { TypeConverter } from "../../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBook, IOrderBook } from "../../../../src/OrderBook.sol";
import { MockPortalV2 } from "../../../mock/MockPortalV2.t.sol";
import { MockERC20 } from "../../../mock/MockERC20.t.sol";
import { MockFeeToken } from "../../../mock/issue-pocs/MockFeeToken.t.sol";

/// @notice Tests for fee-on-transfer token behavior
/// @dev Verifies that orders created with 0% fee tokens revert when fee is later enabled
contract FeeOnTransferTest is Test {
    using TypeConverter for *;

    OrderBook internal orderBook;
    MockPortalV2 internal portal;
    MockFeeToken internal feeToken;
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

        // Deploy fee token (tokenIn) and regular token (tokenOut)
        feeToken = new MockFeeToken("Fee Token", "FEE", 6);
        tokenOut = new MockERC20("Token Out", "OUT", 6);

        // Mint tokens
        feeToken.mint(alice, MINT_AMOUNT);
        tokenOut.mint(solver, MINT_AMOUNT);

        // Deploy OrderBook
        portal = new MockPortalV2();
        vm.deal(admin, 1 ether);
        address implementation = address(new OrderBook(CHAIN_ID, address(portal)));
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
            tokenIn: address(feeToken),
            destChainId: DEST_CHAIN_ID,
            tokenOut: address(tokenOut).toBytes32(),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            recipient: alice.toBytes32(),
            fillDeadline: uint32(block.timestamp) + FILL_DURATION,
            solver: solver.toBytes32()
        });
    }

    /// @notice Test that reportFill NOT revert when fee is enabled after order creation
    /// @dev Order created with 0% fee, then fee increased to 1%, reportFill succeeds bc feeOnTransfer ignored. Solver loses small amount, but not all.
    function test_feeEnabledAfterOrderCreation_reportFillSuccess() public {
        // 1. Create order with 0% fee (default)
        vm.startPrank(alice);
        feeToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // Verify order was created successfully
        IOrderBook.Order memory order = orderBook.getOrder(orderId);
        assertEq(uint8(order.status), uint8(IOrderBook.OrderStatus.Created));

        // 2. Enable 1% fee on the token
        feeToken.setFeePercent(100); // 100 basis points = 1%

        // 3. Attempt reportFill - should revert because safeTransferExact will fail
        // The OrderBook expects to transfer exactly AMOUNT_IN, but fee will reduce actual amount
        vm.prank(address(portal));
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: AMOUNT_OUT,
                amountInToRelease: AMOUNT_IN,
                originRecipient: solver.toBytes32(),
                tokenIn: address(feeToken).toBytes32()
            })
        );
    }

    /// @notice Test that claimRefund does NOT revert when fee is enabled - user loses funds
    /// @dev This documents an inconsistency: reportFill uses safeTransferExact (reverts),
    ///      but _claimRefund uses safeTransfer (does not revert, user loses somes funds to fee, but not all)
    function test_feeEnabledAfterOrderCreation_claimRefundSuccess() public {
        // 1. Create order with 0% fee (default)
        vm.startPrank(alice);
        feeToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // 2. Enable 1% fee on the token before refund
        feeToken.setFeePercent(100); // 100 basis points = 1%

        uint256 aliceBalanceBefore = feeToken.balanceOf(alice);

        // 3. Simulate cancel report arriving from destination chain
        //    reportCancel triggers refund which uses safeTransfer
        vm.prank(address(portal));
        orderBook.reportCancel(
            DEST_CHAIN_ID,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: alice.toBytes32(),
                tokenIn: params.tokenIn.toBytes32()
            })
        );

        // 6. Verify alice received less than expected due to fee
        uint256 aliceBalanceAfter = feeToken.balanceOf(alice);
        uint256 expectedWithFee = AMOUNT_IN - ((AMOUNT_IN * 100) / 10000); // 99% of AMOUNT_IN

        assertEq(aliceBalanceAfter - aliceBalanceBefore, expectedWithFee, "alice should receive amount minus 1% fee");

        // User lost 1% to fee
        uint256 fundsLost = AMOUNT_IN - (aliceBalanceAfter - aliceBalanceBefore);
        assertEq(fundsLost, AMOUNT_IN / 100, "user lost 1% to fee");
    }
}
