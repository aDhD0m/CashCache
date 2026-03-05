use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use talon_types::broker::Timeframe;
use talon_types::exec::SupervisionCommand;
use talon_types::flow::ChartCmd;

use crate::app::{App, Mode, Overlay};

/// Process a single key event. Returns `true` if the app should continue,
/// `false` if it should quit.
pub fn handle_key(app: &mut App, key: KeyEvent) -> bool {
    // Emergency shutdown — Ctrl+X (always active, even over overlays)
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('x') {
        tracing::error!("EMERGENCY SHUTDOWN triggered by operator");
        app.set_status("EMERGENCY SHUTDOWN ACTIVATED");
        app.should_quit = true;
        return false;
    }

    // Overlay-specific keys — ESC closes any overlay
    if app.overlay != Overlay::None {
        return handle_overlay_key(app, key);
    }

    match (key.code, key.modifiers) {
        // Quit
        (KeyCode::Char('q'), KeyModifiers::NONE)
        | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.should_quit = true;
            return false;
        }

        // --- Mode switching ---
        (KeyCode::Char('1'), KeyModifiers::NONE) => app.mode = Mode::Watchlist,
        (KeyCode::Char('2'), KeyModifiers::NONE) => app.mode = Mode::Portfolio,

        // --- Overlay toggles ---
        (KeyCode::Char('?'), _) => {
            app.overlay = Overlay::Help;
        }
        (KeyCode::Char('`'), KeyModifiers::NONE) => {
            app.overlay = Overlay::DebugLog;
        }
        (KeyCode::Char('m'), KeyModifiers::NONE) => {
            app.overlay = Overlay::Modules;
        }
        (KeyCode::Char('x'), KeyModifiers::NONE) => {
            app.overlay = Overlay::RiskMesh;
        }

        // --- EJECT (capital K) ---
        (KeyCode::Char('K'), _) => {
            app.overlay = Overlay::Eject;
        }

        // --- Entry halt toggle (capital H) ---
        (KeyCode::Char('H'), _) => {
            app.entry_halt = !app.entry_halt;
            app.set_status(if app.entry_halt {
                "Entry halt ON"
            } else {
                "Entry halt OFF"
            });
        }

        // --- Watchlist navigation (j/k/Up/Down) ---
        (KeyCode::Char('j') | KeyCode::Down, _) => {
            let max = app.state.watchlist_quotes.len().saturating_sub(1);
            if app.watchlist.selected < max {
                app.watchlist.selected += 1;
                request_chart_data(app);
            }
        }
        (KeyCode::Char('k') | KeyCode::Up, _) => {
            if app.watchlist.selected > 0 {
                app.watchlist.selected -= 1;
                request_chart_data(app);
            }
        }

        // --- Stock detail timeframe (h/l / Left/Right) ---
        (KeyCode::Char('h') | KeyCode::Left, _) => {
            app.stock_detail.timeframe = prev_timeframe(app.stock_detail.timeframe);
            request_chart_data(app);
        }
        (KeyCode::Char('l') | KeyCode::Right, _) => {
            app.stock_detail.timeframe = next_timeframe(app.stock_detail.timeframe);
            request_chart_data(app);
        }
        (KeyCode::Tab, KeyModifiers::NONE) => {
            app.stock_detail.timeframe = next_timeframe(app.stock_detail.timeframe);
            request_chart_data(app);
        }
        (KeyCode::BackTab, _) => {
            app.stock_detail.timeframe = prev_timeframe(app.stock_detail.timeframe);
            request_chart_data(app);
        }

        // --- Detail toggle (t) ---
        (KeyCode::Char('t'), KeyModifiers::NONE) => {
            // Toggle between Watchlist and Portfolio as a quick switch
            app.mode = match app.mode {
                Mode::Watchlist => Mode::Portfolio,
                Mode::Portfolio => Mode::Watchlist,
            };
        }

        // --- Signal approval (when pending intents exist) ---
        (KeyCode::Char('a'), KeyModifiers::NONE) => {
            if let Some(pending) = app.state.pending_intents.get(app.pending_cursor) {
                let _ = app
                    .supervision_tx
                    .try_send(SupervisionCommand::Approve(pending.id));
                app.set_status(format!("Approved: {} {}", pending.symbol, pending.side));
            }
        }
        (KeyCode::Char('r'), KeyModifiers::NONE) => {
            if let Some(pending) = app.state.pending_intents.get(app.pending_cursor) {
                let _ = app
                    .supervision_tx
                    .try_send(SupervisionCommand::Reject(pending.id));
                app.set_status(format!("Rejected: {} {}", pending.symbol, pending.side));
            }
        }
        (KeyCode::Char('A'), _) => {
            if !app.state.pending_intents.is_empty() {
                let _ = app
                    .supervision_tx
                    .try_send(SupervisionCommand::ApproveAll);
                app.set_status(format!(
                    "Approved all ({} intents)",
                    app.state.pending_intents.len()
                ));
            }
        }
        (KeyCode::Char('R'), _) => {
            if !app.state.pending_intents.is_empty() {
                let _ = app
                    .supervision_tx
                    .try_send(SupervisionCommand::RejectAll);
                app.set_status(format!(
                    "Rejected all ({} intents)",
                    app.state.pending_intents.len()
                ));
            }
        }

        // ESC — no overlay to close, ignore
        (KeyCode::Esc, _) => {}

        _ => {}
    }

    true
}

/// Handle keys while an overlay is active.
fn handle_overlay_key(app: &mut App, key: KeyEvent) -> bool {
    match app.overlay {
        Overlay::Eject => {
            // EJECT confirmation: Y to confirm, ESC to cancel
            match key.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    tracing::warn!("EJECT confirmed by operator");
                    app.set_status("EJECT — closing all positions");
                    app.overlay = Overlay::None;
                    // TODO: send eject command via broker channel
                }
                KeyCode::Esc => {
                    app.overlay = Overlay::None;
                    app.set_status("EJECT cancelled");
                }
                _ => {}
            }
        }
        Overlay::Help => match key.code {
            KeyCode::Char('?') | KeyCode::Esc => {
                app.overlay = Overlay::None;
            }
            _ => {}
        },
        Overlay::DebugLog => match key.code {
            KeyCode::Char('`') | KeyCode::Esc => {
                app.overlay = Overlay::None;
            }
            _ => {}
        },
        _ => {
            // Modules, RiskMesh — ESC closes
            if key.code == KeyCode::Esc {
                app.overlay = Overlay::None;
            }
        }
    }

    true
}

fn next_timeframe(tf: Timeframe) -> Timeframe {
    let all = Timeframe::ALL;
    let idx = all.iter().position(|&t| t == tf).unwrap_or(0);
    all[(idx + 1) % all.len()]
}

fn prev_timeframe(tf: Timeframe) -> Timeframe {
    let all = Timeframe::ALL;
    let idx = all.iter().position(|&t| t == tf).unwrap_or(0);
    all[(idx + all.len() - 1) % all.len()]
}

/// Send a chart data request for the currently selected symbol + timeframe.
fn request_chart_data(app: &App) {
    if let Some(symbol) = app.selected_symbol() {
        let _ = app.chart_cmd_tx.try_send(ChartCmd::FetchCandles {
            symbol: symbol.clone(),
            timeframe: app.stock_detail.timeframe,
        });
    }
}
