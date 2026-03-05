//! Risk mesh overlay panel.
//!
//! Shows position heat, daily P&L, margin, per-module exposure.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;
use rust_decimal::Decimal;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(60, 50, area);
    f.render_widget(Clear, popup);

    let state = &app.state;

    let pnl_style = if state.daily_pnl >= Decimal::ZERO {
        ColorSkin::profit()
    } else {
        ColorSkin::loss()
    };

    // Bind formatted strings to variables so references outlive the vec
    let stress_str = format!("{}", state.stress_tier);
    let regime_str = format!("{}", state.regime);
    let pnl_str = format!("${:+.2}", state.daily_pnl);
    let positions_str = state.positions.len().to_string();
    let vault_str = format!("${:.2}", state.vault_total);
    let vault_pending_str = format!("${:.2}", state.vault_pending_harvest);
    let halt_str = if state.entry_halt_active { "ON" } else { "OFF" };
    let uptime_str = format!("{:.0}s", state.uptime_secs);

    let lines = vec![
        Line::from(""),
        metric_line("Stress Tier", &stress_str),
        metric_line("Regime", &regime_str),
        Line::from(vec![
            Span::styled("  Daily P&L       ", ColorSkin::label()),
            Span::styled(&pnl_str, pnl_style),
        ]),
        metric_line("Open Positions", &positions_str),
        metric_line("Vault Total", &vault_str),
        metric_line("Vault Pending", &vault_pending_str),
        metric_line("Entry Halt", halt_str),
        metric_line("Uptime", &uptime_str),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Risk Mesh [ESC to close] ", ColorSkin::header()));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn metric_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<18}", label), ColorSkin::label()),
        Span::styled(value, ColorSkin::value()),
    ])
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
