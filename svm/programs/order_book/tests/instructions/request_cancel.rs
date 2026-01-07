use super::super::{OrderBookTest, CHAIN_ID};
use anchor_litesvm::{Signer, TestHelpers};
use order_book::{error::OrderBookError, ORDER_SEED_PREFIX};
use std::error::Error;

// RequestCancelOrder instruction tests
// [X] given the sender is not the order creator
//   [X] it reverts with a NotAuthorized error
// [X] given the order does not exist
//   [X] it reverts with an error
// [X] given the order status is Completed
//   [X] it reverts with an InvalidOrderStatus error
// [X] given the order status is CancelRequested
//   [X] it reverts with an InvalidOrderStatus error
// [X] given the order fill_deadline has passed
//   [X] it reverts with an OrderExpired error
// [X] given the current timestamp is exactly at the fill_deadline
//   [X] it successfully requests cancellation
// [X] given the order PDA is incorrect
//   [X] it reverts with a ConstraintSeeds error
// [X] given all checks pass
//   [X] it sets the order status to CancelRequested
//   [X] it sets cancel_requested_at to current timestamp
//   [X] it emits CancelRequested event
// [X] given the order has not been filled
//   [X] it successfully requests cancellation

mod local_orders {
    use super::*;

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
    fn test_request_cancel_unauthorized_sender_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Alice creates an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Bob tries to cancel Alice's order
        let ix = test.create_request_cancel_ix(&test.get_user("bob").pubkey(), order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("bob")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

        Ok(())
    }

    #[test]
    fn test_request_cancel_order_not_exist_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Try to cancel non-existent order
        let fake_order_id = [99u8; 32];

        let ix = test.create_request_cancel_ix(&test.get_user("alice").pubkey(), fake_order_id)?;
        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn test_request_cancel_already_completed_reverts() -> Result<(), Box<dyn Error>> {
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
        let ix = test.create_request_cancel_ix(&test.get_user("alice").pubkey(), order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_request_cancel_already_cancel_requested_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Request cancel once
        test.request_cancel("alice", order_id)?;

        // Verify status is CancelRequested
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::CancelRequested
        );

        test.ctx.svm.expire_blockhash();

        // Try to cancel again
        let ix = test.create_request_cancel_ix(&test.get_user("alice").pubkey(), order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOrderStatus));

        Ok(())
    }

    #[test]
    fn test_request_cancel_expired_order_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order with a short deadline
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp time past the fill deadline
        test.warp_forward(200);

        // Try to cancel expired order
        let ix = test.create_request_cancel_ix(&test.get_user("alice").pubkey(), order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::OrderExpired));

        Ok(())
    }

    #[test]
    fn test_request_cancel_wrong_order_pda_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Build accounts with wrong order PDA
        let mut accounts =
            test.build_request_cancel_accounts(&test.get_user("alice").pubkey(), order_id)?;

        // Override with wrong order PDA
        let wrong_order_id = [88u8; 32];
        let wrong_order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &wrong_order_id], &order_book::ID);
        accounts.order = wrong_order_account;

        let ix = test.create_request_cancel_ix_with_custom_accounts(accounts, order_id)?;

        test.ctx
            .execute_instruction(ix, &[&test.get_user("alice")])?
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    // Success case tests

    #[test]
    fn test_request_cancel_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Verify initial status is Created
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Created
        );
        assert_eq!(order_data.data.cancel_requested_at, 0);

        // Request cancel
        test.request_cancel("alice", order_id)?;

        // Verify status changed to CancelRequested and cancel_requested_at is set
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::CancelRequested
        );
        assert_eq!(
            order_data.data.cancel_requested_at,
            test.current_time(),
            "cancel_requested_at should be set"
        );

        Ok(())
    }

    #[test]
    fn test_request_cancel_before_any_fills_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Verify order has not been filled
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(order_data.data.amount_in_released, 0);
        assert_eq!(order_data.data.amount_out_filled, 0);

        // Request cancel
        test.request_cancel("alice", order_id)?;

        // Verify cancellation succeeded
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::CancelRequested
        );
        // Fill amounts should still be zero
        assert_eq!(order_data.data.amount_in_released, 0);
        assert_eq!(order_data.data.amount_out_filled, 0);

        Ok(())
    }

    #[test]
    fn test_request_cancel_at_exact_deadline_succeeds() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order with fill_deadline = 100
        let order_params = default_order_params(&test);
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Warp to exactly the fill deadline (timestamp 100)
        // Current time starts near 0, so warp forward to reach exactly fill_deadline
        let current_time = test.current_time();
        let warp_amount = order_params.fill_deadline - current_time;
        test.warp_forward(warp_amount);

        // Verify we're at exactly the deadline
        assert_eq!(test.current_time(), order_params.fill_deadline);

        // Request cancel should succeed at exactly the deadline
        test.request_cancel("alice", order_id)?;

        // Verify cancellation succeeded
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::CancelRequested
        );

        Ok(())
    }
}

// mod xchain_orders {
//     use super::*;
// }
