mod instructions;

use anchor_litesvm::{AnchorContext, AnchorLiteSVM, Keypair, Pubkey, Signer, TestHelpers, get_anchor_account};
use anchor_lang::solana_program::instruction::Instruction;
use anchor_spl::{associated_token::get_associated_token_address, token::{TokenAccount, spl_token::state::Account}};
use std::{collections::HashMap, error::Error};

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
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Create the Anchor Lite SVM context with the order_book program loaded
        // Ensure that all the standard SPL programs are also loaded
        let mut ctx = AnchorLiteSVM::build_with_program(
            order_book::ID,
            include_bytes!("../../../../target/deploy/order_book.so"),
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

    fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
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
    ) -> Result<(Pubkey, order_book::state::OrderBookGlobal), Box<dyn Error>> {
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
    ) -> Result<(Pubkey, order_book::state::Order<order_book::state::NativeOrder>), Box<dyn Error>> {
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
    ) -> Result<(Pubkey, order_book::state::Order<order_book::state::ForeignOrder>), Box<dyn Error>> {
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
        sender: &Pubkey,
    ) -> Result<(Pubkey, order_book::state::Nonce), Box<dyn Error>> {
        let sender_nonce_account = self.ctx.svm.get_pda(
            &[
                order_book::state::NONCE_SEED_PREFIX,
                &sender.to_bytes(),
            ],
            &order_book::ID,
        );

        // We catch an error if the account does not exist
        // and return account data with nonce == 0 so it can 
        // be used in an open order transaction regardless
        // of whether or not it will be created in the txn
        let sender_nonce_data: order_book::state::Nonce = match get_anchor_account(&self.ctx.svm, &sender_nonce_account) {
            Ok(data) => data,
            Err(_) => order_book::state::Nonce {
                bump: 0u8, // this is invalid, but we don't use the bump offchain
                value: 0u64
            }
        };

        Ok((sender_nonce_account, sender_nonce_data))
    }

    fn get_destination_account(
        &self,
        dest_chain_id: u32,
    ) -> Result<(Pubkey, order_book::state::Destination), Box<dyn Error>> {
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

    fn get_event_authority(&self) -> Result<Pubkey, Box<dyn Error>> {
        let event_authority = self.ctx.svm.get_pda(
            &[b"__event_authority"],
            &order_book::ID,
        );

        Ok(event_authority)
    }

    fn get_token_account(&self, account: &Pubkey) -> Result<anchor_spl::token::TokenAccount, Box<dyn Error>> {
        // Get the account or return a default value if it doesn't exist
        // We use this to check token balances without failing 
        let token_account_data: anchor_spl::token::TokenAccount = match get_anchor_account(&self.ctx.svm, account) {
            Ok(data) => data,
            Err(_) => TokenAccount::default()
        };

        Ok(token_account_data)
    }

    fn get_token_balance(&self, account: &Pubkey) -> Result<u64, Box<dyn Error>> {
        let token_account = self.get_token_account(account)?;
        Ok(token_account.amount)
    }

    // Helpers for token actions
    fn approve_token_delegate(
        &mut self,
        token_mint: &str,
        owner: &str,
        delegate: &str,
        amount: u64,
    ) -> Result<(), Box<dyn Error>> {
        let owner_keypair = self.users.get(owner).unwrap();
        let token_account = self.atas.get(&(token_mint, owner)).unwrap();
        let delegate_pubkey = self.users.get(delegate).unwrap().pubkey();

        let ix = anchor_spl::token::spl_token::instruction::approve(
            &anchor_spl::token::ID,
            token_account,
            &delegate_pubkey,
            &owner_keypair.pubkey(),
            &[],
            amount,
        )?;

        self.ctx.execute_instruction(ix, &[owner_keypair])?;

        Ok(())
    }

    // Helpers to construct instructions
    fn create_open_order_ix(
        &self,
        sender: &Pubkey,
        token_in_mint: &Pubkey,
        sender_token_in_account: &Pubkey,
        token_authority: Option<&Pubkey>,
        order_params: &order_book::instructions::open::OrderParams
    ) -> Result<([u8; 32], Instruction), Box<dyn Error>> {
        let (global_account, global_data) = self.get_global_account().unwrap();
        let (sender_nonce_account, sender_nonce_data) = self.get_sender_nonce_account(sender).unwrap();

        let order_id = order_book::state::compute_order_id(&order_book::state::OrderData {
            version: order_book::constants::VERSION,
            sender: sender.to_bytes(),
            nonce: sender_nonce_data.value,
            origin_chain_id: global_data.chain_id,
            dest_chain_id: order_params.dest_chain_id,
            fill_deadline: order_params.fill_deadline,
            token_out: order_params.token_out,
            amount_in: order_params.amount_in as u128,
            amount_out: order_params.amount_out,
            recipient: order_params.recipient,
            solver: order_params.solver,
        });
        let order = self.ctx.svm.get_pda(
            &[
                order_book::state::ORDER_SEED_PREFIX,
                &order_id,
            ],
            &order_book::ID,
        );
        let order_token_in_ata = get_associated_token_address(&order, token_in_mint);

        let ix = self.ctx.program()
            .accounts(
                order_book::accounts::OpenOrder {
                    program: order_book::ID,
                    event_authority: self.get_event_authority()?,
                    payer: *sender,
                    token_authority: token_authority.cloned(),
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
                    params: order_params.clone(),
                }
            )
            .instruction()?;

        Ok((order_id, ix))
    }
}