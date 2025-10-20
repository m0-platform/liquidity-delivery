// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.26;

import { Test } from "../../../lib/forge-std/src/Test.sol";
import { ERC1967Proxy } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";

import { OrderBook, IOrderBook } from "../../../src/OrderBook.sol";
import { TypeConverter } from "../../../src/libs/TypeConverter.sol";
import { MockMessenger } from "../../mock/MockMessenger.t.sol";
import { MockERC20 } from "../../mock/MockERC20.t.sol";

abstract contract OrderBookTestBase is Test {
    using TypeConverter for *;

    OrderBook internal orderBook;
    MockMessenger internal messenger;

    uint16 internal constant VERSION = 1;
    uint32 internal constant CHAIN_ID = 1;
    uint32 internal constant DEST_CHAIN_ID = 2;
    uint256 internal constant TOKEN_COUNT = 3;
    uint256 internal constant USER_COUNT = 3;
    uint256 internal constant MINT_AMOUNT = 100e6;
    uint128 internal constant AMOUNT_IN = 10e6;
    uint128 internal constant AMOUNT_OUT = 999e4;
    uint40 internal constant FILL_DURATION = 1 hours;

    address internal admin;
    mapping(uint256 => MockERC20) internal tokens;
    mapping(uint256 => address) internal users;

    IOrderBook.OrderParams internal params;

    function setUp() public virtual {

        // Deploy
        messenger = new MockMessenger();
        admin = keccak256(abi.encodePacked("admin")).toAddress();
        vm.deal(admin, 1 ether);
        address implementation = address(new OrderBook(CHAIN_ID, address(messenger)));
        orderBook = OrderBook(
            address(new ERC1967Proxy(
                implementation,
                abi.encodeWithSelector(
                    OrderBook.initialize.selector,
                    admin
                )
            ))
        );

        // Configure
        messenger.setOrderBook(address(orderBook));
        vm.prank(admin);
        orderBook.setDestinationConfig(DEST_CHAIN_ID, true, uint40(10 minutes));

        // Deploy mock tokens
        for (uint256 i = 0; i < TOKEN_COUNT; i++) {
            // TODO test with different decimals
            tokens[i] = new MockERC20(string.concat("Token ", vm.toString(i + 1)), string.concat("T", vm.toString(i + 1)), 6);
        }

        // Create users
        for (uint256 i = 0; i < USER_COUNT; i++) {
            users[i] = keccak256(abi.encodePacked("user", i + 1)).toAddress();
        }

        // Deal eth and tokens to users
        for (uint256 i = 0; i < USER_COUNT; i++) {
            vm.deal(users[i], 1 ether);

            for (uint256 j = 0; j < TOKEN_COUNT; j++) {
                tokens[j].mint(users[i], MINT_AMOUNT);
            }
        }

        // Setup the standard order params used in tests
        params = IOrderBook.OrderParams({
            tokenIn: address(tokens[0]),
            destChainId: DEST_CHAIN_ID,
            tokenOut: address(tokens[1]).toBytes32(),
            amountIn: AMOUNT_IN,
            amountOut: AMOUNT_OUT,
            recipient: users[0].toBytes32(),
            fillDeadline: uint40(block.timestamp) + FILL_DURATION,
            solver: users[2].toBytes32()
        });
    }

    // =========== Helper Functions ========== //

    function _getOrderIdFromParams(address sender_, uint64 nonce_, IOrderBook.OrderParams memory params_) internal view returns (bytes32) {
        return orderBook.getOrderId(IOrderBook.OrderData({
            version: 1,
            originChainId: CHAIN_ID,
            sender: sender_.toBytes32(),
            nonce: nonce_,
            destChainId: params_.destChainId,
            fillDeadline: params_.fillDeadline,
            amountOut: params_.amountOut,
            tokenOut: params_.tokenOut,
            recipient: params_.recipient,
            solver: params_.solver
        }));
    }

    modifier from(address user_) {
        vm.startPrank(user_);
        _;
        vm.stopPrank();
    }

    function _placeOrder(address sender_, IOrderBook.OrderParams memory params_) internal from(sender_) returns (bytes32) {
        tokens[0].approve(address(orderBook), uint256(params_.amountIn));
        bytes32 orderId_ = orderBook.openOrder(params_);

        return orderId_;
    }

    function _fillOrder(address solver_, bytes32 orderId_, uint128 fillAmount_) internal from(solver_) {
        // Get the order data
        IOrderBook.Order memory order = orderBook.getOrder(orderId_);
        MockERC20(order.tokenOut.toAddress()).approve(address(orderBook), fillAmount_);

        orderBook.fillOrder(
            orderId_, 
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
                amountOutToFill: fillAmount_,
                originRecipient: order.solver
            })
        );
    }

    function _reportFill(
        address solver_,
        bytes32 orderId_, 
        uint128 amountOutFilled_
    ) internal {
        // Report the fill back to the origin chain
        vm.prank(address(messenger));
        orderBook.reportFill(IOrderBook.FillReport({
            orderId: orderId_,
            amountOutFilled: amountOutFilled_,
            originRecipient: solver_.toBytes32()
        }));
    }
}