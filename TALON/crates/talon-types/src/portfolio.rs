use std::collections::HashMap;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::broker::FillEvent;
use crate::module::ModuleId;
use crate::order::{OrderId, Side};
use crate::position::{Position, Symbol};

// ---------------------------------------------------------------------------
// Portfolio — active account state (Bevy Resource at Payload tier)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct Portfolio {
    pub positions: HashMap<Symbol, Position>,
    pub cash: Decimal,
    pub buying_power: Decimal,
    pub net_liquidation: Decimal,
    pub daily_pnl: Decimal,
    pub margin_used: Decimal,
    pub realized_pnl_today: Decimal,
    pub fills: Vec<FillRecord>,
}

impl Portfolio {
    pub fn new(starting_capital: Decimal) -> Self {
        Self {
            cash: starting_capital,
            buying_power: starting_capital,
            net_liquidation: starting_capital,
            ..Default::default()
        }
    }

    /// Record a fill and update portfolio state.
    pub fn apply_fill(&mut self, fill: &FillEvent, module: ModuleId) {
        let record = FillRecord {
            order_id: fill.order_id,
            symbol: fill.symbol.clone(),
            side: if fill.qty > 0 { Side::Long } else { Side::Short },
            qty: fill.qty.unsigned_abs(),
            fill_price: fill.price,
            commission: fill.commission,
            module,
            timestamp: fill.timestamp,
            realized_pnl: None,
        };

        if let Some(pos) = self.positions.get_mut(&fill.symbol) {
            // Existing position — check if adding or closing
            let old_qty = pos.qty;
            pos.qty += fill.qty;

            if pos.qty == 0 {
                // Position fully closed — compute realized P&L
                let pnl = (fill.price - pos.avg_entry) * Decimal::from(fill.qty.unsigned_abs());
                let pnl = if old_qty < 0 { -pnl } else { pnl };
                self.realized_pnl_today += pnl - fill.commission;
                self.daily_pnl += pnl - fill.commission;

                let mut record = record;
                record.realized_pnl = Some(pnl - fill.commission);
                self.fills.push(record);
                self.positions.remove(&fill.symbol);
                return;
            } else if (old_qty > 0 && fill.qty < 0) || (old_qty < 0 && fill.qty > 0) {
                // Partial close
                let closed_qty = fill.qty.unsigned_abs().min(old_qty.unsigned_abs());
                let pnl = (fill.price - pos.avg_entry) * Decimal::from(closed_qty);
                let pnl = if old_qty < 0 { -pnl } else { pnl };
                self.realized_pnl_today += pnl - fill.commission;
                self.daily_pnl += pnl - fill.commission;

                let mut record = record;
                record.realized_pnl = Some(pnl - fill.commission);
                self.fills.push(record);
            } else {
                // Adding to position — update avg entry
                let total_cost = pos.avg_entry * Decimal::from(old_qty.unsigned_abs())
                    + fill.price * Decimal::from(fill.qty.unsigned_abs());
                let total_qty = pos.qty.unsigned_abs();
                if total_qty > 0 {
                    pos.avg_entry = total_cost / Decimal::from(total_qty);
                }
                self.fills.push(record);
            }
        } else {
            // New position
            let pos = Position {
                symbol: fill.symbol.clone(),
                module,
                broker_id: fill.broker_id,
                qty: fill.qty,
                avg_entry: fill.price,
                current_price: fill.price,
                stop_loss: None,
                take_profit: None,
                time_stop: None,
                opened_at: fill.timestamp,
                order_id: fill.order_id,
            };
            self.positions.insert(fill.symbol.clone(), pos);
            self.fills.push(record);
        }

        self.cash -= fill.price * Decimal::from(fill.qty) + fill.commission;
    }

    /// Update current prices from quote data.
    pub fn update_price(&mut self, symbol: &Symbol, price: Decimal) {
        if let Some(pos) = self.positions.get_mut(symbol) {
            pos.current_price = price;
        }
        self.recompute_nlv();
    }

    /// Recompute net liquidation from cash + position values.
    fn recompute_nlv(&mut self) {
        let position_value: Decimal = self
            .positions
            .values()
            .map(|p| p.current_price * Decimal::from(p.qty))
            .sum();
        self.net_liquidation = self.cash + position_value;
    }

    /// Positions as a sorted vec for display.
    pub fn positions_vec(&self) -> Vec<Position> {
        let mut v: Vec<Position> = self.positions.values().cloned().collect();
        v.sort_by(|a, b| a.symbol.0.cmp(&b.symbol.0));
        v
    }
}

// ---------------------------------------------------------------------------
// FillRecord — stored fill with P&L for display
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillRecord {
    pub order_id: OrderId,
    pub symbol: Symbol,
    pub side: Side,
    pub qty: u64,
    pub fill_price: Decimal,
    pub commission: Decimal,
    pub module: ModuleId,
    pub timestamp: DateTime<Utc>,
    pub realized_pnl: Option<Decimal>,
}

// ---------------------------------------------------------------------------
// PortfolioSnapshot — for persistence to talon-db
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSnapshot {
    pub net_liquidation: Decimal,
    pub cash: Decimal,
    pub buying_power: Decimal,
    pub daily_pnl: Decimal,
    pub margin_used: Decimal,
    pub realized_pnl_today: Decimal,
    pub position_count: u32,
    pub timestamp: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::BrokerId;

    fn mock_fill(symbol: &str, qty: i64, price: f64) -> FillEvent {
        FillEvent {
            order_id: OrderId::new(),
            symbol: Symbol::new(symbol),
            qty,
            price: Decimal::from_f64_retain(price).unwrap_or(Decimal::ZERO),
            commission: Decimal::new(1, 0), // $1 commission
            timestamp: Utc::now(),
            broker_id: BrokerId::Mock,
        }
    }

    #[test]
    fn new_portfolio_has_starting_capital() {
        let p = Portfolio::new(Decimal::from(10_000));
        assert_eq!(p.net_liquidation, Decimal::from(10_000));
        assert_eq!(p.cash, Decimal::from(10_000));
        assert!(p.positions.is_empty());
    }

    #[test]
    fn apply_fill_opens_position() {
        let mut p = Portfolio::new(Decimal::from(100_000));
        let fill = mock_fill("AAPL", 10, 150.0);
        p.apply_fill(&fill, ModuleId::Firebird);

        assert_eq!(p.positions.len(), 1);
        let pos = p.positions.get(&Symbol::new("AAPL")).unwrap();
        assert_eq!(pos.qty, 10);
        assert_eq!(pos.avg_entry, Decimal::from_f64_retain(150.0).unwrap());
    }

    #[test]
    fn apply_fill_closes_position_with_pnl() {
        let mut p = Portfolio::new(Decimal::from(100_000));

        // Buy 10 AAPL at $150
        let buy = mock_fill("AAPL", 10, 150.0);
        p.apply_fill(&buy, ModuleId::Firebird);
        assert_eq!(p.positions.len(), 1);

        // Sell 10 AAPL at $155 ($5 profit per share = $50 - $1 commission = $49)
        let sell = mock_fill("AAPL", -10, 155.0);
        p.apply_fill(&sell, ModuleId::Firebird);
        assert_eq!(p.positions.len(), 0);
        assert_eq!(p.realized_pnl_today, Decimal::from(49)); // $50 - $1 commission
    }

    #[test]
    fn fill_records_track_history() {
        let mut p = Portfolio::new(Decimal::from(100_000));
        let fill = mock_fill("SPY", 100, 450.0);
        p.apply_fill(&fill, ModuleId::Taxi);
        assert_eq!(p.fills.len(), 1);
        assert_eq!(p.fills[0].symbol, Symbol::new("SPY"));
    }

    #[test]
    fn snapshot_captures_state() {
        let mut p = Portfolio::new(Decimal::from(50_000));
        p.daily_pnl = Decimal::from(100);
        let snap = PortfolioSnapshot::from(&p);
        assert_eq!(snap.net_liquidation, Decimal::from(50_000));
        assert_eq!(snap.daily_pnl, Decimal::from(100));
    }
    use rust_decimal::prelude::FromPrimitive;
}

impl From<&Portfolio> for PortfolioSnapshot {
    fn from(p: &Portfolio) -> Self {
        Self {
            net_liquidation: p.net_liquidation,
            cash: p.cash,
            buying_power: p.buying_power,
            daily_pnl: p.daily_pnl,
            margin_used: p.margin_used,
            realized_pnl_today: p.realized_pnl_today,
            position_count: p.positions.len() as u32,
            timestamp: Utc::now(),
        }
    }
}
