# TALON Runbook — Hatch Tier Operations

## Section 1: Pre-flight Checklist

1. Verify IB Gateway is running: check TWS or IB Gateway window is open
2. Verify Gateway port: default 4002 for paper, 4001 for live
3. Verify client ID 333 is not in use by another connection
4. Verify `.env` file exists with `POLYGON_API_KEY` (optional) and IBKR settings
5. Verify `config/talon.toml` watchlist matches desired symbols
6. Verify `data/` directory exists (created automatically)
7. Run `cargo test --workspace --release` — all 53 tests must pass
8. Verify market hours if testing live data (pre-market 4:00 ET, regular 9:30-16:00 ET)

## Section 2: Normal Operation

### Start
```bash
cargo run --release --bin talon -- --config config/talon.toml
```

Or from the built binary:
```bash
./target/release/talon --config config/talon.toml
```

### Navigation
- `1-5` or `Tab`: switch between Cockpit/Portfolio/Scanner/Flow/Log tabs
- `a`: approve selected pending intent
- `r`: reject selected pending intent
- `A` (Shift+A): approve ALL pending intents
- `R` (Shift+R): reject ALL pending intents
- `Up/Down`: navigate pending intents list
- `Space`: approve (legacy), `Esc`: reject (legacy)
- `h`: toggle entry halt (stops new intents)
- `k`: EJECT — close all positions
- `Ctrl+X`: emergency shutdown
- `q` or `Ctrl+C`: graceful quit

### Shutdown
- `q` for graceful shutdown (flushes EventStore, checkpoints SQLite)
- `Ctrl+C` for graceful shutdown (same as q)
- `Ctrl+X` for emergency shutdown (immediate exit)

### Log Locations
- `data/talon.log` — main application log (file appender, no ANSI)
- `data/events.db` — SQLite WAL event store (events + portfolio snapshots)

## Section 3: Troubleshooting

### Failure Mode 1: IBKR Connection Refused
**Symptom:** `IBKR connect failed after 4 retries`
**Cause:** IB Gateway not running or wrong port
**Fix:**
1. Verify IB Gateway is running
2. Check port matches `config/talon.toml` `[broker.ibkr] port`
3. Check no other TALON instance using client ID 333

### Failure Mode 2: Client ID In Use (Error 326)
**Symptom:** `Client ID in use` in logs
**Fix:** Change `client_id` in `.env` or `config/talon.toml` to unused value

### Failure Mode 3: Rate Limited (Error 100)
**Symptom:** `Max rate of messages per second exceeded`
**Fix:** RateLimiter caps at 50 msgs/sec. If triggered, reduce scan frequency.

### Failure Mode 4: Polygon 429
**Symptom:** `Polygon rate limited` in logs
**Fix:** Increase `poll_interval_seconds` in talon.toml. Free tier: 5 req/min.
Yahoo Finance fallback activates automatically.

### Failure Mode 5: TUI Rendering Issues
**Symptom:** Display corruption, overlapping widgets
**Fix:** Ensure terminal is at least 80x24. Resize triggers automatic redraw.

### Failure Mode 6: SQLite Busy
**Symptom:** `database is locked` errors
**Fix:** Only one TALON instance should access events.db. WAL mode + busy_timeout=5000ms
handles most contention.

### Failure Mode 7: Supervision Timeout
**Symptom:** Intents disappearing from PendingIntents after 10s
**Expected behavior:** This is correct. Unapproved intents auto-reject at 10s.
**Fix:** Approve or reject faster, or increase `[supervision] timeout_secs`.

### Failure Mode 8: Memory Growth
**Symptom:** RSS growing beyond 500MB over extended runs
**Diagnosis:** Check channel buffer buildup with `tokio-console`.
Common causes: unprocessed quote events, AppState clone accumulation.
**Fix:** Restart. If persistent, check for broadcast channel subscriber leaks.

### Failure Mode 9: Order Rejected (Error 201)
**Symptom:** Orders submitted but rejected by IBKR
**Fix:** Check TWS account permissions. Paper trading must have equity order permissions enabled.

## Section 4: Monitoring

### metrics.json Fields
- `intent_received_total` — total intents received from modules
- `intent_approved_total` — intents approved (operator or auto)
- `intent_rejected_total` — intents rejected (risk, operator, or timeout)
- `intent_timeout_total` — intents auto-rejected by timeout
- `intent_auto_executed_total` — intents auto-executed (Hatch modules)
- `fill_received_total` — fills received from broker

### talon.db Queries
```sql
-- Recent events
SELECT ts, kind FROM events ORDER BY id DESC LIMIT 20;

-- Portfolio snapshots
SELECT ts, payload FROM portfolio_snapshots ORDER BY id DESC LIMIT 1;

-- Harvest events
SELECT ts, symbol, harvest_amount FROM harvest_events ORDER BY id DESC LIMIT 10;

-- Event count by type
SELECT kind, COUNT(*) FROM events GROUP BY kind ORDER BY COUNT(*) DESC;
```

### TUI Widget Interpretation
- **Stress Tier**: x1.0 (normal), x0.75 (tier1), x0.50 (tier2), FLAMEOUT x0.25, NOSEDIVE x0.0
- **PendingIntents Age**: white (<5s), yellow (>5s warning), auto-reject at 10s
- **Fills P&L**: cyan = profit, yellow = loss
- **Approaching**: sorted by distance to trigger, arrows show heating/cooling

## Section 5: Emergency Procedures

### Graceful Shutdown
```bash
# From TUI
Press 'q' or Ctrl+C

# From terminal
kill -TERM $(pgrep talon)
```
EventStore flushes, SQLite checkpoints, terminal restores.

### Force Kill (if hung)
```bash
kill -9 $(pgrep talon)
```
May leave SQLite WAL unclean. Next startup auto-recovers.

### Manual Order Cancellation
If TALON crashes with open orders:
1. Open TWS/IB Gateway
2. Navigate to Orders panel
3. Right-click → Cancel All
4. Verify in Trades panel that no fills arrived after crash

### Position Eject
Press `k` in TUI to close all positions at market.
This sends cancel-all + market close orders for every open position.
