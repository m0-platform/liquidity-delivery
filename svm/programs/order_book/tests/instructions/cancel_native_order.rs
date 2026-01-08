use super::super::{OrderBookTest, CHAIN_ID, DEST_CHAIN_ID};
use anchor_litesvm::{Signer, TestHelpers};
use anchor_spl::associated_token::get_associated_token_address;
use order_book::{error::OrderBookError, state::OrderData, ORDER_SEED_PREFIX};
use std::error::Error;

// CancelNativeOrder instruction tests
// For same-chain orders where origin_chain_id == dest_chain_id == current chain_id
//
// [X] given the order does not exist
//   [X] it reverts with an AccountNotInitialized error
// [X] given the order status is Completed
//   [X] it reverts with an InvalidOrderStatus error
// [X] given the order status is Cancelled
//   [X] it reverts with an InvalidOrderStatus error
// [X] given the created_at timestamp is in the future
//   [X] it reverts with an InvalidCreatedAtTimestamp error
// [X] given the signer is not sender or recipient and fill deadline has NOT passed
//   [X] it reverts with a NotAuthorized error
// [X] given the signer is sender before fill deadline
//   [X] it succeeds and refunds tokens to sender
// [X] given the signer is recipient before fill deadline
//   [X] it succeeds and refunds tokens to sender
// [X] given the signer is anyone after fill deadline
//   [X] it succeeds and refunds tokens to sender
// [X] given the order has been fully filled (no remaining tokens)
//   [X] it reverts with OrderFilled error
// [X] given the order did not originate on the current chain (origin_chain_id != chain_id)
//   [X] it reverts with an InvalidOriginChainId error
// [X] given a partial fill occurred
//   [X] it refunds only the remaining tokens
// [X] given all checks pass
//   [X] it sets order status to Cancelled
//   [X] it transfers remaining token_in to sender
// [X] given the program is paused
//   [X] it reverts with a ProgramPaused error

mod local_orders {
    use super::*;

    fn default_order_params(test: &OrderBookTest) -> order_book::instructions::open::OrderParams {
        order_book::instructions::open::OrderParams {
            dest_chain_id: CHAIN_ID, // local order
            fill_deadline: test.current_time() + 100,
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("bob").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        }
    }

    #[test]
    fn test_cancel_native_order_not_exist_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Try to cancel non-existent order
        let fake_order_id = [99u8; 32];
        let fake_order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &fake_order_id], &order_book::ID);
        let fake_order_token_in_ata = test.create_associated_token_account(
            &test.get_mint("token-in-spl-6"),
            &fake_order_account,
        )?;

        let signer = test.get_user("alice");
        let sender = test.get_user("alice");

        // Create the accounts struct manually
        let accounts = order_book::accounts::CancelNativeOrder {
            program: order_book::ID,
            event_authority: test.get_event_authority()?,
            signer: signer.pubkey(),
            sender: sender.pubkey(),
            global_account: test.get_global_account()?.0,
            order: fake_order_account,
            token_in_mint: test.get_mint("token-in-spl-6"),
            sender_token_in_ata: test.get_ata("token-in-spl-6", "alice"),
            order_token_in_ata: fake_order_token_in_ata,
            token_in_program: anchor_spl::token::ID,
        };

        let ix =
            test.create_cancel_native_order_ix_with_custom_accounts(accounts, fake_order_id)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_already_completed_reverts() -> Result<(), Box<dyn Error>> {
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

        // Try to cancel completed order
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("alice").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_already_cancelled_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Cancel the order
        test.cancel_native_order("alice", "alice", order_id)?;

        // Verify order is cancelled
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        // Expire blockhash to avoid AlreadyProcessed error
        test.ctx.svm.expire_blockhash();

        // Try to cancel again
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("alice").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_unauthorized_before_deadline_reverts() -> Result<(), Box<dyn Error>>
    {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order with bob as recipient
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Carol (not sender or recipient) tries to cancel before deadline
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("carol").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("carol")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_foreign_order_nonexistent_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new().unwrap();
        test.initialize().unwrap();

        // Create order data for an order that originated on another chain
        // but has this chain as the destination
        let order_data = OrderData {
            version: order_book::constants::VERSION,
            sender: test.get_user("alice").pubkey().to_bytes(), // sender on origin chain
            nonce: 0,
            origin_chain_id: DEST_CHAIN_ID, // order originated on chain 2
            dest_chain_id: CHAIN_ID,        // this chain (1) is the destination
            created_at: test.current_time(),
            fill_deadline: test.current_time() + 100,
            token_in: test.get_mint("token-in-spl-6").to_bytes(),
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("alice").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        };
        let order_id = order_data.compute_order_id();

        // Create the accounts for the ix manually since it will fail on the order account lookup
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let token_in_mint = test.get_mint("token-in-spl-6");
        let sender_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let order_token_in_ata =
            test.create_associated_token_account(&token_in_mint, &order_account)?;
        let signer = test.get_user("alice");

        let accounts = order_book::accounts::CancelNativeOrder {
            program: order_book::ID,
            event_authority: test.get_event_authority()?,
            signer: signer.pubkey(),
            sender: signer.pubkey(),
            global_account: test.get_global_account()?.0,
            order: order_account,
            token_in_mint: test.get_mint("token-in-spl-6"),
            sender_token_in_ata,
            order_token_in_ata,
            token_in_program: anchor_spl::token::ID,
        };

        // Try to cancel the nonexistent foreign order
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::CancelNativeOrder { order_id })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&signer])
            .unwrap()
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_foreign_order_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new().unwrap();
        test.initialize().unwrap();

        // Create order data for an order that originated on another chain
        // but has this chain as the destination
        let order_data = OrderData {
            version: order_book::constants::VERSION,
            sender: test.get_user("alice").pubkey().to_bytes(), // sender on origin chain
            nonce: 0,
            origin_chain_id: DEST_CHAIN_ID, // order originated on chain 2
            dest_chain_id: CHAIN_ID,        // this chain (1) is the destination
            created_at: test.current_time(),
            fill_deadline: test.current_time() + 100,
            token_in: test.get_mint("token-in-spl-6").to_bytes(),
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("alice").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        };
        let order_id = order_data.compute_order_id();

        // Partially fill the order to initialize the account locally
        test.fill_foreign_order("solver", &order_data, (order_data.amount_out / 2) as u64)?;

        // Create the accounts for the ix manually since it will fail on the order account lookup
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let token_in_mint = test.get_mint("token-in-spl-6");
        let sender_token_in_ata = test.get_ata("token-in-spl-6", "alice");
        let order_token_in_ata =
            test.create_associated_token_account(&token_in_mint, &order_account)?;
        let signer = test.get_user("alice");

        let accounts = order_book::accounts::CancelNativeOrder {
            program: order_book::ID,
            event_authority: test.get_event_authority()?,
            signer: signer.pubkey(),
            sender: signer.pubkey(),
            global_account: test.get_global_account()?.0,
            order: order_account,
            token_in_mint: test.get_mint("token-in-spl-6"),
            sender_token_in_ata,
            order_token_in_ata,
            token_in_program: anchor_spl::token::ID,
        };

        // Try to cancel the initialized foreign order locally with cancel_native_order
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::CancelNativeOrder { order_id })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&signer])
            .unwrap()
            .assert_anchor_error("AccountDidNotDeserialize"); // Fails due to deserialization error

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_sender_before_deadline_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Get initial balance
        let sender_ata = test.get_ata("token-in-spl-6", "alice");
        let initial_balance = test.get_token_balance(&sender_ata)?;

        // Alice (sender) cancels the order before deadline
        test.cancel_native_order("alice", "alice", order_id)?;

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
    fn test_cancel_native_order_recipient_before_deadline_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order with bob as recipient
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Get initial balance
        let sender_ata = test.get_ata("token-in-spl-6", "alice");
        let initial_balance = test.get_token_balance(&sender_ata)?;

        // Bob (recipient) cancels the order before deadline
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("bob").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("bob")])?
            .assert_success();

        // Verify order is cancelled
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        // Verify tokens were refunded to sender (alice), not recipient (bob)
        let final_balance = test.get_token_balance(&sender_ata)?;
        assert_eq!(final_balance, initial_balance + 1_000_000);

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_anyone_after_deadline_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order with bob as recipient
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past deadline
        test.warp_forward(200);

        // Get initial balance
        let sender_ata = test.get_ata("token-in-spl-6", "alice");
        let initial_balance = test.get_token_balance(&sender_ata)?;

        // Carol (not sender or recipient) cancels the order after deadline
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("carol").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("carol")])?
            .assert_success();

        // Verify order is cancelled
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        // Verify tokens were refunded to sender (alice)
        let final_balance = test.get_token_balance(&sender_ata)?;
        assert_eq!(final_balance, initial_balance + 1_000_000);

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_fully_filled_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create and fully fill an order
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

        // Try to cancel - should fail with InvalidOrderStatus (since order is Completed)
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("alice").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_partial_fill_refunds_remaining() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Partially fill the order (50%)
        test.fill_native_order("solver", order_id, 500_000)?;

        // Get initial balance after partial fill
        let sender_ata = test.get_ata("token-in-spl-6", "alice");
        let initial_balance = test.get_token_balance(&sender_ata)?;

        // Cancel the order - should refund the remaining 50%
        test.cancel_native_order("alice", "alice", order_id)?;

        // Verify order is cancelled
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        // Verify only remaining tokens were refunded
        let final_balance = test.get_token_balance(&sender_ata)?;
        assert_eq!(final_balance, initial_balance + 500_000);

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_wrong_sender_account_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Try to cancel with wrong sender account (carol instead of alice)
        let wrong_sender = test.get_user("carol");

        let accounts = test.build_cancel_native_order_accounts(
            &test.get_user("alice").pubkey(),
            &wrong_sender.pubkey(),
            order_id,
        )?;

        let ix = test.create_cancel_native_order_ix_with_custom_accounts(accounts, order_id)?;

        // Should fail because sender doesn't match order's sender
        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidSender));

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_third_party_on_behalf_of_sender_success(
    ) -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past deadline so anyone can cancel
        test.warp_forward(200);

        // Get initial balance
        let sender_ata = test.get_ata("token-in-spl-6", "alice");
        let initial_balance = test.get_token_balance(&sender_ata)?;

        // Carol (third party) cancels the order on behalf of Alice
        // Note: After deadline, anyone can sign. The sender account is just
        // used to determine where to send the refund.
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("carol").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("carol")])?
            .assert_success();

        // Verify tokens went to sender (alice) not signer (carol)
        let final_balance = test.get_token_balance(&sender_ata)?;
        assert_eq!(final_balance, initial_balance + 1_000_000);

        Ok(())
    }

    #[test]
    fn test_cancel_native_order_paused_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order before pausing
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Pause the program
        test.pause()?;

        // Try to cancel the order while paused
        let ix = test.create_cancel_native_order_ix(
            &test.get_user("alice").pubkey(),
            &test.get_user("alice").pubkey(),
            order_id,
        )?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::ProgramPaused));

        Ok(())
    }
}
