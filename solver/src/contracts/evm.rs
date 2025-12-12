use alloy::sol;

sol! {
    #[sol(rpc)]
    interface IOrderBook {
        struct OrderData {
            uint16 version;
            bytes32 sender;
            uint64 nonce;
            uint32 originChainId;
            uint32 destChainId;
            uint64 fillDeadline;
            bytes32 tokenIn;
            bytes32 tokenOut;
            uint128 amountIn;
            uint128 amountOut;
            bytes32 recipient;
            bytes32 solver;
        }

        struct FillParams {
            uint128 amountOutToFill;
            bytes32 originRecipient;
        }

        function fillOrder(
            bytes32 orderId,
            OrderData calldata orderData,
            FillParams calldata fillerParams
        ) external;
    }
}
