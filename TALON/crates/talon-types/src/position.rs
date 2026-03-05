use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::broker::BrokerId;
use crate::module::ModuleId;
use crate::order::OrderId;

// ---------------------------------------------------------------------------
// Symbol
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Symbol(pub String);

impl Symbol {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ---------------------------------------------------------------------------
// Position
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Position {
    pub symbol: Symbol,
    pub module: ModuleId,
    pub broker_id: BrokerId,
    pub qty: i64,
    pub avg_entry: Decimal,
    pub current_price: Decimal,
    pub stop_loss: Option<Decimal>,
    pub take_profit: Option<Decimal>,
    pub time_stop: Option<DateTime<Utc>>,
    pub opened_at: DateTime<Utc>,
    pub order_id: OrderId,
}

impl Position {
    pub fn unrealized_pnl(&self) -> Decimal {
        let diff = self.current_price - self.avg_entry;
        diff * Decimal::from(self.qty)
    }

    pub fn unrealized_pnl_pct(&self) -> Decimal {
        if self.avg_entry.is_zero() {
            return Decimal::ZERO;
        }
        ((self.current_price - self.avg_entry) / self.avg_entry) * Decimal::from(100)
    }

    pub fn is_profitable(&self) -> bool {
        self.unrealized_pnl() > Decimal::ZERO
    }
}

// ---------------------------------------------------------------------------
// Position snapshot (from broker, for reconciliation)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PositionSnapshot {
    pub positions: Vec<BrokerPosition>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct BrokerPosition {
    pub symbol: Symbol,
    pub qty: i64,
    pub avg_cost: Decimal,
    pub market_value: Decimal,
}

// ---------------------------------------------------------------------------
// PDT tracking (S4.7)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PdtTracker {
    pub day_trades: std::collections::VecDeque<NaiveDate>,
    pub account_equity: Decimal,
}

impl PdtTracker {
    pub fn new(equity: Decimal) -> Self {
        Self {
            day_trades: std::collections::VecDeque::new(),
            account_equity: equity,
        }
    }

    pub fn can_day_trade(&self) -> bool {
        if self.account_equity >= Decimal::from(25_000) {
            return true;
        }
        let five_days_ago = chrono::Utc::now().date_naive() - chrono::Duration::days(5);
        let count = self
            .day_trades
            .iter()
            .filter(|d| **d >= five_days_ago)
            .count();
        count < 3
    }

    pub fn record_day_trade(&mut self, date: NaiveDate) {
        self.day_trades.push_back(date);
        // Keep only last 10 business days
        while self.day_trades.len() > 20 {
            self.day_trades.pop_front();
        }
    }
}
