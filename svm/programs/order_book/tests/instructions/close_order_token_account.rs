//! CloseOrderTokenAccount instruction tests
//!
//! Tests for closing the order's token account after finalization
//! and returning SOL rent to the original payer.
//!
//! Scenarios:
//! [X] given the order was cancelled
//!   [X] it refunds ATA rent to the payer
//! [X] given the order was fully filled
//!   [X] it refunds ATA rent to the payer
//! [X] given the order was cross-chain and reported cancelled
//!   [X] it refunds ATA rent to the payer
//! [X] given the order was cross-chain and reported filled
//!   [X] it refunds ATA rent to the payer
//! [X] given the order is not finalized (status = Created)
//!   [X] it reverts with InvalidOrderStatus
//! [X] given a wrong payer is provided
//!   [X] it reverts
//! [X] given a wrong sender is provided
//!   [X] it reverts with InvalidSender
//! [X] given any signer after finalization
//!   [X] it succeeds (permissionless) and refunds ATA rent to the payer
//! [X] given the ATA contains dust tokens
//!   [X] it transfers dust to the sender before closing
//! [X] given dust is present but no recipient is provided
//!   [X] it reverts with DustRecipientRequired
//! [X] given a cancelled cross-chain order with pending fill reports
//!   [X] it reverts with InvalidOrderStatus until all fills arrive
//! [X] given partial fill + partial cancel equals total amount
//!   [X] it succeeds immediately
//! [X] given multiple out-of-order fills with cancel
//!   [X] it reverts until all amounts are accounted for

use super::super::{OrderBookTest, CHAIN_ID, DEST_CHAIN_ID};
use anchor_litesvm::Signer;
use anchor_spl::associated_token::get_associated_token_address;
use order_book::error::OrderBookError;
use std::error::Error;

// === Helper Functions ===

fn default_order_params(test: &OrderBookTest) -> order_book::instructions::open::OrderParams {
    order_book::instructions::open::OrderParams {
        dest_chain_id: CHAIN_ID, // local order
        created_at: test.current_time(),
        fill_deadline: test.current_time() + 100,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    }
}

fn default_xchain_order_params(
    test: &OrderBookTest,
) -> order_book::instructions::open::OrderParams {
    order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID, // cross-chain order
        created_at: test.current_time(),
        fill_deadline: test.current_time() + 100,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("bob").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    }
}

// === Happy Path Tests ===

#[test]
fn test_close_ata_after_cancel_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates an order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Warp time past deadline
    test.warp_forward(200);

    // === STEP 1: Cancel the order ===

    // Track balances BEFORE cancel
    let alice_token_ata = test.get_ata("token-in-spl-6", "alice");
    let alice_token_balance_before = test.get_token_balance(&alice_token_ata)?;
    let alice = test.get_user("alice");
    let alice_sol_balance_before = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // Cancel the order
    test.cancel_native_order("alice", "alice", order_id)?;

    // Verify AFTER cancel
    let (order_account, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );

    // Verify SPL tokens were refunded
    let alice_token_balance_after_cancel = test.get_token_balance(&alice_token_ata)?;
    assert_eq!(
        alice_token_balance_after_cancel - alice_token_balance_before,
        1_000_000,
        "SPL tokens should be refunded during cancel"
    );

    // Verify SOL did NOT increase (ATA not closed yet)
    let alice_sol_balance_after_cancel = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // SOL might decrease slightly due to tx fees, but should not increase
    assert!(
        alice_sol_balance_after_cancel <= alice_sol_balance_before,
        "SOL should not increase during cancel (ATA not closed yet)"
    );

    // Verify ATA still exists with rent
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));
    let ata_account_after_cancel = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert!(
        ata_account_after_cancel.lamports > 0,
        "ATA should still have rent after cancel"
    );
    assert!(
        !ata_account_after_cancel.data.is_empty(),
        "ATA should still have data after cancel"
    );

    // === STEP 2: Close the ATA ===

    // Track SOL balance before close
    let alice_sol_balance_before_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);
    let ata_rent = ata_account_after_cancel.lamports;

    // Close the ATA
    test.close_order_token_account("alice", order_id)?;

    // Verify SOL increased by ATA rent amount
    let alice_sol_balance_after_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    assert!(
        alice_sol_balance_after_close > alice_sol_balance_before_close,
        "SOL should increase after closing ATA"
    );

    // Check that increase is approximately the rent amount (accounting for tx fees)
    let sol_increase = alice_sol_balance_after_close - alice_sol_balance_before_close;
    assert!(
        sol_increase >= ata_rent - 10_000, // Allow small tx fee
        "SOL increase should be approximately ATA rent; expected ~{}, got {}",
        ata_rent,
        sol_increase
    );

    // Verify ATA is fully closed
    let ata_account_after_close = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(
        ata_account_after_close.lamports, 0,
        "ATA must have 0 lamports after close"
    );
    assert!(
        ata_account_after_close.data.is_empty(),
        "ATA must have empty data after close"
    );

    // Verify SPL token balance unchanged from cancel to close
    let alice_token_balance_final = test.get_token_balance(&alice_token_ata)?;
    assert_eq!(
        alice_token_balance_final, alice_token_balance_after_cancel,
        "SPL token balance should not change during ATA close"
    );

    Ok(())
}

#[test]
fn test_close_ata_after_fill_completion() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates a local order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get order account for ATA derivation
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    // Track ATA rent before fill
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    let ata_rent = ata_account.lamports;

    // Fully fill the order
    test.fill_native_order("solver", order_id, 1_000_000)?;

    // Verify order is Completed
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Completed,
        "Order should be Completed after full fill"
    );

    // Track Alice's SOL balance before close
    let alice = test.get_user("alice");
    let alice_sol_before_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // Close the ATA
    test.close_order_token_account("alice", order_id)?;

    // Verify SOL increased
    let alice_sol_after_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    let sol_increase = alice_sol_after_close - alice_sol_before_close;
    assert!(
        sol_increase >= ata_rent - 10_000,
        "SOL increase should be approximately ATA rent; expected ~{}, got {}",
        ata_rent,
        sol_increase
    );

    // Verify ATA is closed
    let ata_account_after = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account_after.lamports, 0, "ATA should be closed");
    assert!(
        ata_account_after.data.is_empty(),
        "ATA data should be empty"
    );

    Ok(())
}

#[test]
fn test_close_after_report_cancel() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates a cross-chain order (origin = here, dest = foreign)
    let order_params = default_xchain_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get order account for ATA derivation
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    // Track ATA rent
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    let ata_rent = ata_account.lamports;

    // Report cancel via portal (simulating cross-chain cancel report)
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify order is Cancelled
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled,
        "Order should be Cancelled after report_cancel"
    );

    // Track Alice's SOL balance before close
    let alice = test.get_user("alice");
    let alice_sol_before_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // Close the ATA
    test.close_order_token_account("alice", order_id)?;

    // Verify SOL increased
    let alice_sol_after_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    let sol_increase = alice_sol_after_close - alice_sol_before_close;
    assert!(
        sol_increase >= ata_rent - 10_000,
        "SOL increase should be approximately ATA rent"
    );

    // Verify ATA is closed
    let ata_account_after = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account_after.lamports, 0, "ATA should be closed");

    Ok(())
}

#[test]
fn test_close_after_report_fill() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates a cross-chain order (origin = here, dest = foreign)
    let order_params = default_xchain_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get order account for ATA derivation
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    // Track ATA rent
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    let ata_rent = ata_account.lamports;

    // Report full fill via portal (simulating cross-chain fill report)
    let fill_report = order_book::instructions::FillReport {
        order_id,
        amount_in_to_release: 1_000_000, // full amount
        amount_out_filled: 1_000_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("bob", order_params.dest_chain_id, &fill_report)?;

    // Verify order is Completed
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Completed,
        "Order should be Completed after report_fill"
    );

    // Track Alice's SOL balance before close
    let alice = test.get_user("alice");
    let alice_sol_before_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // Close the ATA
    test.close_order_token_account("alice", order_id)?;

    // Verify SOL increased
    let alice_sol_after_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    let sol_increase = alice_sol_after_close - alice_sol_before_close;
    assert!(
        sol_increase >= ata_rent - 10_000,
        "SOL increase should be approximately ATA rent"
    );

    // Verify ATA is closed
    let ata_account_after = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account_after.lamports, 0, "ATA should be closed");

    Ok(())
}

// === Authorization Tests ===

#[test]
fn test_close_fails_if_order_not_finalized() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create an order (status = Created)
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Verify order is in Created status
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Created
    );

    // Try to close - should fail because order not finalized
    let ix =
        test.create_close_order_token_account_ix(&test.get_user("alice").pubkey(), order_id)?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("alice")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

    Ok(())
}

#[test]
fn test_close_fails_with_wrong_payer() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates an order (alice is payer)
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Warp time and cancel the order
    test.warp_forward(200);
    test.cancel_native_order("alice", "alice", order_id)?;

    // Verify order is cancelled
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );

    // Bob tries to close with himself as payer (should fail)
    let ix = test.create_close_order_token_account_ix(
        &test.get_user("bob").pubkey(), // Wrong payer!
        order_id,
    )?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("bob")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidPayer));

    Ok(())
}

#[test]
fn test_close_fails_with_wrong_sender() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates an order (alice is sender and payer)
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Warp time and cancel the order
    test.warp_forward(200);
    test.cancel_native_order("alice", "alice", order_id)?;

    // Verify order is cancelled
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );

    // Try to close with correct payer but wrong sender
    // Note: No dust in ATA after cancel, so we don't need recipient
    let ix = test.create_close_order_token_account_ix_custom(
        &test.get_user("alice").pubkey(), // Correct payer
        &test.get_user("bob").pubkey(),   // Wrong sender!
        order_id,
        false, // No recipient needed (no dust after cancel)
    )?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("alice")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidSender));

    Ok(())
}

#[test]
fn test_anyone_can_close_after_finalization() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates an order (alice is payer and sender)
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get order account for ATA derivation
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    // Track ATA rent
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    let ata_rent = ata_account.lamports;

    // Warp time and cancel the order
    test.warp_forward(200);
    test.cancel_native_order("alice", "alice", order_id)?;

    // Track Alice's SOL balance before close (Alice is the original payer)
    let alice = test.get_user("alice");
    let alice_sol_before_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // Track Bob's SOL balance (Bob will pay the tx fee)
    let bob = test.get_user("bob");
    let bob_sol_before_close = test
        .ctx
        .svm
        .get_account(&bob.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // Bob calls close (permissionless), but passes Alice's pubkey as the payer account
    // The instruction must still specify Alice as payer for rent refund
    let ix = test.create_close_order_token_account_ix_custom(
        &alice.pubkey(), // Payer account (Alice) - receives rent
        &alice.pubkey(), // Sender account (Alice)
        order_id,
        true, // include recipient
    )?;

    // Bob signs the transaction (pays tx fee)
    test.ctx.execute_instruction(ix, &[&bob])?.assert_success();

    // Verify Alice's SOL increased (received rent refund)
    let alice_sol_after_close = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    let alice_sol_increase = alice_sol_after_close - alice_sol_before_close;
    assert!(
        alice_sol_increase >= ata_rent - 1000, // Some tolerance
        "Alice should receive ATA rent; expected ~{}, got {}",
        ata_rent,
        alice_sol_increase
    );

    // Verify Bob's SOL decreased (paid tx fee)
    let bob_sol_after_close = test
        .ctx
        .svm
        .get_account(&bob.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    assert!(
        bob_sol_after_close < bob_sol_before_close,
        "Bob should have paid tx fee"
    );

    // Verify ATA is closed
    let ata_account_after = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account_after.lamports, 0, "ATA should be closed");

    Ok(())
}

// === Dust Token Tests ===

#[test]
fn test_close_with_dust_tokens_transferred_to_sender() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates an order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get order account for ATA derivation
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    // Warp time and cancel the order (tokens refunded to Alice)
    test.warp_forward(200);
    test.cancel_native_order("alice", "alice", order_id)?;

    // Verify ATA is empty after cancel
    let ata_balance_after_cancel = test.get_token_balance(&order_token_in_ata)?;
    assert_eq!(
        ata_balance_after_cancel, 0,
        "ATA should be empty after cancel"
    );

    // Simulate griefing: mint dust tokens to the order's ATA
    let dust_amount = 1_000u64; // 1000 dust tokens
    test.mint_to_ata("token-in-spl-6", &order_token_in_ata, dust_amount)?;

    // Verify dust is in the ATA
    let ata_balance_with_dust = test.get_token_balance(&order_token_in_ata)?;
    assert_eq!(
        ata_balance_with_dust, dust_amount,
        "ATA should have dust tokens"
    );

    // Track Alice's token balance before close
    let alice_token_ata = test.get_ata("token-in-spl-6", "alice");
    let alice_token_before_close = test.get_token_balance(&alice_token_ata)?;

    // Close the ATA with recipient (dust should go to Alice)
    test.close_order_token_account("alice", order_id)?;

    // Verify dust was transferred to Alice (sender)
    let alice_token_after_close = test.get_token_balance(&alice_token_ata)?;
    assert_eq!(
        alice_token_after_close - alice_token_before_close,
        dust_amount,
        "Dust tokens should be transferred to sender"
    );

    // Verify ATA is closed
    let ata_account_after = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account_after.lamports, 0, "ATA should be closed");
    assert!(
        ata_account_after.data.is_empty(),
        "ATA data should be empty"
    );

    Ok(())
}

#[test]
fn test_close_fails_without_recipient_when_dust_present() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates an order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get order account for ATA derivation
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    // Warp time and cancel the order
    test.warp_forward(200);
    test.cancel_native_order("alice", "alice", order_id)?;

    // Simulate griefing: mint dust tokens to the order's ATA
    let dust_amount = 1_000u64;
    test.mint_to_ata("token-in-spl-6", &order_token_in_ata, dust_amount)?;

    // Verify dust is in the ATA
    let ata_balance = test.get_token_balance(&order_token_in_ata)?;
    assert_eq!(ata_balance, dust_amount, "ATA should have dust tokens");

    // Try to close WITHOUT providing recipient_token_account
    let ix = test.create_close_order_token_account_ix_custom(
        &test.get_user("alice").pubkey(),
        &test.get_user("alice").pubkey(),
        order_id,
        false, // NO recipient - should fail!
    )?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("alice")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::DustRecipientRequired));

    // Verify ATA is still there (close failed)
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert!(ata_account.lamports > 0, "ATA should still exist");

    Ok(())
}

// === Out-of-Order Message Tests ===

/// Test that closing a cancelled order's token account fails if fill reports
/// haven't arrived yet (out-of-order message scenario).
///
/// This test validates the bug fix in commit ac152cf which prevents premature
/// closing of token accounts when:
/// 1. A cancel report arrives first (marking order as Cancelled)
/// 2. But fill reports haven't arrived yet (funds reserved for fills)
///
/// The scenario:
/// - User creates cross-chain order for 1,000,000 tokens
/// - Cancel report arrives with partial refund (500,000)
/// - Close attempt should FAIL because amount_in_released + amount_in_refunded != amount_in
/// - Fill report arrives (500,000)
/// - Now close should SUCCEED because all funds are accounted for
#[test]
fn test_close_ata_fails_when_cancel_arrives_before_fill_report() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates a cross-chain order (origin = here, dest = foreign chain)
    let order_params = default_xchain_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Verify order is created with full amount_in escrowed
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Created
    );
    assert_eq!(order_data.data.amount_in, 1_000_000);
    assert_eq!(order_data.data.amount_in_released, 0);
    assert_eq!(order_data.data.amount_in_refunded, 0);

    // === SCENARIO: Cancel report arrives BEFORE fill report (out-of-order) ===
    //
    // On the destination chain:
    // 1. Solver fills 500,000 (sends fill report)
    // 2. Order gets cancelled for remaining 500,000 (sends cancel report)
    // 3. Cancel report arrives first on origin chain
    // 4. Fill report is still in flight

    // Report cancel with partial refund (only 500,000 because 500,000 was filled on dest chain)
    let cancel_report = order_book::instructions::CancelReport {
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: 500_000u128, // Only partial refund
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify order is Cancelled but funds are NOT fully accounted for
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled,
        "Order should be Cancelled after cancel report"
    );
    assert_eq!(order_data.data.amount_in_released, 0, "No fill reports yet");
    assert_eq!(
        order_data.data.amount_in_refunded, 500_000,
        "Partial refund recorded"
    );

    // Key invariant violation: amount_in_released + amount_in_refunded != amount_in
    // 0 + 500_000 != 1_000_000 -> close should be blocked
    assert_ne!(
        order_data.data.amount_in_released + order_data.data.amount_in_refunded,
        order_data.data.amount_in,
        "Not all funds are accounted for yet"
    );

    // === STEP 1: Attempt to close ATA - should FAIL ===
    let ix =
        test.create_close_order_token_account_ix(&test.get_user("alice").pubkey(), order_id)?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("alice")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

    // Expire blockhash to avoid AlreadyProcessed error
    test.ctx.svm.expire_blockhash();

    // === STEP 2: Fill report finally arrives ===
    let fill_report = order_book::instructions::FillReport {
        order_id,
        amount_in_to_release: 500_000,
        amount_out_filled: 500_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("admin", order_params.dest_chain_id, &fill_report)?;

    // Verify all funds are now accounted for
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled,
        "Order should remain Cancelled (status doesn't change back)"
    );
    assert_eq!(
        order_data.data.amount_in_released, 500_000,
        "Fill report processed"
    );
    assert_eq!(order_data.data.amount_in_refunded, 500_000, "Refund remains");
    assert_eq!(
        order_data.data.amount_in_released + order_data.data.amount_in_refunded,
        order_data.data.amount_in,
        "All funds should now be accounted for (500k + 500k = 1M)"
    );

    // === STEP 3: Now close should SUCCEED ===
    test.close_order_token_account("alice", order_id)?;

    // Verify ATA is closed
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account.lamports, 0, "ATA should be closed");
    assert!(ata_account.data.is_empty(), "ATA data should be empty");

    Ok(())
}

/// Test that closing succeeds when partial fill + partial refund equals total amount_in.
///
/// This tests the happy path where:
/// - Fill report arrives first (400,000 tokens released to solver)
/// - Cancel report arrives second (600,000 tokens refunded to sender)
/// - Total: 400,000 + 600,000 = 1,000,000 = amount_in
/// - Close should succeed immediately
#[test]
fn test_close_ata_succeeds_when_partial_fill_plus_cancel_equals_total() -> Result<(), Box<dyn Error>>
{
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates a cross-chain order for 1,000,000 tokens
    let order_params = default_xchain_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get order account for ATA derivation
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    // Track ATA rent for verification later
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    let ata_rent = ata_account.lamports;

    // === STEP 1: Report partial fill for 400,000 tokens (40%) ===
    let fill_report = order_book::instructions::FillReport {
        order_id,
        amount_in_to_release: 400_000,
        amount_out_filled: 400_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("admin", order_params.dest_chain_id, &fill_report)?;

    // Verify partial fill state
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Created
    );
    assert_eq!(order_data.data.amount_in_released, 400_000);
    assert_eq!(order_data.data.amount_in_refunded, 0);

    // Expire blockhash
    test.ctx.svm.expire_blockhash();

    // === STEP 2: Report cancel for remaining 600,000 tokens (60%) ===
    let cancel_report = order_book::instructions::CancelReport {
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: 600_000u128, // Remaining amount
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify order is Cancelled with all funds accounted for
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );
    assert_eq!(order_data.data.amount_in_released, 400_000);
    assert_eq!(order_data.data.amount_in_refunded, 600_000);

    // Verify: 400,000 + 600,000 = 1,000,000 (full amount accounted for)
    assert_eq!(
        order_data.data.amount_in_released + order_data.data.amount_in_refunded,
        order_data.data.amount_in,
        "All funds should be accounted for"
    );

    // Track Alice's SOL balance before close
    let alice = test.get_user("alice");
    let alice_sol_before = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);

    // === STEP 3: Close should succeed immediately ===
    test.close_order_token_account("alice", order_id)?;

    // Verify SOL rent was refunded
    let alice_sol_after = test
        .ctx
        .svm
        .get_account(&alice.pubkey())
        .map(|a| a.lamports)
        .unwrap_or(0);
    let sol_increase = alice_sol_after - alice_sol_before;
    assert!(
        sol_increase >= ata_rent - 10_000,
        "Alice should receive ATA rent; expected ~{}, got {}",
        ata_rent,
        sol_increase
    );

    // Verify ATA is closed
    let ata_account_after = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account_after.lamports, 0, "ATA should be closed");
    assert!(
        ata_account_after.data.is_empty(),
        "ATA data should be empty"
    );

    Ok(())
}

/// Test multiple out-of-order fill reports with a cancel report.
///
/// This is the most complex scenario testing the accounting logic:
/// - Order for 1,000,000 tokens
/// - Cancel report arrives first (refund 500,000)
/// - Close fails (500k accounted, 500k missing)
/// - First fill report arrives (release 300,000)
/// - Close still fails (800k accounted, 200k missing)
/// - Second fill report arrives (release 200,000)
/// - Close succeeds (1000k accounted = full amount)
#[test]
fn test_close_ata_multiple_out_of_order_fills_with_cancel() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Alice creates a cross-chain order for 1,000,000 tokens
    let order_params = default_xchain_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // === Out-of-order message arrival scenario ===
    //
    // On destination chain, the events happened in this order:
    // 1. Solver A fills 300,000 (sends fill report 1)
    // 2. Solver B fills 200,000 (sends fill report 2)
    // 3. Order cancelled for remaining 500,000 (sends cancel report)
    //
    // Messages arrive on origin chain in different order:
    // 1. Cancel report arrives first
    // 2. Fill report 1 arrives second
    // 3. Fill report 2 arrives third

    // === STEP 1: Cancel report arrives first (refund 500,000) ===
    let cancel_report = order_book::instructions::CancelReport {
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: 500_000u128,
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify state: Cancelled, 0 released, 500k refunded
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );
    assert_eq!(order_data.data.amount_in_released, 0);
    assert_eq!(order_data.data.amount_in_refunded, 500_000);

    // Attempt close - should FAIL (500k accounted, 500k missing)
    let ix =
        test.create_close_order_token_account_ix(&test.get_user("alice").pubkey(), order_id)?;
    test.ctx
        .execute_instruction(ix, &[&test.get_user("alice")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

    test.ctx.svm.expire_blockhash();

    // === STEP 2: First fill report arrives (release 300,000) ===
    let fill_report_1 = order_book::instructions::FillReport {
        order_id,
        amount_in_to_release: 300_000,
        amount_out_filled: 300_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("admin", order_params.dest_chain_id, &fill_report_1)?;

    // Verify state: still Cancelled, 300k released, 500k refunded
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );
    assert_eq!(order_data.data.amount_in_released, 300_000);
    assert_eq!(order_data.data.amount_in_refunded, 500_000);

    // Total accounted: 300k + 500k = 800k != 1000k
    assert_eq!(
        order_data.data.amount_in_released + order_data.data.amount_in_refunded,
        800_000,
        "Should have 800k accounted for at this point"
    );

    test.ctx.svm.expire_blockhash();

    // Attempt close - should still FAIL (800k accounted, 200k missing)
    let ix =
        test.create_close_order_token_account_ix(&test.get_user("alice").pubkey(), order_id)?;
    test.ctx
        .execute_instruction(ix, &[&test.get_user("alice")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

    test.ctx.svm.expire_blockhash();

    // === STEP 3: Second fill report arrives (release 200,000) ===
    let fill_report_2 = order_book::instructions::FillReport {
        order_id,
        amount_in_to_release: 200_000,
        amount_out_filled: 200_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("admin", order_params.dest_chain_id, &fill_report_2)?;

    // Verify state: Cancelled, 500k released, 500k refunded
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );
    assert_eq!(
        order_data.data.amount_in_released, 500_000,
        "300k + 200k = 500k released"
    );
    assert_eq!(order_data.data.amount_in_refunded, 500_000);

    // Total accounted: 500k + 500k = 1000k == amount_in
    assert_eq!(
        order_data.data.amount_in_released + order_data.data.amount_in_refunded,
        order_data.data.amount_in,
        "All funds should be accounted for now"
    );

    // === STEP 4: Close should now SUCCEED ===
    test.close_order_token_account("alice", order_id)?;

    // Verify ATA is closed
    let (order_account, _) = test.get_native_order_account(&order_id)?;
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));
    let ata_account = test.ctx.svm.get_account(&order_token_in_ata).unwrap();
    assert_eq!(ata_account.lamports, 0, "ATA should be closed");
    assert!(ata_account.data.is_empty(), "ATA data should be empty");

    Ok(())
}
