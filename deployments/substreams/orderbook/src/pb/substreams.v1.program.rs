// @generated
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct Data {
    #[prost(message, repeated, tag="1")]
    pub cancel_reported_event_list: ::prost::alloc::vec::Vec<CancelReported>,
    #[prost(message, repeated, tag="2")]
    pub fill_reported_event_list: ::prost::alloc::vec::Vec<FillReported>,
    #[prost(message, repeated, tag="3")]
    pub order_cancelled_event_list: ::prost::alloc::vec::Vec<OrderCancelled>,
    #[prost(message, repeated, tag="4")]
    pub order_completed_event_list: ::prost::alloc::vec::Vec<OrderCompleted>,
    #[prost(message, repeated, tag="5")]
    pub order_filled_event_list: ::prost::alloc::vec::Vec<OrderFilled>,
    #[prost(message, repeated, tag="6")]
    pub order_opened_event_list: ::prost::alloc::vec::Vec<OrderOpened>,
    #[prost(message, repeated, tag="7")]
    pub refund_claimed_event_list: ::prost::alloc::vec::Vec<RefundClaimed>,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct CancelReported {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub ts: u64,
    #[prost(uint32, tag="3")]
    pub chain_id: u32,
    #[prost(string, tag="4")]
    pub transaction_hash: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub order_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct FillReported {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub ts: u64,
    #[prost(uint32, tag="3")]
    pub chain_id: u32,
    #[prost(string, tag="4")]
    pub transaction_hash: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(uint64, tag="6")]
    pub amount_in_to_release: u64,
    #[prost(uint64, tag="7")]
    pub amount_out_filled: u64,
    #[prost(string, tag="8")]
    pub origin_recipient: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderCancelled {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub ts: u64,
    #[prost(uint32, tag="3")]
    pub chain_id: u32,
    #[prost(string, tag="4")]
    pub transaction_hash: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub order_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderCompleted {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub ts: u64,
    #[prost(uint32, tag="3")]
    pub chain_id: u32,
    #[prost(string, tag="4")]
    pub transaction_hash: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub order_id: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderFilled {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub ts: u64,
    #[prost(uint32, tag="3")]
    pub chain_id: u32,
    #[prost(string, tag="4")]
    pub transaction_hash: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub solver: ::prost::alloc::string::String,
    #[prost(uint64, tag="7")]
    pub amount_in_to_release: u64,
    #[prost(uint64, tag="8")]
    pub amount_out_filled: u64,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct OrderOpened {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub ts: u64,
    #[prost(uint32, tag="3")]
    pub chain_id: u32,
    #[prost(string, tag="4")]
    pub transaction_hash: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub sender: ::prost::alloc::string::String,
    #[prost(string, tag="7")]
    pub token_in: ::prost::alloc::string::String,
    #[prost(uint64, tag="8")]
    pub amount_in: u64,
    #[prost(uint32, tag="9")]
    pub dest_chain_id: u32,
    #[prost(string, tag="10")]
    pub token_out: ::prost::alloc::string::String,
    #[prost(uint64, tag="11")]
    pub amount_out: u64,
    #[prost(string, tag="12")]
    pub solver: ::prost::alloc::string::String,
}
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct RefundClaimed {
    #[prost(string, tag="1")]
    pub id: ::prost::alloc::string::String,
    #[prost(uint64, tag="2")]
    pub ts: u64,
    #[prost(uint32, tag="3")]
    pub chain_id: u32,
    #[prost(string, tag="4")]
    pub transaction_hash: ::prost::alloc::string::String,
    #[prost(string, tag="5")]
    pub order_id: ::prost::alloc::string::String,
    #[prost(string, tag="6")]
    pub sender: ::prost::alloc::string::String,
    #[prost(uint64, tag="7")]
    pub amount: u64,
}
// @@protoc_insertion_point(module)
