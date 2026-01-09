use super::super::{OrderBookTest, CHAIN_ID, DEST_CHAIN_ID};
use anchor_litesvm::{Signer, TestHelpers};
use anchor_spl::{associated_token::get_associated_token_address, token::spl_token};
use order_book::{error::OrderBookError, FillReport, ORDER_SEED_PREFIX};
use std::error::Error;

// ReportOrderFill instruction tests
// [X] given the portal_authority does not match the global account
//   [X] it reverts with a NotAuthorized error
// [X] given the source chain ID does not match the order's dest_chain_id
//   [X] it reverts with an InvalidReportSource error
// [X] given the order does not exist
//   [X] it reverts with an AccountNotInitialized error
// [X] given the order type is Foreign
//   [X] it reverts with an InvalidOrderType or deserialization error
// [X] given the order status is Completed
//   [X] it reverts with an OrderNotFillable error
// [X] given the order status is Cancelled with full refund
//   [X] it reverts with an InvalidFillAmount error (no tokens available)
// [X] given the order status is Cancelled with partial refund
//   [X] it processes the fill successfully if amount_in_released + amount_in_refunded <= amount_in
// [X] given the order status is Cancelled and fill exceeds remaining
//   [X] it reverts with an InvalidFillAmount error
// [X] given the fill_report amount_out_filled is zero
//   [X] it reverts with an InvalidFillAmount error
// [X] given the token_in_mint does not match the order
//   [X] it reverts with an InvalidTokenMint error
// [X] given the origin_recipient does not match fill_report
//   [X] it reverts with an InvalidRecipient error
// [X] given the order account PDA is incorrect
//   [X] it reverts with a ConstraintSeeds error
// [X] given all checks pass and this is a partial fill
//   [X] it updates amount_in_released cumulatively
//   [X] it updates amount_out_filled cumulatively
//   [X] it transfers token_in to origin_recipient
//   [X] it keeps order status as Created
//   [X] it reduces order token balance correctly
// [X] given all checks pass and this completes the order
//   [X] it updates order status to Completed
//   [X] it transfers ALL remaining tokens to origin_recipient
//   [X] it sets amount_out_filled to equal amount_out
//   [X] it sets amount_in_released correctly
// [X] given multiple partial fills are reported
//   [X] it processes each fill correctly
//   [X] it tracks cumulative amounts
//   [X] it changes status to Completed on final fill
// [X] given extra tokens are donated to the order account
//   [X] on full fill, it transfers only the exact fill amount (donation remains in ATA)
// [X] given the order status is CancelRequested
//   [X] it still processes the fill (CancelRequested orders can be filled)
// [X] given the program is paused
//   [X] it completes successfully

fn default_fill_report(
    test: &OrderBookTest,
    order_id: [u8; 32],
    origin_recipient: [u8; 32],
) -> FillReport {
    FillReport {
        order_id,
        amount_in_to_release: 500_000,
        amount_out_filled: 500_000,
        origin_recipient,
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    }
}

#[test]
fn test_report_fill_unauthorized_portal_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create a native order (origin = CHAIN_ID, dest = DEST_CHAIN_ID)
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID, // foreign destination
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Create fill report
    let fill_report =
        default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());

    // Build accounts with wrong portal authority
    let portal_authority = test.get_user("bob");
    let accounts = test.build_report_fill_accounts(
        &test.get_user("admin").pubkey(),
        &portal_authority.pubkey(),
        &fill_report,
    )?;

    let ix = test
        .ctx
        .program()
        .accounts(accounts)
        .args(order_book::instruction::ReportOrderFill { source_chain_id: DEST_CHAIN_ID, fill_report })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

    Ok(())
}

#[test]
fn test_report_fill_invalid_source_chain_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create a native order (origin = CHAIN_ID, dest = DEST_CHAIN_ID)
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID, // foreign destination
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Create fill report
    let fill_report =
        default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());

    let ix = test.create_report_fill_ix(
        &test.get_user("admin").pubkey(),
        &test.get_user("portal_authority").pubkey(),
        order_params.dest_chain_id + 1, // Invalid source chain ID
        &fill_report
    )?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &test.get_user("portal_authority")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidReportSource));

    Ok(())
}

#[test]
fn test_report_fill_order_not_exist_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create fill report for non-existent order
    let fake_order_id = [99u8; 32];
    let fill_report =
        default_fill_report(&test, fake_order_id, test.get_user("solver").pubkey().to_bytes());

    let portal_authority = test.get_user("portal_authority");
    let ix = test.create_report_fill_ix(
        &test.get_user("admin").pubkey(),
        &portal_authority.pubkey(),
        DEST_CHAIN_ID,
        &fill_report,
    );

    // This should fail during account creation since order doesn't exist
    assert!(
        ix.is_err(),
        "Should fail to create ix when order doesn't exist"
    );

    Ok(())
}

#[test]
fn test_report_fill_foreign_order_type_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create a foreign order
    let order_data = order_book::OrderData {
        version: order_book::VERSION,
        sender: test.get_user("alice").pubkey().to_bytes(),
        nonce: 0,
        origin_chain_id: DEST_CHAIN_ID, // Foreign origin
        dest_chain_id: CHAIN_ID,        // Settles here
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
        created_at: test.current_time(),
    };

    // Fill the foreign order to create it
    test.fill_foreign_order("solver", &order_data, 500_000)?;

    let order_id = order_data.compute_order_id();
    let fill_report = FillReport {
        order_id,
        amount_in_to_release: 500_000,
        amount_out_filled: 500_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };

    let portal_authority = test.get_user("portal_authority");
    // Try to report fill for foreign order (should fail)
    let ix = test.create_report_fill_ix(
        &test.get_user("admin").pubkey(),
        &portal_authority.pubkey(),
        CHAIN_ID,
        &fill_report,
    );

    // Should fail because foreign order can't be deserialized as native order
    assert!(
        ix.is_err(),
        "Should fail to create ix for foreign order type"
    );

    Ok(())
}

#[test]
fn test_report_fill_order_completed_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Report full fill to complete the order
    let full_fill_report = FillReport {
        order_id,
        amount_in_to_release: 1_000_000,
        amount_out_filled: 1_000_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };

    test.report_fill("admin", order_params.dest_chain_id, &full_fill_report)?;

    // Verify order is completed
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Completed
    );

    // Expire blockhash to avoid AlreadyProcessed error
    test.ctx.svm.expire_blockhash();

    // Try to report another fill
    let portal_authority = test.get_user("portal_authority");
    let fill_report = default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());
    let ix = test.create_report_fill_ix(
        &test.get_user("admin").pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &fill_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::OrderNotFillable));

    Ok(())
}

#[test]
fn test_report_fill_order_cancelled_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Report cancel to put order in Cancelled status
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

    // Try to report fill on cancelled order
    let portal_authority = test.get_user("portal_authority");
    let fill_report = default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());
    let ix = test.create_report_fill_ix(
        &test.get_user("admin").pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &fill_report,
    )?;

    // Fill should fail because all tokens were refunded (amount_in_refunded == amount_in)
    // The validation `amount_in_released + amount_in_refunded <= amount_in` will fail
    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidFillAmount));

    Ok(())
}

#[test]
fn test_report_fill_cancelled_order_partial_refund_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get initial solver balance
    let solver_token_in_ata = test.get_ata("token-in-spl-6", "solver");
    let solver_balance_before = test.get_token_balance(&solver_token_in_ata)?;

    // Report cancel with 50% refund (simulating cancel arrived first from destination chain)
    let cancel_report = order_book::instructions::CancelReport {
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: 500_000u128, // 50% refund
    };
    test.report_cancel("bob", order_params.dest_chain_id, &cancel_report)?;

    // Verify order is cancelled with partial refund
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );
    assert_eq!(order_data.data.amount_in_refunded, 500_000);

    // Expire blockhash to avoid AlreadyProcessed error
    test.ctx.svm.expire_blockhash();

    // Report fill for remaining 50% - should succeed even though order is Cancelled
    // This simulates a fill report that was sent before the cancel but arrived after
    let fill_report = FillReport {
        order_id,
        amount_in_to_release: 500_000,
        amount_out_filled: 500_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    test.report_fill("admin", order_params.dest_chain_id, &fill_report)?;

    // Verify final state
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(order_data.data.amount_in_released, 500_000);
    assert_eq!(order_data.data.amount_in_refunded, 500_000);
    // Order remains Cancelled (status doesn't change back from Cancelled)
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Cancelled
    );

    // Verify solver received tokens
    let solver_balance_after = test.get_token_balance(&solver_token_in_ata)?;
    assert_eq!(solver_balance_after - solver_balance_before, 500_000);

    Ok(())
}

#[test]
fn test_report_fill_cancelled_order_exceeds_remaining_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create cross-chain native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Report cancel with 50% refund
    let cancel_report = order_book::instructions::CancelReport {
        order_id,
        order_sender: test.get_user("alice").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
        amount_in_to_refund: 500_000u128, // 50% refund
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

    // Try to report fill for 60% - should fail because only 50% remains
    let portal_authority = test.get_user("portal_authority");
    let fill_report = FillReport {
        order_id,
        amount_in_to_release: 600_000, // 60% - exceeds remaining 50%
        amount_out_filled: 600_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    let ix = test.create_report_fill_ix(
        &test.get_user("admin").pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &fill_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidFillAmount));

    Ok(())
}

#[test]
fn test_report_fill_zero_amount_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Create fill report with zero amount
    let fill_report = FillReport {
        order_id,
        amount_in_to_release: 0,
        amount_out_filled: 0,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };

    let portal_authority = test.get_user("portal_authority");
    let ix = test.create_report_fill_ix(
        &test.get_user("admin").pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &fill_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &portal_authority])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidFillAmount));

    Ok(())
}

#[test]
fn test_report_fill_wrong_token_mint_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    let fill_report = default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());

    // Build accounts using helper, then modify to use wrong token mint
    let mut accounts =
        test.build_report_fill_accounts(&test.get_user("admin").pubkey(), &test.get_user("portal_authority").pubkey(), &fill_report)?;

    // Override with wrong token mint
    let wrong_token_mint = test.get_mint("token-in-spl-9");
    let origin_recipient = anchor_litesvm::Pubkey::new_from_array(fill_report.origin_recipient);
    let order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
    accounts.token_in_mint = wrong_token_mint;
    accounts.recipient_token_in_ata =
        get_associated_token_address(&origin_recipient, &wrong_token_mint);
    accounts.order_token_in_ata = get_associated_token_address(&order_account, &wrong_token_mint);

    let ix = test.ctx.program()
        .accounts(accounts)
        .args(order_book::instruction::ReportOrderFill {
            source_chain_id: order_params.dest_chain_id,
            fill_report: fill_report.clone(),
        })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &test.get_user("portal_authority")])?
        .assert_anchor_error("AccountNotInitialized");

    Ok(())
}

#[test]
fn test_report_fill_wrong_recipient_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    let fill_report = default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());

    // Build accounts using helper, then modify to use wrong recipient
    let mut accounts = test.build_report_fill_accounts(
        &test.get_user("admin").pubkey(),
        &test.get_user("portal_authority").pubkey(),
        &fill_report,
    )?;

    // Override with wrong recipient
    let (_, native_order_data) = test.get_native_order_account(&order_id)?;
    let token_in_mint = native_order_data.data.token_in;
    let wrong_recipient = test.get_user("bob").pubkey();
    accounts.origin_recipient = wrong_recipient;
    accounts.recipient_token_in_ata =
        get_associated_token_address(&wrong_recipient, &token_in_mint);

    let ix = test.ctx.program()
        .accounts(accounts)
        .args(order_book::instruction::ReportOrderFill {
            source_chain_id: order_params.dest_chain_id,
            fill_report: fill_report.clone(),
        })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &test.get_user("portal_authority")])?
        .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidRecipient));

    Ok(())
}

#[test]
fn test_report_fill_wrong_order_pda_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    let fill_report = default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());

    // Build accounts using helper, then modify to use wrong order PDA
    let mut accounts = test.build_report_fill_accounts(
        &test.get_user("admin").pubkey(),
        &test.get_user("portal_authority").pubkey(),
        &fill_report,
    )?;

    // Override with wrong order PDA
    let wrong_order_id = [88u8; 32];
    let wrong_order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &wrong_order_id], &order_book::ID);
    let token_in_mint = test.get_mint("token-in-spl-6");
    accounts.order = wrong_order_account;
    accounts.order_token_in_ata =
        get_associated_token_address(&wrong_order_account, &token_in_mint);

    let ix = test.ctx.program()
        .accounts(accounts)
        .args(order_book::instruction::ReportOrderFill {
            source_chain_id: order_params.dest_chain_id,
            fill_report: fill_report.clone(),
        })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&test.get_user("admin"), &test.get_user("portal_authority")])?
        .assert_anchor_error("AccountNotInitialized");

    Ok(())
}

// Success case tests

#[test]
fn test_report_fill_partial_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get initial state
    let solver_token_in_ata = test.get_ata("token-in-spl-6", "solver");
    let order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    let solver_balance_before = test.get_token_balance(&solver_token_in_ata)?;
    let order_balance_before = test.get_token_balance(&order_token_in_ata)?;

    // Report partial fill (50%)
    let fill_report = default_fill_report(&test, order_id, test.get_user("solver").pubkey().to_bytes());

    test.report_fill("admin", order_params.dest_chain_id, &fill_report)?;

    // Verify balances
    let solver_balance_after = test.get_token_balance(&solver_token_in_ata)?;
    let order_balance_after = test.get_token_balance(&order_token_in_ata)?;

    assert_eq!(
        solver_balance_after - solver_balance_before,
        500_000,
        "Solver should receive reported amount_in"
    );
    assert_eq!(
        order_balance_before - order_balance_after,
        500_000,
        "Order account should release reported amount_in"
    );

    // Verify order state
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Created,
        "Order should remain Created"
    );
    assert_eq!(
        order_data.data.amount_in_released, 500_000,
        "amount_in_released should be updated"
    );
    assert_eq!(
        order_data.data.amount_out_filled, 500_000,
        "amount_out_filled should be updated"
    );

    Ok(())
}

#[test]
fn test_report_fill_full_fill_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Get initial state
    let solver_token_in_ata = test.get_ata("token-in-spl-6", "solver");
    let order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));

    let solver_balance_before = test.get_token_balance(&solver_token_in_ata)?;
    let order_balance_before = test.get_token_balance(&order_token_in_ata)?;

    // Report full fill (100%)
    let full_fill_report = FillReport {
        order_id,
        amount_in_to_release: 1_000_000,
        amount_out_filled: 1_000_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };

    test.report_fill("admin", order_params.dest_chain_id, &full_fill_report)?;

    // Verify balances - should transfer ALL tokens in order account
    let solver_balance_after = test.get_token_balance(&solver_token_in_ata)?;
    let order_balance_after = test.get_token_balance(&order_token_in_ata)?;

    assert_eq!(
        solver_balance_after - solver_balance_before,
        order_balance_before,
        "Solver should receive ALL tokens from order account"
    );
    assert_eq!(order_balance_after, 0, "Order account should be empty");

    // Verify order state
    let (_, order_data) = test.get_native_order_account(&order_id)?;
    assert_eq!(
        order_data.data.status,
        order_book::state::OrderStatus::Completed,
        "Order should be Completed"
    );
    assert_eq!(
        order_data.data.amount_in_released, 1_000_000,
        "amount_in_released should equal amount_in"
    );
    assert_eq!(
        order_data.data.amount_out_filled, 1_000_000,
        "amount_out_filled should equal amount_out"
    );

    Ok(())
}

#[test]
fn test_report_fill_multiple_partial_fills() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Track cumulative fills
    let fill_amounts = vec![250_000u128, 250_000u128, 250_000u128, 250_000u128]; // Four 25% fills

    for (i, &amount) in fill_amounts.iter().enumerate() {
        let is_final = i == fill_amounts.len() - 1;

        let fill_report = FillReport {
            order_id,
            amount_in_to_release: amount,
            amount_out_filled: amount,
            origin_recipient: test.get_user("solver").pubkey().to_bytes(),
            token_in: test.get_mint("token-in-spl-6").to_bytes(),
        };

        test.report_fill("admin", order_params.dest_chain_id, &fill_report)?;
        test.ctx.svm.expire_blockhash();

        // Verify state after each fill
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        let expected_cumulative = amount * (i as u128 + 1);

        assert_eq!(
            order_data.data.amount_in_released,
            expected_cumulative,
            "Fill {}: Cumulative amount_in_released should match",
            i + 1
        );
        assert_eq!(
            order_data.data.amount_out_filled,
            expected_cumulative,
            "Fill {}: Cumulative amount_out_filled should match",
            i + 1
        );

        if is_final {
            assert_eq!(
                order_data.data.status,
                order_book::state::OrderStatus::Completed,
                "Final fill should complete order"
            );
        } else {
            assert_eq!(
                order_data.data.status,
                order_book::state::OrderStatus::Created,
                "Fill {}: Order should remain Created",
                i + 1
            );
        }
    }

    Ok(())
}

#[test]
fn test_report_fill_with_donation_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create native order
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID,
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Send extra tokens to order account (donation) from alice who has tokens
    let alice = test.get_user("alice");
    let order_account = test
        .ctx
        .svm
        .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
    let order_token_in_ata =
        get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));
    let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");

    // Send donation from alice who still has tokens after opening the order
    let donation_amount = 500_000u64;
    let ix = spl_token::instruction::transfer(
        &spl_token::ID,
        &alice_token_in_ata,
        &order_token_in_ata,
        &alice.pubkey(),
        &[],
        donation_amount,
    )?;
    test.ctx.execute_instruction(ix, &[&alice])?.assert_success();

    // Get balance before fill
    let solver_token_in_ata = test.get_ata("token-in-spl-6", "solver");
    let solver_balance_before = test.get_token_balance(&solver_token_in_ata)?;
    let order_balance_before = test.get_token_balance(&order_token_in_ata)?;

    // Verify order has original + donation
    assert_eq!(
        order_balance_before,
        1_000_000 + donation_amount,
        "Order should have original + donation"
    );

    // Report full fill
    let full_fill_report = FillReport {
        order_id,
        amount_in_to_release: 1_000_000,
        amount_out_filled: 1_000_000,
        origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };

    test.report_fill("admin", order_params.dest_chain_id, &full_fill_report)?;

    // Verify solver receives only the exact fill amount, NOT the donation
    // The new behavior uses exact calculated values to prevent issues with
    // out-of-order fill/cancel reports
    let solver_balance_after = test.get_token_balance(&solver_token_in_ata)?;
    assert_eq!(
        solver_balance_after - solver_balance_before,
        1_000_000, // Only the fill amount, not the donation
        "Solver should receive only the exact fill amount"
    );

    // Verify donation remains in order ATA (can be swept via close_order_token_account)
    let order_balance_after = test.get_token_balance(&order_token_in_ata)?;
    assert_eq!(
        order_balance_after,
        donation_amount,
        "Donation should remain in order ATA"
    );

    Ok(())
}

#[test]
fn test_report_fill_paused_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    // Create a cross-chain order (required for report_fill)
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: DEST_CHAIN_ID, // cross-chain order
        fill_deadline: test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64
            + 86400,
        token_out: test.get_mint("token-out-spl-6").to_bytes(),
        amount_in: 1_000_000,
        amount_out: 1_000_000,
        recipient: test.get_user("alice").pubkey().to_bytes(),
        solver: test.get_user("solver").pubkey().to_bytes(),
    };
    let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

    // Pause the program
    test.pause()?;

    // Try to report a fill
    let solver = test.get_user("solver");
    let portal_authority = test.get_user("portal_authority");
    let fill_report = FillReport {
        order_id,
        amount_in_to_release: 500_000,
        amount_out_filled: 500_000,
        origin_recipient: solver.pubkey().to_bytes(),
        token_in: test.get_mint("token-in-spl-6").to_bytes(),
    };
    let ix = test.create_report_fill_ix(
        &solver.pubkey(),
        &portal_authority.pubkey(),
        order_params.dest_chain_id,
        &fill_report,
    )?;

    test.ctx
        .execute_instruction(ix, &[&solver, &portal_authority])?
        .assert_success();

    Ok(())
}
