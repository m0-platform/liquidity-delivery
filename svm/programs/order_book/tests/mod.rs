mod instructions;

use anchor_lang::{prelude::declare_program, solana_program::instruction::Instruction};
use anchor_litesvm::{
    get_anchor_account, AnchorContext, AnchorLiteSVM, Keypair, Pubkey, Signer, TestHelpers,
};
use anchor_spl::{associated_token::get_associated_token_address, token::TokenAccount};
use order_book::{OrderData, GLOBAL_SEED, ORDER_SEED_PREFIX};
use std::{collections::HashMap, error::Error};

declare_program!(portal);

const LAMPORTS_PER_SOL: u64 = 1_000_000_000;
const INITIAL_FUNDS: u64 = 10 * LAMPORTS_PER_SOL;

const CHAIN_ID: u32 = 1; // Example chain ID for testing
const DEST_CHAIN_ID: u32 = 2; // Example destination chain ID for testing

struct OrderBookTest {
    pub ctx: AnchorContext,
    users: HashMap<&'static str, Keypair>,
    mints: HashMap<&'static str, Pubkey>,
    atas: HashMap<(&'static str, &'static str), Pubkey>,
}

impl OrderBookTest {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Create the Anchor Lite SVM context with the order_book program loaded
        // Ensure that all the standard SPL programs are also loaded
        let mut ctx = AnchorLiteSVM::build_with_programs(&[
            (
                order_book::ID,
                include_bytes!("../../../target/deploy/order_book.so"),
            ),
            (
                portal::ID,
                include_bytes!("../../../target/deploy/mock_portal.so"),
            ),
        ]);

        // Create users and fund them
        let mut users: HashMap<&str, Keypair> = HashMap::new();
        users.insert("admin", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert("alice", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert("bob", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert("carol", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert("solver", ctx.svm.create_funded_account(INITIAL_FUNDS)?);
        users.insert(
            "portal_authority",
            ctx.svm.create_funded_account(INITIAL_FUNDS)?,
        );

        // Create token mints with different decimal places
        // TODO anchor-litesvm doesn't have token2022 convenience methods yet
        let admin = users.get("admin").unwrap();
        let mut mints: HashMap<&str, Pubkey> = HashMap::new();
        mints.insert(
            "token-in-spl-6",
            ctx.svm.create_token_mint(admin, 6)?.pubkey(),
        );
        mints.insert(
            "token-out-spl-6",
            ctx.svm.create_token_mint(admin, 6)?.pubkey(),
        );
        mints.insert(
            "token-in-spl-9",
            ctx.svm.create_token_mint(admin, 9)?.pubkey(),
        );
        mints.insert(
            "token-out-spl-9",
            ctx.svm.create_token_mint(admin, 9)?.pubkey(),
        );

        // Create ATAs for alice and the solver and mint them tokens
        let alice = users.get("alice").unwrap();
        let carol = users.get("carol").unwrap();
        let solver = users.get("solver").unwrap();
        let mut atas: HashMap<(&str, &str), Pubkey> = HashMap::new();

        for (token_name, token_mint) in mints.iter() {
            let alice_ata = ctx.svm.create_associated_token_account(token_mint, alice)?;
            let carol_ata = ctx.svm.create_associated_token_account(token_mint, carol)?;
            let solver_ata = ctx
                .svm
                .create_associated_token_account(token_mint, solver)?;

            atas.insert((token_name, "alice"), alice_ata);
            atas.insert((token_name, "carol"), carol_ata);
            atas.insert((token_name, "solver"), solver_ata);

            ctx.svm
                .mint_to(token_mint, &alice_ata, admin, INITIAL_FUNDS)?;

            ctx.svm
                .mint_to(token_mint, &solver_ata, admin, INITIAL_FUNDS)?;
        }

        Ok(Self {
            ctx,
            users,
            mints,
            atas,
        })
    }

    fn initialize(&mut self) -> Result<(), Box<dyn Error>> {
        let admin = self.users.get("admin").unwrap();
        let portal_authority = self.users.get("portal_authority").unwrap();

        // Initialize the order book global account
        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::Initialize {
                admin: admin.pubkey(),
                global_account: self
                    .ctx
                    .svm
                    .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(order_book::instruction::Initialize {
                chain_id: CHAIN_ID,
                portal_authority: portal_authority.pubkey(),
            })
            .instruction()?;

        self.ctx.execute_instruction(ix, &[admin])?;

        // Add the destination chain configuration
        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::AddDestination {
                program: order_book::ID,
                event_authority: self.get_event_authority()?,
                admin: admin.pubkey(),
                global_account: self
                    .ctx
                    .svm
                    .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID),
                destination_account: self.ctx.svm.get_pda(
                    &[
                        order_book::state::DESTINATION_SEED_PREFIX,
                        &DEST_CHAIN_ID.to_le_bytes(),
                    ],
                    &order_book::ID,
                ),
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(order_book::instruction::AddDestination {
                dest_chain_id: DEST_CHAIN_ID,
            })
            .instruction()?;

        self.ctx.execute_instruction(ix, &[admin])?;

        Ok(())
    }

    // Helpers to fetch program accounts

    fn get_global_account(
        &self,
    ) -> Result<(Pubkey, order_book::state::OrderBookGlobal), Box<dyn Error>> {
        let global_account = self
            .ctx
            .svm
            .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);

        let global_account_data: order_book::state::OrderBookGlobal =
            get_anchor_account(&self.ctx.svm, &global_account)?;

        Ok((global_account, global_account_data))
    }

    fn get_native_order_account(
        &self,
        order_id: &[u8; 32],
    ) -> Result<
        (
            Pubkey,
            order_book::state::Order<order_book::state::NativeOrder>,
        ),
        Box<dyn Error>,
    > {
        let order_account = self.ctx.svm.get_pda(
            &[order_book::state::ORDER_SEED_PREFIX, order_id],
            &order_book::ID,
        );

        let order_account_data: order_book::state::Order<order_book::state::NativeOrder> =
            get_anchor_account(&self.ctx.svm, &order_account)?;

        Ok((order_account, order_account_data))
    }

    fn get_foreign_order_account(
        &self,
        order_id: &[u8; 32],
    ) -> Result<
        (
            Pubkey,
            order_book::state::Order<order_book::state::ForeignOrder>,
        ),
        Box<dyn Error>,
    > {
        let order_account = self.ctx.svm.get_pda(
            &[order_book::state::ORDER_SEED_PREFIX, order_id],
            &order_book::ID,
        );

        let order_account_data: order_book::state::Order<order_book::state::ForeignOrder> =
            get_anchor_account(&self.ctx.svm, &order_account)?;

        Ok((order_account, order_account_data))
    }

    fn get_sender_nonce_account(
        &self,
        sender: &Pubkey,
    ) -> Result<(Pubkey, order_book::state::Nonce), Box<dyn Error>> {
        let sender_nonce_account = self.ctx.svm.get_pda(
            &[order_book::state::NONCE_SEED_PREFIX, &sender.to_bytes()],
            &order_book::ID,
        );

        // We catch an error if the account does not exist
        // and return account data with nonce == 0 so it can
        // be used in an open order transaction regardless
        // of whether or not it will be created in the txn
        let sender_nonce_data: order_book::state::Nonce =
            match get_anchor_account(&self.ctx.svm, &sender_nonce_account) {
                Ok(data) => data,
                Err(_) => order_book::state::Nonce {
                    bump: 0u8, // this is invalid, but we don't use the bump offchain
                    value: 0u64,
                },
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
                &dest_chain_id.to_le_bytes(),
            ],
            &order_book::ID,
        );

        let destination_account_data: order_book::state::Destination =
            get_anchor_account(&self.ctx.svm, &destination_account)?;

        Ok((destination_account, destination_account_data))
    }

    fn get_event_authority(&self) -> Result<Pubkey, Box<dyn Error>> {
        let event_authority = self
            .ctx
            .svm
            .get_pda(&[b"__event_authority"], &order_book::ID);

        Ok(event_authority)
    }

    fn get_token_account(
        &self,
        account: &Pubkey,
    ) -> Result<anchor_spl::token::TokenAccount, Box<dyn Error>> {
        // Get the account or return a default value if it doesn't exist
        // We use this to check token balances without failing
        let token_account_data: anchor_spl::token::TokenAccount =
            match get_anchor_account(&self.ctx.svm, account) {
                Ok(data) => data,
                Err(_) => TokenAccount::default(),
            };

        Ok(token_account_data)
    }

    fn get_token_balance(&self, account: &Pubkey) -> Result<u64, Box<dyn Error>> {
        let token_account = self.get_token_account(account)?;
        Ok(token_account.amount)
    }

    // Helpers to fetch test data
    fn get_user(&self, user: &str) -> Keypair {
        self.users.get(user).unwrap().insecure_clone()
    }

    fn get_mint(&self, mint: &str) -> Pubkey {
        self.mints.get(mint).unwrap().clone()
    }

    fn get_ata(&self, mint: &str, owner: &str) -> Pubkey {
        self.atas.get(&(mint, owner)).unwrap().clone()
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

    fn create_associated_token_account(
        &mut self,
        token_mint: &Pubkey,
        owner: &Pubkey,
    ) -> Result<Pubkey, Box<dyn Error>> {
        // Modified from litesvm-utils to allow passing owners that are off the curve
        let payer = self.get_user("admin");

        let ata = get_associated_token_address(owner, token_mint);

        // Create ATA instruction
        let create_ata_ix = anchor_spl::associated_token::spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            owner,
            token_mint,
            &anchor_spl::token::ID,
        );

        // Send transaction
        self.ctx.execute_instruction(create_ata_ix, &[&payer])?;

        Ok(ata)
    }

    /// Mint tokens to any ATA (used for simulating dust/griefing donations)
    fn mint_to_ata(
        &mut self,
        token_name: &str,
        ata: &Pubkey,
        amount: u64,
    ) -> Result<(), Box<dyn Error>> {
        let admin = self.get_user("admin");
        let mint = self.get_mint(token_name);
        self.ctx.svm.mint_to(&mint, ata, &admin, amount)?;
        Ok(())
    }

    // Helpers to construct account objects to pass to instructions
    fn build_fill_native_order_accounts(
        &self,
        solver: &Pubkey,
        order_id: [u8; 32],
    ) -> Result<order_book::accounts::FillNativeOrder, Box<dyn Error>> {
        let (order_account, native_order_data) = self.get_native_order_account(&order_id)?;
        let (global_account, _) = self.get_global_account()?;

        let token_out_mint = Pubkey::new_from_array(native_order_data.data.token_out);
        let recipient = Pubkey::new_from_array(native_order_data.data.recipient);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);
        let solver_token_out_ata = get_associated_token_address(solver, &token_out_mint);
        let token_in_mint = native_order_data.data.token_in;
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);
        let solver_token_in_ata = get_associated_token_address(solver, &token_in_mint);

        Ok(order_book::accounts::FillNativeOrder {
            program: order_book::ID,
            event_authority: self.get_event_authority()?,
            solver: *solver,
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
        })
    }

    fn build_fill_native_order_accounts_from_order_data(
        &self,
        solver: &Pubkey,
        order_data: &OrderData,
    ) -> Result<order_book::accounts::FillNativeOrder, Box<dyn Error>> {
        let order_account = self.ctx.svm.get_pda(
            &[ORDER_SEED_PREFIX, order_data.compute_order_id().as_slice()],
            &order_book::ID,
        );
        let (global_account, _) = self.get_global_account()?;

        let token_out_mint = Pubkey::new_from_array(order_data.token_out);
        let recipient = Pubkey::new_from_array(order_data.recipient);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);
        let solver_token_out_ata = get_associated_token_address(solver, &token_out_mint);
        let token_in_mint = Pubkey::new_from_array(order_data.token_in);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);
        let solver_token_in_ata = get_associated_token_address(solver, &token_in_mint);

        Ok(order_book::accounts::FillNativeOrder {
            program: order_book::ID,
            event_authority: self.get_event_authority()?,
            solver: *solver,
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
        })
    }

    fn build_fill_foreign_order_accounts_from_order_data(
        &self,
        solver: &Pubkey,
        order_data: &OrderData,
    ) -> Result<order_book::accounts::FillForeignOrder, Box<dyn Error>> {
        let order_account = self.ctx.svm.get_pda(
            &[ORDER_SEED_PREFIX, order_data.compute_order_id().as_slice()],
            &order_book::ID,
        );
        let (global_account, _) = self.get_global_account()?;

        let token_out_mint = Pubkey::new_from_array(order_data.token_out);
        let recipient = Pubkey::new_from_array(order_data.recipient);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);
        let solver_token_out_ata = get_associated_token_address(solver, &token_out_mint);

        Ok(order_book::accounts::FillForeignOrder {
            program: order_book::ID,
            event_authority: self.get_event_authority()?,
            solver: *solver,
            global_account,
            token_out_mint,
            solver_token_out_account: solver_token_out_ata,
            recipient,
            recipient_token_out_ata,
            token_out_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
            order: order_account,
            portal_program: portal::ID, // the following accounts are not checked by the mock
            portal_global: self.ctx.svm.get_pda(&[GLOBAL_SEED], &portal::ID),
            portal_authority: self.ctx.svm.get_pda(&[b"authority"], &portal::ID),
            bridge_adapter: self.ctx.svm.get_pda(&[b"bridge_adapter"], &portal::ID),
        })
    }

    fn build_cancel_native_order_accounts(
        &self,
        signer: &Pubkey,
        sender: &Pubkey,
        order_id: [u8; 32],
    ) -> Result<order_book::accounts::CancelNativeOrder, Box<dyn Error>> {
        let (global_account, _) = self.get_global_account()?;
        let order_account = self
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);
        let (_, native_order_data) = self.get_native_order_account(&order_id)?;

        let token_in_mint = native_order_data.data.token_in;
        let sender_token_in_ata = get_associated_token_address(sender, &token_in_mint);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

        Ok(order_book::accounts::CancelNativeOrder {
            program: order_book::ID,
            event_authority: self.get_event_authority()?,
            signer: *signer,
            sender: *sender,
            global_account,
            order: order_account,
            token_in_mint,
            sender_token_in_ata,
            order_token_in_ata,
            token_in_program: anchor_spl::token::ID,
        })
    }

    fn build_cancel_foreign_order_accounts(
        &self,
        signer: &Pubkey,
        order_data: &OrderData,
    ) -> Result<order_book::accounts::CancelForeignOrder, Box<dyn Error>> {
        let (global_account, _) = self.get_global_account()?;
        let order_id = order_data.compute_order_id();
        let order_account = self
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &order_id], &order_book::ID);

        Ok(order_book::accounts::CancelForeignOrder {
            program: order_book::ID,
            event_authority: self.get_event_authority()?,
            signer: *signer,
            global_account,
            order: order_account,
            portal_program: portal::ID,
            portal_global: self.ctx.svm.get_pda(&[GLOBAL_SEED], &portal::ID),
            portal_authority: self.ctx.svm.get_pda(&[b"authority"], &portal::ID),
            bridge_adapter: self.ctx.svm.get_pda(&[b"bridge_adapter"], &portal::ID),
            system_program: anchor_lang::solana_program::system_program::ID,
        })
    }

    fn build_report_cancel_accounts(
        &self,
        relayer: &Pubkey,
        portal_authority: &Pubkey,
        cancel_report: &order_book::instructions::CancelReport,
    ) -> Result<order_book::accounts::ReportOrderCancel, Box<dyn Error>> {
        let (global_account, _) = self.get_global_account()?;
        let order_account = self.ctx.svm.get_pda(
            &[ORDER_SEED_PREFIX, &cancel_report.order_id],
            &order_book::ID,
        );
        let (_, native_order_data) = self.get_native_order_account(&cancel_report.order_id)?;

        let token_in_mint = native_order_data.data.token_in;
        let order_sender = native_order_data.data.sender;
        let sender_token_in_ata = get_associated_token_address(&order_sender, &token_in_mint);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

        Ok(order_book::accounts::ReportOrderCancel {
            program: order_book::ID,
            event_authority: self.get_event_authority()?,
            relayer: *relayer,
            portal_authority: *portal_authority,
            global_account,
            order: order_account,
            token_in_mint,
            order_sender,
            sender_token_in_ata,
            order_token_in_ata,
            token_in_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        })
    }

    fn build_report_fill_accounts(
        &self,
        relayer: &Pubkey,
        portal_authority: &Pubkey,
        fill_report: &order_book::instructions::FillReport,
    ) -> Result<order_book::accounts::ReportOrderFill, Box<dyn Error>> {
        let (global_account, _) = self.get_global_account()?;
        let order_account = self
            .ctx
            .svm
            .get_pda(&[ORDER_SEED_PREFIX, &fill_report.order_id], &order_book::ID);
        let (_, native_order_data) = self.get_native_order_account(&fill_report.order_id)?;

        let token_in_mint = native_order_data.data.token_in;
        let origin_recipient = Pubkey::new_from_array(fill_report.origin_recipient);
        let recipient_token_in_ata =
            get_associated_token_address(&origin_recipient, &token_in_mint);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

        Ok(order_book::accounts::ReportOrderFill {
            program: order_book::ID,
            event_authority: self.get_event_authority()?,
            relayer: *relayer,
            portal_authority: *portal_authority,
            global_account,
            order: order_account,
            token_in_mint,
            origin_recipient,
            recipient_token_in_ata,
            order_token_in_ata,
            token_in_program: anchor_spl::token::ID,
            associated_token_program: anchor_spl::associated_token::ID,
            system_program: anchor_lang::solana_program::system_program::ID,
        })
    }

    // Helpers to construct instructions
    fn create_initialize_ix(
        &self,
        admin: &Pubkey,
        chain_id: u32,
        portal_authority: &Pubkey,
    ) -> Result<Instruction, Box<dyn Error>> {
        let global_account = self
            .ctx
            .svm
            .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::Initialize {
                admin: *admin,
                global_account,
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(order_book::instruction::Initialize {
                chain_id,
                portal_authority: *portal_authority,
            })
            .instruction()?;

        Ok(ix)
    }

    fn create_open_order_ix(
        &self,
        sender: &Pubkey,
        token_in_mint: &Pubkey,
        sender_token_in_account: &Pubkey,
        token_authority: Option<&Pubkey>,
        order_params: &order_book::instructions::open::OrderParams,
    ) -> Result<([u8; 32], Instruction), Box<dyn Error>> {
        let (global_account, global_data) = self.get_global_account().unwrap();
        let (sender_nonce_account, sender_nonce_data) =
            self.get_sender_nonce_account(sender).unwrap();

        let destination_account = if order_params.dest_chain_id != global_data.chain_id {
            Some(self.ctx.svm.get_pda(
                &[
                    order_book::state::DESTINATION_SEED_PREFIX,
                    &order_params.dest_chain_id.to_le_bytes(),
                ],
                &order_book::ID,
            ))
        } else {
            None
        };

        let order_id = order_book::state::compute_order_id(&order_book::state::OrderData {
            version: order_book::constants::VERSION,
            sender: sender.to_bytes(),
            nonce: sender_nonce_data.value,
            origin_chain_id: global_data.chain_id,
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
        let order = self.ctx.svm.get_pda(
            &[order_book::state::ORDER_SEED_PREFIX, &order_id],
            &order_book::ID,
        );
        let order_token_in_ata = get_associated_token_address(&order, token_in_mint);

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::OpenOrder {
                program: order_book::ID,
                event_authority: self.get_event_authority()?,
                payer: *sender,
                token_authority: token_authority.cloned(),
                global_account,
                destination_account,
                token_in_mint: *token_in_mint,
                sender_token_in_account: *sender_token_in_account,
                sender_nonce_account,
                order,
                order_token_in_ata,
                token_in_program: anchor_spl::token::ID,
                associated_token_program: anchor_spl::associated_token::ID,
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(order_book::instruction::OpenOrder {
                params: order_params.clone(),
            })
            .instruction()?;

        Ok((order_id, ix))
    }

    fn create_add_destination_ix(
        &self,
        admin: &Pubkey,
        dest_chain_id: u32,
    ) -> Result<Instruction, Box<dyn Error>> {
        let global_account = self
            .ctx
            .svm
            .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);
        let destination_account = self.ctx.svm.get_pda(
            &[
                order_book::state::DESTINATION_SEED_PREFIX,
                &dest_chain_id.to_le_bytes(),
            ],
            &order_book::ID,
        );

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::AddDestination {
                program: order_book::ID,
                event_authority: self.get_event_authority()?,
                admin: *admin,
                global_account,
                destination_account,
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(order_book::instruction::AddDestination { dest_chain_id })
            .instruction()?;

        Ok(ix)
    }

    fn create_remove_destination_ix(
        &self,
        admin: &Pubkey,
        dest_chain_id: u32,
    ) -> Result<Instruction, Box<dyn Error>> {
        let global_account = self
            .ctx
            .svm
            .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);
        let destination_account = self.ctx.svm.get_pda(
            &[
                order_book::state::DESTINATION_SEED_PREFIX,
                &dest_chain_id.to_le_bytes(),
            ],
            &order_book::ID,
        );

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::RemoveDestination {
                program: order_book::ID,
                event_authority: self.get_event_authority()?,
                admin: *admin,
                global_account,
                destination_account,
                system_program: anchor_lang::solana_program::system_program::ID,
            })
            .args(order_book::instruction::RemoveDestination { dest_chain_id })
            .instruction()?;

        Ok(ix)
    }

    fn create_fill_native_order_ix(
        &self,
        solver: &Pubkey,
        order_id: [u8; 32],
        fill_params: &order_book::instructions::fill::FillParams,
    ) -> Result<Instruction, Box<dyn Error>> {
        let accounts = self.build_fill_native_order_accounts(solver, order_id)?;

        let (_, native_order_data) = self.get_native_order_account(&order_id)?;
        let (_, global_data) = self.get_global_account()?;

        let order_data =
            OrderData::new_from_native_order(native_order_data.data, global_data.chain_id);

        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillNativeOrder {
                order_id,
                order_data: Box::new(order_data),
                fill_params: fill_params.clone(),
            })
            .instruction()?;

        Ok(ix)
    }

    fn create_fill_foreign_order_ix(
        &self,
        solver: &Pubkey,
        order_data: &OrderData,
        fill_params: &order_book::instructions::fill::FillParams,
    ) -> Result<Instruction, Box<dyn Error>> {
        let accounts =
            self.build_fill_foreign_order_accounts_from_order_data(solver, order_data)?;
        let order_id = order_data.compute_order_id();

        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::FillForeignOrder {
                order_id,
                order_data: order_data.clone(),
                fill_params: fill_params.clone(),
            })
            .instruction()?;

        Ok(ix)
    }

    fn create_cancel_native_order_ix(
        &self,
        signer: &Pubkey,
        sender: &Pubkey,
        order_id: [u8; 32],
    ) -> Result<Instruction, Box<dyn Error>> {
        let accounts = self.build_cancel_native_order_accounts(signer, sender, order_id)?;

        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::CancelNativeOrder { order_id })
            .instruction()?;

        Ok(ix)
    }

    fn create_cancel_native_order_ix_with_custom_accounts(
        &self,
        accounts: order_book::accounts::CancelNativeOrder,
        order_id: [u8; 32],
    ) -> Result<Instruction, Box<dyn Error>> {
        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::CancelNativeOrder { order_id })
            .instruction()?;

        Ok(ix)
    }

    fn create_cancel_foreign_order_ix(
        &self,
        signer: &Pubkey,
        order_data: &OrderData,
    ) -> Result<Instruction, Box<dyn Error>> {
        let accounts = self.build_cancel_foreign_order_accounts(signer, order_data)?;
        let order_id = order_data.compute_order_id();

        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::CancelForeignOrder {
                order_id,
                order_data: order_data.clone(),
            })
            .instruction()?;

        Ok(ix)
    }

    fn create_cancel_foreign_order_ix_with_custom_accounts(
        &self,
        accounts: order_book::accounts::CancelForeignOrder,
        order_id: [u8; 32],
        order_data: OrderData,
    ) -> Result<Instruction, Box<dyn Error>> {
        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::CancelForeignOrder {
                order_id,
                order_data,
            })
            .instruction()?;

        Ok(ix)
    }

    fn create_report_cancel_ix(
        &self,
        relayer: &Pubkey,
        portal_authority: &Pubkey,
        source_chain_id: u32,
        cancel_report: &order_book::instructions::CancelReport,
    ) -> Result<Instruction, Box<dyn Error>> {
        let accounts =
            self.build_report_cancel_accounts(relayer, portal_authority, cancel_report)?;

        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::ReportOrderCancel {
                source_chain_id,
                cancel_report: cancel_report.clone(),
            })
            .instruction()?;

        Ok(ix)
    }

    fn create_report_fill_ix(
        &self,
        relayer: &Pubkey,
        portal_authority: &Pubkey,
        source_chain_id: u32,
        fill_report: &order_book::instructions::FillReport,
    ) -> Result<Instruction, Box<dyn Error>> {
        let accounts = self.build_report_fill_accounts(relayer, portal_authority, fill_report)?;

        let ix = self
            .ctx
            .program()
            .accounts(accounts)
            .args(order_book::instruction::ReportOrderFill {
                source_chain_id,
                fill_report: fill_report.clone(),
            })
            .instruction()?;

        Ok(ix)
    }

    // Helpers to quickly execute instructions on the program

    fn open_order(
        &mut self,
        sender: &str,
        token_in_mint: &str,
        order_params: &order_book::instructions::open::OrderParams,
    ) -> Result<[u8; 32], Box<dyn Error>> {
        let sender_keypair = self.users.get(sender).unwrap();
        let token_in_mint_pubkey = self.mints.get(token_in_mint).unwrap();
        let sender_token_in_account = self.atas.get(&(token_in_mint, sender)).unwrap();

        let (order_id, ix) = self.create_open_order_ix(
            &sender_keypair.pubkey(),
            token_in_mint_pubkey,
            sender_token_in_account,
            None,
            order_params,
        )?;

        self.ctx
            .execute_instruction(ix, &[sender_keypair])?
            .assert_success();

        Ok(order_id)
    }

    fn add_destination(&mut self, dest_chain_id: u32) -> Result<(), Box<dyn Error>> {
        let admin_keypair = self.users.get("admin").unwrap();

        let ix = self.create_add_destination_ix(&admin_keypair.pubkey(), dest_chain_id)?;

        self.ctx
            .execute_instruction(ix, &[admin_keypair])?
            .assert_success();

        Ok(())
    }

    fn remove_destination(&mut self, dest_chain_id: u32) -> Result<(), Box<dyn Error>> {
        let admin_keypair = self.users.get("admin").unwrap();

        let ix = self.create_remove_destination_ix(&admin_keypair.pubkey(), dest_chain_id)?;

        self.ctx
            .execute_instruction(ix, &[admin_keypair])?
            .assert_success();

        Ok(())
    }

    fn fill_native_order(
        &mut self,
        solver: &str,
        order_id: [u8; 32],
        amount_out_to_fill: u64,
    ) -> Result<(), Box<dyn Error>> {
        let solver_keypair = self.users.get(solver).unwrap();

        let fill_params = order_book::instructions::FillParams {
            amount_out_to_fill,
            origin_recipient: solver_keypair.pubkey().to_bytes(),
        };

        let ix =
            self.create_fill_native_order_ix(&solver_keypair.pubkey(), order_id, &fill_params)?;

        self.ctx
            .execute_instruction(ix, &[solver_keypair])?
            .assert_success();

        Ok(())
    }

    fn fill_foreign_order(
        &mut self,
        solver: &str,
        order_data: &OrderData,
        amount_out_to_fill: u64,
    ) -> Result<(), Box<dyn Error>> {
        let solver_keypair = self.users.get(solver).unwrap();

        let fill_params = order_book::instructions::FillParams {
            amount_out_to_fill,
            origin_recipient: solver_keypair.pubkey().to_bytes(),
        };

        let ix =
            self.create_fill_foreign_order_ix(&solver_keypair.pubkey(), order_data, &fill_params)?;

        self.ctx
            .execute_instruction(ix, &[solver_keypair])?
            .assert_success();

        Ok(())
    }

    fn report_fill(
        &mut self,
        relayer: &str,
        source_chain_id: u32,
        fill_report: &order_book::instructions::FillReport,
    ) -> Result<(), Box<dyn Error>> {
        let relayer_keypair = self.users.get(relayer).unwrap();
        let portal_authority = self.users.get("portal_authority").unwrap();

        let ix = self.create_report_fill_ix(
            &relayer_keypair.pubkey(),
            &portal_authority.pubkey(),
            source_chain_id,
            fill_report,
        )?;

        self.ctx
            .execute_instruction(ix, &[relayer_keypair, portal_authority])?
            .assert_success();
        Ok(())
    }

    fn cancel_native_order(
        &mut self,
        signer: &str,
        sender: &str,
        order_id: [u8; 32],
    ) -> Result<(), Box<dyn Error>> {
        let signer_keypair = self.users.get(signer).unwrap();
        let sender_keypair = self.users.get(sender).unwrap();

        let ix = self.create_cancel_native_order_ix(
            &signer_keypair.pubkey(),
            &sender_keypair.pubkey(),
            order_id,
        )?;

        self.ctx
            .execute_instruction(ix, &[signer_keypair])?
            .assert_success();

        Ok(())
    }

    fn cancel_foreign_order(
        &mut self,
        signer: &str,
        order_data: &OrderData,
    ) -> Result<(), Box<dyn Error>> {
        let signer_keypair = self.users.get(signer).unwrap();

        let ix = self.create_cancel_foreign_order_ix(&signer_keypair.pubkey(), order_data)?;

        self.ctx
            .execute_instruction(ix, &[signer_keypair])?
            .assert_success();

        Ok(())
    }

    fn report_cancel(
        &mut self,
        relayer: &str,
        source_chain_id: u32,
        cancel_report: &order_book::instructions::CancelReport,
    ) -> Result<(), Box<dyn Error>> {
        let relayer_keypair = self.users.get(relayer).unwrap();
        let portal_authority = self.users.get("portal_authority").unwrap();

        let ix = self.create_report_cancel_ix(
            &relayer_keypair.pubkey(),
            &portal_authority.pubkey(),
            source_chain_id,
            cancel_report,
        )?;

        self.ctx
            .execute_instruction(ix, &[relayer_keypair, portal_authority])?
            .assert_success();

        Ok(())
    }

    fn create_close_order_token_account_ix(
        &self,
        payer: &Pubkey,
        order_id: [u8; 32],
    ) -> Result<Instruction, Box<dyn Error>> {
        let (order_account, native_order_data) = self.get_native_order_account(&order_id)?;
        let token_in_mint = native_order_data.data.token_in;
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);
        let sender_token_ata =
            get_associated_token_address(&native_order_data.data.sender, &token_in_mint);

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::CloseOrderTokenAccount {
                sender: native_order_data.data.sender.into(),
                payer: *payer,
                order: order_account,
                token_in_mint,
                recipient_token_account: Some(sender_token_ata),
                order_token_in_ata,
                token_in_program: anchor_spl::token::ID,
            })
            .args(order_book::instruction::CloseOrderTokenAccount { order_id })
            .instruction()?;

        Ok(ix)
    }

    fn close_order_token_account(
        &mut self,
        payer_name: &str,
        order_id: [u8; 32],
    ) -> Result<(), Box<dyn Error>> {
        let payer = self.get_user(payer_name);
        let ix = self.create_close_order_token_account_ix(&payer.pubkey(), order_id)?;

        self.ctx
            .execute_instruction(ix, &[&payer])?
            .assert_success();

        Ok(())
    }

    /// Create close instruction with custom payer/sender (for testing validation errors)
    fn create_close_order_token_account_ix_custom(
        &self,
        payer: &Pubkey,
        sender: &Pubkey,
        order_id: [u8; 32],
        include_recipient: bool,
    ) -> Result<Instruction, Box<dyn Error>> {
        let (order_account, native_order_data) = self.get_native_order_account(&order_id)?;
        let token_in_mint = native_order_data.data.token_in;
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);
        let sender_token_ata = get_associated_token_address(sender, &token_in_mint);

        let recipient_token_account = if include_recipient {
            Some(sender_token_ata)
        } else {
            None
        };

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::CloseOrderTokenAccount {
                sender: *sender,
                payer: *payer,
                order: order_account,
                token_in_mint,
                recipient_token_account,
                order_token_in_ata,
                token_in_program: anchor_spl::token::ID,
            })
            .args(order_book::instruction::CloseOrderTokenAccount { order_id })
            .instruction()?;

        Ok(ix)
    }

    // Convenience functions for common test actions can go here

    fn warp_forward(&mut self, seconds: u64) {
        let mut clock = self.ctx.svm.get_sysvar::<anchor_lang::prelude::Clock>();
        clock.unix_timestamp += seconds as i64;
        // clock.slot += 1; // increment slot as well
        self.ctx.svm.set_sysvar(&clock);
    }

    fn current_time(&self) -> u64 {
        let clock = self.ctx.svm.get_sysvar::<anchor_lang::prelude::Clock>();
        clock.unix_timestamp as u64
    }

    // Pause/unpause helpers

    fn create_pause_ix(&self, admin: &Pubkey) -> Result<Instruction, Box<dyn Error>> {
        let global_account = self
            .ctx
            .svm
            .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::AdminInstruction {
                admin: *admin,
                global_account,
            })
            .args(order_book::instruction::Pause {})
            .instruction()?;

        Ok(ix)
    }

    fn create_unpause_ix(&self, admin: &Pubkey) -> Result<Instruction, Box<dyn Error>> {
        let global_account = self
            .ctx
            .svm
            .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::AdminInstruction {
                admin: *admin,
                global_account,
            })
            .args(order_book::instruction::Unpause {})
            .instruction()?;

        Ok(ix)
    }

    fn create_set_portal_authority_ix(
        &self,
        admin: &Pubkey,
        portal_authority: &Pubkey,
    ) -> Result<Instruction, Box<dyn Error>> {
        let global_account = self
            .ctx
            .svm
            .get_pda(&[order_book::state::GLOBAL_SEED], &order_book::ID);

        let ix = self
            .ctx
            .program()
            .accounts(order_book::accounts::AdminInstruction {
                admin: *admin,
                global_account,
            })
            .args(order_book::instruction::SetPortalAuthority {
                portal_authority: *portal_authority,
            })
            .instruction()?;

        Ok(ix)
    }

    fn pause(&mut self) -> Result<(), Box<dyn Error>> {
        let admin_keypair = self.users.get("admin").unwrap();
        let ix = self.create_pause_ix(&admin_keypair.pubkey())?;

        self.ctx
            .execute_instruction(ix, &[admin_keypair])?
            .assert_success();

        Ok(())
    }

    fn unpause(&mut self) -> Result<(), Box<dyn Error>> {
        let admin_keypair = self.users.get("admin").unwrap();
        let ix = self.create_unpause_ix(&admin_keypair.pubkey())?;

        self.ctx
            .execute_instruction(ix, &[admin_keypair])?
            .assert_success();

        Ok(())
    }
}
