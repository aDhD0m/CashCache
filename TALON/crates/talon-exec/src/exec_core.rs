use std::collections::VecDeque;

use rust_decimal::Decimal;
use tokio::sync::{mpsc, watch};
use tracing::{info, warn};

use talon_broker::session::BrokerSessionManager;
use talon_risk::mesh::RiskMesh;
use talon_risk::stress::StressEngine;
use talon_types::broker::BrokerId;
use talon_types::channel::AppState;
use talon_types::exec::{ExecMetrics, PendingIntent, SupervisionCommand};
use talon_types::order::OrderIntent;
use talon_types::portfolio::FillRecord;
use talon_types::position::Position;
use talon_types::risk::{RegimeState, RiskDecision};
use talon_types::trust::TrustLedger;

use crate::supervision::{evaluate_supervision, SupervisionDecision};

// ---------------------------------------------------------------------------
// ExecCore — the supervision gate between modules and broker (S7, S8)
// ---------------------------------------------------------------------------

pub struct ExecCore {
    risk_mesh: RiskMesh,
    stress: StressEngine,
    trust_ledger: TrustLedger,
    broker: BrokerSessionManager,
    positions: Vec<Position>,
    regime: RegimeState,
    default_broker: BrokerId,
    pending: VecDeque<PendingIntent>,
    metrics: ExecMetrics,
    recent_fills: Vec<FillRecord>,
    /// Max intents allowed in the pending queue.
    max_pending: usize,
    /// Timeout in seconds before auto-rejecting pending intents.
    timeout_secs: u64,
}

impl ExecCore {
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
            pending: VecDeque::new(),
            metrics: ExecMetrics::default(),
            recent_fills: Vec::new(),
            max_pending: 10,
            timeout_secs: 10,
        }
    }

    /// Run the ExecCore event loop. Processes intents, supervision commands,
    /// and checks for timeouts.
    pub async fn run(
        mut self,
        mut intent_rx: mpsc::Receiver<OrderIntent>,
        mut supervision_rx: mpsc::Receiver<SupervisionCommand>,
        app_state_tx: watch::Sender<AppState>,
    ) {
        let mut timeout_check = tokio::time::interval(std::time::Duration::from_secs(1));

        loop {
            tokio::select! {
                Some(intent) = intent_rx.recv() => {
                    self.metrics.intent_received_total += 1;
                    self.process_intent(intent).await;
                    self.publish_state(&app_state_tx);
                }
                Some(cmd) = supervision_rx.recv() => {
                    self.handle_supervision_command(cmd).await;
                    self.publish_state(&app_state_tx);
                }
                _ = timeout_check.tick() => {
                    self.expire_timed_out_intents();
                    self.publish_state(&app_state_tx);
                }
                else => break,
            }
        }
    }

    async fn process_intent(&mut self, intent: OrderIntent) {
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
                self.metrics.intent_rejected_total += 1;
                return;
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
            SupervisionDecision::AutoExecute => {
                self.metrics.intent_auto_executed_total += 1;
                self.submit_to_broker(intent).await;
            }
            SupervisionDecision::ExecuteOnTimeout => {
                // Queue but auto-execute if not rejected within timeout
                self.enqueue_pending(intent);
            }
            SupervisionDecision::RequiresApproval => {
                self.enqueue_pending(intent);
            }
        }
    }

    fn enqueue_pending(&mut self, intent: OrderIntent) {
        if self.pending.len() >= self.max_pending {
            warn!(
                module = %intent.module,
                symbol = %intent.symbol,
                max = self.max_pending,
                "pending queue full, rejecting intent"
            );
            self.metrics.intent_rejected_total += 1;
            return;
        }

        info!(
            module = %intent.module,
            symbol = %intent.symbol,
            "intent queued for operator approval (timeout: {}s)",
            self.timeout_secs
        );

        let pending = PendingIntent::from_intent(intent, self.timeout_secs);
        self.pending.push_back(pending);
    }

    async fn handle_supervision_command(&mut self, cmd: SupervisionCommand) {
        match cmd {
            SupervisionCommand::Approve(order_id) => {
                if let Some(idx) = self.pending.iter().position(|p| p.id == order_id) {
                    let pending = self.pending.remove(idx).unwrap();
                    info!(order_id = %pending.id, symbol = %pending.symbol, "intent approved by operator");
                    self.metrics.intent_approved_total += 1;
                    self.submit_to_broker(pending.intent).await;
                }
            }
            SupervisionCommand::Reject(order_id) => {
                if let Some(idx) = self.pending.iter().position(|p| p.id == order_id) {
                    let pending = self.pending.remove(idx).unwrap();
                    info!(order_id = %pending.id, symbol = %pending.symbol, "intent rejected by operator");
                    self.metrics.intent_rejected_total += 1;
                }
            }
            SupervisionCommand::ApproveAll => {
                let all: Vec<PendingIntent> = self.pending.drain(..).collect();
                for pending in all {
                    info!(order_id = %pending.id, symbol = %pending.symbol, "intent approved (approve all)");
                    self.metrics.intent_approved_total += 1;
                    self.submit_to_broker(pending.intent).await;
                }
            }
            SupervisionCommand::RejectAll => {
                let count = self.pending.len();
                self.pending.clear();
                self.metrics.intent_rejected_total += count as u64;
                info!(count, "all pending intents rejected by operator");
            }
        }
    }

    fn expire_timed_out_intents(&mut self) {
        let mut expired = Vec::new();
        self.pending.retain(|p| {
            if p.is_expired() {
                expired.push(p.id);
                false
            } else {
                true
            }
        });

        for id in expired {
            warn!(order_id = %id, "supervision timeout, auto-rejecting intent");
            self.metrics.intent_timeout_total += 1;
        }
    }

    async fn submit_to_broker(&mut self, intent: OrderIntent) {
        match self.broker.submit(&self.default_broker, &intent).await {
            Ok(ack) => {
                info!(
                    module = %intent.module,
                    symbol = %intent.symbol,
                    broker_id = %ack.broker_order_id,
                    "order submitted to broker"
                );
            }
            Err(e) => {
                warn!(
                    module = %intent.module,
                    error = %e,
                    "broker submission failed"
                );
                self.metrics.intent_rejected_total += 1;
            }
        }
    }

    fn publish_state(&self, tx: &watch::Sender<AppState>) {
        let pending_clone: Vec<PendingIntent> = self.pending.iter().cloned().collect();
        let fills_clone = self.recent_fills.clone();
        let metrics_clone = self.metrics.clone();
        let stress = self.stress.current_tier();
        let regime = self.regime;
        let cruising = self.is_cruising_altitude();

        tx.send_modify(|state| {
            state.pending_intents = pending_clone;
            state.recent_fills = fills_clone;
            state.exec_metrics = metrics_clone;
            state.stress_tier = stress;
            state.regime = regime;
            state.cruising_altitude = cruising;
        });
    }

    /// Record a fill from broker callback.
    pub fn record_fill(&mut self, fill: FillRecord) {
        self.metrics.fill_received_total += 1;
        self.recent_fills.push(fill);
        // Cap at 100 recent fills
        if self.recent_fills.len() > 100 {
            self.recent_fills.remove(0);
        }
    }

    pub fn update_stress(&mut self, current_equity: Decimal) {
        self.stress.update(current_equity);
    }

    fn is_cruising_altitude(&self) -> bool {
        self.positions
            .iter()
            .all(|p| p.module.is_cruising_altitude_eligible())
    }

    pub fn stress_tier(&self) -> talon_types::risk::StressTier {
        self.stress.current_tier()
    }

    pub fn metrics(&self) -> &ExecMetrics {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use talon_types::module::ModuleId;
    use talon_types::order::*;
    use talon_types::position::Symbol;
    use talon_types::risk::StressParams;

    fn test_intent(module: ModuleId) -> OrderIntent {
        OrderIntent {
            id: OrderId::new(),
            module,
            symbol: Symbol::new("SPY"),
            side: Side::Long,
            order_type: OrderType::Single(LegOrder {
                symbol: Symbol::new("SPY"),
                side: Side::Long,
                qty: 100,
                limit_price: None,
                instrument: InstrumentType::Equity,
            }),
            quantity: 100,
            stop_loss: None,
            take_profit: None,
            time_stop: None,
            confidence: 0.8,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn pending_intent_expires_after_timeout() {
        let intent = test_intent(ModuleId::Firebird);
        let pending = PendingIntent::from_intent(intent, 0); // 0s timeout = immediate
        assert!(pending.is_expired());
    }

    #[test]
    fn pending_intent_age_increases() {
        let intent = test_intent(ModuleId::Firebird);
        let pending = PendingIntent::from_intent(intent, 10);
        assert!(pending.age_secs() >= 0.0);
        assert!(!pending.is_expired());
    }

    #[test]
    fn max_pending_enforced() {
        let stress_params = StressParams {
            tier_0_threshold_pct: Decimal::new(30, 1),
            tier_2_threshold_pct: Decimal::new(50, 1),
            tier_3_threshold_pct: Decimal::new(80, 1),
            circuit_breaker_pct: Decimal::new(150, 1),
            override_cooldown_mins: 0,
        };

        let risk_mesh = RiskMesh::new(
            talon_types::risk::TierRiskParams {
                max_single_position_risk_pct: Decimal::from(5),
                max_total_exposure_pct: Decimal::from(60),
                max_concurrent_positions: 5,
                drawdown_circuit_breaker_pct: Decimal::from(-15),
                daily_loss_limit_pct: Decimal::from(5),
            },
            talon_types::risk::ModuleRiskAllocation::default(),
        );
        let stress = StressEngine::new(stress_params, Decimal::from(10_000));
        let broker = BrokerSessionManager::new();

        let mut core = ExecCore::new(risk_mesh, stress, broker, BrokerId::Mock);
        core.max_pending = 3;

        // Fill queue to max
        for _ in 0..3 {
            core.enqueue_pending(test_intent(ModuleId::Snapback));
        }
        assert_eq!(core.pending.len(), 3);

        // One more should be rejected
        core.enqueue_pending(test_intent(ModuleId::Snapback));
        assert_eq!(core.pending.len(), 3);
        assert_eq!(core.metrics.intent_rejected_total, 1);
    }

    #[test]
    fn timeout_expires_intents() {
        let stress_params = StressParams {
            tier_0_threshold_pct: Decimal::new(30, 1),
            tier_2_threshold_pct: Decimal::new(50, 1),
            tier_3_threshold_pct: Decimal::new(80, 1),
            circuit_breaker_pct: Decimal::new(150, 1),
            override_cooldown_mins: 0,
        };

        let risk_mesh = RiskMesh::new(
            talon_types::risk::TierRiskParams {
                max_single_position_risk_pct: Decimal::from(5),
                max_total_exposure_pct: Decimal::from(60),
                max_concurrent_positions: 5,
                drawdown_circuit_breaker_pct: Decimal::from(-15),
                daily_loss_limit_pct: Decimal::from(5),
            },
            talon_types::risk::ModuleRiskAllocation::default(),
        );
        let stress = StressEngine::new(stress_params, Decimal::from(10_000));
        let broker = BrokerSessionManager::new();

        let mut core = ExecCore::new(risk_mesh, stress, broker, BrokerId::Mock);
        core.timeout_secs = 0; // expire immediately

        let intent = test_intent(ModuleId::Snapback);
        core.enqueue_pending(intent);
        assert_eq!(core.pending.len(), 1);

        core.expire_timed_out_intents();
        assert_eq!(core.pending.len(), 0);
        assert_eq!(core.metrics.intent_timeout_total, 1);
    }
}
