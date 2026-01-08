// SPDX-License-Identifier: UNLICENSED
pragma solidity >=0.8.33;

import { IPortalV2Like, IOrderBook } from "../../src/interfaces/IPortalV2Like.sol";

contract MockPortalV2 is IPortalV2Like {
    event FillReportSent(uint32 destinationChainId, IOrderBook.FillReport report);
    event CancelReportSent(uint32 destinationChainId, IOrderBook.CancelReport report);

    address public orderBook;

    mapping(bytes32 => IOrderBook.FillReport) public fillReports;
    mapping(bytes32 => bool) public cancelReports;

    function setOrderBook(address orderBook_) external {
        orderBook = orderBook_;
    }

    function sendFillReport(
        uint32 destinationChainId,
        IOrderBook.FillReport calldata report,
        bytes32 refundAddress,
        bytes calldata message
    ) external payable override returns (bytes32 messageId) {
        fillReports[report.orderId] = report;
        emit FillReportSent(destinationChainId, report);
    }

    function sendFillReport(
        uint32 destinationChainId,
        IOrderBook.FillReport calldata report,
        bytes32 refundAddress,
        address bridgeAdapter,
        bytes calldata bridgeAdapterArgs
    ) external payable override returns (bytes32 messageId) {
        fillReports[report.orderId] = report;
        emit FillReportSent(destinationChainId, report);
    }

    function sendCancelReport(
        uint32 destinationChainId,
        IOrderBook.CancelReport calldata report,
        bytes32 refundAddress,
        bytes calldata bridgeAdapterArgs
    ) external payable override returns (bytes32 messageId) {
        cancelReports[report.orderId] = true;
        emit CancelReportSent(destinationChainId, report);
    }

    function sendCancelReport(
        uint32 destinationChainId,
        IOrderBook.CancelReport calldata report,
        bytes32 refundAddress,
        address bridgeAdapter,
        bytes calldata bridgeAdapterArgs
    ) external payable override returns (bytes32 messageId) {
        cancelReports[report.orderId] = true;
        emit CancelReportSent(destinationChainId, report);
    }

    function receiveFillReport(uint32 sourceChainId, IOrderBook.FillReport calldata report) external {
        IOrderBook(orderBook).reportFill(sourceChainId, report);
    }

    function receiveCancelReport(uint32 sourceChainId, IOrderBook.CancelReport calldata report) external {
        IOrderBook(orderBook).reportCancel(sourceChainId, report);
    }

    function isFillReported(bytes32 orderId) external view returns (bool) {
        return fillReports[orderId].amountOutFilled != 0;
    }

    function isCancelReported(bytes32 orderId) external view returns (bool) {
        return cancelReports[orderId];
    }
}
