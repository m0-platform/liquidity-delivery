use anchor_client::anchor_lang::AnchorSerialize;
use anchor_client::solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    system_program,
    transaction::Transaction,
};
use async_trait::async_trait;
use m0_liquidity_sdk::types::ChainRuntime;
use slog::{error, info, Logger};
use spl_associated_token_account::get_associated_token_address;
use std::str::FromStr;
use std::sync::Arc;

use crate::components::ComponentParams;
use crate::config::ChainConfig;
use crate::error::{Result, SolverError};
use crate::events::{EventHandler, EventProcessor, FillOrderSuccessfulEvent, SolverEvent};
use crate::providers::ProviderManager;
use crate::stores::OrderStore;
use crate::utils::{
    anchor_discriminator, chain_runtime, decode_order_id, derive_wormhole_accounts, find_pda,
    PORTAL_PROGRAM_ID, WORMHOLE_ADAPTER,
};

pub struct SvmWriter {
    order_store: Arc<OrderStore>,
    provider_manager: Arc<ProviderManager>,
    chains: Vec<ChainConfig>,
    keypair: Arc<Keypair>,
    logger: Logger,
}

impl SvmWriter {
    pub fn new(params: &ComponentParams) -> Self {
        Self {
            order_store: Arc::new(OrderStore::new()),
            provider_manager: params.provider_manager.clone(),
            chains: params.config.chains.clone(),
            keypair: params.config.signers.svm_keypair(),
            logger: params.logger.new(slog::o!("component" => "SvmWriter")),
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

    fn get_bridge_adapter(&self, chain_id: u32) -> Result<Pubkey> {
        let chain = self
            .chains
            .iter()
            .find(|c| c.chain_id == chain_id)
            .ok_or_else(|| SolverError::Component("Chain not found for bridge adapter".into()))?;

        match &chain.bridge_adapter {
            Some(adapter) => Pubkey::from_str(adapter)
                .map_err(|e| SolverError::Component(format!("Invalid adapter program ID: {e}"))),
            None => Ok(WORMHOLE_ADAPTER),
        }
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

        let solver_token_out_account =
            get_associated_token_address(&solver_pubkey, &token_out_mint);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);
        let solver_token_in_account = get_associated_token_address(&solver_pubkey, &token_in_mint);
        let order_token_in_ata = get_associated_token_address(&order_account, &token_in_mint);

        let fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: amount_out_to_fill as u64,
            origin_recipient: solver_pubkey.to_bytes(),
        };

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
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new(order_account, false),
            AccountMeta::new_readonly(token_in_mint, false),
            AccountMeta::new(solver_token_in_account, false),
            AccountMeta::new(order_token_in_ata, false),
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(event_authority, false),
        ];

        let ix = Instruction {
            program_id,
            accounts,
            data: ix_data,
        };

        // Build and send transaction
        let recent_blockhash = rpc_client.get_latest_blockhash().await.map_err(|e| {
            SolverError::Component(format!("Failed to get recent blockhash: {}", e))
        })?;

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&solver_pubkey),
            &[self.keypair.as_ref()],
            recent_blockhash,
        );

        let signature = rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| {
                SolverError::Component(format!(
                    "Failed to send fill_native_order transaction: {}",
                    e
                ))
            })?;

        Ok(signature.to_string())
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

        let program_id = self.get_order_book_program_id(dest_chain_id)?;
        let bridge_adapter = self.get_bridge_adapter(dest_chain_id)?;

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

        let solver_token_out_account =
            get_associated_token_address(&solver_pubkey, &token_out_mint);
        let recipient_token_out_ata = get_associated_token_address(&recipient, &token_out_mint);

        let fill_params = order_book::instructions::fill::FillParams {
            amount_out_to_fill: amount_out_to_fill as u64,
            origin_recipient: solver_pubkey.to_bytes(),
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
            AccountMeta::new_readonly(spl_token::ID, false),
            AccountMeta::new_readonly(spl_associated_token_account::ID, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new(order_account, false),
            AccountMeta::new_readonly(PORTAL_PROGRAM_ID, false),
            AccountMeta::new(portal_global, false),
            AccountMeta::new_readonly(portal_authority, false),
            AccountMeta::new_readonly(bridge_adapter, false),
            AccountMeta::new_readonly(event_authority, false),
        ];

        match bridge_adapter {
            adapter if adapter == WORMHOLE_ADAPTER => {
                accounts.extend(derive_wormhole_accounts(dest_chain_id))
            }
            _ => {
                error!(
                    self.logger,
                    "Unsupported bridge adapter for foreign SVM fill";
                    "adapter" => %bridge_adapter,
                );
            }
        }

        let ix = Instruction {
            program_id,
            accounts,
            data: ix_data,
        };

        // Build and send transaction
        let recent_blockhash = rpc_client.get_latest_blockhash().await.map_err(|e| {
            SolverError::Component(format!("Failed to get recent blockhash: {}", e))
        })?;

        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&solver_pubkey),
            &[self.keypair.as_ref()],
            recent_blockhash,
        );

        let signature = rpc_client
            .send_and_confirm_transaction(&tx)
            .await
            .map_err(|e| {
                SolverError::Component(format!(
                    "Failed to send fill_foreign_order transaction: {}",
                    e
                ))
            })?;

        Ok(signature.to_string())
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
