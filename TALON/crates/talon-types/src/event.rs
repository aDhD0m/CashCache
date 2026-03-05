use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::broker::BrokerId;
use crate::module::ModuleId;
use crate::order::OrderId;
use crate::position::Symbol;
use crate::risk::StressTier;

// ---------------------------------------------------------------------------
// Event — the core event-sourcing type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: u64,
    pub timestamp: DateTime<Utc>,
    pub kind: EventKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventKind {
    // --- Order lifecycle ---
    OrderIntentCreated {
        order_id: OrderId,
        module: ModuleId,
        symbol: Symbol,
        qty: u64,
        side: crate::order::Side,
        confidence: f64,
    },
    OrderApproved {
        order_id: OrderId,
    },
    OrderRejected {
        order_id: OrderId,
        reason: String,
    },
    OrderSubmitted {
        order_id: OrderId,
        broker_id: BrokerId,
        broker_order_id: String,
    },
    OrderFilled {
        order_id: OrderId,
        symbol: Symbol,
        qty: i64,
        price: Decimal,
        commission: Decimal,
    },
    OrderCancelled {
        order_id: OrderId,
    },

    // --- Position lifecycle ---
    PositionOpened {
        order_id: OrderId,
        module: ModuleId,
        symbol: Symbol,
        qty: i64,
        avg_entry: Decimal,
    },
    PositionClosed {
        order_id: OrderId,
        symbol: Symbol,
        realized_pnl: Decimal,
    },
    StopAdjusted {
        order_id: OrderId,
        old_stop: Decimal,
        new_stop: Decimal,
        reason: StopAdjustReason,
    },

    // --- Risk / Stress ---
    StressMultiplierChanged {
        old: StressTier,
        new: StressTier,
        drawdown_pct: Decimal,
    },
    StressOverride {
        from: StressTier,
        to: StressTier,
        justification: String,
    },
    FlameoutEngaged {
        positions_tightened: u32,
    },
    NosediveTriggered {
        drawdown_pct: Decimal,
    },

    // --- Forced cover (S7.7) ---
    ForcedCover {
        symbol: Symbol,
        qty: i64,
        price: Decimal,
        reason: String,
    },

    // --- CashCache Vault ---
    HarvestQueued {
        source_order_id: OrderId,
        amount: Decimal,
        symbol: Symbol,
    },
    HarvestExecuted {
        amount: Decimal,
        from_account: BrokerId,
    },
    HarvestCancelled {
        source_order_id: OrderId,
        reason: String,
    },
    VaultWithdrawalRequested {
        amount: Decimal,
    },
    VaultWithdrawalCompleted {
        amount: Decimal,
    },

    // --- Trust calibration ---
    TrustApproval {
        module: ModuleId,
        action_class: crate::trust::ActionClass,
    },
    TrustRejection {
        module: ModuleId,
        action_class: crate::trust::ActionClass,
    },
    TrustAutoGranted {
        module: ModuleId,
        action_class: crate::trust::ActionClass,
    },
    TrustRevoked {
        module: ModuleId,
        action_class: crate::trust::ActionClass,
    },

    // --- System ---
    SystemStartup {
        version: String,
    },
    SystemShutdown,
    ReconciliationStarted,
    ReconciliationCompleted {
        discrepancies: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StopAdjustReason {
    Manual,
    TrailingStop,
    Flameout,
    ForcedCover,
}
