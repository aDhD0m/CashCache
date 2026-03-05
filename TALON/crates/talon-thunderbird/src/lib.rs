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
// Thunderbird — Overextension Fade (S4.3)
//
// Scans for price exceeding upper Bollinger Band with volume climax.
// Config: bollinger_deviation = 2.5, volume_climax_multiplier = 3.0
// ---------------------------------------------------------------------------

pub struct Thunderbird {
    state: ModuleState,
    /// Per-symbol rolling price history for Bollinger calculation.
    price_history: HashMap<Symbol, VecDeque<Decimal>>,
    /// Per-symbol rolling volume history.
    volume_history: HashMap<Symbol, VecDeque<u64>>,
    /// Previous distance-from-upper-band per symbol (for approach direction).
    prev_bb_dist: HashMap<Symbol, Decimal>,
    /// Bollinger calculation period.
    bb_period: usize,
    /// Bollinger standard deviation multiplier.
    bb_deviation: Decimal,
    /// Volume must exceed avg * this to qualify as climax.
    vol_climax_mult: Decimal,
    /// Total signals generated since startup.
    signal_count: u32,
}

impl Thunderbird {
    pub fn new() -> Self {
        Self {
            state: ModuleState::Idle,
            price_history: HashMap::new(),
            volume_history: HashMap::new(),
            prev_bb_dist: HashMap::new(),
            bb_period: 20,
            bb_deviation: Decimal::new(25, 1), // 2.5
            vol_climax_mult: Decimal::from(3),
            signal_count: 0,
        }
    }
}

impl Default for Thunderbird {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TradingModule for Thunderbird {
    fn id(&self) -> ModuleId {
        ModuleId::Thunderbird
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
        while prices.len() > self.bb_period + 2 {
            prices.pop_front();
        }

        let volumes = self
            .volume_history
            .entry(quote.symbol.clone())
            .or_default();
        volumes.push_back(quote.volume);
        while volumes.len() > self.bb_period + 2 {
            volumes.pop_front();
        }

        // Calculate Bollinger bands
        let (upper, middle, _lower) =
            match indicators::bollinger(prices, self.bb_period, self.bb_deviation) {
                Some(bands) => bands,
                None => {
                    self.state = ModuleState::Idle;
                    return result;
                }
            };

        // Distance from upper band: negative = above band, positive = below
        let band_width = upper - middle;
        if band_width.is_zero() {
            self.state = ModuleState::Idle;
            return result;
        }

        let dist_from_upper = upper - quote.last;
        let dist_pct = (dist_from_upper / band_width * Decimal::from(100))
            .to_f64()
            .unwrap_or(100.0);

        // RVOL check
        let current_rvol = quote
            .rvol()
            .or_else(|| {
                // Fallback: compute from our volume history
                let avg = indicators::sma(
                    &volumes.iter().map(|v| Decimal::from(*v)).collect(),
                    self.bb_period.min(volumes.len()),
                );
                avg.map(|a| {
                    if a.is_zero() {
                        Decimal::ZERO
                    } else {
                        Decimal::from(quote.volume) / a
                    }
                })
            })
            .unwrap_or(Decimal::ZERO);

        // Direction tracking
        let prev_dist = self.prev_bb_dist.get(&quote.symbol).copied();
        let direction = match prev_dist {
            Some(p) if dist_pct < p.to_f64().unwrap_or(100.0) => ApproachDir::Heating,
            _ => ApproachDir::Cooling,
        };
        self.prev_bb_dist
            .insert(quote.symbol.clone(), Decimal::from_f64_retain(dist_pct).unwrap_or(Decimal::ZERO));

        // Signal: price above upper BB + volume climax
        if quote.last > upper && current_rvol >= self.vol_climax_mult {
            self.signal_count += 1;
            self.state = ModuleState::SignalGenerated;

            // Fade = short or put. For Phase 0 equity-only, emit as a short signal.
            let intent = OrderIntent {
                id: OrderId::new(),
                module: ModuleId::Thunderbird,
                symbol: quote.symbol.clone(),
                side: Side::Short,
                order_type: OrderType::Single(LegOrder {
                    symbol: quote.symbol.clone(),
                    side: Side::Short,
                    qty: 100,
                    limit_price: Some(quote.last),
                    instrument: InstrumentType::Equity,
                }),
                quantity: 100,
                stop_loss: Some(quote.last + (quote.last * Decimal::new(3, 2))), // 3% stop
                take_profit: Some(middle), // target mean reversion to SMA
                time_stop: None,
                confidence: 0.65,
                created_at: Utc::now(),
            };

            tracing::info!(
                symbol = %quote.symbol,
                price = %quote.last,
                upper_bb = %upper,
                rvol = %current_rvol,
                "Thunderbird: overextension fade signal"
            );

            result.intents.push(intent);
        }
        // Approaching: price within 50% of upper band distance
        else if dist_pct < 50.0 && dist_pct > 0.0 {
            result.approaching.push(ApproachingSetup {
                symbol: quote.symbol.clone(),
                module: ModuleId::Thunderbird,
                criteria: format!(
                    "BB {:.2} → upper {:.2} (RVOL {:.1}x)",
                    quote.last, upper, current_rvol
                ),
                distance_pct: dist_pct.abs(),
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
