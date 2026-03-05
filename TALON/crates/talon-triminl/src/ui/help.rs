//! Help overlay — full keybinding reference.
//!
//! Toggled via `?` key.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect) {
    let popup = centered_rect(60, 70, area);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  TRiMiNL v3.2.0 — Keyboard Reference",
            ColorSkin::header(),
        )),
        Line::from(""),
        section("General"),
        key_line("?", "Show/hide this help"),
        key_line("`", "Toggle debug log"),
        key_line("q / Ctrl+C", "Quit"),
        key_line("Ctrl+X", "EMERGENCY SHUTDOWN"),
        key_line("1 / 2", "Switch Watchlist / Portfolio"),
        Line::from(""),
        section("Watchlist"),
        key_line("j / k / Up / Down", "Navigate watchlist"),
        key_line("t", "Toggle detail view"),
        key_line("G", "Switch watchlist group"),
        Line::from(""),
        section("Stock Detail"),
        key_line("h / l / Left / Right", "Switch candlestick interval"),
        key_line("Tab / Shift+Tab", "Next / prev timeframe"),
        Line::from(""),
        section("Trading"),
        key_line("a", "Approve selected signal"),
        key_line("r", "Reject selected signal"),
        key_line("A (Shift)", "Approve all pending"),
        key_line("R (Shift)", "Reject all pending"),
        key_line("H", "Toggle entry halt"),
        key_line("K", "EJECT — flatten all positions"),
        Line::from(""),
        section("Overlays"),
        key_line("m", "Module status grid"),
        key_line("x", "Risk mesh panel"),
        key_line("ESC", "Close overlay"),
        Line::from(""),
        Line::from(Span::styled(
            "  Press ? or ESC to close",
            ColorSkin::muted(),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Help ", ColorSkin::header()));

    f.render_widget(
        Paragraph::new(lines).block(block).alignment(Alignment::Left),
        popup,
    );
}

fn section(name: &str) -> Line<'_> {
    Line::from(Span::styled(format!("  {name}"), ColorSkin::header()))
}

fn key_line<'a>(key: &'a str, desc: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("    {:<24}", key), ColorSkin::active()),
        Span::styled(desc, ColorSkin::value()),
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
