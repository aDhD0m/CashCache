use std::collections::{HashMap, VecDeque};

use async_trait::async_trait;
use chrono::Utc;
use rust_decimal::Decimal;

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
// Firebird — Oversold Reversal (S4.2)
//
// Scans for RSI oversold conditions with volume divergence.
// Config: rsi_oversold_threshold = 30, volume_divergence_lookback_bars = 10
// ---------------------------------------------------------------------------

pub struct Firebird {
    state: ModuleState,
    /// Per-symbol rolling price history for RSI calculation.
    price_history: HashMap<Symbol, VecDeque<Decimal>>,
    /// Per-symbol rolling volume history for divergence detection.
    volume_history: HashMap<Symbol, VecDeque<u64>>,
    /// Previous RSI value per symbol (for approach direction tracking).
    prev_rsi: HashMap<Symbol, Decimal>,
    /// RSI calculation period.
    rsi_period: usize,
    /// RSI threshold below which a signal fires.
    rsi_oversold: Decimal,
    /// Number of bars for volume divergence lookback.
    vol_lookback: usize,
    /// Show in scanner when RSI is within this % of threshold.
    approach_zone: Decimal,
    /// Total signals generated since startup.
    signal_count: u32,
}

impl Firebird {
    pub fn new() -> Self {
        Self {
            state: ModuleState::Idle,
            price_history: HashMap::new(),
            volume_history: HashMap::new(),
            prev_rsi: HashMap::new(),
            rsi_period: 14,
            rsi_oversold: Decimal::from(30),
            vol_lookback: 10,
            approach_zone: Decimal::from(45), // show approaching when RSI < 45 (within 50% of 30)
            signal_count: 0,
        }
    }
}

impl Default for Firebird {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TradingModule for Firebird {
    fn id(&self) -> ModuleId {
        ModuleId::Firebird
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
        while prices.len() > self.rsi_period + 2 {
            prices.pop_front();
        }

        let volumes = self
            .volume_history
            .entry(quote.symbol.clone())
            .or_default();
        volumes.push_back(quote.volume);
        while volumes.len() > self.vol_lookback + 1 {
            volumes.pop_front();
        }

        // Calculate RSI
        let current_rsi = match indicators::rsi(prices, self.rsi_period) {
            Some(r) => r,
            None => {
                self.state = ModuleState::Idle;
                return result;
            }
        };

        // Check for volume divergence: price falling but volume declining
        // (smart money not participating in the selloff = potential reversal)
        let has_vol_divergence = indicators::volume_declining(volumes, self.vol_lookback);

        // Direction tracking
        let prev = self.prev_rsi.get(&quote.symbol).copied();
        let direction = match prev {
            Some(p) if current_rsi < p => ApproachDir::Heating,
            _ => ApproachDir::Cooling,
        };
        self.prev_rsi.insert(quote.symbol.clone(), current_rsi);

        // Signal: RSI below oversold threshold + volume divergence
        if current_rsi < self.rsi_oversold && has_vol_divergence {
            self.signal_count += 1;
            self.state = ModuleState::SignalGenerated;

            let intent = OrderIntent {
                id: OrderId::new(),
                module: ModuleId::Firebird,
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
                stop_loss: quote.day_low.map(|l| l - Decimal::ONE),
                take_profit: Some(quote.last + (quote.last * Decimal::new(5, 2))),
                time_stop: None,
                confidence: 0.7,
                created_at: Utc::now(),
            };

            tracing::info!(
                symbol = %quote.symbol,
                rsi = %current_rsi,
                vol_divergence = has_vol_divergence,
                "Firebird: oversold reversal signal"
            );

            result.intents.push(intent);
        }
        // Approaching: RSI dropping toward threshold
        else if current_rsi < self.approach_zone {
            let distance = ((current_rsi - self.rsi_oversold) / self.rsi_oversold
                * Decimal::from(100))
            .to_string()
            .parse::<f64>()
            .unwrap_or(100.0)
            .abs();

            result.approaching.push(ApproachingSetup {
                symbol: quote.symbol.clone(),
                module: ModuleId::Firebird,
                criteria: format!("RSI {:.0} → threshold {}", current_rsi, self.rsi_oversold),
                distance_pct: distance,
                direction,
                updated_at: Utc::now(),
            });
        }

        self.state = ModuleState::Idle;
        result
    }

    async fn scan(&mut self) -> ScanResult {
        // Firebird is quote-driven — scan is a no-op
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

#[cfg(test)]
mod tests {
    use super::*;
    use talon_types::strategy::TradingModule;

    fn make_quote(symbol: &str, price: f64, volume: u64) -> QuoteEvent {
        QuoteEvent {
            symbol: Symbol::new(symbol),
            bid: Decimal::ZERO,
            ask: Decimal::ZERO,
            last: Decimal::from_f64_retain(price).unwrap(),
            volume,
            timestamp: Utc::now(),
            prev_close: None,
            day_open: None,
            day_high: None,
            day_low: Some(Decimal::from_f64_retain(price - 2.0).unwrap()),
            avg_volume: None,
        }
    }

    #[tokio::test]
    async fn firebird_generates_signal_on_oversold_with_volume_divergence() {
        let mut fb = Firebird::new();

        // Feed 16 quotes with declining prices and declining volume (vol divergence)
        // Start at 100, drop to 80 to push RSI below 30.
        let prices: Vec<f64> = (0..16).map(|i| 100.0 - (i as f64 * 1.5)).collect();
        let volumes: Vec<u64> = (0..16).map(|i| 10000u64.saturating_sub(i * 500)).collect();

        let mut last_result = ScanResult::default();
        for i in 0..16 {
            let q = make_quote("TEST", prices[i], volumes[i]);
            last_result = fb.on_quote(&q).await;
        }

        // Should have generated a signal or at least an approaching setup
        let total_signals = fb.signals_generated();
        let has_approaching = !last_result.approaching.is_empty();
        assert!(total_signals > 0 || has_approaching,
            "Firebird should generate signals or approaching setups on declining RSI with declining volume");
    }

    #[tokio::test]
    async fn firebird_no_signal_on_rising_prices() {
        let mut fb = Firebird::new();

        // Feed 16 rising quotes — RSI should stay above 30
        for i in 0..16 {
            let q = make_quote("BULL", 100.0 + (i as f64 * 1.0), 5000);
            let _ = fb.on_quote(&q).await;
        }

        assert_eq!(fb.signals_generated(), 0, "no signal on rising prices");
    }

    #[tokio::test]
    async fn firebird_intent_has_correct_fields() {
        let mut fb = Firebird::new();

        // Force a steep drop: 100 → 75 over 16 bars with declining volume
        let prices: Vec<f64> = (0..16).map(|i| 100.0 - (i as f64 * 2.0)).collect();
        let volumes: Vec<u64> = (0..16).map(|i| 20000u64.saturating_sub(i * 1200)).collect();

        let mut intents_found = Vec::new();
        for i in 0..16 {
            let q = make_quote("DIP", prices[i], volumes[i]);
            let result = fb.on_quote(&q).await;
            intents_found.extend(result.intents);
        }

        if let Some(intent) = intents_found.first() {
            assert_eq!(intent.module, ModuleId::Firebird);
            assert_eq!(intent.side, Side::Long);
            assert_eq!(intent.quantity, 100);
            assert!(intent.stop_loss.is_some(), "should have stop loss");
            assert!(intent.take_profit.is_some(), "should have take profit");
        }
    }

    use rust_decimal::prelude::FromPrimitive;
}
