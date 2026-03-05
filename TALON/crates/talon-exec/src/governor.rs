use rust_decimal::Decimal;
use tracing::{info, warn};

use talon_broker::session::BrokerSessionManager;
use talon_risk::mesh::RiskMesh;
use talon_risk::stress::StressEngine;
use talon_types::broker::BrokerId;
use talon_types::order::OrderIntent;
use talon_types::position::Position;
use talon_types::risk::{RegimeState, RiskDecision};
use talon_types::trust::TrustLedger;

use crate::supervision::{evaluate_supervision, SupervisionDecision};

// ---------------------------------------------------------------------------
// Governor — async actor orchestrating modules (S7, S8)
// ---------------------------------------------------------------------------

pub struct Governor {
    risk_mesh: RiskMesh,
    stress: StressEngine,
    trust_ledger: TrustLedger,
    broker: BrokerSessionManager,
    positions: Vec<Position>,
    regime: RegimeState,
    default_broker: BrokerId,
}

impl Governor {
    pub fn new(
        risk_mesh: RiskMesh,
        stress: StressEngine,
        broker: BrokerSessionManager,
        default_broker: BrokerId,
    ) -> Self {
        Self {
            risk_mesh,
            stress,
            trust_ledger: TrustLedger::default(),
            broker,
            positions: Vec::new(),
            regime: RegimeState::Standalone,
            default_broker,
        }
    }

    /// Process an incoming order intent through the full pipeline.
    pub async fn process_intent(&mut self, intent: OrderIntent) -> GovernorDecision {
        let net_liq = self
            .broker
            .account_snapshot(&self.default_broker)
            .await
            .map(|s| s.net_liquidation)
            .unwrap_or(Decimal::ZERO);

        // 1. Risk mesh evaluation
        let risk_decision =
            self.risk_mesh
                .evaluate(&intent, &self.positions, net_liq, &self.stress);

        match risk_decision {
            RiskDecision::Rejected { reason } => {
                warn!(
                    module = %intent.module,
                    symbol = %intent.symbol,
                    reason = %reason,
                    "intent rejected by risk mesh"
                );
                return GovernorDecision::Rejected { reason };
            }
            RiskDecision::ReducedSize {
                original,
                approved,
                reason,
            } => {
                info!(
                    module = %intent.module,
                    original = original,
                    approved = approved,
                    reason = %reason,
                    "intent size reduced by risk mesh"
                );
            }
            RiskDecision::Approved => {}
        }

        // 2. Supervision check
        let supervision = evaluate_supervision(&intent, self.regime, &self.trust_ledger);

        match supervision {
            SupervisionDecision::AutoExecute | SupervisionDecision::ExecuteOnTimeout => {
                // 3. Submit to broker
                match self.broker.submit(&self.default_broker, &intent).await {
                    Ok(ack) => {
                        info!(
                            module = %intent.module,
                            symbol = %intent.symbol,
                            broker_id = %ack.broker_order_id,
                            "order submitted"
                        );
                        GovernorDecision::Executed {
                            broker_order_id: ack.broker_order_id,
                        }
                    }
                    Err(e) => {
                        warn!(
                            module = %intent.module,
                            error = %e,
                            "broker submission failed"
                        );
                        GovernorDecision::Rejected {
                            reason: format!("broker error: {e}"),
                        }
                    }
                }
            }
            SupervisionDecision::RequiresApproval => {
                info!(
                    module = %intent.module,
                    symbol = %intent.symbol,
                    "intent queued for operator approval"
                );
                GovernorDecision::PendingApproval { intent }
            }
        }
    }

    /// Update stress engine with current equity.
    pub fn update_stress(&mut self, current_equity: Decimal) -> Option<talon_types::risk::StressTier> {
        self.stress.update(current_equity)
    }

    /// Check Cruising Altitude eligibility (S11.10).
    pub fn is_cruising_altitude(&self) -> bool {
        // All positions must be in eligible modules
        let all_eligible = self
            .positions
            .iter()
            .all(|p| p.module.is_cruising_altitude_eligible());

        // No intraday module scanning
        let no_intraday_scanning = true; // TODO: check module states

        all_eligible && no_intraday_scanning
    }

    pub fn positions(&self) -> &[Position] {
        &self.positions
    }

    pub fn stress_tier(&self) -> talon_types::risk::StressTier {
        self.stress.current_tier()
    }

    pub fn regime(&self) -> RegimeState {
        self.regime
    }
}

#[derive(Debug)]
pub enum GovernorDecision {
    Executed { broker_order_id: String },
    PendingApproval { intent: OrderIntent },
    Rejected { reason: String },
}
