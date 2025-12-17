use alloy::{primitives::Bytes, sol, sol_types::SolError};

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

        #[derive(Debug)]
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

        #[derive(Debug)]
        struct FillParams {
            uint128 amountOutToFill;
            bytes32 originRecipient;
        }

        #[derive(Debug)]
        struct FilledAmounts {
            uint128 amountInReleased;
            uint128 amountOutFilled;
        }

        function getOrder(bytes32 orderId) external view returns (Order memory);

        function fillOrder(
            bytes32 orderId,
            OrderData calldata orderData,
            FillParams calldata fillerParams
        ) external;

        function getFilledAmounts(bytes32 orderId_) external view override returns (FilledAmounts memory);

        error AmountInZero();
        error AmountOutZero();
        error FillAmountZero();
        error FinalityPending();
        error InvalidDeadline();
        error InvalidDestinationChain();
        error InvalidFinalityBuffer();
        error InvalidNonce();
        error InvalidOrderStatus();
        error InvalidOrderVersion();
        error InvalidOriginChain();
        error InvalidRecipient();
        error InvalidReport();
        error NotAuthorized();
        error OrderExpired();
        error OrderAlreadyFilled();
        error OrderIdMismatch();
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

pub fn decode_custom_err(revert_data: Option<Bytes>) -> Option<String> {
    let data = revert_data.as_ref()?;

    macro_rules! try_decode {
        ($($error:ident),* $(,)?) => {
            $(
                if IOrderBook::$error::abi_decode(data).is_ok() {
                    return Some(stringify!($error).to_string());
                }
            )*
        };
    }

    try_decode!(
        AmountInZero,
        AmountOutZero,
        FillAmountZero,
        FinalityPending,
        InvalidDeadline,
        InvalidDestinationChain,
        InvalidFinalityBuffer,
        InvalidNonce,
        InvalidOrderStatus,
        InvalidOrderVersion,
        InvalidOriginChain,
        InvalidRecipient,
        InvalidReport,
        NotAuthorized,
        OrderExpired,
        OrderAlreadyFilled,
        OrderIdMismatch,
    );

    None
}
