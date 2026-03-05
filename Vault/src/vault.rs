use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

use crate::harvest::{HarvestConfig, HarvestEngine};
use crate::wash::WashSaleTracker;

// ---------------------------------------------------------------------------
// CashCache Vault — event-sourced profit preservation (S4.5)
// ---------------------------------------------------------------------------

pub struct Vault {
    pub total: Decimal,
    pub harvest_engine: HarvestEngine,
    pub wash_tracker: WashSaleTracker,
    events: Vec<VaultEvent>,
}

#[derive(Debug, Clone)]
pub enum VaultEvent {
    HarvestReceived {
        amount: Decimal,
        timestamp: DateTime<Utc>,
    },
    WithdrawalRequested {
        amount: Decimal,
        timestamp: DateTime<Utc>,
    },
    WithdrawalCompleted {
        amount: Decimal,
        timestamp: DateTime<Utc>,
    },
}

impl Vault {
    pub fn new(harvest_config: HarvestConfig) -> Self {
        Self {
            total: Decimal::ZERO,
            harvest_engine: HarvestEngine::new(harvest_config),
            wash_tracker: WashSaleTracker::new(),
            events: Vec::new(),
        }
    }

    /// Receive a harvest transfer into the vault.
    pub fn receive_harvest(&mut self, amount: Decimal) {
        self.total += amount;
        self.events.push(VaultEvent::HarvestReceived {
            amount,
            timestamp: Utc::now(),
        });
        tracing::info!(vault_total = %self.total, amount = %amount, "vault harvest received");
    }

    /// Reconstruct vault state from events (event-sourcing fold).
    pub fn from_events(events: &[VaultEvent], harvest_config: HarvestConfig) -> Self {
        let mut vault = Self::new(harvest_config);
        for event in events {
            match event {
                VaultEvent::HarvestReceived { amount, .. } => {
                    vault.total += amount;
                }
                VaultEvent::WithdrawalCompleted { amount, .. } => {
                    vault.total -= amount;
                }
                VaultEvent::WithdrawalRequested { .. } => {}
            }
        }
        vault.events = events.to_vec();
        vault
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vault_accumulates_harvests() {
        let mut vault = Vault::new(HarvestConfig::default());
        vault.receive_harvest(Decimal::from(30));
        vault.receive_harvest(Decimal::from(45));
        assert_eq!(vault.total, Decimal::from(75));
        assert_eq!(vault.event_count(), 2);
    }

    #[test]
    fn vault_from_events() {
        let events = vec![
            VaultEvent::HarvestReceived {
                amount: Decimal::from(100),
                timestamp: Utc::now(),
            },
            VaultEvent::HarvestReceived {
                amount: Decimal::from(50),
                timestamp: Utc::now(),
            },
            VaultEvent::WithdrawalCompleted {
                amount: Decimal::from(25),
                timestamp: Utc::now(),
            },
        ];
        let vault = Vault::from_events(&events, HarvestConfig::default());
        assert_eq!(vault.total, Decimal::from(125)); // 100 + 50 - 25
    }
}
