// SPDX-License-Identifier: UNLICENSED
pragma solidity >=0.8.26;

import { IMessenger, IOrderBook } from "../../src/interfaces/IMessenger.sol";

contract MockMessenger is IMessenger {
    event FillReportSent(uint32 destinationChainId, IOrderBook.FillReport report);

    address public orderBook;

    mapping(bytes32 => IOrderBook.FillReport) public fillReports;

    function setOrderBook(address orderBook_) external {
        orderBook = orderBook_;
    }

    function sendFillReport(
        uint32 destinationChainId,
        IOrderBook.FillReport calldata report,
        bytes calldata messageData
    ) external override {
        fillReports[report.orderId] = report;
        emit FillReportSent(destinationChainId, report);
    }

    function receiveFillReport(uint32 sourceChainId, IOrderBook.FillReport calldata report) external {
        IOrderBook(orderBook).reportFill(sourceChainId, report);
    }

    function isFillReported(bytes32 orderId) external view returns (bool) {
        return fillReports[orderId].amountOutFilled != 0;
    }
}
