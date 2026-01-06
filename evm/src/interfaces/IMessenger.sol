// SPDX-License-Identifier: GPL-3.0
pragma solidity ^0.8;

import { IOrderBook } from "./IOrderBook.sol";

interface IMessenger {
    function sendFillReport(
        uint32 destinationChainId,
        IOrderBook.FillReport calldata report,
        bytes calldata messageData
    ) external;

    function sendCancelReport(
        uint32 destinationChainId,
        IOrderBook.CancelReport calldata report,
        bytes calldata messageData
    ) external;
}
