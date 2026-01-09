use anchor_lang::prelude::*;

// Same as actual portal program ID for testing purposes
declare_id!("MzBrgc8yXBj4P16GTkcSyDZkEQZB9qDqf3fh9bByJce");

// This is a placeholder to provide an interface to build against
#[program]
pub mod mock_portal {
    use super::*;

    pub fn send_fill_report(
        _ctx: Context<SendReport>,
        order_id: [u8; 32],
        token_in: [u8; 32],
        amount_in_to_release: u128,
        amount_out_filled: u128,
        origin_recipient: [u8; 32],
        origin_chain_id: u32,
    ) -> Result<()> {
        msg!(
            "Sending order filled message to chain {}: {:?}",
            origin_chain_id,
            (order_id, token_in, amount_in_to_release, amount_out_filled, origin_recipient)
        );
        Ok(())
    }

    pub fn send_cancel_report(
        _ctx: Context<SendReport>,
        order_id: [u8; 32],
        order_sender: [u8; 32],
        token_in: [u8; 32],
        amount_in_to_refund: u128,
        origin_chain_id: u32,
    ) -> Result<()> {
        msg!(
            "Sending order cancel message to chain {}: {:?}",
            origin_chain_id,
            (order_id, order_sender, token_in, amount_in_to_refund)
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SendReport<'info> {
    /// CHECK: any account can pay for the message
    #[account(mut)]
    pub sender: Signer<'info>,

    #[account(mut)]
    pub order_book_global: Signer<'info>,

    /// CHECK: we don't validate this in the mock
    #[account(mut)]
    pub portal_global: UncheckedAccount<'info>,

    /// CHECK: we don't validate this in the mock
    pub portal_authority: UncheckedAccount<'info>,

    /// CHECK: we don't validate this in the mock
    pub bridge_adapter: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>
}
