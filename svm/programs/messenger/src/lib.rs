use anchor_lang::prelude::*;

declare_id!("8KytuBaFrs7wv2oprMgrkscrogPVi1baqTewfgUZoaHr");

#[program]
pub mod messenger {
    use super::*;

    pub fn send_fill_report(_ctx: Context<SendFillReport>, origin_chain_id: u32, fill_report: FillReport) -> Result<()> {
        // TODO this is a placeholder to provide an interface to build against
        msg!("Sending order filled message to chain {}: {:?}", origin_chain_id, fill_report);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SendFillReport<'info> {
    // TODO restrict to order book global account
    pub signer: Signer<'info>,
}

#[derive(Debug, AnchorSerialize, AnchorDeserialize)]
pub struct FillReport {
    pub order_id: [u8; 32],
    pub amount_out_filled: u128,
    pub origin_recipient: [u8; 32],
}

#[constant]
pub const AUTHORITY_SEED: &[u8] = b"authority";