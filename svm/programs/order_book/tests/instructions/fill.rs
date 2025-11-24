use super::super::{INITIAL_FUNDS, OrderBookTest, CHAIN_ID, DEST_CHAIN_ID, messenger};
use anchor_litesvm::{AssertionHelpers, TestHelpers, Signer};
use anchor_lang::prelude::Clock;
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
    // [ ] given the order id does not match the order account provided
    //   [ ] it reverts with an ConstraintSeeds error
    // [ ] given the origin chain of the order is not the current chain
    //   [ ] it reverts with an InvalidOriginChainId error
    // [ ] given the order status is completed
    //   [ ] it reverts with an InvalidOrderStatus error
    // [ ] given all those checks pass and no fills have occured yet
    //   [ ] given the amount_out_to_fill is greater than or equal to the amount out of the order
    //     [ ] it transfers amount_out of token_out to the recipient
    //     [ ] it transfers amount_in of token_in from the order book to the solver
    //     [ ] it updates the order status to Completed
    //     [ ] it updates amount_out_filled and amount_in_released on the order
    //   [ ] given the amount_out_to_fill is less than the amount out of the 
    //     [ ] it transfers amount_out_to_fill of token_out to the recipient
    //     [ ] it transfers the proportional amount_in of token_in from the order book to the solver
    //     [ ] it updates amount_out_filled and amount_in_released on the order

    use anchor_litesvm::Pubkey;
    use order_book::{ORDER_SEED_PREFIX, OrderData};

    // fill_foreign_order
    // [ ] given all other checks pass
    //   [ ] it reverts with an InvalidOrderType error 
    
    use super::*;

    fn default_order_params(test: &OrderBookTest, sender: &str) -> order_book::instructions::open::OrderParams {
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


        let order_account = test.ctx.svm.get_pda(
            &[ORDER_SEED_PREFIX, &order_id],
            &order_book::ID
        );
        let (global_account, _) = test.get_global_account()?;
        
        let token_out_mint = Pubkey::new_from_array(order_params.token_out);
        let recipient = Pubkey::new_from_array(order_params.recipient);
        let recipient_token_out_ata = get_associated_token_address(
            &recipient,
            &token_out_mint,
        );
        let solver_token_out_ata = get_associated_token_address(
            &solver.pubkey(),
            &token_out_mint,
        );
        let order_token_in_ata = get_associated_token_address(
            &order_account,
            &token_in_mint,
        );
        let solver_token_in_ata = get_associated_token_address(
            &solver.pubkey(),
            &token_in_mint,
        );

        let order_data = order_book::state::OrderData {
            version: order_book::VERSION,
            sender: sender.pubkey().to_bytes(),
            nonce: 0,
            origin_chain_id: CHAIN_ID,
            dest_chain_id: order_params.dest_chain_id,
            fill_deadline: order_params.fill_deadline,
            token_in: token_in_mint.to_bytes(),
            token_out: order_params.token_out,
            amount_in: order_params.amount_in as u128,
            amount_out: order_params.amount_out,
            recipient: order_params.recipient,
            solver: solver.pubkey().to_bytes()
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
        
        let ix = test.ctx.program()
            .accounts(accounts)
            .args(
                order_book::instruction::FillNativeOrder {
                    order_id,
                    order_data,
                    fill_params: fill_params.clone(),
                }
            )
            .instruction()?;

        test.ctx.execute_instruction(ix, &[&solver])?
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
        
        let ix = test.create_fill_native_order_ix(
            &solver.pubkey(),
            order_id,
            &fill_params
        )?;

        test.ctx.execute_instruction(ix, &[&solver])?
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
        accounts.recipient_token_out_ata = get_associated_token_address(&accounts.recipient, token_out);
        accounts.solver_token_out_account = get_associated_token_address(&solver.pubkey(), token_out);
        // Token Program is already the same

        // Construct the ix with the modified accounts
        let ix = test.ctx.program()
            .accounts(accounts)
            .args(
                order_book::instruction::FillNativeOrder {
                    order_id,
                    order_data,
                    fill_params
                }
            )
            .instruction()?;

        // Execute the instruction
        // Expect it to revert with an InvalidTokenOutMint error
        test.ctx.execute_instruction(ix, &[&solver])?
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
        accounts.recipient_token_out_ata = get_associated_token_address(&wrong_recipient.pubkey(), &token_out_mint);

        // Construct the ix
        let ix = test.ctx.program()
            .accounts(accounts)
            .args(
                order_book::instruction::FillNativeOrder {
                    order_id,
                    order_data,
                    fill_params
                }
            )
            .instruction()?;

        test.ctx.execute_instruction(ix, &[&solver])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidRecipient));

        Ok(())
    }

    #[test]
    fn fill_native_order_order_type_not_native_and_not_initialized_reverts() -> Result<(), Box<dyn Error>> {
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
            fill_deadline: order_params.fill_deadline,
            token_in: token_in_mint.to_bytes(),
            token_out: order_params.token_out,
            amount_in: order_params.amount_in as u128,
            amount_out: order_params.amount_out,
            recipient: order_params.recipient,
            solver: order_params.solver
        };

        let order_id = order_data.compute_order_id();

        // Get the accounts from the order data
        let accounts = test.build_fill_native_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;

        // Construct the ix
        let ix = test.ctx.program()
            .accounts(accounts)
            .args(
                order_book::instruction::FillNativeOrder {
                    order_id,
                    order_data,
                    fill_params
                }
            )
            .instruction()?;

        test.ctx.execute_instruction(ix, &[&solver])?
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
            fill_deadline: order_params.fill_deadline,
            token_in: token_in_mint.to_bytes(),
            token_out: order_params.token_out,
            amount_in: order_params.amount_in as u128,
            amount_out: order_params.amount_out,
            recipient: order_params.recipient,
            solver: order_params.solver
        };

        let order_id = order_data.compute_order_id();


        // Partially fill the order using the correct ix (fill_foreigh_order)
        test.fill_foreign_order("solver", &order_data, fill_params.amount_out_to_fill)?;

        // Get the accounts from the order data for a fill native order ix
        let accounts = test.build_fill_native_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;

        // Construct the ix
        let ix = test.ctx.program()
            .accounts(accounts)
            .args(
                order_book::instruction::FillNativeOrder {
                    order_id,
                    order_data,
                    fill_params
                }
            )
            .instruction()?;

        test.ctx.execute_instruction(ix, &[&solver])?
            .assert_log_error("AccountDidNotDeserialize");

        Ok(())
    }

    #[test]
    fn fill_native_order_order_id_does_not_match_order_data() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let expected_order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let (_, native_order) = test.get_native_order_account(&expected_order_id)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);

        let mut order_id = Pubkey::new_unique().to_bytes();
        // Ensure that we don't randomly get the right ID, odds are very low
        while order_id == expected_order_id {
            order_id = Pubkey::new_unique().to_bytes();
        }

        let order_data = OrderData::new_from_native_order(native_order.data, CHAIN_ID);
        let accounts = test.build_fill_native_order_accounts_from_order_data(&solver.pubkey(), &order_data)?;

        let ix = test.ctx.program()
            .accounts(accounts)
            .args(
                order_book::instruction::FillNativeOrder {
                    order_id,
                    order_data,
                    fill_params
                }
            )
            .instruction()?;

        test.ctx.execute_instruction(ix, &[&solver])?
            .assert_anchor_error("ConstraintSeeds");

        Ok(())
    }
    
    #[test]
    fn fill_native_order_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;
        let order_params = default_order_params(&test, "alice");
        let order_id = test.open_order("alice", "token-in-spl-6", &order_params)?;
        let solver = test.get_user("solver");
        let fill_params = default_fill_params(&test);
        
        let ix = test.create_fill_native_order_ix(
            &solver.pubkey(),
            order_id,
            &fill_params
        )?;

        test.ctx.execute_instruction(ix, &[&solver])?
            .assert_success();

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
        let recipient_token_out_ata = get_associated_token_address(
            &recipient,
            &token_out_mint,
        );
        let solver_token_out_ata = get_associated_token_address(
            &solver.pubkey(),
            &token_out_mint,
        );

        let order_data = OrderData::new_from_native_order(native_order_data.data, global_data.chain_id);

        let ix = test.ctx.program()
            .accounts(
                order_book::accounts::FillForeignOrder {
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
                    messenger_program: messenger::ID,
                    system_program: anchor_lang::system_program::ID
                }
            )
            .args(
                order_book::instruction::FillForeignOrder {
                    order_id,
                    order_data,
                    fill_params
                }
            )
            .instruction()?;

        test.ctx.execute_instruction(ix, &[&solver])?
            .assert_anchor_error("ConstraintSpace"); // triggered before invalid type because the size of the account is too large

        Ok(())
    }
}