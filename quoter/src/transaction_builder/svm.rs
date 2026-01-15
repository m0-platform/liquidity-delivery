use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use borsh::BorshSerialize;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    message::Message,
    pubkey::Pubkey,
    system_program,
    transaction::Transaction,
};
use spl_associated_token_account::get_associated_token_address_with_program_id;
use std::str::FromStr;

use super::error::TransactionBuilderError;
use super::order_id::OrderData;
use super::{OpenOrderInput, TransactionResult};

const DEFAULT_ORDER_BOOK_PROGRAM_ID: &str = "MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK";
const OPEN_ORDER_DISCRIMINATOR: [u8; 8] = [206, 88, 88, 143, 38, 136, 50, 224];

/// Seed prefixes matching the SVM program
const GLOBAL_SEED: &[u8] = b"global";
const NONCE_SEED_PREFIX: &[u8] = b"nonce";
const DESTINATION_SEED_PREFIX: &[u8] = b"destination";
const ORDER_SEED_PREFIX: &[u8] = b"order";
const EVENT_AUTHORITY_SEED: &[u8] = b"__event_authority";
const VERSION: u16 = 1;

#[derive(BorshSerialize)]
pub struct OrderParams {
    pub dest_chain_id: u32,
    pub fill_deadline: u64,
    pub token_out: [u8; 32],
    pub amount_in: u64,
    pub amount_out: u128,
    pub recipient: [u8; 32],
    pub solver: [u8; 32],
}

pub struct SvmTransactionBuilder {
    rpc_url: String,
    program_id: Pubkey,
    chain_id: u32,
}

impl SvmTransactionBuilder {
    pub fn new(
        rpc_url: String,
        program_id: Option<String>,
        chain_id: u32,
    ) -> Result<Self, TransactionBuilderError> {
        let program_id = program_id
            .map(|s| Pubkey::from_str(&s))
            .unwrap_or_else(|| Pubkey::from_str(DEFAULT_ORDER_BOOK_PROGRAM_ID))
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;

        Ok(Self {
            rpc_url,
            program_id,
            chain_id,
        })
    }

    /// Fetch the current nonce for a sender from the Nonce PDA
    pub async fn get_sender_nonce(&self, sender: &Pubkey) -> Result<u64, TransactionBuilderError> {
        let client = RpcClient::new(self.rpc_url.clone());

        let (nonce_pda, _) =
            Pubkey::find_program_address(&[NONCE_SEED_PREFIX, sender.as_ref()], &self.program_id);

        // Try to fetch the nonce account
        match client.get_account(&nonce_pda).await {
            Ok(account) => {
                if account.data.len() >= 17 {
                    let nonce = u64::from_le_bytes(
                        account.data[9..17]
                            .try_into()
                            .map_err(|_| TransactionBuilderError::AccountParseError)?,
                    );
                    Ok(nonce)
                } else {
                    Err(TransactionBuilderError::AccountParseError)
                }
            }
            Err(_) => {
                // Account doesn't exist yet, nonce is 0
                Ok(0)
            }
        }
    }

    async fn get_recent_blockhash(
        &self,
    ) -> Result<solana_sdk::hash::Hash, TransactionBuilderError> {
        let client = RpcClient::new(self.rpc_url.clone());
        let blockhash = client
            .get_latest_blockhash()
            .await
            .map_err(|e| TransactionBuilderError::RpcError(e.to_string()))?;
        Ok(blockhash)
    }

    pub async fn build_open_order_transaction(
        &self,
        input: &OpenOrderInput,
    ) -> Result<TransactionResult, TransactionBuilderError> {
        let sender = Pubkey::from_str(&input.sender_address)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;

        let token_in_mint = Pubkey::from_str(&input.token_in)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;

        let nonce = self.get_sender_nonce(&sender).await?;
        let blockhash = self.get_recent_blockhash().await?;
        let token_out = parse_bytes32_svm(&input.token_out)?;

        let order_data = OrderData {
            version: VERSION,
            sender: sender.to_bytes(),
            nonce,
            origin_chain_id: self.chain_id,
            dest_chain_id: input.dest_chain_id,
            fill_deadline: input.fill_deadline,
            token_in: token_in_mint.to_bytes(),
            token_out,
            amount_in: input.amount_in as u128,
            amount_out: input.amount_out,
            recipient: input.recipient,
            solver: input.solver,
        };
        let order_id = order_data.compute_order_id();

        // Derive all PDAs
        let (global_account, _) = Pubkey::find_program_address(&[GLOBAL_SEED], &self.program_id);

        let (nonce_pda, _) =
            Pubkey::find_program_address(&[NONCE_SEED_PREFIX, sender.as_ref()], &self.program_id);

        let (order_pda, _) =
            Pubkey::find_program_address(&[ORDER_SEED_PREFIX, &order_id], &self.program_id);

        let (event_authority, _) =
            Pubkey::find_program_address(&[EVENT_AUTHORITY_SEED], &self.program_id);

        // Destination PDA (optional - only if dest_chain_id != origin chain)
        let destination_account = if input.dest_chain_id != self.chain_id {
            let (dest_pda, _) = Pubkey::find_program_address(
                &[DESTINATION_SEED_PREFIX, &input.dest_chain_id.to_be_bytes()],
                &self.program_id,
            );
            Some(dest_pda)
        } else {
            None
        };

        let sender_token_in_account =
            get_associated_token_address_with_program_id(&sender, &token_in_mint, &spl_token::ID);

        let order_token_in_ata = get_associated_token_address_with_program_id(
            &order_pda,
            &token_in_mint,
            &spl_token::ID,
        );

        let order_params = OrderParams {
            dest_chain_id: input.dest_chain_id,
            fill_deadline: input.fill_deadline,
            token_out,
            amount_in: input.amount_in,
            amount_out: input.amount_out,
            recipient: input.recipient,
            solver: input.solver,
        };

        // Serialize instruction data: discriminator + params
        let mut data = Vec::with_capacity(8 + 128);
        data.extend_from_slice(&OPEN_ORDER_DISCRIMINATOR);
        order_params
            .serialize(&mut data)
            .map_err(|e| TransactionBuilderError::SerializationError(e.to_string()))?;

        // Build account metas
        let mut accounts = vec![AccountMeta::new(sender, true)];

        // token_authority - optional, we'll skip it (None case means payer is authority)
        accounts.push(AccountMeta::new_readonly(self.program_id, false));

        // global_account
        accounts.push(AccountMeta::new_readonly(global_account, false));

        // destination_account (optional)
        if let Some(dest) = destination_account {
            accounts.push(AccountMeta::new_readonly(dest, false));
        } else {
            // None placeholder
            accounts.push(AccountMeta::new_readonly(self.program_id, false));
        }

        // Remaining accounts
        accounts.extend_from_slice(&[
            AccountMeta::new_readonly(token_in_mint, false),
            AccountMeta::new(sender_token_in_account, false),
            AccountMeta::new(nonce_pda, false),
            AccountMeta::new(order_pda, false),
            AccountMeta::new(order_token_in_ata, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(self.program_id, false),
        ]);

        let instruction = Instruction {
            program_id: self.program_id,
            accounts,
            data,
        };

        // Build unsigned transaction
        let message = Message::new(&[instruction], Some(&sender));
        let mut transaction = Transaction::new_unsigned(message);
        transaction.message.recent_blockhash = blockhash;

        // Serialize transaction
        let serialized = bincode::serialize(&transaction)
            .map_err(|e| TransactionBuilderError::SerializationError(e.to_string()))?;

        Ok(TransactionResult {
            transaction: BASE64_STANDARD.encode(&serialized),
            order_id: format!("0x{}", hex::encode(order_id)),
            nonce,
            contract_address: self.program_id.to_string(),
        })
    }
}

/// Parse a string as [u8; 32], supporting various formats
fn parse_bytes32_svm(s: &str) -> Result<[u8; 32], TransactionBuilderError> {
    // Try Solana base58 pubkey first
    if let Ok(pubkey) = Pubkey::from_str(s) {
        return Ok(pubkey.to_bytes());
    }

    // Try hex (with or without 0x prefix)
    let s = s.strip_prefix("0x").unwrap_or(s);
    if s.len() == 64 {
        let bytes: [u8; 32] = hex::decode(s)
            .map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?
            .try_into()
            .map_err(|_| TransactionBuilderError::InvalidAddress("Invalid length".to_string()))?;
        return Ok(bytes);
    }

    // EVM address (20 bytes) - left-pad
    if s.len() == 40 {
        let mut bytes = [0u8; 32];
        let addr_bytes =
            hex::decode(s).map_err(|e| TransactionBuilderError::InvalidAddress(e.to_string()))?;
        bytes[12..].copy_from_slice(&addr_bytes);
        return Ok(bytes);
    }

    Err(TransactionBuilderError::InvalidAddress(format!(
        "Cannot parse: {}",
        s
    )))
}
