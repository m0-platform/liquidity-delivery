use anchor_lang::prelude::*;
use order_book::instructions::FillReport;

declare_id!("8KytuBaFrs7wv2oprMgrkscrogPVi1baqTewfgUZoaHr");

// This is a placeholder to provide an interface to build against
#[program]
pub mod messenger {
    use super::*;

    pub fn send_fill_report(
        _ctx: Context<SendFillReport>,
        origin_chain_id: u32,
        fill_report: FillReport,
    ) -> Result<()> {
        msg!(
            "Sending order filled message to chain {}: {:?}",
            origin_chain_id,
            fill_report
        );
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SendFillReport<'info> {
    pub signer: Signer<'info>,
}
