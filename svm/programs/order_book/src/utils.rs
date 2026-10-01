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

/// Computes floor(a * b / denominator) using a 256-bit intermediate product so the
/// multiplication cannot overflow. Errors with MathOverflow if the quotient does not fit in a u128.
pub fn muldiv_u128(a: u128, b: u128, denominator: u128) -> Result<u128> {
    require!(denominator > 0, OrderBookError::MathUnderflow);

    // Fast path: the product fits in a u128
    if let Some(product) = a.checked_mul(b) {
        return Ok(product / denominator);
    }

    // Full 256-bit product of a * b via 64-bit limbs, as (hi, lo) u128 halves
    let lo_mask: u128 = u64::MAX as u128;
    let (a_lo, a_hi) = (a & lo_mask, a >> 64);
    let (b_lo, b_hi) = (b & lo_mask, b >> 64);
    let p0 = a_lo * b_lo;
    let p1 = a_lo * b_hi;
    let p2 = a_hi * b_lo;
    let p3 = a_hi * b_hi;
    let mid = (p0 >> 64) + (p1 & lo_mask) + (p2 & lo_mask);
    let lo = (mid << 64) | (p0 & lo_mask);
    let hi = p3 + (p1 >> 64) + (p2 >> 64) + (mid >> 64);

    // The quotient fits in a u128 only if hi < denominator
    require!(hi < denominator, OrderBookError::MathOverflow);

    // Binary long division of the 256-bit product by the denominator
    let mut quotient: u128 = 0;
    let mut remainder: u128 = 0;
    for i in (0..256u32).rev() {
        let bit = if i >= 128 {
            (hi >> (i - 128)) & 1
        } else {
            (lo >> i) & 1
        };
        // The remainder can carry out of 128 bits on the shift; wrapping arithmetic below
        // stays correct because the true value is always < 2 * denominator <= 2^129
        let carry = remainder >> 127;
        remainder = (remainder << 1) | bit;
        quotient <<= 1;
        if carry != 0 || remainder >= denominator {
            remainder = remainder.wrapping_sub(denominator);
            quotient |= 1;
        }
    }

    Ok(quotient)
}

#[cfg(test)]
mod tests {
    use super::muldiv_u128;

    #[test]
    fn muldiv_small_values_match_direct_math() {
        assert_eq!(muldiv_u128(6, 7, 3).unwrap(), 14);
        assert_eq!(muldiv_u128(10, 10, 3).unwrap(), 33); // floor
        assert_eq!(muldiv_u128(0, u128::MAX, 5).unwrap(), 0);
    }

    #[test]
    fn muldiv_overflowing_product_is_exact() {
        // a * b overflows u128 but the quotient fits
        let a = 1_000_000_000_000_000_000_000u128; // 1e21
        let b = u128::MAX / 2;
        assert_eq!(muldiv_u128(a, b, b).unwrap(), a);
        assert_eq!(muldiv_u128(a, b, a).unwrap(), b);
        assert_eq!(
            muldiv_u128(u128::MAX, u128::MAX, u128::MAX).unwrap(),
            u128::MAX
        );
        // floor((2^128 - 1) * 3 / 7)
        assert_eq!(
            muldiv_u128(u128::MAX, 3, 7).unwrap(),
            ((u128::MAX / 7) * 3) + ((u128::MAX % 7) * 3) / 7
        );
    }

    #[test]
    fn muldiv_quotient_too_large_errors() {
        assert!(muldiv_u128(u128::MAX, u128::MAX, 2).is_err());
        assert!(muldiv_u128(u128::MAX, 2, 1).is_err());
    }

    #[test]
    fn muldiv_zero_denominator_errors() {
        assert!(muldiv_u128(1, 1, 0).is_err());
    }
}
