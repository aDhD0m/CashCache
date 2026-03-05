//! Right top — order book (bid/ask depth display).
//!
//! Shows Level 1 (NBBO) or deeper book if available.
//! Data source: AppState.flow.book (real L2 from IBKR via FlowManager).

use ratatui::layout::{Constraint, Rect};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use ratatui::Frame;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let max_levels = area.height.saturating_sub(3) as usize;
    let book = &app.state.flow.book;

    if book.bids.is_empty() && book.asks.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Book ", ColorSkin::header()));
        let msg = Paragraph::new("  Select ticker for L2")
            .style(ColorSkin::muted())
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let mut rows: Vec<Row> = Vec::new();

    // Asks (reversed — best ask closest to spread)
    let ask_count = book.asks.len().min(max_levels / 2);
    for level in book.asks.iter().take(ask_count).rev() {
        rows.push(
            Row::new(vec![
                Span::styled(format!("{:>8.2}", level.price), ColorSkin::loss()),
                Span::styled(format!("{:>8.0}", level.size), ColorSkin::loss()),
            ]),
        );
    }

    // Spread row
    let spread_text = book
        .spread()
        .map(|s| format!("Spread: {:.2}", s))
        .unwrap_or_else(|| "\u{2014}".to_string());
    rows.push(Row::new(vec![
        Span::styled(spread_text, ColorSkin::active()),
        Span::raw(""),
    ]));

    // Bids (best bid at top)
    let bid_count = book.bids.len().min(max_levels / 2);
    for level in book.bids.iter().take(bid_count) {
        rows.push(
            Row::new(vec![
                Span::styled(format!("{:>8.2}", level.price), ColorSkin::profit()),
                Span::styled(format!("{:>8.0}", level.size), ColorSkin::profit()),
            ]),
        );
    }

    let table = Table::new(
        rows,
        [Constraint::Length(12), Constraint::Min(10)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Book ", ColorSkin::header())),
    )
    .header(Row::new(vec!["PRICE", "SIZE"]).style(ColorSkin::label()));

    f.render_widget(table, area);
}
