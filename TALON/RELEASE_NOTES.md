# TALON v3.2.0-hatch-complete — Release Notes

## Completed Features

### Signal-to-Fill Pipeline
- Full intent lifecycle: module scan → risk evaluation → supervision gate → broker submission → fill processing
- 3 active strategy modules (Firebird, Thunderbird, Taxi) generating OrderIntents from market data
- RiskMesh evaluation with stress-adjusted limits (nosedive gate, position limits, exposure caps)

### ExecCore Supervision Gate
- Supervision routing by module tier (SupervisedAutonomy/DualControl/DualControlStrict)
- Pending intent queue (max 10 intents, 10s timeout auto-rejection)
- TUI operator approval flow with 'a'/'r'/Up/Down/'A'/'R' keybindings
- Metrics emission (approved/rejected/timeout/auto-executed counts)

### TRiMiNL Operator Terminal
- 5-tab interface: Cockpit, Portfolio, Scanner, Flow, Log
- PendingIntents widget with 7 columns and age highlighting (>5s yellow)
- Fills widget with P&L color-coding (cyan=profit, yellow=loss)
- Flow tab: L2 DOM ladder, T&S tape with Lee-Ready classification, volume profile, delta sparkline
- Splash sequence on startup
- Event-driven rendering (zero CPU when idle)

### Portfolio Resource
- Position tracking with real-time P&L computation
- Fill recording with realized P&L on close (including commission)
- PortfolioSnapshot persistence to SQLite every 5s
- Crash recovery from latest snapshot on restart

### Carousel Profit Harvest
- Harvest calculation: 15% of realized P&L (configurable)
- Inverse account size scaling (rate decreases at larger NLV, 5% floor)
- Minimum harvest threshold ($1.00)
- Logging only at Hatch tier (harvest_enabled=false, no actual transfers)

### Broker Infrastructure
- IBKR Gateway adapter with ibapi v2 (full BrokerCommands + BrokerStreams)
- Exponential backoff retry on connection (1s, 2s, 4s, 8s max, 4 retries)
- Rate limiter (50 msgs/sec sliding window)
- 34 IBKR error codes mapped with descriptions and transient/fatal classification
- MockBroker for testing

### Market Data
- Polygon.io REST snapshot polling (optional, keyed on POLYGON_API_KEY)
- Yahoo Finance fallback (always-on, 30s interval)
- IBKR real-time bars (5s interval)

### Persistence
- SQLite WAL event store on dedicated OS thread (never blocks async)
- Portfolio snapshots table with crash recovery
- Harvest events table for audit trail

## Test Suite: 53 Tests
- talon-util: 11 (indicators)
- talon-risk: 10 (mesh + stress engine)
- talon-types: 10 (trust + sovereign + portfolio)
- talon-exec: 7 (supervision + ExecCore)
- talon-broker: 5 (mock + rate limiter)
- talon-carousel: 5 (harvest calculation)
- talon-firebird: 3 (signal generation)
- talon-db: 2 (event store)

## Known Issues
- Polygon.io free tier limited to 5 requests/minute; Yahoo fallback activates automatically
- TUI may overflow on terminals smaller than 80x24
- Session replay not yet implemented (scaffold only)
- Risk mesh uses hardcoded $100 mock entry price for single position risk calculation
- OrderId uses `clone()` where `Copy` would suffice (clippy suggestion)

## Next Steps (Takeoff Tier)
- Bevy ECS migration for 8 concurrent strategy entities
- G2 semi-supervised gate with trust graduation (auto-approve low-risk)
- Carousel vault transfers enabled
- Regime detector activation
- Trust ledger persistence
- Blackbird (RAMjet/Chine/Spike) subsystem spec
