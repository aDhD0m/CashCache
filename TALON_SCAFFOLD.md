# TALON v3.2.0 -- Project Scaffold (FINAL)

## Co-Commander Audit: PASSED

Audited: 2026-03-03 (revision 3)
Changes from prior drafts:
- Vault is the LONG-TERM HOLD portfolio (CashCache/Vault/), NOT internal state
- TALON's active portfolio state is a Bevy Resource in talon-types, not a crate
- Added talon-carousel: harvest module that transfers profits from TALON to Vault
- Removed fake vault path dependency from talon-risk
- Payload is the premium tier name, NOT an evolution of Snapback
- Snapback evolution is a single step: Snapback --> YoYo
- Added talon-data crate (market data pipeline, Parquet, DashMap cache)
- Added rust_decimal, arrow, parquet, csv, rand to workspace deps
- ibapi remains UNAUDITED -- listed but commented out with warning
- yatws noted as alternative IBKR crate (rate limiter, session replay)
- Blackbird scope gated to Payload tier

---

## Monorepo Structure

```
CashCache/
+-- TALON/                  # Active trading terminal (this scaffold)
+-- Vault/                  # Long-term hold portfolio (profit preservation)
+-- FiREPLY/                # Market intelligence & alerting
```

These are siblings. TALON does NOT depend on Vault as a Cargo path
dependency. The only interface between them is Carousel -- a TALON
module that initiates harvest transfers to Vault after winning trades.

Vault is cold storage. TALON is the cockpit. Carousel is the baggage
carousel that moves profits from one to the other.

---

## TALON Workspace

```
CashCache/TALON/
+-- Cargo.toml              # Workspace root
+-- config/
|   +-- talon.toml          # Runtime config (committed)
|   +-- carousel.toml       # Harvest config -- rates, instruments, deployment rules
+-- .env                    # Secrets -- IBKR creds, account ID (NOT committed)
+-- CLAUDE.md               # Architectural rules for AI-assisted development
+-- README.md
+-- crates/
    +-- talon-types/        # Leaf crate -- shared types, enums, traits, Portfolio Resource
    +-- talon-data/         # Market data pipeline -- DashMap cache, Parquet I/O, synthetic gen
    +-- talon-db/           # SQLite WAL persistence (orders, journal, graduation)
    +-- talon-broker/       # IBKR Gateway adapter (port 4002 paper, 4001 live)
    +-- talon-risk/         # Pre-routing risk gate (reads Portfolio Resource)
    +-- talon-exec/         # ExecCore -- signal intake, graduation gate, order lifecycle
    +-- talon-grad/         # Graduation system -- Trust Tiers (T0-T3), Gate Levels (G0-G4)
    +-- talon-regime/       # Regime detector -- market classification, module activation
    +-- talon-carousel/     # Harvest module -- profit transfer from TALON to Vault
    +-- talon-util/         # Time, math, diagnostics
    +-- talon-triminl/      # TRiMiNL -- Ratatui operator terminal
    +-- talon-firebird/     # Strategy: oversold reversal (put credit spreads)
    +-- talon-thunderbird/  # Strategy: overextension fade (long OTM puts)
    +-- talon-taxi/         # Strategy: [spec TBD]
    +-- talon-climb/        # Strategy: riding momentum moves up
    +-- talon-sage/         # Strategy: gamma exposure scalping (0DTE options)
    +-- talon-parashort/    # Strategy: parabolic fade (short selling)
    +-- talon-siphon/       # Strategy: theta decay harvesting (range-bound)
    +-- talon-snapback/     # Strategy: 0DTE mean reversion (evolves to YoYo)
    +-- talon/              # Binary crate -- main(), startup, arg parsing
```

---

## Portfolio State Model

TALON's active portfolio state is NOT a separate crate or repo. It is
a Bevy Resource defined in talon-types:

```rust
#[derive(Resource)]
pub struct Portfolio {
    pub positions: HashMap<Symbol, Position>,
    pub cash: Decimal,
    pub buying_power: Decimal,
    pub net_liquidation: Decimal,
    pub daily_pnl: Decimal,
    pub session_pnl: Decimal,
    pub margin_used: Decimal,
    pub margin_available: Decimal,
}
```

- ExecCore WRITES to Portfolio on every fill (via Bevy system access)
- ExecCore populates Portfolio on startup via broker reconciliation
- talon-risk READS Portfolio to check capacity before routing signals
- TRiMiNL READS Portfolio for display
- Carousel READS Portfolio to calculate harvest amounts after wins
- Strategy modules NEVER read Portfolio directly -- they get risk
  allocation via their SignalEnvelope's RiskParams

This is in-process shared state, not a cross-repo dependency.

---

## Carousel (Profit Harvest)

Carousel is the bridge between TALON (active trading) and Vault
(long-term holds). It implements the profit preservation logic from
cashcache.toml:

```
TALON closes a winning trade
  --> ExecCore emits ExecutionReport (fill + realized P&L)
  --> Carousel calculates harvest amount (per harvest mode config)
  --> Carousel initiates transfer to Vault sub-account
  --> Vault deploys into configured instruments (SPY/QQQ/SCHD)
```

Carousel does NOT execute Vault-side deployments. It only initiates
the cash transfer. Vault has its own deployment logic.

Harvest modes (from config):
- fixed: flat percentage of every win
- inverse_account_size: higher harvest rate at smaller account sizes
- win_streak: escalating harvest on consecutive wins
- off: disabled

---

## Crate Dependency Graph

```
talon (binary)
+-- talon-triminl           TRiMiNL operator terminal
|   +-- talon-types         (Portfolio Resource, display types)
|   +-- talon-data          (read-only: quote cache for display)
|   +-- talon-exec          (read-only: event subscriptions, state display)
|   +-- talon-regime        (read-only: current regime, module activation states)
|
+-- talon-exec              ExecCore
|   +-- talon-types         (Portfolio Resource -- write access)
|   +-- talon-broker
|   +-- talon-risk
|   +-- talon-grad
|   +-- talon-db            (WAL journaling)
|
+-- talon-carousel          Harvest Module
|   +-- talon-types         (Portfolio Resource -- read access)
|   +-- talon-broker        (initiates cash transfers to Vault sub-account)
|   +-- talon-db            (harvest journal)
|
+-- talon-regime            Regime Detector
|   +-- talon-types
|   +-- talon-data          (reads market data for classification)
|
+-- talon-data              Market Data Pipeline
|   +-- talon-types
|   +-- talon-broker        (live data subscription)
|
+-- talon-risk              Risk Gate
|   +-- talon-types         (Portfolio Resource -- read access)
|
+-- talon-grad              Graduation System
|   +-- talon-types
|   +-- talon-db            (read: trade journal, graduation tables)
|
+-- talon-broker            Broker Adapter
|   +-- talon-types
|
+-- talon-db                Persistence
|   +-- talon-types
|
+-- talon-util              Utilities
|   +-- talon-types
|
+-- talon-types             Leaf (no internal deps)
|
+-- Strategy modules (all share identical dep signature):
    talon-firebird
    talon-thunderbird
    talon-taxi
    talon-climb
    talon-sage
    talon-parashort
    talon-siphon
    talon-snapback
    +-- talon-types
    +-- talon-data          (read-only: quote cache, historical bars)
    +-- talon-util
```

### Firewall Rules

1. talon-exec CANNOT depend on any strategy crate.
   Strategy modules are signal sources. ExecCore is signal-agnostic.

2. talon-grad CANNOT be depended on by talon-exec directly for tier queries
   in a way that lets the bot infer its own trust level and change behavior.
   Graduation state flows through the type-state gate only.

3. Strategy crates CANNOT depend on talon-exec.
   Communication is via bounded channels (SignalEnvelope in, ExecutionReport out).
   No shared mutable state.

4. talon-triminl CANNOT write to talon-exec or talon-grad.
   The TUI reads state for display. Operator commands flow through
   dedicated command channels, not direct struct mutation.

5. No strategy crate depends on another strategy crate.
   Module isolation is absolute.

6. talon-data owns the DashMap quote cache. All readers (TUI, strategy modules,
   regime detector) get read-only access. Only the broker feed handler writes.

7. Strategy crates CANNOT read Portfolio directly.
   Risk allocation comes through RiskParams in SignalEnvelope, enforced by
   talon-risk. Strategies do not know account size, total exposure, or
   other modules' positions.

8. talon-carousel CANNOT modify Portfolio.
   It reads realized P&L from execution reports and initiates transfers
   via the broker adapter. It does not manipulate TALON's active state.

---

## Workspace Cargo.toml

```toml
[workspace]
resolver = "2"
members = [
    "crates/talon-types",
    "crates/talon-data",
    "crates/talon-db",
    "crates/talon-broker",
    "crates/talon-risk",
    "crates/talon-exec",
    "crates/talon-grad",
    "crates/talon-regime",
    "crates/talon-carousel",
    "crates/talon-util",
    "crates/talon-triminl",
    "crates/talon-firebird",
    "crates/talon-thunderbird",
    "crates/talon-taxi",
    "crates/talon-climb",
    "crates/talon-sage",
    "crates/talon-parashort",
    "crates/talon-siphon",
    "crates/talon-snapback",
    "crates/talon",
]

[workspace.package]
edition = "2024"
rust-version = "1.85"
license = "UNLICENSED"
publish = false

[workspace.dependencies]
# Async runtime
tokio = { version = "1", features = ["full"] }

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Error handling
thiserror = "2"
anyhow = "1"

# Observability
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Time
chrono = { version = "0.4", features = ["serde"] }

# Exact decimal arithmetic -- NEVER use f64 for money
rust_decimal = "1"
rust_decimal_macros = "1"

# Database (orders, journal, graduation, harvest log)
rusqlite = { version = "0.32", features = ["bundled", "chrono"] }

# Market data storage
arrow = { version = "53", features = ["prettyprint"] }
parquet = { version = "53", features = ["arrow"] }
csv = "1"

# Configuration
config = { version = "0.14", default-features = false, features = ["toml"] }

# Lock-free state sharing
arc-swap = "1"

# Async trait support
async-trait = "0.1"

# Concurrent maps (quote cache)
dashmap = "6"

# Terminal
crossterm = { version = "0.28", features = ["event-stream"] }
ratatui = "0.29"

# CLI
clap = { version = "4", features = ["derive"] }

# Statistics
statrs = "0.18"
hdrhistogram = "7"

# Option pricing -- hot path (sub-microsecond Black-Scholes)
# quantrs = "0.1"  # TODO: pin version after benchmarking

# ECS (justified at 8 concurrent strategy entities)
bevy_ecs = "0.15"

# HTTP (market data APIs, external data)
reqwest = { version = "0.12", features = ["json"] }

# Synthetic data generation (dev/test only)
rand = "0.8"
rand_distr = "0.4"

# IBKR adapter -- PICK ONE after audit:
# ibapi = "1"    # UNAUDITED -- verify maintenance, API coverage, combo orders
# yatws = "0.1"  # Alternative -- has rate limiter, session replay, options builder
#                 # WARNING: v0.1 API will break. License unconfirmed.

# Testing
tokio-test = "0.4"
```

---

## Bevy ECS Entity Map

### Infrastructure Systems (always running, not regime-gated)

| System        | Role                                               |
|---------------|-----------------------------------------------------|
| ExecCore      | Signal intake, graduation gate, order routing       |
| Portfolio     | Bevy Resource -- active positions, cash, margin     |
| RegimeDetect  | Market classification, module activation gates      |
| Carousel      | Harvest profits from wins, transfer to Vault        |
| TRiMiNL       | Operator terminal rendering + command input         |
| DataPipeline  | Quote cache writes, Parquet archival, feed health   |
| Persistence   | WAL flush, journal writes                           |

### Strategy Entities (regime-gated, 8 total)

| Entity       | Crate             | Activation Conditions                       |
|--------------|-------------------|---------------------------------------------|
| Firebird     | talon-firebird    | Capitulation detected on quality names       |
| Thunderbird  | talon-thunderbird | Overextension on large/mid-cap               |
| Taxi         | talon-taxi        | [TBD]                                        |
| Climb        | talon-climb       | Confirmed momentum, uptrend regime           |
| SAGE         | talon-sage        | Gamma flip at key strikes, GEX anomaly       |
| ParaShort    | talon-parashort   | Parabolic blow-off on low-float names        |
| Siphon       | talon-siphon      | Range-bound, elevated IV, theta-rich         |
| Snapback     | talon-snapback    | Range day, mean-reversion regime (0DTE)      |

All 8 entities are loaded at Payload tier. The regime detector flips
activation components. Dormant modules receive market data and update
internal state but are suppressed from signal generation.

### Tier Progression

| Tier     | Description                                        |
|----------|----------------------------------------------------|
| Hatch    | Single module, operator-supervised, training wheels |
| Takeoff  | Multi-module, graduated autonomy                   |
| Payload  | All 8 entities hot, regime-gated, full autonomy path |

### Evolution Slot

Snapback is the only entity that evolves in-place:

```
Snapback (MR0DTEAN lineage)
  --> YoYo (BR0DTEAN lineage: adds trend-following FSM via regime switching)
```

Same ECS entity, upgraded strategy logic. Never concurrent.
Prerequisites for YoYo: 90+ days live Snapback, 200+ trades, replay
system functional, Gate D rejection analysis complete.

---

## Data Pipeline Architecture

```
Live Data (IBKR Gateway via talon-broker)
    |
    v
DashMap<Symbol, QuoteData>       <-- talon-data concurrent cache
    |
    +---> TRiMiNL renderer (10fps poll, read-only)
    |
    +---> Strategy modules (read-only, per-module subscription)
    |
    +---> Regime detector (read-only, classification input)
    |
    v
Parquet Archive (end-of-session flush via talon-data)
    |
    v
Historical Backtest (offline, arrow-rs reader)
```

Development/testing substitutes the live feed with synthetic generators
(random walk with drift). The DashMap interface is identical -- no
component downstream knows whether data is live or synthetic.

All prices stored as rust_decimal::Decimal in memory, String in Parquet.
Never f64 for money.

---

## Profit Flow (TALON --> Vault)

```
TALON strategy wins a trade
    |
    v
ExecCore emits ExecutionReport (realized P&L > 0)
    |
    v
Carousel reads harvest config (carousel.toml)
    |
    +-- mode: inverse_account_size
    |   Account NLV $3,000 --> 25% harvest rate
    |   Realized P&L $200 --> harvest $50
    |
    v
Carousel initiates $50 transfer via talon-broker
    TALON active account --> Vault sub-account (IBKR internal transfer)
    |
    v
Vault deploys $50 into SPY/QQQ/SCHD per instrument weights
    (Vault-side logic, outside TALON's scope)
```

Carousel is an infrastructure system, not regime-gated. It runs after
every winning trade regardless of which strategy module generated it.

---

## Scaffold Commands

```bash
cd CashCache/TALON

# Create all crate directories
for crate in talon-types talon-data talon-db talon-broker talon-risk \
             talon-exec talon-grad talon-regime talon-carousel talon-util \
             talon-triminl talon-firebird talon-thunderbird talon-taxi \
             talon-climb talon-sage talon-parashort talon-siphon \
             talon-snapback; do
    mkdir -p "crates/$crate/src"
    echo "// $crate" > "crates/$crate/src/lib.rs"
done

# Binary crate
mkdir -p crates/talon/src
cat > crates/talon/src/main.rs << 'EOF'
fn main() {
    println!("TALON v3.2.0 starting...");
}
EOF
```

---

## Verify

```bash
cargo check --workspace
```

Should compile with zero errors (empty crates, no logic yet).

---

## What This Scaffold Does NOT Include

- Individual crate Cargo.toml files (generate per the dependency graph above)
- Vault's internal structure (lives in CashCache/Vault/, separate scaffold)
- FiREPLY's structure (lives in CashCache/FiREPLY/, separate scaffold)
- Strategy module internals (each has its own engineering spec)
- IBKR broker adapter implementation (depends on ibapi vs yatws audit)
- Blackbird subsystem (RAMjet/Chine/Spike -- not in scope until Payload tier)
- Taxi module spec (undefined)
- Greeks computation strategy (RustQuant offline vs quantrs hot-path -- benchmark first)
- Watchlist CSV schema (see research doc 05 for pattern)
- Vault-side deployment logic (Vault owns its own instrument allocation)
