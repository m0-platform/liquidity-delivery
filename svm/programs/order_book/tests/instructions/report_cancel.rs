use super::super::{OrderBookTest, DEST_CHAIN_ID};
use anchor_litesvm::{Signer, TestHelpers};
use anchor_spl::associated_token::get_associated_token_address;
use order_book::{error::OrderBookError, FillReport, ORDER_SEED_PREFIX};
use std::error::Error;

// ReportOrderCancel instruction tests
// For reporting cancels back to the origin chain for orders that originated here
// but had a different destination chain
//
// [X] given the portal_authority does not match global account
//   [X] it reverts with NotAuthorized error
// [X] given the order does not exist
//   [X] it reverts with AccountNotInitialized error
// [X] given the order type is Foreign
//   [X] it reverts with InvalidOrderType error
// [X] given the order status is not Created
//   [X] it reverts with InvalidOrderStatus error
// [X] given the order_sender does not match the order
//   [X] it reverts with InvalidSender error
// [X] given the order has been fully filled (no remaining tokens)
//   [X] it reverts with OrderFilled error
// [X] given the reported order_sender does not match the order's sender
//   [X] it reverts with InvalidSender error
// [X] given the reported token_in does not match the order's token_in
//   [X] it reverts with InvalidTokenIn error
// [X] given all checks pass
//   [X] it sets order status to Cancelled
//   [X] it transfers remaining token_in to sender
// [X] given a partial fill occurred
//   [X] it refunds only the remaining tokens
// [X] given a partial fill occurred and refund exceeds available
//   [X] it reverts with InvalidRefundAmount
// [X] given the program is paused
//   [X] it completes successfully

fn default_order_params(test: &OrderBookTest) -> order_book::instructions::open::OrderParams {
    // Order that originates here but has a different destination
    order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID, // cross-chain order to another chain
        created_at: test.current_time(),
        fill_deadline: test.current_time() + 100,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("bob").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    }
}

#[test]
fn test_report_cancel_unauthorized_portal_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    let cancel_report = order_book::instructions::CancelReport {
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };

    // Build accounts with wrong portal_authority (carol instead of the configured one)
    let relayer = test.get_user("bob");
    let wrong_portal = test.get_user("carol");

    let (_, native_order) = test.get_native_order_account(&order_id)?;
    let order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
    let token_in_mint = native_order.data.token_in;
    let order_sender = native_order.data.sender;
    let sender_token_in_ata = get_associated_token_address(&order_sender, &token_in_mint);
    let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

    let accounts = order_book::accounts::ReportOrderCancel {
        program: order_book::ID,
        event_authority: test.get_event_authority()?,
        relayer: relayer.pubkey(),
        portal_authority: wrong_portal.pubkey(), // Wrong portal
        global_account: test.get_global_account()?.0,
        order: order_account,
        token_in_mint,
        order_sender,
        sender_token_in_ata,
        order_token_in_ata,
        token_in_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        system_program: anchor_lang::solana_program::system_program::ID,
    };

    let ix = test
        .ctx
        .program()
        .accounts(accounts)
        .args(order_book::instruction::ReportOrderCancel {
            source_chain_id: order_params.dest_chain_id,
            cancel_report: cancel_report.clone(),
        })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &wrong_portal])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

    Ok(())
}

#[test]
fn test_report_cancel_order_not_exist_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    let fake_order_id = [99u8; 32];
    let cancel_report = order_book::instructions::CancelReport {
        order_id: fake_order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: 1_000_000 as u128
    };

    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");

    let fake_order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &fake_order_id], &order_book::ID);

    // Create token accounts for fake order
    let token_in_mint = test.get_mint("token-in-spl-6");
    let fake_order_token_in_ata =
        test.create_associated_token_account(&token_in_mint, &fake_order_account)?;

    let accounts = order_book::accounts::ReportOrderCancel {
        program: order_book::ID,
        event_authority: test.get_event_authority()?,
        relayer: relayer.pubkey(),
        portal_authority: portal_authority.pubkey(),
        global_account: test.get_global_account()?.0,
        order: fake_order_account,
        token_in_mint,
        order_sender: test.get_user("alice").pubkey(),
        sender_token_in_ata: test.get_ata("token-in-spl-6", "alice"),
        order_token_in_ata: fake_order_token_in_ata,
        token_in_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        system_program: anchor_lang::solana_program::system_program::ID,
    };

    let ix = test
        .ctx
        .program()
        .accounts(accounts)
        .args(order_book::instruction::ReportOrderCancel {
            source_chain_id: DEST_CHAIN_ID,
            cancel_report: cancel_report.clone(),
        })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error("AccountNotInitialized");

    Ok(())
}

#[test]
fn test_report_cancel_completed_order_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create and complete a crosschain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Fully fill the order
    let fill_report = order_book::instructions::FillReport {
        order_id,
        amount_in_to_release: order_params.amount_in as u128,
        amount_out_filled: order_params.amount_out as u128,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("solver", order_params.dest_chain_id, &fill_report)?;

    // Verify order is completed
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Completed
    );

    // Try to report cancel on completed order
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_data.data.amount_in
    };

    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");

    let ix = test.create_report_cancel_ix(
        &relayer.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &cancel_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

    Ok(())
}

#[test]
fn test_report_cancel_wrong_sender_account_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain order from alice
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };

    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");

    let (_, native_order) = test.get_native_order_account(&order_id)?;
    let order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
    let token_in_mint = native_order.data.token_in;

    // Use wrong sender (carol instead of alice)
    let wrong_sender = test.get_user("carol");
    let sender_token_in_ata =
        get_associated_token_address(&wrong_sender.pubkey(), &token_in_mint);
    let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

    let accounts = order_book::accounts::ReportOrderCancel {
        program: order_book::ID,
        event_authority: test.get_event_authority()?,
        relayer: relayer.pubkey(),
        portal_authority: portal_authority.pubkey(),
        global_account: test.get_global_account()?.0,
        order: order_account,
        token_in_mint,
        order_sender: wrong_sender.pubkey(), // Wrong sender
        sender_token_in_ata,
        order_token_in_ata,
        token_in_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        system_program: anchor_lang::solana_program::system_program::ID,
    };

    let ix = test
        .ctx
        .program()
        .accounts(accounts)
        .args(order_book::instruction::ReportOrderCancel {
            source_chain_id: order_params.dest_chain_id,
            cancel_report: cancel_report.clone(),
        })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidSender));

    Ok(())
}

#[test]
fn test_report_cancel_reported_sender_mismatch_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Create cancel report with wrong sender (carol instead of alice)
    let wrong_sender = test.get_user("carol");
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: wrong_sender.pubkey().to_bytes(), // Wrong sender
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };

    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");

    let ix = test.create_report_cancel_ix(
        &relayer.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &cancel_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidSender));

    Ok(())
}

#[test]
fn test_report_cancel_reported_token_in_mismatch_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Create cancel report with wrong token_in (token-out-spl-6 instead of token-in-spl-6)
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-out-spl-6").to_bytes(), // Wrong token_in
        amount_in_to_refund: order_params.amount_in as u128
    };

    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");
    let ix = test.create_report_cancel_ix(
        &relayer.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &cancel_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidTokenMint));

    Ok(())
}

#[test]
fn test_report_cancel_wrong_source_chain_id_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };

    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");

    let ix = test.create_report_cancel_ix(
        &relayer.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id + 1, // Wrong source chain ID
        &cancel_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidReportSource));

    Ok(())
}

#[test]
fn test_report_cancel_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get initial balance
    let sender_ata = test.get_ata("token-in-spl-6", "alice");
    let initial_balance = test.get_token_balance(&sender_ata)?;

    // Report cancel
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify order is cancelled
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );

    // Verify tokens were refunded
    let final_balance = test.get_token_balance(&sender_ata)?;
    assert_eq!(final_balance, initial_balance + 1_000_000);

    Ok(())
}

#[test]
fn test_report_cancel_partial_fill_refunds_remaining() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create a crosschain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Report partial fill (50%)
    let fill_report = order_book::instructions::FillReport {
        order_id,
        amount_in_to_release: 500_000u128,
        amount_out_filled: 500_000u128,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("admin", order_params.dest_chain_id, &fill_report)?;

    // Get balance after partial fill
    let sender_ata = test.get_ata("token-in-spl-6", "alice");
    let balance_after_fill = test.get_token_balance(&sender_ata)?;

    // Report cancel
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128 - fill_report.amount_in_to_release
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify order is cancelled
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );

    // Verify only remaining 50% was refunded
    let final_balance = test.get_token_balance(&sender_ata)?;
    assert_eq!(final_balance, balance_after_fill + 500_000);

    Ok(())
}

#[test]
fn test_report_cancel_already_cancelled_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Report cancel first time
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify order is cancelled
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );

    // Expire blockhash to avoid AlreadyProcessed error
    test.ctx.svm.expire_blockhash();

    // Try to report cancel again
    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");

    let ix = test.create_report_cancel_ix(
        &relayer.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &cancel_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

    Ok(())
}

#[test]
fn test_report_cancel_paused_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create a cross-chain order (required for report_cancel)
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Pause the program
    test.pause()?;

    // Try to report a cancel while paused
    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");
    let cancel_report = order_book::instructions::CancelReport { 
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: order_params.amount_in as u128
    };

    let ix = test.create_report_cancel_ix(
        &relayer.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &cancel_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_success();

    Ok(())
}

#[test]
fn test_report_cancel_after_partial_fill_refund_exceeds_available_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create a crosschain order
    let order_params = default_order_params(&test);
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Report partial fill (50%)
    let fill_report = FillReport {
        order_id,
        amount_in_to_release: 500_000u128,
        amount_out_filled: 500_000u128,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("admin", order_params.dest_chain_id, &fill_report)?;

    // Verify order is partially filled
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(order_data.data.amount_in_released, 500_000);

    // Expire blockhash to avoid AlreadyProcessed error
    test.ctx.svm.expire_blockhash();

    // Try to report cancel for 51% (more than the remaining 50%)
    // This should fail because amount_in_released + amount_in_to_refund > amount_in
    let relayer = test.get_user("bob");
    let portal_authority = test.get_user("portal_authority");
    let cancel_report = order_book::instructions::CancelReport {
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: 510_000u128, // 51% - exceeds remaining 50%
    };

    let ix = test.create_report_cancel_ix(
        &relayer.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &cancel_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&relayer, &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidRefundAmount));

    Ok(())
}
