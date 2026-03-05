# TRiMiNL v3.2.0 -- Design Merge Plan

## Status of Longbridge Terminal

Longbridge Terminal v0.6.0 is a closed-source binary. The GitHub repo
(`longbridge/longbridge-terminal`) contains only an install script and a README that says
"This is not an open-source project." No Rust source available. Last release: July 2023.
12 commits, all non-code.

However, the Longbridge OpenAPI SDK (`longbridge/openapi-sdk`) IS open source and provides
the data primitives (quotes, depth, trades, candlesticks, portfolio). We are NOT using
Longbridge as a broker -- TALON targets IBKR and US markets. But the SDK's data model is
worth studying for struct design.

**Decision: Longbridge Terminal is the UX gold standard. We replicate its layout and
interaction patterns in a clean Ratatui build, wired to TALON's own data infrastructure.**


---


## Part 1: What to Steal from Longbridge Terminal

All observations from your 19 screenshots. These are the concrete patterns TRiMiNL must match.

### Layout: 3-Column + Header + Footer

```
+-----------------------------------------------------------------------+
| WATCHLIST [1] | PORTFOLIO [2]     Hi,User | Help[?] | Search[/] | ^c |
+-----------------------------------------------------------------------+
| WATCHLIST     | STOCK DETAIL                  | ORDER BOOK / TRADES   |
| CODE NAME     | Ticker  Price  Chg            | Bid: price  sz  ct   |
| PRICE CHG     | OHLC / PE / EPS / BPS / Div   | Ask: price  sz  ct   |
|               | Vol / Turnover / Lot Size      |                       |
| (scrollable,  |                                |-----------------------|
|  color-coded, | [1m 5m 15m 30m 1h Day Wk Mo Yr]| TRADES TAPE           |
|  highlighted  |                                | ts  price  dir  vol   |
|  row)         | +--CANDLESTICK CHART---------+ | (green/red bars)      |
|               | | OHLC candles               | |                       |
|               | | Volume bars underneath     | |                       |
|               | +----------------------------+ |                       |
|               | Price | High | Low | Var | Avg  |                       |
+-----------------------------------------------------------------------+
| Dow 50188 +0.10% | NASDAQ 23102 -0.58% | S&P ETF 693.73 -0.03%      |
+-----------------------------------------------------------------------+
```

### Specific UI Elements

**Watchlist (Left Column, ~250px)**
- Columns: CODE / NAME / PRICE / CHG (percent)
- Market prefix per row (US, HK, SZ, SH)
- Green for positive change, red/pink for negative
- Selected row: full-width highlight with contrasting background
- Scrollable with j/k or arrow keys
- Watchlist groups switchable via `G` key
- Toggle between watchlist and stock detail view via `t`

**Stock Detail (Center, flexible width)**
- Header: Ticker name + current price + change (absolute + percent), colored
- Status line: "Status: Trading" / "Status: Post"
- Fundamentals block: Open / Prev Close / High / Low / Average / Volume / Turnover /
  P/E (TTM) / EPS (TTM) / Shares / Shares Float / BPS / Div Yield (TTM) / Min Lot Size
- Timeframe selector: horizontal tab bar [1m | 5m | 15m | 30m | 1h | Day | Week | Month | Year]
  with `h/l` or left/right to switch, selected tab has border highlight
- Candlestick chart: Full OHLC candles (green body = up, red body = down) with
  wicks. Volume bars underneath the chart in corresponding colors.
  Y-axis price labels on left. Grid lines.
- Chart footer: Price / Highest / Lowest / Var (percent) / Avg / Current Volume

**Order Book (Right Top, ~280px)**
- Two rows: one bid, one ask (Level 1 depth shown)
- Bid side: price in green, size, count. Spread indicator "Bid: XX.X%"
- Ask side: price in red/pink, size, count
- Updates in real time -- prices and sizes flicker as they change

**Trades Tape (Right Bottom)**
- Columns: Timestamp / Price / Direction arrow / Volume
- Direction: green up-arrow for uptick, red down-arrow for downtick
- Volume shown as green/red horizontal bar proportional to size
- Auto-scrolls, newest at top
- High density -- fills the available vertical space

**Bottom Ticker Bar**
- Scrolling/static index quotes
- Market-dependent: US mode shows Dow/NASDAQ/S&P ETF
- HK mode shows HSI/HSCEI/HSTECH
- CN mode shows SSE Composite/SZSE Component/GEM Index
- Format: INDEX_NAME PRICE CHANGE PERCENT [Q/W/E sector tags]

**Keyboard Model (from Help screen)**
- General: `?` help, `` ` `` debug log, `/` search, `q/ESC` dismiss, `Enter` action, `R` refresh
- Stock Detail: `t` toggle watchlist view, `TAB/Shift+TAB` switch kline sampling,
  `h/l/Left/Right` switch candlestick interval
- Watchlist: `G` switch group, `t` toggle detail, `j/k/Up/Down` navigate
- Portfolio: `a` switch account, `c` switch currency, `j/k` navigate holdings

**Debug Log Panel**
- Toggled via backtick key
- Full-width panel at bottom showing timestamped log lines
- Format: `YYYY-MM-DDTHH:MM:SS.mmm+TZ LEVEL ThreadId(N) module::path: message`
- Shows quote updates, depth updates, system events
- Scrollable

**Portfolio View (Tab 2)**
- Expanded watchlist with additional columns: Volume / Turnover / Status (Trading/Post)
- Same left-column layout but wider, center shows Longbridge branding/about
- Sorted differently from watchlist (by position, not by code)


---


## Part 2: What TRiMiNL Adds on Top

Longbridge Terminal is a passive market viewer. TRiMiNL is an active trading operator console.
These are the TALON-specific layers that don't exist in Longbridge.

### TALON Overlay Panels

**ExecCore Status (replaces/augments Stock Detail header)**
- Current tier: Hatch / Takeoff / Payload
- Gate level: G0 (propose only) / G1 (auto within boundaries) / G2 (full auto)
- Control mode: DUAL / SUPERVISED / STRICT
- Regime: from RegimeDetect (TRENDING / MEAN_REVERTING / ELEVATED_IV / CRISIS / etc.)
- Stress multiplier: current value and tier

**Strategy Module Array**
- 9 modules displayed as status blocks (can overlay or replace watchlist in a mode)
- Each: Name / State (HOT / SUPPRESSED / STANDBY / HALTED) / Target description
- Color-coded by state (green/orange/dim/red)
- Selectable -- entering a module shows its signal history, current positions, trust level

**Signal Approval Queue (Dual-Control)**
- When a module proposes a trade, it appears as a queued item
- Shows: Module name, ticker, direction, size, entry price, stop, target, risk $, confidence
- Operator keys: `y` approve, `n` reject, `m` modify, `d` details
- Queue depth indicator in header
- This is the core DCC interaction loop

**Risk Mesh Panel**
- Position heat (aggregate exposure as % of buying power)
- Daily P&L vs loss cap
- Per-module exposure breakdown
- Margin utilization (if margin account)
- Cross-module correlation warnings

**Carousel / Vault Status**
- Session P&L
- Skim rate and accumulated vault amount
- Harvest events log

**EJECT Button**
- Single keystroke (configurable, default: `Ctrl+E` or dedicated key)
- Confirm dialog (one additional keystroke)
- Flattens all positions via market orders
- Same as Longbridge's `Ctrl+C` quit but for positions, not the app

**Blackbird (Hidden)**
- Not visible by default
- Revealed via specific key combo or config flag
- Shows RAMjet / Chine / Spike subsystem status
- Purple/encrypted aesthetic as in the HTML mockup


---


## Part 3: Ratatui Widget Mapping

Every UI element maps to a Ratatui widget or composition of widgets.

| Longbridge Element         | Ratatui Widget                          | TALON Data Source              |
|----------------------------|-----------------------------------------|--------------------------------|
| Watchlist table            | Table (stateful, scrollable)            | Polygon.io REST + WebSocket    |
| Selected row highlight     | Table::highlight_style()                | --                             |
| Candlestick chart          | Canvas or custom widget                 | Polygon.io aggregates          |
| Volume bars                | BarChart or Canvas overlay              | Polygon.io aggregates          |
| Timeframe tabs             | Tabs widget                             | Local state                    |
| Order book depth           | Table (2-row, colored)                  | IBKR L2 or Polygon L2         |
| Trades tape                | List (stateful, auto-scroll)            | Polygon.io trades stream       |
| Bottom ticker bar          | Paragraph with styled spans             | Polygon.io index quotes        |
| Top tab bar                | Tabs widget                             | Local state                    |
| Debug log                  | List (toggled, bottom panel)            | tracing subscriber             |
| Signal approval queue      | List or Table (custom)                  | ExecCore SignalEnvelope channel |
| Module status array        | Custom grid of Block + Paragraph        | Module state channels          |
| Risk mesh                  | Table or custom gauge widgets           | RiskMesh aggregator            |
| EJECT overlay              | Clear + Popup (Block centered)          | ExecCore kill switch           |

### Custom Widgets Needed

1. **CandlestickChart** -- No built-in OHLC widget in Ratatui. Must build on Canvas.
   Reference: `ratatui-candlestick` crate exists but is minimal. Likely roll our own.
   Needs: OHLC bars with wicks, color-coded, Y-axis labels, X-axis time labels,
   volume bars underneath, crosshair on hover (optional for v1).

2. **SignalCard** -- Dual-Control approval card. Block with structured content:
   module name, ticker, direction arrow, size, prices, risk, confidence meter.
   Action keys rendered at bottom.

3. **ModuleGrid** -- 4x2 grid (or 3x3) of module status blocks. Each block is a
   small Block with styled Paragraph inside. Hover/select reveals detail tooltip
   (if terminal supports mouse) or detail panel on Enter.

4. **TickerBar** -- Bottom bar showing scrolling/static index quotes. Paragraph with
   Span styling, updated on timer.


---


## Part 4: Data Pipeline

Every widget must have a verified data source. No dead screens.

### Market Data: Polygon.io

Already integrated for PRESSbox. Provides:
- REST: Aggregates (candles), snapshots, ticker details, fundamentals
- WebSocket: Real-time trades, quotes (NBBO), per-second aggregates
- Coverage: US equities and options. No HK/CN (not needed for TALON).

Data flow:
```
Polygon.io WS --> talon-datafeed crate --> broadcast channel
                                       --> Parquet writer (FiREPLY)
                                       |
                                       +--> TRiMiNL subscriber (renders)
                                       +--> RegimeDetect subscriber
                                       +--> Module signal generators
```

### Execution: IBKR TWS API

Data flow:
```
ExecCore --> ibkr adapter --> TWS Gateway --> IBKR
                          <-- fills, positions, account data
```

The ibkr adapter surfaces:
- Account data (buying power, margin, P&L)
- Position state (open positions, unrealized P&L)
- Order status (pending, filled, rejected)
- L2 depth (if subscribed)

### Internal State: Channels

All TALON subsystems communicate via tokio broadcast/mpsc channels.
TRiMiNL subscribes to:
- `MarketData` (quotes, trades, candles)
- `SignalEnvelope` (proposed trades from modules)
- `OrderEvent` (fills, rejections, status changes)
- `RiskState` (position heat, drawdown, margin)
- `ModuleState` (per-module status changes)
- `SystemEvent` (regime changes, tier transitions, errors)


---


## Part 5: Crate Structure in CashCache

```
CashCache/
  TALON/
    talon-triminl/          <-- THIS IS THE NEW CRATE
      src/
        main.rs             -- Entry point, terminal setup, event loop
        app.rs              -- App state, mode management, key dispatch
        ui/
          mod.rs            -- Top-level layout (3-column + header + footer)
          watchlist.rs      -- Left column watchlist table
          stock_detail.rs   -- Center column (fundamentals + chart)
          candlestick.rs    -- Custom OHLC chart widget
          order_book.rs     -- Right top: bid/ask depth
          trades_tape.rs    -- Right bottom: live trades
          ticker_bar.rs     -- Bottom index ticker
          modules.rs        -- Strategy module grid overlay
          signal_queue.rs   -- Dual-Control approval queue
          risk_panel.rs     -- Risk mesh display
          vault_panel.rs    -- Carousel/Vault status
          eject.rs          -- EJECT confirmation overlay
          debug_log.rs      -- Toggled debug log panel
          blackbird.rs      -- Hidden subsystem panel
        data/
          mod.rs            -- Data layer trait definitions
          polygon.rs        -- Polygon.io adapter (WS + REST)
          ibkr.rs           -- IBKR account/position adapter
          mock.rs           -- Mock data for development/testing
        event.rs            -- Terminal event handling (keys, mouse, resize)
        config.rs           -- TUI config (keybindings, layout, colors)
      Cargo.toml
```

Dependencies:
- `ratatui` (TUI framework)
- `crossterm` (terminal backend)
- `tokio` (async runtime, already in TALON)
- `talon-core` (ExecCore, RiskMesh, module interfaces)
- `talon-datafeed` (Polygon.io adapter)
- `talon-ibkr` (IBKR adapter)


---


## Part 6: Build Sequence

### Phase 0: Skeleton (Sessions 1-2)

Objective: Ratatui app that starts, shows the 3-column layout, and quits on `q`.

- Terminal init/restore with crossterm
- App struct with mode enum (Watchlist, Portfolio, ModuleView)
- Layout: Header | Left (250) | Center (flex) | Right (280) | Footer
- Static placeholder text in each panel
- Key handling: `q` quit, `1`/`2` tab switch, `?` help overlay
- VERIFIED: compiles, runs, renders, exits cleanly

### Phase 1: Watchlist + Mock Data (Sessions 3-4)

Objective: Scrollable watchlist with mock tickers, keyboard navigation.

- Mock data source: 20 tickers with randomized prices/changes
- Table widget with CODE / NAME / PRICE / CHG columns
- j/k navigation with highlighted row
- Green/red coloring based on change sign
- Selected ticker updates center panel header
- VERIFIED: can scroll through all tickers, colors correct

### Phase 2: Stock Detail + Candlestick Chart (Sessions 5-7)

Objective: Center panel shows fundamentals and OHLC chart for selected ticker.

- Fundamentals block: OHLC, PE, EPS, volume, etc. (mock data initially)
- Timeframe tab bar: h/l to switch intervals
- Custom CandlestickChart widget on Canvas
- Volume bars underneath candles
- Chart footer with summary stats
- VERIFIED: chart renders with mock OHLC data, timeframe switching works

### Phase 3: Polygon.io Live Data (Sessions 8-9)

Objective: Replace mock data with live Polygon.io feeds.

- Polygon.io WebSocket connection for real-time quotes
- REST calls for historical aggregates (candle data)
- REST calls for ticker details (fundamentals)
- Watchlist updates in real time
- Chart updates with streaming data
- VERIFIED: live prices flowing, chart populating from real data

### Phase 4: Order Book + Trades Tape (Sessions 10-11)

Objective: Right panel shows live depth and trades.

- Order book: Bid/Ask table from Polygon NBBO or IBKR L2
- Trades tape: List widget with auto-scroll
- Color-coded volume bars in tape
- Direction arrows (uptick/downtick)
- Bottom ticker bar with index quotes
- VERIFIED: real-time trades flowing, order book updating

### Phase 5: TALON Integration -- Modules + Signals (Sessions 12-14)

Objective: Wire TRiMiNL to ExecCore for signal flow.

- Module status grid (overlay mode, toggled via key)
- Signal approval queue (Dual-Control UI)
- y/n/m/d key handling for signal approval
- Trust level display per module
- Regime indicator in header
- VERIFIED: mock signals appear in queue, approval/rejection flows through

### Phase 6: Risk + Vault + EJECT (Sessions 15-16)

Objective: Risk monitoring and kill switch.

- Risk mesh panel (position heat, daily P&L, margin)
- Vault status (session P&L, skim rate, accumulated)
- EJECT overlay with confirmation
- EJECT sends kill command to ExecCore
- VERIFIED: EJECT flattens all mock positions, risk panel updates

### Phase 7: Debug Log + Blackbird + Polish (Sessions 17-18)

Objective: Debug tooling and hidden features.

- Debug log panel (toggled via backtick)
- tracing subscriber that writes to TUI buffer
- Blackbird hidden panel (specific key combo)
- Color theme refinement (match the dark terminal aesthetic)
- Help overlay with full keybinding reference
- VERIFIED: debug log shows real system events, Blackbird reveals

### Phase 8: IBKR Live Execution (Sessions 19-20)

Objective: Real orders flowing through.

- IBKR adapter wired to ExecCore
- Account data displayed in portfolio tab
- Order submission from approved signals
- Fill confirmations in trades tape
- Position tracking in risk panel
- VERIFIED: paper trading account receives orders, fills reflected in TUI


---


## Part 7: Key Differences from Longbridge Terminal

| Aspect                | Longbridge Terminal          | TRiMiNL                          |
|-----------------------|------------------------------|----------------------------------|
| Purpose               | Passive market viewer        | Active trading operator console  |
| Data source           | Longbridge proprietary API   | Polygon.io + IBKR               |
| Markets               | HK, CN, US, SG, JP, UK, DE  | US only (equities + options)     |
| Execution             | View only (no order API)     | Full order flow via ExecCore     |
| Supervision           | None                         | DCC model with signal approval   |
| Modules               | None                         | 9 strategy modules with states   |
| Risk management       | None                         | RiskMesh with position heat      |
| Profit harvesting     | None                         | Carousel/Vault system            |
| Kill switch           | Ctrl+C (quit app)            | EJECT (flatten all positions)    |
| Source                 | Closed binary                | Open, in CashCache monorepo     |
| TUI framework         | Ratatui (assumed)            | Ratatui (confirmed)              |
| Keyboard model        | Vim-style                    | Vim-style (matching Longbridge)  |


---


## Part 8: Open Questions

1. **L2 Depth source:** Polygon.io provides NBBO (top of book) on their standard plans.
   Full L2 depth requires either Polygon's higher tier or IBKR's L2 subscription.
   For v1, NBBO (bid/ask with size) is sufficient. Full depth can come later.

2. **Options chain display:** Longbridge doesn't show options chains in its TUI.
   TALON trades options heavily (SAGE, Firebird, Siphon). TRiMiNL will eventually need
   an options chain viewer. Not in v1 -- module signals abstract away strike selection.

3. **Multi-monitor:** Longbridge is single-terminal. TRiMiNL could support tmux-style
   detach or multiple terminal instances showing different views. Defer to post-v1.

4. **Mouse support:** Longbridge appears keyboard-only. Ratatui supports mouse events.
   Worth enabling for chart interaction (click to select candle, scroll to zoom).
   Low priority -- keyboard-first matches the DCC operator model.

5. **Candlestick crate:** The `ratatui-candlestick` crate on crates.io exists but is
   minimal and may not meet our needs (wick rendering, volume overlay, timeframe
   switching). Evaluate before deciding to use vs. build custom. Build custom is likely.
