//! Tab body renderers — one function per tab view.

use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Row, Table};
use ratatui::Frame;
use rust_decimal::Decimal;

use talon_types::channel::ApproachDir;
use talon_types::flow::TradeSide;
use talon_types::module::ModuleState;

use crate::app::{App, Tab};
use crate::skin::ColorSkin;

/// Dispatch to the correct tab body renderer.
pub fn draw_tab_body(f: &mut Frame, area: Rect, app: &App) {
    match app.active_tab {
        Tab::Cockpit => draw_cockpit(f, area, app),
        Tab::Portfolio => draw_portfolio(f, area, app),
        Tab::Scanner => draw_scanner(f, area, app),
        Tab::Flow => draw_flow(f, area, app),
        Tab::Log => draw_log(f, area, app),
    }
}

// ---------------------------------------------------------------------------
// Tab 1: Cockpit — overview metrics + module states
// ---------------------------------------------------------------------------

fn draw_cockpit(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    // Top row: dashboard + modules. Bottom: pending intents + fills.
    let has_pending = !state.pending_intents.is_empty();
    let has_fills = !state.recent_fills.is_empty();
    let bottom_height = if has_pending || has_fills { 35 } else { 0 };

    let cockpit_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(100u16.saturating_sub(bottom_height)),
            Constraint::Percentage(bottom_height),
        ])
        .split(area);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cockpit_rows[0]);

    // Left: metrics table
    let pnl_label = format!("{:+.2}", state.daily_pnl);
    let pnl_style = if state.daily_pnl >= Decimal::ZERO {
        ColorSkin::profit()
    } else {
        ColorSkin::loss()
    };

    let halt_label = match (app.entry_halt, state.entry_halt_active) {
        (true, true) => "ON (USER + SYSTEM)",
        (true, false) => "ON (USER)",
        (false, true) => "ON (SYSTEM)",
        (false, false) => "OFF",
    };
    let halt_style = match (app.entry_halt, state.entry_halt_active) {
        (true, true) => ColorSkin::status_error(),
        (true, false) => ColorSkin::status_warn(),
        (false, true) => ColorSkin::status_restrict(),
        (false, false) => ColorSkin::status_ok(),
    };

    let rows = vec![
        metric_row("Stress Tier", &format!("{}", state.stress_tier), ColorSkin::value()),
        metric_row("Regime", &format!("{}", state.regime), ColorSkin::value()),
        metric_row("Daily P&L", &pnl_label, pnl_style),
        metric_row(
            "Open Positions",
            &state.positions.len().to_string(),
            ColorSkin::value(),
        ),
        metric_row("Entry Halt", halt_label, halt_style),
        metric_row(
            "Vault Total",
            &format!("${:.2}", state.vault_total),
            ColorSkin::profit(),
        ),
        metric_row(
            "Vault Pending",
            &format!("${:.2}", state.vault_pending_harvest),
            ColorSkin::muted(),
        ),
        metric_row(
            "Cruising Alt.",
            if state.cruising_altitude { "YES" } else { "---" },
            if state.cruising_altitude {
                ColorSkin::active()
            } else {
                ColorSkin::muted()
            },
        ),
        metric_row(
            "Approaching",
            &state.approaching_setups.len().to_string(),
            if state.approaching_setups.is_empty() {
                ColorSkin::muted()
            } else {
                ColorSkin::active()
            },
        ),
        metric_row(
            "Uptime",
            &format!("{:.0}s", state.uptime_secs),
            ColorSkin::muted(),
        ),
    ];

    let table = Table::new(rows, [Constraint::Length(18), Constraint::Fill(1)])
        .block(Block::default().borders(Borders::ALL).title(Span::styled(
            " Dashboard ",
            ColorSkin::header(),
        )));
    f.render_widget(table, cols[0]);

    // Right: module states with color-coded indicators
    if !state.module_states.is_empty() {
        let lines: Vec<Line> = state
            .module_states
            .iter()
            .map(|ms| {
                let state_style = match ms.state {
                    ModuleState::Scanning => ColorSkin::active(),
                    ModuleState::SignalGenerated => ColorSkin::profit(),
                    ModuleState::Active => ColorSkin::profit(),
                    ModuleState::Paused | ModuleState::Disabled => ColorSkin::loss(),
                    _ => ColorSkin::muted(),
                };

                Line::from(vec![
                    Span::styled(format!("  {:<12}", ms.module), ColorSkin::value()),
                    Span::styled(format!("{:<10?}", ms.state), state_style),
                    Span::styled(
                        format!(
                            " {}g/{}a/{}r",
                            ms.signals_generated, ms.signals_approved, ms.signals_rejected
                        ),
                        ColorSkin::muted(),
                    ),
                ])
            })
            .collect();

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Modules ", ColorSkin::header()));
        let paragraph = Paragraph::new(lines).block(block);
        f.render_widget(paragraph, cols[1]);
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Modules ", ColorSkin::header()));
        let text = Paragraph::new(Line::from(Span::styled(
            "  Waiting for data...",
            ColorSkin::muted(),
        )))
        .block(block);
        f.render_widget(text, cols[1]);
    }

    // --- Bottom: Pending Intents + Fills ---
    if has_pending || has_fills {
        let bottom_cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(cockpit_rows[1]);

        draw_pending_intents(f, bottom_cols[0], app);
        draw_fills(f, bottom_cols[1], app);
    }
}

// ---------------------------------------------------------------------------
// PendingIntents widget — intents awaiting operator approval
// ---------------------------------------------------------------------------

fn draw_pending_intents(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    let pi_rows: Vec<Row> = state
        .pending_intents
        .iter()
        .enumerate()
        .map(|(i, pi)| {
            let age = pi.age_secs();
            let age_str = format!("{:.0}s", age);
            let age_style = if age > 5.0 {
                ColorSkin::status_warn()
            } else {
                ColorSkin::muted()
            };

            let id_prefix = &pi.id.to_string()[..8.min(pi.id.to_string().len())];
            let price_str = pi
                .limit_price
                .map(|p| format!("${:.2}", p))
                .unwrap_or_else(|| "MKT".to_string());

            let row = Row::new(vec![
                Span::styled(id_prefix.to_string(), ColorSkin::muted()),
                Span::styled(format!("{:<6}", pi.symbol), ColorSkin::value()),
                Span::styled(format!("{}", pi.side), if pi.side == talon_types::order::Side::Long {
                    ColorSkin::profit()
                } else {
                    ColorSkin::loss()
                }),
                Span::styled(format!("{:>5}", pi.quantity), ColorSkin::value()),
                Span::styled(format!("{:>10}", price_str), ColorSkin::value()),
                Span::styled(format!("{:<10}", pi.strategy_name), ColorSkin::muted()),
                Span::styled(age_str, age_style),
            ]);

            if i == app.pending_cursor {
                row.style(ColorSkin::selected())
            } else {
                row
            }
        })
        .collect();

    let pending_count = state.pending_intents.len();
    let title = format!(" Pending ({pending_count}) [a]pprove [r]eject [A]ll [R]ej all ");

    let table = Table::new(
        pi_rows,
        [
            Constraint::Length(8),  // ID
            Constraint::Length(6),  // SYMBOL
            Constraint::Length(4),  // SIDE
            Constraint::Length(6),  // QTY
            Constraint::Length(10), // PRICE
            Constraint::Length(10), // STRATEGY
            Constraint::Min(5),    // AGE
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, ColorSkin::active())),
    )
    .header(
        Row::new(vec!["ID", "SYMBOL", "SIDE", "QTY", "PRICE", "STRATEGY", "AGE"])
            .style(ColorSkin::label()),
    );

    f.render_widget(table, area);
}

// ---------------------------------------------------------------------------
// Fills widget — recent fills with P&L color-coding
// ---------------------------------------------------------------------------

fn draw_fills(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;
    let max_rows = area.height.saturating_sub(3) as usize;

    let fill_rows: Vec<Row> = state
        .recent_fills
        .iter()
        .rev()
        .take(max_rows)
        .map(|fill| {
            let pnl_str = fill
                .realized_pnl
                .map(|p| format!("${:+.2}", p))
                .unwrap_or_else(|| "---".to_string());

            let pnl_style = match fill.realized_pnl {
                Some(p) if p >= Decimal::ZERO => ColorSkin::profit(),
                Some(_) => ColorSkin::loss(),
                None => ColorSkin::muted(),
            };

            let ts = fill.timestamp.format("%H:%M:%S");

            Row::new(vec![
                Span::styled(format!(" {ts} "), ColorSkin::muted()),
                Span::styled(format!("{:<6}", fill.symbol), ColorSkin::value()),
                Span::styled(format!("{}", fill.side), if fill.side == talon_types::order::Side::Long {
                    ColorSkin::profit()
                } else {
                    ColorSkin::loss()
                }),
                Span::styled(format!("{:>5}", fill.qty), ColorSkin::value()),
                Span::styled(format!("${:.2}", fill.fill_price), ColorSkin::value()),
                Span::styled(pnl_str, pnl_style),
            ])
        })
        .collect();

    let fill_count = state.recent_fills.len();
    let title = format!(" Fills ({fill_count}) ");

    let table = Table::new(
        fill_rows,
        [
            Constraint::Length(10), // TIME
            Constraint::Length(6),  // SYMBOL
            Constraint::Length(4),  // SIDE
            Constraint::Length(6),  // QTY
            Constraint::Length(10), // PRICE
            Constraint::Min(10),   // P&L
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, ColorSkin::header())),
    )
    .header(
        Row::new(vec!["TIME", "SYMBOL", "SIDE", "QTY", "PRICE", "P&L"])
            .style(ColorSkin::label()),
    );

    f.render_widget(table, area);
}

fn metric_row(label: &str, value: &str, style: ratatui::style::Style) -> Row<'static> {
    Row::new(vec![
        Span::styled(format!("  {label}"), ColorSkin::label()),
        Span::styled(value.to_string(), style),
    ])
}

// ---------------------------------------------------------------------------
// Tab 2: Portfolio (Active) — open positions with P&L
// ---------------------------------------------------------------------------

fn draw_portfolio(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    let rows: Vec<Row> = state
        .positions
        .iter()
        .map(|pos| {
            let pnl = pos.unrealized_pnl();
            let pnl_pct = pos.unrealized_pnl_pct();
            let pnl_color = if pnl >= Decimal::ZERO {
                ColorSkin::profit()
            } else {
                ColorSkin::loss()
            };

            Row::new(vec![
                format!("{:<6}", pos.symbol),
                format!("{:>+5}", pos.qty),
                format!("${:.2}", pos.avg_entry),
                format!("${:.2}", pos.current_price),
                format!("{:>+.1}%", pnl_pct),
                format!("${:>+.0}", pnl),
                format!("{}", pos.module),
                pos.stop_loss
                    .map(|s| format!("${s}"))
                    .unwrap_or_default(),
                pos.take_profit
                    .map(|t| format!("${t}"))
                    .unwrap_or_default(),
            ])
            .style(pnl_color)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(8),  // SYMBOL
            Constraint::Length(6),  // QTY
            Constraint::Length(10), // ENTRY
            Constraint::Length(10), // CURRENT
            Constraint::Length(8),  // P&L%
            Constraint::Length(10), // P&L$
            Constraint::Length(12), // MODULE
            Constraint::Length(10), // STOP
            Constraint::Min(10),   // TARGET
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Portfolio (Active) ", ColorSkin::header())),
    )
    .header(
        Row::new(vec![
            "SYMBOL", "QTY", "ENTRY", "CURRENT", "P&L %", "P&L $", "MODULE", "STOP", "TARGET",
        ])
        .style(ColorSkin::label()),
    );

    f.render_widget(table, area);
}

// ---------------------------------------------------------------------------
// Tab 3: Scanner — unified watchlist with inline distance + target lock
// ---------------------------------------------------------------------------

fn draw_scanner(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    // Build a map of symbol → closest approaching setup for inline display.
    let mut approach_map: std::collections::HashMap<&talon_types::position::Symbol, &talon_types::channel::ApproachingSetup> =
        std::collections::HashMap::new();
    for setup in &state.approaching_setups {
        let existing = approach_map.get(&setup.symbol);
        if existing.is_none() || setup.distance_pct < existing.unwrap().distance_pct {
            approach_map.insert(&setup.symbol, setup);
        }
    }

    // Determine layout: target lock banner (if pending) + watchlist table
    let has_lock = !state.pending_intents.is_empty();
    let lock_height = if has_lock { 4u16 } else { 0 };

    let sections = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(lock_height),
            Constraint::Min(5),
        ])
        .split(area);

    // --- Target Lock Banner ---
    if has_lock {
        draw_target_lock(f, sections[0], app);
    }

    // --- Watchlist with inline distance ---
    let approaching_count = state.approaching_setups.len();

    // Filter out quotes with no real data
    let valid_quotes: Vec<_> = state
        .watchlist_quotes
        .iter()
        .filter(|q| q.last > Decimal::ZERO)
        .collect();

    let quote_rows: Vec<Row> = valid_quotes
        .iter()
        .map(|q| {
            let change = q
                .change_pct()
                .filter(|c| c.abs() < Decimal::from(999))
                .map(|c| format!("{:+.1}%", c))
                .unwrap_or_default();

            let change_style = q
                .change_pct()
                .map(|c| {
                    if c >= Decimal::ZERO {
                        ColorSkin::profit()
                    } else {
                        ColorSkin::loss()
                    }
                })
                .unwrap_or(ColorSkin::muted());

            let rvol = q
                .rvol()
                .map(|r| format!("{:.1}x", r))
                .unwrap_or_default();

            let bid_str = if q.bid > Decimal::ZERO {
                format!("{:.2}", q.bid)
            } else {
                String::new()
            };

            let ask_str = if q.ask > Decimal::ZERO {
                format!("{:.2}", q.ask)
            } else {
                String::new()
            };

            // Inline approaching distance
            let (dist_str, dist_style) = match approach_map.get(&q.symbol) {
                Some(setup) => {
                    let arrow = match setup.direction {
                        ApproachDir::Heating => "\u{25b2}", // ▲
                        ApproachDir::Cooling => "\u{25bc}", // ▼
                    };
                    let style = match setup.direction {
                        ApproachDir::Heating => ColorSkin::profit(),
                        ApproachDir::Cooling => ColorSkin::muted(),
                    };
                    (format!("{}{:.0}%", arrow, setup.distance_pct), style)
                }
                None => (String::new(), ColorSkin::muted()),
            };

            Row::new(vec![
                Span::styled(format!("{:<5}", q.symbol), ColorSkin::value()),
                Span::styled(format!("{:>8.2}", q.last), ColorSkin::value()),
                Span::styled(format!("{:>7}", change), change_style),
                Span::styled(format!("{:>8}", bid_str), ColorSkin::muted()),
                Span::styled(format!("{:>8}", ask_str), ColorSkin::muted()),
                Span::styled(format!("{:>5}", rvol), ColorSkin::value()),
                Span::styled(format!("{:>8}", q.volume), ColorSkin::muted()),
                Span::styled(format!("{:>6}", dist_str), dist_style),
            ])
        })
        .collect();

    let scan_status = if approaching_count > 0 {
        format!(" Scanner ({} approaching) ", approaching_count)
    } else {
        " Scanner ".to_string()
    };

    let quote_table = Table::new(
        quote_rows,
        [
            Constraint::Length(6),  // SYMBOL
            Constraint::Length(9),  // LAST
            Constraint::Length(8),  // CHG%
            Constraint::Length(9),  // BID
            Constraint::Length(9),  // ASK
            Constraint::Length(6),  // RVOL
            Constraint::Length(9),  // VOL
            Constraint::Min(7),    // DIST
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(scan_status, ColorSkin::header())),
    )
    .header(
        Row::new(vec!["SYM", "LAST", "CHG%", "BID", "ASK", "RVOL", "VOL", "DIST"])
            .style(ColorSkin::label()),
    );

    f.render_widget(quote_table, sections[1]);
}

// ---------------------------------------------------------------------------
// Target Lock Banner — shows qualified intent with ENGAGE? prompt
// ---------------------------------------------------------------------------

fn draw_target_lock(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    // Pick the first (most urgent) pending intent for the banner
    let intent = match state.pending_intents.first() {
        Some(p) => p,
        None => return,
    };

    let age = intent.age_secs();
    let remaining = (intent.timeout_secs as f64 - age).max(0.0);

    // Flash effect: first 1s = bright accent, then normal
    let is_fresh = app
        .target_lock_flash
        .map(|t| t.elapsed().as_secs_f64() < 1.0)
        .unwrap_or(false);

    let border_style = if is_fresh {
        ColorSkin::critical()
    } else {
        ColorSkin::active()
    };

    let title_style = if is_fresh {
        ratatui::style::Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(ratatui::style::Modifier::BOLD | ratatui::style::Modifier::SLOW_BLINK)
    } else {
        ColorSkin::active()
    };

    let side_str = format!("{}", intent.side);
    let price_str = intent
        .limit_price
        .map(|p| format!("${:.2}", p))
        .unwrap_or_else(|| "MKT".to_string());

    let countdown = format!("[{:.0}s]", remaining);
    let countdown_style = if remaining < 3.0 {
        ColorSkin::loss()
    } else {
        ColorSkin::value()
    };

    let lock_line = Line::from(vec![
        Span::styled("  ", ColorSkin::value()),
        Span::styled(format!("{}", intent.symbol), ColorSkin::value()),
        Span::styled("  ", ColorSkin::value()),
        Span::styled(side_str, if intent.side == talon_types::order::Side::Long {
            ColorSkin::profit()
        } else {
            ColorSkin::loss()
        }),
        Span::styled(format!(" {}  ", intent.quantity), ColorSkin::value()),
        Span::styled(&intent.strategy_name, ColorSkin::muted()),
        Span::styled("  ", ColorSkin::value()),
        Span::styled(price_str, ColorSkin::value()),
        Span::styled("  ", ColorSkin::value()),
        Span::styled(countdown, countdown_style),
    ]);

    let keys_line = Line::from(vec![
        Span::styled("  [a]", ColorSkin::active()),
        Span::styled(" ENGAGE   ", ColorSkin::value()),
        Span::styled("[r]", ColorSkin::active()),
        Span::styled(" PASS   ", ColorSkin::value()),
        Span::styled("[A]", ColorSkin::active()),
        Span::styled(" ALL   ", ColorSkin::value()),
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
            title_style,
        ));

    let content = Paragraph::new(vec![lock_line, keys_line]).block(block);
    f.render_widget(content, area);
}

// ---------------------------------------------------------------------------
// Tab 4: Flow — L2 depth, T&S tape, volume profile, delta sparkline
// ---------------------------------------------------------------------------

fn draw_flow(f: &mut Frame, area: Rect, app: &App) {
    let flow = &app.state.flow;

    // No symbol selected — show prompt.
    if flow.symbol.is_none() {
        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled("  Flow", ColorSkin::header())),
            Line::from(""),
        ];

        if app.flow.input_active {
            lines.push(Line::from(vec![
                Span::styled("  Symbol: ", ColorSkin::label()),
                Span::styled(&app.flow.symbol_input, ColorSkin::active()),
                Span::styled("_", ColorSkin::active()),
            ]));
        } else {
            lines.push(Line::from(Span::styled(
                "  Press [/] to enter symbol or [[] []] to browse watchlist",
                ColorSkin::muted(),
            )));
        }

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Flow ", ColorSkin::header()));
        f.render_widget(Paragraph::new(lines).block(block), area);
        return;
    }

    let sym_label = flow
        .symbol
        .as_ref()
        .map(|s| s.to_string())
        .unwrap_or_default();

    // Main layout: left 30% (tape) | right 70% (DOM + profile + delta)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(area);

    // --- Left: T&S Tape ---
    draw_flow_tape(f, cols[0], app);

    // --- Right: 3-row split ---
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // DOM Ladder
            Constraint::Percentage(30), // Volume Profile
            Constraint::Percentage(20), // Delta Sparkline
        ])
        .split(cols[1]);

    draw_flow_dom(f, right_rows[0], app, &sym_label);
    draw_flow_volume_profile(f, right_rows[1], app);
    draw_flow_delta(f, right_rows[2], app);
}

fn draw_flow_tape(f: &mut Frame, area: Rect, app: &App) {
    let flow = &app.state.flow;
    let max_lines = area.height.saturating_sub(2) as usize;

    let lines: Vec<Line> = flow
        .tape
        .iter()
        .take(max_lines)
        .map(|entry| {
            let ts = entry.time.format("%H:%M:%S");
            let side_style = match entry.side {
                TradeSide::Buy => ColorSkin::profit(),
                TradeSide::Sell => ColorSkin::loss(),
                TradeSide::Unknown => ColorSkin::muted(),
            };

            Line::from(vec![
                Span::styled(format!(" {ts} "), ColorSkin::muted()),
                Span::styled(format!("{:.2}", entry.price), side_style),
                Span::raw(" "),
                Span::styled(format!("{:.0}", entry.size), side_style),
                Span::styled(format!(" {}", entry.exchange), ColorSkin::muted()),
            ])
        })
        .collect();

    let title = if app.flow.input_active {
        format!(" T&S: {}_ ", app.flow.symbol_input)
    } else if app.flow.tape_following {
        " T&S ".to_string()
    } else {
        " T&S [PAUSED] ".to_string()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(title, ColorSkin::header()));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_flow_dom(f: &mut Frame, area: Rect, app: &App, sym_label: &str) {
    let flow = &app.state.flow;

    // Build interleaved ask (reversed) + spread row + bid rows.
    let max_levels = ((area.height.saturating_sub(3)) / 2) as usize;

    let mut rows: Vec<Row> = Vec::new();

    // Asks — reversed so best ask is at the bottom of the ask section.
    let ask_count = flow.book.asks.len().min(max_levels);
    for level in flow.book.asks.iter().take(ask_count).rev() {
        rows.push(Row::new(vec![
            String::new(),
            String::new(),
            String::new(),
            format!("{:.2}", level.price),
            format!("{:.0}", level.size),
        ]).style(ColorSkin::loss()));
    }

    // Spread row.
    let spread_text = flow
        .book
        .spread()
        .map(|s| format!("{:.2}", s))
        .unwrap_or_else(|| "\u{2014}".to_string());
    rows.push(
        Row::new(vec![
            String::new(),
            String::new(),
            format!(" {spread_text} "),
            String::new(),
            String::new(),
        ])
        .style(ColorSkin::active()),
    );

    // Bids — best bid at the top.
    let bid_count = flow.book.bids.len().min(max_levels);
    for level in flow.book.bids.iter().take(bid_count) {
        rows.push(Row::new(vec![
            format!("{:.0}", level.size),
            format!("{:.2}", level.price),
            String::new(),
            String::new(),
            String::new(),
        ]).style(ColorSkin::profit()));
    }

    let live_label = if flow.is_live { "LIVE" } else { "---" };
    let title = format!(" DOM {sym_label} [{live_label}] ");

    let table = Table::new(
        rows,
        [
            Constraint::Length(10), // BID_SZ
            Constraint::Length(10), // BID
            Constraint::Length(8),  // SPREAD
            Constraint::Length(10), // ASK
            Constraint::Min(10),   // ASK_SZ
        ],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(title, ColorSkin::header())),
    )
    .header(
        Row::new(vec!["BID SZ", "BID", "SPREAD", "ASK", "ASK SZ"])
            .style(ColorSkin::label()),
    );

    f.render_widget(table, area);
}

fn draw_flow_volume_profile(f: &mut Frame, area: Rect, app: &App) {
    let flow = &app.state.flow;
    let inner_width = area.width.saturating_sub(2) as usize;
    let max_rows = area.height.saturating_sub(2) as usize;

    if flow.volume_profile.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Volume Profile ", ColorSkin::header()));
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Waiting for trades...",
                ColorSkin::muted(),
            )))
            .block(block),
            area,
        );
        return;
    }

    // Find max total volume for scaling.
    let max_vol = flow
        .volume_profile
        .iter()
        .map(|b| b.total())
        .max()
        .unwrap_or(Decimal::ONE);

    // Find POC (point of control = price with highest volume).
    let poc_price = flow
        .volume_profile
        .iter()
        .max_by_key(|b| b.total())
        .map(|b| b.price);

    // Label width: "$NNN.NN " = 9 chars
    let label_width = 9usize;
    let bar_width = inner_width.saturating_sub(label_width + 1);

    // Center around the midpoint of the profile, take max_rows.
    let profile_len = flow.volume_profile.len();
    let skip = if profile_len > max_rows {
        (profile_len - max_rows) / 2
    } else {
        0
    };

    let lines: Vec<Line> = flow
        .volume_profile
        .iter()
        .skip(skip)
        .take(max_rows)
        .map(|bucket| {
            let ratio = if max_vol > Decimal::ZERO {
                (bucket.total() / max_vol)
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let filled = (ratio * bar_width as f64).round() as usize;

            let buy_ratio = if bucket.total() > Decimal::ZERO {
                (bucket.buy_volume / bucket.total())
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.5)
            } else {
                0.5
            };
            let buy_chars = (buy_ratio * filled as f64).round() as usize;
            let sell_chars = filled.saturating_sub(buy_chars);

            let is_poc = poc_price == Some(bucket.price);
            let price_style = if is_poc {
                ColorSkin::active()
            } else {
                ColorSkin::muted()
            };

            Line::from(vec![
                Span::styled(format!(" ${:<7.2}", bucket.price), price_style),
                Span::styled(
                    "\u{2588}".repeat(buy_chars),
                    ColorSkin::profit(),
                ),
                Span::styled(
                    "\u{2588}".repeat(sell_chars),
                    ColorSkin::loss(),
                ),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Volume Profile ", ColorSkin::header()));
    f.render_widget(Paragraph::new(lines).block(block), area);
}

fn draw_flow_delta(f: &mut Frame, area: Rect, app: &App) {
    let flow = &app.state.flow;
    let inner_width = area.width.saturating_sub(2) as usize;

    let delta_label = format!("DELTA {:+.0}", flow.cumulative_delta);
    let delta_style = if flow.cumulative_delta >= Decimal::ZERO {
        ColorSkin::profit()
    } else {
        ColorSkin::loss()
    };

    if flow.delta_sparkline.is_empty() {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(" Delta ", ColorSkin::header()));
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {delta_label}  Waiting..."),
                delta_style,
            )))
            .block(block),
            area,
        );
        return;
    }

    // Build sparkline from delta buckets.
    let spark_chars = [' ', '\u{2581}', '\u{2582}', '\u{2583}', '\u{2584}', '\u{2585}', '\u{2586}', '\u{2587}', '\u{2588}'];

    let max_abs = flow
        .delta_sparkline
        .iter()
        .map(|b| b.delta.abs())
        .max()
        .unwrap_or(Decimal::ONE);

    let label_len = delta_label.len() + 4; // "  DELTA +1234  "
    let spark_width = inner_width.saturating_sub(label_len);

    let spark_spans: Vec<Span> = flow
        .delta_sparkline
        .iter()
        .rev() // oldest first visually
        .take(spark_width)
        .map(|bucket| {
            let ratio = if max_abs > Decimal::ZERO {
                (bucket.delta.abs() / max_abs)
                    .to_string()
                    .parse::<f64>()
                    .unwrap_or(0.0)
            } else {
                0.0
            };
            let idx = (ratio * 8.0).round() as usize;
            let ch = spark_chars[idx.min(8)];
            let style = if bucket.delta >= Decimal::ZERO {
                ColorSkin::profit()
            } else {
                ColorSkin::loss()
            };
            Span::styled(ch.to_string(), style)
        })
        .collect();

    let mut spans = vec![
        Span::styled(format!("  {delta_label}  "), delta_style),
    ];
    spans.extend(spark_spans);

    let line = Line::from(spans);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Delta ", ColorSkin::header()));
    f.render_widget(
        Paragraph::new(vec![line])
            .block(block)
            .alignment(Alignment::Left),
        area,
    );
}

// ---------------------------------------------------------------------------
// Tab 5: Log — event log with color-coded entries
// ---------------------------------------------------------------------------

fn draw_log(f: &mut Frame, area: Rect, app: &App) {
    let state = &app.state;

    let max_lines = area.height.saturating_sub(2) as usize;
    let lines: Vec<Line> = state
        .recent_events
        .iter()
        .rev()
        .take(max_lines)
        .map(|ev| {
            let ts = ev.timestamp.format("%H:%M:%S");
            let (prefix, color) = match &ev.kind {
                talon_types::event::EventKind::OrderFilled { .. } => ("OK", ColorSkin::P1),
                talon_types::event::EventKind::OrderApproved { .. } => ("GO", ColorSkin::P1),
                talon_types::event::EventKind::OrderRejected { .. } => ("!!", ColorSkin::P2),
                talon_types::event::EventKind::HarvestExecuted { .. } => ("$$", ColorSkin::ACCENT),
                talon_types::event::EventKind::StressMultiplierChanged { .. } => {
                    ("**", ColorSkin::P2)
                }
                talon_types::event::EventKind::FlameoutEngaged { .. } => {
                    ("FL", ColorSkin::DANGER)
                }
                talon_types::event::EventKind::NosediveTriggered { .. } => {
                    ("ND", ColorSkin::DANGER)
                }
                _ => ("--", ColorSkin::MUTED),
            };

            Line::from(vec![
                Span::styled(format!(" {ts} "), ColorSkin::muted()),
                Span::styled(
                    format!("{prefix:>2}"),
                    ratatui::style::Style::default().fg(color),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("{:?}", std::mem::discriminant(&ev.kind)),
                    ColorSkin::value(),
                ),
            ])
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(" Log ", ColorSkin::header()));
    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}
