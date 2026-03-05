//! Startup splash screen and background watermark.
//!
//! Embeds the TALON ASCII logo at compile time and provides:
//! - `draw_splash()` — render logo for the flash animation sequence.
//! - `draw_watermark()` — very dim logo behind the live interface.

use ratatui::layout::{Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::Frame;

/// The TALON logo, embedded at compile time.
const LOGO: &str = include_str!("../../../TALON-ASCII.txt");

/// Watermark style — very dim, blends into dark terminal backgrounds.
const WATERMARK_STYLE: Style = Style::new().fg(Color::Rgb(40, 40, 55));

/// Splash animation frames: (style, hold_ms).
/// `None` style = blank frame (blink off).
pub const SPLASH_FRAMES: &[(Option<Style>, u64)] = &[
    (
        Some(Style::new().fg(Color::White).add_modifier(Modifier::BOLD)),
        250,
    ),
    (None, 60),
    (
        Some(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        250,
    ),
    (Some(Style::new().fg(Color::Cyan)), 700),
    (Some(Style::new().fg(Color::DarkGray)), 300),
];

/// Render the TALON logo centered in the given area.
/// Only writes non-space characters so existing buffer content shows through.
fn render_logo(f: &mut Frame, area: Rect, style: Style) {
    let lines: Vec<&str> = LOGO.lines().collect();
    let logo_height = lines.len() as u16;
    let logo_width = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u16;

    let y_off = area.y + area.height.saturating_sub(logo_height) / 2;
    let x_off = area.x + area.width.saturating_sub(logo_width) / 2;

    let buf = f.buffer_mut();
    for (i, line) in lines.iter().enumerate() {
        let y = y_off + i as u16;
        if y >= area.y + area.height {
            break;
        }
        for (j, ch) in line.chars().enumerate() {
            if ch == ' ' {
                continue;
            }
            let x = x_off + j as u16;
            if x >= area.x + area.width {
                break;
            }
            if let Some(cell) = buf.cell_mut(Position::new(x, y)) {
                cell.set_char(ch);
                cell.set_style(style);
            }
        }
    }
}

/// Draw the logo centered on screen with the given style (for splash animation).
pub fn draw_splash(f: &mut Frame, style: Style) {
    let area = f.area();
    render_logo(f, area, style);
}

/// Draw the watermark — very dim eagle behind the live UI.
pub fn draw_watermark(f: &mut Frame, area: Rect) {
    render_logo(f, area, WATERMARK_STYLE);
}
