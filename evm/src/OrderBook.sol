// SPDX-License-Identifier: BUSL-1.1
pragma solidity 0.8.33;

import { IERC20 } from "../lib/common/lib/openzeppelin-contracts-upgradeable/lib/openzeppelin-contracts/contracts/interfaces/IERC20.sol";
import { IERC20Extended } from "../lib/common/src/interfaces/IERC20Extended.sol";
import { AccessControlUpgradeable } from "../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/access/AccessControlUpgradeable.sol";
import { PausableUpgradeable } from "../lib/common/lib/openzeppelin-contracts-upgradeable/contracts/utils/PausableUpgradeable.sol";
import { ERC712ExtendedUpgradeable } from "../lib/common/src/ERC712ExtendedUpgradeable.sol";
import { TypeConverter } from "../lib/common/src/libs/TypeConverter.sol";
import { UIntMath } from "../lib/common/src/libs/UIntMath.sol";
import { SafeERC20 } from "./libs/SafeERC20.sol";

import { IOrderBook } from "./interfaces/IOrderBook.sol";
import { IPortalV2Like } from "./interfaces/IPortalV2Like.sol";

abstract contract OrderBookStorageLayout {
    /// @custom:storage-location erc7201:M0.storage.OrderBook
    struct OrderBookStorageStruct {
        // supported destination chains
        mapping(uint32 destChainId => bool isSupported) supportedDestinations;
        // only store full data about origin orders, status is tracked for all orders
        mapping(bytes32 orderId => IOrderBook.Order) orders;
        // store fill amounts for both origin and destination orders
        mapping(bytes32 orderId => IOrderBook.FilledAmounts) filledAmounts;
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

contract OrderBook is
    IOrderBook,
    OrderBookStorageLayout,
    AccessControlUpgradeable,
    PausableUpgradeable,
    ERC712ExtendedUpgradeable
{
    using TypeConverter for *;
    using SafeERC20 for IERC20;
    using UIntMath for uint256;

    // ========== State Variables ========== //

    bytes32 public constant PAUSER_ROLE = keccak256("PAUSER_ROLE");

    /// @notice Version of the limit order system
    uint16 public constant VERSION = 1;

    /// @notice the type hash used for gasless order submission
    /// @dev keccak256("GaslessOrderParams(uint16 version,address sender,uint64 nonce,uint32 originChainId,uint32 destChainId,uint32 fillDeadline,address tokenIn,bytes32 tokenOut,uint128 amountIn,uint128 amountOut,bytes32 recipient,bytes32 solver)")
    bytes32 public constant GASLESS_ORDER_TYPEHASH = 0xdcc220f897990a71a7c6f1069339af0681016bb96f2d791f2214e234d7029603;

    /// @notice the type hash used for cancel order signatures
    /// @dev keccak256("CancelOrder(bytes32 orderId, address bridgeAdapter, bytes bridgeAdapterArgs)")
    bytes32 public constant CANCEL_ORDER_TYPEHASH = 0x6919f4958bcd1b5b4e13b800c6d41c4792cfc2a12d0bd9ad19da6e0bfe8ac04f;

    /// @notice the portal contract used for cross-chain communication
    /// @dev sends crosschain messages to report fills and cancels on this chain to other chains
    ///      receives crosschain messages reporting fills and cancels on other chains to this chain
    address public immutable portal;

    /* ========== Construct and Initialize ========== */

    constructor(address portal_) {
        _disableInitializers(); // prevent initializing the implementation contract

        if (portal_ == address(0)) revert ZeroPortal();
        portal = portal_;
    }

    function initialize(address admin, address pauser) external initializer {
        if (admin == address(0)) revert ZeroAdmin();
        if (pauser == address(0)) revert ZeroPauser();

        __ERC712ExtendedUpgradeable_init("M0 OrderBook");

        _grantRole(DEFAULT_ADMIN_ROLE, admin);
        _grantRole(PAUSER_ROLE, pauser);
    }

    /* ========== Creating Orders ========== */

    /// @inheritdoc IOrderBook
    function openOrder(OrderParams calldata orderParams_) external override returns (bytes32 orderId_) {
        orderId_ = _openOrder(msg.sender, orderParams_);
    }

    /// @inheritdoc IOrderBook
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        uint8 v_,
        bytes32 r_,
        bytes32 s_
    ) external override returns (bytes32 orderId_) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                msg.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                v_,
                r_,
                s_
            )
        {} catch {}
        orderId_ = _openOrder(msg.sender, orderParams_);
    }

    /// @inheritdoc IOrderBook
    function openOrderWithPermit(
        OrderParams calldata orderParams_,
        uint256 deadline_,
        bytes memory permitSignature_
    ) external override returns (bytes32 orderId_) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                msg.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                permitSignature_
            )
        {} catch {}
        orderId_ = _openOrder(msg.sender, orderParams_);
    }

    /// @inheritdoc IOrderBook
    function openOrderFor(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_
    ) external override returns (bytes32 orderId_) {
        orderId_ = _openOrderFor(orderParams_, orderSignature_);
    }

    /// @inheritdoc IOrderBook
    function openOrderForWithPermit(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_,
        uint256 deadline_,
        uint8 v_,
        bytes32 r_,
        bytes32 s_
    ) external override returns (bytes32 orderId_) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                orderParams_.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                v_,
                r_,
                s_
            )
        {} catch {}
        orderId_ = _openOrderFor(orderParams_, orderSignature_);
    }

    /// @inheritdoc IOrderBook
    function openOrderForWithPermit(
        GaslessOrderParams calldata orderParams_,
        bytes calldata orderSignature_,
        uint256 deadline_,
        bytes memory permitSignature_
    ) external override returns (bytes32 orderId_) {
        try
            IERC20Extended(orderParams_.tokenIn).permit(
                orderParams_.sender,
                address(this),
                uint256(orderParams_.amountIn),
                deadline_,
                permitSignature_
            )
        {} catch {}
        orderId_ = _openOrderFor(orderParams_, orderSignature_);
    }

    function _openOrder(
        address sender_,
        OrderParams memory orderParams_
    ) internal whenNotPaused returns (bytes32 orderId_) {
        // Validate order parameters
        if (uint256(orderParams_.fillDeadline) < block.timestamp) revert InvalidDeadline();
        if (orderParams_.amountIn == 0) revert AmountInZero();
        if (orderParams_.amountOut == 0) revert AmountOutZero();
        if (orderParams_.recipient == bytes32(0)) revert InvalidRecipient();
        if (orderParams_.solver == orderParams_.recipient) revert InvalidSolver();

        // Validate that tokenIn and tokenOut are not the same for same-chain orders
        uint32 chainId = block.chainid.safe32();
        if (orderParams_.destChainId == chainId && orderParams_.tokenOut == orderParams_.tokenIn.toBytes32())
            revert SameTokenOrder();

        // Destination chain must either be the current chain or a supported destination
        if (!isDestinationSupported(orderParams_.destChainId)) revert InvalidDestinationChain();

        // Create order
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        uint64 nonce_ = $.senderNonces[sender_]++;

        orderId_ = getOrderId(
            OrderData({
                version: VERSION, // origin contract version
                originChainId: chainId,
                sender: sender_.toBytes32(),
                nonce: nonce_,
                destChainId: orderParams_.destChainId,
                createdAt: uint64(block.timestamp),
                fillDeadline: uint64(orderParams_.fillDeadline),
                tokenIn: orderParams_.tokenIn.toBytes32(),
                tokenOut: orderParams_.tokenOut,
                recipient: orderParams_.recipient,
                amountIn: orderParams_.amountIn,
                amountOut: orderParams_.amountOut,
                solver: orderParams_.solver
            })
        );

        // Shouldn't be needed due to uniqueness of order ID, but
        // it is good to be explicit about the expected state
        // and this protects against a (very unlikely) hash collision
        if ($.orders[orderId_].status != OrderStatus.DoesNotExist) revert OrderAlreadyExists();

        $.orders[orderId_] = Order({
            version: VERSION, // origin contract version
            status: OrderStatus.Created,
            destChainId: orderParams_.destChainId,
            createdAt: uint32(block.timestamp),
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

        emit OrderOpened(
            orderId_,
            sender_,
            orderParams_.tokenIn,
            orderParams_.amountIn,
            orderParams_.destChainId,
            orderParams_.tokenOut,
            orderParams_.amountOut,
            orderParams_.solver
        );
    }

    function _openOrderFor(
        GaslessOrderParams calldata orderParams_,
        bytes calldata signature_
    ) internal returns (bytes32 orderId_) {
        // Verify signature
        if (signature_.length == 64) {
            (bytes32 r, bytes32 vs) = abi.decode(signature_, (bytes32, bytes32));
            _revertIfInvalidSignature(orderParams_.sender, getGaslessOrderDigest(orderParams_), r, vs);
        } else {
            _revertIfInvalidSignature(orderParams_.sender, getGaslessOrderDigest(orderParams_), signature_);
        }

        // Verify origin chain and sender nonce
        if (orderParams_.originChainId != block.chainid) revert InvalidOriginChain();
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        // Requiring a nonce in the order provides replay protection for the sender
        if (orderParams_.nonce != $.senderNonces[orderParams_.sender]) revert InvalidNonce();
        // Verify version matches the current version
        if (orderParams_.version != VERSION) revert InvalidOrderVersion();

        // Open order on behalf of the sender
        orderId_ = _openOrder(
            orderParams_.sender,
            OrderParams({
                destChainId: orderParams_.destChainId,
                tokenIn: orderParams_.tokenIn,
                tokenOut: orderParams_.tokenOut,
                amountIn: orderParams_.amountIn,
                amountOut: orderParams_.amountOut,
                recipient: orderParams_.recipient,
                fillDeadline: orderParams_.fillDeadline,
                solver: orderParams_.solver
            })
        );
    }

    /* ========== Refunding Orders ========== */

    /// @inheritdoc IOrderBook
    function cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _cancelOrder(orderId_, orderData_, address(0), new bytes(0));
    }

    /// @inheritdoc IOrderBook
    function cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata bridgeAdapterArgs_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _cancelOrder(orderId_, orderData_, address(0), bridgeAdapterArgs_);
    }

    /// @inheritdoc IOrderBook
    function cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _cancelOrder(orderId_, orderData_, bridgeAdapter_, bridgeAdapterArgs_);
    }

    function _cancelOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        address bridgeAdapter_,
        bytes memory bridgeAdapterArgs_
    ) internal returns (bytes32 messageId_) {
        // Cancellation Authorization:
        // 1. Before deadline:
        //   - Same-chain orders: sender OR recipient
        //   - Cross-chain orders: recipient only (sender address is non-native)
        // 2. After deadline:
        //   - All orders: anyone (enables permissionless refunds)
        if (
            block.timestamp <= orderData_.fillDeadline &&
            !(orderData_.recipient.toAddress() == msg.sender ||
                (orderData_.originChainId == block.chainid && orderData_.sender.toAddress() == msg.sender))
        ) revert NotAuthorized();

        messageId_ = _cancel(orderId_, orderData_, bridgeAdapter_, bridgeAdapterArgs_);
    }

    /// @inheritdoc IOrderBook
    function cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata signature_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _cancelOrderFor(orderId_, orderData_, signature_, address(0), new bytes(0));
    }

    /// @inheritdoc IOrderBook
    function cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata signature_,
        bytes calldata bridgeAdapterArgs_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _cancelOrderFor(orderId_, orderData_, signature_, address(0), bridgeAdapterArgs_);
    }

    /// @inheritdoc IOrderBook
    function cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata signature_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _cancelOrderFor(orderId_, orderData_, signature_, bridgeAdapter_, bridgeAdapterArgs_);
    }

    function _cancelOrderFor(
        bytes32 orderId_,
        OrderData calldata orderData_,
        bytes calldata signature_,
        address bridgeAdapter_,
        bytes memory bridgeAdapterArgs_
    ) internal returns (bytes32 messageId_) {
        // Verify signature
        if (signature_.length == 64) {
            (bytes32 r, bytes32 vs) = abi.decode(signature_, (bytes32, bytes32));
            _revertIfInvalidSignature(
                orderData_.recipient.toAddress(),
                getCancelOrderDigest(orderId_, bridgeAdapter_, bridgeAdapterArgs_),
                r,
                vs
            );
        } else {
            _revertIfInvalidSignature(
                orderData_.recipient.toAddress(),
                getCancelOrderDigest(orderId_, bridgeAdapter_, bridgeAdapterArgs_),
                signature_
            );
        }

        messageId_ = _cancel(orderId_, orderData_, bridgeAdapter_, bridgeAdapterArgs_);
    }

    function _cancel(
        bytes32 orderId_,
        OrderData calldata orderData_,
        address bridgeAdapter_,
        bytes memory bridgeAdapterArgs_
    ) internal whenNotPaused returns (bytes32 messageId_) {
        _revertIfOrderIdMismatch(orderId_, orderData_);

        // Can't cancel an order before it's created
        if (orderData_.createdAt > block.timestamp) revert InvalidTimestamp();

        Order storage order = _getOrderBookStorageLocation().orders[orderId_];
        _revertIfInvalidStatusToFillOrCancel(order, orderData_);

        // Order destination chain must be this chain
        if (block.chainid != orderData_.destChainId) revert InvalidDestinationChain();

        // Calculate amount to refund
        IOrderBook.FilledAmounts storage filledAmounts = _getOrderBookStorageLocation().filledAmounts[orderId_];
        uint128 amountInRemaining_ = orderData_.amountIn - filledAmounts.amountInReleased;
        // filledAmounts.amountInRefunded doesn't need to be considered here because it must be zero prior to cancellation

        // Update order status and refunded amount
        order.status = OrderStatus.Cancelled;
        filledAmounts.amountInRefunded += amountInRemaining_;

        if (orderData_.originChainId == block.chainid) {
            if (msg.value != 0) revert InvalidMsgValue();

            // Local orders can be immediately refunded
            IERC20(order.tokenIn).safeTransfer(order.sender, uint256(amountInRemaining_));

            emit RefundClaimed(orderId_, order.sender, amountInRemaining_);
        } else {
            // Cross-chain orders require sending a cancel report to the origin chain
            CancelReport memory report_ = CancelReport({
                orderId: orderId_,
                orderSender: orderData_.sender,
                tokenIn: orderData_.tokenIn,
                amountInToRefund: amountInRemaining_
            });

            messageId_ = bridgeAdapter_ == address(0)
                ? IPortalV2Like(portal).sendCancelReport{ value: msg.value }(
                    orderData_.originChainId,
                    report_,
                    msg.sender.toBytes32(), // refundAddress
                    bridgeAdapterArgs_
                )
                : IPortalV2Like(portal).sendCancelReport{ value: msg.value }(
                    orderData_.originChainId,
                    report_,
                    msg.sender.toBytes32(), // refundAddress
                    bridgeAdapter_,
                    bridgeAdapterArgs_
                );
        }

        emit OrderCancelled(orderId_, messageId_);
    }

    /* ========== Filling Orders ========== */

    /// @inheritdoc IOrderBook
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _fillOrder(orderId_, orderData_, fillerParams_, address(0), new bytes(0));
    }

    /// @inheritdoc IOrderBook
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        bytes calldata bridgeAdapterArgs_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _fillOrder(orderId_, orderData_, fillerParams_, address(0), bridgeAdapterArgs_);
    }

    /// @inheritdoc IOrderBook
    function fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        address bridgeAdapter_,
        bytes calldata bridgeAdapterArgs_
    ) external payable override returns (bytes32 messageId_) {
        messageId_ = _fillOrder(orderId_, orderData_, fillerParams_, bridgeAdapter_, bridgeAdapterArgs_);
    }

    function _fillOrder(
        bytes32 orderId_,
        OrderData calldata orderData_,
        FillParams calldata fillerParams_,
        address bridgeAdapter_,
        bytes memory bridgeAdapterArgs_
    ) internal whenNotPaused returns (bytes32 messageId_) {
        _revertIfOrderIdMismatch(orderId_, orderData_);

        // Validate fill data
        if (block.chainid != orderData_.destChainId) revert InvalidDestinationChain();
        if (orderData_.fillDeadline < block.timestamp) revert OrderExpired();
        if (orderData_.version != VERSION) revert InvalidOrderVersion();
        if (orderData_.createdAt > block.timestamp) revert InvalidTimestamp();
        if (fillerParams_.amountOutToFill == 0) revert FillAmountZero();
        if (fillerParams_.originRecipient == bytes32(0)) revert InvalidRecipient();

        // If the solver is specified, ensure that the caller is the designated solver
        address solver_ = orderData_.solver.toAddress();
        if (solver_ != address(0) && solver_ != msg.sender) revert NotAuthorized();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.orders[orderId_];
        _revertIfInvalidStatusToFillOrCancel(order, orderData_);

        uint128 amountInToRelease_;
        uint128 amountOutToFill_;
        // Local scope to avoid stack too deep errors
        {
            // Calculate fill amount as the minimum of the filler provided amount and the remaining unfilled amount
            IOrderBook.FilledAmounts storage filledAmounts = $.filledAmounts[orderId_];

            bool fullFill_;
            (fullFill_, amountInToRelease_, amountOutToFill_) = _calculateFill(
                orderData_.amountIn,
                orderData_.amountOut,
                filledAmounts.amountInReleased,
                filledAmounts.amountOutFilled,
                fillerParams_.amountOutToFill
            );

            // Update filled amounts
            filledAmounts.amountOutFilled += amountOutToFill_;
            filledAmounts.amountInReleased += amountInToRelease_;

            // If full fill, update order status to completed
            if (fullFill_) {
                order.status = OrderStatus.Completed;
                emit OrderCompleted(orderId_);
            } else {
                // Set order status to created in case of uninitialized cross-chain order
                if (orderData_.originChainId != block.chainid && order.status == OrderStatus.DoesNotExist) {
                    order.status = OrderStatus.Created;
                }
            }
        }

        // Transfer tokens from the solver to the recipient
        IERC20(orderData_.tokenOut.toAddress()).safeTransferExactFrom(
            msg.sender,
            orderData_.recipient.toAddress(),
            uint256(amountOutToFill_)
        );

        // If local order, release the corresponding amount of origin tokens to the filler
        if (block.chainid == orderData_.originChainId) {
            // Validate msg.value is 0 for local fills
            if (msg.value != 0) revert InvalidMsgValue();

            // If this is a fill on the origin chain, we can immediately release the token in to the filler
            // This is because the origin and destination chains are the same, so no cross-chain messaging is needed
            IERC20(order.tokenIn).safeTransferExact(
                fillerParams_.originRecipient.toAddress(),
                uint256(amountInToRelease_)
            );
        } else {
            // If this is a fill on a different chain than the origin chain,
            // we need to send a message back to the origin chain to release
            // the corresponding amount of tokenIn to the solver's recipient
            FillReport memory report_ = FillReport({
                orderId: orderId_,
                originRecipient: fillerParams_.originRecipient,
                amountOutFilled: amountOutToFill_,
                amountInToRelease: amountInToRelease_,
                tokenIn: orderData_.tokenIn
            });

            // Send fill report to the origin chain and pass along msg.value
            // to the portal for crosschain message fee
            bytes32 refundAddress = fillerParams_.refundAddress == bytes32(0)
                ? msg.sender.toBytes32()
                : fillerParams_.refundAddress;
            messageId_ = bridgeAdapter_ == address(0)
                ? IPortalV2Like(portal).sendFillReport{ value: msg.value }(
                    orderData_.originChainId, // destinationChainId (of this message)
                    report_,
                    refundAddress,
                    bridgeAdapterArgs_
                )
                : IPortalV2Like(portal).sendFillReport{ value: msg.value }(
                    orderData_.originChainId, // destinationChainId (of this message)
                    report_,
                    refundAddress,
                    bridgeAdapter_,
                    bridgeAdapterArgs_
                );
        }

        emit OrderFilled(orderId_, msg.sender, amountInToRelease_, amountOutToFill_, messageId_);
    }

    /* ========== Receiving Crosschain Reports ========== */

    /// @inheritdoc IOrderBook
    function reportFill(uint32 sourceChainId_, FillReport calldata report_) external override {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        Order storage order = $.orders[report_.orderId];

        // Validate the fill report and sender
        if (msg.sender != portal) revert NotAuthorized();
        // We allow reporting fills for both Created and Cancelled orders
        // The latter allows for fills that were in-flight at the time of cancellation
        // that may have arrived after the cancel report was processed due to the fact that
        // crosschain messages do not have to processed in the order they were sent.
        if (!(order.status == OrderStatus.Created || order.status == OrderStatus.Cancelled))
            revert InvalidOrderStatus();
        if (report_.tokenIn != order.tokenIn.toBytes32()) revert InvalidReport();
        if (sourceChainId_ != order.destChainId) revert InvalidReportSource();

        // Calculate the fill to determine if it completely fills the order
        (bool fullFill, , ) = _calculateFill(
            order.amountIn,
            order.amountOut,
            $.filledAmounts[report_.orderId].amountInReleased,
            $.filledAmounts[report_.orderId].amountOutFilled,
            report_.amountOutFilled
        );

        // Update the fill amounts for the order
        IOrderBook.FilledAmounts storage filledAmounts = $.filledAmounts[report_.orderId];
        filledAmounts.amountOutFilled += report_.amountOutFilled;
        filledAmounts.amountInReleased += report_.amountInToRelease;

        // Validate that the filled amounts do not exceed the order amounts
        // For tokenIn amounts, this includes both released and refunded amounts since
        // both reduce the amount available to be filled. Refunded amounts may have been
        // paid out previously via a cancel report.
        if (
            filledAmounts.amountOutFilled > order.amountOut ||
            filledAmounts.amountInReleased + filledAmounts.amountInRefunded > order.amountIn
        ) revert InvalidReport();

        // Mark order as completed if fully filled
        if (fullFill) {
            order.status = OrderStatus.Completed;
            emit OrderCompleted(report_.orderId);
        }

        // Transfer the amount in to release to the recipient specified by the filler
        // We do not check fee on transfer here to avoid potential reverts on reported fills
        IERC20(order.tokenIn).safeTransfer(report_.originRecipient.toAddress(), uint256(report_.amountInToRelease));

        emit FillReported(
            report_.orderId,
            report_.originRecipient.toAddress(),
            report_.amountInToRelease,
            report_.amountOutFilled
        );
    }

    /// @inheritdoc IOrderBook
    function reportCancel(uint32 sourceChainId_, CancelReport calldata report_) external override {
        Order storage order = _getOrderBookStorageLocation().orders[report_.orderId];

        // Validate the cancel report and sender
        if (msg.sender != portal) revert NotAuthorized();
        if (order.status != OrderStatus.Created) revert InvalidOrderStatus();
        if (sourceChainId_ != order.destChainId) revert InvalidReportSource();

        // Validate the reported order sender and token in match
        // This isn't strictly required because we use local data,
        // but invalid reports should not be sent so we prevent this
        if (order.tokenIn != report_.tokenIn.toAddress() || order.sender != report_.orderSender.toAddress())
            revert InvalidReport();

        // Update order status and refunded amount
        order.status = OrderStatus.Cancelled;
        FilledAmounts storage filledAmounts = _getOrderBookStorageLocation().filledAmounts[report_.orderId];
        filledAmounts.amountInRefunded += report_.amountInToRefund;

        // Validate that the refunded amount does not cause over-refunding
        if (filledAmounts.amountInRefunded + filledAmounts.amountInReleased > order.amountIn) revert InvalidReport();

        // Transfer the refund amount to the original order sender
        IERC20(order.tokenIn).safeTransfer(order.sender, uint256(report_.amountInToRefund));

        emit CancelReported(report_.orderId);
        emit RefundClaimed(report_.orderId, order.sender, report_.amountInToRefund);
    }

    /* ========== Admin Functions ========== */

    /// @inheritdoc IOrderBook
    function setDestinationSupported(
        uint32 destChainId_,
        bool isSupported_
    ) external override onlyRole(DEFAULT_ADMIN_ROLE) {
        if (destChainId_ == block.chainid) revert InvalidDestinationChain();

        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        bool isSupported = $.supportedDestinations[destChainId_];

        // Don't update if the value is the same
        if (isSupported == isSupported_) return;

        $.supportedDestinations[destChainId_] = isSupported_;

        emit DestinationSupportUpdated(destChainId_, isSupported_);
    }

    /// @inheritdoc IOrderBook
    function pause() external onlyRole(PAUSER_ROLE) {
        _pause();
    }

    /// @inheritdoc IOrderBook
    function unpause() external onlyRole(PAUSER_ROLE) {
        _unpause();
    }

    /* ========== View Functions ========== */

    /// @inheritdoc IOrderBook
    function getOrderId(OrderData memory orderData_) public pure override returns (bytes32) {
        return
            keccak256(
                abi.encodePacked(
                    orderData_.version,
                    orderData_.sender,
                    orderData_.nonce,
                    orderData_.originChainId,
                    orderData_.destChainId,
                    orderData_.createdAt,
                    orderData_.fillDeadline,
                    orderData_.tokenIn,
                    orderData_.tokenOut,
                    orderData_.amountIn,
                    orderData_.amountOut,
                    orderData_.recipient,
                    orderData_.solver
                )
            );
    }

    /// @inheritdoc IOrderBook
    function getOrder(bytes32 orderId_) external view override returns (Order memory) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.orders[orderId_];
    }

    /// @inheritdoc IOrderBook
    function getFilledAmounts(bytes32 orderId_) external view override returns (FilledAmounts memory) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.filledAmounts[orderId_];
    }

    /// @inheritdoc IOrderBook
    function getSenderNonce(address sender_) external view override returns (uint64) {
        OrderBookStorageStruct storage $ = _getOrderBookStorageLocation();
        return $.senderNonces[sender_];
    }

    /// @inheritdoc IOrderBook
    function isDestinationSupported(uint32 destChainId_) public view override returns (bool) {
        return destChainId_ == block.chainid || _getOrderBookStorageLocation().supportedDestinations[destChainId_];
    }

    /* ========== EIP-712 Digest Functions ========== */

    /// @inheritdoc IOrderBook
    function getGaslessOrderDigest(GaslessOrderParams memory orderParams_) public view override returns (bytes32) {
        return
            _getDigest(
                keccak256(
                    abi.encode(
                        GASLESS_ORDER_TYPEHASH,
                        orderParams_.version,
                        orderParams_.sender,
                        orderParams_.nonce,
                        orderParams_.originChainId,
                        orderParams_.destChainId,
                        orderParams_.fillDeadline,
                        orderParams_.tokenIn,
                        orderParams_.tokenOut,
                        orderParams_.amountIn,
                        orderParams_.amountOut,
                        orderParams_.recipient,
                        orderParams_.solver
                    )
                )
            );
    }

    /// @inheritdoc IOrderBook
    function getCancelOrderDigest(
        bytes32 orderId_,
        address bridgeAdapter_,
        bytes memory bridgeAdapterArgs_
    ) public view override returns (bytes32) {
        return
            _getDigest(
                keccak256(abi.encode(CANCEL_ORDER_TYPEHASH, orderId_, bridgeAdapter_, keccak256(bridgeAdapterArgs_)))
            );
    }

    /* ========== Internal Helper Functions ========== */

    function _revertIfInvalidStatusToFillOrCancel(Order storage order_, OrderData memory orderData_) internal view {
        // Check order status
        // If local order, status must be Created
        // If cross-chain order, status must be DoesNotExist (if not filled at all yet) or Created (if already partially filled)
        if (
            !(
                orderData_.originChainId == block.chainid
                    ? order_.status == OrderStatus.Created
                    : (order_.status == OrderStatus.Created || order_.status == OrderStatus.DoesNotExist)
            )
        ) revert InvalidOrderStatus();
    }

    function _revertIfOrderIdMismatch(bytes32 orderId_, OrderData memory orderData_) internal pure {
        // Ensures that the specified orderId, used to retrieve the order data from storage,
        // is bound to the orderData provided as input. This prevents the caller from specifying
        // inconsistent (malicious) orderData compared to the content of the order fetched from storage.
        if (orderId_ != getOrderId(orderData_)) revert OrderIdMismatch();
    }

    /// @notice Calculates the fill amounts for an order fill from the provided state and amount out to fill
    /// @param totalAmountIn_ The total amount of token in for the order
    /// @param totalAmountOut_ The total amount of token out for the order
    /// @param amountInReleased_ The amount of token in already released for the order
    /// @param amountOutFilled_ The amount of token out already filled for the order
    /// @param amountOutToFill_ The amount of token out the filler wants to fill
    /// @return fullFill_ Whether the fill is a full fill
    /// @return amountInToRelease_ The amount of token in to release to the filler
    /// @return amountOutToFill_ The amount of token out to fill, this is the minimum of the provided amount and the remaining unfilled amount
    function _calculateFill(
        uint128 totalAmountIn_,
        uint128 totalAmountOut_,
        uint128 amountInReleased_,
        uint128 amountOutFilled_,
        uint128 amountOutToFill_
    ) internal pure returns (bool, uint128, uint128) {
        // Determine the amount out to fill as the minimum of the filler provided amount and the remaining unfilled amount
        uint128 amountOutRemaining_ = totalAmountOut_ - amountOutFilled_; // can't underflow bc amountOutFilled_ <= totalAmountOut_
        bool fullFill_ = amountOutToFill_ >= amountOutRemaining_;
        amountOutToFill_ = fullFill_ ? amountOutRemaining_ : amountOutToFill_;

        // Calculate the corresponding amount of token in to release to the filler
        uint128 amountInToRelease_ = fullFill_
            ? totalAmountIn_ - amountInReleased_ // remaining amount
            : ((uint256(totalAmountIn_) * amountOutToFill_) / totalAmountOut_).toUint128();

        return (fullFill_, amountInToRelease_, amountOutToFill_);
    }
}
