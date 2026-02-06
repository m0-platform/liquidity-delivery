mod abi;
mod idl;
#[allow(unused)]
mod pb;

use abi::orderbook_contract::events as abi_events;
use anchor_lang::AnchorDeserialize;
use anchor_lang::Discriminator;
use hex_literal::hex;
use idl::idl::program::events as idl_events;
use pb::substreams::v1::program::{
    CancelReported, Data, FillReported, OrderCancelled, OrderCompleted, OrderFilled, OrderOpened,
    RefundClaimed,
};
use substreams::Hex;
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::Event;
use substreams_solana::pb::sf::solana::r#type::v1::Block;

#[allow(unused_imports)]
use {num_traits::cast::ToPrimitive, std::str::FromStr, substreams::scalar::BigDecimal};

substreams_ethereum::init!();

const CPI_EVENT_DISCRIMINATOR: [u8; 8] = [228, 69, 165, 46, 81, 203, 154, 29];
const EVM_ORDERBOOK_CONTRACT: [u8; 20] = hex!("e39b012ab3b20e94a9beea557eb0de4171d4d3e4");

#[substreams::handlers::map]
fn map_svm_events(chain_id_str: String, blk: Block) -> Data {
    let mut data = Data::default();
    let chain_id = chain_id_str.parse::<u32>().unwrap_or_default();

    for transaction in blk.transactions() {
        let Some(meta) = &transaction.meta else {
            continue;
        };

        // Filter out emit_cpi instruction data
        let instruction_datas: Vec<Vec<u8>> = meta
            .inner_instructions
            .iter()
            .flat_map(|inner| inner.instructions.iter())
            .filter_map(|instruction| {
                instruction
                    .data
                    .strip_prefix(&CPI_EVENT_DISCRIMINATOR)
                    .map(|rest| rest.to_vec())
            })
            .collect();

        for (i, cpi_event) in instruction_datas.iter().enumerate() {
            substreams::log::info!(
                "Processing cpi event for tx {}: 0x{}",
                transaction.id(),
                Hex::encode(&cpi_event.clone())
            );

            let (discriminator, event_data) = cpi_event.split_at(8);
            let mut event_data = event_data;
            let transaction_hash = transaction.id();
            let id = format!("{}:{}", transaction_hash, i + 1);
            let ts: u64 = blk
                .block_time
                .and_then(|t| Some(t.timestamp as u64))
                .unwrap_or(0);

            match discriminator {
                idl_events::CancelReported::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::CancelReported::deserialize(&mut event_data) {
                        data.cancel_reported_event_list.push(CancelReported {
                            id,
                            chain_id,
                            ts,
                            transaction_hash,
                            order_id: encode_hex(e.order_id),
                        });
                    }
                }
                idl_events::FillReported::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::FillReported::deserialize(&mut event_data) {
                        data.fill_reported_event_list.push(FillReported {
                            id,
                            chain_id,
                            ts,
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
                            id,
                            chain_id,
                            ts,
                            transaction_hash,
                            order_id: encode_hex(e.order_id),
                        });
                    }
                }
                idl_events::OrderCompleted::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::OrderCompleted::deserialize(&mut event_data) {
                        data.order_completed_event_list.push(OrderCompleted {
                            id,
                            chain_id,
                            ts,
                            transaction_hash,
                            order_id: encode_hex(e.order_id),
                        });
                    }
                }
                idl_events::OrderFilled::DISCRIMINATOR => {
                    if let Ok(e) = idl_events::OrderFilled::deserialize(&mut event_data) {
                        data.order_filled_event_list.push(OrderFilled {
                            id,
                            chain_id,
                            ts,
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
                            id,
                            chain_id,
                            ts,
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
                            id,
                            chain_id,
                            ts,
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

#[substreams::handlers::map]
fn map_evm_events(chain_id_str: String, blk: eth::Block) -> Data {
    let mut data = Data::default();
    let chain_id = chain_id_str.parse::<u32>().unwrap_or_default();

    for (i, rcpt) in blk.receipts().into_iter().enumerate() {
        for log in rcpt
            .receipt
            .logs
            .iter()
            .filter(|log| log.address == EVM_ORDERBOOK_CONTRACT)
        {
            let ts = blk.timestamp().seconds as u64;
            let transaction_hash = format!("0x{}", Hex::encode(rcpt.transaction.hash.clone()));
            let id = format!("{}:{}", transaction_hash, i + 1);

            if let Some(event) = abi_events::CancelReported::match_and_decode(log) {
                data.cancel_reported_event_list.push(CancelReported {
                    id,
                    chain_id,
                    transaction_hash,
                    order_id: encode_hex(event.order_id),
                    ts,
                });
                continue;
            }
            if let Some(event) = abi_events::FillReported::match_and_decode(log) {
                data.fill_reported_event_list.push(FillReported {
                    id,
                    chain_id,
                    transaction_hash,
                    ts,
                    amount_in_to_release: event.amount_in_to_release.to_u64(),
                    amount_out_filled: event.amount_out_filled.to_u64(),
                    order_id: encode_hex(event.order_id),
                    origin_recipient: format!("0x{}", Hex::encode(event.origin_recipient)),
                });
                continue;
            }
            if let Some(event) = abi_events::OrderCancelled::match_and_decode(log) {
                data.order_cancelled_event_list.push(OrderCancelled {
                    id,
                    chain_id,
                    transaction_hash,
                    ts,
                    order_id: encode_hex(event.order_id),
                });
                continue;
            }
            if let Some(event) = abi_events::OrderCompleted::match_and_decode(log) {
                data.order_completed_event_list.push(OrderCompleted {
                    id,
                    chain_id,
                    transaction_hash,
                    ts,
                    order_id: encode_hex(event.order_id),
                });
                continue;
            }
            if let Some(event) = abi_events::OrderFilled::match_and_decode(log) {
                data.order_filled_event_list.push(OrderFilled {
                    id,
                    chain_id,
                    transaction_hash,
                    ts,
                    amount_in_to_release: event.amount_in_to_release.to_u64(),
                    amount_out_filled: event.amount_out_filled.to_u64(),
                    order_id: encode_hex(event.order_id),
                    solver: format!("0x{}", Hex::encode(event.solver)),
                });
                continue;
            }
            if let Some(event) = abi_events::OrderOpened::match_and_decode(log) {
                data.order_opened_event_list.push(OrderOpened {
                    id,
                    chain_id,
                    transaction_hash,
                    ts,
                    amount_in: event.amount_in.to_u64(),
                    amount_out: event.amount_out.to_u64(),
                    dest_chain_id: event.dest_chain_id.to_i32() as u32,
                    order_id: encode_hex(event.order_id),
                    sender: format!("0x{}", Hex::encode(event.sender)),
                    solver: format!("0x{}", Hex::encode(event.solver)),
                    token_in: format!("0x{}", Hex::encode(event.token_in)),
                    token_out: format!("0x{}", Hex::encode(event.token_out)),
                });
                continue;
            }
            if let Some(event) = abi_events::RefundClaimed::match_and_decode(log) {
                data.refund_claimed_event_list.push(RefundClaimed {
                    id,
                    chain_id,
                    transaction_hash,
                    ts,
                    amount: event.amount_in_refunded.to_u64(),
                    order_id: encode_hex(event.order_id),
                    sender: format!("0x{}", Hex::encode(event.sender)),
                });
                continue;
            }
        }
    }

    data
}

fn encode_hex(data: [u8; 32]) -> String {
    format!("0x{}", Hex::encode(data))
}
