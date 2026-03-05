//! Center column — stock detail panel.
//!
//! Shows: ticker header, fundamentals block, timeframe tabs, candlestick chart.
//! Data source: AppState.watchlist_quotes (selected ticker) + AppState.chart_candles.

use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;
use rust_decimal::Decimal;

use talon_types::broker::Timeframe;

use crate::app::App;
use crate::skin::ColorSkin;
use crate::ui::candlestick;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let quote = match app.selected_quote() {
        Some(q) => q,
        None => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(" Stock Detail ", ColorSkin::header()));
            f.render_widget(
                Paragraph::new("  Select a ticker from the watchlist").block(block),
                area,
            );
            return;
        }
    };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),     // ticker header
            Constraint::Length(5),     // fundamentals
            Constraint::Length(1),     // timeframe tabs
            Constraint::Min(5),        // candlestick chart
            Constraint::Length(1),     // chart footer
        ])
        .split(area);

    // --- Ticker Header ---
    let change = quote.prev_close.map(|pc| quote.last - pc).unwrap_or(Decimal::ZERO);
    let change_pct = quote.change_pct().unwrap_or(Decimal::ZERO);
    let change_style = if change >= Decimal::ZERO {
        ColorSkin::profit()
    } else {
        ColorSkin::loss()
    };

    let header_line = Line::from(vec![
        Span::styled(format!(" {} ", quote.symbol), ColorSkin::header()),
        Span::styled(format!("  {:.2}", quote.last), ColorSkin::value()),
        Span::styled(format!("  {:+.2} ({:+.1}%)", change, change_pct), change_style),
    ]);

    let status_line = Line::from(vec![
        Span::styled("  Status: ", ColorSkin::label()),
        Span::styled("Trading", ColorSkin::status_ok()),
    ]);

    let header = Paragraph::new(vec![Line::from(""), header_line, status_line]);
    f.render_widget(header, sections[0]);

    // --- Fundamentals ---
    draw_fundamentals(f, sections[1], quote);

    // --- Timeframe Tabs ---
    draw_timeframe_tabs(f, sections[2], app.stock_detail.timeframe);

    // --- Candlestick Chart ---
    let candles = &app.state.chart_candles;
    candlestick::draw(f, sections[3], candles);

    // --- Chart Footer ---
    if !candles.is_empty() {
        let high = candles.iter().map(|c| c.high).max().unwrap_or(Decimal::ZERO);
        let low = candles.iter().map(|c| c.low).min().unwrap_or(Decimal::ZERO);
        let last_close = candles.last().map(|c| c.close).unwrap_or(Decimal::ZERO);
        let total_vol: u64 = candles.iter().map(|c| c.volume).sum();

        let footer_line = Line::from(vec![
            Span::styled(format!(" Price: {:.2}", last_close), ColorSkin::value()),
            Span::styled(format!("  High: {:.2}", high), ColorSkin::profit()),
            Span::styled(format!("  Low: {:.2}", low), ColorSkin::loss()),
            Span::styled(format!("  Vol: {}", format_volume(total_vol)), ColorSkin::muted()),
        ]);
        f.render_widget(Paragraph::new(footer_line), sections[4]);
    }
}

fn draw_fundamentals(f: &mut Frame, area: Rect, quote: &talon_types::broker::QuoteEvent) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let open_str = quote.day_open.map(|o| format!("{:.2}", o)).unwrap_or_else(|| "---".into());
    let prev_close_str = quote.prev_close.map(|p| format!("{:.2}", p)).unwrap_or_else(|| "---".into());
    let high_str = quote.day_high.map(|h| format!("{:.2}", h)).unwrap_or_else(|| "---".into());
    let low_str = quote.day_low.map(|l| format!("{:.2}", l)).unwrap_or_else(|| "---".into());
    let vol_str = format_volume(quote.volume);

    let left_lines = vec![
        fund_line("Open", &open_str),
        fund_line("Prev Close", &prev_close_str),
        fund_line("High", &high_str),
        fund_line("Low", &low_str),
        fund_line("Volume", &vol_str),
    ];

    let rvol_str = quote.rvol().map(|r| format!("{:.1}x", r)).unwrap_or_else(|| "---".into());
    let bid_str = if quote.bid.is_zero() { "---".into() } else { format!("{:.2}", quote.bid) };
    let ask_str = if quote.ask.is_zero() { "---".into() } else { format!("{:.2}", quote.ask) };
    let spread_str = if !quote.bid.is_zero() && !quote.ask.is_zero() {
        format!("{:.2}", quote.ask - quote.bid)
    } else {
        "---".into()
    };
    let avg_vol_str = quote.avg_volume.map(format_volume).unwrap_or_else(|| "---".into());

    let right_lines = vec![
        fund_line("Bid", &bid_str),
        fund_line("Ask", &ask_str),
        fund_line("Spread", &spread_str),
        fund_line("RVOL", &rvol_str),
        fund_line("Avg Vol", &avg_vol_str),
    ];

    let left = Paragraph::new(left_lines)
        .block(Block::default().borders(Borders::RIGHT));
    let right = Paragraph::new(right_lines);

    f.render_widget(left, cols[0]);
    f.render_widget(right, cols[1]);
}

fn fund_line<'a>(label: &'a str, value: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::styled(format!("  {:<14}", label), ColorSkin::label()),
        Span::styled(value, ColorSkin::value()),
    ])
}

fn draw_timeframe_tabs(f: &mut Frame, area: Rect, active: Timeframe) {
    let spans: Vec<Span> = Timeframe::ALL
        .iter()
        .flat_map(|&tf| {
            let style = if tf == active {
                ColorSkin::active()
            } else {
                ColorSkin::muted()
            };
            vec![
                Span::styled(format!(" {} ", tf.label()), style),
                Span::styled("|", ColorSkin::label()),
            ]
        })
        .collect();

    let line = Line::from(vec![Span::styled("  ", ColorSkin::label())]
        .into_iter()
        .chain(spans)
        .collect::<Vec<_>>());

    f.render_widget(Paragraph::new(line), area);
}

fn format_volume(vol: u64) -> String {
    if vol >= 1_000_000 {
        format!("{:.1}M", vol as f64 / 1_000_000.0)
    } else if vol >= 1_000 {
        format!("{:.1}K", vol as f64 / 1_000.0)
    } else {
        vol.to_string()
    }
}
