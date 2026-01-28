// external dependencies
use anchor_lang::prelude::*;
use anchor_spl::token_interface::{
    transfer_checked, Mint, TokenAccount, TokenInterface, TransferChecked,
};
use crate::error::OrderBookError;

pub fn transfer_tokens_from_program<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &InterfaceAccount<'info, TokenAccount>,
    amount: u64,
    mint: &InterfaceAccount<'info, Mint>,
    authority: &AccountInfo<'info>,
    authority_seeds: &[&[&[u8]]],
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    // Build the arguments for the transfer instruction
    let transfer_options = TransferChecked {
        from: from.to_account_info(),
        to: to.to_account_info(),
        mint: mint.to_account_info(),
        authority: authority.clone(),
    };
    let cpi_context = CpiContext::new_with_signer(
        token_program.to_account_info(),
        transfer_options,
        authority_seeds,
    );

    // Call the transfer instruction
    transfer_checked(cpi_context, amount, mint.decimals)?;

    Ok(())
}

pub fn transfer_exact_tokens_from_program<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &mut InterfaceAccount<'info, TokenAccount>,
    amount: u64,
    mint: &InterfaceAccount<'info, Mint>,
    authority: &AccountInfo<'info>,
    authority_seeds: &[&[&[u8]]],
    token_program: &Interface<'info, TokenInterface>, 
) -> Result<()> {
    // Cache the balance of the `to` account before the transfer
    let to_start_balance = to.amount;

    // Perform the transfer
    transfer_tokens_from_program(from, to, amount, mint, authority, authority_seeds, token_program)?;

    // Reload the account to get the updated balance
    to.reload()?;

    // Check that the expected amount was actually transferred, i.e. no fee on transfer occurred
    require!(
        to_start_balance + amount <= to.amount,
        OrderBookError::TransferExactFailed
    );

    Ok(())
}

pub fn transfer_tokens<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &InterfaceAccount<'info, TokenAccount>,
    amount: u64,
    mint: &InterfaceAccount<'info, Mint>,
    authority: &AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    // Build the arguments for the transfer instruction
    let transfer_options = TransferChecked {
        from: from.to_account_info(),
        to: to.to_account_info(),
        mint: mint.to_account_info(),
        authority: authority.clone(),
    };
    let cpi_context = CpiContext::new(token_program.to_account_info(), transfer_options);

    // Call the transfer instruction
    transfer_checked(cpi_context, amount, mint.decimals)?;

    Ok(())
}

pub fn transfer_exact_tokens<'info>(
    from: &InterfaceAccount<'info, TokenAccount>,
    to: &mut InterfaceAccount<'info, TokenAccount>,
    amount: u64,
    mint: &InterfaceAccount<'info, Mint>,
    authority: &AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
) -> Result<()> {
    // Cache the balance of the `to` account before the transfer
    let to_start_balance = to.amount;

    // Perform the transfer
    transfer_tokens(from, to, amount, mint, authority, token_program)?;

    // Reload the account to get the updated balance
    to.reload()?;

    // Check that the expected amount was actually transferred, i.e. no fee on transfer occurred
    require!(
        to_start_balance + amount <= to.amount,
        OrderBookError::TransferExactFailed
    );

    Ok(())
}




