mod abi;
#[allow(unused)]
mod pb;
use hex_literal::hex;
use pb::contract::v1 as contract;
use substreams::Hex;
use substreams_ethereum::pb::eth::v2 as eth;
use substreams_ethereum::Event;

#[allow(unused_imports)] // Might not be needed depending on actual ABI, hence the allow
use {num_traits::cast::ToPrimitive, std::str::FromStr, substreams::scalar::BigDecimal};

substreams_ethereum::init!();

const ORDERBOOK_TRACKED_CONTRACT: [u8; 20] = hex!("e39b012ab3b20e94a9beea557eb0de4171d4d3e4");

fn map_orderbook_events(blk: &eth::Block, events: &mut contract::Events) {
    for rcpt in blk.receipts() {
        for log in rcpt.receipt.logs.iter().filter(|log| log.address == ORDERBOOK_TRACKED_CONTRACT) {
            if let Some(event) = abi::orderbook_contract::events::AdminChanged::match_and_decode(log) {
                events.orderbook_admin_changeds.push(contract::OrderbookAdminChanged {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    new_admin: event.new_admin,
                    previous_admin: event.previous_admin,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::CancelReported::match_and_decode(log) {
                events.orderbook_cancel_reporteds.push(contract::OrderbookCancelReported {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    order_id: Vec::from(event.order_id),
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::DestinationSupportUpdated::match_and_decode(log) {
                events.orderbook_destination_support_updateds.push(contract::OrderbookDestinationSupportUpdated {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    dest_chain_id: event.dest_chain_id.to_u64(),
                    is_supported: event.is_supported,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::Eip712DomainChanged::match_and_decode(log) {
                events.orderbook_eip712_domain_changeds.push(contract::OrderbookEip712DomainChanged {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::FillReported::match_and_decode(log) {
                events.orderbook_fill_reporteds.push(contract::OrderbookFillReported {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    amount_in_to_release: event.amount_in_to_release.to_string(),
                    amount_out_filled: event.amount_out_filled.to_string(),
                    order_id: Vec::from(event.order_id),
                    origin_recipient: event.origin_recipient,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::Initialized::match_and_decode(log) {
                events.orderbook_initializeds.push(contract::OrderbookInitialized {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    version: event.version.to_u64(),
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::OrderCancelled::match_and_decode(log) {
                events.orderbook_order_cancelleds.push(contract::OrderbookOrderCancelled {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    message_id: Vec::from(event.message_id),
                    order_id: Vec::from(event.order_id),
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::OrderCompleted::match_and_decode(log) {
                events.orderbook_order_completeds.push(contract::OrderbookOrderCompleted {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    order_id: Vec::from(event.order_id),
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::OrderFilled::match_and_decode(log) {
                events.orderbook_order_filleds.push(contract::OrderbookOrderFilled {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    amount_in_to_release: event.amount_in_to_release.to_string(),
                    amount_out_filled: event.amount_out_filled.to_string(),
                    message_id: Vec::from(event.message_id),
                    order_id: Vec::from(event.order_id),
                    solver: event.solver,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::OrderOpened::match_and_decode(log) {
                events.orderbook_order_openeds.push(contract::OrderbookOrderOpened {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    amount_in: event.amount_in.to_string(),
                    amount_out: event.amount_out.to_string(),
                    dest_chain_id: event.dest_chain_id.to_u64(),
                    order_id: Vec::from(event.order_id),
                    sender: event.sender,
                    solver: Vec::from(event.solver),
                    token_in: event.token_in,
                    token_out: Vec::from(event.token_out),
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::Paused::match_and_decode(log) {
                events.orderbook_pauseds.push(contract::OrderbookPaused {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    account: event.account,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::RefundClaimed::match_and_decode(log) {
                events.orderbook_refund_claimeds.push(contract::OrderbookRefundClaimed {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    amount_in_refunded: event.amount_in_refunded.to_string(),
                    order_id: Vec::from(event.order_id),
                    sender: event.sender,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::RoleAdminChanged::match_and_decode(log) {
                events.orderbook_role_admin_changeds.push(contract::OrderbookRoleAdminChanged {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    new_admin_role: Vec::from(event.new_admin_role),
                    previous_admin_role: Vec::from(event.previous_admin_role),
                    role: Vec::from(event.role),
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::RoleGranted::match_and_decode(log) {
                events.orderbook_role_granteds.push(contract::OrderbookRoleGranted {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    account: event.account,
                    role: Vec::from(event.role),
                    sender: event.sender,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::RoleRevoked::match_and_decode(log) {
                events.orderbook_role_revokeds.push(contract::OrderbookRoleRevoked {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    account: event.account,
                    role: Vec::from(event.role),
                    sender: event.sender,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::Unpaused::match_and_decode(log) {
                events.orderbook_unpauseds.push(contract::OrderbookUnpaused {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    account: event.account,
                });
                continue;
            }
            if let Some(event) = abi::orderbook_contract::events::Upgraded::match_and_decode(log) {
                events.orderbook_upgradeds.push(contract::OrderbookUpgraded {
                    evt_tx_hash: Hex(&rcpt.transaction.hash).to_string(),
                    evt_index: log.block_index,
                    evt_block_time: Some(blk.timestamp().to_owned()),
                    evt_block_number: blk.number,
                    implementation: event.implementation,
                });
                continue;
            }
        }
    }
}
#[substreams::handlers::map]
fn map_events(blk: eth::Block) -> Result<contract::Events, substreams::errors::Error> {
    let mut events = contract::Events::default();
    map_orderbook_events(&blk, &mut events);
    Ok(events)
}

