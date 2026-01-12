use alloy::sol;

sol! {
    #[sol(rpc)]
    interface IOrderBook {
        #[derive(Debug)]
        enum OrderStatus {
            DoesNotExist,
            Created,
            CancelRequested,
            Completed
        }

        #[derive(Debug)]
        struct Order {
            OrderStatus status;
            uint16 version;
            address sender;
            uint64 nonce;
            uint32 destChainId;
            uint32 fillDeadline;
            uint32 cancelRequestedAt;
            address tokenIn;
            bytes32 tokenOut;
            uint128 amountIn;
            uint128 amountOut;
            bytes32 recipient;
            bytes32 solver;
        }

        function getOrder(bytes32 orderId) external view returns (Order memory);

        #[derive(Debug)]
        event OrderOpened(
            bytes32 indexed orderId,
            address sender,
            address tokenIn,
            uint128 amountIn,
            uint32 indexed destChainId,
            bytes32 tokenOut,
            uint128 amountOut,
            bytes32 indexed solver
        );
    }
}
