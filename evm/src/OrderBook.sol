// SPDX-License-Identifier: GPL-3.0
pragma solidity 0.8.26;

import { IERC20 } from "../lib/common/src/interfaces/IERC20.sol";

import { IOrderBook } from "./interfaces/IOrderBook.sol";
import { IMessenger } from "./interfaces/IMessenger.sol";
import { TypeConverter } from "./libs/TypeConverter.sol";

contract OrderBook is IOrderBook {
    using TypeConverter for *;

    // ========== Errors ========== //
    // TODO use custom errors vs. strings for gas savings

    // ========== State Variables ========== //

    /// @notice the chain ID of this chain according to the messaging network used by this contract
    uint32 public chainId; 

    // version of the limit order system
    uint16 public constant version = 1;

    // TODO this messaging setup is unclear, but this is a simple stand-in
    // sends crosschain messages to report fills on this chain to other chains
    // receive crosschain messages to report fills on other chains to this chain
    address public messenger;
    // Alternative: chain-specific messengers
    // mapping(address => uint32) public messengerOriginId;// messenger contract address => this chain's origin ID for that messenger (it can be different for different messaging networks)
    // mapping(uint32 => address) public chainMessenger; // chain ID => contract to use for sending messages to and receiving messages from that chain

    // chain ID => number of seconds to wait for finality after fill deadline before allowing refunds
    mapping(uint32 destChainId => uint40 finalityBuffer) public destChainFinalityBuffer; 

    // only store full data about origin orders
    mapping(bytes32 orderId => Order) public localOrders;

    // store fill amounts for both origin and destination orders
    mapping(bytes32 orderId => uint128 filledAmount) public orderAmountOutFilled;

    // track nonces for each sender to ensure unique order IDs
    mapping(address sender => uint64 nonce) public senderNonces;

    // // TODO do we want to restrict tokens can be used?
    // mapping(address => bool) public acceptOrdersIn; // whether to accept orders with a given origin token
    // mapping(address => bool) public acceptFillsIn; // whether to accept fills with a given destination token

    // ========== Constructor ========== //

    constructor() {
        chainId = uint32(block.chainid); // TODO replace with messaging network chain ID if different
    }


    // ========== Initiating Orders ========== //

    function openOrder(OnchainOrderParams calldata orderParams) external override returns (bytes32 orderId) {
        // Validate order parameters

        if (uint256(orderParams.fillDeadline) < block.timestamp) {
            revert("OrderBook: fill deadline invalid");
        }

        if (orderParams.amountIn == 0 || orderParams.amountOut == 0) {
            revert("OrderBook: amountIn and amountOut must be greater than zero");
        }

        // TODO should we store a list of valid destination chains? What about destination tokens?
        // With the fillDeadline boundaries, the worst case is that an unsupported order can't be filled and is refunded after expiry
        // However, that may not be great UX

        // Create order
        uint64 nonce = senderNonces[msg.sender]++;

        orderId = getOrderId(OrderData({
            version: version, // origin contract version
            originChainId: chainId,
            sender: msg.sender.toBytes32(),
            nonce: nonce,
            destChainId: orderParams.destChainId,
            fillDeadline: uint64(orderParams.fillDeadline),
            tokenOut: orderParams.tokenOut,
            recipient: orderParams.recipient,
            amountOut: orderParams.amountOut,
            solver: orderParams.solver
        }));

        localOrders[orderId] = Order({
            version: version, // origin contract version
            status: OrderStatus.Created,
            destChainId: orderParams.destChainId,
            fillDeadline: orderParams.fillDeadline,
            nonce: nonce,
            tokenIn: orderParams.tokenIn,
            tokenOut: orderParams.tokenOut,
            sender: msg.sender,
            recipient: orderParams.recipient,
            amountIn: orderParams.amountIn,
            amountOut: orderParams.amountOut,
            solver: orderParams.solver
        });

        // Transfer tokens in from the sender
        IERC20(orderParams.tokenIn).transferFrom(msg.sender, address(this), uint256(orderParams.amountIn));

        emit OrderOpen(orderId, orderParams.tokenIn, orderParams.amountIn, orderParams.destChainId, orderParams.tokenOut, orderParams.amountOut, orderParams.solver);
    }

    // ========== Refunding Orders ========== //

    function requestCancelOrder(bytes32 orderId) external override {
        Order storage order = localOrders[orderId];

        // Validate that the order can be cancelled
        if (order.status != OrderStatus.Created) {
            revert("OrderBook: order not active");
        }

        if (order.sender != msg.sender) {
            revert("OrderBook: caller is not order sender");
        }

        if (uint256(order.fillDeadline) <= block.timestamp) {
            revert("OrderBook: order fill deadline passed. refund already available");
        }

        // Mark the order as cancel requested
        order.status = OrderStatus.CancelRequested;

        // Set the fill deadline to the current time
        // This will allow the caller to claim a refund after the finality buffer has passed
        order.fillDeadline = uint40(block.timestamp); // can't overflow until year 36812
        
        emit CancelRequest(orderId, order.fillDeadline);
    }


    // Note: this function allows anyone to trigger a refund of an order after its fill deadline + finality buffer has passed
    // This allows applications to gracefully handle refunds for orders that weren't filled 
    // Alternatively, if a user requested a refund, they can claim it here
    function claimRefund(bytes32 orderId) external override {
        Order storage order = localOrders[orderId];

        // Validate that the order can be refunded
        if (order.status != OrderStatus.Created || order.status != OrderStatus.CancelRequested) {
            revert("OrderBook: order not active or cancel requested");
        }

        // Check that the fill deadline + finality buffer has passed
        uint40 finalityBuffer = destChainFinalityBuffer[order.destChainId];
        if (uint256(order.fillDeadline) + finalityBuffer >= block.timestamp) {
            revert("OrderBook: order fill deadline not yet passed");
        }


        // Calculate the refund amount
        uint128 outFilled = orderAmountOutFilled[orderId];
        uint128 outRemaining = order.amountOut - outFilled;

        if (outRemaining == 0) {
            revert("OrderBook: order already fully filled");
        }

        // TODO need to think about rounding and precision loss with different token decimal values
        // We can cast to uin256 for multiplication and then cast back after division because order.amountOut >= outRemaining
        uint128 inRemaining = outFilled == 0 ? order.amountIn : ((uint256(order.amountIn) * outRemaining) / order.amountOut).toUint128();

        // Update the order amountIn and amountOut values to reflect the refund
        // This prevents double refunds if this function is called again
        order.amountIn -= inRemaining;
        order.amountOut -= outRemaining;

        // Set the order status to completed
        order.status = OrderStatus.Completed;

        // Transfer the remaining amount back to the sender
        IERC20(order.tokenIn).transfer(order.sender, uint256(inRemaining));

        emit RefundClaimed(orderId, order.sender, inRemaining);
    }


    // ========== Filling Orders ========== //
    function fillOrder(bytes32 orderId, OrderData calldata orderData, FillParams calldata fillerParams) external override {
        // Validate fill data
        if (chainId != orderData.destChainId) {
            revert("OrderBook: fill data destination chain ID does not match protocol chain ID");
        }

        if (uint256(orderData.fillDeadline) < block.timestamp) {
            revert("OrderBook: order fill deadline passed");
        }

        if (orderData.version != version) {
            revert("OrderBook: order version not supported");
        }

        // If the solver is specified, ensure that the caller is the designated solver
        address solver = orderData.solver.toAddress();
        if (solver != address(0) && solver != msg.sender) {
            revert("OrderBook: caller is not designated solver for order");
        }

        // Ensure the provided order ID matches the computed order ID from the order data
        // This check is not strictly required, but it is a useful sanity check for solvers
        // to ensure they have the order data correct
        bytes32 orderId_ = getOrderId(orderData);

        if (orderId != orderId_) {
            revert("OrderBook: order ID does not match fill data");
        }

        // Calculate fill amount as the minimum of the filler provided amount and the remaining unfilled amount
        uint128 outFilled = orderAmountOutFilled[orderId];
        uint128 outRemaining = orderData.amountOut - outFilled;
        if (outRemaining == 0) {
            revert("OrderBook: order already fully filled");
        }
        bool fullFill = fillerParams.amountOutToFill >= outRemaining;
        uint128 fillAmount = fullFill ? outRemaining : fillerParams.amountOutToFill;

        // Update order fill amount
        orderAmountOutFilled[orderId] += fillAmount;

        // Handle releasing the corresponding amount of origin tokens to the filler
        if (chainId == orderData.originChainId) {
            // If a full fill, mark the order as completed
            if (fullFill) {
                localOrders[orderId].status = OrderStatus.Completed;
                emit OrderCompleted(orderId);
            }

            // Calculate the amount of origin tokens to release to the filler
            Order storage order = localOrders[orderId];
            // TODO same concerns about rounding and precision loss with different token decimal values
            uint128 inToRelease = order.amountOut == fillAmount ? order.amountIn : ((uint256(order.amountIn) * fillAmount) / order.amountOut).toUint128();

            // If this is a fill on the origin chain, we can immediately release the corresponding amount of origin tokens to the recipient
            // This is because the origin and destination chains are the same, so no cross-chain messaging is needed
            IERC20(order.tokenIn).transferFrom(address(this), fillerParams.originRecipient.toAddress(), uint256(inToRelease));
        }
        
        // Transfer tokens from the solver to the recipient
        IERC20(orderData.tokenOut.toAddress()).transferFrom(msg.sender, orderData.recipient.toAddress(), uint256(fillAmount));
        
        // This block is split out to allow the above transfer to happen before any cross-chain messaging
        if (chainId != orderData.originChainId) {
            // If this is a fill on a different chain than the origin chain, we need to send a message back to the origin chain to release the corresponding amount of origin tokens to the recipient
            // TODO implement cross-chain messaging to report the fill back to the origin chain
            // TODO determine best method for batching these messages
            IMessenger(messenger).sendFillReport(
                orderData.originChainId,
                FillReport({
                    orderId: orderId,
                    originRecipient: fillerParams.originRecipient,
                    amountOutFilled: fillAmount
                })
            );
        }

        emit Fill(orderId, msg.sender, fillAmount);
    }

    // ========== Receiving Fill Reports ========== //

    // TODO allow batch reporting to save gas and reduce the number of required messages
    function reportFill(FillReport calldata report) external override {
        Order storage order = localOrders[report.orderId];

        if (!(order.status == OrderStatus.Created || order.status == OrderStatus.CancelRequested)) {
            revert("OrderBook: order not active");
        }

        if (msg.sender != messenger) {
            revert("OrderBook: caller is not permitted messenger for destination chain");
        }

        // Update the fill amount for the order
        orderAmountOutFilled[report.orderId] += report.amountOutFilled;
        uint128 outFilled = orderAmountOutFilled[report.orderId];
        if (outFilled == order.amountOut) {
            order.status = OrderStatus.Completed;
            emit OrderCompleted(report.orderId);
        }

        // Calculate the corresponding amount of origin tokens to release to the solver's designated recipient
        // TODO same concerns about rounding and precision loss with different token decimal values
        uint128 inToRelease = order.amountOut == report.amountOutFilled ? order.amountIn : ((uint256(order.amountIn) * report.amountOutFilled) / order.amountOut).toUint128();

        // Transfer the corresponding amount of origin tokens to the filler
        IERC20(order.tokenIn).transferFrom(address(this), report.originRecipient.toAddress(), uint256(inToRelease));
    }


    // Order IDs are unique across chains and allow using fill data to compute the identifier
    // This is useful for tracking data against orders on both the origin and destination chains
    function getOrderId(OrderData memory orderData) internal pure returns (bytes32) {
        return keccak256(abi.encodePacked(
            orderData.version,
            orderData.originChainId,
            orderData.sender,
            orderData.nonce,
            orderData.destChainId,
            orderData.fillDeadline,
            orderData.amountOut,
            orderData.tokenOut,
            orderData.recipient,
            orderData.solver
        ));
    }
}
