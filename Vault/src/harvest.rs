use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use talon_types::order::OrderId;
use talon_types::position::Symbol;

// ---------------------------------------------------------------------------
// Harvest — profit preservation (CashCache spec S3)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct HarvestConfig {
    /// Percentage of realized gain to harvest (default: 30%)
    pub harvest_pct: Decimal,
    /// Minimum harvest amount to avoid uneconomical micro-transfers
    pub gravity_well: Decimal,
}

impl Default for HarvestConfig {
    fn default() -> Self {
        Self {
            harvest_pct: Decimal::new(30, 0), // 30%
            gravity_well: Decimal::new(500, 2), // $5.00 minimum
        }
    }
}

#[derive(Debug, Clone)]
pub struct HarvestEntry {
    pub source_order_id: OrderId,
    pub symbol: Symbol,
    pub realized_pnl: Decimal,
    pub harvest_amount: Decimal,
    pub status: HarvestStatus,
    pub created_at: DateTime<Utc>,
    pub executed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HarvestStatus {
    /// Waiting for settlement
    PendingSettlement,
    /// Ready to transfer
    ReadyToTransfer,
    /// Transfer in progress
    Transferring,
    /// Successfully transferred to vault
    Completed,
    /// Cancelled (e.g., forced cover on source position)
    Cancelled,
}

// ---------------------------------------------------------------------------
// HarvestEngine — computes and queues harvests
// ---------------------------------------------------------------------------

pub struct HarvestEngine {
    config: HarvestConfig,
    queue: Vec<HarvestEntry>,
}

impl HarvestEngine {
    pub fn new(config: HarvestConfig) -> Self {
        Self {
            config,
            queue: Vec::new(),
        }
    }

    /// Calculate harvest from a profitable close. Returns None if below gravity well.
    pub fn calculate_harvest(
        &self,
        order_id: OrderId,
        symbol: Symbol,
        realized_pnl: Decimal,
    ) -> Option<HarvestEntry> {
        if realized_pnl <= Decimal::ZERO {
            return None;
        }

        let harvest_amount = realized_pnl * self.config.harvest_pct / Decimal::from(100);

        if harvest_amount < self.config.gravity_well {
            tracing::debug!(
                symbol = %symbol,
                pnl = %realized_pnl,
                harvest = %harvest_amount,
                gravity_well = %self.config.gravity_well,
                "harvest below gravity well, skipping"
            );
            return None;
        }

        Some(HarvestEntry {
            source_order_id: order_id,
            symbol,
            realized_pnl,
            harvest_amount,
            status: HarvestStatus::PendingSettlement,
            created_at: Utc::now(),
            executed_at: None,
        })
    }

    pub fn queue_harvest(&mut self, entry: HarvestEntry) {
        tracing::info!(
            symbol = %entry.symbol,
            amount = %entry.harvest_amount,
            "harvest queued"
        );
        self.queue.push(entry);
    }

    pub fn pending_total(&self) -> Decimal {
        self.queue
            .iter()
            .filter(|e| e.status != HarvestStatus::Completed && e.status != HarvestStatus::Cancelled)
            .map(|e| e.harvest_amount)
            .sum()
    }

    pub fn queue(&self) -> &[HarvestEntry] {
        &self.queue
    }

    pub fn cancel_for_order(&mut self, order_id: &OrderId) {
        for entry in &mut self.queue {
            if entry.source_order_id == *order_id
                && entry.status == HarvestStatus::PendingSettlement
            {
                entry.status = HarvestStatus::Cancelled;
                tracing::info!(
                    order_id = %order_id,
                    amount = %entry.harvest_amount,
                    "harvest cancelled"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harvest_profitable_trade() {
        let engine = HarvestEngine::new(HarvestConfig::default());
        let entry = engine
            .calculate_harvest(
                OrderId::new(),
                Symbol::new("AAPL"),
                Decimal::from(100), // $100 profit
            )
            .unwrap();
        assert_eq!(entry.harvest_amount, Decimal::from(30)); // 30% of $100
    }

    #[test]
    fn no_harvest_on_loss() {
        let engine = HarvestEngine::new(HarvestConfig::default());
        let entry = engine.calculate_harvest(
            OrderId::new(),
            Symbol::new("MSFT"),
            Decimal::from(-50),
        );
        assert!(entry.is_none());
    }

    #[test]
    fn gravity_well_filters_small_harvests() {
        let engine = HarvestEngine::new(HarvestConfig {
            harvest_pct: Decimal::from(30),
            gravity_well: Decimal::from(10),
        });
        // $20 profit -> $6 harvest -> below $10 gravity well
        let entry = engine.calculate_harvest(
            OrderId::new(),
            Symbol::new("GME"),
            Decimal::from(20),
        );
        assert!(entry.is_none());
    }
}
