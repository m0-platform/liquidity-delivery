use crate::components::ComponentParams;
use crate::config::{self, ChainConfig};
use crate::error::{Result, SolverError};
use crate::events::{EventHandler, EventProcessor, FillOrderSuccessfulEvent, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::OrderStore;
use crate::utils::{
    anchor_discriminator, chain_runtime, decode_address, decode_evm_address, decode_order_id,
    find_pda, PORTAL_PROGRAM_ID, WORMHOLE_ADAPTER,
};
use alloy::primitives::Address;
use anchor_client::anchor_lang::AnchorSerialize;
use anchor_client::solana_sdk::address_lookup_table::state::AddressLookupTable;
use anchor_client::solana_sdk::{
    address_lookup_table::AddressLookupTableAccount,
    instruction::{AccountMeta, Instruction},
    message::{v0, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::VersionedTransaction,
};
use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD, Engine};
use m0_liquidity_sdk::types::ChainRuntime;
use m0_portal_common::{
    build_relay_instruction, get_wormhole_chain_id, wormhole, WormholeRemainingAccounts,
};
use slog::{error, info, Logger};
use spl_associated_token_account::{
    get_associated_token_address_with_program_id,
    instruction::create_associated_token_account_idempotent,
};
use std::str::FromStr;
use std::sync::Arc;

pub struct SvmWriter {
    order_store: Arc<OrderStore>,
    provider_manager: Arc<ProviderManager>,
    chains: Vec<ChainConfig>,
    keypair: Arc<Keypair>,
    evm_solver: Address,
    logger: Logger,
    network: config::Network,
}

impl SvmWriter {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            order_store: Arc::new(OrderStore::new()),
            provider_manager: params.provider_manager.clone(),
            chains: params.config.chains.clone(),
            keypair: params.config.signers.svm_keypair(),
            evm_solver: params.config.signers.evm_address(),
            logger: params.logger.new(slog::o!("component" => "SvmWriter")),
            network: params.config.network.clone(),
        }
    }

    fn get_order_book_program_id(&self, chain_id: u32) -> Result<Pubkey> {
        self.chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .map(|c| Pubkey::from_str(&c.order_book_address).unwrap())
            .ok_or_else(|| {
                SolverError::Component("Order book program ID not found for chain".to_string())
            })
    }

    fn get_destination_wormhole_adapter(&self, chain_id: u32) -> Result<[u8; 32]> {
        let config = self
            .chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .ok_or_else(|| {
                SolverError::Component("Destination chain config not found".to_string())
            })?;

        decode_address(config.wormhole_adapter.clone(), chain_id)
            .ok_or_else(|| SolverError::Component("Failed to decode wormhole adapter".to_string()))
    }

    fn get_lut_address(&self, chain_id: u32) -> Option<Pubkey> {
        self.chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .and_then(|c| c.lut_address.as_ref())
            .and_then(|addr| Pubkey::from_str(addr).ok())
    }

    async fn fetch_lut_account(
        &self,
        rpc_client: &solana_client::nonblocking::rpc_client::RpcClient,
        lut_address: &Pubkey,
    ) -> Result<AddressLookupTableAccount> {
        let account = rpc_client
            .get_account(lut_address)
            .await
            .map_err(|e| SolverError::Component(format!("Failed to fetch LUT account: {}", e)))?;

        let lookup_table = AddressLookupTableAccount {
            key: *lut_address,
            addresses: AddressLookupTable::deserialize(&account.data)
                .map_err(|e| SolverError::Component(format!("Failed to deserialize LUT: {}", e)))?
                .addresses
                .to_vec(),
        };

        Ok(lookup_table)
    }

    /// Determines the token program ID for a given mint by checking the mint account's owner
    async fn get_token_program_for_mint(
        &self,
        rpc_client: &solana_client::nonblocking::rpc_client::RpcClient,
        mint: &Pubkey,
    ) -> Result<Pubkey> {
        let account = rpc_client.get_account(mint).await.map_err(|e| {
            SolverError::Component(format!("Failed to fetch mint account {}: {}", mint, e))
        })?;

        if account.owner == spl_token_2022::ID {
            Ok(spl_token_2022::ID)
        } else {
            Ok(spl_token::ID)
        }
    }

    async fn build_and_send_versioned_transaction(
        &self,
        rpc_client: &solana_client::nonblocking::rpc_client::RpcClient,
        instructions: Vec<Instruction>,
        chain_id: u32,
    ) -> Result<String> {
        let recent_blockhash = rpc_client.get_latest_blockhash().await.map_err(|e| {
            SolverError::Component(format!("Failed to get recent blockhash: {}", e))
        })?;

        let payer = self.keypair.pubkey();

        // Try to use LUT if available
        let address_lookup_tables = if let Some(lut_address) = self.get_lut_address(chain_id) {
            match self.fetch_lut_account(rpc_client, &lut_address).await {
                Ok(lut) => vec![lut],
                Err(e) => {
                    error!(
                        self.logger,
                        "Failed to fetch LUT, falling back to legacy transaction";
                        "error" => %e,
                    );
                    vec![]
                }
            }
        } else {
            vec![]
        };

        // Build versioned message (v0)
        let message = v0::Message::try_compile(
            &payer,
            &instructions,
            &address_lookup_tables,
            recent_blockhash,
        )
        .map_err(|e| SolverError::Component(format!("Failed to compile message: {}", e)))?;

        let versioned_message = VersionedMessage::V0(message);

        // Sign the versioned transaction
        let tx = VersionedTransaction::try_new(versioned_message, &[self.keypair.as_ref()])
            .map_err(|e| {
                SolverError::Component(format!("Failed to sign versioned transaction: {}", e))
            })?;

        let signature = rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| {
                let tx_base64 = bincode::serialize(&tx)
                    .map(|bytes| STANDARD.encode(&bytes))
                    .unwrap_or_else(|_| "failed to serialize".to_string());

                error!(
                    self.logger,
                    "Failed to send transaction";
                    "error" => %e,
                    "tx_base64" => %tx_base64,
                );
                SolverError::Component(format!("Failed to send transaction: {}", e))
            })?;

        Ok(signature.to_string())
    }

    async fn fill_native_order(
        &self,
        order_id: &str,
        order_data: &order_book::OrderData,
        amount_out_to_fill: u128,
        dest_chain_id: u32,
    ) -> Result<String> {
        let provider = self
            .provider_manager
            .get_svm_provider(dest_chain_id)
            .await?;
        let rpc_client = provider.client().await;

        let program_id = self.get_order_book_program_id(dest_chain_id)?;
        let solver_pubkey = self.keypair.pubkey();
        let order_id_bytes = decode_order_id(&order_id.to_string());

        // Derive accounts
        let global_account = find_pda(&[order_book::GLOBAL_SEED], &program_id);
        let event_authority = find_pda(&[b"__event_authority"], &program_id);
        let order_account = find_pda(
            &[order_book::ORDER_SEED_PREFIX, &order_id_bytes],
            &program_id,
        );

        let token_out_mint = Pubkey::new_from_array(order_data.token_out);
        let token_in_mint = Pubkey::new_from_array(order_data.token_in);
        let recipient = Pubkey::new_from_array(order_data.recipient);

        // Determine the correct token program for each mint
        let token_out_program = self
            .get_token_program_for_mint(&rpc_client, &token_out_mint)
            .await?;
        let token_in_program = self
            .get_token_program_for_mint(&rpc_client, &token_in_mint)
            .await?;

        let solver_token_out_account = get_associated_token_address_with_program_id(
            &solver_pubkey,
            &token_out_mint,
            &token_out_program,
        );
        let recipient_token_out_ata = get_associated_token_address_with_program_id(
            &recipient,
            &token_out_mint,
            &token_out_program,
        );
        let solver_token_in_account = get_associated_token_address_with_program_id(
            &solver_pubkey,
            &token_in_mint,
            &token_in_program,
        );
        let order_token_in_ata = get_associated_token_address_with_program_id(
            &order_account,
            &token_in_mint,
            &token_in_program,
        );

        let fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: amount_out_to_fill as u64,
            origin_recipient: solver_pubkey.to_bytes(),
        };

        // Check if solver's token_in ATA exists, create instruction if needed
        let solver_token_in_exists = rpc_client
            .get_account(&solver_token_in_account)
            .await
            .is_ok();

        // Build instruction data using Anchor format:
        let mut ix_data = vec![];
        ix_data.extend_from_slice(&anchor_discriminator("fill_native_order"));
        ix_data.extend_from_slice(&order_id_bytes);
        order_data.serialize(&mut ix_data).map_err(|e| {
            SolverError::Component(format!("Failed to serialize order data: {}", e))
        })?;
        fill_params.serialize(&mut ix_data).map_err(|e| {
            SolverError::Component(format!("Failed to serialize fill params: {}", e))
        })?;

        // Build instruction with account metas matching FillNativeOrder struct order
        let accounts = vec![
            AccountMeta::new(solver_pubkey, true),
            AccountMeta::new_readonly(global_account, false),
            AccountMeta::new_readonly(token_out_mint, false),
            AccountMeta::new(solver_token_out_account, false),
            AccountMeta::new_readonly(recipient, false),
            AccountMeta::new(recipient_token_out_ata, false),
            AccountMeta::new_readonly(token_out_program, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new(order_account, false),
            AccountMeta::new_readonly(token_in_mint, false),
            AccountMeta::new(solver_token_in_account, false),
            AccountMeta::new(order_token_in_ata, false),
            AccountMeta::new_readonly(token_in_program, false),
            AccountMeta::new_readonly(event_authority, false),
        ];

        let ix = Instruction {
            program_id,
            accounts,
            data: ix_data,
        };

        // Build instructions list
        let mut instructions = vec![];

        // Add create ATA instruction if solver's token_in account doesn't exist
        if !solver_token_in_exists {
            instructions.push(create_associated_token_account_idempotent(
                &solver_pubkey,
                &solver_pubkey,
                &token_in_mint,
                &token_in_program,
            ));
        }

        instructions.push(ix);

        // Build and send versioned transaction (with LUT if available)
        self.build_and_send_versioned_transaction(&rpc_client, instructions, dest_chain_id)
            .await
    }

    async fn fill_foreign_order(
        &self,
        order_id: &str,
        order_data: &order_book::OrderData,
        amount_out_to_fill: u128,
        dest_chain_id: u32,
    ) -> Result<String> {
        let provider = self
            .provider_manager
            .get_svm_provider(dest_chain_id)
            .await?;
        let rpc_client = provider.client().await;
        let is_devnet = self.network == config::Network::Devnet;

        let program_id = self.get_order_book_program_id(dest_chain_id)?;
        let solver_pubkey = self.keypair.pubkey();
        let order_id_bytes = decode_order_id(&order_id.to_string());

        // Derive OrderBook accounts
        let global_account = find_pda(&[order_book::GLOBAL_SEED], &program_id);
        let event_authority = find_pda(&[b"__event_authority"], &program_id);
        let order_account = find_pda(
            &[order_book::ORDER_SEED_PREFIX, &order_id_bytes],
            &program_id,
        );

        // Derive Portal accounts
        let portal_global = find_pda(&[b"global"], &PORTAL_PROGRAM_ID);
        let portal_authority = find_pda(&[b"authority"], &PORTAL_PROGRAM_ID);

        let token_out_mint = Pubkey::new_from_array(order_data.token_out);
        let recipient = Pubkey::new_from_array(order_data.recipient);

        // Determine the correct token program for the token_out mint
        let token_out_program = self
            .get_token_program_for_mint(&rpc_client, &token_out_mint)
            .await?;

        let solver_token_out_account = get_associated_token_address_with_program_id(
            &solver_pubkey,
            &token_out_mint,
            &token_out_program,
        );
        let recipient_token_out_ata = get_associated_token_address_with_program_id(
            &recipient,
            &token_out_mint,
            &token_out_program,
        );

        let fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: amount_out_to_fill as u64,
            origin_recipient: decode_evm_address(self.evm_solver),
        };

        // Build instruction data using Anchor format:
        let mut ix_data = vec![];
        ix_data.extend_from_slice(&anchor_discriminator("fill_foreign_order"));
        ix_data.extend_from_slice(&order_id_bytes);
        order_data.serialize(&mut ix_data).map_err(|e| {
            SolverError::Component(format!("Failed to serialize order data: {}", e))
        })?;
        fill_params.serialize(&mut ix_data).map_err(|e| {
            SolverError::Component(format!("Failed to serialize fill params: {}", e))
        })?;

        // Build instruction with account metas matching FillForeignOrder struct order
        let mut accounts = vec![
            AccountMeta::new(solver_pubkey, true),
            AccountMeta::new(global_account, false),
            AccountMeta::new_readonly(token_out_mint, false),
            AccountMeta::new(solver_token_out_account, false),
            AccountMeta::new_readonly(recipient, false),
            AccountMeta::new(recipient_token_out_ata, false),
            AccountMeta::new_readonly(token_out_program, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new(order_account, false),
            AccountMeta::new_readonly(PORTAL_PROGRAM_ID, false),
            AccountMeta::new(portal_global, false),
            AccountMeta::new_readonly(portal_authority, false),
            AccountMeta::new_readonly(WORMHOLE_ADAPTER, false),
            AccountMeta::new_readonly(event_authority, false),
            AccountMeta::new_readonly(program_id, false),
        ];

        accounts.extend(WormholeRemainingAccounts::account_metas(is_devnet));

        let fill_ix = Instruction {
            program_id,
            accounts,
            data: ix_data,
        };

        // Build instructions list - fill instruction is always included
        let mut instructions = vec![fill_ix];

        // If using Wormhole adapter, add request_for_execution instruction
        {
            let sequence = wormhole::get_current_sequence(rpc_client, is_devnet)
                .await
                .map_err(|_| {
                    SolverError::Component("Error getting wormhole sequence".to_string())
                })?;

            let relay_ix = build_relay_instruction(
                &solver_pubkey,
                get_wormhole_chain_id(order_data.origin_chain_id).unwrap(),
                sequence,
                &self.get_destination_wormhole_adapter(order_data.origin_chain_id)?,
                Some(500_000),
                Some(20_000_000),
            )
            .map_err(|_| SolverError::Component("Error building relay instruction".to_string()))?;

            instructions.push(relay_ix);
        }

        // Build and send versioned transaction (with LUT if available)
        self.build_and_send_versioned_transaction(&rpc_client, instructions, dest_chain_id)
            .await
    }
}

#[async_trait]
impl EventHandler for SvmWriter {
    fn name(&self) -> &'static str {
        "SvmWriter"
    }

    async fn initialize(&self) -> Result<()> {
        self.order_store.initialize().await?;
        Ok(())
    }

    async fn handle_event(&self, event: SolverEvent) -> Result<Vec<SolverEvent>> {
        let _ = self.order_store.handle_event(event.clone()).await;

        match event {
            SolverEvent::RequestFillOrder(e) => {
                let order = self.order_store.get_order(&e.order_id).await?;
                let dest_chain_id = order.data.dest_chain_id;

                // Only handle SVM destination chains
                if chain_runtime(dest_chain_id) != ChainRuntime::Svm {
                    return Ok(vec![]);
                }

                // Wait until order's created_at timestamp before filling
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    - 15; // Subtract 15 seconds to account for chain time drift
                if now < order.data.created_at as u64 {
                    let wait_secs = order.data.created_at as u64 - now;
                    info!(
                        self.logger,
                        "Waiting for order created_at timestamp";
                        "order_id" => %e.order_id,
                        "created_at" => order.data.created_at,
                        "wait_secs" => wait_secs,
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;
                }

                let order_id = &e.order_id;
                let amount = e.amount;

                // Determine if this is a native (same-chain) or foreign (cross-chain) fill
                let is_native = order.data.origin_chain_id == order.data.dest_chain_id;

                let result = if is_native {
                    info!(
                        self.logger,
                        "Processing native SVM fill order";
                        "order_id" => order_id,
                        "amount" => amount,
                    );
                    self.fill_native_order(order_id, &order.data, amount, dest_chain_id)
                        .await
                } else {
                    info!(
                        self.logger,
                        "Processing foreign SVM fill order";
                        "order_id" => order_id,
                        "amount" => amount,
                        "origin_chain_id" => order.data.origin_chain_id,
                    );
                    self.fill_foreign_order(order_id, &order.data, amount, dest_chain_id)
                        .await
                };

                match result {
                    Ok(signature) => {
                        info!(
                            self.logger,
                            "Fill order transaction confirmed";
                            "order_id" => order_id,
                            "signature" => &signature,
                        );
                        return Ok(vec![SolverEvent::FillOrderSuccessful(
                            FillOrderSuccessfulEvent::new(order_id.clone()),
                        )]);
                    }
                    Err(err) => {
                        error!(
                            self.logger,
                            "Failed to fill order";
                            "order_id" => order_id,
                            "error" => %err,
                        );
                    }
                }
            }
            _ => {}
        }

        Ok(vec![])
    }
}
