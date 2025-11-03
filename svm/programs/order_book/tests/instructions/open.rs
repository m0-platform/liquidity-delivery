use super::super::OrderBookTest;
use anchor_litesvm::{AssertionHelpers, TestHelpers, Signer};
use anchor_spl::associated_token::get_associated_token_address;

use order_book;

// Test cases
// Local orders
// [ ] given amount_in is zero 
//   [ ] it reverts with an InvalidAmountIn error

#[test]
fn test_localOrder_amountInZero_reverts() -> Result<(), Box<dyn std::error::Error>> {
    // Setup test environment
    let mut test = OrderBookTest::new()?;
    test.initialize()?;

    let alice = test.users.get("alice").unwrap();
    let token_in_mint = test.mints.get("token-in-spl-6").unwrap();
    let sender_token_in_account = test.atas.get(&("token-in-spl-6", "alice")).unwrap();

    let (global_account, global_data) = test.get_global_account()?;
    // let (sender_nonce_account, sender_nonce_data) = test.get_sender_nonce_account(alice)?;
    let sender_nonce_account = test.ctx.svm.get_pda(
        &[
            order_book::state::NONCE_SEED_PREFIX,
            &alice.pubkey().to_bytes(),
        ],
        &order_book::ID,
    );

    // Prepare order parameters with amount_in set to zero
    let order_params = order_book::instructions::open::OrderParams {
        dest_chain_id: global_data.chain_id, // local order
        fill_deadline: test.ctx.svm.get_sysvar::<anchor_lang::prelude::Clock>().unix_timestamp as u64 + 86400,
        token_out: test.mints.get("token-out-spl-6").unwrap().to_bytes(),
        amount_in: 0, // Invalid amount_in
        amount_out: 1_000_000,
        recipient: alice.pubkey().to_bytes(),
        solver: test.users.get("solver").unwrap().pubkey().to_bytes(),
    };

    let order_id = order_book::state::compute_order_id(&order_book::state::OrderData {
        version: order_book::constants::VERSION,
        sender: alice.pubkey().to_bytes(),
        nonce: 0,
        origin_chain_id: global_data.chain_id,
        dest_chain_id: order_params.dest_chain_id,
        fill_deadline: order_params.fill_deadline,
        token_out: order_params.token_out,
        amount_in: order_params.amount_in as u128,
        amount_out: order_params.amount_out,
        recipient: order_params.recipient,
        solver: order_params.solver,
    });
    let order = test.ctx.svm.get_pda(
        &[
            order_book::state::ORDER_SEED_PREFIX,
            &order_id,
        ],
         &order_book::ID,
    );
    let order_token_in_ata = get_associated_token_address(&order, token_in_mint);

    // Attempt to open order and expect failure
    let ix = test.ctx.program()
        .accounts(
            order_book::accounts::OpenOrder {
                program: order_book::ID,
                event_authority: test.get_event_authority()?,
                payer: alice.pubkey(),
                token_authority: None,
                global_account,
                destination_account: None,
                token_in_mint: *token_in_mint,
                sender_token_in_account: *sender_token_in_account,
                sender_nonce_account,
                order,
                order_token_in_ata,
                token_in_program: anchor_spl::token::ID,
                associated_token_program: anchor_spl::associated_token::ID,
                system_program: anchor_lang::solana_program::system_program::ID
            }
        )
        .args(
            order_book::instruction::OpenOrder {
                params: order_params,
            }
        )
        .instruction()?;

    test.ctx.execute_instruction(ix, &[alice])?
        .assert_anchor_error("InvalidAmountIn");    

    Ok(())
}