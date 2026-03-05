//! Flow tab types — L2 depth, time & sales, volume profile, delta.
//!
//! These types travel through the `watch::channel<AppState>` to the TUI,
//! so everything derives `Clone`.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::broker::Timeframe;
use crate::position::Symbol;

// ---------------------------------------------------------------------------
// Capacity constants
// ---------------------------------------------------------------------------

/// Maximum tape entries retained (ring buffer).
pub const TAPE_CAP: usize = 500;

/// Number of one-minute delta buckets in the sparkline.
pub const DELTA_BUCKETS: usize = 60;

/// DOM depth (price levels per side).
pub const DOM_DEPTH: usize = 20;

// ---------------------------------------------------------------------------
// Trade side classification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    /// Trade at or above the best ask — buyer aggressor.
    Buy,
    /// Trade at or below the best bid — seller aggressor.
    Sell,
    /// Between bid and ask, or DOM not yet available.
    Unknown,
}

// ---------------------------------------------------------------------------
// TapeEntry — one T&S print (owned by FlowSnapshot)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TapeEntry {
    pub time: DateTime<Utc>,
    pub price: Decimal,
    pub size: Decimal,
    pub side: TradeSide,
    pub exchange: String,
    pub conditions: String,
}

// ---------------------------------------------------------------------------
// DomLevel — one price level in the order book
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DomLevel {
    pub price: Decimal,
    pub size: Decimal,
    /// MPID or exchange name for L2 entries.
    pub market_maker: Option<String>,
}

// ---------------------------------------------------------------------------
// OrderBook — maintained from MarketDepth operations
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct OrderBook {
    /// Bids sorted descending by price (best bid first).
    pub bids: Vec<DomLevel>,
    /// Asks sorted ascending by price (best ask first).
    pub asks: Vec<DomLevel>,
}

impl OrderBook {
    pub fn best_bid(&self) -> Option<Decimal> {
        self.bids.first().map(|l| l.price)
    }

    pub fn best_ask(&self) -> Option<Decimal> {
        self.asks.first().map(|l| l.price)
    }

    pub fn spread(&self) -> Option<Decimal> {
        match (self.best_bid(), self.best_ask()) {
            (Some(b), Some(a)) if a > b => Some(a - b),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// VolumeBucket — one price level in the volume profile
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct VolumeBucket {
    pub price: Decimal,
    pub buy_volume: Decimal,
    pub sell_volume: Decimal,
}

impl VolumeBucket {
    pub fn total(&self) -> Decimal {
        self.buy_volume + self.sell_volume
    }

    pub fn delta(&self) -> Decimal {
        self.buy_volume - self.sell_volume
    }
}

// ---------------------------------------------------------------------------
// DeltaBucket — one time slice for the delta sparkline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DeltaBucket {
    pub time: DateTime<Utc>,
    /// buy_volume − sell_volume for this bucket window.
    pub delta: Decimal,
}

// ---------------------------------------------------------------------------
// FlowSnapshot — aggregate state rendered by the TUI
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct FlowSnapshot {
    /// Which symbol this snapshot covers.
    pub symbol: Option<Symbol>,
    /// Live order book.
    pub book: OrderBook,
    /// T&S ring buffer — newest first, capped at `TAPE_CAP`.
    pub tape: Vec<TapeEntry>,
    /// Volume profile buckets, sorted by price.
    pub volume_profile: Vec<VolumeBucket>,
    /// Rolling delta sparkline (last `DELTA_BUCKETS` one-minute buckets).
    pub delta_sparkline: Vec<DeltaBucket>,
    /// Cumulative session delta (buy vol − sell vol).
    pub cumulative_delta: Decimal,
    /// Whether live L2 data is actively streaming.
    pub is_live: bool,
    /// Error message if subscription failed.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// FlowCmd — TUI → FlowManager commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum FlowCmd {
    /// Subscribe to L2 data for this symbol.
    SelectSymbol(Symbol),
    /// Unsubscribe from current symbol.
    ClearSymbol,
    /// Clear accumulated tape, volume profile, and delta.
    ResetAccumulators,
}

// ---------------------------------------------------------------------------
// ChartCmd — TUI → ChartManager commands
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum ChartCmd {
    /// Fetch candles for this symbol at the given timeframe.
    FetchCandles { symbol: Symbol, timeframe: Timeframe },
}

// ---------------------------------------------------------------------------
// Broker-boundary types (f64 from ibapi, converted in FlowManager)
// ---------------------------------------------------------------------------

/// Raw T&S print from the broker before Decimal conversion.
#[derive(Debug, Clone)]
pub struct TapeRaw {
    pub time: DateTime<Utc>,
    pub price: f64,
    pub size: f64,
    pub exchange: String,
    pub conditions: String,
}

/// Raw depth operation from the broker.
#[derive(Debug, Clone)]
pub struct DepthRaw {
    pub position: usize,
    pub operation: DepthOp,
    pub side: DepthSide,
    pub price: f64,
    pub size: f64,
    pub market_maker: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthOp {
    Insert,
    Update,
    Delete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthSide {
    Bid,
    Ask,
}
