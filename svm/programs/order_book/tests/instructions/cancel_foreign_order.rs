use super::super::{OrderBookTest, CHAIN_ID, DEST_CHAIN_ID};
use anchor_litesvm::Signer;
use order_book::{error::OrderBookError, state::OrderData};
use std::error::Error;

// CancelForeignOrder instruction tests
// For cross-chain orders where this chain is the destination (dest_chain_id == chain_id)
// but the order originated elsewhere (origin_chain_id != chain_id)
//
// [X] given the dest_chain_id does not match the current chain_id
//   [X] it reverts with InvalidDestChainId error
// [X] given the order_id does not match the computed order id from order_data
//   [X] it reverts with InvalidOrderId error
// [X] given the order status is Completed
//   [X] it reverts with InvalidOrderStatus error
// [X] given the order status is Cancelled
//   [X] it reverts with InvalidOrderStatus error
// [X] given the created_at timestamp is in the future
//   [X] it reverts with InvalidCreatedAtTimestamp error
// [X] given the signer is not recipient and fill deadline has NOT passed
//   [X] it reverts with NotAuthorized error
// [X] given the order originated on this chain (origin_chain_id == chain_id)
//   [X] it reverts with InvalidOriginChainId error 
// [X] given the signer is recipient before fill deadline
//   [X] it succeeds
// [X] given the signer is anyone after fill deadline
//   [X] it succeeds
// [X] given the order does not exist yet (DoesNotExist status)
//   [X] it creates the order account with Cancelled status
// [X] given the order exists and is Created
//   [X] it sets order status to Cancelled
// [X] given the program is paused
//   [X] it reverts with a ProgramPaused error

mod xchain_orders {
    use order_book::OrderParams;

    use super::*;

    fn create_foreign_order_data(test: &OrderBookTest) -> OrderData {
        // Create order data for an order that originated on another chain
        // but has this chain as the destination
        OrderData {
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
            recipient: test.get_user("bob").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        }
    }

    #[test]
    fn test_cancel_foreign_order_invalid_dest_chain_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create order data with wrong destination chain
        let mut order_data = create_foreign_order_data(&test);
        order_data.dest_chain_id = 999; // Not this chain

        let signer = test.get_user("bob"); // recipient

        let ix = test.create_cancel_foreign_order_ix(&signer.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidDestChainId));

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_invalid_order_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let wrong_order_id = [99u8; 32]; // Wrong order ID

        let signer = test.get_user("bob"); // recipient

        // Build accounts with wrong order_id
        let accounts = test.build_cancel_foreign_order_accounts(&signer.pubkey(), &order_data)?;

        let ix = test.create_cancel_foreign_order_ix_with_custom_accounts(
            accounts,
            wrong_order_id,
            order_data,
        )?;

        // Anchor's seeds constraint validation happens before custom validation,
        // so we get ConstraintSeeds instead of InvalidOrderId
        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error("ConstraintSeeds");

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_already_cancelled_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let signer = test.get_user("bob"); // recipient

        // First cancel - should succeed
        test.cancel_foreign_order("bob", &order_data)?;

        // Verify order is cancelled
        let order_id = order_data.compute_order_id();
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        // Expire blockhash to avoid AlreadyProcessed error
        test.ctx.svm.expire_blockhash();

        // Try to cancel again - should fail
        let ix = test.create_cancel_foreign_order_ix(&signer.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_unauthorized_before_deadline_reverts() -> Result<(), Box<dyn Error>>
    {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);

        // Carol (not recipient) tries to cancel before deadline
        let carol = test.get_user("carol");
        let ix = test.create_cancel_foreign_order_ix(&carol.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&carol])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_on_non_existant_native_order_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create order data with origin_chain_id == chain_id
        let mut order_data = create_foreign_order_data(&test);
        order_data.origin_chain_id = CHAIN_ID; // Originated on this chain

        let signer = test.get_user("bob"); // recipient

        let ix = test.create_cancel_foreign_order_ix(&signer.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOriginChainId));

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_on_native_order_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create a same chain order (origin_chain_id == chain_id)
        let order_params = OrderParams {
            dest_chain_id: CHAIN_ID,
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("alice").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
            fill_deadline: test.current_time() + 100,
        };
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let (_, native_order) = test.get_native_order_account(&order_id)?;
        let order_data = OrderData::new_from_native_order(native_order.data, CHAIN_ID);

        let signer = test.get_user("alice"); // recipient
        let ix = test.create_cancel_foreign_order_ix(&signer.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_failure(); // Fails due to deserialization error, but it's good to have the check in case the account changes later

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_recipient_before_deadline_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let order_id = order_data.compute_order_id();

        // Bob (recipient) cancels before deadline
        test.cancel_foreign_order("bob", &order_data)?;

        // Verify order is cancelled
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_anyone_after_deadline_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let order_id = order_data.compute_order_id();

        // Warp time past deadline
        test.warp_forward(200);

        // Carol (anyone) can cancel after deadline
        let carol = test.get_user("carol");
        let ix = test.create_cancel_foreign_order_ix(&carol.pubkey(), &order_data)?;

        test.ctx.execute_instruction(ix, &[&carol])?.assert_success();

        // Verify order is cancelled
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_creates_account_if_not_exist() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let order_id = order_data.compute_order_id();

        // Verify order doesn't exist yet
        let result = test.get_foreign_order_account(&order_id);
        assert!(result.is_err());

        // Cancel the order - this should create it with Cancelled status
        test.cancel_foreign_order("bob", &order_data)?;

        // Verify order was created with Cancelled status
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_existing_order_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let order_id = order_data.compute_order_id();

        // First, partially fill the order to create it with Created status
        test.fill_foreign_order("solver", &order_data, 500_000)?;

        // Verify order exists with Created status
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Created
        );

        // Expire blockhash to avoid AlreadyProcessed error
        test.ctx.svm.expire_blockhash();

        // Now cancel the order
        test.cancel_foreign_order("bob", &order_data)?;

        // Verify order is now Cancelled
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_partial_fill_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let order_id = order_data.compute_order_id();

        // Partially fill the order
        test.fill_foreign_order("solver", &order_data, 300_000)?;

        // Verify order is partially filled
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Created
        );
        assert_eq!(foreign_order.data.amount_out_filled, 300_000);

        // Expire blockhash to avoid AlreadyProcessed error
        test.ctx.svm.expire_blockhash();

        // Cancel should still work for partially filled orders
        test.cancel_foreign_order("bob", &order_data)?;

        // Verify order is cancelled
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Cancelled
        );

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_completed_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_data = create_foreign_order_data(&test);
        let order_id = order_data.compute_order_id();

        // Fully fill the order
        test.fill_foreign_order("solver", &order_data, 1_000_000)?;

        // Verify order is completed
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.status,
            order_book::state::OrderStatus::Completed
        );

        // Try to cancel completed order
        let signer = test.get_user("bob");
        let ix = test.create_cancel_foreign_order_ix(&signer.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_created_at_future_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create order data with future created_at
        let mut order_data = create_foreign_order_data(&test);
        order_data.created_at = test.current_time() + 1000; // In the future

        let signer = test.get_user("bob"); // recipient
        let ix = test.create_cancel_foreign_order_ix(&signer.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidCreatedAtTimestamp));

        Ok(())
    }

    #[test]
    fn test_cancel_foreign_order_paused_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Pause the program
        test.pause()?;

        // Try to cancel a foreign order while paused
        let order_data = create_foreign_order_data(&test);
        let signer = test.get_user("bob"); // recipient

        let ix = test.create_cancel_foreign_order_ix(&signer.pubkey(), &order_data)?;

        test.ctx
            .execute_instruction(ix, &[&signer])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::ProgramPaused));

        Ok(())
    }
}
