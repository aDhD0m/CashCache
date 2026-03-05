# 01 -- AI Orchestration, CLAUDE.md, and Skill Schemas

Audited: 2026-03-03 | Source: Exhaustive Resource Registry, Section 1

---

## Audit Summary

| Resource | Status | TALON Relevance |
|----------|--------|-----------------|
| ruvnet/claude-flow CLAUDE-MD-Rust | VALID -- 14K+ stars, active | Medium -- batch-ops pattern useful for CI |
| rshah515/claude-code-subagents | VALID -- subagent routing examples | Low -- different orchestration model |
| wboayue/rust-ibapi CLAUDE.md | VALID -- ships with ibapi crate | HIGH -- direct IBKR integration config |
| actionbook/rust-skills CLAUDE.md | VALID -- ownership error patterns | Medium -- Arc<T> redesign guidance |
| wshobson gist (AI orchestrator prompts) | VALID -- identity prompt examples | Low -- generic orchestration |
| longbridge/longbridge-terminal CLAUDE.md | VALID -- production trading TUI | HIGH -- Bevy ECS + clippy::pedantic patterns |
| mcpmarket.com skill definitions | UNVERIFIABLE -- domain may not resolve | Low -- generic TUI skill schemas |
| massive.com/docs | UNVERIFIABLE -- generic domain | Low -- no specific Rust content confirmed |

---

## Resource Details

### 1. claude-flow CLAUDE-MD-Rust (ruvnet)

- **URL:** https://github.com/ruvnet/claude-flow/wiki/CLAUDE-MD-Rust
- **Files:** .md
- **Stars:** 14.1K+ | **Forks:** 1.7K+
- **License:** Check repo (not confirmed MIT)

**Core Rule -- "1 MESSAGE = ALL MEMORY-SAFE OPERATIONS":**

The central principle is that all Rust operations in a single Claude Code session
must be batched into one message to avoid sequential overhead. This applies to:

- Cargo build/test/run commands
- Crate dependency installations
- Test suite execution (cargo test)
- Borrowing/ownership pattern implementations
- Async/threading implementations

**Correct batch pattern:**

```
[Single Message]:
- TodoWrite { todos: [10+ todos with all Rust tasks] }
- Task("You are Rust architect. Coordinate via hooks for ownership design...")
- Task("You are Systems programmer. Coordinate via hooks for performance...")
- Task("You are Safety engineer. Coordinate via hooks for memory safety...")
- Bash("cargo new my-rust-app --bin")
- Bash("cd my-rust-app && cargo add serde tokio reqwest")
- Bash("cd my-rust-app && cargo add --dev proptest criterion")
- Write("Cargo.toml", cargoConfiguration)
- Write("src/main.rs", mainApplication)
- Write("src/lib.rs", libraryModule)
- Bash("cd my-rust-app && cargo build && cargo test && cargo run")
```

**Agent Roles for Rust Swarms:**

1. Systems Architect Agent -- memory management, ownership patterns
2. Performance Engineer Agent -- zero-cost abstractions, optimization
3. Safety Specialist Agent -- borrow checker, lifetime management
4. Concurrency Expert Agent -- async/await, threading, channels
5. Testing Agent -- unit, integration, property testing
6. Ecosystem Agent -- crate selection, FFI, WebAssembly

**Memory Safety Coordination Files:**

```
src/ownership/smart_pointers.rs
src/ownership/lifetimes.rs
src/ownership/borrowing.rs
src/memory/allocator.rs
src/safety/invariants.rs
tests/memory_safety.rs
```

Validate with: `cargo build && cargo miri test`

**TALON Application:** The batch-ops pattern applies to CI pipelines and Claude Code
development sessions. For TALON's multi-module build (Firebird, Thunderbird, SAGE,
etc.), batching all module compilations and test runs into single commands reduces
wall-clock time.

---

### 2. wboayue/rust-ibapi CLAUDE.md

- **URL:** https://github.com/wboayue/rust-ibapi/blob/main/CLAUDE.md
- **Files:** .md, .toml
- **Crate:** ibapi v2.2.2 (Nov 2025) -- 238 stars, 57 forks

**IBKR Connection Ports (hardcoded in CLAUDE.md):**

| Mode | Application | Port |
|------|-------------|------|
| Live | TWS | 7496 |
| Paper | TWS | 7497 |
| Live | IB Gateway | 4001 |
| Paper | IB Gateway | 4002 |

**Critical:** Always use `127.0.0.1` not `localhost`. Some systems resolve localhost
to IPv6, which TWS blocks. TWS only allows specifying IPv4 addresses in the
allowed IP list.

**Connection Examples (both async and blocking):**

Blocking client:
```rust
use ibapi::client::blocking::Client;
use ibapi::prelude::*;

fn main() {
    let connection_url = "127.0.0.1:4002";
    let client = Client::connect(connection_url, 100)
        .expect("connection to TWS failed!");
    println!("Successfully connected to TWS at {connection_url}");
}
```

Async client:
```rust
use ibapi::prelude::*;

#[tokio::main]
async fn main() {
    let connection_url = "127.0.0.1:4002";
    let client = Client::connect(connection_url, 100)
        .await
        .expect("connection to TWS failed!");
    println!("Successfully connected to TWS at {connection_url}");
}
```

**Contract Builder API (type-safe):**

```rust
// Stock -- defaults to USD, SMART routing
let contract = Contract::stock("TSLA").build();

// Stock with custom exchange
let contract = Contract::stock("7203")
    .on_exchange("TSEJ")
    .in_currency("JPY")
    .build();

// Options -- required fields enforced at compile time
let option = Contract::call("AAPL")
    .strike(150.0)
    .expires_on(2024, 12, 20)
    .build();

// Futures -- front month convenience
let futures = Contract::futures("ES")
    .front_month()
    .build();

// Forex
let forex = Contract::forex("EUR", "USD").build();

// Bonds via CUSIP/ISIN
let treasury = Contract::bond_cusip("912810RN0");
let euro_bond = Contract::bond_isin("DE0001102309");
```

**Order Submission (fluent API):**

```rust
// Market buy
let order_id = client.order(&contract)
    .buy(100)
    .market()
    .submit()
    .expect("order submission failed!");

// Limit sell with GTC
let order_id = client.order(&contract)
    .sell(50)
    .limit(150.00)
    .good_till_cancel()
    .outside_rth()
    .submit()
    .expect("order submission failed!");
```

**Algo Order Builders (v2.2+):**

```rust
use ibapi::orders::builder::vwap;

let order_id = client.order(&contract)
    .buy(1000)
    .limit(150.0)
    .algo(vwap()
        .max_pct_vol(0.2)
        .start_time("09:00:00 US/Eastern")
        .end_time("16:00:00 US/Eastern")
        .build()?)
    .submit()?;
```

**Startup Message Callback (capture orders on connect):**

```rust
use ibapi::{Client, StartupMessageCallback};
use ibapi::messages::IncomingMessages;
use std::sync::{Arc, Mutex};

let orders = Arc::new(Mutex::new(Vec::new()));
let orders_clone = orders.clone();

let callback: StartupMessageCallback = Box::new(move |msg| {
    match msg.message_type() {
        IncomingMessages::OpenOrder | IncomingMessages::OrderStatus => {
            orders_clone.lock().unwrap().push(msg);
        }
        _ => {}
    }
});

let client = Client::connect_with_callback(
    "127.0.0.1:4002", 100, Some(callback)
).await.expect("connection failed");
```

**Auto-Reconnect:** Fibonacci backoff, up to 30 retries on disconnect.

**Docker Gateway (same author):**

```bash
# Paper trading
docker run --env-file credentials.env \
    -e TRADING_MODE=paper \
    -p 5900:5900 -p 4002:4002 \
    wboayue/ib-gateway:latest

# Live trading
docker run --env-file credentials.env \
    -p 5900:5900 -p 4001:4001 \
    wboayue/ib-gateway:latest
```

Gateway v10.20, IBC v3.16.0. VNC on port 5900 (no password -- assumes secure network).

credentials.env format:
```
TWS_USERID=your_username
TWS_PASSWORD=your_password
TRADING_MODE=paper
```

---

### 3. longbridge/longbridge-terminal CLAUDE.md

- **URL:** https://github.com/longbridge/longbridge-terminal/blob/main/CLAUDE.md
- **Files:** .rs, .yml, .toml

**Production Trading Terminal Constraints:**

- clippy::pedantic rules enforced project-wide
- rust-i18n localization with locale files in locales/*.yml
- Bevy ECS architecture (app.rs as entry point)
- DashMap for concurrent stock data caching (stock.rs)
- Longport OpenAPI for broker integration

**TALON Application:** The clippy::pedantic + Bevy ECS pattern is directly
comparable to TALON's Ratatui TUI architecture. The DashMap caching pattern
is relevant for real-time quote storage in TALON modules.

---

### 4. actionbook/rust-skills CLAUDE.md

- **URL:** https://github.com/actionbook/rust-skills/blob/main/CLAUDE.md
- **Files:** .md

**Arc<T> Redesign Guidance:**

Layer-by-layer reasoning instructions for resolving ownership errors.
The pattern: when the borrow checker rejects a design, don't fight it with
unsafe -- redesign the data flow using Arc<T>, channels, or interior mutability.

**TALON Application:** Directly relevant to the BrokerSessionManager pattern
where `HashMap<BrokerId, (Box<dyn BrokerCommands>, Box<dyn BrokerStreams>)>`
needs shared access across modules.

---

### 5. claude-code-subagents (rshah515)

- **URL:** https://github.com/rshah515/claude-code-subagents/blob/main/CLAUDE.md
- **Files:** .md

Contains examples of subagent routing files including
`trading-platform-expert.md` for delegating specialized financial logic
to domain-specific AI agents. The routing pattern separates concerns:
one agent handles order execution logic, another handles risk management,
another handles UI rendering.

---

## Dead/Unverifiable Resources

- **mcpmarket.com** -- Domain for MCP skill marketplace. Two URLs listed for
  TUI development skills. Content may exist but cannot be independently verified.
  The skill schemas described (layout constraint logic for Ratatui, integration
  patterns for clap/inquire/ratatui) would be useful if accessible.

- **massive.com/docs** -- Listed as "foundational architectural documentation."
  This appears to be a generic domain unrelated to Rust trading systems.
  Likely a placeholder or incorrect URL.

- **wshobson gist** -- Generic AI orchestrator identity prompts. Low value
  for TALON-specific work.
