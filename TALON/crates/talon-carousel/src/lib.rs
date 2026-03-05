//! talon-carousel — Profit harvest module: transfers realized wins from
//! TALON's active account to Vault (long-term hold portfolio).
//!
//! At Hatch tier, harvest_enabled=false (logging only, no actual transfers).
//! At Payload tier, inverse_account_size scaling drops rate to 10-12%.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use tracing::info;

use talon_types::module::ModuleId;
use talon_types::order::OrderId;
use talon_types::position::Symbol;

// ---------------------------------------------------------------------------
// Carousel config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CarouselConfig {
    /// Base harvest rate (e.g., 0.15 = 15% of realized P&L).
    pub harvest_rate: Decimal,
    /// Whether actual transfers are enabled (false at Hatch).
    pub harvest_enabled: bool,
    /// Minimum P&L to trigger harvest calculation.
    pub min_harvest_amount: Decimal,
    /// Inverse account size scaling: at larger account sizes, rate decreases.
    pub inverse_scale_base: Decimal,
}

impl Default for CarouselConfig {
    fn default() -> Self {
        Self {
            harvest_rate: Decimal::new(15, 2), // 0.15 = 15%
            harvest_enabled: false,
            min_harvest_amount: Decimal::new(100, 2), // $1.00 minimum
            inverse_scale_base: Decimal::from(25_000),
        }
    }
}

// ---------------------------------------------------------------------------
// HarvestEvent — logged per winning trade
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HarvestEvent {
    pub source_order_id: OrderId,
    pub symbol: Symbol,
    pub realized_pnl: Decimal,
    pub harvest_amount: Decimal,
    pub harvest_rate_used: Decimal,
    pub module: ModuleId,
    pub timestamp: DateTime<Utc>,
    pub transferred: bool,
}

// ---------------------------------------------------------------------------
// Carousel — profit harvest engine
// ---------------------------------------------------------------------------

pub struct Carousel {
    config: CarouselConfig,
    /// Accumulated harvests since startup.
    pub pending_harvest: Decimal,
    /// Total harvested all-time (loaded from DB on startup).
    pub total_harvested: Decimal,
    /// Recent harvest events for display.
    pub recent_events: Vec<HarvestEvent>,
}

impl Carousel {
    pub fn new(config: CarouselConfig) -> Self {
        Self {
            config,
            pending_harvest: Decimal::ZERO,
            total_harvested: Decimal::ZERO,
            recent_events: Vec::new(),
        }
    }

    /// Calculate harvest for a winning trade close.
    /// Returns `Some(HarvestEvent)` if trade is profitable, `None` otherwise.
    pub fn calculate_harvest(
        &mut self,
        order_id: OrderId,
        symbol: Symbol,
        realized_pnl: Decimal,
        module: ModuleId,
        account_nlv: Decimal,
    ) -> Option<HarvestEvent> {
        if realized_pnl <= Decimal::ZERO {
            return None;
        }

        let effective_rate = self.effective_rate(account_nlv);
        let harvest_amount = realized_pnl * effective_rate;

        if harvest_amount < self.config.min_harvest_amount {
            return None;
        }

        let event = HarvestEvent {
            source_order_id: order_id,
            symbol: symbol.clone(),
            realized_pnl,
            harvest_amount,
            harvest_rate_used: effective_rate,
            module,
            timestamp: Utc::now(),
            transferred: self.config.harvest_enabled,
        };

        info!(
            symbol = %symbol,
            pnl = %realized_pnl,
            harvest = %harvest_amount,
            rate = %effective_rate,
            enabled = self.config.harvest_enabled,
            "[Carousel] Harvest: ${:.2} from {} (P&L: ${:.2})",
            harvest_amount, symbol, realized_pnl
        );

        self.pending_harvest += harvest_amount;
        self.total_harvested += harvest_amount;
        self.recent_events.push(event.clone());

        // Cap recent events at 50
        if self.recent_events.len() > 50 {
            self.recent_events.remove(0);
        }

        Some(event)
    }

    /// Effective harvest rate, scaled by account size.
    /// At base ($25k), rate is the configured rate (15%).
    /// Larger accounts get a lower rate (inverse scaling).
    fn effective_rate(&self, account_nlv: Decimal) -> Decimal {
        if account_nlv <= self.config.inverse_scale_base {
            return self.config.harvest_rate;
        }
        // Scale down: rate × (base / nlv)
        let ratio = self.config.inverse_scale_base / account_nlv;
        let scaled = self.config.harvest_rate * ratio;
        // Floor at 5%
        scaled.max(Decimal::new(5, 2))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harvest_on_winning_trade() {
        let config = CarouselConfig::default();
        let mut carousel = Carousel::new(config);

        let event = carousel.calculate_harvest(
            OrderId::new(),
            Symbol::new("SPY"),
            Decimal::from(100), // $100 P&L
            ModuleId::Firebird,
            Decimal::from(10_000),
        );

        assert!(event.is_some());
        let ev = event.unwrap();
        assert_eq!(ev.harvest_amount, Decimal::from(15)); // 15% of $100
        assert!(!ev.transferred); // harvest_enabled=false at Hatch
    }

    #[test]
    fn no_harvest_on_losing_trade() {
        let config = CarouselConfig::default();
        let mut carousel = Carousel::new(config);

        let event = carousel.calculate_harvest(
            OrderId::new(),
            Symbol::new("SPY"),
            Decimal::from(-50), // -$50 P&L
            ModuleId::Firebird,
            Decimal::from(10_000),
        );

        assert!(event.is_none());
    }

    #[test]
    fn inverse_scaling_reduces_rate() {
        let config = CarouselConfig::default();
        let mut carousel = Carousel::new(config);

        // At $100k NLV (4x base), rate should be ~3.75% but floored at 5%
        let event = carousel.calculate_harvest(
            OrderId::new(),
            Symbol::new("AAPL"),
            Decimal::from(1000),
            ModuleId::Taxi,
            Decimal::from(100_000),
        );

        assert!(event.is_some());
        let ev = event.unwrap();
        // 5% floor × $1000 = $50
        assert_eq!(ev.harvest_amount, Decimal::from(50));
    }

    #[test]
    fn below_minimum_skipped() {
        let config = CarouselConfig {
            min_harvest_amount: Decimal::from(10), // $10 minimum
            ..CarouselConfig::default()
        };
        let mut carousel = Carousel::new(config);

        // $5 P&L × 15% = $0.75 — below $10 minimum
        let event = carousel.calculate_harvest(
            OrderId::new(),
            Symbol::new("SPY"),
            Decimal::from(5),
            ModuleId::Firebird,
            Decimal::from(10_000),
        );

        assert!(event.is_none());
    }

    #[test]
    fn pending_accumulates() {
        let config = CarouselConfig::default();
        let mut carousel = Carousel::new(config);

        carousel.calculate_harvest(
            OrderId::new(),
            Symbol::new("SPY"),
            Decimal::from(100),
            ModuleId::Firebird,
            Decimal::from(10_000),
        );
        carousel.calculate_harvest(
            OrderId::new(),
            Symbol::new("AAPL"),
            Decimal::from(200),
            ModuleId::Taxi,
            Decimal::from(10_000),
        );

        assert_eq!(carousel.pending_harvest, Decimal::from(45)); // 15 + 30
        assert_eq!(carousel.recent_events.len(), 2);
    }
}
