mod idl;
#[allow(unused)]
mod pb;

use anchor_lang::AnchorDeserialize;
use anchor_lang::Discriminator;
use idl::idl::program::events as idl_events;
use pb::substreams::v1::program::{
    CancelReported, Data, FillReported, OrderCancelled, OrderCompleted, OrderFilled, OrderOpened,
    RefundClaimed,
};
use substreams::Hex;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

const CPI_EVENT_DISCRIMINATOR: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];

#[substreams::handlers::map]
fn map_program_data(blk: Block) -> Data {
    let mut data = Data::default();

    for transaction in blk.transactions() {
        let Some(meta) = &transaction.meta else {
            continue;
        };

        let mut instruction_datas = vec![];
        for inner in meta.inner_instructions.iter() {
            for instruction in inner.instructions.iter() {
                if instruction.data.len() > 16 && instruction.data[0..8] == CPI_EVENT_DISCRIMINATOR
                {
                    instruction_datas.push(instruction.data[8..].to_vec());
                }
            }
        }

        for cpi_event in instruction_datas {
            substreams::log::info!(
                "Processing cpi event for tx {}: 0x{}",
                transaction.id(),
                Hex::encode(&cpi_event.clone())
            );

            let (discriminator, event_data) = cpi_event.split_at(8);
            let mut event_data = event_data;
            let transaction_hash = transaction.id();

            match discriminator {
                idl_events::CancelReported::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::CancelReported::deserialize(&mut event_data) {
                        data.cancel_reported_event_list.push(CancelReported {
                            transaction_hash,
                            order_id: encode_hex(e.order_id),
                        });
                    }
                }
                idl_events::FillReported::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::FillReported::deserialize(&mut event_data) {
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
                    if let Ok(e) = idl_events::OrderCancelled::deserialize(&mut event_data) {
                        data.order_cancelled_event_list.push(OrderCancelled {
                            transaction_hash,
                            order_id: encode_hex(e.order_id),
                        });
                    }
                }
                idl_events::OrderCompleted::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::OrderCompleted::deserialize(&mut event_data) {
                        data.order_completed_event_list.push(OrderCompleted {
                            transaction_hash,
                            order_id: encode_hex(e.order_id),
                        });
                    }
                }
                idl_events::OrderFilled::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::OrderFilled::deserialize(&mut event_data) {
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
                    if let Ok(e) = idl_events::OrderOpened::deserialize(&mut event_data) {
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
                    if let Ok(e) = idl_events::RefundClaimed::deserialize(&mut event_data) {
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

    data
}

fn encode_hex(data: [u8; 32]) -> String {
    format!("0x{}", Hex::encode(data))
}
