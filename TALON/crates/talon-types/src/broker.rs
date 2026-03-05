use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::order::OrderId;
use crate::position::Symbol;

// ---------------------------------------------------------------------------
// BrokerId
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BrokerId {
    Ibkr,
    Alpaca,
    Webull,
    Cobra,
    CenterPoint,
    Mock,
}

impl std::fmt::Display for BrokerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ibkr => write!(f, "IBKR"),
            Self::Alpaca => write!(f, "Alpaca"),
            Self::Webull => write!(f, "Webull"),
            Self::Cobra => write!(f, "Cobra"),
            Self::CenterPoint => write!(f, "CenterPoint"),
            Self::Mock => write!(f, "Mock"),
        }
    }
}

// ---------------------------------------------------------------------------
// Acks
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct OrderAck {
    pub broker_order_id: String,
    pub order_id: OrderId,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CancelAck {
    pub order_id: OrderId,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ModifyAck {
    pub order_id: OrderId,
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Stream events
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuoteEvent {
    pub symbol: Symbol,
    pub bid: Decimal,
    pub ask: Decimal,
    pub last: Decimal,
    pub volume: u64,
    pub timestamp: DateTime<Utc>,
    /// Yesterday's closing price (for % change calculation).
    pub prev_close: Option<Decimal>,
    /// Today's opening price.
    pub day_open: Option<Decimal>,
    /// Today's high.
    pub day_high: Option<Decimal>,
    /// Today's low.
    pub day_low: Option<Decimal>,
    /// 20-day average volume (for RVOL calculation).
    pub avg_volume: Option<u64>,
}

impl QuoteEvent {
    /// Daily change percentage vs previous close.
    pub fn change_pct(&self) -> Option<Decimal> {
        self.prev_close
            .filter(|p| !p.is_zero())
            .map(|p| ((self.last - p) / p) * Decimal::from(100))
    }

    /// Relative volume: current volume / average volume.
    pub fn rvol(&self) -> Option<Decimal> {
        self.avg_volume
            .filter(|&a| a > 0)
            .map(|a| Decimal::from(self.volume) / Decimal::from(a))
    }
}

#[derive(Debug, Clone)]
pub struct FillEvent {
    pub order_id: OrderId,
    pub symbol: Symbol,
    pub qty: i64,
    pub price: Decimal,
    pub commission: Decimal,
    pub timestamp: DateTime<Utc>,
    pub broker_id: BrokerId,
}

#[derive(Debug, Clone)]
pub struct MarginEvent {
    pub maintenance_margin: Decimal,
    pub excess_liquidity: Decimal,
    pub timestamp: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Short availability
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ShortAvailability {
    Available { shares: u64, fee_rate: Decimal },
    HardToBorrow { shares: u64, fee_rate: Decimal },
    Unavailable,
}

#[derive(Debug, Clone)]
pub struct LocateResult {
    pub symbol: Symbol,
    pub shares_located: u64,
    pub fee_rate: Decimal,
    pub valid_until: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Account snapshot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AccountSnapshot {
    pub broker_id: BrokerId,
    pub net_liquidation: Decimal,
    pub buying_power: Decimal,
    pub cash: CashBalance,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct CashBalance {
    pub settled: Decimal,
    pub unsettled: Decimal,
    pub pending_settlement: Vec<SettlementEvent>,
}

#[derive(Debug, Clone)]
pub struct SettlementEvent {
    pub amount: Decimal,
    pub settles_on: NaiveDate,
    pub source_order_id: OrderId,
}

// ---------------------------------------------------------------------------
// Timeframe — candle aggregation period
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    Min1,
    Min5,
    Min15,
    Min30,
    Hour1,
    Day,
    Week,
    Month,
}

impl Timeframe {
    pub const ALL: [Timeframe; 8] = [
        Self::Min1, Self::Min5, Self::Min15, Self::Min30,
        Self::Hour1, Self::Day, Self::Week, Self::Month,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Min1 => "1m",
            Self::Min5 => "5m",
            Self::Min15 => "15m",
            Self::Min30 => "30m",
            Self::Hour1 => "1h",
            Self::Day => "Day",
            Self::Week => "Wk",
            Self::Month => "Mo",
        }
    }

    /// Polygon API multiplier and timespan.
    pub fn polygon_params(self) -> (u32, &'static str) {
        match self {
            Self::Min1 => (1, "minute"),
            Self::Min5 => (5, "minute"),
            Self::Min15 => (15, "minute"),
            Self::Min30 => (30, "minute"),
            Self::Hour1 => (1, "hour"),
            Self::Day => (1, "day"),
            Self::Week => (1, "week"),
            Self::Month => (1, "month"),
        }
    }

    /// How many bars to request for a reasonable chart view.
    pub fn default_bar_count(self) -> usize {
        match self {
            Self::Min1 => 390,    // full trading day
            Self::Min5 => 78,     // full trading day
            Self::Min15 => 52,    // 2 days
            Self::Min30 => 26,    // 2 days
            Self::Hour1 => 42,    // ~6 days
            Self::Day => 120,     // ~6 months
            Self::Week => 104,    // 2 years
            Self::Month => 60,    // 5 years
        }
    }
}

impl std::fmt::Display for Timeframe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ---------------------------------------------------------------------------
// CandleBar — one OHLCV bar
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleBar {
    pub time: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: u64,
    pub vwap: Option<Decimal>,
    pub trade_count: Option<u64>,
}

// ---------------------------------------------------------------------------
// Stream handle
// ---------------------------------------------------------------------------

pub struct StreamHandle {
    pub _cancel: tokio::sync::oneshot::Sender<()>,
    pub join: tokio::task::JoinHandle<()>,
}
