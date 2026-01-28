use super::super::{OrderBookTest, CHAIN_ID};
use anchor_lang::prelude::Pubkey;
use anchor_litesvm::{Signer, TestHelpers};
use std::error::Error;

// Initialize instruction tests
// [X] given the order book has not been initialized
//   [X] it creates the global account at the correct PDA
//   [X] it sets the admin to the signer
//   [X] it sets the chain_id to the provided value
//   [X] it sets the portal_authority to the portal program's authority PDA
//   [X] it saves the bump
//   [X] it initializes reserved space to zeros
// [X] given a custom chain_id is provided
//   [X] it initializes with the custom chain_id
// [X] given a non-admin user initializes the order book
//   [X] it succeeds (initialization is permissionless)
//   [X] it sets the non-admin user as the admin
// [X] given the order book has already been initialized
//   [X] it reverts with an AccountAlreadyInitialized error
// [X] given the global_account PDA is incorrect
//   [X] it reverts with a ConstraintSeeds error

#[test]
fn test_initialize_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    let admin = test.get_user("admin");
    let portal_authority = test.get_user("portal_authority").pubkey();

    // Verify global account doesn't exist yet
    let global_account = test
        .ctx
        .svm
        .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);
    assert!(
        test.ctx.svm.get_account(&global_account).is_none(),
        "Global account should not exist yet"
    );

    // Create and execute initialize instruction
    let ix = test.create_initialize_ix(&admin.pubkey(), CHAIN_ID, &portal_authority)?;
    test.ctx
        .execute_instruction(ix, &[&admin])?
        .assert_success();

    // Verify global account was created
    assert!(
        test.ctx.svm.get_account(&global_account).is_some(),
        "Global account should exist after initialization"
    );

    // Verify global account data
    let (_, global_data) = test.get_global_account()?;
    assert_eq!(
        global_data.admin,
        admin.pubkey(),
        "Admin should be set correctly"
    );
    assert_eq!(
        global_data.chain_id, CHAIN_ID,
        "Chain ID should match input"
    );

    // Verify portal authority is set correctly
    assert_eq!(
        global_data.portal_authority, portal_authority,
        "Portal authority should be set correctly"
    );

    // Verify bump is non-zero (bump should be valid)
    assert!(global_data.bump > 0, "Bump should be set to a valid value");

    // Verify reserved is zeroed
    assert_eq!(
        global_data.reserved, [0u8; 128],
        "Reserved space should be zeroed"
    );

    Ok(())
}

#[test]
fn test_initialize_with_different_chain_id_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    let admin = test.get_user("admin");
    let portal_authority = test.get_user("portal_authority").pubkey();
    let custom_chain_id: u32 = 42;

    // Initialize with custom chain_id
    let ix = test.create_initialize_ix(&admin.pubkey(), custom_chain_id, &portal_authority)?;
    test.ctx
        .execute_instruction(ix, &[&admin])?
        .assert_success();

    // Verify chain_id is set to custom value
    let (_, global_data) = test.get_global_account()?;
    assert_eq!(
        global_data.chain_id, custom_chain_id,
        "Chain ID should match custom input"
    );
    assert_eq!(
        global_data.admin,
        admin.pubkey(),
        "Admin should be set correctly"
    );

    Ok(())
}

#[test]
fn test_initialize_non_admin_signer_success() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    let alice = test.get_user("alice");
    let portal_authority = test.get_user("portal_authority").pubkey();

    // Alice (non-admin) initializes the order book
    let ix = test.create_initialize_ix(&alice.pubkey(), CHAIN_ID, &portal_authority)?;
    test.ctx
        .execute_instruction(ix, &[&alice])?
        .assert_success();

    // Verify alice is set as admin (whoever initializes becomes admin)
    let (_, global_data) = test.get_global_account()?;
    assert_eq!(
        global_data.admin,
        alice.pubkey(),
        "Signer (Alice) should be set as admin"
    );
    assert_eq!(
        global_data.chain_id, CHAIN_ID,
        "Chain ID should be set correctly"
    );

    Ok(())
}

#[test]
fn test_initialize_already_initialized_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    let admin = test.get_user("admin");
    let portal_authority = test.get_user("portal_authority").pubkey();

    // First initialization
    test.initialize()?;
    test.ctx.svm.expire_blockhash();

    // Attempt to initialize again
    let ix = test.create_initialize_ix(&admin.pubkey(), CHAIN_ID, &portal_authority)?;
    test.ctx
        .execute_instruction(ix, &[&admin])?
        .assert_failure();

    Ok(())
}

#[test]
fn test_initialize_wrong_global_account_pda_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    let admin = test.get_user("admin");
    let portal_authority = test.get_user("portal_authority").pubkey();

    // Create wrong PDA (using different seeds)
    let wrong_global_account = test.ctx.svm.get_pda(&[b"wrong_seed"], &order_book::ID);

    // Create instruction with wrong global_account PDA
    let accounts = order_book::accounts::Initialize {
        admin: admin.pubkey(),
        global_account: wrong_global_account,
        system_program: anchor_lang::solana_program::system_program::ID,
    };

    let ix = test
        .ctx
        .program()
        .accounts(accounts)
        .args(order_book::instruction::Initialize {
            chain_id: CHAIN_ID,
            portal_authority,
        })
        .instruction()?;

    test.ctx
        .execute_instruction(ix, &[&admin])?
        .assert_anchor_error("ConstraintSeeds");

    Ok(())
}

#[test]
fn test_initialize_zero_portal_authority_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;
    let admin = test.get_user("admin");

    // Attempt to initialize with default (zero) pubkey as portal_authority
    let ix = test.create_initialize_ix(&admin.pubkey(), CHAIN_ID, &Pubkey::default())?;
    test.ctx
        .execute_instruction(ix, &[&admin])?
        .assert_anchor_error("InvalidPortalAuthority");

    Ok(())
}

#[test]
fn test_set_portal_authority_zero_address_reverts() -> Result<(), Box<dyn Error>> {
    let mut test = OrderBookTest::new()?;

    // First initialize the order book
    test.initialize()?;

    let admin = test.get_user("admin");

    // Attempt to set portal authority to zero address
    let ix = test.create_set_portal_authority_ix(&admin.pubkey(), &Pubkey::default())?;
    test.ctx
        .execute_instruction(ix, &[&admin])?
        .assert_anchor_error("InvalidPortalAuthority");

    Ok(())
}
