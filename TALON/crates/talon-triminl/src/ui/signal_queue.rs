//! Signal approval queue — Dual-Control banner for pending intents.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::App;
use crate::skin::ColorSkin;

pub fn draw(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    if state.pending_intents.is_empty() {
        return;
    }

    let intent = &state.pending_intents[0];
    let age = intent.age_secs();
    let remaining = (intent.timeout_secs as f64 - age).max(0.0);

    let is_fresh = app
        .target_lock_flash
        .map(|t| t.elapsed().as_secs_f64() < 1.0)
        .unwrap_or(false);

    let border_style = if is_fresh {
        ColorSkin::critical()
    } else {
        ColorSkin::active()
    };

    let side_style = if intent.side == talon_types::order::Side::Long {
        ColorSkin::profit()
    } else {
        ColorSkin::loss()
    };

    let price_str = intent
        .limit_price
        .map(|p| format!("${:.2}", p))
        .unwrap_or_else(|| "MKT".to_string());

    let countdown_style = if remaining < 3.0 {
        ColorSkin::loss()
    } else {
        ColorSkin::value()
    };

    let lock_line = Line::from(vec![
        Span::styled(format!("  {} ", intent.symbol), ColorSkin::value()),
        Span::styled(format!("{} ", intent.side), side_style),
        Span::styled(format!("{}  ", intent.quantity), ColorSkin::value()),
        Span::styled(&intent.strategy_name, ColorSkin::muted()),
        Span::styled(format!("  {price_str}"), ColorSkin::value()),
        Span::styled(format!("  [{:.0}s]", remaining), countdown_style),
    ]);

    let keys_line = Line::from(vec![
        Span::styled("  [a]", ColorSkin::active()),
        Span::styled(" ENGAGE  ", ColorSkin::value()),
        Span::styled("[r]", ColorSkin::active()),
        Span::styled(" PASS  ", ColorSkin::value()),
        Span::styled("[A]", ColorSkin::active()),
        Span::styled(" ALL  ", ColorSkin::value()),
        Span::styled("[R]", ColorSkin::active()),
        Span::styled(" PASS ALL", ColorSkin::value()),
    ]);

    let extra = if state.pending_intents.len() > 1 {
        format!(" +{} more", state.pending_intents.len() - 1)
    } else {
        String::new()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            format!(" TARGET LOCK{extra} "),
            if is_fresh {
                ratatui::style::Style::default()
                    .fg(ratatui::style::Color::White)
                    .add_modifier(ratatui::style::Modifier::BOLD)
            } else {
                ColorSkin::active()
            },
        ));

    let content = Paragraph::new(vec![lock_line, keys_line]).block(block);
    f.render_widget(content, area);
}
