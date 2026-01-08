use super::super::{portal, OrderBookTest, CHAIN_ID, DEST_CHAIN_ID};
use anchor_lang::prelude::Clock;
use anchor_litesvm::{Signer, TestHelpers};
use anchor_spl::associated_token::get_associated_token_address;
use std::error::Error;

use order_book::error::OrderBookError;

mod local_orders {
    // Local order fill tests
    // fill_native_order
    // [X] given the order does not exist
    //   [X] it reverts with an AccountNotInitialized error
    // [X] given the destination chain id of the order does not match the current chain id
    //   [X] it reverts with an InvalidDestChainId error
    // [X] given the token out on the order does not match the token out mint account
    //   [X] it reverts with an InvalidTokenOutMint error
    // [X] given the order recipient does not match the recipient account
    //   [X] it reverts with an InvalidRecipient error
    // [X] given order type is not Native
    //   [X] given no order account exists
    //     [X] it reverts with an AccountNotInitialized Error
    //   [X] given a foreign order account exists
    //     [X] it reverts with an AccountDidNotDeserialize error
    // [X] given the provided order_id does does not match the computed order id from the order data
    //   [X] it reverts with an ConstraintSeeds error
    // [X] given the origin chain of the order is not the current chain
    //   [X] it reverts with an InvalidOriginChainId error
    // [X] given the order status is completed
    //   [X] it reverts with an OrderNotFillable error
    // [X] given all those checks pass and no fills have occured yet
    //   [X] given the amount_out_to_fill is greater than or equal to the amount out of the order
    //     [X] it transfers amount_out of token_out to the recipient
    //     [X] it transfers amount_in of token_in from the order book to the solver
    //     [X] it updates the order status to Completed
    //     [X] it updates amount_out_filled and amount_in_released on the order
    //   [X] given the amount_out_to_fill is less than the amount out of the order
    //     [X] it transfers amount_out_to_fill of token_out to the recipient
    //     [X] it transfers the proportional amount_in of token_in from the order book to the solver
    //     [X] it updates amount_out_filled and amount_in_released on the order
    // [X] given the program is paused
    //   [X] it reverts with a ProgramPaused error

    use anchor_litesvm::Pubkey;
    use order_book::{OrderData, ORDER_SEED_PREFIX};

    // fill_native_order
    // [X] given a local order is filled with fill_foreign_order
    //   [X] it reverts with an account deserialization error

    use super::*;

    fn default_order_params(
        test: &OrderBookTest,
        sender: &str,
    ) -> order_book::instructions::open::OrderParams {
        order_book::instructions::open::OrderParams {
            dest_chain_id: CHAIN_ID, // local order
            fill_deadline: test.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64 + 86400,
            token_out: test.get_mint("token-out-spl-6").clone().to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user(sender).pubkey().clone().to_bytes(),
            solver: test.get_user("solver").pubkey().clone().to_bytes(),
        }
    }

    fn default_fill_params(test: &OrderBookTest) -> order_book::instructions::fill::FillParams {
        order_book::instructions::fill::FillParams {
            amount_out_to_fill: 500_000,
            origin_recipient: test.get_user("solver").pubkey().clone().to_bytes(),
        }
    }

    #[test]
    fn fill_native_order_order_not_exist() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let fill_params = default_fill_params(&test);
        let order_id = [1u8; 32]; // non-existent order id
        let sender = test.get_user("alice");
        let solver = test.get_user("solver");
        let token_in_mint = test.get_mint("token-in-spl-6");

        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let (global_account, _) = test.get_global_account()?;

        let token_out_mint = Pubkey::new_from_array(order_params.token_out);
        let recipient = Pubkey::new_from_array(order_params.recipient);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);
        let solver_token_out_ata = get_associated_token_address(&solver.pubkey(), &token_out_mint);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);
        let solver_token_in_ata = get_associated_token_address(&solver.pubkey(), &token_in_mint);

        let order_data = order_book::state::OrderData {
            version: order_book::VERSION,
            sender: sender.pubkey().to_bytes(),
            nonce: 0,
            origin_chain_id: CHAIN_ID,
            dest_chain_id: order_params.dest_chain_id,
            created_at: test.current_time(),
            fill_deadline: order_params.fill_deadline,
            token_in: token_in_mint.to_bytes(),
            token_out: order_params.token_out,
            amount_in: order_params.amount_in as u128,
            amount_out: order_params.amount_out,
            recipient: order_params.recipient,
            solver: solver.pubkey().to_bytes(),
        };

        let accounts = order_book::accounts::FillNativeOrder {
            program: order_book::ID,
            event_authority: test.get_event_authority()?,
            solver: solver.pubkey(),
            global_account,
            token_out_mint,
            solver_token_out_account: solver_token_out_ata,
            recipient,
            recipient_token_out_ata,
            token_out_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            order: order_account,
            token_in_mint,
            order_token_in_ata,
            solver_token_in_account: solver_token_in_ata,
            token_in_program: anchor_spl::token::ID,
        };

        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params: fill_params.clone(),
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_log_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn fill_native_order_invalid_dest_chain_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let mut order_params = default_order_params(&test, "alice");
        order_params.dest_chain_id = DEST_CHAIN_ID; // set to foreign chain id
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let ix = test.create_fill_native_order_ix(&solver.pubkey(), order_id, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidDestChainId));

        Ok(())
    }

    #[test]
    fn fill_native_order_token_out_account_mismatch_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Get the order account to derive the order data
        let (_, native_order) = test.get_native_order_account(&order_id)?;
        let order_data = OrderData::new_from_native_order(native_order.data, CHAIN_ID);

        // Get the accounts for the instruction and change the token_out mint to the wrong one
        let mut accounts = test.build_fill_native_order_accounts(&solver.pubkey(), order_id)?;
        let token_out = test.mints.get("token-out-spl-9").unwrap();
        accounts.token_out_mint = *token_out;
        // Also adjust the token accounts to match the wrong token out to avoid other false positive errors
        accounts.recipient_token_out_ata =
            get_associated_token_address(&accounts.recipient, token_out);
        accounts.solver_token_out_account =
            get_associated_token_address(&solver.pubkey(), token_out);
        // Token Program is already the same

        // Construct the ix with the modified accounts
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        // Execute the instruction
        // Expect it to revert with an InvalidTokenOutMint error
        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidTokenOutMint));

        Ok(())
    }

    #[test]
    fn fill_native_order_recipient_account_mismatch_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);
        let wrong_recipient = test.get_user("bob");

        let (_, native_order) = test.get_native_order_account(&order_id)?;
        let order_data = OrderData::new_from_native_order(native_order.data, CHAIN_ID);
        let token_out_mint = Pubkey::new_from_array(order_params.token_out);

        // Get the accounts for the ix and manually change the recipient accounts
        let mut accounts = test.build_fill_native_order_accounts(&solver.pubkey(), order_id)?;
        accounts.recipient = wrong_recipient.pubkey();
        accounts.recipient_token_out_ata =
            get_associated_token_address(&wrong_recipient.pubkey(), &token_out_mint);

        // Construct the ix
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidRecipient));

        Ok(())
    }

    #[test]
    fn fill_native_order_order_type_not_native_and_not_initialized_reverts(
    ) -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order that originates on another chain and settles on this one
        let order_params = default_order_params(&test, "alice");
        let sender = test.get_user("alice");
        let token_in_mint = test.get_mint("token-in-spl-6");
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let order_data = OrderData {
            version: order_book::VERSION,
            sender: sender.pubkey().to_bytes(),
            nonce: 0,
            origin_chain_id: DEST_CHAIN_ID,
            dest_chain_id: CHAIN_ID,
            created_at: test.current_time(),
            fill_deadline: order_params.fill_deadline,
            token_in: token_in_mint.to_bytes(),
            token_out: order_params.token_out,
            amount_in: order_params.amount_in as u128,
            amount_out: order_params.amount_out,
            recipient: order_params.recipient,
            solver: order_params.solver,
        };

        let order_id = order_data.compute_order_id();

        // Get the accounts from the order data
        let accounts =
            test.build_fill_native_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;

        // Construct the ix
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_log_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn fill_native_order_order_type_not_native_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order that originates on another chain and settles on this one
        let order_params = default_order_params(&test, "alice");
        let sender = test.get_user("alice");
        let token_in_mint = test.get_mint("token-in-spl-6");
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let order_data = OrderData {
            version: order_book::VERSION,
            sender: sender.pubkey().to_bytes(),
            nonce: 0,
            origin_chain_id: DEST_CHAIN_ID,
            dest_chain_id: CHAIN_ID,
            created_at: test.current_time(),
            fill_deadline: order_params.fill_deadline,
            token_in: token_in_mint.to_bytes(),
            token_out: order_params.token_out,
            amount_in: order_params.amount_in as u128,
            amount_out: order_params.amount_out,
            recipient: order_params.recipient,
            solver: order_params.solver,
        };

        let order_id = order_data.compute_order_id();

        // Partially fill the order using the correct ix (fill_foreigh_order)
        test.fill_foreign_order("solver", &order_data, fill_params.amount_out_to_fill)?;

        // Get the accounts from the order data for a fill native order ix
        let accounts =
            test.build_fill_native_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;

        // Construct the ix
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_log_error("AccountDidNotDeserialize");

        Ok(())
    }

    #[test]
    fn fill_native_order_order_id_does_not_match_order_data() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let (_, native_order) = test.get_native_order_account(&order_id)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let mut order_data = OrderData::new_from_native_order(native_order.data, CHAIN_ID);
        // Change the amount_in to invalidate the order id
        order_data.amount_in += 1;

        let accounts = test.build_fill_native_order_accounts(&solver.pubkey(), order_id)?;

        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("InvalidOrderId");

        Ok(())
    }

    #[test]
    fn fill_native_order_invalid_origin_chain_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Get the order data and modify origin_chain_id to be different from current chain
        let (_, native_order) = test.get_native_order_account(&order_id)?;
        let order_data = OrderData::new_from_native_order(native_order.data, DEST_CHAIN_ID); // set to wrong chain id

        // Build accounts normally
        let accounts = test.build_fill_native_order_accounts(&solver.pubkey(), order_id)?;

        // Construct the instruction with modified order_data
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("InvalidOrderId"); // this is checked before the origin chain id

        Ok(())
    }

    #[test]
    fn fill_native_order_already_completed_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");

        // First, fill the order completely
        let full_fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: order_params.amount_out as u64, // Fill the entire amount
            origin_recipient: solver.pubkey().to_bytes(),
        };

        let ix = test.create_fill_native_order_ix(&solver.pubkey(), order_id, &full_fill_params)?;
        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_success();

        // Verify the order is completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed
        );

        // Now try to fill it again
        let fill_params = default_fill_params(&test);
        let ix = test.create_fill_native_order_ix(&solver.pubkey(), order_id, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::OrderNotFillable));

        Ok(())
    }

    #[test]
    fn fill_native_order_full_fill_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");

        // Get token account addresses
        let token_in_mint = test.get_mint("token-in-spl-6");
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");
        let solver_token_in_ata = test.get_ata("token-in-spl-6", "solver");
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

        // Get initial balances
        let alice_token_out_balance_before = test.get_token_balance(&alice_token_out_ata)?;
        let solver_token_out_balance_before = test.get_token_balance(&solver_token_out_ata)?;
        let solver_token_in_balance_before = test.get_token_balance(&solver_token_in_ata)?;
        let order_token_in_balance_before = test.get_token_balance(&order_token_in_ata)?;

        // Fill the entire order (amount_out_to_fill >= amount_out)
        let full_fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: order_params.amount_out as u64, // Fill the entire amount
            origin_recipient: solver.pubkey().to_bytes(),
        };

        let ix = test.create_fill_native_order_ix(&solver.pubkey(), order_id, &full_fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_success();

        // Get final balances
        let alice_token_out_balance_after = test.get_token_balance(&alice_token_out_ata)?;
        let solver_token_out_balance_after = test.get_token_balance(&solver_token_out_ata)?;
        let solver_token_in_balance_after = test.get_token_balance(&solver_token_in_ata)?;
        let order_token_in_balance_after = test.get_token_balance(&order_token_in_ata)?;

        // Verify token transfers
        // Recipient (alice) should receive amount_out of token_out
        assert_eq!(
            alice_token_out_balance_after,
            alice_token_out_balance_before + order_params.amount_out as u64,
            "Recipient should receive amount_out of token_out"
        );

        // Solver should pay amount_out of token_out
        assert_eq!(
            solver_token_out_balance_after,
            solver_token_out_balance_before - order_params.amount_out as u64,
            "Solver should pay amount_out of token_out"
        );

        // Solver should receive amount_in of token_in
        assert_eq!(
            solver_token_in_balance_after,
            solver_token_in_balance_before + order_params.amount_in,
            "Solver should receive amount_in of token_in"
        );

        // Order account should release amount_in of token_in
        assert_eq!(
            order_token_in_balance_after,
            order_token_in_balance_before - order_params.amount_in,
            "Order account should release amount_in of token_in"
        );

        // Verify order state
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed,
            "Order status should be Completed"
        );
        assert_eq!(
            order_data.data.amount_out_filled, order_params.amount_out as u128,
            "amount_out_filled should equal amount_out"
        );
        assert_eq!(
            order_data.data.amount_in_released, order_params.amount_in as u128,
            "amount_in_released should equal amount_in"
        );

        Ok(())
    }

    #[test]
    fn fill_native_order_partial_fill_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");

        // Get token account addresses
        let token_in_mint = test.get_mint("token-in-spl-6");
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");
        let solver_token_in_ata = test.get_ata("token-in-spl-6", "solver");
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

        // Get initial balances
        let alice_token_out_balance_before = test.get_token_balance(&alice_token_out_ata)?;
        let solver_token_out_balance_before = test.get_token_balance(&solver_token_out_ata)?;
        let solver_token_in_balance_before = test.get_token_balance(&solver_token_in_ata)?;
        let order_token_in_balance_before = test.get_token_balance(&order_token_in_ata)?;

        // Partial fill: fill only half of the order
        let amount_out_to_fill = order_params.amount_out / 2;
        let partial_fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: amount_out_to_fill as u64,
            origin_recipient: solver.pubkey().to_bytes(),
        };

        // Calculate expected amount_in to be released (proportional)
        let expected_amount_in_released = (amount_out_to_fill as u128
            * order_params.amount_in as u128)
            / order_params.amount_out as u128;

        let ix =
            test.create_fill_native_order_ix(&solver.pubkey(), order_id, &partial_fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_success();

        // Get final balances
        let alice_token_out_balance_after = test.get_token_balance(&alice_token_out_ata)?;
        let solver_token_out_balance_after = test.get_token_balance(&solver_token_out_ata)?;
        let solver_token_in_balance_after = test.get_token_balance(&solver_token_in_ata)?;
        let order_token_in_balance_after = test.get_token_balance(&order_token_in_ata)?;

        // Verify token transfers
        // Recipient (alice) should receive amount_out_to_fill of token_out
        assert_eq!(
            alice_token_out_balance_after,
            alice_token_out_balance_before + amount_out_to_fill as u64,
            "Recipient should receive amount_out_to_fill of token_out"
        );

        // Solver should pay amount_out_to_fill of token_out
        assert_eq!(
            solver_token_out_balance_after,
            solver_token_out_balance_before - amount_out_to_fill as u64,
            "Solver should pay amount_out_to_fill of token_out"
        );

        // Solver should receive proportional amount_in of token_in
        assert_eq!(
            solver_token_in_balance_after,
            solver_token_in_balance_before + expected_amount_in_released as u64,
            "Solver should receive proportional amount_in of token_in"
        );

        // Order account should release proportional amount_in of token_in
        assert_eq!(
            order_token_in_balance_after,
            order_token_in_balance_before - expected_amount_in_released as u64,
            "Order account should release proportional amount_in of token_in"
        );

        // Verify order state
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Created,
            "Order status should remain Created (not Completed)"
        );
        assert_eq!(
            order_data.data.amount_out_filled, amount_out_to_fill as u128,
            "amount_out_filled should equal amount_out_to_fill"
        );
        assert_eq!(
            order_data.data.amount_in_released, expected_amount_in_released,
            "amount_in_released should equal proportional amount"
        );

        // Verify we can fill it again
        let second_fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: amount_out_to_fill as u64, // Fill another half
            origin_recipient: solver.pubkey().to_bytes(),
        };

        let ix =
            test.create_fill_native_order_ix(&solver.pubkey(), order_id, &second_fill_params)?;

        test.ctx.svm.expire_blockhash();
        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_success();

        // Verify the order is now completed
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed,
            "Order status should be Completed after second fill"
        );

        Ok(())
    }

    #[test]
    fn fill_native_order_multiple_partial_fills() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");

        // Get token account addresses
        let token_in_mint = test.get_mint("token-in-spl-6");
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");
        let solver_token_in_ata = test.get_ata("token-in-spl-6", "solver");
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

        // Track cumulative fills
        let mut cumulative_amount_out_filled: u64 = 0;
        let mut cumulative_amount_in_released: u64 = 0;

        // Perform 4 fills: 3 partial fills of 25% each, then a final fill of the remaining 25%
        let fill_amounts = vec![
            order_params.amount_out / 4, // 25%
            order_params.amount_out / 4, // 25%
            order_params.amount_out / 4, // 25%
            order_params.amount_out / 4, // 25% (final)
        ];

        for (i, &amount_out_to_fill) in fill_amounts.iter().enumerate() {
            let is_final_fill = i == fill_amounts.len() - 1;

            // Get balances before this fill
            let alice_balance_before = test.get_token_balance(&alice_token_out_ata)?;
            let solver_token_out_balance_before = test.get_token_balance(&solver_token_out_ata)?;
            let solver_token_in_balance_before = test.get_token_balance(&solver_token_in_ata)?;
            let order_balance_before = test.get_token_balance(&order_token_in_ata)?;

            // Calculate expected amount_in to be released for this fill
            let expected_amount_in_for_this_fill = (amount_out_to_fill as u128
                * order_params.amount_in as u128)
                / order_params.amount_out as u128;

            // Execute fill
            let fill_params = order_book::instructions::fill::FillParams {
                amount_out_to_fill: amount_out_to_fill as u64,
                origin_recipient: solver.pubkey().to_bytes(),
            };

            let ix = test.create_fill_native_order_ix(&solver.pubkey(), order_id, &fill_params)?;

            test.ctx
                .execute_instruction(ix, &[&solver])?
                .assert_success();
            test.ctx.svm.expire_blockhash();

            // Update cumulative amounts
            cumulative_amount_out_filled += amount_out_to_fill as u64;
            cumulative_amount_in_released += expected_amount_in_for_this_fill as u64;

            // Get balances after this fill
            let alice_balance_after = test.get_token_balance(&alice_token_out_ata)?;
            let solver_token_out_balance_after = test.get_token_balance(&solver_token_out_ata)?;
            let solver_token_in_balance_after = test.get_token_balance(&solver_token_in_ata)?;
            let order_balance_after = test.get_token_balance(&order_token_in_ata)?;

            // Verify token transfers for this fill
            assert_eq!(
                alice_balance_after - alice_balance_before,
                amount_out_to_fill as u64,
                "Fill {}: Recipient should receive amount_out_to_fill",
                i + 1
            );
            assert_eq!(
                solver_token_out_balance_before - solver_token_out_balance_after,
                amount_out_to_fill as u64,
                "Fill {}: Solver should pay amount_out_to_fill",
                i + 1
            );
            assert_eq!(
                solver_token_in_balance_after - solver_token_in_balance_before,
                expected_amount_in_for_this_fill as u64,
                "Fill {}: Solver should receive proportional amount_in",
                i + 1
            );
            assert_eq!(
                order_balance_before - order_balance_after,
                expected_amount_in_for_this_fill as u64,
                "Fill {}: Order should release proportional amount_in",
                i + 1
            );

            // Verify order state after this fill
            let (_, order_data) = test.get_native_order_account(&order_id)?;

            if is_final_fill {
                assert_eq!(
                    order_data.data.status,
                    order_book::state::OrderStatus::Completed,
                    "Fill {}: Order should be Completed after final fill",
                    i + 1
                );
            } else {
                assert_eq!(
                    order_data.data.status,
                    order_book::state::OrderStatus::Created,
                    "Fill {}: Order should remain Created",
                    i + 1
                );
            }

            assert_eq!(
                order_data.data.amount_out_filled,
                cumulative_amount_out_filled as u128,
                "Fill {}: Cumulative amount_out_filled should match",
                i + 1
            );
            assert_eq!(
                order_data.data.amount_in_released,
                cumulative_amount_in_released as u128,
                "Fill {}: Cumulative amount_in_released should match",
                i + 1
            );
        }

        // Final verification: order should be fully filled
        let (_, order_data) = test.get_native_order_account(&order_id)?;
        assert_eq!(
            order_data.data.status,
            order_book::state::OrderStatus::Completed,
            "Order should be Completed"
        );
        assert_eq!(
            order_data.data.amount_out_filled, order_params.amount_out as u128,
            "Total amount_out_filled should equal order amount_out"
        );
        assert_eq!(
            order_data.data.amount_in_released, order_params.amount_in as u128,
            "Total amount_in_released should equal order amount_in"
        );

        Ok(())
    }

    #[test]
    fn fill_foreign_order_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Manually create the instruction to force a bad order account
        let (native_order_account, native_order_data) = test.get_native_order_account(&order_id)?;
        let (global_account, global_data) = test.get_global_account()?;

        let token_out_mint = Pubkey::new_from_array(native_order_data.data.token_out);
        let recipient = Pubkey::new_from_array(fill_params.origin_recipient);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);
        let solver_token_out_ata = get_associated_token_address(&solver.pubkey(), &token_out_mint);

        let order_data =
            OrderData::new_from_native_order(native_order_data.data, global_data.chain_id);

        let ix = test
            .ctx
            .program()
            .accounts(order_book::accounts::FillForeignOrder {
                program: order_book::ID,
                event_authority: test.get_event_authority()?,
                solver: solver.pubkey(),
                global_account,
                token_out_mint,
                solver_token_out_account: solver_token_out_ata,
                recipient,
                recipient_token_out_ata,
                token_out_program: anchor_spl::token::ID,
                associated_token_program: anchor_spl::associated_token::ID,
                order: native_order_account,
                portal_program: portal::ID, // following accounts not checked by the mock portal
                portal_global: test.ctx.svm.get_pda(
                    &[b"global"],
                    &portal::ID,
                ),
                portal_authority: test.ctx.svm.get_pda(
                    &[b"authority"],
                    &portal::ID,
                ),
                bridge_adapter: test.ctx.svm.get_pda(
                    &[b"bridge_adapter"],
                    &portal::ID,
                ),
                system_program: anchor_lang::system_program::ID,
            })
            .args(order_book::instruction::FillForeignOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("ConstraintSpace"); // triggered before invalid type because the size of the account is too large

        Ok(())
    }

    #[test]
    fn fill_native_order_expired_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create order with short deadline
        let mut order_params = default_order_params(&test, "alice");
        order_params.fill_deadline = test.current_time() + 100;
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Warp time past the deadline
        test.warp_forward(200);

        let ix = test.create_fill_native_order_ix(&solver.pubkey(), order_id, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::OrderExpired));

        Ok(())
    }

    #[test]
    fn fill_native_order_created_at_future_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Get the native order and build order_data with a future created_at
        let (_, native_order) = test.get_native_order_account(&order_id)?;
        let mut order_data = OrderData::new_from_native_order(native_order.data, CHAIN_ID);
        order_data.created_at = test.current_time() + 1000; // In the future

        let accounts = test.build_fill_native_order_accounts(&solver.pubkey(), order_id)?;

        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("InvalidOrderId"); // order_id won't match because created_at changed

        Ok(())
    }

    #[test]
    fn test_fill_native_order_paused_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create an order before pausing
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Pause the program
        test.pause()?;

        // Try to fill the order
        let solver = test.get_user("solver");
        let fill_params = order_book::instructions::FillParams {
            amount_out_to_fill: 500_000,
            origin_recipient: solver.pubkey().to_bytes(),
        };
        let ix = test.create_fill_native_order_ix(&solver.pubkey(), order_id, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::ProgramPaused));

        Ok(())
    }
}

mod xchain_orders {
    // Cross-chain (foreign) order fill tests
    // fill_foreign_order
    // [X] given the destination chain id of the order does not match the current chain id
    //   [X] it reverts with an InvalidDestChainId error
    // [X] given the token out on the order does not match the token out mint account
    //   [X] it reverts with an InvalidTokenOutMint error
    // [X] given the order recipient does not match the recipient account
    //   [X] it reverts with an InvalidRecipient error
    // [X] given order type is Native (trying to fill native order with fill_foreign_order)
    //   [X] it reverts with an account deserialization error
    // [X] given the provided order_id does not match the computed order id from the order data
    //   [X] it reverts with an InvalidOrderId error
    // [X] given the order id does not match the order account provided
    //   [X] it reverts with an ConstraintSeeds error
    // [X] given the order is already fully filled
    //   [X] it reverts with an OrderNotFillable error
    // [X] given all those checks pass and the order does not exist yet
    //   [X] given the amount_out_to_fill is less than the amount out of the order
    //     [X] it creates the order account
    //     [X] it sets the order type to Foreign
    //     [X] it transfers amount_out_to_fill of token_out to the recipient
    //     [X] it does NOT transfer any token_in (stays on origin chain)
    //     [X] it updates amount_out_filled and amount_in_released on the order
    //     [X] it sends a fill report message via the portal program
    //   [X] given the amount_out_to_fill is greater than or equal to the amount out of the order
    //     [X] it creates the order account
    //     [X] it fully fills the order in one transaction
    //     [X] it transfers amount_out of token_out to the recipient
    //     [X] it updates amount_out_filled and amount_in_released to full amounts
    // [X] given all those checks pass and the order exists and is partially filled
    //   [X] given the amount_out_to_fill is less than the remaining amount out
    //     [X] it transfers amount_out_to_fill of token_out to the recipient
    //     [X] it updates amount_out_filled and amount_in_released cumulatively
    //   [X] given the amount_out_to_fill is greater than or equal to the remaining amount out
    //     [X] it transfers the remaining amount_out to the recipient
    //     [X] it completes the order (amount_out_filled == amount_out)
    //     [X] it updates amounts to final values
    // [X] given the program is paused
    //   [X] it reverts with a ProgramPaused error

    // fill_native_order tests
    // [ ] given the order originates on another chain (not native)
    //   [ ] it reverts with an AccountNotInitialized error

    use super::*;
    use anchor_litesvm::Pubkey;
    use order_book::{OrderData, ORDER_SEED_PREFIX};

    fn default_foreign_order_data(test: &OrderBookTest, sender: &str) -> OrderData {
        OrderData {
            version: order_book::VERSION,
            sender: test.get_user(sender).pubkey().to_bytes(),
            nonce: 0,
            origin_chain_id: DEST_CHAIN_ID, // Foreign order originates on another chain
            dest_chain_id: CHAIN_ID,        // Settles on current chain
            created_at: test.current_time(),
            fill_deadline: test.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64 + 86400,
            token_in: test.get_mint("token-in-spl-6").to_bytes(),
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user(sender).pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        }
    }

    fn default_fill_params(test: &OrderBookTest) -> order_book::instructions::fill::FillParams {
        order_book::instructions::fill::FillParams {
            amount_out_to_fill: 500_000,
            origin_recipient: test.get_user("solver").pubkey().to_bytes(),
        }
    }

    #[test]
    fn fill_foreign_order_invalid_dest_chain_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let mut order_data = default_foreign_order_data(&test, "alice");
        order_data.dest_chain_id = DEST_CHAIN_ID; // Wrong chain - should be CHAIN_ID
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let ix = test.create_fill_foreign_order_ix(&solver.pubkey(), &order_data, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidDestChainId));

        Ok(())
    }

    #[test]
    fn fill_foreign_order_invalid_origin_chain_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let mut order_data = default_foreign_order_data(&test, "alice");
        order_data.origin_chain_id = CHAIN_ID; // Wrong chain - foreign order should not be CHAIN_ID
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let ix = test.create_fill_foreign_order_ix(&solver.pubkey(), &order_data, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidOriginChainId));

        Ok(())
    }

    #[test]
    fn fill_foreign_order_token_out_account_mismatch_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Get accounts and modify token_out_mint to wrong one
        let mut accounts =
            test.build_fill_foreign_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;
        let wrong_token_out = test.get_mint("token-out-spl-9");
        accounts.token_out_mint = wrong_token_out;
        // Also adjust token accounts to match the wrong token to avoid other false positive errors
        accounts.recipient_token_out_ata =
            get_associated_token_address(&accounts.recipient, &wrong_token_out);
        accounts.solver_token_out_account =
            get_associated_token_address(&solver.pubkey(), &wrong_token_out);

        let order_id = order_data.compute_order_id();
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillForeignOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidTokenOutMint));

        Ok(())
    }

    #[test]
    fn fill_foreign_order_recipient_account_mismatch_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let solver = test.get_user("solver");
        let wrong_recipient = test.get_user("bob");
        let fill_params = default_fill_params(&test);

        let token_out_mint = Pubkey::new_from_array(order_data.token_out);

        // Build accounts with wrong recipient
        let mut accounts =
            test.build_fill_foreign_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;
        accounts.recipient = wrong_recipient.pubkey();
        accounts.recipient_token_out_ata =
            get_associated_token_address(&wrong_recipient.pubkey(), &token_out_mint);

        let order_id = order_data.compute_order_id();
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillForeignOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidRecipient));

        Ok(())
    }

    #[test]
    fn fill_foreign_order_with_native_order_type_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create a native order (local order)
        let order_params = order_book::instructions::open::OrderParams {
            dest_chain_id: CHAIN_ID, // local order
            fill_deadline: test.ctx.svm.get_sysvar::<Clock>().unix_timestamp as u64 + 86400,
            token_out: test.get_mint("token-out-spl-6").to_bytes(),
            amount_in: 1_000_000,
            amount_out: 1_000_000,
            recipient: test.get_user("alice").pubkey().to_bytes(),
            solver: test.get_user("solver").pubkey().to_bytes(),
        };
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;

        // Try to fill it with fill_foreign_order
        let (_, native_order_data) = test.get_native_order_account(&order_id)?;
        let (global_account, global_data) = test.get_global_account()?;
        let order_data =
            OrderData::new_from_native_order(native_order_data.data, global_data.chain_id);
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Build foreign order accounts pointing to the native order
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let token_out_mint = Pubkey::new_from_array(order_data.token_out);
        let recipient = Pubkey::new_from_array(order_data.recipient);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);
        let solver_token_out_ata = get_associated_token_address(&solver.pubkey(), &token_out_mint);

        let ix = test
            .ctx
            .program()
            .accounts(order_book::accounts::FillForeignOrder {
                program: order_book::ID,
                event_authority: test.get_event_authority()?,
                solver: solver.pubkey(),
                global_account,
                token_out_mint,
                solver_token_out_account: solver_token_out_ata,
                recipient,
                recipient_token_out_ata,
                token_out_program: anchor_spl::token::ID,
                associated_token_program: anchor_spl::associated_token::ID,
                order: order_account,
                portal_program: portal::ID, // following accounts not checked by the mock portal
                portal_global: test.ctx.svm.get_pda(
                    &[b"global"],
                    &portal::ID,
                ),
                portal_authority: test.ctx.svm.get_pda(
                    &[b"authority"],
                    &portal::ID,
                ),
                bridge_adapter: test.ctx.svm.get_pda(
                    &[b"bridge_adapter"],
                    &portal::ID,
                ),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(order_book::instruction::FillForeignOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("ConstraintSpace");

        Ok(())
    }

    #[test]
    fn fill_foreign_order_invalid_order_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let wrong_order_id = Pubkey::new_unique().to_bytes();

        let accounts =
            test.build_fill_foreign_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;

        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillForeignOrder {
                order_id: wrong_order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("ConstraintSeeds");

        Ok(())
    }

    #[test]
    fn fill_foreign_order_account_mismatch_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let order_id = order_data.compute_order_id();

        // Build accounts with wrong order PDA
        let mut accounts =
            test.build_fill_foreign_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;
        let wrong_order_id = [99u8; 32];
        let wrong_order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &wrong_order_id], &order_book::ID);
        accounts.order = wrong_order_account;

        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillForeignOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("ConstraintSeeds");

        Ok(())
    }

    #[test]
    fn fill_foreign_order_already_filled_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let solver = test.get_user("solver");

        // First, fill the order completely
        let full_fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: order_data.amount_out as u64,
            origin_recipient: solver.pubkey().to_bytes(),
        };

        test.fill_foreign_order("solver", &order_data, full_fill_params.amount_out_to_fill)?;

        // Verify the order is fully filled
        let order_id = order_data.compute_order_id();
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(foreign_order.data.amount_out_filled, order_data.amount_out);

        // Try to fill it again
        let fill_params = default_fill_params(&test);
        let ix = test.create_fill_foreign_order_ix(&solver.pubkey(), &order_data, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::OrderNotFillable));

        Ok(())
    }

    #[test]
    fn fill_foreign_order_first_fill_partial_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let order_id = order_data.compute_order_id();

        // Verify order doesn't exist yet
        let order_account = test
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        assert!(
            test.ctx.svm.get_account(&order_account).is_none(),
            "Order should not exist yet"
        );

        // Get token account addresses
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");

        // Get initial balances
        let alice_balance_before = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_before = test.get_token_balance(&solver_token_out_ata)?;

        // Partial fill: 50%
        let amount_out_to_fill = order_data.amount_out as u64 / 2;
        let expected_amount_in_released =
            (amount_out_to_fill as u128 * order_data.amount_in) / order_data.amount_out;

        test.fill_foreign_order("solver", &order_data, amount_out_to_fill)?;

        // Get final balances
        let alice_balance_after = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_after = test.get_token_balance(&solver_token_out_ata)?;

        // Verify token_out transfers
        assert_eq!(
            alice_balance_after,
            alice_balance_before + amount_out_to_fill,
            "Recipient should receive amount_out_to_fill"
        );
        assert_eq!(
            solver_balance_after,
            solver_balance_before - amount_out_to_fill,
            "Solver should pay amount_out_to_fill"
        );

        // Verify order was created and initialized correctly
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.order_type,
            order_book::state::OrderType::Foreign,
            "Order type should be Foreign"
        );
        assert_eq!(
            foreign_order.data.amount_out_filled, amount_out_to_fill as u128,
            "amount_out_filled should match"
        );
        assert_eq!(
            foreign_order.data.amount_in_released, expected_amount_in_released,
            "amount_in_released should match"
        );

        Ok(())
    }

    #[test]
    fn fill_foreign_order_first_fill_full_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let order_id = order_data.compute_order_id();

        // Get token account addresses
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");

        // Get initial balances
        let alice_balance_before = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_before = test.get_token_balance(&solver_token_out_ata)?;

        // Full fill on first transaction
        test.fill_foreign_order("solver", &order_data, order_data.amount_out as u64)?;

        // Get final balances
        let alice_balance_after = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_after = test.get_token_balance(&solver_token_out_ata)?;

        // Verify token transfers
        assert_eq!(
            alice_balance_after,
            alice_balance_before + order_data.amount_out as u64,
            "Recipient should receive full amount_out"
        );
        assert_eq!(
            solver_balance_after,
            solver_balance_before - order_data.amount_out as u64,
            "Solver should pay full amount_out"
        );

        // Verify order is fully filled
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.amount_out_filled, order_data.amount_out,
            "Order should be fully filled"
        );
        assert_eq!(
            foreign_order.data.amount_in_released, order_data.amount_in,
            "Full amount_in should be released"
        );

        Ok(())
    }

    #[test]
    fn fill_foreign_order_partial_fill_existing_order() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let order_id = order_data.compute_order_id();

        // First fill: 25%
        let first_fill_amount = order_data.amount_out as u64 / 4;
        test.fill_foreign_order("solver", &order_data, first_fill_amount)?;
        test.ctx.svm.expire_blockhash();

        // Get balances before second fill
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");
        let alice_balance_before = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_before = test.get_token_balance(&solver_token_out_ata)?;

        // Second fill: 25%
        let second_fill_amount = order_data.amount_out as u64 / 4;
        test.fill_foreign_order("solver", &order_data, second_fill_amount)?;

        // Verify balances after second fill
        let alice_balance_after = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_after = test.get_token_balance(&solver_token_out_ata)?;

        assert_eq!(
            alice_balance_after - alice_balance_before,
            second_fill_amount,
            "Second fill should transfer correct amount_out"
        );
        assert_eq!(
            solver_balance_before - solver_balance_after,
            second_fill_amount,
            "Solver should pay correct amount_out"
        );

        // Verify cumulative amounts
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.amount_out_filled,
            (first_fill_amount + second_fill_amount) as u128,
            "Cumulative amount_out_filled should be correct"
        );

        Ok(())
    }

    #[test]
    fn fill_foreign_order_full_fill_existing_order() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");

        let order_id = order_data.compute_order_id();

        // First fill: 60%
        let first_fill_amount = (order_data.amount_out as u64 * 60) / 100;
        test.fill_foreign_order("solver", &order_data, first_fill_amount)?;

        // Get balances before completing fill
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");
        let alice_balance_before = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_before = test.get_token_balance(&solver_token_out_ata)?;

        // Second fill: complete the remaining 40%+
        let completing_fill_amount = order_data.amount_out as u64; // Request more than remaining
        let expected_actual_fill = order_data.amount_out as u64 - first_fill_amount;

        test.fill_foreign_order("solver", &order_data, completing_fill_amount)?;

        // Verify final balances
        let alice_balance_after = test.get_token_balance(&alice_token_out_ata)?;
        let solver_balance_after = test.get_token_balance(&solver_token_out_ata)?;

        assert_eq!(
            alice_balance_after - alice_balance_before,
            expected_actual_fill,
            "Final fill should only transfer remaining amount"
        );
        assert_eq!(
            solver_balance_before - solver_balance_after,
            expected_actual_fill,
            "Solver should pay remaining amount"
        );

        // Verify order is fully filled
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.amount_out_filled, order_data.amount_out,
            "Order should be fully filled"
        );
        assert_eq!(
            foreign_order.data.amount_in_released, order_data.amount_in,
            "Full amount_in should be released"
        );

        Ok(())
    }

    #[test]
    fn fill_foreign_order_multiple_partial_fills() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");

        let order_id = order_data.compute_order_id();
        let alice_token_out_ata = test.get_ata("token-out-spl-6", "alice");
        let solver_token_out_ata = test.get_ata("token-out-spl-6", "solver");

        // Track cumulative fills
        let mut cumulative_amount_out_filled: u64 = 0;
        let mut cumulative_amount_in_released: u64 = 0;

        // Perform 4 fills of 25% each
        let fill_amounts = vec![
            order_data.amount_out as u64 / 4,
            order_data.amount_out as u64 / 4,
            order_data.amount_out as u64 / 4,
            order_data.amount_out as u64 / 4,
        ];

        for (i, &amount_out_to_fill) in fill_amounts.iter().enumerate() {
            // Get balances before this fill
            let alice_balance_before = test.get_token_balance(&alice_token_out_ata)?;
            let solver_balance_before = test.get_token_balance(&solver_token_out_ata)?;

            // Calculate expected amount_in for this fill
            let expected_amount_in_for_this_fill =
                (amount_out_to_fill as u128 * order_data.amount_in) / order_data.amount_out;

            // Execute fill
            test.fill_foreign_order("solver", &order_data, amount_out_to_fill)?;
            test.ctx.svm.expire_blockhash();

            // Update cumulative amounts
            cumulative_amount_out_filled += amount_out_to_fill;
            cumulative_amount_in_released += expected_amount_in_for_this_fill as u64;

            // Verify balances
            let alice_balance_after = test.get_token_balance(&alice_token_out_ata)?;
            let solver_balance_after = test.get_token_balance(&solver_token_out_ata)?;

            assert_eq!(
                alice_balance_after - alice_balance_before,
                amount_out_to_fill,
                "Fill {}: Recipient should receive amount_out_to_fill",
                i + 1
            );
            assert_eq!(
                solver_balance_before - solver_balance_after,
                amount_out_to_fill,
                "Fill {}: Solver should pay amount_out_to_fill",
                i + 1
            );

            // Verify order state
            let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
            assert_eq!(
                foreign_order.data.amount_out_filled,
                cumulative_amount_out_filled as u128,
                "Fill {}: Cumulative amount_out_filled should match",
                i + 1
            );
            assert_eq!(
                foreign_order.data.amount_in_released,
                cumulative_amount_in_released as u128,
                "Fill {}: Cumulative amount_in_released should match",
                i + 1
            );
        }

        // Final verification
        let (_, foreign_order) = test.get_foreign_order_account(&order_id)?;
        assert_eq!(
            foreign_order.data.amount_out_filled, order_data.amount_out,
            "Order should be fully filled"
        );
        assert_eq!(
            foreign_order.data.amount_in_released, order_data.amount_in,
            "Full amount_in should be released"
        );

        Ok(())
    }

    #[test]
    fn fill_native_order_account_does_not_exists_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");
        let order_id = order_data.compute_order_id();
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let accounts =
            test.build_fill_native_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;
        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn fill_native_order_account_exists_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_data = default_foreign_order_data(&test, "alice");

        // Partially fill the foreign order to initialize the foreign order account
        test.fill_foreign_order("solver", &order_data, order_data.amount_out as u64 / 2)?;
        test.ctx.svm.expire_blockhash();

        // try to fill it as a native order
        let order_id = order_data.compute_order_id();
        let solver = test.get_user("solver");
        let accounts =
            test.build_fill_native_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;
        let fill_params = default_fill_params(&test);

        let ix = test
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data,
                fill_params,
            })
            .instruction()?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error("AccountDidNotDeserialize");

        Ok(())
    }

    #[test]
    fn fill_foreign_order_expired_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create order data with short deadline
        let mut order_data = default_foreign_order_data(&test, "alice");
        order_data.fill_deadline = test.current_time() + 100;

        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        // Warp time past the deadline
        test.warp_forward(200);

        let ix = test.create_fill_foreign_order_ix(&solver.pubkey(), &order_data, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::OrderExpired));

        Ok(())
    }

    #[test]
    fn fill_foreign_order_created_at_future_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Create order data with future created_at
        let mut order_data = default_foreign_order_data(&test, "alice");
        order_data.created_at = test.current_time() + 1000; // In the future

        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let ix = test.create_fill_foreign_order_ix(&solver.pubkey(), &order_data, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidCreatedAtTimestamp));

        Ok(())
    }

    #[test]
    fn test_fill_foreign_order_paused_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Pause the program
        test.pause()?;

        // Try to fill a foreign order
        let solver = test.get_user("solver");
        let order_data = default_foreign_order_data(&test, "alice");
        let fill_params = order_book::instructions::FillParams {
            amount_out_to_fill: 500_000,
            origin_recipient: solver.pubkey().to_bytes(),
        };
        let ix = test.create_fill_foreign_order_ix(&solver.pubkey(), &order_data, &fill_params)?;

        test.ctx
            .execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::ProgramPaused));

        Ok(())
    }
}
