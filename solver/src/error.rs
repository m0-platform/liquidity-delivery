use thiserror::Error;

#[derive(Error, Debug)]
pub enum SolverError {
    #[error("Store error: {0}")]
    Store(String),

    #[error("Component error: {0}")]
    Component(String),

    #[error("Order not found: {0}")]
    OrderNotFound(String),
}

pub type Result<T> = std::result::Result<T, SolverError>;
