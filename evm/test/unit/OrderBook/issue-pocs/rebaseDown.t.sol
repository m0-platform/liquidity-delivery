// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Test } from "../../../../lib/forge-std/src/Test.sol";
import { ERC1967Proxy } from "../../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { TypeConverter } from "../../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBook, IOrderBook } from "../../../../src/OrderBook.sol";
import { MockPortalV2 } from "../../../mock/MockPortalV2.t.sol";
import { MockERC20 } from "../../../mock/MockERC20.t.sol";
import { MockRebaseDownToken } from "../../../mock/issue-pocs/MockRebaseDownToken.t.sol";

/// @notice Tests for downward rebasing token behavior
/// @dev Verifies behavior when token balance decreases after order creation
contract RebaseDownTest is Test {
    using TypeConverter for *;

    OrderBook internal orderBook;
    MockPortalV2 internal portal;
    MockRebaseDownToken internal rebaseToken;
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

        // Deploy rebase token (tokenIn) and regular token (tokenOut)
        rebaseToken = new MockRebaseDownToken("Rebase Token", "RBT", 6);
        tokenOut = new MockERC20("Token Out", "OUT", 6);

        // Mint tokens
        rebaseToken.mint(alice, MINT_AMOUNT);
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
            tokenIn: address(rebaseToken),
            destChainId: DEST_CHAIN_ID,
            tokenOut: address(tokenOut).toBytes32(),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            recipient: alice.toBytes32(),
            fillDeadline: uint32(block.timestamp) + FILL_DURATION,
            solver: solver.toBytes32()
        });
    }

    /// @notice Test that reportFill reverts when token rebases down
    /// @dev Order created with 100e6 tokens, rebase decreases OrderBook balance to 90e6
    ///      reportFill tries to transfer 100e6, but only 90e6 available
    function test_rebaseDown_reportFillReverts() public {
        // 1. Create order - OrderBook receives 100e6 tokens
        vm.startPrank(alice);
        rebaseToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // Verify OrderBook received the tokens
        assertEq(rebaseToken.balanceOf(address(orderBook)), AMOUNT_IN);

        // 2. Rebase DOWN by 10% - OrderBook now has 90e6 tokens
        rebaseToken.rebaseDownAccount(address(orderBook), 1000); // 10% = 1000 bps

        // Verify balance decreased
        uint256 newBalance = rebaseToken.balanceOf(address(orderBook));
        assertEq(newBalance, 90e6, "OrderBook balance should be 90% of original");

        // 3. Attempt reportFill - should revert because OrderBook has insufficient balance
        vm.prank(address(portal));
        vm.expectRevert(); // Will revert due to insufficient balance
        orderBook.reportFill(
            params.destChainId,
            IOrderBook.FillReport({
                orderId: orderId,
                amountOutFilled: AMOUNT_OUT,
                amountInToRelease: AMOUNT_IN, // Trying to release 100e6, but only have 90e6
                originRecipient: solver.toBytes32(),
                tokenIn: address(rebaseToken).toBytes32()
            })
        );
    }

    /// @notice Test reportCancel behavior when token rebases down
    /// @dev Verify whether refund also fails when balance < amountIn
    function test_rebaseDown_reportCancelReverts() public {
        // 1. Create order - OrderBook receives 100e6 tokens
        vm.startPrank(alice);
        rebaseToken.approve(address(orderBook), AMOUNT_IN);
        bytes32 orderId = orderBook.openOrder(params);
        vm.stopPrank();

        // 2. Rebase DOWN by 10%
        rebaseToken.rebaseDownAccount(address(orderBook), 1000);

        // Verify balance decreased
        assertEq(rebaseToken.balanceOf(address(orderBook)), 90e6);

        // 3. Simulate cancel report arriving from destination chain
        //    reportCancel triggers refund which should fail due to insufficient balance
        vm.prank(address(portal));
        vm.expectRevert(); // Will revert due to insufficient balance
        orderBook.reportCancel(
            DEST_CHAIN_ID,
            IOrderBook.CancelReport({
                orderId: orderId,
                orderSender: alice.toBytes32(),
                tokenIn: params.tokenIn.toBytes32()
            })
        );
    }
}
