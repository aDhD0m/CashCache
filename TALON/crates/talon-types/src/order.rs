use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::module::ModuleId;
use crate::position::Symbol;

// ---------------------------------------------------------------------------
// OrderId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub Uuid);

impl OrderId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for OrderId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OrderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// OrderIntent — what modules emit (never a broker call)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OrderIntent {
    pub id: OrderId,
    pub module: ModuleId,
    pub symbol: Symbol,
    pub side: Side,
    pub order_type: OrderType,
    pub quantity: u64,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    pub time_stop: Option<DateTime<Utc>>,
    pub confidence: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    Long,
    Short,
}

impl std::fmt::Display for Side {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Long => write!(f, "BUY"),
            Self::Short => write!(f, "SELL"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum OrderType {
    Single(LegOrder),
    Spread {
        legs: Vec<LegOrder>,
        net_debit_limit: Option<Decimal>,
        net_credit_limit: Option<Decimal>,
    },
}

#[derive(Debug, Clone)]
pub struct LegOrder {
    pub symbol: Symbol,
    pub side: Side,
    pub qty: u64,
    pub limit_price: Option<Decimal>,
    pub instrument: InstrumentType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InstrumentType {
    Equity,
    Call {
        strike: Decimal,
        expiration: chrono::NaiveDate,
    },
    Put {
        strike: Decimal,
        expiration: chrono::NaiveDate,
    },
}

// ---------------------------------------------------------------------------
// OrderModify
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OrderModify {
    pub new_limit_price: Option<Decimal>,
    pub new_stop_price: Option<Decimal>,
    pub new_qty: Option<u64>,
}
