// SPDX-License-Identifier: UNLICENSED
pragma solidity 0.8.33;

import { IERC20 } from "../../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/interfaces/IERC20.sol";
import { console2 } from "../../lib/forge-std/src/Script.sol";

import { ScriptBase } from "../ScriptBase.s.sol";
import { IOrderBook } from "../../src/interfaces/IOrderBook.sol";

/// @title OpenOrder
/// @notice Script to create test orders on testnets
/// @dev Usage: forge script script/test/OpenOrder.s.sol --rpc-url <rpc> --broadcast \
///             --sig "run(address,uint128,uint32,bytes32,uint128,bytes32,bytes32,uint32)" \
///             <tokenIn> <amountIn> <destChainId> <tokenOut> <amountOut> <recipient> <solver> <deadlineOffset>
/// @dev OrderData can be queried from the contract using getOrderData(orderId)
contract OpenOrder is ScriptBase {
    /// @notice Default deadline offset (1 hour)
    uint32 constant DEFAULT_DEADLINE_OFFSET = 3600;

    /// @notice Create an order and output OrderData to JSON
    /// @param tokenIn_ Address of the input token on this chain
    /// @param amountIn_ Amount of input token to provide
    /// @param destChainId_ Destination chain ID where order will be filled
    /// @param tokenOut_ Address of output token on destination chain (as bytes32)
    /// @param amountOut_ Amount of output token expected
    /// @param recipient_ Address to receive funds on destination (defaults to sender if zero)
    /// @param solver_ Designated solver address (zero = any solver can fill)
    /// @param deadlineOffset_ Seconds from now for fill deadline (defaults to 1 hour if zero)
    /// @return orderId_ The unique ID of the created order
    function run(
        address tokenIn_,
        uint128 amountIn_,
        uint32 destChainId_,
        bytes32 tokenOut_,
        uint128 amountOut_,
        bytes32 recipient_,
        bytes32 solver_,
        uint32 deadlineOffset_
    ) external returns (bytes32 orderId_) {
        address sender_ = vm.rememberKey(vm.envUint("SENDER_PRIVATE_KEY"));
        address orderBook_ = _readDeployment(block.chainid);

        // Approve orderbook to spend tokenIn if needed
        uint256 currentAllowance_ = IERC20(tokenIn_).allowance(sender_, orderBook_);
        if (currentAllowance_ < amountIn_) {
            vm.broadcast(sender_);
            IERC20(tokenIn_).approve(orderBook_, type(uint256).max);
        }

        // Build order params
        IOrderBook.OrderParams memory orderParams_ = _buildOrderParams(
            sender_,
            tokenIn_,
            amountIn_,
            destChainId_,
            tokenOut_,
            amountOut_,
            recipient_,
            solver_,
            deadlineOffset_
        );

        // Execute order creation
        orderId_ = _executeOpenOrder(sender_, orderBook_, orderParams_);

        // Fetch OrderData directly from the contract for logging
        IOrderBook.OrderData memory orderData_ = IOrderBook(orderBook_).getOrderData(orderId_);

        // Verify order ID matches (sanity check)
        bytes32 computedOrderId_ = IOrderBook(orderBook_).getOrderId(orderData_);
        require(orderId_ == computedOrderId_, "Order ID mismatch");

        // Log order details
        _logOrderDetails(orderId_, orderData_);
    }

    /// @notice Build order params from inputs
    function _buildOrderParams(
        address sender_,
        address tokenIn_,
        uint128 amountIn_,
        uint32 destChainId_,
        bytes32 tokenOut_,
        uint128 amountOut_,
        bytes32 recipient_,
        bytes32 solver_,
        uint32 deadlineOffset_
    ) internal view returns (IOrderBook.OrderParams memory) {
        // Default recipient to sender if not specified
        bytes32 recipient = recipient_ == bytes32(0) ? bytes32(uint256(uint160(sender_))) : recipient_;

        // Default deadline offset to 1 hour
        uint32 offset_ = deadlineOffset_ == 0 ? DEFAULT_DEADLINE_OFFSET : deadlineOffset_;

        return
            IOrderBook.OrderParams({
                destChainId: destChainId_,
                fillDeadline: uint32(block.timestamp) + offset_,
                tokenIn: tokenIn_,
                tokenOut: tokenOut_,
                amountIn: amountIn_,
                amountOut: amountOut_,
                recipient: recipient,
                solver: solver_
            });
    }

    /// @notice Execute the order creation transaction
    function _executeOpenOrder(
        address sender_,
        address orderBook_,
        IOrderBook.OrderParams memory orderParams_
    ) internal returns (bytes32 orderId_) {
        // Check and approve token allowance
        uint256 currentAllowance_ = IERC20(orderParams_.tokenIn).allowance(sender_, orderBook_);

        vm.startBroadcast(sender_);

        if (currentAllowance_ < orderParams_.amountIn) {
            IERC20(orderParams_.tokenIn).approve(orderBook_, type(uint256).max);
        }

        // Open order
        orderId_ = IOrderBook(orderBook_).openOrder(orderParams_);

        vm.stopBroadcast();
    }

    /// @notice Log order details
    function _logOrderDetails(bytes32 orderId_, IOrderBook.OrderData memory orderData_) internal pure {
        // solhint-disable-next-line no-console
        console2.log("\n=== Order Created ===");
        // solhint-disable-next-line no-console
        console2.log("Order ID:", vm.toString(orderId_));
        // solhint-disable-next-line no-console
        console2.log("Origin Chain:", orderData_.originChainId);
        // solhint-disable-next-line no-console
        console2.log("Sender:", address(uint160(uint256(orderData_.sender))));
        // solhint-disable-next-line no-console
        console2.log("Token In:", address(uint160(uint256(orderData_.tokenIn))));
        // solhint-disable-next-line no-console
        console2.log("Amount In:", uint256(orderData_.amountIn));
        // solhint-disable-next-line no-console
        console2.log("Destination Chain:", orderData_.destChainId);
        // solhint-disable-next-line no-console
        console2.log("Token Out:");
        // solhint-disable-next-line no-console
        console2.logBytes32(orderData_.tokenOut);
        // solhint-disable-next-line no-console
        console2.log("Amount Out:", uint256(orderData_.amountOut));
        // solhint-disable-next-line no-console
        console2.log("Fill Deadline:", orderData_.fillDeadline);
        // solhint-disable-next-line no-console
        console2.log("=== End Order Details ===\n");
    }
}
