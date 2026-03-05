# TALON — Trade Autonomously with Limited Override Necessity

## What TALON Is

A semi-autonomous Rust trading terminal targeting IBKR Gateway. Phase 0 (Hatch tier): operator-supervised, paper trading on port 4002. The system scans a 98-symbol watchlist, generates order intents through strategy modules, routes them through risk checks and a supervision gate, and renders everything in a Ratatui terminal (TRiMiNL).

## Commands

```bash
cargo check --workspace          # Typecheck all 20 crates
cargo test --workspace           # Run all 53 tests
cargo clippy --workspace         # Lint (must pass with zero warnings)
cargo run -p talon               # Launch TRiMiNL terminal (needs IBKR Gateway or mock)
cargo test -p talon-firebird     # Test a single crate
```

Requires: Rust 1.85+, edition 2024. Config at `config/talon.toml`, secrets in `.env` (never committed).

## Tier Progression

| Tier | Description |
|------|-------------|
| **Hatch** | Single module, operator-supervised, training wheels |
| **Takeoff** | Multi-module, graduated autonomy |
| **Payload** | All 8 strategy entities hot, regime-gated, full autonomy path |

## Workspace Layout (20 crates)

```
CashCache/TALON/
├── Cargo.toml              # Workspace root (NO binary here)
├── config/talon.toml       # Runtime config (watchlist, tiers, modes)
├── .env                    # Secrets (POLYGON_API_KEY, IBKR creds) — NOT committed
├── crates/
│   ├── talon/              # Binary crate — main(), startup, wiring
│   ├── talon-types/        # Leaf crate — shared types, enums, traits
│   ├── talon-data/         # Market data — Polygon.io + Yahoo Finance polling
│   ├── talon-db/           # SQLite WAL persistence (event store)
│   ├── talon-broker/       # IBKR Gateway adapter (ibapi v2)
│   ├── talon-risk/         # Risk mesh + stress engine (drawdown tiers)
│   ├── talon-exec/         # ExecCore — intent processing, supervision gate
│   ├── talon-grad/         # [scaffold] Graduation system (Trust Tiers T0-T3)
│   ├── talon-regime/       # [scaffold] Regime detector (market classification)
│   ├── talon-util/         # Indicators: SMA, RSI, Bollinger, RVOL, volume_declining
│   ├── talon-triminl/      # TRiMiNL — Ratatui operator terminal
│   ├── talon-carousel/     # [scaffold] Profit harvest — transfers wins to Vault
│   ├── talon-firebird/     # Strategy: oversold reversal (RSI + volume divergence)
│   ├── talon-thunderbird/  # Strategy: overextension fade (Bollinger + volume climax)
│   ├── talon-taxi/         # Strategy: pullback-to-support swing entry
│   ├── talon-climb/        # [scaffold] Strategy: momentum
│   ├── talon-sage/         # [scaffold] Strategy: gamma scalping (0DTE)
│   ├── talon-parashort/    # [scaffold] Strategy: parabolic fade
│   ├── talon-siphon/       # [scaffold] Strategy: theta harvesting
│   └── talon-snapback/     # [scaffold] Strategy: 0DTE mean reversion
```

`[scaffold]` = empty lib.rs with doc comment, compiles but no logic yet.

**Implemented crates** (have logic, tests, or full functionality):
talon, talon-types, talon-data, talon-db, talon-broker, talon-risk, talon-exec,
talon-util, talon-triminl, talon-carousel, talon-firebird, talon-thunderbird, talon-taxi.

## Strategy Modules (8 total)

3 implemented (have scanning logic): Firebird, Thunderbird, Taxi.
5 scaffold-only: Climb, SAGE, ParaShort, Siphon, Snapback.

**Evolution chain:** Snapback → YoYo (single step). YoYo adds a regime-switching FSM
(trend-following + mean reversion). Same ECS entity, upgraded logic. Never concurrent.

## Carousel — Profit Harvest Module

`talon-carousel` transfers realized wins from TALON's active account to Vault
(the long-term hold portfolio at `CashCache/Vault/`). Deps: `talon-types`,
`talon-broker` (transfer initiation), `talon-db` (harvest journal).

## Portfolio — Active Account State

TALON's active portfolio state (positions, cash, buying power, margin) is a **Bevy
Resource** in `talon-types`, not a separate crate:

```rust
#[derive(Resource)]
pub struct Portfolio {
    pub positions: HashMap<Symbol, Position>,
    pub cash: Decimal,
    pub buying_power: Decimal,
    pub net_liquidation: Decimal,
    pub daily_pnl: Decimal,
    pub margin_used: Decimal,
}
```

- ExecCore **writes** it on fills.
- talon-risk **reads** it for capacity checks.
- TRiMiNL **reads** it for display.
- Strategy modules **never read it directly** — they get risk allocation through
  `SignalEnvelope` `RiskParams`.

This is distinct from **Vault** (`CashCache/Vault/`), which is the long-term hold
portfolio managed as a sibling project.

## Dependency Graph Rules (Firewalls)

1. **talon-exec CANNOT depend on any strategy crate.** ExecCore is signal-agnostic.
2. **Strategy crates CANNOT depend on talon-exec.** Communication via channels only.
3. **No strategy crate depends on another strategy crate.** Module isolation is absolute.
4. **talon-triminl CANNOT write to talon-exec or talon-grad.** Read-only display; commands via channels.
5. **Strategy crates all share the same dep signature:** `talon-types` + `talon-util` (+ `talon-data` when needed).
6. **Strategy crates CANNOT read Portfolio directly.** Risk allocation comes through `RiskParams` in the `SignalEnvelope`.
7. **talon-carousel CANNOT modify Portfolio.** Read-only access to realized P&L; initiates transfers via broker.
8. **talon-data owns the DashMap quote cache.** All consumers (TRiMiNL, strategy modules, regime detector) get read-only access. Only the broker feed handler writes.

## Key Architectural Patterns

- **All prices are `rust_decimal::Decimal`.** Never f64 for money. No exceptions.
- **`TradingModule` trait** lives in `talon-types::strategy` — implemented by each strategy crate.
- **`ScanResult`** = `Vec<OrderIntent>` (fire signals) + `Vec<ApproachingSetup>` (almost-signals for scanner display).
- **Channel architecture:** `broadcast::channel<QuoteEvent>` for market data fan-out. `mpsc` for intents (modules → ExecCore) and fills (broker → ExecCore).
- **`AppState`** shared via `arc_swap` + `watch::channel` — the TUI polls at 10fps.
- **Event store:** dedicated OS thread writer with `mpsc::channel<StoreCommand>`, SQLite WAL mode. Never blocks async.
- **Stress engine:** 5 tiers (Normal → Tier1 → Flameout → Nosedive → CircuitBreaker) based on equity drawdown from peak.

## Test Count: 53

| Crate | Tests |
|-------|-------|
| talon-util (indicators) | 11 |
| talon-risk (mesh + stress) | 10 |
| talon-types (trust + sovereign + portfolio) | 10 |
| talon-exec (supervision + ExecCore) | 7 |
| talon-broker (mock + rate limiter) | 5 |
| talon-carousel (harvest) | 5 |
| talon-firebird (signal gen) | 3 |
| talon-db (event store) | 2 |

## Config

- `config/talon.toml` — watchlist (98 symbols), tier definitions, regulatory flags, observability
- `config/risk.toml` — risk mesh parameters
- `config/graduation.toml` — graduation gate thresholds
- `config/brokers/` — broker-specific config
- `.env` — API keys, never committed

## Rust Version

- Edition 2024, rust-version 1.85
- `cargo check --workspace` and `cargo test --workspace` both pass clean

## Sibling Projects (under CashCache/)

- **Vault/** — Long-term hold portfolio (moved from talon-vault)
- **FiREPLY/** — Market intelligence & alerting (planned)

## ExecCore Supervision Pipeline

ExecCore (`talon-exec::exec_core`) manages the full intent lifecycle:

1. **Receive** intent from modules via mpsc channel
2. **Risk evaluate** through RiskMesh (nosedive gate, position limits, exposure)
3. **Supervision gate** routes by module tier:
   - SupervisedAutonomy (Hatch) → AutoExecute
   - DualControl (Takeoff) → Check trust ledger
   - DualControlStrict (0DTE/shorts) → RequiresApproval always
4. **Pending queue** (max 10 intents) with 10s timeout auto-rejection
5. **Operator approval** via TUI keybindings ('a'/'r'/Up/Down/'A'/'R')
6. **Broker submission** through BrokerSessionManager (spawn_blocking pattern)

## TRiMiNL — Operator Terminal

Longbridge Terminal-inspired 3-column Ratatui TUI (`talon-triminl`).

**Layout:** Header | Signal Queue (conditional) | Body | Ticker Bar | Footer
- **Watchlist mode [1]:** Left (watchlist 28col) | Center (stock detail + OHLC chart) | Right (order book + trades tape 30col)
- **Portfolio mode [2]:** Full-width position table

**Overlays** (rendered on top of main layout):
- `?` Help — keybinding reference
- `` ` `` Debug log — timestamped event log
- `m` Modules — strategy module grid
- `x` Risk mesh — stress/regime/P&L/vault
- `K` EJECT — flatten all positions (Y to confirm)

**Keybindings:** `j/k` watchlist nav, `h/l` timeframe, `a/r` approve/reject signal, `A/R` all, `H` entry halt, `ESC` close overlay, `q` quit, `Ctrl+X` emergency shutdown.

**Data flow:** `AppState` via `watch::channel` (one-way from Governor). Mock data via `RefCell<CandleCache>` (only regenerates on ticker/timeframe change). Live data in Phase 3+.

**File structure:** `lib.rs` → `app.rs` (Mode/Overlay/App), `input.rs` (keybindings), `render.rs` (event loop), `skin.rs` (colorblind-safe palette), `ui/mod.rs` (layout), `ui/*.rs` (12 panel widgets), `data/mock.rs` (synthetic market data).

## Carousel Harvest

`talon-carousel::Carousel` calculates profit harvest on winning trade closes:
- Base rate: 15% of realized P&L
- Inverse account size scaling (drops to 5% floor at large NLV)
- Minimum harvest threshold: $1.00
- `harvest_enabled=false` at Hatch tier (logging only, no transfers)

## Known Limitations (Hatch Tier)

- Hatch tier requires manual approval for DualControl/DualControlStrict modules
- 15s Firebird scan latency (quote-driven, depends on IBKR bar frequency)
- Polygon.io free tier: 5 requests/minute, Yahoo fallback at 30s intervals
- TUI requires minimum 80×15 terminal (shows "Terminal too small" if undersized)
- Session replay not yet implemented (scaffold only)
- No auto-trading: every non-Hatch intent requires operator approval

## Takeoff Tier Roadmap

- Bevy ECS migration for 8 concurrent strategy entities
- G2 semi-supervised gate: auto-approve low-risk intents via trust graduation
- Carousel vault transfers enabled (currently logging only)
- Regime detector activation (market classification → module enable/disable)
- Trust ledger persistence and cross-session auto-trust

## Reference Documents

- `TALON_SCAFFOLD.md` — v3.2.0 scaffold spec (20-crate layout, dep graph, ECS entity map, tier progression)
- `WIRING.md` — Channel topology, signal-to-fill pipeline, verification checklist
- `TESTING.md` — Test organization, MockBroker usage, test patterns
- `RUNBOOK.md` — Pre-flight, operations, troubleshooting, monitoring, emergency procedures
