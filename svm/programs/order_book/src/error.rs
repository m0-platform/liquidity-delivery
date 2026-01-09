use anchor_lang::prelude::*;

#[error_code]
pub enum OrderBookError {
    #[msg("Invalid token out mint")]
    InvalidTokenOutMint,
    #[msg("Invalid order type")]
    InvalidOrderType,
    #[msg("Order is not fillable")]
    OrderNotFillable,
    #[msg("Math overflow")]
    MathOverflow,
    #[msg("Math underflow")]
    MathUnderflow,
    #[msg("Invalid fill amount")]
    InvalidFillAmount,
    #[msg("Invalid order ID")]
    InvalidOrderId,
    #[msg("Order has expired")]
    OrderExpired,
    #[msg("Invalid order version")]
    InvalidOrderVersion,
    #[msg("Invalid destination chain ID")]
    InvalidDestChainId,
    #[msg("Invalid solver")]
    InvalidSolver,
    #[msg("Invalid amount in")]
    InvalidAmountIn,
    #[msg("Invalid amount out")]
    InvalidAmountOut,
    #[msg("Invalid fill deadline")]
    InvalidFillDeadline,
    #[msg("Invalid finality buffer")]
    InvalidFinalityBuffer,
    #[msg("Not authorized")]
    NotAuthorized,
    #[msg("Invalid token mint")]
    InvalidTokenMint,
    #[msg("Invalid origin chain ID")]
    InvalidOriginChainId,
    #[msg("Report fill amount exceeds order amount")]
    Overfill,
    #[msg("Invalid recipient address")]
    InvalidRecipient,
    #[msg("Invalid order status")]
    InvalidOrderStatus,
    #[msg("Order is not expired yet")]
    OrderNotExpired,
    #[msg("Destination chain not supported")]
    DestinationNotSupported,
    #[msg("Recipient token account required - order token account has dust balance that must be swept")]
    DustRecipientRequired,
    #[msg("Order is already filled")]
    OrderFilled,
    #[msg("Order status is not finalized")]
    FinalityPending,
    #[msg("Invalid destination account")]
    InvalidDestinationAccount,
    #[msg("Order has not been created yet")]
    InvalidCreatedAtTimestamp,
    #[msg("Sender address does not match order sender")]
    InvalidSender,
    #[msg("Invalid report source chain ID")]
    InvalidReportSource,
    #[msg("Payer address does not match order payer")]
    InvalidPayer,
    #[msg("Token account has non-zero balance - cannot close")]
    TokenAccountNotEmpty,
    #[msg("Program is paused")]
    ProgramPaused,
    #[msg("Portal authority cannot be default pubkey")]
    InvalidPortalAuthority,
    #[msg("Reported refund amount is greater than available")]
    InvalidRefundAmount
}
