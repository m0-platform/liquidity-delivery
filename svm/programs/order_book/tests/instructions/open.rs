use super::super::{INITIAL_FUNDS, OrderBookTest};
use anchor_litesvm::{AssertionHelpers, TestHelpers, Signer};
use anchor_lang::prelude::Clock;
use anchor_spl::associated_token::get_associated_token_address;
use std::error::Error;

use order_book::error::OrderBookError;

mod local_orders {
    // Local order test cases
    // [X] given the amount in is zero 
    //   [X] it reverts with an InvalidAmountIn error
    // [X] given the amount out is zero
    //   [X] it reverts with an InvalidAmountOut error
    // [ ] given the fill deadline is before the current slot timestamp
    //   [ ] it reverts with an InvalidDeadline error
    // [ ] given the sender is not the owner of (or not delegated to manage) the token in account
    //   [ ] it reverts with a ? error
    // [ ] given the sender 

    use super::*;

    fn default_order_params(test: &OrderBookTest, sender: &str) -> order_book::instructions::open::OrderParams {
        order_book::instructions::open::OrderParams {
            dest_chain_id: test.get_global_account().unwrap().1.chain_id, // local order
            fill_deadline: test.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64 + 86400,
            token_out: test.mints.get("token-out-spl-6").unwrap().to_bytes(),
            amount_in: 1_000_000, 
            amount_out: 1_000_000,
            recipient: test.users.get(sender).unwrap().pubkey().to_bytes(),
            solver: test.users.get("solver").unwrap().pubkey().to_bytes(),
        }
    }

    #[test]
    fn test_local_order_amount_in_zero_reverts() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let alice = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let sender_token_in_account = test.atas.get(&("token-in-spl-6", "alice")).unwrap();

        // Prepare order parameters with amount_in set to zero
        let mut order_params = default_order_params(&test, "alice");
        order_params.amount_in = 0u64;

        let (_, ix) = test.create_open_order_ix(
            &alice.pubkey(),
            &token_in_mint,
            &sender_token_in_account,
            None,
            &order_params,
        )?;

        test.ctx.execute_instruction(ix, &[alice])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidAmountIn));    

        Ok(())
    }

    #[test]
    fn test_local_order_amount_out_zero_reverts() -> Result<(), Box<dyn Error>> { 
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let alice = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let sender_token_in_account = test.atas.get(&("token-in-spl-6", "alice")).unwrap();

        // Prepare order parameters with amount_in set to zero
        let mut order_params = default_order_params(&test, "alice");
        order_params.amount_out = 0;

        let (_, ix) = test.create_open_order_ix(
            &alice.pubkey(),
            &token_in_mint,
            &sender_token_in_account,
            None,
            &order_params,
        )?;

        test.ctx.execute_instruction(ix, &[alice])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidAmountOut));

        Ok(())
    }

    #[test]
    fn test_fill_deadline_before_current_time_reverts() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Warp to a future timestamp to ensure the fill deadline is in the past
        let mut clock = test.ctx.svm.get_sysvar::<Clock>();
        clock.unix_timestamp += 200;
        test.ctx.svm.set_sysvar(&clock);

        let alice = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let sender_token_in_account = test.atas.get(&("token-in-spl-6", "alice")).unwrap();

        // Prepare order parameters with fill deadline in the past
        let mut order_params = default_order_params(&test, "alice");
        order_params.fill_deadline = test.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64 - 1;

        let (_, ix) = test.create_open_order_ix(
            &alice.pubkey(),
            &token_in_mint,
            &sender_token_in_account,
            None,
            &order_params,
        )?;

        test.ctx.execute_instruction(ix, &[alice])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidFillDeadline));

        Ok(())
    }

    #[test]
    fn test_sender_not_authorized_to_spend_token_in_reverts() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let alice = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let other_token_in_account = test.atas.get(&("token-in-spl-6", "solver")).unwrap();

        // Prepare order parameters
        let order_params = default_order_params(&test, "alice");

        let (_, ix) = test.create_open_order_ix(
            &alice.pubkey(),
            &token_in_mint,
            &other_token_in_account,
            None,
            &order_params,
        )?;

        test.ctx.execute_instruction(ix, &[alice])?
            .assert_failure();

        Ok(()) 
    }

    #[test]
    fn test_sender_ata_does_not_exist_reverts() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let bob = test.users.get("bob").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let non_existent_ata = get_associated_token_address(&bob.pubkey(), &token_in_mint);

        // Prepare order parameters
        let order_params = default_order_params(&test, "bob");

        let (_, ix) = test.create_open_order_ix(
            &bob.pubkey(),
            &token_in_mint,
            &non_existent_ata,
            None,
            &order_params,
        )?;

        test.ctx.execute_instruction(ix, &[bob])?
            .assert_failure();

        Ok(()) 
    }

    #[test]
    fn test_sender_does_not_have_enough_token_in_reverts() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let sender = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let sender_token_in_account = test.atas.get(&("token-in-spl-6", "alice")).unwrap();

        // Prepare order parameters with amount_in greater than balance
        let mut order_params = default_order_params(&test, "alice");
        order_params.amount_in = 10 * INITIAL_FUNDS; // more than the initial funds

        let (_, ix) = test.create_open_order_ix(
            &sender.pubkey(),
            &token_in_mint,
            &sender_token_in_account,
            None,
            &order_params,
        )?;

        test.ctx.execute_instruction(ix, &[sender])?
            .assert_log_error("insufficient funds");

        Ok(()) 
    }

    #[test]
    fn test_sender_token_in_account_wrong_mint_reverts() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let sender = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let wrong_token_in_account = test.atas.get(&("token-in-spl-9", "alice")).unwrap();

        // Prepare order parameters with amount_in greater than balance
        let mut order_params = default_order_params(&test, "alice");
        order_params.amount_in = 10 * INITIAL_FUNDS; // more than the initial funds

        let (_, ix) = test.create_open_order_ix(
            &sender.pubkey(),
            &token_in_mint,
            &wrong_token_in_account,
            None,
            &order_params,
        )?;

        test.ctx.execute_instruction(ix, &[sender])?
            .assert_anchor_error("TokenMint");

        Ok(())  
    }

    #[test]
    fn test_success() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let sender = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let sender_token_in_account = test.atas.get(&("token-in-spl-6", "alice")).unwrap();

        // Prepare order parameters with amount_in greater than balance
        let order_params = default_order_params(&test, "alice");

        let (order_id, ix) = test.create_open_order_ix(
            &sender.pubkey(),
            &token_in_mint,
            &sender_token_in_account,
            None,
            &order_params,
        )?;

        // Cache the starting balance of the sender's token in account
        let starting_balance = test.get_token_balance(&sender_token_in_account)?;

        // Open the order
        test.ctx.execute_instruction(ix, &[sender])?
            .assert_success();

        // Verify the order account was created with correct data
        let (order_account, order) = test.get_native_order_account(&order_id)?;

        assert_eq!(order.data.sender, sender.pubkey());
        assert_eq!(order.data.nonce, 0);
        assert_eq!(order.data.dest_chain_id, order_params.dest_chain_id);
        assert_eq!(order.data.fill_deadline, order_params.fill_deadline);
        assert_eq!(order.data.token_out, order_params.token_out);
        assert_eq!(order.data.amount_in, order_params.amount_in as u128);
        assert_eq!(order.data.amount_out, order_params.amount_out);
        assert_eq!(order.data.recipient, order_params.recipient);
        assert_eq!(order.data.solver, order_params.solver);

        // Verify the sender's token in account balance decreased by amount_in
        // and the order's token in account balance increased by amount_in
        assert_eq!(
            test.get_token_balance(&sender_token_in_account)?,
            starting_balance - order_params.amount_in
        );
        assert_eq!(
            test.get_token_balance(&get_associated_token_address(&order_account, token_in_mint))?,
            order_params.amount_in
        );

        Ok(()) 
    }

    #[test]
    fn test_success_with_delegated_token_authority() -> Result<(), Box<dyn Error>> {
        // Setup test environment
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Approve the delegated authority to spend tokens on behalf of the sender
        test.approve_token_delegate(
            "token-in-spl-6",
            "alice",
            "bob",
            1_000_000,
        )?;

        // Setup the instruction
        let sender = test.users.get("alice").unwrap();
        let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
        let sender_token_in_account = test.atas.get(&("token-in-spl-6", "alice")).unwrap();
        let delegated_authority = test.users.get("bob").unwrap();

        // Prepare order parameters with amount_in less than or equal to delegated amount
        let order_params = default_order_params(&test, "alice");

        let (order_id, ix) = test.create_open_order_ix(
            &sender.pubkey(),
            &token_in_mint,
            &sender_token_in_account,
            Some(&delegated_authority.pubkey()),
            &order_params,
        )?;

        // Cache the starting balance of the sender's token in account
        let starting_balance = test.get_token_balance(&sender_token_in_account)?;

        // Open the order using the delegated authority
        test.ctx.execute_instruction(ix, &[&sender, &delegated_authority])?
            .assert_success();

        // Verify the order account was created with correct data
        let (order_account, order) = test.get_native_order_account(&order_id)?;
        assert_eq!(order.data.sender, sender.pubkey());
        assert_eq!(order.data.nonce, 0);
        assert_eq!(order.data.dest_chain_id, order_params.dest_chain_id);
        assert_eq!(order.data.fill_deadline, order_params.fill_deadline);
        assert_eq!(order.data.token_out, order_params.token_out);
        assert_eq!(order.data.amount_in, order_params.amount_in as u128);
        assert_eq!(order.data.amount_out, order_params.amount_out);
        assert_eq!(order.data.recipient, order_params.recipient);
        assert_eq!(order.data.solver, order_params.solver);

        // Verify the sender's token in account balance decreased by amount_in
        // and the order's token in account balance increased by amount_in
        assert_eq!(
            test.get_token_balance(&sender_token_in_account)?,
            starting_balance - order_params.amount_in
        );
        assert_eq!(
            test.get_token_balance(&get_associated_token_address(&order_account, token_in_mint))?,
            order_params.amount_in
        );

        Ok(()) 
    }

}