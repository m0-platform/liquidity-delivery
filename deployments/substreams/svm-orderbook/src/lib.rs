mod idl;
#[allow(unused)]
mod pb;

use anchor_lang::AnchorDeserialize;
use anchor_lang::Discriminator;
use base64::prelude::*;
use pb::substreams::v1::program::{
    CancelReported, Data, DestinationSupportUpdated, FillReported, OrderCancelled, OrderCompleted,
    OrderFilled, OrderOpened, RefundClaimed,
};
use sologger_log_context::programs_selector::ProgramsSelector;
use sologger_log_context::sologger_log_context::LogContext;
use substreams::Hex;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

use idl::idl::program::events as idl_events;

const PROGRAM_ID: &str = "MzLoYnJ6sF6eeejs4vV95TNmXqS3W4cAtLGKkjT4ZrK";
const DISCRIMINATOR_LEN: usize = 8;

#[substreams::handlers::map]
fn map_program_data(blk: Block) -> Data {
    let mut data = Data::default();
    let programs_selector = ProgramsSelector::new(&["*".to_string()]);

    for transaction in blk.transactions() {
        let Some(meta) = &transaction.meta else {
            continue;
        };

        let log_contexts = LogContext::parse_logs_basic(&meta.log_messages, &programs_selector);

        for context in log_contexts
            .iter()
            .filter(|ctx| ctx.program_id == PROGRAM_ID)
        {
            for log in &context.data_logs {
                let Ok(decoded) = BASE64_STANDARD.decode(log) else {
                    continue;
                };
                if decoded.len() < DISCRIMINATOR_LEN {
                    continue;
                }

                let discriminator = &decoded[..DISCRIMINATOR_LEN];
                let event_data = &mut &decoded[DISCRIMINATOR_LEN..];
                let transaction_hash = transaction.id();

                match discriminator {
                    idl_events::CancelReported::DISCRIMINATOR => {
                        if let Ok(e) = idl_events::CancelReported::deserialize(event_data) {
                            data.cancel_reported_event_list.push(CancelReported {
                                transaction_hash,
                                order_id: encode_hex(e.order_id),
                            });
                        }
                    }
                    idl_events::DestinationSupportUpdated::DISCRIMINATOR => {
                        if let Ok(e) =
                            idl_events::DestinationSupportUpdated::deserialize(event_data)
                        {
                            data.destination_support_updated_event_list.push(
                                DestinationSupportUpdated {
                                    transaction_hash,
                                    dest_chain_id: e.dest_chain_id,
                                    is_supported: e.is_supported,
                                },
                            );
                        }
                    }
                    idl_events::FillReported::DISCRIMINATOR => {
                        if let Ok(e) = idl_events::FillReported::deserialize(event_data) {
                            data.fill_reported_event_list.push(FillReported {
                                transaction_hash,
                                order_id: encode_hex(e.order_id),
                                amount_in_to_release: e.amount_in_to_release as u64,
                                amount_out_filled: e.amount_out_filled as u64,
                                origin_recipient: encode_hex(e.origin_recipient),
                            });
                        }
                    }
                    idl_events::OrderCancelled::DISCRIMINATOR => {
                        if let Ok(e) = idl_events::OrderCancelled::deserialize(event_data) {
                            data.order_cancelled_event_list.push(OrderCancelled {
                                transaction_hash,
                                order_id: encode_hex(e.order_id),
                            });
                        }
                    }
                    idl_events::OrderCompleted::DISCRIMINATOR => {
                        if let Ok(e) = idl_events::OrderCompleted::deserialize(event_data) {
                            data.order_completed_event_list.push(OrderCompleted {
                                transaction_hash,
                                order_id: encode_hex(e.order_id),
                            });
                        }
                    }
                    idl_events::OrderFilled::DISCRIMINATOR => {
                        if let Ok(e) = idl_events::OrderFilled::deserialize(event_data) {
                            data.order_filled_event_list.push(OrderFilled {
                                transaction_hash,
                                order_id: encode_hex(e.order_id),
                                solver: e.solver.to_string(),
                                amount_in_to_release: e.amount_in_to_release as u64,
                                amount_out_filled: e.amount_out_filled as u64,
                            });
                        }
                    }
                    idl_events::OrderOpened::DISCRIMINATOR => {
                        if let Ok(e) = idl_events::OrderOpened::deserialize(event_data) {
                            data.order_opened_event_list.push(OrderOpened {
                                transaction_hash,
                                order_id: encode_hex(e.order_id),
                                sender: e.sender.to_string(),
                                token_in: e.token_in.to_string(),
                                amount_in: e.amount_in,
                                dest_chain_id: e.dest_chain_id,
                                token_out: encode_hex(e.token_out),
                                amount_out: e.amount_out as u64,
                                solver: encode_hex(e.solver),
                            });
                        }
                    }
                    idl_events::RefundClaimed::DISCRIMINATOR => {
                        if let Ok(e) = idl_events::RefundClaimed::deserialize(event_data) {
                            data.refund_claimed_event_list.push(RefundClaimed {
                                transaction_hash,
                                order_id: encode_hex(e.order_id),
                                sender: e.sender.to_string(),
                                amount: e.amount,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    data
}

fn encode_hex(data: [u8; 32]) -> String {
    format!("0x{}", Hex::encode(data))
}
