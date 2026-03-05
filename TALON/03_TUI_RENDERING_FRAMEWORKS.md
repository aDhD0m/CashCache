# 03 -- Terminal User Interfaces (TUI) and Rendering Frameworks

Audited: 2026-03-03 | Source: Exhaustive Resource Registry, Section 3

---

## Audit Summary

| Resource | Status | TALON Relevance |
|----------|--------|-----------------|
| ratatui-org/ratatui | VALID -- core dependency | CRITICAL -- TALON's TUI framework |
| fdehau/tui-rs | VALID but DEPRECATED | None -- superseded by Ratatui |
| longbridge/longbridge-terminal | VALID -- production trading TUI | HIGH -- architecture reference |
| r3bl-org/r3bl-open-core tui | VALID -- async TUI alternative | Low -- different paradigm |
| nazmulidris/rust-scratch | VALID -- terminal_async examples | Low -- TCP server focus |

---

## Resource Details

### 1. Ratatui -- TALON's TUI Framework

- **URL:** https://github.com/ratatui-org/ratatui
- **Files:** .rs, .toml, .md
- **License:** MIT
- **Status:** Actively maintained, community fork of tui-rs

Ratatui is TALON's rendering layer. All TUI widgets, layouts, and terminal
interactions go through this crate. Key concepts:

**Immediate Mode Rendering:**
Ratatui uses immediate mode -- you redraw the entire frame every tick.
There is no retained widget tree. This means:
- State lives in your application, not in the framework
- Every render call receives a `Frame` and draws into it
- The framework diffs the output and only sends changed cells to the terminal

**Core Layout System:**

```rust
use ratatui::layout::{Constraint, Direction, Layout};

let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),      // Fixed 3 rows (header)
        Constraint::Min(10),        // At least 10 rows (main content)
        Constraint::Length(1),      // Fixed 1 row (status bar)
    ])
    .split(frame.area());
```

**Constraint Types:**
- `Length(n)` -- exact n cells
- `Min(n)` -- at least n cells
- `Max(n)` -- at most n cells
- `Percentage(n)` -- n% of parent
- `Ratio(num, den)` -- fractional
- `Fill(weight)` -- fill remaining space (weighted)

**Widget Trait:**

```rust
pub trait Widget {
    fn render(self, area: Rect, buf: &mut Buffer);
}

pub trait StatefulWidget {
    type State;
    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State);
}
```

**Key Widgets for Trading TUIs:**

| Widget | Use Case |
|--------|----------|
| Table | Position lists, order books |
| BarChart | Volume bars, P&L bars |
| Chart (with Dataset) | Price charts, equity curves |
| Sparkline | Compact inline price movement |
| Gauge | Margin utilization, fill progress |
| Paragraph | Log output, status messages |
| Tabs | Module switching (Firebird/Thunderbird/SAGE) |
| Block | Container with borders and titles |

**Chart Widget (price data):**

```rust
use ratatui::widgets::{Chart, Dataset, Axis, GraphType};
use ratatui::style::{Color, Style};
use ratatui::symbols::Marker;

let data: Vec<(f64, f64)> = prices.iter()
    .enumerate()
    .map(|(i, p)| (i as f64, *p))
    .collect();

let dataset = Dataset::default()
    .name("AAPL")
    .marker(Marker::Braille)
    .graph_type(GraphType::Line)
    .style(Style::default().fg(Color::Cyan))
    .data(&data);

let chart = Chart::new(vec![dataset])
    .x_axis(Axis::default()
        .title("Time")
        .bounds([0.0, data.len() as f64]))
    .y_axis(Axis::default()
        .title("Price")
        .bounds([min_price, max_price]));

frame.render_widget(chart, area);
```

**Table Widget (positions/orders):**

```rust
use ratatui::widgets::{Table, Row, Cell};

let rows = positions.iter().map(|p| {
    Row::new(vec![
        Cell::from(p.symbol.clone()),
        Cell::from(format!("{}", p.quantity)),
        Cell::from(format!("${:.2}", p.pnl))
            .style(if p.pnl >= 0.0 {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::Red)
            }),
    ])
});

let table = Table::new(rows, [
    Constraint::Length(8),   // Symbol
    Constraint::Length(6),   // Qty
    Constraint::Length(12),  // P&L
])
.header(Row::new(["Symbol", "Qty", "P&L"])
    .style(Style::default().bold()))
.block(Block::bordered().title("Positions"));

frame.render_widget(table, area);
```

**Event Loop Pattern (crossterm backend):**

```rust
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use std::time::Duration;

loop {
    // Draw
    terminal.draw(|frame| ui(frame, &app_state))?;

    // Poll events with timeout (controls refresh rate)
    if event::poll(Duration::from_millis(100))? {
        if let Event::Key(key) = event::read()? {
            match (key.modifiers, key.code) {
                (KeyModifiers::CONTROL, KeyCode::Char('c')) => break,
                (_, KeyCode::Char('q')) => break,
                (_, KeyCode::Tab) => app_state.next_module(),
                (_, KeyCode::Char('e')) => app_state.eject(), // EJECT
                _ => {}
            }
        }
    }
}
```

**TALON-Specific Patterns:**

The poll timeout (100ms = 10fps) controls how often the screen refreshes.
For trading TUIs showing live quotes, 100-250ms is typical. Going below
50ms wastes CPU without visible benefit on a terminal.

For TALON's module tabs (Firebird, Thunderbird, SAGE, etc.), use Ratatui's
`Tabs` widget with a state index tracking the active module.

---

### 2. longbridge-terminal -- Production Trading TUI Reference

- **URL:** https://github.com/longbridge/longbridge-terminal
- **Files:** .rs, .yml, .toml
- **Architecture:** Bevy ECS + DashMap + Longport OpenAPI

**Why This Matters for TALON:**

This is one of the few open-source production trading terminals in Rust.
Its architecture choices are instructive:

**Bevy ECS for UI State:**
Using Bevy's Entity Component System for a TUI is unconventional but solves
the "how do I share state across widgets" problem. Each quote, order, and
position is an entity. Components store the data. Systems query and update
components. This avoids the `Arc<Mutex<>>` hell common in trading UIs.

**DashMap for Quote Cache:**
DashMap provides concurrent read/write access without a global lock.
Each stock symbol maps to its latest quote data. Writers (WebSocket
feed handler) and readers (TUI renderer) never block each other.

```rust
// Conceptual pattern from longbridge-terminal
use dashmap::DashMap;

struct QuoteCache {
    quotes: DashMap<String, QuoteData>,
}

impl QuoteCache {
    fn update(&self, symbol: &str, quote: QuoteData) {
        self.quotes.insert(symbol.to_string(), quote);
    }

    fn get(&self, symbol: &str) -> Option<QuoteData> {
        self.quotes.get(symbol).map(|r| r.value().clone())
    }
}
```

**clippy::pedantic Enforcement:**
The project enforces clippy::pedantic across the entire codebase. Key
lints this enables:

- `must_use_candidate` -- forces #[must_use] on functions returning values
- `missing_errors_doc` -- requires error documentation
- `needless_pass_by_value` -- catches unnecessary clones
- `cast_possible_truncation` -- catches unsafe numeric casts

**Localization (rust-i18n):**
Locale files in locales/*.yml. Not directly relevant to TALON unless
internationalization becomes a requirement.

**TALON Application:**

The DashMap pattern maps directly to TALON's need for concurrent quote
access. The TUI renderer reads quotes at 10fps while the WebSocket
handler writes at market tick rate. DashMap eliminates the bottleneck.

Bevy ECS is likely overkill for TALON's simpler module architecture,
but the concept of separating data (components) from behavior (systems)
is worth adopting even with plain Rust structs and traits.

---

### 3. tui-rs (DEPRECATED)

- **URL:** https://github.com/fdehau/tui-rs
- **Status:** Archived. Ratatui is the maintained community fork.

**Do not use.** Listed in the registry for historical context only.
All tui-rs APIs are available in Ratatui with the same signatures.
Migration is a crate name swap in Cargo.toml.

---

### 4. r3bl_tui -- Async TUI Alternative

- **URL:** https://github.com/r3bl-org/r3bl-open-core/tree/main/tui
- **Files:** .rs, .toml

Provides an async terminal rendering paradigm. Different model from
Ratatui's immediate mode -- r3bl_tui uses a retained component tree
with async event handling.

**TALON Application:** Low relevance. TALON is committed to Ratatui.
However, if the Ratatui immediate-mode pattern becomes a performance
bottleneck (unlikely for a trading TUI), r3bl_tui's async model is
the fallback to evaluate.

---

### 5. nazmulidris/rust-scratch

- **URL:** https://github.com/nazmulidris/rust-scratch/
- **Files:** .rs

Examples of using terminal_async for complex TCP API server interfaces.

**TALON Application:** Low direct relevance. The TCP server interface
patterns might inform TALON's TWS TCP connection management, but ibapi
already abstracts this.
