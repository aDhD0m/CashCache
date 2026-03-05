use std::collections::{HashMap, VecDeque};

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::ToPrimitive;

use talon_types::broker::QuoteEvent;
use talon_types::channel::{ApproachDir, ApproachingSetup};
use talon_types::module::{ModuleId, ModuleState};
use talon_types::order::{
    InstrumentType, LegOrder, OrderId, OrderIntent, OrderType, Side,
};
use talon_types::position::Symbol;

use talon_util::indicators;
use talon_types::strategy::{ScanResult, TradingModule};

// ---------------------------------------------------------------------------
// Taxi — Equity Swing Loader (S4.4)
//
// Scans for pullback-to-support patterns with declining volume.
// Config: hard_stop_risk_pct = 2%, equity_long, max_concurrent = 5
// ---------------------------------------------------------------------------

pub struct Taxi {
    state: ModuleState,
    /// Per-symbol rolling price history.
    price_history: HashMap<Symbol, VecDeque<Decimal>>,
    /// Per-symbol rolling volume history.
    volume_history: HashMap<Symbol, VecDeque<u64>>,
    /// Tracked swing low support levels per symbol.
    swing_lows: HashMap<Symbol, Decimal>,
    /// Previous distance-to-support per symbol (for approach direction).
    prev_support_dist: HashMap<Symbol, f64>,
    /// Hard stop risk percentage.
    stop_risk_pct: Decimal,
    /// Lookback period for support detection.
    lookback: usize,
    /// Total signals generated since startup.
    signal_count: u32,
}

impl Taxi {
    pub fn new() -> Self {
        Self {
            state: ModuleState::Idle,
            price_history: HashMap::new(),
            volume_history: HashMap::new(),
            swing_lows: HashMap::new(),
            prev_support_dist: HashMap::new(),
            stop_risk_pct: Decimal::new(2, 0), // 2%
            lookback: 20,
            signal_count: 0,
        }
    }
}

impl Default for Taxi {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the lowest value in a price window (swing low / support).
fn find_swing_low(prices: &VecDeque<Decimal>, lookback: usize) -> Option<Decimal> {
    if prices.len() < lookback {
        return None;
    }
    prices.iter().rev().take(lookback).min().copied()
}

#[async_trait]
impl TradingModule for Taxi {
    fn id(&self) -> ModuleId {
        ModuleId::Taxi
    }

    fn state(&self) -> ModuleState {
        self.state
    }

    async fn on_quote(&mut self, quote: &QuoteEvent) -> ScanResult {
        self.state = ModuleState::Scanning;
        let mut result = ScanResult::default();

        // Update rolling windows
        let prices = self
            .price_history
            .entry(quote.symbol.clone())
            .or_default();
        prices.push_back(quote.last);
        while prices.len() > self.lookback + 2 {
            prices.pop_front();
        }

        let volumes = self
            .volume_history
            .entry(quote.symbol.clone())
            .or_default();
        volumes.push_back(quote.volume);
        while volumes.len() > self.lookback + 2 {
            volumes.pop_front();
        }

        // Need enough data
        if prices.len() < self.lookback {
            self.state = ModuleState::Idle;
            return result;
        }

        // Find/update support level (swing low in lookback window)
        if let Some(swing_low) = find_swing_low(prices, self.lookback) {
            self.swing_lows.insert(quote.symbol.clone(), swing_low);
        }

        let support = match self.swing_lows.get(&quote.symbol) {
            Some(s) => *s,
            None => {
                self.state = ModuleState::Idle;
                return result;
            }
        };

        // Don't trigger if price IS the support (at the low)
        if support.is_zero() || quote.last <= support {
            self.state = ModuleState::Idle;
            return result;
        }

        // Distance from current price to support (as percentage)
        let dist_pct = ((quote.last - support) / support * Decimal::from(100))
            .to_f64()
            .unwrap_or(100.0);

        // Direction tracking
        let prev_dist = self.prev_support_dist.get(&quote.symbol).copied();
        let direction = match prev_dist {
            Some(p) if dist_pct < p => ApproachDir::Heating,
            _ => ApproachDir::Cooling,
        };
        self.prev_support_dist
            .insert(quote.symbol.clone(), dist_pct);

        // Volume declining on the pullback?
        let vol_declining = indicators::volume_declining(volumes, 5.min(volumes.len()));

        // Signal: price within 1% of support AND volume declining
        if dist_pct < 1.5 && vol_declining {
            self.signal_count += 1;
            self.state = ModuleState::SignalGenerated;

            // Stop loss below support by stop_risk_pct
            let stop = support * (Decimal::ONE - self.stop_risk_pct / Decimal::from(100));
            // Target: 3:1 risk/reward from support
            let risk = quote.last - stop;
            let target = quote.last + risk * Decimal::from(3);

            let intent = OrderIntent {
                id: OrderId::new(),
                module: ModuleId::Taxi,
                symbol: quote.symbol.clone(),
                side: Side::Long,
                order_type: OrderType::Single(LegOrder {
                    symbol: quote.symbol.clone(),
                    side: Side::Long,
                    qty: 100,
                    limit_price: Some(quote.last),
                    instrument: InstrumentType::Equity,
                }),
                quantity: 100,
                stop_loss: Some(stop),
                take_profit: Some(target),
                time_stop: None,
                confidence: 0.6,
                created_at: Utc::now(),
            };

            tracing::info!(
                symbol = %quote.symbol,
                price = %quote.last,
                support = %support,
                dist_pct = dist_pct,
                "Taxi: pullback-to-support swing entry"
            );

            result.intents.push(intent);
        }
        // Approaching: price within 5% of support on pullback
        else if dist_pct < 5.0 {
            result.approaching.push(ApproachingSetup {
                symbol: quote.symbol.clone(),
                module: ModuleId::Taxi,
                criteria: format!(
                    "${:.2} → support ${:.2} ({:.1}%)",
                    quote.last, support, dist_pct
                ),
                distance_pct: dist_pct,
                direction,
                updated_at: Utc::now(),
            });
        }

        self.state = ModuleState::Idle;
        result
    }

    async fn scan(&mut self) -> ScanResult {
        ScanResult::default()
    }

    fn go_idle(&mut self) {
        self.state = ModuleState::Idle;
    }

    fn go_scanning(&mut self) {
        self.state = ModuleState::Scanning;
    }

    fn pause(&mut self) {
        self.state = ModuleState::Paused;
    }

    fn signals_generated(&self) -> u32 {
        self.signal_count
    }
}
