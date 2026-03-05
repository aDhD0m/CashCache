//! Debug log panel — toggled via backtick key.
//!
//! Shows timestamped event log with color-coded entries.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    // Bottom half of screen
    let popup = bottom_panel(area, 40);
    f.render_widget(Clear, popup);

    let state = &app.state;
    let max_lines = popup.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = state
        .recent_events
        .iter()
        .rev()
        .take(max_lines)
        .map(|ev| {
            let ts = ev.timestamp.format("%H:%M:%S%.3f");
            let (prefix, color) = match &ev.kind {
                talon_types::event::EventKind::OrderFilled { .. } => ("OK", ColorSkin::P1),
                talon_types::event::EventKind::OrderApproved { .. } => ("GO", ColorSkin::P1),
                talon_types::event::EventKind::OrderRejected { .. } => ("!!", ColorSkin::P2),
                talon_types::event::EventKind::HarvestExecuted { .. } => ("$$", ColorSkin::ACCENT),
                talon_types::event::EventKind::StressMultiplierChanged { .. } => {
                    ("**", ColorSkin::P2)
                }
                talon_types::event::EventKind::FlameoutEngaged { .. } => {
                    ("FL", ColorSkin::DANGER)
                }
                talon_types::event::EventKind::NosediveTriggered { .. } => {
                    ("ND", ColorSkin::DANGER)
                }
                _ => ("--", ColorSkin::MUTED),
            };

            Line::from(vec![
                Span::styled(format!(" {ts} "), ColorSkin::muted()),
                Span::styled(
                    format!("{prefix:>2}"),
                    ratatui::style::Style::default().fg(color),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:?}", std::mem::discriminant(&ev.kind)),
                    ColorSkin::value(),
                ),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Debug Log [` to close] ", ColorSkin::header()));

    f.render_widget(Paragraph::new(lines).block(block), popup);
}

fn bottom_panel(area: Rect, percent_height: u16) -> Rect {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(100 - percent_height),
            Constraint::Percentage(percent_height),
        ])
        .split(area);
    layout[1]
}
