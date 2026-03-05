use chrono::{DateTime, Utc};
use tokio::sync::{broadcast, mpsc, watch};

use crate::broker::{CandleBar, FillEvent, QuoteEvent, Timeframe};
use crate::event::Event;
use crate::exec::{ExecMetrics, PendingIntent, SupervisionCommand};
use crate::flow::{ChartCmd, FlowCmd, FlowSnapshot};
use crate::module::ModuleId;
use crate::order::OrderIntent;
use crate::portfolio::FillRecord;
use crate::position::Symbol;

// ---------------------------------------------------------------------------
// Channel bus — central inter-crate communication
// ---------------------------------------------------------------------------

/// Channels used across the system. Created once, distributed by reference.
pub struct ChannelBus {
    /// Modules -> Governor: order intents for risk evaluation.
    pub intent_tx: mpsc::Sender<OrderIntent>,
    pub intent_rx: mpsc::Receiver<OrderIntent>,

    /// Broker -> System: fill events.
    pub fill_tx: broadcast::Sender<FillEvent>,

    /// Broker -> System: quote events (high-frequency).
    pub quote_tx: broadcast::Sender<QuoteEvent>,

    /// EventStore: events for persistence.
    pub event_tx: mpsc::Sender<Event>,
    pub event_rx: mpsc::Receiver<Event>,

    /// Governor -> TUI: application state snapshot.
    pub app_state_tx: watch::Sender<AppState>,
    pub app_state_rx: watch::Receiver<AppState>,

    /// TUI -> FlowManager: symbol selection and control commands.
    pub flow_cmd_tx: mpsc::Sender<FlowCmd>,
    pub flow_cmd_rx: mpsc::Receiver<FlowCmd>,

    /// TUI -> ExecCore: supervision commands (approve/reject intents).
    pub supervision_tx: mpsc::Sender<SupervisionCommand>,
    pub supervision_rx: mpsc::Receiver<SupervisionCommand>,

    /// TUI -> ChartManager: chart data requests.
    pub chart_cmd_tx: mpsc::Sender<ChartCmd>,
    pub chart_cmd_rx: mpsc::Receiver<ChartCmd>,
}

impl ChannelBus {
    pub fn new() -> Self {
        let (intent_tx, intent_rx) = mpsc::channel(256);
        let (fill_tx, _) = broadcast::channel(1024);
        let (quote_tx, _) = broadcast::channel(4096);
        let (event_tx, event_rx) = mpsc::channel(1024);
        let (app_state_tx, app_state_rx) = watch::channel(AppState::default());
        let (flow_cmd_tx, flow_cmd_rx) = mpsc::channel(32);
        let (supervision_tx, supervision_rx) = mpsc::channel(50);
        let (chart_cmd_tx, chart_cmd_rx) = mpsc::channel(16);

        Self {
            intent_tx,
            intent_rx,
            fill_tx,
            quote_tx,
            event_tx,
            event_rx,
            app_state_tx,
            app_state_rx,
            flow_cmd_tx,
            flow_cmd_rx,
            supervision_tx,
            supervision_rx,
            chart_cmd_tx,
            chart_cmd_rx,
        }
    }
}

impl Default for ChannelBus {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// AppState — read by TUI via watch channel (one-way data flow)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AppState {
    pub system_health: SystemHealth,
    pub stress_tier: crate::risk::StressTier,
    pub regime: crate::risk::RegimeState,
    pub positions: Vec<crate::position::Position>,
    pub module_states: Vec<ModuleStateEntry>,
    pub vault_total: rust_decimal::Decimal,
    pub vault_pending_harvest: rust_decimal::Decimal,
    pub recent_events: Vec<crate::event::Event>,
    pub cruising_altitude: bool,
    /// Latest quotes for watched symbols.
    pub watchlist_quotes: Vec<crate::broker::QuoteEvent>,
    /// Live account balance from broker.
    pub account: Option<crate::broker::AccountSnapshot>,
    /// Broker connection status.
    pub connection_status: ConnectionStatus,
    /// Aggregate daily P&L.
    pub daily_pnl: rust_decimal::Decimal,
    /// System-side entry halt (governor-driven).
    pub entry_halt_active: bool,
    /// System uptime in seconds.
    pub uptime_secs: f64,
    /// Stocks approaching module trigger levels ("almost orders").
    pub approaching_setups: Vec<ApproachingSetup>,
    /// Flow tab snapshot — updated by FlowManager.
    pub flow: FlowSnapshot,
    /// Intents pending operator approval.
    pub pending_intents: Vec<PendingIntent>,
    /// Recent fills (most recent first, capped at 100).
    pub recent_fills: Vec<FillRecord>,
    /// ExecCore metrics counters.
    pub exec_metrics: ExecMetrics,
    /// Historical candles for the selected chart symbol + timeframe.
    pub chart_candles: Vec<CandleBar>,
    /// Which symbol the chart candles are for.
    pub chart_symbol: Option<Symbol>,
    /// Which timeframe the chart candles are for.
    pub chart_timeframe: Timeframe,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            system_health: SystemHealth::Green,
            stress_tier: crate::risk::StressTier::Normal,
            regime: crate::risk::RegimeState::Standalone,
            positions: Vec::new(),
            module_states: Vec::new(),
            vault_total: rust_decimal::Decimal::ZERO,
            vault_pending_harvest: rust_decimal::Decimal::ZERO,
            recent_events: Vec::new(),
            cruising_altitude: false,
            watchlist_quotes: Vec::new(),
            account: None,
            connection_status: ConnectionStatus::Disconnected,
            daily_pnl: rust_decimal::Decimal::ZERO,
            entry_halt_active: false,
            uptime_secs: 0.0,
            approaching_setups: Vec::new(),
            flow: FlowSnapshot::default(),
            pending_intents: Vec::new(),
            recent_fills: Vec::new(),
            exec_metrics: ExecMetrics::default(),
            chart_candles: Vec::new(),
            chart_symbol: None,
            chart_timeframe: Timeframe::Day,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemHealth {
    Green,
    Yellow,
    Red,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connected,
    Disconnected,
    Reconnecting { attempt: u32 },
}

#[derive(Debug, Clone)]
pub struct ModuleStateEntry {
    pub module: crate::module::ModuleId,
    pub state: crate::module::ModuleState,
    pub signals_generated: u32,
    pub signals_approved: u32,
    pub signals_rejected: u32,
    pub pending_intent: Option<crate::order::OrderIntent>,
}

// ---------------------------------------------------------------------------
// ApproachingSetup — "almost orders" for stocks near trigger levels
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ApproachingSetup {
    pub symbol: Symbol,
    pub module: ModuleId,
    /// Human-readable criteria description, e.g. "RSI 34 → threshold 30"
    pub criteria: String,
    /// How close to triggering (0.0 = triggered, 100.0 = far away).
    pub distance_pct: f64,
    /// Whether getting closer or farther from trigger.
    pub direction: ApproachDir,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApproachDir {
    /// Getting closer to trigger threshold.
    Heating,
    /// Moving away from trigger threshold.
    Cooling,
}
