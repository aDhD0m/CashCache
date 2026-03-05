//! Strategy module grid overlay.
//!
//! Shows all 9 strategy modules with state, signal counts, and trust level.
//! Toggled via key binding.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use talon_types::module::ModuleState;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let popup = centered_rect(70, 60, area);
    f.render_widget(Clear, popup);

    let state = &app.state;

    let lines: Vec<Line> = if state.module_states.is_empty() {
        vec![
            Line::from(""),
            Line::from(Span::styled(
                "  Waiting for module data...",
                ColorSkin::muted(),
            )),
        ]
    } else {
        state
            .module_states
            .iter()
            .map(|ms| {
                let state_style = match ms.state {
                    ModuleState::Scanning => ColorSkin::active(),
                    ModuleState::SignalGenerated | ModuleState::Active => ColorSkin::profit(),
                    ModuleState::Paused | ModuleState::Disabled => ColorSkin::loss(),
                    _ => ColorSkin::muted(),
                };

                Line::from(vec![
                    Span::styled(format!("  {:<14}", ms.module), ColorSkin::value()),
                    Span::styled(format!("{:<12?}", ms.state), state_style),
                    Span::styled(
                        format!(
                            " {}g/{}a/{}r",
                            ms.signals_generated, ms.signals_approved, ms.signals_rejected
                        ),
                        ColorSkin::muted(),
                    ),
                ])
            })
            .collect()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Modules [ESC to close] ", ColorSkin::header()));

    f.render_widget(Paragraph::new(lines).block(block), popup);
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
