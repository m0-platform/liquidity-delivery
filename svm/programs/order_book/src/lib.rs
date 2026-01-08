pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_program!(portal);

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "M0 OrderBook Program",
    project_url: "https://m0.org/",
    contacts: "email:security@m0.xyz",
    policy: "https://github.com/m0-foundation/liquidity-delivery/blob/main/SECURITY.md", // TODO
    preferred_languages: "en",
    source_code: "https://github.com/m0-foundation/liquidity-delivery/tree/main/programs/order_book",
    auditors: "" // TODO
}

declare_id!("MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK");

#[program]
pub mod order_book {
    use super::*;

    // Admin actions

    pub fn initialize(
        ctx: Context<Initialize>,
        chain_id: u32,
        portal_authority: Pubkey,
    ) -> Result<()> {
        Initialize::handler(ctx, chain_id, portal_authority)
    }

    pub fn add_destination(
        ctx: Context<AddDestination>,
        dest_chain_id: u32,
    ) -> Result<()> {
        AddDestination::handler(ctx, dest_chain_id)
    }

    pub fn remove_destination(
        ctx: Context<RemoveDestination>,
        dest_chain_id: u32,
    ) -> Result<()> {
        RemoveDestination::handler(ctx, dest_chain_id)
    }

    pub fn set_portal_authority(
        ctx: Context<AdminInstruction>,
        portal_authority: Pubkey,
    ) -> Result<()> {
        AdminInstruction::set_portal_authority(ctx, portal_authority)
    }

    pub fn set_new_admin(
        ctx: Context<AdminInstruction>,
        new_admin: Pubkey,
    ) -> Result<()> {
        AdminInstruction::set_new_admin(ctx, new_admin)
    }

    pub fn clear_new_admin(
        ctx: Context<AdminInstruction>,
    ) -> Result<()> {
        AdminInstruction::clear_new_admin(ctx)
    }

    pub fn accept_admin_role(
        ctx: Context<AcceptAdminRole>,
    ) -> Result<()> {
        AcceptAdminRole::handler(ctx)
    }

    pub fn pause(
        ctx: Context<AdminInstruction>,
    ) -> Result<()> {
        AdminInstruction::pause(ctx)
    }

    pub fn unpause(
        ctx: Context<AdminInstruction>,
    ) -> Result<()> {
        AdminInstruction::unpause(ctx)
    }

    // User actions

    pub fn open_order(ctx: Context<OpenOrder>, params: OrderParams) -> Result<()> {
        OpenOrder::handler(ctx, params)
    }

    pub fn cancel_native_order(
        ctx: Context<CancelNativeOrder>,
        order_id: [u8; 32],
    ) -> Result<()> {
        CancelNativeOrder::handler(ctx, order_id)
    }

    pub fn cancel_foreign_order<'info>(
        ctx: Context<'_, '_, 'info, 'info, CancelForeignOrder<'info>>,
        order_id: [u8; 32],
        order_data: OrderData,
    ) -> Result<()> {
        CancelForeignOrder::handler(ctx, order_id, order_data)
    }

    // Solver actions

    pub fn fill_native_order(
        ctx: Context<FillNativeOrder>,
        order_id: [u8; 32],
        order_data: OrderData,
        fill_params: FillParams,
    ) -> Result<()> {
        FillNativeOrder::handler(ctx, order_id, order_data, fill_params)
    }

    pub fn fill_foreign_order<'info>(
        ctx: Context<'_, '_, 'info, 'info, FillForeignOrder<'info>>,
        order_id: [u8; 32],
        order_data: OrderData,
        fill_params: FillParams,
    ) -> Result<()> {
        FillForeignOrder::handler(ctx, order_id, order_data, fill_params)
    }

    pub fn close_order_token_account(
        ctx: Context<CloseOrderTokenAccount>,
        order_id: [u8; 32],
    ) -> Result<()> {
        CloseOrderTokenAccount::handler(ctx, order_id)
    }

    // Crosschain messaging actions

    pub fn report_order_fill(ctx: Context<ReportOrderFill>, source_chain_id: u32, fill_report: FillReport) -> Result<()> {
        ReportOrderFill::handler(ctx, source_chain_id, fill_report)
    }

    pub fn report_order_cancel(ctx: Context<ReportOrderCancel>, source_chain_id: u32, cancel_report: CancelReport) -> Result<()> {
        ReportOrderCancel::handler(ctx, source_chain_id, cancel_report)
    } 


    // Dummy IDL instruction
    // Included to ensure the order types are included in the IDL build
    #[cfg(feature = "idl-build")]
    pub fn idl_instruction(_ctx: Context<Dummy>, foreign: ForeignOrder, native: NativeOrder) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "idl-build")]
#[derive(Accounts)]
pub struct Dummy {}