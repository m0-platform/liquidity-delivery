use alloy::sol_types::sol;

// Define Solidity events
sol! {
    #![sol(rpc, alloy_sol_types = alloy::sol_types)]

    #[derive(Debug)]
    event OrderOpened(
        bytes32 indexed orderId,
        address tokenIn,
        uint128 amountIn,
        uint32 indexed destChainId,
        bytes32 tokenOut,
        uint128 amountOut,
        bytes32 indexed solver
    );

    #[derive(Debug)]
    event OrderFilled(
        bytes32 indexed orderId,
        address indexed solver,
        uint128 amountInToRelease,
        uint128 amountOutFilled
    );

    #[derive(Debug)]
    event CancelRequested(bytes32 indexed orderId, uint32 cancelRequestedAt);

    #[derive(Debug)]
    event RefundClaimed(
        bytes32 indexed orderId,
        address indexed sender,
        uint128 amountInRefunded
    );


    #[derive(Debug)]
    event OrderCompleted(bytes32 orderId);
}
