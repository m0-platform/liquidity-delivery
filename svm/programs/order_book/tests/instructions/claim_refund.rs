use super::super::{OrderBookTest, CHAIN_ID, DEST_CHAIN_ID};
use anchor_litesvm::{Signer, TestHelpers};
use anchor_spl::associated_token::get_associated_token_address;
use order_book::{error::OrderBookError, ORDER_SEED_PREFIX};
use std::error::Error;

mod local_orders {
    use super::*;

    // claim_refund instruction tests for local orders (dest_chain_id == chain_id)
    // [X] given the sender is not the order creator
    //   [X] it reverts with a NotAuthorized error
    // [X] given the order does not exist
    //   [X] it reverts with an error
    // [X] given the order status is Completed
    //   [X] it reverts with an InvalidOrderStatus error
    // [X] given the token_in_mint is incorrect
    //   [X] it reverts with an InvalidTokenMint error
    // [X] given the sender_token_in_ata is incorrect
    //   [X] it reverts with a constraint error
    // [X] given the order PDA is incorrect
    //   [X] it reverts with a ConstraintSeeds error
    // [X] given the order was partially filled
    //   [X] it refunds only the remaining tokens
    //   [X] it completes the order
    // [X] given the order expired
    //   [X] it sets order status to Completed
    //   [X] it transfers all remaining tokens to sender
    // [ ] given the order was canceled
    //   [X] it sets order status to Completed
    //   [X] it transfers all remaining tokens to sender
    // [X] given a third party claims refund on behalf of sender
    //   [X] it succeeds (sender doesn't need to sign)
    //   [X] tokens go to sender's ATA

    fn default_order_params(test: &OrderBookTest) -> order_book::instructions::open::OrderParams {
        order_book::instructions::open::OrderParams {
            dest_chain_id: CHAIN_ID, // local order
            fill_deadline: 100,
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("alice").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        }
    }

    #[test]
    fn test_claim_refund_invalid_sender_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past fill_deadline
        test.warp_forward(200);

        // Carol tries to claim Alice's refund with his own account
        let wrong_sender = test.get_user("carol");
        let accounts = test.build_claim_refund_accounts(&wrong_sender.pubkey(), order_id)?;

        let ix = test.create_claim_refund_ix_with_custom_accounts(accounts, order_id)?;

        test.ctx
            .execute_instruction(ix, &[&wrong_sender])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

        Ok(())
    }

    #[test]
    fn test_claim_refund_order_does_not_exist_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Try to claim refund for non-existent order
        let fake_order_id = [99u8; 32];
        let fake_order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &fake_order_id], &order_book::ID);
        let fake_order_token_in_ata = test.create_associated_token_account(
            &test.get_mint("token-in-spl-6"),
            &fake_order_account,
        )?;

        let sender = test.get_user("alice");

        // Create the accounts struct manually
        let accounts = order_book::accounts::ClaimRefund {
            program: order_book::ID,
            event_authority: test.get_event_authority()?,
            sender: sender.pubkey(),
            global_account: test.get_global_account()?.0,
            destination_account: None,
            order: fake_order_account,
            token_in_mint: test.get_mint("token-in-spl-6"),
            sender_token_in_ata: test.get_ata("token-in-spl-6", "alice"),
            order_token_in_ata: fake_order_token_in_ata,
            token_in_program: anchor_spl::token::ID,
        };

        // Create the instruction
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::ClaimRefund {
                order_id: fake_order_id,
            })
            .instruction()?;

        // Execute and expect failure
        test.ctx
            .execute_instruction(ix, &[&sender])?
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn test_claim_refund_order_completed_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create and fill an order completely
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Fill the order completely
        test.fill_native_order("solver", order_id, 1_000_000)?;

        // Verify order is completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        // Try to claim refund on completed order
        let ix = test.create_claim_refund_ix(&test.get_user("alice").pubkey(), order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_claim_refund_wrong_token_mint_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past fill_deadline
        test.warp_forward(200);

        // Build accounts with wrong token mint
        let mut accounts =
            test.build_claim_refund_accounts(&test.get_user("alice").pubkey(), order_id)?;

        // Override with wrong mint
        let wrong_token_mint = test.get_mint("token-in-spl-9");
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let order_token_in_ata =
            test.create_associated_token_account(&wrong_token_mint, &order_account)?;
        accounts.token_in_mint = wrong_token_mint;
        accounts.sender_token_in_ata =
            get_associated_token_address(&test.get_user("alice").pubkey(), &wrong_token_mint);
        accounts.order_token_in_ata = order_token_in_ata;

        let ix = test.create_claim_refund_ix_with_custom_accounts(accounts, order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidTokenMint));

        Ok(())
    }

    #[test]
    fn test_claim_refund_wrong_sender_ata_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past fill_deadline
        test.warp_forward(200);

        // Build accounts with wrong sender ATA
        let mut accounts =
            test.build_claim_refund_accounts(&test.get_user("alice").pubkey(), order_id)?;

        // Override with carols's ATA (wrong owner)
        accounts.sender_token_in_ata =
            get_associated_token_address(&test.get_user("carol").pubkey(), &accounts.token_in_mint);

        let ix = test.create_claim_refund_ix_with_custom_accounts(accounts, order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("carol")])?
            .assert_anchor_error("ConstraintTokenOwner");

        Ok(())
    }

    #[test]
    fn test_claim_refund_wrong_order_pda_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past fill_deadline
        test.warp_forward(200);

        // Build accounts with wrong order PDA
        let mut accounts =
            test.build_claim_refund_accounts(&test.get_user("alice").pubkey(), order_id)?;

        // Override with wrong order PDA
        let wrong_order_id = [88u8; 32];
        let wrong_order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &wrong_order_id], &order_book::ID);
        accounts.order = wrong_order_account;

        let ix = test.create_claim_refund_ix_with_custom_accounts(accounts, order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    // Success case tests

    #[test]
    fn test_claim_refund_expired_order_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Get initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Warp time past fill_deadline
        test.warp_forward(200);

        // Claim refund
        test.claim_refund("alice", order_id)?;

        // Verify balance increased by full amount
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;
        assert_eq!(
            alice_balance_after - alice_balance_before,
            1_000_000,
            "Alice should receive full refund"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }

    #[test]
    fn test_claim_refund_canceled_order_combined_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Get initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Request cancel and claim refund in the same transaction
        let sender_keypair = test.get_user("alice");
        let ix_cancel = test.create_request_cancel_ix(&sender_keypair.pubkey(), order_id)?;
        let ix_refund = test.create_claim_refund_ix(&sender_keypair.pubkey(), order_id)?;

        test.ctx.execute_instructions(vec![ix_cancel, ix_refund], &[&sender_keypair])?
            .assert_success();

        // Verify balance increased by full amount
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;
        assert_eq!(
            alice_balance_after - alice_balance_before,
            1_000_000,
            "Alice should receive full refund"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }

    #[test]
    fn test_claim_refund_partial_fill_refund_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Fill the order partially (50%)
        test.fill_native_order("solver", order_id, 500_000)?;

        // Verify order has remaining tokens
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let order_token_in_ata =
            get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));
        let order_balance = test.get_token_balance(&order_token_in_ata)?;
        assert_eq!(order_balance, 500_000, "Order should have 50% remaining");

        // Get initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Warp time past fill_deadline
        test.warp_forward(200);

        // Claim refund
        test.claim_refund("alice", order_id)?;

        // Verify balance increased by remaining amount only
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;
        assert_eq!(
            alice_balance_after - alice_balance_before,
            500_000,
            "Alice should receive refund for remaining 50%"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }

    #[test]
    fn test_claim_refund_by_third_party_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Get Alice's initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Warp time past fill_deadline
        test.warp_forward(200);

        // Bob claims refund on behalf of Alice (sender doesn't need to sign)
        let ix = test.create_claim_refund_ix(&test.get_user("alice").pubkey(), order_id)?;
        test.ctx.execute_instruction(ix, &[&test.get_user("bob")])?;

        // Verify Alice's balance increased (not Bob's)
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;
        assert_eq!(
            alice_balance_after - alice_balance_before,
            1_000_000,
            "Alice should receive refund even though Bob executed"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }
}

mod xchain_orders {
    use super::*;

    // claim_refund instruction tests for xchain orders
    // [X] given the order status is Created and finality buffer has not passed
    //   [X] it reverts with a FinalityPending error
    // [X] given the order status is CancelRequested and finality buffer has not passed
    //   [X] it reverts with a FinalityPending error
    // [X] given destination account is missing
    //   [X] it reverts with a DestinationAccountRequired error
    // [X] given the order fill_deadline + finality buffer has passed (Created status)
    //   [X] it sets order status to Completed
    //   [X] it transfers all remaining tokens to sender
    //   [X] it emits RefundClaimed event
    // [X] given the order cancel_requested_at + finality buffer has passed
    //   [X] it successfully claims refund
    //   [X] it transfers correct amount
    // [X] given the order was partially filled
    //   [X] it refunds only the remaining tokens
    //   [X] it completes the order
    // [X] given the sender claims refund
    //   [X] it succeeds
    //   [X] tokens go to sender's ATA
    // [X] given a third party claims refund on behalf of sender
    //   [X] it succeeds (sender doesn't need to sign)
    //   [X] tokens go to sender's ATA

    fn default_order_params(test: &OrderBookTest) -> order_book::instructions::open::OrderParams {
        order_book::instructions::open::OrderParams {
            dest_chain_id: DEST_CHAIN_ID, // xchain order
            fill_deadline: 100,
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("alice").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        }
    }

    #[test]
    fn test_claim_refund_finality_pending_created_status_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let (_, destination) = test.get_destination_account(DEST_CHAIN_ID)?;
        println!("Destination config: {:?}", destination);

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        let (_, native_order) = test.get_native_order_account(&order_id)?;

        // Warp time past fill_deadline but NOT past fill_deadline + finality buffer (which is 1000)
        test.warp_forward(200);

        // Try to claim refund before finality buffer passes
        let ix = test.create_claim_refund_ix(&test.get_user("alice").pubkey(), order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::FinalityPending));

        Ok(())
    }

    #[test]
    fn test_claim_refund_finality_pending_cancel_requested_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Request cancel
        test.request_cancel("alice", order_id)?;

        // Warp time a bit but NOT past finality buffer
        test.warp_forward(50);

        // Try to claim refund before finality buffer passes
        let ix = test.create_claim_refund_ix(&test.get_user("alice").pubkey(), order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::FinalityPending));

        Ok(())
    }

    #[test]
    fn test_claim_refund_missing_destination_account_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order with foreign destination
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past fill_deadline + finality buffer
        let (_, dest_data) = test.get_destination_account(DEST_CHAIN_ID)?;
        test.warp_forward(200 + dest_data.effective_finality_buffer(test.current_time()));

        // Build accounts but remove destination account
        let mut accounts =
            test.build_claim_refund_accounts(&test.get_user("alice").pubkey(), order_id)?;
        accounts.destination_account = None;

        let ix = test.create_claim_refund_ix_with_custom_accounts(accounts, order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::DestinationAccountRequired));

        Ok(())
    }

    #[test]
    fn test_claim_refund_expired_order_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        test.ctx.svm.expire_blockhash();

        // Get initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Warp time past fill_deadline + finality buffer
        let (_, dest_data) = test.get_destination_account(DEST_CHAIN_ID)?;
        test.warp_forward(200 + dest_data.effective_finality_buffer(test.current_time()));

        // Claim refund
        test.claim_refund("alice", order_id)?;

        // Verify balance increased by full amount
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;

        assert_eq!(
            alice_balance_after - alice_balance_before,
            1_000_000,
            "Alice should receive full refund"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }

    #[test]
    fn test_claim_refund_after_cancel_request_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Request cancel
        test.request_cancel("alice", order_id)?;

        // Get initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Warp time past cancel_requested_at + finality buffer
        let (_, dest_data) = test.get_destination_account(DEST_CHAIN_ID)?;
        test.warp_forward(10 + dest_data.effective_finality_buffer(test.current_time()));

        // Claim refund
        test.claim_refund("alice", order_id)?;

        // Verify balance increased
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;
        assert_eq!(
            alice_balance_after - alice_balance_before,
            1_000_000,
            "Alice should receive full refund"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }

    #[test]
    fn test_claim_refund_partial_fill_refund_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Report a partial fill (50%)
        let fill_report = order_book::FillReport {
            order_id,
            amount_in_to_release: 500_000,
            amount_out_filled: 500_000,
            origin_recipient: test.get_user("solver").pubkey().to_bytes(),
            token_in: test.get_mint("token-in-spl-6").to_bytes(),
        };
        test.report_fill("admin", &fill_report)?;

        // Verify order has remaining tokens
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let order_token_in_ata =
            get_associated_token_address(&order_account, &test.get_mint("token-in-spl-6"));
        let order_balance = test.get_token_balance(&order_token_in_ata)?;
        assert_eq!(order_balance, 500_000, "Order should have 50% remaining");

        // Get initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Warp time past fill_deadline + finality buffer
        let (_, dest_data) = test.get_destination_account(DEST_CHAIN_ID)?;
        test.warp_forward(200 + dest_data.effective_finality_buffer(test.current_time()));

        // Claim refund
        test.claim_refund("alice", order_id)?;

        // Verify balance increased by remaining amount only
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;
        assert_eq!(
            alice_balance_after - alice_balance_before,
            500_000,
            "Alice should receive refund for remaining 50%"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }

    #[test]
    fn test_claim_refund_by_third_party_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order
        let current_time = test
            .ctx
            .svm
            .get_sysvar::<anchor_lang::prelude::Clock>()
            .unix_timestamp as u64;
        let order_params = order_book::instructions::open::OrderParams {
            dest_chain_id: DEST_CHAIN_ID,
            fill_deadline: current_time + 100,
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("alice").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        };
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Get Alice's initial balance
        let alice_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let alice_balance_before = test.get_token_balance(&alice_token_in_ata)?;

        // Warp time past fill_deadline + finality buffer
        let (_, dest_data) = test.get_destination_account(DEST_CHAIN_ID)?;
        test.warp_forward(200 + dest_data.effective_finality_buffer(test.current_time()));

        // Bob claims refund on behalf of Alice (sender doesn't need to sign)
        let ix = test.create_claim_refund_ix(&test.get_user("alice").pubkey(), order_id)?;
        test.ctx.execute_instruction(ix, &[&test.get_user("bob")])?;

        // Verify Alice's balance increased (not Bob's)
        let alice_balance_after = test.get_token_balance(&alice_token_in_ata)?;
        assert_eq!(
            alice_balance_after - alice_balance_before,
            1_000_000,
            "Alice should receive refund even though Bob executed"
        );

        // Verify order status is Completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        Ok(())
    }
}
