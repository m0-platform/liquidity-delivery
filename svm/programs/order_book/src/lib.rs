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