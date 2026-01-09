// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { Test } from "../../../lib/forge-std/src/Test.sol";
import { ERC1967Proxy } from "../../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/proxy/ERC1967/ERC1967Proxy.sol";
import { TypeConverter } from "../../../lib/common/src/libs/TypeConverter.sol";

import { OrderBook, IOrderBook } from "../../../src/OrderBook.sol";
import { MockPortalV2 } from "../../mock/MockPortalV2.t.sol";
import { MockERC20 } from "../../mock/MockERC20.t.sol";

abstract contract OrderBookTestBase is Test {
    using TypeConverter for *;

    OrderBook internal orderBook;
    MockPortalV2 internal portal;

    uint16 internal constant VERSION = 1;
    uint32 internal constant CHAIN_ID = 1;
    uint32 internal constant DEST_CHAIN_ID = 2;
    uint256 internal constant MINT_AMOUNT = 1000;
    uint128 internal constant AMOUNT_IN = 100;
    uint128 internal constant AMOUNT_OUT = 99;
    uint32 internal constant FILL_DURATION = 1 hours;

    struct Token {
        string name;
        string symbol;
        uint8 decimals;
    }

    Token[] internal TOKENS;
    string[] internal USERS;

    address internal admin;
    address internal pauser;
    MockERC20 internal tokenIn;
    MockERC20 internal tokenOut;
    mapping(string => MockERC20) internal tokens;
    mapping(string => address) internal users;

    IOrderBook.OrderParams internal params;

    constructor() {
        // Insert tokens to be deployed
        TOKENS.push(Token("token-in-6D", "TI6", 6));
        TOKENS.push(Token("token-out-6D", "TO6", 6));
        TOKENS.push(Token("token-in-18D", "TI18", 18));
        TOKENS.push(Token("token-out-18D", "TO18", 18));

        // Insert users to be created
        USERS.push("admin");
        USERS.push("pauser");
        USERS.push("solver");
        USERS.push("alice");
        USERS.push("bob");
        USERS.push("carol");
    }

    function setUp() public virtual {
        // Deploy mock tokens
        uint256 tokenCount = TOKENS.length;
        for (uint256 i = 0; i < tokenCount; i++) {
            Token memory tokenInfo = TOKENS[i];
            MockERC20 token = new MockERC20(tokenInfo.name, tokenInfo.symbol, tokenInfo.decimals);
            tokens[tokenInfo.name] = token;
        }

        // Create users
        uint256 userCount = USERS.length;
        for (uint256 i = 0; i < userCount; i++) {
            address user = (keccak256(abi.encodePacked(USERS[i])) >> 96).toAddress();
            users[USERS[i]] = user;
        }

        // Deal eth and tokens to users
        for (uint256 i = 0; i < userCount; i++) {
            vm.deal(users[USERS[i]], 1 ether);

            for (uint256 j = 0; j < tokenCount; j++) {
                tokens[TOKENS[j].name].mint(users[USERS[i]], MINT_AMOUNT * (10 ** TOKENS[j].decimals));
            }
        }

        // Deploy
        portal = new MockPortalV2();
        admin = users["admin"];
        pauser = users["pauser"];
        address implementation = address(new OrderBook(CHAIN_ID, address(portal)));
        orderBook = OrderBook(
            address(
                new ERC1967Proxy(implementation, abi.encodeWithSelector(OrderBook.initialize.selector, admin, pauser))
            )
        );

        // Configure
        portal.setOrderBook(address(orderBook));
        vm.prank(admin);
        orderBook.setDestinationSupported(DEST_CHAIN_ID, true);

        // Setup the standard order params used in tests
        params = IOrderBook.OrderParams({
            tokenIn: address(tokens["token-in-6D"]),
            destChainId: DEST_CHAIN_ID,
            tokenOut: address(tokens["token-out-6D"]).toBytes32(),
            amountIn: AMOUNT_IN * 1e6, // Adjust amount in to match 6 decimals,
            amountOut: AMOUNT_OUT * 1e6, // Adjust amount out to match 6 decimals,
            recipient: users["alice"].toBytes32(),
            fillDeadline: uint32(block.timestamp) + FILL_DURATION,
            solver: users["solver"].toBytes32()
        });
        tokenIn = tokens["token-in-6D"];
        tokenOut = tokens["token-out-6D"];
    }

    // =========== Helper Functions ========== //

    function _getOrderIdFromParams(
        address sender_,
        uint64 nonce_,
        IOrderBook.OrderParams memory params_
    ) internal view returns (bytes32) {
        return
            orderBook.getOrderId(
                IOrderBook.OrderData({
                    version: 1,
                    originChainId: CHAIN_ID,
                    sender: sender_.toBytes32(),
                    nonce: nonce_,
                    destChainId: params_.destChainId,
                    createdAt: uint64(block.timestamp),
                    fillDeadline: params_.fillDeadline,
                    amountIn: params_.amountIn,
                    amountOut: params_.amountOut,
                    tokenIn: params_.tokenIn.toBytes32(),
                    tokenOut: params_.tokenOut,
                    recipient: params_.recipient,
                    solver: params_.solver
                })
            );
    }

    modifier from(address user_) {
        vm.startPrank(user_);
        _;
        vm.stopPrank();
    }

    function _placeOrder(
        address sender_,
        IOrderBook.OrderParams memory params_
    ) internal from(sender_) returns (bytes32) {
        tokenIn.approve(address(orderBook), uint256(params_.amountIn));
        bytes32 orderId_ = orderBook.openOrder(params_);

        return orderId_;
    }

    function _fillOrder(address solver_, bytes32 orderId_, uint128 fillAmount_) internal from(solver_) {
        // Get the order data
        IOrderBook.Order memory order = orderBook.getOrder(orderId_);
        MockERC20(order.tokenOut.toAddress()).approve(address(orderBook), fillAmount_);

        orderBook.fillOrder(
            orderId_,
            _getOrderDataFromOrder(orderId_, order),
            IOrderBook.FillParams({ amountOutToFill: fillAmount_, originRecipient: order.solver })
        );
    }

    function _reportFill(
        address solver_,
        bytes32 orderId_,
        uint128 amountOutFilled_,
        uint128 amountInToRelease_
    ) internal {
        // Report the fill back to the origin chain
        vm.prank(address(portal));
        orderBook.reportFill(
            DEST_CHAIN_ID,
            IOrderBook.FillReport({
                orderId: orderId_,
                amountOutFilled: amountOutFilled_,
                amountInToRelease: amountInToRelease_,
                originRecipient: solver_.toBytes32(),
                tokenIn: address(tokenIn).toBytes32()
            })
        );
    }

    function _getOrderDataFromOrder(
        bytes32 orderId_,
        IOrderBook.Order memory order_
    ) internal view returns (IOrderBook.OrderData memory) {
        return
            IOrderBook.OrderData({
                version: order_.version,
                originChainId: CHAIN_ID,
                sender: order_.sender.toBytes32(),
                nonce: order_.nonce,
                destChainId: order_.destChainId,
                createdAt: uint64(order_.createdAt),
                fillDeadline: uint64(order_.fillDeadline),
                amountIn: order_.amountIn,
                amountOut: order_.amountOut,
                tokenIn: order_.tokenIn.toBytes32(),
                tokenOut: order_.tokenOut,
                recipient: order_.recipient,
                solver: order_.solver
            });
    }

    function _cancelOrder(address caller_, bytes32 orderId_, IOrderBook.Order memory order_) internal {
        vm.prank(caller_);
        orderBook.cancelOrder(orderId_, _getOrderDataFromOrder(orderId_, order_), new bytes(0));
    }

    function _reportCancel(
        bytes32 orderId_,
        address orderSender_,
        address tokenIn_,
        uint128 amountInToRefund_
    ) internal {
        vm.prank(address(portal));
        orderBook.reportCancel(
            DEST_CHAIN_ID,
            IOrderBook.CancelReport({
                orderId: orderId_,
                orderSender: orderSender_.toBytes32(),
                tokenIn: tokenIn_.toBytes32(),
                amountInToRefund: amountInToRefund_
            })
        );
    }

    // =========== Test Modifiers ========== //
    modifier givenTokenOutDecimals(uint8 decimals_) {
        // Adjust params to have token out with specified decimals
        if (decimals_ == 6) {
            params.tokenOut = address(tokens["token-out-6D"]).toBytes32();
            params.amountOut = AMOUNT_OUT * 1e6;
            tokenOut = tokens["token-out-6D"];
        } else if (decimals_ == 18) {
            params.tokenOut = address(tokens["token-out-18D"]).toBytes32();
            params.amountOut = AMOUNT_OUT * 1e18;
            tokenOut = tokens["token-out-18D"];
        } else {
            revert("Unsupported decimals");
        }
        _;
    }

    modifier givenTokenInDecimals(uint8 decimals_) {
        // Adjust params to have token in with specified decimals
        if (decimals_ == 6) {
            params.tokenIn = address(tokens["token-in-6D"]);
            params.amountIn = AMOUNT_IN * 1e6;
            tokenIn = tokens["token-in-6D"];
        } else if (decimals_ == 18) {
            params.tokenIn = address(tokens["token-in-18D"]);
            params.amountIn = AMOUNT_IN * 1e18;
            tokenIn = tokens["token-in-18D"];
        } else {
            revert("Unsupported decimals");
        }
        _;
    }
}
