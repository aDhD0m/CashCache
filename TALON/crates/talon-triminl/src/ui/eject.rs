//! EJECT confirmation overlay.
//!
//! Single keystroke kill switch — flattens all positions via market orders.
//! Shows confirmation dialog before executing.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect) {
    let popup = centered_rect(40, 20, area);
    f.render_widget(Clear, popup);

    let lines = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  EJECT — FLATTEN ALL POSITIONS",
            ColorSkin::critical(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  This will close ALL open positions",
            ColorSkin::value(),
        )),
        Line::from(Span::styled(
            "  via market orders. Cannot be undone.",
            ColorSkin::value(),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  [Y]", ColorSkin::critical()),
            Span::styled(" Confirm   ", ColorSkin::value()),
            Span::styled("[ESC]", ColorSkin::active()),
            Span::styled(" Cancel", ColorSkin::value()),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(ColorSkin::critical())
        .title(Span::styled(" EJECT ", ColorSkin::critical()));

    f.render_widget(
        Paragraph::new(lines).block(block).alignment(Alignment::Left),
        popup,
    );
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
