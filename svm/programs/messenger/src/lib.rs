use anchor_lang::prelude::*;

declare_id!("8KytuBaFrs7wv2oprMgrkscrogPVi1baqTewfgUZoaHr");

declare_program!(order_book);

#[program]
pub mod messenger {
    use super::*;

    pub fn send_fill_report(_ctx: Context<SendFillReport>, origin_chain_id: u32, fill_report: FillReport) -> Result<()> {
        // TODO this is a placeholder to provide an interface to build against
        msg!("Sending order filled message to chain {}: {:?}", origin_chain_id, fill_report);
        Ok(())
    }

    /// Report a fill to the order book program via CPI
    /// This is a test helper that allows the messenger authority PDA to sign
    pub fn report_fill(ctx: Context<ReportFill>, fill_report: FillReport) -> Result<()> {
        // Get the bump for the authority PDA
        let authority_bump = ctx.bumps.authority;
        let authority_seeds: &[&[&[u8]]] = &[&[AUTHORITY_SEED, &[authority_bump]]];

        // Build the CPI context with the authority PDA as signer
        let cpi_accounts = order_book::cpi::accounts::ReportOrderFill {
            program: ctx.accounts.order_book_program.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            relayer: ctx.accounts.relayer.to_account_info(),
            messenger_authority: ctx.accounts.authority.to_account_info(),
            global_account: ctx.accounts.global_account.to_account_info(),
            order: ctx.accounts.order.to_account_info(),
            token_in_mint: ctx.accounts.token_in_mint.to_account_info(),
            origin_recipient: ctx.accounts.origin_recipient.to_account_info(),
            recipient_token_in_ata: ctx.accounts.recipient_token_in_ata.to_account_info(),
            order_token_in_ata: ctx.accounts.order_token_in_ata.to_account_info(),
            token_in_program: ctx.accounts.token_in_program.to_account_info(),
            associated_token_program: ctx.accounts.associated_token_program.to_account_info(),
            system_program: ctx.accounts.system_program.to_account_info(),
        };

        let cpi_ctx = CpiContext::new_with_signer(
            ctx.accounts.order_book_program.to_account_info(),
            cpi_accounts,
            authority_seeds,
        );

        order_book::cpi::report_order_fill(cpi_ctx, fill_report)
    }
}

#[derive(Accounts)]
pub struct SendFillReport<'info> {
    // TODO restrict to order book global account
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct ReportFill<'info> {
    #[account(mut)]
    pub relayer: Signer<'info>,

    /// The messenger authority PDA that will sign the CPI call
    #[account(
        seeds = [AUTHORITY_SEED],
        bump
    )]
    /// CHECK: This is the PDA authority
    pub authority: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    pub event_authority: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    #[account(mut)]
    pub global_account: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    #[account(mut)]
    pub order: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    pub token_in_mint: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    pub origin_recipient: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    #[account(mut)]
    pub recipient_token_in_ata: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    #[account(mut)]
    pub order_token_in_ata: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    pub token_in_program: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    pub associated_token_program: UncheckedAccount<'info>,

    /// CHECK: Passed through to order_book CPI
    pub system_program: UncheckedAccount<'info>,

    /// The order book program
    pub order_book_program: Program<'info, order_book::program::OrderBook>,
}

#[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FillReport {
    pub order_id: [u8; 32],
    pub amount_in_to_release: u128,
    pub amount_out_filled: u128,
    pub origin_recipient: [u8; 32],
}

#[constant]
pub const AUTHORITY_SEED: &[u8] = b"authority";