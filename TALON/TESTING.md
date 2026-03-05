# TALON Testing Guide

## Test Count: 53

| Crate | Tests | Description |
|-------|-------|-------------|
| talon-util | 11 | SMA, RSI, Bollinger, RVOL, volume_declining indicators |
| talon-risk | 10 | RiskMesh (approve/reject/nosedive), StressEngine (tiers, overrides) |
| talon-types | 10 | Trust ledger (2), Sovereign (3), Portfolio (5) |
| talon-exec | 7 | Supervision routing (3), ExecCore pending/timeout/max (4) |
| talon-broker | 5 | MockBroker (2), RateLimiter (3) |
| talon-carousel | 5 | Harvest calculation, scaling, minimum, accumulation |
| talon-firebird | 3 | Signal generation, no-signal, intent fields |
| talon-db | 2 | Event append/replay, multiple events |

## Running Tests

```bash
# All tests
cargo test --workspace

# Single crate
cargo test -p talon-exec

# Release mode (matches production)
cargo test --workspace --release

# With output
cargo test --workspace -- --nocapture

# Specific test
cargo test -p talon-risk -- mesh::tests::approve_within_limits
```

## MockBroker Usage

The `MockBroker` in `talon-broker::mock` provides a full `BrokerCommands` + `BrokerStreams`
implementation for testing:

```rust
use talon_broker::mock::MockBroker;
use rust_decimal::Decimal;

let broker = MockBroker::new(Decimal::from(10_000)); // $10k starting balance

// Submit orders — adds to in-memory positions
let ack = broker.submit_order(&intent).unwrap();

// Check positions
let positions = broker.positions().unwrap();

// Account snapshot
let snap = broker.account_snapshot().unwrap();
assert_eq!(snap.net_liquidation, Decimal::from(10_000));
```

Stream subscriptions (`subscribe_quotes`, `subscribe_fills`, etc.) return no-op handles.

## Test Patterns

### Strategy Module Tests
Feed a series of `QuoteEvent` values to `on_quote()` and check `ScanResult`:
- Verify intents fire under correct conditions (RSI oversold + volume divergence)
- Verify no intents on rising prices
- Verify intent fields are correct (module, side, quantity, stops)

### ExecCore Tests
Test the supervision gate without a live broker:
- `PendingIntent::from_intent()` + `is_expired()` for timeout
- `ExecCore::enqueue_pending()` for max queue enforcement
- `ExecCore::expire_timed_out_intents()` for auto-rejection

### Portfolio Tests
Test position tracking and P&L calculation:
- `Portfolio::apply_fill()` for opening and closing positions
- Realized P&L computation on close (including commission)
- `PortfolioSnapshot` serialization
