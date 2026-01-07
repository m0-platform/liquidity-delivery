use super::super::{OrderBookTest, CHAIN_ID, DEST_CHAIN_ID};
use anchor_litesvm::Signer;
use order_book::error::OrderBookError;
use std::error::Error;

// AddDestination instruction tests
// [X] given the signer is not admin
//   [X] it reverts with NotAuthorized error
// [X] given the dest_chain_id equals the current chain_id
//   [X] it reverts with InvalidDestChainId error
// [X] given the destination account already exists
//   [X] it reverts (account already initialized)
// [X] given all checks pass
//   [X] it creates the destination account
//   [X] it sets is_supported to true

mod add_destination {
    use super::*;

    #[test]
    fn test_add_destination_unauthorized_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        // Initialize without adding destination
        let admin = test.get_user("admin");
        let messenger_authority = test.get_user("messenger_authority");

        let ix = test.create_initialize_ix(&admin.pubkey(), CHAIN_ID, &messenger_authority.pubkey())?;
        test.ctx.execute_instruction(ix, &[&admin])?.assert_success();

        // Try to add destination with non-admin (alice)
        let alice = test.get_user("alice");

        let ix = test.create_add_destination_ix(&alice.pubkey(), 3)?;

        test.ctx
            .execute_instruction(ix, &[&alice])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

        Ok(())
    }

    #[test]
    fn test_add_destination_same_chain_id_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        // Initialize without adding destination
        let admin = test.get_user("admin");
        let messenger_authority = test.get_user("messenger_authority");

        let ix = test.create_initialize_ix(&admin.pubkey(), CHAIN_ID, &messenger_authority.pubkey())?;
        test.ctx.execute_instruction(ix, &[&admin])?.assert_success();

        // Try to add current chain as destination
        let ix = test.create_add_destination_ix(&admin.pubkey(), CHAIN_ID)?;

        test.ctx
            .execute_instruction(ix, &[&admin])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::InvalidDestChainId));

        Ok(())
    }

    #[test]
    fn test_add_destination_already_exists_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?; // This adds DEST_CHAIN_ID (2) as destination

        // Verify destination exists
        let (_, dest) = test.get_destination_account(DEST_CHAIN_ID)?;
        assert!(dest.is_supported);

        // Expire blockhash to avoid AlreadyProcessed error
        test.ctx.svm.expire_blockhash();

        // Try to add the same destination again
        let admin = test.get_user("admin");
        let ix = test.create_add_destination_ix(&admin.pubkey(), DEST_CHAIN_ID)?;

        // Should fail because the account already exists
        test.ctx
            .execute_instruction(ix, &[&admin])?
            .assert_anchor_error("already in use");

        Ok(())
    }

    #[test]
    fn test_add_destination_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        // Initialize without adding destination
        let admin = test.get_user("admin");
        let messenger_authority = test.get_user("messenger_authority");

        let ix = test.create_initialize_ix(&admin.pubkey(), CHAIN_ID, &messenger_authority.pubkey())?;
        test.ctx.execute_instruction(ix, &[&admin])?.assert_success();

        // Verify destination doesn't exist yet
        let result = test.get_destination_account(3);
        assert!(result.is_err());

        // Add destination
        test.add_destination(3)?;

        // Verify destination was created
        let (_, dest) = test.get_destination_account(3)?;
        assert!(dest.is_supported);

        Ok(())
    }
}

// RemoveDestination instruction tests
// [X] given the signer is not admin
//   [X] it reverts with NotAuthorized error
// [X] given the destination account does not exist
//   [X] it reverts (account not initialized)
// [X] given all checks pass
//   [X] it closes the destination account
//   [X] it returns rent to admin

mod remove_destination {
    use super::*;

    #[test]
    fn test_remove_destination_unauthorized_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?; // Adds DEST_CHAIN_ID as destination

        // Try to remove destination with non-admin (alice)
        let alice = test.get_user("alice");

        let ix = test.create_remove_destination_ix(&alice.pubkey(), DEST_CHAIN_ID)?;

        test.ctx
            .execute_instruction(ix, &[&alice])?
            .assert_anchor_error(&format!("{:?}", OrderBookError::NotAuthorized));

        Ok(())
    }

    #[test]
    fn test_remove_destination_not_exist_reverts() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?;

        // Try to remove destination that doesn't exist
        let admin = test.get_user("admin");
        let ix = test.create_remove_destination_ix(&admin.pubkey(), 999)?;

        test.ctx
            .execute_instruction(ix, &[&admin])?
            .assert_anchor_error("AccountNotInitialized");

        Ok(())
    }

    #[test]
    fn test_remove_destination_success() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        test.initialize()?; // Adds DEST_CHAIN_ID as destination

        // Verify destination exists
        let (_, dest) = test.get_destination_account(DEST_CHAIN_ID)?;
        assert!(dest.is_supported);

        // Remove destination
        test.remove_destination(DEST_CHAIN_ID)?;

        // Verify destination was removed
        let result = test.get_destination_account(DEST_CHAIN_ID);
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_add_then_remove_destination_flow() -> Result<(), Box<dyn Error>> {
        let mut test = OrderBookTest::new()?;
        // Initialize without adding destination
        let admin = test.get_user("admin");
        let messenger_authority = test.get_user("messenger_authority");

        let ix = test.create_initialize_ix(&admin.pubkey(), CHAIN_ID, &messenger_authority.pubkey())?;
        test.ctx.execute_instruction(ix, &[&admin])?.assert_success();

        // Add destination chain 3
        test.add_destination(3)?;

        // Verify it exists
        let (_, dest) = test.get_destination_account(3)?;
        assert!(dest.is_supported);

        // Expire blockhash to avoid AlreadyProcessed error
        test.ctx.svm.expire_blockhash();

        // Remove it
        test.remove_destination(3)?;

        // Verify it's gone
        let result = test.get_destination_account(3);
        assert!(result.is_err());

        // Expire blockhash to avoid AlreadyProcessed error
        test.ctx.svm.expire_blockhash();

        // Can add it again
        test.add_destination(3)?;

        // Verify it exists again
        let (_, dest) = test.get_destination_account(3)?;
        assert!(dest.is_supported);

        Ok(())
    }
}
