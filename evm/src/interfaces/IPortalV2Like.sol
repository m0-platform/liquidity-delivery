// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8;

import { IOrderBook } from "./IOrderBook.sol";

interface IPortalV2Like {
    /// @notice Sends the fill report to the destination chain using the default bridge adapter.
    /// @param  destinationChainId The ID of the destination chain.
    /// @param  report             The OrderBook fill report to send.
    /// @param  refundAddress      The address to receive excess native gas on the source chain.
    /// @param  bridgeAdapterArgs  The optional bridge adapter arguments, could be empty.
    /// @return messageId          The ID uniquely identifying the message.
    function sendFillReport(
        uint32 destinationChainId,
        IOrderBook.FillReport calldata report,
        bytes32 refundAddress,
        bytes calldata bridgeAdapterArgs
    ) external payable returns (bytes32 messageId);

    /// @notice Sends the fill report to the destination chain using the specified bridge adapter.
    /// @param  destinationChainId The ID of the destination chain.
    /// @param  report             The OrderBook fill report to send.
    /// @param  refundAddress      The address to receive excess native gas on the source chain.
    /// @param  bridgeAdapter      The address of the bridge adapter to use.
    /// @param  bridgeAdapterArgs  The optional bridge adapter arguments, could be empty.
    /// @return messageId          The ID uniquely identifying the message.
    function sendFillReport(
        uint32 destinationChainId,
        IOrderBook.FillReport calldata report,
        bytes32 refundAddress,
        address bridgeAdapter,
        bytes calldata bridgeAdapterArgs
    ) external payable returns (bytes32 messageId);

    /// @notice Sends the cancel report to the destination chain using the default bridge adapter.
    /// @param  destinationChainId The ID of the destination chain.
    /// @param  report             The OrderBook cancel report to send.
    /// @param  refundAddress      The address to receive excess native gas on the source chain
    /// @param  bridgeAdapterArgs  The optional bridge adapter arguments, could be empty.
    /// @return messageId          The ID uniquely identifying the message.
    function sendCancelReport(
        uint32 destinationChainId,
        IOrderBook.CancelReport calldata report,
        bytes32 refundAddress,
        bytes calldata bridgeAdapterArgs
    ) external payable returns (bytes32 messageId);

    /// @notice Sends the cancel report to the destination chain using the specified bridge adapter.
    /// @param  destinationChainId The ID of the destination chain.
    /// @param  report             The OrderBook cancel report to send.
    /// @param  refundAddress      The address to receive excess native gas on the source chain
    /// @param  bridgeAdapter      The address of the bridge adapter to use.
    /// @param  bridgeAdapterArgs  The optional bridge adapter arguments, could be empty.
    /// @return messageId          The ID uniquely identifying the message.
    function sendCancelReport(
        uint32 destinationChainId,
        IOrderBook.CancelReport calldata report,
        bytes32 refundAddress,
        address bridgeAdapter,
        bytes calldata bridgeAdapterArgs
    ) external payable returns (bytes32 messageId);
}
