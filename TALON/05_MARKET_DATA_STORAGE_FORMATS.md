# 05 -- Market Data Storage, Formats, and Generation

Audited: 2026-03-03 | Source: Exhaustive Resource Registry, Section 5

---

## Audit Summary

| Resource | Status | TALON Relevance |
|----------|--------|-----------------|
| piekstra/market-data | VALID -- Parquet OHLCV storage | HIGH -- data pipeline reference |
| danielbeach/datahobbit | VALID -- synthetic data CLI | Medium -- test data generation |
| market-data-source crate | VALID on crates.io | Medium -- streaming synthetic data |
| michaeljwright/robobull-trading-bot | VALID -- Alpaca integration | Low -- different broker |
| marketcalls/openalgo | VALID -- MCP + CSV symbols | Low -- Python-centric |

---

## Resource Details

### 1. piekstra/market-data -- Parquet OHLCV Storage

- **URL:** https://github.com/piekstra/market-data
- **Files:** .parquet, .rs
- **Key Design:** Idempotent population, exact decimal strings

**Why Parquet for Market Data:**

Parquet is columnar, compressed, and supports predicate pushdown. For
OHLCV time series:

| Format | 1M bars file size | Column scan time | Random row access |
|--------|-------------------|------------------|-------------------|
| CSV | ~80 MB | Slow (full scan) | Impossible |
| JSON | ~120 MB | Slow (parse all) | Impossible |
| Parquet | ~15 MB | Fast (column skip) | Via row groups |
| SQLite | ~60 MB | Medium (B-tree) | Fast (indexed) |

Parquet wins on storage and columnar queries (e.g., "give me all close
prices for 2024"). SQLite wins on random access and transactional writes.

**Idempotent Population Pattern:**

The key design principle: inserting the same data twice produces the same
result. This prevents duplicate bars when re-running data pipelines after
failures.

```rust
// Conceptual pattern from piekstra/market-data
// Use (symbol, timestamp) as the natural key
// On conflict, overwrite (idempotent)

struct OhlcvBar {
    symbol: String,
    timestamp: i64,     // Unix epoch seconds
    open: String,       // Exact decimal string, NOT f64
    high: String,
    low: String,
    close: String,
    volume: String,
}
```

**Critical: Exact Decimal Strings**

This repo stores prices as strings, not f64. Why:

```
f64: 150.05 -> stored as 150.04999999999998 (IEEE 754 rounding)
String: "150.05" -> stored as "150.05" (exact)
```

For trading systems, f64 price representation causes:
- Incorrect P&L calculations (penny rounding errors accumulate)
- Failed order price matching (limit at 150.05 vs 150.04999...)
- Audit trail discrepancies

**TALON Application:**

TALON should store all prices as `rust_decimal::Decimal` in memory and
`String` in Parquet/persistence. Never use f64 for money.

```rust
use rust_decimal::Decimal;
use rust_decimal_macros::dec;

let price = dec!(150.05);      // Exact
let shares = dec!(100);
let value = price * shares;    // dec!(15005.00) -- exact
```

The Barter ecosystem already uses `rust_decimal::Decimal` throughout.

**Parquet Write Pattern (using arrow-rs):**

```rust
use arrow::array::{StringArray, Int64Array};
use arrow::datatypes::{Schema, Field, DataType};
use arrow::record_batch::RecordBatch;
use parquet::arrow::ArrowWriter;
use std::fs::File;
use std::sync::Arc;

let schema = Schema::new(vec![
    Field::new("symbol", DataType::Utf8, false),
    Field::new("timestamp", DataType::Int64, false),
    Field::new("open", DataType::Utf8, false),
    Field::new("high", DataType::Utf8, false),
    Field::new("low", DataType::Utf8, false),
    Field::new("close", DataType::Utf8, false),
    Field::new("volume", DataType::Utf8, false),
]);

let batch = RecordBatch::try_new(
    Arc::new(schema.clone()),
    vec![
        Arc::new(StringArray::from(vec!["AAPL", "AAPL"])),
        Arc::new(Int64Array::from(vec![1709510400, 1709510460])),
        Arc::new(StringArray::from(vec!["150.05", "150.10"])),
        Arc::new(StringArray::from(vec!["150.20", "150.15"])),
        Arc::new(StringArray::from(vec!["149.95", "150.00"])),
        Arc::new(StringArray::from(vec!["150.10", "150.12"])),
        Arc::new(StringArray::from(vec!["1000000", "850000"])),
    ],
)?;

let file = File::create("ohlcv.parquet")?;
let mut writer = ArrowWriter::try_new(file, Arc::new(schema), None)?;
writer.write(&batch)?;
writer.close()?;
```

**Parquet Read Pattern:**

```rust
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;
use std::fs::File;

let file = File::open("ohlcv.parquet")?;
let builder = ParquetRecordBatchReaderBuilder::try_new(file)?;

// Predicate pushdown: only read "close" column
let reader = builder
    .with_projection(parquet::arrow::ProjectionMask::columns(
        builder.parquet_schema(), &["close"]
    ))
    .build()?;

for batch in reader {
    let batch = batch?;
    // Process batch...
}
```

---

### 2. datahobbit -- Synthetic Data Generation CLI

- **URL:** https://github.com/danielbeach/datahobbit
- **Files:** .csv, .parquet, .json

CLI tool that generates synthetic data based on JSON schema definitions.
Outputs to CSV, Parquet, or JSON.

**TALON Application:**

Use datahobbit to generate test data for TALON's TUI widgets during
development. Define a schema matching TALON's OHLCV format:

```json
{
  "schema": {
    "symbol": {"type": "string", "values": ["AAPL", "TSLA", "SPY"]},
    "timestamp": {"type": "integer", "min": 1709510400, "max": 1709596800},
    "open": {"type": "decimal", "min": 140.0, "max": 160.0, "precision": 2},
    "high": {"type": "decimal", "min": 140.0, "max": 165.0, "precision": 2},
    "low": {"type": "decimal", "min": 135.0, "max": 160.0, "precision": 2},
    "close": {"type": "decimal", "min": 140.0, "max": 160.0, "precision": 2},
    "volume": {"type": "integer", "min": 100000, "max": 5000000}
  },
  "rows": 100000,
  "output": "parquet"
}
```

This ensures TUI widgets always have data during development. No dead
screens -- every widget must have a verified data source wired on_mount.

---

### 3. market-data-source Crate

- **URL:** https://crates.io/crates/market-data-source
- **Files:** .rs, .csv, .json

Generates realistic synthetic market data using random walk with drift.
Supports streaming for large datasets.

**Random Walk with Drift Model:**

```
P(t+1) = P(t) * exp(mu * dt + sigma * sqrt(dt) * Z)

where:
  mu    = drift (annualized return, e.g., 0.08 for 8%)
  sigma = volatility (annualized, e.g., 0.25 for 25%)
  dt    = time step (e.g., 1/252 for daily)
  Z     = standard normal random variable
```

This produces price paths that look realistic (trending, volatile,
mean-reverting around the drift) without requiring historical data
downloads.

**TALON Application:**

For module development and testing:
- Firebird: Generate oversold scenarios (high negative drift + spike)
- Thunderbird: Generate overextension scenarios (high positive drift)
- SAGE: Generate oscillating gamma exposure scenarios
- ParaShort: Generate parabolic runups (high positive drift + low vol)

The streaming support means you can simulate a live data feed for
integration testing without IBKR connectivity.

---

### 4. robobull-trading-bot -- Alpaca Integration

- **URL:** https://github.com/michaeljwright/robobull-trading-bot
- **Files:** .rs, .json, .txt

Alpaca-integrated bot using settings.json for live/paper configuration
and .env for API keys.

**Configuration Pattern:**

```json
{
  "mode": "paper",
  "symbols": ["AAPL", "TSLA", "SPY"],
  "max_position_size": 1000,
  "stop_loss_pct": 2.0,
  "take_profit_pct": 5.0
}
```

```
# .env
APCA_API_KEY_ID=your_key
APCA_API_SECRET_KEY=your_secret
APCA_API_BASE_URL=https://paper-api.alpaca.markets
```

**TALON Application:** Low direct relevance (different broker), but
the configuration pattern (JSON for strategy params, .env for credentials)
is a clean separation of concerns. TALON should follow a similar pattern:
- `talon_config.toml` for module parameters and thresholds
- `.env` for IBKR credentials and port configuration
- Never commit credentials to version control

---

### 5. openalgo -- MCP + CSV Symbol Management

- **URL:** https://github.com/marketcalls/openalgo
- **Files:** .rs, .csv, .py

Platform integrating MCP servers. Uses CSV for bulk symbol configuration
and hosts isolated Python strategies.

**Bulk Symbol Configuration Pattern:**

```csv
symbol,exchange,sec_type,currency,enabled
AAPL,SMART,STK,USD,true
TSLA,SMART,STK,USD,true
SPY,SMART,STK,USD,true
QQQ,SMART,STK,USD,true
AAPL,SMART,OPT,USD,false
```

**TALON Application:** The CSV symbol configuration pattern is useful
for TALON's watchlist management. A CSV file is version-controllable,
diff-able, and trivially parseable:

```rust
use csv::Reader;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct WatchlistEntry {
    symbol: String,
    exchange: String,
    sec_type: String,
    currency: String,
    enabled: bool,
}

fn load_watchlist(path: &str) -> Vec<WatchlistEntry> {
    let mut rdr = Reader::from_path(path).expect("watchlist file not found");
    rdr.deserialize()
        .filter_map(|r| r.ok())
        .filter(|e: &WatchlistEntry| e.enabled)
        .collect()
}
```

---

## Data Pipeline Architecture for TALON

Based on the audited resources, the recommended data flow:

```
Live Data (IBKR TWS via ibapi)
    |
    v
DashMap<Symbol, QuoteData>  <-- concurrent cache (from longbridge pattern)
    |
    +---> TUI Renderer (10fps poll)
    |
    +---> Signal Generator (module-specific)
    |
    v
Parquet Archive (end-of-day flush)
    |
    v
Historical Backtest (offline, arrow-rs reader)
```

**Development/Testing Data Flow:**

```
Synthetic Generator (market-data-source crate OR datahobbit)
    |
    v
Mock DashMap<Symbol, QuoteData>
    |
    +---> TUI Renderer (same code path as live)
    |
    +---> Signal Generator (same code path as live)
```

The critical rule: the TUI renderer and signal generator NEVER know
whether they are reading live or synthetic data. The DashMap interface
is identical. This is the "no dead screens" rule implemented at the
architecture level.

---

## Crate Dependencies for Data Layer

```toml
[dependencies]
# Exact decimal arithmetic (never use f64 for money)
rust_decimal = "1"
rust_decimal_macros = "1"

# Parquet I/O
arrow = { version = "53", features = ["prettyprint"] }
parquet = { version = "53", features = ["arrow"] }

# CSV parsing
csv = "1"
serde = { version = "1", features = ["derive"] }

# Concurrent cache
dashmap = "6"

# Synthetic data (dev only)
rand = "0.8"
rand_distr = "0.4"
```
