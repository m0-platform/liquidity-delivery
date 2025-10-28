use alloy::sol_types::sol;

// Define Solidity events
sol! {
    #![sol(rpc, alloy_sol_types = alloy::sol_types)]

    #[derive(Debug)]
    event OrderOpen(
        bytes32 indexed orderId,
        address tokenIn,
        uint128 amountIn,
        uint32 indexed destChainId,
        bytes32 indexed tokenOut,
        uint128 amountOut,
        bytes32 solver
    );

    #[derive(Debug)]
    event Fill(
        bytes32 indexed orderId,
        address indexed solver,
        uint128 amountOutFilled
    );

    #[derive(Debug)]
    event CancelRequest(
        bytes32 indexed orderId,
        uint40 newFillDeadline
    );

    #[derive(Debug)]
    event RefundClaimed(
        bytes32 indexed orderId,
        address indexed sender,
        uint128 amountInRefunded
    );

    #[derive(Debug)]
    event OrderCompleted(
        bytes32 indexed orderId
    );
}
