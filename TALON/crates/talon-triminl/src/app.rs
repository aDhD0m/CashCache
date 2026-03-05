use std::time::Instant;

use tokio::sync::mpsc;

use talon_types::broker::Timeframe;
use talon_types::channel::AppState;
use talon_types::exec::SupervisionCommand;
use talon_types::flow::{ChartCmd, FlowCmd};

/// Transient status notification with auto-expiry.
pub struct StatusMessage {
    pub text: String,
    pub created_at: Instant,
}

impl StatusMessage {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            created_at: Instant::now(),
        }
    }

    pub fn text_if_fresh(&self, ttl_secs: f64) -> Option<&str> {
        if self.created_at.elapsed().as_secs_f64() < ttl_secs {
            Some(&self.text)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Mode — top-level view mode (Longbridge-style tabs)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 3-column: Watchlist | Stock Detail + Chart | Order Book + Trades
    Watchlist,
    /// Portfolio view — expanded position table
    Portfolio,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Watchlist => "WATCHLIST",
            Self::Portfolio => "PORTFOLIO",
        }
    }

    pub fn key(self) -> char {
        match self {
            Self::Watchlist => '1',
            Self::Portfolio => '2',
        }
    }
}

// ---------------------------------------------------------------------------
// Overlay — toggled panels that appear over the main layout
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overlay {
    None,
    Help,
    DebugLog,
    Modules,
    RiskMesh,
    Eject,
}

// ---------------------------------------------------------------------------
// Watchlist state
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct WatchlistState {
    pub selected: usize,
    pub scroll_offset: usize,
}


// ---------------------------------------------------------------------------
// Stock detail state
// ---------------------------------------------------------------------------

pub struct StockDetailState {
    pub timeframe: Timeframe,
}

impl Default for StockDetailState {
    fn default() -> Self {
        Self {
            timeframe: Timeframe::Day,
        }
    }
}

// ---------------------------------------------------------------------------
// App — TUI application state owned by the render loop
// ---------------------------------------------------------------------------

pub struct App {
    pub should_quit: bool,
    pub status: Option<StatusMessage>,
    pub mode: Mode,
    pub overlay: Overlay,
    /// Latest snapshot from the watch channel.
    pub state: AppState,
    /// Operator entry-halt toggle (TUI-local).
    pub entry_halt: bool,
    /// Command channel to FlowManager.
    pub flow_cmd_tx: mpsc::Sender<FlowCmd>,
    /// Command channel to ExecCore for supervision.
    pub supervision_tx: mpsc::Sender<SupervisionCommand>,
    /// Command channel for chart data requests.
    pub chart_cmd_tx: mpsc::Sender<ChartCmd>,
    /// Selected row in PendingIntents.
    pub pending_cursor: usize,
    /// Flash effect for new target lock.
    pub target_lock_flash: Option<Instant>,
    pub prev_pending_count: usize,

    // --- Longbridge-style state ---
    pub watchlist: WatchlistState,
    pub stock_detail: StockDetailState,
}

impl App {
    pub fn new(
        initial_state: AppState,
        flow_cmd_tx: mpsc::Sender<FlowCmd>,
        supervision_tx: mpsc::Sender<SupervisionCommand>,
        chart_cmd_tx: mpsc::Sender<ChartCmd>,
    ) -> Self {
        Self {
            should_quit: false,
            status: None,
            mode: Mode::Watchlist,
            overlay: Overlay::None,
            state: initial_state,
            entry_halt: false,
            flow_cmd_tx,
            supervision_tx,
            chart_cmd_tx,
            pending_cursor: 0,
            target_lock_flash: None,
            prev_pending_count: 0,
            watchlist: WatchlistState::default(),
            stock_detail: StockDetailState::default(),
        }
    }

    pub fn update_target_lock(&mut self) {
        let current_count = self.state.pending_intents.len();
        if current_count > self.prev_pending_count {
            self.target_lock_flash = Some(Instant::now());
        }
        self.prev_pending_count = current_count;
    }

    pub fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some(StatusMessage::new(text));
    }

    /// Get the currently selected quote from AppState.
    pub fn selected_quote(&self) -> Option<&talon_types::broker::QuoteEvent> {
        self.state.watchlist_quotes.get(self.watchlist.selected)
    }

    /// Get the symbol name of the currently selected watchlist entry.
    pub fn selected_symbol(&self) -> Option<&talon_types::position::Symbol> {
        self.selected_quote().map(|q| &q.symbol)
    }
}
