//! Bottom ticker bar — scrolling/static index quotes.
//!
//! US mode: SPY, QQQ, DIA (S&P 500, NASDAQ-100, Dow Jones ETFs).
//! Data source: AppState.watchlist_quotes (real data).

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use rust_decimal::Decimal;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let indices = [("SPY", "S&P 500"), ("QQQ", "NASDAQ"), ("DIA", "Dow")];

    let mut spans: Vec<Span> = vec![Span::styled(" ", ColorSkin::muted())];

    for (sym, label) in &indices {
        let quote = app
            .state
            .watchlist_quotes
            .iter()
            .find(|q| q.symbol.0 == *sym);

        let Some(q) = quote else {
            continue;
        };

        let change_pct = q.change_pct().unwrap_or(Decimal::ZERO);
        let change_style = if change_pct >= Decimal::ZERO {
            ColorSkin::profit()
        } else {
            ColorSkin::loss()
        };

        spans.push(Span::styled(format!("{label} "), ColorSkin::muted()));
        spans.push(Span::styled(format!("{:.2} ", q.last), ColorSkin::value()));
        spans.push(Span::styled(format!("{:+.2}%", change_pct), change_style));
        spans.push(Span::styled("   ", ColorSkin::muted()));
    }

    if spans.len() == 1 {
        spans.push(Span::styled("Waiting for index data...", ColorSkin::muted()));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
}
