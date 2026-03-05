use rust_decimal::Decimal;

use talon_types::order::OrderIntent;
use talon_types::position::Position;
use talon_types::risk::{ModuleRiskAllocation, RiskDecision, TierRiskParams};

use crate::stress::StressEngine;

// ---------------------------------------------------------------------------
// RiskMesh — evaluates order intents against tier limits (S7.2, S7.6)
// ---------------------------------------------------------------------------

pub struct RiskMesh {
    tier_params: TierRiskParams,
    module_allocation: ModuleRiskAllocation,
}

impl RiskMesh {
    pub fn new(tier_params: TierRiskParams, module_allocation: ModuleRiskAllocation) -> Self {
        Self {
            tier_params,
            module_allocation,
        }
    }

    /// Evaluate an order intent against the risk mesh.
    pub fn evaluate(
        &self,
        intent: &OrderIntent,
        positions: &[Position],
        net_liquidation: Decimal,
        stress: &StressEngine,
    ) -> RiskDecision {
        // Nosedive = no new positions
        if stress.is_nosedive() {
            return RiskDecision::Rejected {
                reason: "NOSEDIVE: circuit breaker tripped, exit-only mode".into(),
            };
        }

        let multiplier = stress.multiplier();

        // Check concurrent positions limit (stress-adjusted)
        let effective_max_positions =
            Decimal::from(self.tier_params.max_concurrent_positions) * multiplier;
        let current_count = Decimal::from(positions.len() as u32);
        if current_count >= effective_max_positions {
            return RiskDecision::Rejected {
                reason: format!(
                    "max concurrent positions ({effective_max_positions}) reached (stress: {multiplier}x)"
                ),
            };
        }

        // Check module allocation
        let module_alloc_pct = self.module_allocation.get(&intent.module);
        let module_positions = positions
            .iter()
            .filter(|p| p.module == intent.module)
            .count() as u32;
        let module_max = (Decimal::from(self.tier_params.max_concurrent_positions)
            * module_alloc_pct
            / Decimal::from(100))
            * multiplier;
        if Decimal::from(module_positions) >= module_max && module_max > Decimal::ZERO {
            return RiskDecision::Rejected {
                reason: format!(
                    "module {} allocation exhausted ({module_positions}/{module_max})",
                    intent.module
                ),
            };
        }

        // Check single position risk (stress-adjusted)
        let effective_max_risk_pct = self.tier_params.max_single_position_risk_pct * multiplier;
        if let Some(stop) = &intent.stop_loss {
            let risk_per_share = (Decimal::new(10000, 2) - stop).abs(); // mock entry $100
            let total_risk = risk_per_share * Decimal::from(intent.quantity);
            let risk_pct = if net_liquidation.is_zero() {
                Decimal::from(100)
            } else {
                (total_risk / net_liquidation) * Decimal::from(100)
            };
            if risk_pct > effective_max_risk_pct {
                return RiskDecision::Rejected {
                    reason: format!(
                        "single position risk {risk_pct}% > max {effective_max_risk_pct}% (stress: {multiplier}x)"
                    ),
                };
            }
        }

        // Check total exposure (stress-adjusted)
        let effective_max_exposure_pct = self.tier_params.max_total_exposure_pct * multiplier;
        let current_exposure: Decimal = positions
            .iter()
            .map(|p| p.avg_entry * Decimal::from(p.qty.unsigned_abs()))
            .sum();
        let new_exposure = current_exposure + Decimal::new(10000, 2) * Decimal::from(intent.quantity);
        let exposure_pct = if net_liquidation.is_zero() {
            Decimal::from(100)
        } else {
            (new_exposure / net_liquidation) * Decimal::from(100)
        };
        if exposure_pct > effective_max_exposure_pct {
            return RiskDecision::Rejected {
                reason: format!(
                    "total exposure {exposure_pct}% > max {effective_max_exposure_pct}% (stress: {multiplier}x)"
                ),
            };
        }

        RiskDecision::Approved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use talon_types::broker::BrokerId;
    use talon_types::module::ModuleId;
    use talon_types::order::{InstrumentType, LegOrder, OrderId, OrderType, Side};
    use talon_types::position::Symbol;
    use talon_types::risk::StressParams;

    fn test_params() -> TierRiskParams {
        TierRiskParams {
            max_single_position_risk_pct: Decimal::from(5),
            max_total_exposure_pct: Decimal::from(60),
            max_concurrent_positions: 5,
            drawdown_circuit_breaker_pct: Decimal::from(-15),
            daily_loss_limit_pct: Decimal::from(5),
        }
    }

    fn test_stress() -> StressEngine {
        StressEngine::new(
            StressParams {
                tier_0_threshold_pct: Decimal::new(30, 1),
                tier_2_threshold_pct: Decimal::new(50, 1),
                tier_3_threshold_pct: Decimal::new(80, 1),
                circuit_breaker_pct: Decimal::new(150, 1),
                override_cooldown_mins: 0,
            },
            Decimal::from(10_000),
        )
    }

    fn test_intent() -> OrderIntent {
        OrderIntent {
            id: OrderId::new(),
            module: ModuleId::Firebird,
            symbol: Symbol::new("AAPL"),
            side: Side::Long,
            order_type: OrderType::Single(LegOrder {
                symbol: Symbol::new("AAPL"),
                side: Side::Long,
                qty: 10,
                limit_price: Some(Decimal::new(15000, 2)),
                instrument: InstrumentType::Equity,
            }),
            quantity: 10,
            stop_loss: Some(Decimal::new(9800, 2)),
            take_profit: Some(Decimal::new(11000, 2)),
            time_stop: None,
            confidence: 0.85,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn approve_within_limits() {
        let alloc = ModuleRiskAllocation::default();
        let mesh = RiskMesh::new(test_params(), alloc);
        let stress = test_stress();

        let decision = mesh.evaluate(&test_intent(), &[], Decimal::from(10_000), &stress);
        assert!(matches!(decision, RiskDecision::Approved));
    }

    #[test]
    fn reject_at_nosedive() {
        let alloc = ModuleRiskAllocation::default();
        let mesh = RiskMesh::new(test_params(), alloc);
        let mut stress = test_stress();
        stress.update(Decimal::from(8_000)); // Nosedive

        let decision = mesh.evaluate(&test_intent(), &[], Decimal::from(8_000), &stress);
        assert!(matches!(decision, RiskDecision::Rejected { .. }));
    }

    #[test]
    fn reject_max_positions() {
        let alloc = ModuleRiskAllocation::default();
        let mesh = RiskMesh::new(test_params(), alloc);
        let stress = test_stress();

        // Fill up 5 positions
        let positions: Vec<Position> = (0..5)
            .map(|i| Position {
                symbol: Symbol::new(format!("SYM{i}")),
                module: ModuleId::Taxi,
                broker_id: BrokerId::Mock,
                qty: 10,
                avg_entry: Decimal::from(100),
                current_price: Decimal::from(102),
                stop_loss: Some(Decimal::from(95)),
                take_profit: Some(Decimal::from(110)),
                time_stop: None,
                opened_at: Utc::now(),
                order_id: OrderId::new(),
            })
            .collect();

        let decision =
            mesh.evaluate(&test_intent(), &positions, Decimal::from(10_000), &stress);
        assert!(matches!(decision, RiskDecision::Rejected { .. }));
    }
}
