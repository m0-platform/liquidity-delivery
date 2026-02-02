use alloy::sol;

sol! {
    #[sol(rpc)]
    interface IOrderBook {
        #[derive(Debug)]
        enum OrderStatus {
            DoesNotExist,
            Created,
            Cancelled,
            Completed
        }

        #[derive(Debug)]
        struct Order {
            OrderStatus status;
            uint16 version;
            address sender;
            uint64 nonce;
            uint32 destChainId;
            uint32 createdAt;
            uint32 fillDeadline;
            address tokenIn;
            bytes32 tokenOut;
            uint128 amountIn;
            uint128 amountOut;
            bytes32 recipient;
            bytes32 solver;
        }

        #[derive(Debug)]
        struct OrderData {
            uint16 version;
            bytes32 sender;
            uint64 nonce;
            uint32 originChainId;
            uint32 destChainId;
            uint64 createdAt;
            uint64 fillDeadline;
            bytes32 tokenIn;
            bytes32 tokenOut;
            uint128 amountIn;
            uint128 amountOut;
            bytes32 recipient;
            bytes32 solver;
        }

        #[derive(Debug)]
        struct FillParams {
            uint128 amountOutToFill;
            bytes32 originRecipient;
            bytes32 refundAddress;
        }

        #[derive(Debug)]
        struct FilledAmounts {
            uint128 amountInRefunded;
            uint128 amountInReleased;
            uint128 amountOutFilled;
        }

        function getOrder(bytes32 orderId) external view returns (Order memory);

        function fillOrder(
            bytes32 orderId,
            OrderData calldata orderData,
            FillParams calldata fillerParams,
            address bridgeAdapter,
            bytes calldata bridgeAdapterArgs
        ) external payable;

        function getFilledAmounts(bytes32 orderId_) external view returns (FilledAmounts memory);

        error AmountInZero();
        error AmountOutZero();
        error FillAmountZero();
        error InvalidDeadline();
        error InvalidDestinationChain();
        error InvalidMsgValue();
        error InvalidNonce();
        error InvalidOrderStatus();
        error InvalidOrderVersion();
        error InvalidOriginChain();
        error InvalidRecipient();
        error InvalidSolver();
        error InvalidReport();
        error InvalidTimestamp();
        error InvalidReportSource();
        error NotAuthorized();
        error OrderExpired();
        error OrderAlreadyExists();
        error OrderIdMismatch();
        error SameTokenOrder();
        error ZeroAdmin();
        error ZeroPauser();
        error ZeroPortal();
    }
}

sol! {
    #[sol(rpc)]
    interface IERC20 {
        function allowance(address owner, address spender) external view returns (uint256);
        function approve(address spender, uint256 amount) external returns (bool);
        function balanceOf(address account) external view returns (uint256);
        function decimals() external view returns (uint8);
    }
}

sol! {
    #[sol(rpc)]
    interface IPortal {
        #[derive(Debug)]
        enum PayloadType {
            TokenTransfer,
            Index,
            RegistrarKey,
            RegistrarList,
            FillReport
        }

        function quote(uint32 destinationChainId, PayloadType payloadType) external view returns (uint256);
    }
}
