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
