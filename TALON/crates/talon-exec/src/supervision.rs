use talon_types::module::SupervisionModel;
use talon_types::order::OrderIntent;
use talon_types::trust::{ActionClass, TrustKey, TrustLedger};
use talon_types::risk::RegimeState;

// ---------------------------------------------------------------------------
// Supervision — determines if an intent needs approval (S7.1, S7.8, S8)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupervisionDecision {
    /// Auto-execute (SupervisedAutonomy, or auto-trust granted)
    AutoExecute,
    /// Requires operator approval
    RequiresApproval,
    /// Auto-execute on timeout (stops, forced cover, etc.)
    ExecuteOnTimeout,
}

pub fn evaluate_supervision(
    intent: &OrderIntent,
    regime: RegimeState,
    trust_ledger: &TrustLedger,
) -> SupervisionDecision {
    let model = intent.module.supervision_model();

    match model {
        SupervisionModel::SupervisedAutonomy => {
            // Hatch modules: auto-execute within boundaries
            SupervisionDecision::AutoExecute
        }
        SupervisionModel::DualControlStrict => {
            // 0DTE, shorts: always require approval (S8.6)
            SupervisionDecision::RequiresApproval
        }
        SupervisionModel::DualControl => {
            // Check trust calibration
            let trust_key = TrustKey {
                module: intent.module,
                regime,
                action_class: ActionClass::Entry,
            };
            if trust_ledger.has_auto_trust(&trust_key) {
                SupervisionDecision::AutoExecute
            } else {
                SupervisionDecision::RequiresApproval
            }
        }
    }
}

/// Timeout defaults (S7.8)
pub fn timeout_default(intent: &OrderIntent, is_stop_loss: bool) -> SupervisionDecision {
    if is_stop_loss {
        // Stop-loss: implicitly approved at entry time
        return SupervisionDecision::ExecuteOnTimeout;
    }

    let model = intent.module.supervision_model();
    match model {
        SupervisionModel::SupervisedAutonomy => SupervisionDecision::AutoExecute,
        _ => {
            // New position entry: reject on timeout
            SupervisionDecision::RequiresApproval
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use talon_types::module::ModuleId;
    use talon_types::order::*;
    use talon_types::position::Symbol;

    fn test_intent(module: ModuleId) -> OrderIntent {
        OrderIntent {
            id: OrderId::new(),
            module,
            symbol: Symbol::new("SPY"),
            side: Side::Long,
            order_type: OrderType::Single(LegOrder {
                symbol: Symbol::new("SPY"),
                side: Side::Long,
                qty: 1,
                limit_price: None,
                instrument: InstrumentType::Equity,
            }),
            quantity: 1,
            stop_loss: None,
            take_profit: None,
            time_stop: None,
            confidence: 0.8,
            created_at: Utc::now(),
        }
    }

    #[test]
    fn hatch_module_auto_executes() {
        let ledger = TrustLedger::default();
        let intent = test_intent(ModuleId::Firebird);
        let decision = evaluate_supervision(&intent, RegimeState::Standalone, &ledger);
        assert_eq!(decision, SupervisionDecision::AutoExecute);
    }

    #[test]
    fn strict_module_always_requires_approval() {
        let ledger = TrustLedger::default();
        let intent = test_intent(ModuleId::Snapback);
        let decision = evaluate_supervision(&intent, RegimeState::Standalone, &ledger);
        assert_eq!(decision, SupervisionDecision::RequiresApproval);
    }

    #[test]
    fn dual_control_requires_approval_without_trust() {
        let ledger = TrustLedger::default();
        let intent = test_intent(ModuleId::Climb);
        let decision = evaluate_supervision(&intent, RegimeState::Trending, &ledger);
        assert_eq!(decision, SupervisionDecision::RequiresApproval);
    }
}
