//! TRiMiNL layout — Longbridge Terminal-style 3-column design.
//!
//! ```text
//! +-----------------------------------------------------------------------+
//! | WATCHLIST [1] | PORTFOLIO [2]    TALON | Hatch | IBKR: Connected     |
//! +-----------------------------------------------------------------------+
//! | WATCHLIST     | STOCK DETAIL                  | ORDER BOOK           |
//! | SYM PRICE CHG | Ticker Price Chg             | Bid: price  sz       |
//! |               | OHLC / PE / EPS / Vol        | Ask: price  sz       |
//! | (scrollable,  |                               |----------------------|
//! |  color-coded) | [1m 5m 15m 30m 1h Day Wk Mo] | TRADES TAPE          |
//! |               | +--CANDLESTICK CHART--------+ | ts price dir vol     |
//! |               | | OHLC candles + vol bars   | |                      |
//! |               | +---------------------------+ |                      |
//! +-----------------------------------------------------------------------+
//! | SPY 520.10 +0.3%  QQQ 440.80 -0.2%  DIA 395.60 +0.1%               |
//! +-----------------------------------------------------------------------+
//! | [a] Approve  [r] Reject  [K] Eject  [Ctrl+X] Emergency  [Q] Quit    |
//! +-----------------------------------------------------------------------+
//! ```

pub mod watchlist;
pub mod stock_detail;
pub mod candlestick;
pub mod order_book;
pub mod trades_tape;
pub mod ticker_bar;
pub mod modules;
pub mod signal_queue;
pub mod risk_panel;
pub mod eject;
pub mod debug_log;
pub mod help;

use chrono::{Local, Timelike};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use talon_types::channel::ConnectionStatus;

use crate::app::{App, Mode, Overlay};
use crate::skin::ColorSkin;

const STATUS_TTL_SECS: f64 = 5.0;
const MIN_WIDTH: u16 = 80;
const MIN_HEIGHT: u16 = 15;

/// Draw the full TRiMiNL UI.
pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();

    // Guard: minimum terminal size
    if area.width < MIN_WIDTH || area.height < MIN_HEIGHT {
        let msg = format!(
            "Terminal too small: {}x{}\nMinimum: {}x{}",
            area.width, area.height, MIN_WIDTH, MIN_HEIGHT
        );
        f.render_widget(
            Paragraph::new(msg)
                .alignment(Alignment::Center)
                .block(Block::default().borders(Borders::ALL).title(" TRiMiNL ")),
            area,
        );
        return;
    }

    let has_notification = app
        .status
        .as_ref()
        .and_then(|s| s.text_if_fresh(STATUS_TTL_SECS))
        .is_some();
    let notif_height: u16 = if has_notification { 1 } else { 0 };

    // Has pending intents → show signal queue banner
    let has_pending = !app.state.pending_intents.is_empty();
    let signal_height: u16 = if has_pending { 4 } else { 0 };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),                 // header + tab bar
            Constraint::Length(signal_height),      // signal approval queue
            Constraint::Min(10),                   // body (3-column or portfolio)
            Constraint::Length(1),                  // ticker bar
            Constraint::Length(notif_height),       // notification
            Constraint::Length(1),                  // footer
        ])
        .split(area);

    draw_header(f, chunks[0], app);

    if has_pending {
        signal_queue::draw(f, chunks[1], app);
    }

    match app.mode {
        Mode::Watchlist => draw_watchlist_mode(f, chunks[2], app),
        Mode::Portfolio => draw_portfolio_mode(f, chunks[2], app),
    }

    ticker_bar::draw(f, chunks[3], app);

    if has_notification {
        draw_notification(f, chunks[4], app);
    }

    draw_footer(f, chunks[5], app);

    // Overlay rendering (on top of everything)
    match app.overlay {
        Overlay::Help => help::draw(f, area),
        Overlay::DebugLog => debug_log::draw(f, area, app),
        Overlay::Modules => modules::draw(f, area, app),
        Overlay::RiskMesh => risk_panel::draw(f, area, app),
        Overlay::Eject => eject::draw(f, area),
        Overlay::None => {}
    }
}

/// 3-column Longbridge layout: Watchlist | Stock Detail | Order Book + Trades
fn draw_watchlist_mode(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),    // left: watchlist
            Constraint::Min(40),       // center: stock detail + chart
            Constraint::Length(30),    // right: order book + trades
        ])
        .split(area);

    watchlist::draw(f, cols[0], app);
    stock_detail::draw(f, cols[1], app);

    // Right column split: order book (top) + trades tape (bottom)
    let right_split = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),     // order book (compact L1)
            Constraint::Min(5),        // trades tape
        ])
        .split(cols[2]);

    order_book::draw(f, right_split[0], app);
    trades_tape::draw(f, right_split[1], app);
}

/// Portfolio mode — full-width position table
fn draw_portfolio_mode(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    let rows: Vec<ratatui::widgets::Row> = state
        .positions
        .iter()
        .map(|pos| {
            let pnl = pos.unrealized_pnl();
            let pnl_pct = pos.unrealized_pnl_pct();
            let pnl_color = if pnl >= rust_decimal::Decimal::ZERO {
                ColorSkin::profit()
            } else {
                ColorSkin::loss()
            };

            ratatui::widgets::Row::new(vec![
                format!("{:<6}", pos.symbol),
                format!("{:>+5}", pos.qty),
                format!("${:.2}", pos.avg_entry),
                format!("${:.2}", pos.current_price),
                format!("{:>+.1}%", pnl_pct),
                format!("${:>+.0}", pnl),
                format!("{}", pos.module),
                pos.stop_loss
                    .map(|s| format!("${s}"))
                    .unwrap_or_default(),
                pos.take_profit
                    .map(|t| format!("${t}"))
                    .unwrap_or_default(),
            ])
            .style(pnl_color)
        })
        .collect();

    let table = ratatui::widgets::Table::new(
        rows,
        [
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(8),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Length(10),
            Constraint::Min(10),
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Portfolio (Active) ", ColorSkin::header())),
    )
    .header(
        ratatui::widgets::Row::new(vec![
            "SYMBOL", "QTY", "ENTRY", "CURRENT", "P&L %", "P&L $", "MODULE", "STOP", "TARGET",
        ])
        .style(ColorSkin::label()),
    );

    f.render_widget(table, area);
}

// ---------------------------------------------------------------------------
// Header — tab bar + branding + status
// ---------------------------------------------------------------------------

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Fill(1), Constraint::Min(50)])
        .split(area);

    // Left: tab bar + branding
    let tab1_style = if app.mode == Mode::Watchlist {
        ColorSkin::active()
    } else {
        ColorSkin::muted()
    };
    let tab2_style = if app.mode == Mode::Portfolio {
        ColorSkin::active()
    } else {
        ColorSkin::muted()
    };

    let stress_label = format!("{}", state.stress_tier);
    let stress_style = match state.stress_tier {
        talon_types::risk::StressTier::Normal => ColorSkin::status_ok(),
        talon_types::risk::StressTier::Tier1 | talon_types::risk::StressTier::Tier2 => {
            ColorSkin::status_warn()
        }
        talon_types::risk::StressTier::Flameout | talon_types::risk::StressTier::Nosedive => {
            ColorSkin::critical()
        }
    };

    let left_line = Line::from(vec![
        Span::styled(" WATCHLIST [1] ", tab1_style),
        Span::styled(" PORTFOLIO [2] ", tab2_style),
        Span::styled("  TALON ", ColorSkin::header()),
        Span::styled(" | ", ColorSkin::label()),
        Span::styled(stress_label, stress_style),
        Span::styled(" | ", ColorSkin::label()),
        Span::styled(format!("{}", state.regime), ColorSkin::value()),
    ]);

    // Right: clock + connection + account
    let now = Local::now();
    let clock = format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second());
    let market = market_status_label(&now);
    let conn_label = connection_label(state.connection_status);
    let conn_style = connection_style(state.connection_status);

    let account_span = if let Some(ref acct) = state.account {
        format!("NLV ${:.0}  BP ${:.0}", acct.net_liquidation, acct.buying_power)
    } else {
        "[no account]".to_string()
    };
    let account_style = if state.account.is_some() {
        ColorSkin::value()
    } else {
        ColorSkin::muted()
    };

    let right_line = Line::from(vec![
        Span::styled(market, ColorSkin::value()),
        Span::styled("  ", ColorSkin::label()),
        Span::styled(clock, ColorSkin::muted()),
        Span::styled("  ", ColorSkin::label()),
        Span::styled(conn_label, conn_style),
        Span::styled("  ", ColorSkin::label()),
        Span::styled(account_span, account_style),
        Span::raw(" "),
    ]);

    let left = Paragraph::new(left_line).block(Block::default().borders(Borders::BOTTOM));
    let right = Paragraph::new(right_line)
        .alignment(Alignment::Right)
        .block(Block::default().borders(Borders::BOTTOM));

    f.render_widget(left, cols[0]);
    f.render_widget(right, cols[1]);
}

// ---------------------------------------------------------------------------
// Notification bar
// ---------------------------------------------------------------------------

fn draw_notification(f: &mut Frame, area: Rect, app: &App) {
    let text = app
        .status
        .as_ref()
        .and_then(|s| s.text_if_fresh(STATUS_TTL_SECS))
        .unwrap_or("");

    let line = Line::from(vec![
        Span::styled(" >> ", ColorSkin::active()),
        Span::styled(text, ColorSkin::value()),
    ]);

    f.render_widget(Paragraph::new(line), area);
}

// ---------------------------------------------------------------------------
// Footer
// ---------------------------------------------------------------------------

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let has_pending = !app.state.pending_intents.is_empty();

    let footer_line = if has_pending {
        Line::from(vec![
            Span::styled(" [a]", ColorSkin::active()),
            Span::styled(" Approve ", ColorSkin::value()),
            Span::styled("[r]", ColorSkin::active()),
            Span::styled(" Reject ", ColorSkin::value()),
            Span::styled("[A]", ColorSkin::active()),
            Span::styled(" All ", ColorSkin::value()),
            Span::styled("[R]", ColorSkin::active()),
            Span::styled(" Rej All ", ColorSkin::value()),
            Span::styled("[K]", ColorSkin::active()),
            Span::styled(" Eject ", ColorSkin::value()),
            Span::styled("[?]", ColorSkin::active()),
            Span::styled(" Help ", ColorSkin::value()),
            Span::styled("[Ctrl+X]", ColorSkin::critical()),
            Span::styled(" Emergency ", ColorSkin::value()),
            Span::styled("[Q]", ColorSkin::active()),
            Span::styled(" Quit", ColorSkin::value()),
        ])
    } else {
        let halt_indicator = if app.entry_halt || app.state.entry_halt_active {
            " ON"
        } else {
            ""
        };

        Line::from(vec![
            Span::styled(" [j/k]", ColorSkin::active()),
            Span::styled(" Nav ", ColorSkin::value()),
            Span::styled("[h/l]", ColorSkin::active()),
            Span::styled(" Timeframe ", ColorSkin::value()),
            Span::styled("[H]", ColorSkin::active()),
            Span::styled(format!(" Halt{halt_indicator} "), ColorSkin::value()),
            Span::styled("[K]", ColorSkin::active()),
            Span::styled(" Eject ", ColorSkin::value()),
            Span::styled("[?]", ColorSkin::active()),
            Span::styled(" Help ", ColorSkin::value()),
            Span::styled("[`]", ColorSkin::active()),
            Span::styled(" Debug ", ColorSkin::value()),
            Span::styled("[Q]", ColorSkin::active()),
            Span::styled(" Quit", ColorSkin::value()),
        ])
    };

    f.render_widget(Paragraph::new(footer_line), area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn connection_label(status: ConnectionStatus) -> String {
    match status {
        ConnectionStatus::Connected => "Connected".to_string(),
        ConnectionStatus::Disconnected => "Disconnected".to_string(),
        ConnectionStatus::Reconnecting { attempt } => format!("Recon ({attempt})"),
    }
}

fn connection_style(status: ConnectionStatus) -> ratatui::style::Style {
    match status {
        ConnectionStatus::Connected => ColorSkin::status_ok(),
        ConnectionStatus::Disconnected => ColorSkin::status_error(),
        ConnectionStatus::Reconnecting { .. } => ColorSkin::status_warn(),
    }
}

fn market_status_label(now: &chrono::DateTime<Local>) -> &'static str {
    let time_mins = now.hour() * 60 + now.minute();
    if time_mins < 240 {
        "CLOSED"
    } else if time_mins < 570 {
        "PRE-MKT"
    } else if time_mins < 960 {
        "OPEN"
    } else if time_mins < 1200 {
        "AFTER-HRS"
    } else {
        "CLOSED"
    }
}
