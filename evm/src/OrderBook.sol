// SPDX-License-Identifier: GPL-3.0
pragma solidity 0.8.26;

import { IERC20 } from "../lib/common/src/interfaces/IERC20.sol";
import { AccessControlUpgradeable } from "../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/access/AccessControlUpgradeable.sol";
import { ERC712ExtendedUpgradeable } from "../lib/common/src/ERC712ExtendedUpgradeable.sol";

import { IOrderBook } from "./interfaces/IOrderBook.sol";
import { IMessenger } from "./interfaces/IMessenger.sol";
import { TypeConverter } from "./libs/TypeConverter.sol";
import { TransferHelper } from "./libs/TransferHelper.sol";

abstract contract OrderBookStorageLayout {
    /// @custom:storage-location erc7201:M0.storage.OrderBook
    struct OrderBookStorageStruct {
        // destination configuration
        mapping(uint32 destChainId => IOrderBook.Destination) destinations;

        // only store full data about origin orders
        mapping(bytes32 orderId => IOrderBook.Order) localOrders;

        // store fill amounts for both origin and destination orders
        mapping(bytes32 orderId => uint128 filledAmount) orderAmountOutFilled;

        // track nonces for each sender to ensure unique order IDs
        mapping(address sender => uint64 nonce) senderNonces;
    }

    // keccak256(abi.encode(uint256(keccak256("M0.storage.OrderBook")) - 1)) & ~bytes32(uint256(0xff))
    bytes32 private constant _ORDER_BOOK_STORAGE_LOCATION =
        0x820bf725beb8a0ae85e433f17c2d2091cee8f490a62ab8bd0d9dd95db3dddc00;

    function _getOrderBookStorageLocation() internal pure returns (OrderBookStorageStruct storage $) {
        assembly {
            $.slot := _ORDER_BOOK_STORAGE_LOCATION
        }
    }
}

contract OrderBook is IOrderBook, OrderBookStorageLayout, AccessControlUpgradeable, ERC712ExtendedUpgradeable {
    using TypeConverter for *;
    using TransferHelper for IERC20;

    // ========== State Variables ========== //

    /// @notice Version of the limit order system
    uint16 public constant VERSION = 1;

    /// @notice the type hash used for gasless order submission
    /// @dev keccak256("GaslessOrderParams(uint32 originChainId,address tokenIn,uint32 destChainId,uint128 amountIn,uint128 amountOut,address sender,bytes32 recipient,uint40 openDeadline,uint40 fillDeadline,bytes32 solver)")
    bytes32 public constant GASLESS_ORDER_TYPEHASH = 0xb92a85c378d8070874bdbc3157612525c745e3714640049872848bf1a261b5e8;

    /// @notice the chain ID of this chain according to the messaging network used by this contract
    uint32 public immutable chainId; 

    // TODO this messaging setup is unclear, but this is a simple stand-in
    // sends crosschain messages to report fills on this chain to other chains
    // receive crosschain messages to report fills on other chains to this chain
    address public immutable messenger;
    // Alternative: chain-specific messengers
    // mapping(address => uint32) public messengerOriginId;// messenger contract address => this chain's origin ID for that messenger (it can be different for different messaging networks)
    // mapping(uint32 => address) public chainMessenger; // chain ID => contract to use for sending messages to and receiving messages from that chain

    /* ========== Construct and Initialize ========== */

    constructor(uint32 chainId_, address messenger_) {
        chainId = chainId_;
        messenger = messenger_;
    }

    function initialize(address admin) external initializer {
        __ERC712ExtendedUpgradeable_init("M0 OrderBook");
    }

    /* ========== Initiating Orders ========== */

    /// @inheritdoc IOrderBook
    function openOrder(OrderParams calldata orderParams_) external override returns (bytes32) {
        return _openOrder(msg.sender, orderParams_);
    }

    /// @inheritdoc IOrderBook
    function openOrderFor(
        GaslessOrderParams calldata orderParams_,
        bytes calldata signature_
    ) external override returns (bytes32) {
        // Verify signature
        _revertIfInvalidSignature(orderParams_.sender, _getDigest(_getGaslessOrderInternalDigest(orderParams_)), signature_);

        // Verify origin chain and sender nonce
        if (orderParams_.originChainId != chainId) revert InvalidOriginChain();
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        // Requiring a nonce in the order provides replay protection for the sender
        if (orderParams_.nonce != $.senderNonces[orderParams_.sender]) revert InvalidNonce();

        // Open order on behalf of the sender
        return _openOrder(orderParams_.sender, OrderParams({
            destChainId: orderParams_.destChainId,
            tokenIn: orderParams_.tokenIn,
            tokenOut: orderParams_.tokenOut,
            amountIn: orderParams_.amountIn,
            amountOut: orderParams_.amountOut,
            recipient: orderParams_.recipient,
            fillDeadline: orderParams_.fillDeadline,
            solver: orderParams_.solver
        }));
    }

    function _openOrder(address sender_, OrderParams memory orderParams_) internal returns (bytes32) {
        // Validate order parameters
        if (uint256(orderParams_.fillDeadline) < block.timestamp) revert InvalidDeadline();
        if (orderParams_.amountIn == 0) revert AmountInZero();
        if (orderParams_.amountOut == 0) revert AmountOutZero();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();

        // Destination chain must either be the current chain or a supported destination
        if (orderParams_.destChainId != chainId && !$.destinations[orderParams_.destChainId].isSupported) revert InvalidDestinationChain();

        // Create order
        uint64 nonce_ = $.senderNonces[sender_]++;

        bytes32 orderId_ = getOrderId(OrderData({
            version: VERSION, // origin contract version
            originChainId: chainId,
            sender: sender_.toBytes32(),
            nonce: nonce_,
            destChainId: orderParams_.destChainId,
            fillDeadline: uint64(orderParams_.fillDeadline),
            tokenOut: orderParams_.tokenOut,
            recipient: orderParams_.recipient,
            amountOut: orderParams_.amountOut,
            solver: orderParams_.solver
        }));

        $.localOrders[orderId_] = Order({
            version: VERSION, // origin contract version
            status: OrderStatus.Created,
            destChainId: orderParams_.destChainId,
            refundRequestedAt: uint40(0),
            fillDeadline: orderParams_.fillDeadline,
            nonce: nonce_,
            tokenIn: orderParams_.tokenIn,
            tokenOut: orderParams_.tokenOut,
            sender: sender_,
            recipient: orderParams_.recipient,
            amountIn: orderParams_.amountIn,
            amountOut: orderParams_.amountOut,
            solver: orderParams_.solver
        });

        // Transfer tokens in from the sender, ensuring the required amount is received
        IERC20(orderParams_.tokenIn).safeTransferExactFrom(sender_, address(this), uint256(orderParams_.amountIn));

        emit OrderOpen(orderId_, orderParams_.tokenIn, orderParams_.amountIn, orderParams_.destChainId, orderParams_.tokenOut, orderParams_.amountOut, orderParams_.solver);

        return orderId_;
    }

    // ========== Refunding Orders ========== //

    /// @inheritdoc IOrderBook
    function requestCancelOrder(bytes32 orderId_) external override {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[orderId_];

        // Validate that the order can be cancelled and the caller is the sender
        if (order.status != OrderStatus.Created) revert InvalidOrderStatus();
        if (uint256(order.fillDeadline) <= block.timestamp) revert OrderExpired();
        if (order.sender != msg.sender) revert NotAuthorized();

        // Mark the order as cancel requested
        order.status = OrderStatus.CancelRequested;

        // Set the refundRequestedAt timestamp to the current time
        // This will allow the caller to claim a refund after the finality buffer has passed
        order.refundRequestedAt = uint40(block.timestamp); // can't overflow until year 36812

        emit CancelRequested(orderId_, order.refundRequestedAt);
    }

    /// @inheritdoc IOrderBook
    function claimRefund(bytes32 orderId_) external override {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[orderId_];

        // Validate that the order can be refunded
        // If the order is local, the finality buffer is 0
        uint40 finalityBuffer_ = order.destChainId == chainId ? 0 : $.destinations[order.destChainId].finalityBuffer;
        if (order.status == OrderStatus.Created) {
            // If the order is still in Created status, it can only be refunded if the fill deadline + finality buffer has passed
            if (uint256(order.fillDeadline) + finalityBuffer_ >= block.timestamp) revert FinalityPending();
        } else if (order.status == OrderStatus.CancelRequested) {
            // If the order is in CancelRequested status, it can only be refunded if the refund was requested at least finality buffer ago
            if (uint256(order.refundRequestedAt) + finalityBuffer_ >= block.timestamp) revert FinalityPending();
        } else {
            // If the order is in any other status, it cannot be refunded
            revert InvalidOrderStatus();
        }

        // Calculate the refund amount
        uint128 outFilled_ = $.orderAmountOutFilled[orderId_];
        uint128 outRemaining_ = order.amountOut - outFilled_;

        // TODO need to think about rounding and precision loss with different token decimal values
        // We can cast to uin256 for multiplication and then cast back after division because order.amountOut >= outRemaining
        uint128 inRemaining_ = outFilled_ == 0 ? order.amountIn : ((uint256(order.amountIn) * outRemaining_) / order.amountOut).toUint128();

        // Update the order amountIn and amountOut values to reflect the refund
        // This prevents double refunds if this function is called again
        order.amountIn -= inRemaining_;
        order.amountOut -= outRemaining_;

        // Set the order status to completed
        order.status = OrderStatus.Completed;

        // Transfer the remaining amount back to the sender
        IERC20(order.tokenIn).safeTransfer(order.sender, uint256(inRemaining_));

        emit RefundClaimed(orderId_, order.sender, inRemaining_);
    }


    /* ========== Filling Orders ========== */

    /// @inheritdoc IOrderBook
    function fillOrder(bytes32 orderId_, OrderData calldata orderData_, FillParams calldata fillerParams_) external override {
        // Validate fill data
        if (chainId != orderData_.destChainId) revert InvalidDestinationChain();
        if (uint256(orderData_.fillDeadline) < block.timestamp) revert OrderExpired();
        if (orderData_.version != VERSION) revert InvalidOrderVersion();

        // If the solver is specified, ensure that the caller is the designated solver
        address solver_ = orderData_.solver.toAddress();
        if (solver_ != address(0) && solver_ != msg.sender) revert NotAuthorized();

        // Ensure the provided order ID matches the computed order ID from the order data
        // This check is not strictly required, but it is a useful sanity check for solvers
        // to ensure they have the order data correct
        if (orderId_ != getOrderId(orderData_)) revert OrderIdMismatch();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();

        // Calculate fill amount as the minimum of the filler provided amount and the remaining unfilled amount
        uint128 outFilled_ = $.orderAmountOutFilled[orderId_];
        uint128 outRemaining_ = orderData_.amountOut - outFilled_;
        if (outRemaining_ == 0) revert OrderFilled();
        bool fullFill_ = fillerParams_.amountOutToFill >= outRemaining_;
        uint128 fillAmount_ = fullFill_ ? outRemaining_ : fillerParams_.amountOutToFill;

        // Update order fill amount
        $.orderAmountOutFilled[orderId_] += fillAmount_;

        // Handle releasing the corresponding amount of origin tokens to the filler
        if (chainId == orderData_.originChainId) {
            // If a full fill, mark the order as completed
            Order storage order = $.localOrders[orderId_];

            if (fullFill_) {
                order.status = OrderStatus.Completed;
                emit OrderCompleted(orderId_);
            }

            // Calculate the amount of origin tokens to release to the filler            
            // TODO same concerns about rounding and precision loss with different token decimal values
            uint128 inToRelease_ = order.amountOut == fillAmount_ ? order.amountIn : ((uint256(order.amountIn) * fillAmount_) / order.amountOut).toUint128();

            // If this is a fill on the origin chain, we can immediately release the corresponding amount of origin tokens to the recipient
            // This is because the origin and destination chains are the same, so no cross-chain messaging is needed
            IERC20(order.tokenIn).safeTransferExact(fillerParams_.originRecipient.toAddress(), uint256(inToRelease_));
        }
        
        // Transfer tokens from the solver to the recipient
        IERC20(orderData_.tokenOut.toAddress()).safeTransferExactFrom(msg.sender, orderData_.recipient.toAddress(), uint256(fillAmount_));

        // This block is split out to allow the above transfer to happen before any cross-chain messaging
        if (chainId != orderData_.originChainId) {
            // If this is a fill on a different chain than the origin chain, 
            // we need to send a message back to the origin chain to release 
            // the corresponding amount of origin tokens to the recipient
            IMessenger(messenger).sendFillReport(
                orderData_.originChainId,
                FillReport({
                    orderId: orderId_,
                    originRecipient: fillerParams_.originRecipient,
                    amountOutFilled: fillAmount_
                })
            );
        }

        emit Fill(orderId_, msg.sender, fillAmount_);
    }

    /* ========== Receiving Fill Reports ========== */

    /// @inheritdoc IOrderBook
    function reportFill(FillReport calldata report_) external override {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.localOrders[report_.orderId];

        // Validate the fill report and sender
        if (msg.sender != messenger) revert NotAuthorized();
        if (order.status != OrderStatus.Created && order.status != OrderStatus.CancelRequested) revert InvalidOrderStatus();
        
        // Update the fill amount for the order
        uint128 outFilled = ($.orderAmountOutFilled[report_.orderId] += report_.amountOutFilled);
        if (outFilled == order.amountOut) {
            order.status = OrderStatus.Completed;
            emit OrderCompleted(report_.orderId);
        }

        // Calculate the corresponding amount of origin tokens to release to the solver's designated recipient
        // TODO same concerns about rounding and precision loss with different token decimal values
        uint128 inToRelease_ = order.amountOut == report_.amountOutFilled ? order.amountIn : ((uint256(order.amountIn) * report_.amountOutFilled) / order.amountOut).toUint128();

        // Transfer the corresponding amount of origin tokens to the filler
        IERC20(order.tokenIn).safeTransferExact(report_.originRecipient.toAddress(), uint256(inToRelease_));
    }

    /* ========== Admin Functions ========== */

    function setDestinationConfig(uint32 destChainId_, bool isSupported_, uint40 finalityBuffer_) external override {
        // TODO add access control

        if (isSupported_ && finalityBuffer_ == 0) revert InvalidFinalityBuffer();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        $.destinations[destChainId_] = Destination({
            isSupported: isSupported_,
            finalityBuffer: finalityBuffer_
        });
    }

    /* ========== View Functions ========== */

    // Order IDs are unique across chains and allow using fill data to compute the identifier
    // This is useful for tracking data against orders on both the origin and destination chains
    function getOrderId(OrderData memory orderData_) public pure override returns (bytes32) {
        return keccak256(abi.encodePacked(
            orderData_.version,
            orderData_.originChainId,
            orderData_.sender,
            orderData_.nonce,
            orderData_.destChainId,
            orderData_.fillDeadline,
            orderData_.amountOut,
            orderData_.tokenOut,
            orderData_.recipient,
            orderData_.solver
        ));
    }

    function getOrder(bytes32 orderId_) external view override returns (Order memory) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.localOrders[orderId_];
    }

    function getAmountOutFilled(bytes32 orderId_) external view override returns (uint128) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.orderAmountOutFilled[orderId_];
    }

    function isDestinationSupported(uint32 destChainId_) external view override returns (bool) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.destinations[destChainId_].isSupported;
    }

    function getDestinationFinalityBuffer(uint32 destChainId_) external view override returns (uint40) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.destinations[destChainId_].finalityBuffer;
    }

    function _getGaslessOrderInternalDigest(GaslessOrderParams memory orderParams_) internal pure returns (bytes32) {
        return keccak256(abi.encode(
            GASLESS_ORDER_TYPEHASH,
            orderParams_.originChainId,
            orderParams_.tokenIn,
            orderParams_.destChainId,
            orderParams_.amountIn,
            orderParams_.amountOut,
            orderParams_.sender,
            orderParams_.nonce,
            orderParams_.recipient,
            orderParams_.fillDeadline,
            orderParams_.solver
        ));
    }
}
