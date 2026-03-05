# 02 -- Algorithmic Trading Engines and Broker Integrations

Audited: 2026-03-03 | Source: Exhaustive Resource Registry, Section 2

---

## Audit Summary

| Resource | Status | TALON Relevance |
|----------|--------|-----------------|
| drpngx/yatws | VALID -- v0.1.7, production claims | HIGH -- rate limiter, session replay, options builders |
| barter-rs/barter-rs | VALID -- MIT, active ecosystem | HIGH -- engine architecture reference |
| NautilusTrader | VALID -- Rust core + Python bindings | Medium -- reference architecture only |
| algotrading crate | VALID on crates.io | Medium -- SIMD quant primitives |
| databento Rust client | VALID -- DBN format | Medium -- historical data replay |
| bloomberg crate | VALID on crates.io | Low -- requires Bloomberg Terminal |
| hedge0/trading_bot_rust | VALID -- IBKR options bot | HIGH -- Calendar/Butterfly spread impl |
| austin-starks/NextTrade | VALID -- TS-to-Rust migration | Low -- migration case study |
| ThePredictiveDev trading system | VALID -- FIX protocol | Low -- different protocol than TWS |

---

## Resource Details

### 1. yatws -- Yet Another TWS API (drpngx)

- **URL:** https://github.com/drpngx/yatws
- **Crate:** yatws v0.1.7
- **License:** Verify before use (not confirmed in search results)
- **Production claims:** 9 figures of dollar volume traded

**Architecture:** Manager-based, not the traditional EClient/EWrapper pattern.
Each functional domain (orders, account, market data, FA) has its own manager
accessed via the client instance.

**Key Differentiators from ibapi:**

1. Built-in rate limiting (ibapi has none)
2. Session recording/replay via SQLite
3. OptionsStrategyBuilder with pre-built spreads
4. Observer pattern split by functionality
5. ~3ms order placement latency
6. Financial Advisor (FA) manager for multi-account

**Rate Limiter:**

```rust
// Enable with defaults (50 msgs/sec, 50 historical, 100 market data lines)
client.enable_rate_limiting()?;

// Custom configuration
let mut config = RateLimiterConfig::default();
config.enabled = true;
config.max_messages_per_second = 40;
config.max_historical_requests = 30;
config.rate_limit_wait_timeout = Duration::from_secs(10);
client.configure_rate_limiter(config)?;

// Monitor status
if let Some(status) = client.get_rate_limiter_status() {
    println!("Current message rate: {:.2} msgs/sec", status.current_message_rate);
    println!("Active historical: {}/{}",
        status.active_historical_requests,
        config.max_historical_requests);
}

// Cleanup stale requests (long-running apps)
let (hist_cleaned, mkt_cleaned) = client.cleanup_stale_rate_limiter_requests(
    Duration::from_secs(300)
)?;
```

**Session Recording/Replay (testing without live gateway):**

```rust
// Record a session
let client = IBKRClient::new(
    "127.0.0.1", 7497, 0,
    Some(("sessions.db", "my_trading_session"))
)?;

// Replay the session (offline testing)
let replay_client = IBKRClient::from_db(
    "sessions.db", "my_trading_session"
)?;
```

**Options Strategy Builder:**

```rust
let builder = OptionsStrategyBuilder::new(
    client.data_ref(),
    "AAPL",
    150.0,  // Current price
    10.0,   // Quantity (10 spreads)
    SecType::Stock
)?;

let (contract, order) = builder.bull_call_spread(
    NaiveDate::from_ymd_opt(2025, 12, 19).unwrap(),
    150.0,  // Lower strike
    160.0   // Higher strike
)?;
```

**Conditional Orders:**

```rust
let (contract, order) = OrderBuilder::new(OrderSide::Buy, 100.0)
    .for_stock("AAPL")
    .limit(150.0)
    .add_price_condition(
        265598,             // SPY con_id
        "ISLAND",           // Exchange
        400.0,              // Price
        TriggerMethod::Last,
        false               // Is less than 400
    )
    .build()?;
```

**Account Management:**

```rust
// Subscribe to account updates
client.account().subscribe_account_updates()?;

// Get account summary
let info = client.account().get_account_info()?;
println!("Account Net Liq: {}", info.net_liquidation);

// Get specific value
let bp = client.account().get_account_value(AccountValueKey::BuyingPower)?;

// List positions
let positions = client.account().list_open_positions()?;

// Today's executions
let executions = client.account().get_day_executions()?;

// Pre-liquidation warning check
if client.account().has_received_pre_liquidation_warning() {
    println!("WARNING: Pre-liquidation warning received");
}
```

**Error Handling:**

```rust
match client.account().get_net_liquidation() {
    Ok(net_liq_value) => println!("Net Liq: ${}", net_liq_value),
    Err(IBKRError::Timeout) => println!("Operation timed out"),
    Err(IBKRError::ApiError(code, msg)) => println!("API error {}: {}", code, msg),
    Err(e) => println!("Other error: {:?}", e),
}
```

**Observer Pattern (event-driven, closest to official TWS API):**

```rust
use yatws::{IBKRClient, IBKRError, contract::Contract,
    data::{MarketDataType, TickType, TickAttrib},
    data_observer::MarketDataObserver};

struct MyMarketObserver {
    name: String,
    tick_count: Arc<Mutex<usize>>,
}

impl MarketDataObserver for MyMarketObserver {
    fn on_tick_price(&self, req_id: i32, tick_type: TickType,
                     price: f64, attrib: TickAttrib) {
        println!("[{}] TickPrice: ReqID={}, Type={:?}, Price={}",
            self.name, req_id, tick_type, price);
        *self.tick_count.lock().unwrap() += 1;
    }

    fn on_error(&self, req_id: i32, error_code: i32, error_message: &str) {
        eprintln!("[{}] Error: ReqID={}, Code={}, Msg='{}'",
            self.name, req_id, error_code, error_message);
    }
}
```

**Financial Advisor (multi-account):**

```rust
let fa_manager = client.financial_advisor();
fa_manager.request_fa_data(yatws::FADataType::Groups)?;
let fa_config = fa_manager.get_config();
println!("FA Groups: {:?}", fa_config.groups);
```

**TALON Application:** yatws's rate limiter design should be studied for
TALON's own rate limiting layer. The session replay is invaluable for
testing modules without a live gateway. The OptionsStrategyBuilder pattern
maps directly to SAGE and Range R0LEx spread construction. The pre-liquidation
warning check maps to TALON's Flameout/Nosedive state transitions.

**WARNING:** v0.1 API will break. License unconfirmed. Do not take a hard
dependency -- extract patterns and ideas.

---

### 2. Barter Ecosystem (barter-rs)

- **URL:** https://github.com/barter-rs/barter-rs
- **Crate:** barter (workspace of sub-crates)
- **License:** MIT
- **Status:** Active, educational disclaimer (not production-certified)

**Sub-crate Architecture:**

| Crate | Purpose |
|-------|---------|
| barter | Core engine -- SystemBuilder, Strategy, RiskManager |
| barter-data | WebSocket market data streams (Binance, Coinbase, OKX, Bybit, etc.) |
| barter-execution | Order execution (live or mock) |
| barter-integration | Low-level protocol adapters (WS, FIX, HTTP) |
| barter-instrument | Instrument/contract type definitions |

**Engine Architecture (reference for TALON):**

```rust
const FILE_PATH_SYSTEM_CONFIG: &str =
    "barter/examples/config/system_config.json";
const RISK_FREE_RETURN: Decimal = dec!(0.05);

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();

    let SystemConfig { instruments, executions } = load_config()?;
    let instruments = IndexedInstruments::new(instruments);

    let market_stream = init_indexed_multi_exchange_market_stream(
        &instruments,
        &[SubKind::PublicTrades, SubKind::OrderBooksL1],
    ).await?;

    let args = SystemArgs::new(
        &instruments,
        executions,
        LiveClock,
        DefaultStrategy::default(),
        DefaultRiskManager::default(),
        market_stream,
    );

    let mut system = SystemBuilder::new(args)
        .engine_feed_mode(EngineFeedMode::Iterator)
        .trading_state(TradingState::Disabled)
        .build()?;

    system.run().await?;
    Ok(())
}
```

**Key Design Patterns:**

- O(1) state lookups via indexed data structures (not HashMap)
- Plug-and-play Strategy/RiskManager traits
- TradingState enum for external on/off control (maps to TALON's Cruising Altitude / EJECT states)
- EngineState replica for non-hot-path monitoring (UI, Telegram)
- Engine Commands from external process: CloseAllPositions, OpenOrders, CancelOrders
- Near-identical live/backtest system (swap MarketStream and Execution impls)

**Trait Architecture:**

- `MarketGenerator` -- heartbeat via market events
- `SignalGenerator` -- Strategy produces advisory Signals
- `MarketUpdater` / `OrderGenerator` / `FillUpdater` -- Portfolio state machine
- `ExecutionClient` -- generates FillEvents from OrderEvents

**TALON Application:**

- The SystemBuilder pattern maps to TALON's module initialization
- TradingState::Disabled/Enabled maps to Cruising Altitude / EJECT
- The EngineState replica pattern solves the TUI rendering problem -- hot path stays clean
- Engine Commands (CloseAllPositions) map to TALON's atomic kill switches
- Barter is crypto-focused (Binance, Coinbase, etc.) -- NOT equity/options. It cannot execute IBKR orders. But the architecture is a valuable reference.

**WARNING:** Barter carries an explicit educational disclaimer. It is NOT
production-certified. Extract architecture patterns only.

---

### 3. hedge0/trading_bot_rust -- IBKR Options Bot

- **URL:** https://github.com/hedge0/trading_bot_rust
- **Files:** .rs, .env

Implements Calendar and Butterfly spreads on IBKR. Uses .env for credentials.

**TALON Application:** Direct reference implementation for multi-leg option
strategies. Verify: does it submit spreads atomically (single combo order)
or sequentially (dangerous)? If sequential, the code is a risk model, not
a production reference.

---

### 4. NautilusTrader

- **URL:** https://nautilustrader.io/
- **Files:** .rs, .py, .md
- **Architecture:** High-performance Rust core with Python bindings

IBKR integration docs confirm standard port configuration and provide
error code reference:

| Code | Meaning |
|------|---------|
| 200 | No security definition found |
| 201 | Order rejected |
| 202 | Order cancelled |
| 300 | Can't find EId with ticker ID |
| 354 | Market data not subscribed |
| 2104 | Market data farm connection OK |
| 2106 | HMDS data farm connection OK |

**TALON Application:** Error code table is directly useful for IBKR
error handling in TALON's broker adapter layer.

---

### 5. Databento Rust Client

- **URL:** https://databento.com/blog/rust-client-library
- **Files:** .rs
- **Performance:** 19 million events/sec replay via DBN format

**TALON Application:** If TALON needs historical order book replay for
backtesting (particularly for SAGE gamma scalping validation), Databento's
DBN format and replay speed are relevant. Paid service.

---

### 6. algotrading Crate

- **URL:** https://crates.io/crates/algotrading
- **Files:** .rs, .toml
- **Features:** SIMD-accelerated operations, zero-cost const-generic data structures

**TALON Application:** Potential dependency for quantitative computations
in signal generation. Verify: does it cover options Greeks, or just
equity indicators?

---

### 7. bloomberg Crate

- **URL:** https://crates.io/crates/bloomberg
- **Files:** .rs, .toml

Rust wrapper for Bloomberg API. Requires Bloomberg Terminal subscription.

**TALON Application:** Out of scope for retail. Only relevant if TALON
ever targets institutional Bloomberg users.

---

### 8. Lower-Priority Resources

**NextTrade (austin-starks):** TypeScript-to-Rust migration case study.
Useful as a "what not to do" reference for architecture decisions, not
as a code dependency.

**ThePredictiveDev Automated Trading System:** FIX protocol, market
microstructure, order book dynamics. Different protocol surface than
TWS TCP. Low relevance unless TALON adds FIX support (CenterPoint/Clear
Street integration).
