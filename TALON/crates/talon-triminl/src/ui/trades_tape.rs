//! Right bottom — live trades tape.
//!
//! Columns: TIME | PRICE | DIR | VOL
//! Direction arrows: up-tick = profit color, down-tick = loss color.
//! Auto-scrolls, newest at top.
//! Data source: AppState.flow.tape (real T&S from IBKR via FlowManager).

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use talon_types::flow::TradeSide;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let max_lines = area.height.saturating_sub(2) as usize;
    let tape = &app.state.flow.tape;

    if tape.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Trades ", ColorSkin::header()));
        let msg = Paragraph::new("  Waiting for T&S...")
            .style(ColorSkin::muted())
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let lines: Vec<Line> = tape
        .iter()
        .take(max_lines)
        .map(|entry| {
            let ts = entry.time.format("%H:%M:%S");
            let (arrow, side_style) = match entry.side {
                TradeSide::Buy => ("\u{25b2}", ColorSkin::profit()),   // ▲
                TradeSide::Sell => ("\u{25bc}", ColorSkin::loss()),     // ▼
                TradeSide::Unknown => ("\u{25cf}", ColorSkin::muted()), // ●
            };

            Line::from(vec![
                Span::styled(format!("{ts} "), ColorSkin::muted()),
                Span::styled(format!("{:.2}", entry.price), side_style),
                Span::styled(format!(" {arrow}"), side_style),
                Span::styled(format!(" {:.0}", entry.size), side_style),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Trades ", ColorSkin::header()));

    f.render_widget(Paragraph::new(lines).block(block), area);
}
