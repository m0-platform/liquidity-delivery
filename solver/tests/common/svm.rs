use anchor_client::{
    anchor_lang::system_program,
    solana_sdk::{
        program_pack::Pack, pubkey::Pubkey, signature::Keypair, signer::Signer, system_instruction,
        transaction::Transaction,
    },
    Program,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use spl_associated_token_account::{
    get_associated_token_address, instruction::create_associated_token_account,
};
use spl_token::instruction::{initialize_mint, mint_to};
use std::sync::Arc;

pub async fn create_open_order(
    program: Program<Arc<Keypair>>,
    token_in_mint: &Pubkey,
    order_params: &order_book::instructions::open::OrderParams,
) -> String {
    let global_account = orderbook_pda(&[b"global"]);
    let event_authority = orderbook_pda(&[b"__event_authority"]);
    let sender_nonce_account = orderbook_pda(&[
        order_book::state::NONCE_SEED_PREFIX,
        &program.payer().to_bytes(),
    ]);
    let destination_account = orderbook_pda(&[
        order_book::state::DESTINATION_SEED_PREFIX,
        &1u32.to_be_bytes(),
    ]);

    let sender_nonce = 0;

    let order_id = order_book::state::compute_order_id(&order_book::state::OrderData {
        version: order_book::constants::VERSION,
        sender: program.payer().to_bytes(),
        nonce: sender_nonce,
        origin_chain_id: 1399811149,
        dest_chain_id: order_params.dest_chain_id,
        created_at: order_params.created_at,
        fill_deadline: order_params.fill_deadline,
        token_in: token_in_mint.to_bytes(),
        token_out: order_params.token_out,
        amount_in: order_params.amount_in as u128,
        amount_out: order_params.amount_out,
        recipient: order_params.recipient,
        solver: order_params.solver,
    });

    let order = orderbook_pda(&[order_book::state::ORDER_SEED_PREFIX, &order_id]);
    let order_token_in_ata = get_associated_token_address(&order, token_in_mint);
    let sender_token_in_account = get_associated_token_address(&program.payer(), token_in_mint);

    program
        .request()
        .accounts(order_book::accounts::OpenOrder {
            program: order_book::ID,
            event_authority,
            payer: program.payer(),
            token_authority: None,
            global_account,
            destination_account: Some(destination_account),
            token_in_mint: *token_in_mint,
            sender_token_in_account,
            sender_nonce_account,
            order,
            order_token_in_ata,
            token_in_program: spl_token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: system_program::ID,
        })
        .args(order_book::instruction::OpenOrder {
            params: order_params.clone(),
        })
        .send()
        .await
        .expect("failed to build order transaction");

    hex::encode(order_id)
}

pub fn orderbook_pda(seeds: &[&[u8]]) -> Pubkey {
    Pubkey::find_program_address(seeds, &order_book::ID).0
}

/// Create a new SPL token mint and mint tokens to a user
pub async fn create_and_mint_token(
    rpc_client: RpcClient,
    payer: &Keypair,
    user: &Pubkey,
    amount: u64,
) -> Pubkey {
    let mint = Keypair::from_base58_string(
        "5MWfiivY5yu2Q3M9uJqP2owncWtw8CuzyMqrbLAXMvUQLEhr2WZRgR5H3RQN5kMveS17vxXrRd3tXYVNaXEWZYrB",
    );
    let mint_pubkey = mint.pubkey();

    let rent = rpc_client
        .get_minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN)
        .await
        .expect("failed to get rent");

    // Create mint account
    let create_account_ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint_pubkey,
        rent,
        spl_token::state::Mint::LEN as u64,
        &spl_token::ID,
    );

    // Initialize mint
    let init_mint_ix = initialize_mint(
        &spl_token::ID,
        &mint_pubkey,
        &payer.pubkey(),
        Some(&payer.pubkey()),
        6,
    )
    .expect("failed to create initialize mint instruction");

    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .await
        .expect("failed to get blockhash");

    let mint_tx = Transaction::new_signed_with_payer(
        &[create_account_ix, init_mint_ix],
        Some(&payer.pubkey()),
        &[&payer, &mint],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&mint_tx)
        .await
        .expect("failed to create mint");

    // Create user's associated token account
    let user_token_account = get_associated_token_address(user, &mint_pubkey);

    let create_ata_ix =
        create_associated_token_account(&payer.pubkey(), user, &mint_pubkey, &spl_token::ID);

    // Mint tokens to user
    let mint_to_ix = mint_to(
        &spl_token::ID,
        &mint_pubkey,
        &user_token_account,
        &payer.pubkey(),
        &[],
        amount,
    )
    .expect("failed to create mint_to instruction");

    // Create transaction for ATA and minting
    let recent_blockhash = rpc_client
        .get_latest_blockhash()
        .await
        .expect("failed to get blockhash");

    let mint_to_tx = Transaction::new_signed_with_payer(
        &[create_ata_ix, mint_to_ix],
        Some(&payer.pubkey()),
        &[payer],
        recent_blockhash,
    );

    rpc_client
        .send_and_confirm_transaction(&mint_to_tx)
        .await
        .expect("failed to mint tokens");

    mint_pubkey
}
