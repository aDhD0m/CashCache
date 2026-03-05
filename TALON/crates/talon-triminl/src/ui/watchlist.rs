//! Left column — scrollable watchlist table.
//!
//! Columns: SYM | PRICE | CHG%
//! Color-coded by change sign. Selected row highlighted.
//! Navigation: j/k or Up/Down.
//! Data source: AppState.watchlist_quotes (real data from IBKR/Polygon/Yahoo).

use ratatui::layout::Constraint;
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Row, Table};
use ratatui::Frame;
use ratatui::layout::Rect;
use rust_decimal::Decimal;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let visible_rows = area.height.saturating_sub(3) as usize;
    let quotes = &app.state.watchlist_quotes;

    if quotes.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Watchlist (waiting) ", ColorSkin::header()));
        let msg = ratatui::widgets::Paragraph::new("  Waiting for market data...")
            .style(ColorSkin::muted())
            .block(block);
        f.render_widget(msg, area);
        return;
    }

    let rows: Vec<Row> = quotes
        .iter()
        .enumerate()
        .skip(app.watchlist.scroll_offset)
        .take(visible_rows)
        .map(|(i, quote)| {
            let change = quote
                .change_pct()
                .filter(|c| c.abs() < Decimal::from(999));

            let change_str = change
                .map(|c| format!("{:+.1}%", c))
                .unwrap_or_default();

            let change_style = change
                .map(|c| {
                    if c >= Decimal::ZERO {
                        ColorSkin::profit()
                    } else {
                        ColorSkin::loss()
                    }
                })
                .unwrap_or(ColorSkin::muted());

            let row = Row::new(vec![
                Span::styled(format!(" {:<5}", quote.symbol), ColorSkin::value()),
                Span::styled(format!("{:>8.2}", quote.last), ColorSkin::value()),
                Span::styled(format!("{:>7}", change_str), change_style),
            ]);

            if i == app.watchlist.selected {
                row.style(ColorSkin::selected())
            } else {
                row
            }
        })
        .collect();

    let count = quotes.len();
    let title = format!(" Watchlist ({count}) ");

    let table = Table::new(
        rows,
        [
            Constraint::Length(7),  // SYM
            Constraint::Length(9),  // PRICE
            Constraint::Min(7),    // CHG%
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, ColorSkin::header())),
    )
    .header(
        Row::new(vec!["SYM", "PRICE", "CHG%"]).style(ColorSkin::label()),
    );

    f.render_widget(table, area);
}
