use thiserror::Error;

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Event bus error: {0}")]
    EventBus(String),
    
    #[error("Store error: {0}")]
    Store(String),
    
    #[error("Component error: {0}")]
    Component(String),
    
    #[error("Order not found: {0}")]
    OrderNotFound(String),
    
    #[error("Invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },
}

pub type Result<T> = std::result::Result<T, SolverError>;
