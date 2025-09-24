pub mod constants;
pub mod error;
pub mod instructions;
pub mod state;
pub mod utils;

use anchor_lang::prelude::*;

pub use constants::*;
pub use instructions::*;
pub use state::*;

use messenger::FillReport;

declare_id!("4Qgxc6VkBGaAQAikirnkApYNyy1W6asQgMHZxKgRcSL8");

#[program]
pub mod order_book {
    use super::*;

    // User actions

    pub fn open_order(ctx: Context<OpenOrder>, params: OrderParams) -> Result<()> {
        OpenOrder::handler(ctx, params)
    }

    // pub fn request_cancel_order(ctx: Context<RequestCancelOrder>, order_id: [u8; 32], order_data: OrderData) -> Result<()> {
    //     RequestCancelOrder::handler(ctx, order_id, order_data)
    // }

    // pub fn claim_refund(ctx: Context<ClaimRefund>, order_id: [u8; 32]) -> Result<()> {
    //     ClaimRefund::handler(ctx, order_id)
    // }

    // Solver actions

    pub fn fill_native_order(ctx: Context<FillNativeOrder>, order_id: [u8; 32], order_data: OrderData, fill_params: FillParams) -> Result<()> {
        FillNativeOrder::handler(ctx, order_id, order_data, fill_params)
    }

    pub fn fill_foreign_order(ctx: Context<FillForeignOrder>, order_id: [u8; 32], order_data: OrderData, fill_params: FillParams) -> Result<()> {
        FillForeignOrder::handler(ctx, order_id, order_data, fill_params)
    }

    // Crosschain messaging actions

    pub fn report_order_fill(ctx: Context<ReportOrderFill>, fill_report: FillReport) -> Result<()> {
        ReportOrderFill::handler(ctx, fill_report)
    }
}
