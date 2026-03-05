//! Event-driven TUI render loop.
//!
//! Uses `tokio::select!` to wait for either:
//!   1. A new `AppState` snapshot from the `watch` channel, or
//!   2. A crossterm terminal event (key press, resize).
//!
//! Zero CPU when idle — no polling, no fixed-interval redraws.

use std::io::{self, stdout};
use std::time::Duration;

use crossterm::cursor::Hide;
use crossterm::event::{Event, EventStream, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use crossterm::execute;
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use tokio::sync::{mpsc, watch};

use talon_types::channel::AppState;
use talon_types::exec::SupervisionCommand;
use talon_types::flow::{ChartCmd, FlowCmd};

use crate::app::App;
use crate::input;
use crate::panic_hook;
use crate::splash;
use crate::ui;

/// Run the TUI event loop. Returns when the user quits or emergency
/// shutdown is activated.
///
/// This function:
///   1. Installs the panic hook (terminal restoration on panic).
///   2. Enters alternate screen + raw mode.
///   3. Selects on state changes and terminal events.
///   4. Restores terminal on exit.
pub async fn run_tui(
    mut state_rx: watch::Receiver<AppState>,
    flow_cmd_tx: mpsc::Sender<FlowCmd>,
    supervision_tx: mpsc::Sender<SupervisionCommand>,
    chart_cmd_tx: mpsc::Sender<ChartCmd>,
) -> io::Result<()> {
    // Install panic hook before entering raw mode.
    panic_hook::install_panic_hook();

    // Enter alternate screen and raw mode.
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, Hide)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // --- Splash sequence: flash the TALON eagle ---
    for &(style, ms) in splash::SPLASH_FRAMES {
        if let Some(s) = style {
            terminal.draw(|f| splash::draw_splash(f, s))?;
        } else {
            terminal.draw(|_f| {})?;
        }
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }
    terminal.clear()?;

    let mut app = App::new(state_rx.borrow_and_update().clone(), flow_cmd_tx, supervision_tx, chart_cmd_tx);
    let mut event_stream = EventStream::new();

    // Initial draw with current state.
    terminal.draw(|f| ui::draw(f, &app))?;

    loop {
        tokio::select! {
            // Branch 1: AppState changed on the watch channel.
            result = state_rx.changed() => {
                match result {
                    Ok(()) => {
                        app.state = state_rx.borrow_and_update().clone();
                        app.update_target_lock();
                        terminal.draw(|f| ui::draw(f, &app))?;
                    }
                    Err(_) => {
                        // Sender dropped — session is shutting down.
                        break;
                    }
                }
            }

            // Branch 2: Terminal event (key press, resize, etc.).
            maybe_event = event_stream.next() => {
                match maybe_event {
                    Some(Ok(Event::Key(key))) if key.kind == KeyEventKind::Press => {
                        if !input::handle_key(&mut app, key) {
                            break;
                        }
                        // Re-render after key handling (state may have changed).
                        terminal.draw(|f| ui::draw(f, &app))?;
                    }
                    Some(Ok(Event::Resize(_, _))) => {
                        // Terminal resized — force a full redraw.
                        terminal.draw(|f| ui::draw(f, &app))?;
                    }
                    Some(Err(e)) => {
                        tracing::error!("crossterm event error: {}", e);
                        break;
                    }
                    None => {
                        // Event stream ended.
                        break;
                    }
                    // Ignore other event types (mouse, focus, paste).
                    _ => {}
                }
            }
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal.
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    terminal.show_cursor()?;

    Ok(())
}
