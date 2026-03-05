//! Color skin — accessibility-first palette with NO red/green.
//!
//! P1 (Cyan) = profit / positive / OK status.
//! P2 (Yellow) = loss / warning.
//! Accent (Magenta) = active selection / highlights.
//! Danger (LightRed) = critical alerts only (emergency shutdown, kill switch halt).
//! Never use pure `Color::Red` or `Color::Green` anywhere in the TUI.

use ratatui::style::{Color, Modifier, Style};

/// Static color skin. All methods are `const` so they can be used in
/// const contexts and carry zero runtime cost.
pub struct ColorSkin;

impl ColorSkin {
    pub const P1: Color = Color::Cyan;
    pub const P2: Color = Color::Yellow;
    pub const ACCENT: Color = Color::Magenta;
    pub const DANGER: Color = Color::LightRed;
    pub const MUTED: Color = Color::DarkGray;
    pub const TEXT: Color = Color::White;
    pub const BG: Color = Color::Reset;

    /// Bold cyan — section headers, titles.
    pub const fn header() -> Style {
        Style::new().fg(Self::P1).add_modifier(Modifier::BOLD)
    }

    /// Dark gray — field labels, secondary text.
    pub const fn label() -> Style {
        Style::new().fg(Self::MUTED)
    }

    /// White — primary data values.
    pub const fn value() -> Style {
        Style::new().fg(Self::TEXT)
    }

    /// Cyan — profit / positive P&L.
    pub const fn profit() -> Style {
        Style::new().fg(Self::P1)
    }

    /// Yellow — loss / negative P&L.
    pub const fn loss() -> Style {
        Style::new().fg(Self::P2)
    }

    /// Bold light-red — emergency shutdown, kill switch halt.
    pub const fn critical() -> Style {
        Style::new().fg(Self::DANGER).add_modifier(Modifier::BOLD)
    }

    /// Magenta — active tab, selected items.
    pub const fn active() -> Style {
        Style::new().fg(Self::ACCENT)
    }

    /// Cyan — connected, healthy.
    pub const fn status_ok() -> Style {
        Style::new().fg(Self::P1)
    }

    /// Yellow — warning, degraded.
    pub const fn status_warn() -> Style {
        Style::new().fg(Self::P2)
    }

    /// Bold yellow — restrict (auto-halting entries, more severe than warn).
    pub const fn status_restrict() -> Style {
        Style::new().fg(Self::P2).add_modifier(Modifier::BOLD)
    }

    /// Bold light-red — error, halted.
    pub const fn status_error() -> Style {
        Style::new().fg(Self::DANGER).add_modifier(Modifier::BOLD)
    }

    /// Dark gray — muted / inactive tab.
    pub const fn muted() -> Style {
        Style::new().fg(Self::MUTED)
    }

    /// Reversed magenta — selected row in tables.
    pub const fn selected() -> Style {
        Style::new()
            .fg(Self::TEXT)
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    }
}
