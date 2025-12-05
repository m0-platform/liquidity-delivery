pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

declare_id!("4Qgxc6VkBGaAQAikirnkApYNyy1W6asQgMHZxKgRcSL8");

declare_program!(portal);

#[program]
pub mod order_book {
    use super::*;

    // Admin actions

    pub fn initialize(
        ctx: Context<Initialize>,
        chain_id: u32,
        messenger_authority: Pubkey,
    ) -> Result<()> {
        Initialize::handler(ctx, chain_id, messenger_authority)
    }

    pub fn configure_destination(
        ctx: Context<ConfigureDestination>,
        dest_chain_id: u32,
        is_supported: bool,
        finality_buffer: Option<u64>,
    ) -> Result<()> {
        ConfigureDestination::handler(ctx, dest_chain_id, is_supported, finality_buffer)
    }

    pub fn set_messenger_authority(
        ctx: Context<AdminInstruction>,
        messenger_authority: Pubkey,
    ) -> Result<()> {
        AdminInstruction::set_messenger_authority(ctx, messenger_authority)
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

    // User actions

    pub fn open_order(ctx: Context<OpenOrder>, params: OrderParams) -> Result<()> {
        OpenOrder::handler(ctx, params)
    }

    pub fn request_cancel_order(
        ctx: Context<RequestCancelOrder>,
        order_id: [u8; 32],
    ) -> Result<()> {
        RequestCancelOrder::handler(ctx, order_id)
    }

    pub fn claim_refund(ctx: Context<ClaimRefund>, order_id: [u8; 32]) -> Result<()> {
        ClaimRefund::handler(ctx, order_id)
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

    // Crosschain messaging actions

    pub fn report_order_fill(ctx: Context<ReportOrderFill>, fill_report: FillReport) -> Result<()> {
        ReportOrderFill::handler(ctx, fill_report)
    }
}
