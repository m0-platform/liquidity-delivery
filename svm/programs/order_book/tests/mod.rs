mod instructions;

use anchor_litesvm::{AnchorContext, AnchorLiteSVM, Keypair, Pubkey, Signer, TestHelpers, get_anchor_account};
use std::collections::HashMap;

anchor_lang::declare_program!(messenger);

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
const INITIAL_FUNDS: u64 = 10 * LAMPORTS_PER_SOL;

const CHAIN_ID: u32 = 1; // Example chain ID for testing
    
struct OrderBookTest {
    pub ctx: AnchorContext,
    pub users: HashMap<&'static str, Keypair>,
    pub mints: HashMap<&'static str, Pubkey>,
    pub atas: HashMap<(&'static str, &'static str), Pubkey>,
}

impl OrderBookTest {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Create the Anchor Lite SVM context with the order_book program loaded
        // Ensure that all the standard SPL programs are also loaded
        let mut ctx = AnchorLiteSVM::build_with_program(
            order_book::ID,
            include_bytes!("../../../target/deploy/order_book.so"),
        );

        // Create users and fund them
        let mut users: HashMap<&str, Keypair> = HashMap::new();
        users.insert("admin", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert("alice", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert("bob", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert("solver", ctx.svm.create_funded_account(INITIAL_FUNDS)?);

        // Create token mints with different decimal places
        // TODO anchor-litesvm doesn't have token2022 convenience methods yet
        let admin = users.get("admin").unwrap();
        let mut mints: HashMap<&str, Pubkey> = HashMap::new();
        mints.insert("token-in-spl-6", ctx.svm.create_token_mint(admin, 6)?.pubkey());
        mints.insert("token-out-spl-6", ctx.svm.create_token_mint(admin, 6)?.pubkey());
        mints.insert("token-in-spl-9", ctx.svm.create_token_mint(admin, 9)?.pubkey());
        mints.insert("token-out-spl-9", ctx.svm.create_token_mint(admin, 9)?.pubkey());

        // Create ATAs for alice and the solver and mint them tokens
        let alice = users.get("alice").unwrap();
        let solver = users.get("solver").unwrap();
        let mut atas: HashMap<(&str, &str), Pubkey> = HashMap::new();

        for (token_name, token_mint) in mints.iter() {
            let alice_ata = ctx.svm.create_associated_token_account(
                token_mint,
                alice,
            )?;
            let solver_ata = ctx.svm.create_associated_token_account(
                token_mint,
                solver,
            )?;

            atas.insert((token_name, "alice"), alice_ata);
            atas.insert((token_name, "solver"), solver_ata);

            ctx.svm.mint_to(
                token_mint,
                &alice_ata,
                admin,
                INITIAL_FUNDS
            )?;

            ctx.svm.mint_to(
                token_mint,
                &solver_ata,
                admin,
                INITIAL_FUNDS
            )?;
        }

        Ok(Self {
            ctx,
            users,
            mints,
            atas
        })
    }

    fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let admin = self.users.get("admin").unwrap();

        let ix = self.ctx.program()
            .accounts(
                order_book::accounts::Initialize {
                    admin: admin.pubkey(),
                    global_account: self.ctx.svm.get_pda(
                        &[
                            order_book::state::GLOBAL_SEED,
                        ],
                        &order_book::ID,
                    ),
                    messenger_authority: self.ctx.svm.get_pda(
                        &[
                            messenger::constants::AUTHORITY_SEED,
                        ],
                        &messenger::ID,
                    ),
                    system_program: anchor_lang::solana_program::system_program::ID,
                }
            )
            .args(
                order_book::instruction::Initialize {
                    chain_id: CHAIN_ID,
                }
            )
            .instruction()?;

        self.ctx.execute_instruction(ix, &[admin])?;

        Ok(())
    }

    // Helpers to fetch program accounts

    fn get_global_account(
        &self,
    ) -> Result<(Pubkey, order_book::state::OrderBookGlobal), Box<dyn std::error::Error>> {
        let global_account = self.ctx.svm.get_pda(
            &[
                order_book::state::GLOBAL_SEED,
            ],
            &order_book::ID,
        );

        let global_account_data: order_book::state::OrderBookGlobal = get_anchor_account(&self.ctx.svm, &global_account)?;

        Ok((global_account, global_account_data))
    }

    fn get_native_order_account(
        &self,
        order_id: &[u8; 32]
    ) -> Result<(Pubkey, order_book::state::Order<order_book::state::NativeOrder>), Box<dyn std::error::Error>> {
        let order_account = self.ctx.svm.get_pda(
            &[
                order_book::state::ORDER_SEED_PREFIX,
                order_id,
            ],
            &order_book::ID,
        );

        let order_account_data: order_book::state::Order<order_book::state::NativeOrder> = get_anchor_account(&self.ctx.svm, &order_account)?;

        Ok((order_account, order_account_data))
    }

    fn get_foreign_order_account(
        &self,
        order_id: &[u8; 32]
    ) -> Result<(Pubkey, order_book::state::Order<order_book::state::ForeignOrder>), Box<dyn std::error::Error>> {
        let order_account = self.ctx.svm.get_pda(
            &[
                order_book::state::ORDER_SEED_PREFIX,
                order_id,
            ],
            &order_book::ID,
        );

        let order_account_data: order_book::state::Order<order_book::state::ForeignOrder> = get_anchor_account(&self.ctx.svm, &order_account)?;

        Ok((order_account, order_account_data))
    }

    fn get_sender_nonce_account(
        &self,
        sender: &Keypair,
    ) -> Result<(Pubkey, order_book::state::Nonce), Box<dyn std::error::Error>> {
        let sender_nonce_account = self.ctx.svm.get_pda(
            &[
                order_book::state::NONCE_SEED_PREFIX,
                &sender.pubkey().to_bytes(),
            ],
            &order_book::ID,
        );

        let sender_nonce_data: order_book::state::Nonce = get_anchor_account(&self.ctx.svm, &sender_nonce_account)?;

        Ok((sender_nonce_account, sender_nonce_data))
    }

    fn get_destination_account(
        &self,
        dest_chain_id: u32,
    ) -> Result<(Pubkey, order_book::state::Destination), Box<dyn std::error::Error>> {
        let destination_account = self.ctx.svm.get_pda(
            &[
                order_book::state::DESTINATION_SEED_PREFIX,
                &dest_chain_id.to_be_bytes(),
            ],
            &order_book::ID,
        );

        let destination_account_data: order_book::state::Destination = get_anchor_account(&self.ctx.svm, &destination_account)?;

        Ok((destination_account, destination_account_data))
    }

    fn get_event_authority(&self) -> Result<Pubkey, Box<dyn std::error::Error>> {
        let event_authority = self.ctx.svm.get_pda(
            &[b"__event_authority"],
            &order_book::ID,
        );

        Ok(event_authority)
    }
}