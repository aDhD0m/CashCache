use thiserror::Error;

#[derive(Debug, Error)]
pub enum TalonError {
    #[error("config error: {0}")]
    Config(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("broker error: {0}")]
    Broker(#[from] BrokerError),
    #[error("risk rejection: {0}")]
    RiskRejection(String),
    #[error("module error: {0}")]
    Module(String),
    #[error("reconciliation required: {0}")]
    Reconciliation(String),
}

#[derive(Debug, Error, Clone)]
pub enum BrokerError {
    #[error("connection lost: {0}")]
    ConnectionLost(String),
    #[error("order rejected: {0}")]
    OrderRejected(String),
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("locate failed: {symbol}")]
    LocateFailed { symbol: String },
    #[error("runtime panic: {0}")]
    RuntimePanic(String),
    #[error("timeout")]
    Timeout,
    #[error("unsupported operation: {0}")]
    Unsupported(String),
}
