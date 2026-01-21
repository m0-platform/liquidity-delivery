use super::super::{OrderBookTest, CHAIN_ID};
use anchor_litesvm::Signer;
use order_book::error::OrderBookError;
use std::error::Error;

// Pause/Unpause instruction tests
// [X] given a non-admin calls pause
//   [X] it reverts with a NotAuthorized error
// [X] given a non-admin calls unpause
//   [X] it reverts with a NotAuthorized error
// [X] given the admin calls pause
//   [X] it sets paused to true
// [X] given the admin calls unpause
//   [X] it sets paused to false
//
// Unpause allows instructions tests
// [X] given the program was paused and then unpaused
//   [X] open_order succeeds
//
// Note: Pause blocking tests for individual instructions are located in their
// respective test files (open.rs, fill.rs, cancel_native_order.rs, cancel_foreign_order.rs)

// Helper functions

fn default_order_params(test: &OrderBookTest) -> order_book::instructions::open::OrderParams {
    order_book::instructions::open::OrderParams {
        dest_chain_id: CHAIN_ID, // local order
        created_at: test.current_time(),
        fill_deadline: test.current_time() + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    }
}

// =============================================================================
// Admin Authorization Tests
// =============================================================================

#[test]
fn test_pause_requires_admin() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Non-admin (alice) tries to pause
    let alice = test.get_user("alice");
    let ix = test.create_pause_ix(&alice.pubkey())?;

    test.ctx
        .execute_instruction(ix, &[&alice])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

    // Verify program is still not paused
    let (_, global_data) = test.get_global_account()?;
    assert!(!global_data.paused);

    Ok(())
}

#[test]
fn test_unpause_requires_admin() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Admin pauses first
    test.pause()?;

    // Non-admin (alice) tries to unpause
    let alice = test.get_user("alice");
    let ix = test.create_unpause_ix(&alice.pubkey())?;

    test.ctx
        .execute_instruction(ix, &[&alice])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

    // Verify program is still paused
    let (_, global_data) = test.get_global_account()?;
    assert!(global_data.paused);

    Ok(())
}

#[test]
fn test_admin_can_pause() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Verify not paused initially
    let (_, global_data) = test.get_global_account()?;
    assert!(!global_data.paused);

    // Admin pauses
    test.pause()?;

    // Verify now paused
    let (_, global_data) = test.get_global_account()?;
    assert!(global_data.paused);

    Ok(())
}

#[test]
fn test_admin_can_unpause() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Pause first
    test.pause()?;

    // Verify paused
    let (_, global_data) = test.get_global_account()?;
    assert!(global_data.paused);

    // Unpause
    test.unpause()?;

    // Verify unpaused
    let (_, global_data) = test.get_global_account()?;
    assert!(!global_data.paused);

    Ok(())
}

// =============================================================================
// Unpause Allows Instructions Tests
// =============================================================================

#[test]
fn test_unpause_allows_open_order() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Pause the program
    test.pause()?;

    // Verify open_order fails while paused
    let alice = test.get_user("alice");
    let order_params = default_order_params(&test);
    let (_, ix) = test.create_open_order_ix(
        &alice.pubkey(),
        &test.get_mint("token-in-spl-6"),
        &test.get_ata("token-in-spl-6", "alice"),
        None,
        &order_params,
    )?;

    test.ctx
        .execute_instruction(ix, &[&alice])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::ProgramPaused));

    // Unpause the program
    test.unpause()?;

    // Now open_order should succeed
    test.ctx.svm.expire_blockhash(); // Need fresh blockhash for new transaction
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Verify order was created
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Created
    );

    Ok(())
}
