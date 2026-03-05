use chrono::{NaiveDate, Utc};

use talon_types::position::Symbol;

// ---------------------------------------------------------------------------
// WashGroup — ETF family tracking (S4.5)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum WashGroup {
    Ticker(Symbol),
    EtfFamily(Vec<Symbol>),
}

// ---------------------------------------------------------------------------
// WashSaleTracker — 30-day lookback on losing positions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct WashSaleTracker {
    /// Losing positions closed in last 30 days: symbol -> close date
    losses: Vec<(Symbol, NaiveDate)>,
    /// Known ETF families
    families: Vec<Vec<Symbol>>,
}

impl WashSaleTracker {
    pub fn new() -> Self {
        Self {
            losses: Vec::new(),
            families: vec![
                // S&P 500 ETF family
                vec![
                    Symbol::new("SPY"),
                    Symbol::new("VOO"),
                    Symbol::new("IVV"),
                    Symbol::new("SPLG"),
                ],
                // Nasdaq 100
                vec![
                    Symbol::new("QQQ"),
                    Symbol::new("QQQM"),
                ],
                // Russell 2000
                vec![
                    Symbol::new("IWM"),
                    Symbol::new("VTWO"),
                ],
            ],
        }
    }

    pub fn record_loss(&mut self, symbol: Symbol, date: NaiveDate) {
        self.losses.push((symbol, date));
        self.prune();
    }

    /// Check if buying `symbol` would trigger a wash sale.
    pub fn is_blocked(&self, symbol: &Symbol) -> bool {
        let today = Utc::now().date_naive();
        let cutoff = today - chrono::Duration::days(30);

        // Direct ticker match
        if self.losses.iter().any(|(s, d)| s == symbol && *d >= cutoff) {
            return true;
        }

        // ETF family match
        if let Some(family) = self.find_family(symbol) {
            for member in family {
                if self
                    .losses
                    .iter()
                    .any(|(s, d)| s == member && *d >= cutoff)
                {
                    return true;
                }
            }
        }

        false
    }

    /// Get all blocked symbols with days remaining.
    pub fn blocked_symbols(&self) -> Vec<(Symbol, u32)> {
        let today = Utc::now().date_naive();
        let cutoff = today - chrono::Duration::days(30);

        self.losses
            .iter()
            .filter(|(_, d)| *d >= cutoff)
            .map(|(s, d)| {
                let days_remaining = 30 - (today - *d).num_days() as u32;
                (s.clone(), days_remaining)
            })
            .collect()
    }

    fn find_family(&self, symbol: &Symbol) -> Option<&[Symbol]> {
        self.families
            .iter()
            .find(|f| f.contains(symbol))
            .map(|f| f.as_slice())
    }

    fn prune(&mut self) {
        let cutoff = Utc::now().date_naive() - chrono::Duration::days(31);
        self.losses.retain(|(_, d)| *d >= cutoff);
    }
}

impl Default for WashSaleTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_ticker_blocked() {
        let mut tracker = WashSaleTracker::new();
        let today = Utc::now().date_naive();
        tracker.record_loss(Symbol::new("AAPL"), today - chrono::Duration::days(5));
        assert!(tracker.is_blocked(&Symbol::new("AAPL")));
    }

    #[test]
    fn etf_family_blocked() {
        let mut tracker = WashSaleTracker::new();
        let today = Utc::now().date_naive();
        tracker.record_loss(Symbol::new("SPY"), today - chrono::Duration::days(10));
        // VOO is in same family as SPY
        assert!(tracker.is_blocked(&Symbol::new("VOO")));
        assert!(tracker.is_blocked(&Symbol::new("IVV")));
        // Unrelated ticker not blocked
        assert!(!tracker.is_blocked(&Symbol::new("AAPL")));
    }

    #[test]
    fn old_loss_not_blocked() {
        let mut tracker = WashSaleTracker::new();
        let today = Utc::now().date_naive();
        tracker.record_loss(Symbol::new("MSFT"), today - chrono::Duration::days(35));
        assert!(!tracker.is_blocked(&Symbol::new("MSFT")));
    }
}
