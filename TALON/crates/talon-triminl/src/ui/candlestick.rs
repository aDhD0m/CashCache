//! Custom OHLC candlestick chart widget.
//!
//! Renders candles as vertical lines (wicks) with filled bodies,
//! plus volume bars underneath. Built on direct buffer manipulation
//! for maximum control over the terminal cells.
//! Data source: AppState.chart_candles (real data from Polygon aggregates).

use ratatui::layout::Rect;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::text::Span;
use ratatui::Frame;
use rust_decimal::Decimal;

use talon_types::broker::CandleBar;
use crate::skin::ColorSkin;

/// Unicode block characters for sub-cell rendering.
const FULL_BLOCK: char = '\u{2588}';    // █
const LOWER_HALF: char = '\u{2584}';    // ▄
const VERT_LINE: char = '\u{2502}';     // │

/// Draw a candlestick chart with volume bars.
pub fn draw(f: &mut Frame, area: Rect, candles: &[CandleBar]) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Chart ", ColorSkin::header()));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if candles.is_empty() {
        // Empty state — show message instead of blank space
        if inner.width >= 10 && inner.height >= 2 {
            let msg = Paragraph::new("  Waiting for candle data...")
                .style(ColorSkin::muted());
            f.render_widget(msg, Rect {
                x: inner.x,
                y: inner.y + inner.height / 2,
                width: inner.width,
                height: 1,
            });
        }
        return;
    }

    if inner.width < 4 || inner.height < 4 {
        return;
    }

    // Reserve bottom 20% for volume bars, rest for price candles.
    let vol_height = (inner.height as f64 * 0.2).max(2.0) as u16;
    let price_height = inner.height.saturating_sub(vol_height + 1); // +1 for separator
    let price_area = Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: price_height,
    };
    let vol_area = Rect {
        x: inner.x,
        y: inner.y + price_height + 1,
        width: inner.width,
        height: vol_height.min(inner.height.saturating_sub(price_height + 1)),
    };

    // Layout: Y-axis labels on the left, candles to the right
    let candle_width = 1u16;
    let gap = 1u16;
    let label_width = 8u16;
    let chart_x_start = price_area.x + label_width;
    let chart_width = price_area.width.saturating_sub(label_width);

    // Slice candles to fit the chart area (not the full inner width)
    let candles_in_chart = ((chart_width + gap) / (candle_width + gap)) as usize;
    let visible_candles = &candles[candles.len().saturating_sub(candles_in_chart)..];

    if visible_candles.is_empty() {
        return;
    }

    // Price range — computed from visible candles only
    let price_high = visible_candles
        .iter()
        .map(|c| c.high)
        .max()
        .unwrap_or(Decimal::ONE);
    let price_low = visible_candles
        .iter()
        .map(|c| c.low)
        .min()
        .unwrap_or(Decimal::ZERO);
    let price_range = price_high - price_low;
    if price_range <= Decimal::ZERO {
        return;
    }

    // Volume range
    let vol_max = visible_candles
        .iter()
        .map(|c| c.volume)
        .max()
        .unwrap_or(1)
        .max(1);

    let buf = f.buffer_mut();

    // Draw Y-axis labels
    let label_count = (price_height / 3).max(2);
    for i in 0..label_count {
        let y = price_area.y + (i * price_height / label_count);
        let price_at_y = price_high - (price_range * Decimal::from(i)) / Decimal::from(label_count);
        let label = format!("{:.1}", price_at_y);
        let label_chars: Vec<char> = label.chars().collect();

        for (j, &ch) in label_chars.iter().enumerate().take(label_width as usize - 1) {
            let x = price_area.x + j as u16;
            if x < chart_x_start && y < price_area.y + price_height
                && let Some(cell) = buf.cell_mut(ratatui::layout::Position::new(x, y)) {
                    cell.set_char(ch);
                    cell.set_style(ColorSkin::muted());
                }
        }
    }

    // Draw candles
    for (i, candle) in visible_candles.iter().enumerate() {
        let x = chart_x_start + (i as u16) * (candle_width + gap);
        if x >= price_area.x + price_area.width {
            break;
        }

        let is_bullish = candle.close >= candle.open;
        let style = if is_bullish {
            ColorSkin::profit()
        } else {
            ColorSkin::loss()
        };

        let body_top = if is_bullish { candle.close } else { candle.open };
        let body_bot = if is_bullish { candle.open } else { candle.close };

        // Map prices to Y coordinates (inverted: top = high price)
        let y_high = price_to_y(candle.high, price_high, price_range, price_area);
        let y_low = price_to_y(candle.low, price_high, price_range, price_area);
        let y_body_top = price_to_y(body_top, price_high, price_range, price_area);
        let y_body_bot = price_to_y(body_bot, price_high, price_range, price_area);

        // Draw wick (thin line)
        for y in y_high..y_body_top {
            if y < price_area.y + price_height
                && let Some(cell) = buf.cell_mut(ratatui::layout::Position::new(x, y)) {
                    cell.set_char(VERT_LINE);
                    cell.set_style(style);
                }
        }

        // Draw body (full block)
        for y in y_body_top..=y_body_bot {
            if y < price_area.y + price_height
                && let Some(cell) = buf.cell_mut(ratatui::layout::Position::new(x, y)) {
                    cell.set_char(FULL_BLOCK);
                    cell.set_style(style);
                }
        }

        // Draw lower wick
        for y in (y_body_bot + 1)..=y_low {
            if y < price_area.y + price_height
                && let Some(cell) = buf.cell_mut(ratatui::layout::Position::new(x, y)) {
                    cell.set_char(VERT_LINE);
                    cell.set_style(style);
                }
        }

        // Draw volume bar
        if vol_area.height > 0 {
            let vol_ratio = candle.volume as f64 / vol_max as f64;
            let bar_height = (vol_ratio * vol_area.height as f64).ceil().min(vol_area.height as f64) as u16;
            let vol_style = if is_bullish {
                ColorSkin::profit()
            } else {
                ColorSkin::loss()
            };

            for dy in 0..bar_height {
                let y = vol_area.y + vol_area.height - 1 - dy;
                if y >= vol_area.y && x < vol_area.x + vol_area.width
                    && let Some(cell) = buf.cell_mut(ratatui::layout::Position::new(x, y)) {
                        cell.set_char(LOWER_HALF);
                        cell.set_style(vol_style);
                    }
            }
        }
    }
}

/// Map a price to a Y coordinate within the price area.
fn price_to_y(price: Decimal, high: Decimal, range: Decimal, area: Rect) -> u16 {
    if range <= Decimal::ZERO {
        return area.y;
    }
    let ratio = (high - price) / range;
    let ratio_f64 = ratio.to_string().parse::<f64>().unwrap_or(0.0);
    let y = area.y as f64 + ratio_f64 * (area.height.saturating_sub(1)) as f64;
    (y.round() as u16).clamp(area.y, area.y + area.height.saturating_sub(1))
}
