use anchor_lang::prelude::*;
use crate::{
    error::OrderBookError,
    state::{
        Order, NativeOrder, OrderStatus, ORDER_SEED_PREFIX,
    }
};

#[event_cpi]
#[derive(Accounts)]
#[instruction(order_id: [u8; 32])]
pub struct RequestCancelOrder<'info> {
    pub sender: Signer<'info>,

    #[account(
        mut,
        seeds = [ORDER_SEED_PREFIX, order_id.as_ref()],
        bump = order.bump,
        constraint = order.data.sender == sender.key() @ OrderBookError::NotAuthorized,
    )]
    pub order: Account<'info, Order::<NativeOrder>>,
}

impl RequestCancelOrder<'_> {
    fn validate(&self) -> Result<()> {
        let order = &self.order.data;

        // Validate the order has a valid status for cancellation
        if order.status != OrderStatus::Created {
            return err!(OrderBookError::InvalidOrderStatus);
        }

        // Validate the fill deadline is in the future, otherwise, there is no need to cancel
        let current_timestamp = Clock::get()?.unix_timestamp as u64;
        if order.fill_deadline <= current_timestamp {
            return err!(OrderBookError::OrderExpired);
        }

        Ok(())
    }

    #[access_control(ctx.accounts.validate())]
    pub fn handler(ctx: Context<Self>, order_id: [u8; 32]) -> Result<()> {
        // Set the order status to CancelRequested and the fill deadline to the current timestamp
        let order = &mut ctx.accounts.order.data;
        order.status = OrderStatus::CancelRequested;

        let current_timestamp = Clock::get()?.unix_timestamp as u64;
        order.fill_deadline = current_timestamp;

        // Emit cancel requested event to notify solvers to not fill the order any longer
        emit_cpi!(CancelRequest {
            order_id
        });

        Ok(())
    }

}

#[event]
pub struct CancelRequest {
    pub order_id: [u8; 32],
}
