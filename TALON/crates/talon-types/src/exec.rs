use std::time::Instant;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::broker::BrokerId;
use crate::module::ModuleId;
use crate::order::{OrderId, OrderIntent, Side};
use crate::position::Symbol;

// ---------------------------------------------------------------------------
// PendingIntent — an intent awaiting operator approval
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PendingIntent {
    pub id: OrderId,
    pub intent: OrderIntent,
    pub module: ModuleId,
    pub symbol: Symbol,
    pub side: Side,
    pub quantity: u64,
    pub limit_price: Option<Decimal>,
    pub strategy_name: String,
    pub submitted_at: DateTime<Utc>,
    /// Monotonic clock for age calculation (not serialized).
    pub submitted_mono: Instant,
    pub timeout_secs: u64,
}

impl PendingIntent {
    pub fn from_intent(intent: OrderIntent, timeout_secs: u64) -> Self {
        let limit_price = match &intent.order_type {
            crate::order::OrderType::Single(leg) => leg.limit_price,
            crate::order::OrderType::Spread { .. } => None,
        };
        Self {
            id: intent.id,
            module: intent.module,
            symbol: intent.symbol.clone(),
            side: intent.side,
            quantity: intent.quantity,
            limit_price,
            strategy_name: intent.module.as_str().to_string(),
            submitted_at: intent.created_at,
            submitted_mono: Instant::now(),
            timeout_secs,
            intent,
        }
    }

    /// Age in seconds since submission (monotonic).
    pub fn age_secs(&self) -> f64 {
        self.submitted_mono.elapsed().as_secs_f64()
    }

    /// Whether this intent has timed out.
    pub fn is_expired(&self) -> bool {
        self.submitted_mono.elapsed().as_secs() >= self.timeout_secs
    }
}

// ---------------------------------------------------------------------------
// ExecutionReport — what comes back after broker processes an order
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExecutionReport {
    Filled {
        order_id: OrderId,
        symbol: Symbol,
        side: Side,
        qty: u64,
        fill_price: Decimal,
        commission: Decimal,
        broker_id: BrokerId,
        timestamp: DateTime<Utc>,
    },
    PartialFill {
        order_id: OrderId,
        symbol: Symbol,
        filled_qty: u64,
        remaining_qty: u64,
        fill_price: Decimal,
        timestamp: DateTime<Utc>,
    },
    Rejected {
        order_id: OrderId,
        reason: String,
        timestamp: DateTime<Utc>,
    },
    Cancelled {
        order_id: OrderId,
        timestamp: DateTime<Utc>,
    },
}

impl ExecutionReport {
    pub fn order_id(&self) -> &OrderId {
        match self {
            Self::Filled { order_id, .. }
            | Self::PartialFill { order_id, .. }
            | Self::Rejected { order_id, .. }
            | Self::Cancelled { order_id, .. } => order_id,
        }
    }
}

// ---------------------------------------------------------------------------
// SupervisionCommand — TUI → ExecCore approval/rejection channel
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum SupervisionCommand {
    Approve(OrderId),
    Reject(OrderId),
    ApproveAll,
    RejectAll,
}

// ---------------------------------------------------------------------------
// ExecMetrics — counters for the supervision gate
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecMetrics {
    pub intent_received_total: u64,
    pub intent_approved_total: u64,
    pub intent_rejected_total: u64,
    pub intent_timeout_total: u64,
    pub intent_auto_executed_total: u64,
    pub fill_received_total: u64,
}
