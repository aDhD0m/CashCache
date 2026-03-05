# TALON Wiring — Signal-to-Fill Pipeline

## Channel Topology

```
Modules (Firebird/Thunderbird/Taxi)
    │ on_quote() → ScanResult
    │
    ├── OrderIntent ──→ [mpsc 256] ──→ ExecCore
    │                                     │
    │                                     ├── RiskMesh::evaluate()
    │                                     ├── evaluate_supervision()
    │                                     │
    │                                     ├── AutoExecute ──→ BrokerSessionManager::submit()
    │                                     │                        │
    │                                     │                        └── IbkrBroker → TWS API
    │                                     │
    │                                     └── RequiresApproval ──→ PendingIntent queue (max 10)
    │                                                                  │
    │                                                                  ├── TUI 'a' → Approve → submit
    │                                                                  ├── TUI 'r' → Reject → drop
    │                                                                  └── 10s timeout → auto-reject
    │
    └── ApproachingSetup ──→ AppState (watch channel) ──→ TRiMiNL Scanner tab

Broker (IbkrBroker)
    │ subscribe_fills()
    │
    └── FillEvent ──→ [broadcast 1024] ──→ Portfolio::apply_fill()
                                              │
                                              └── Carousel::calculate_harvest()

QuoteEvent flow:
    IBKR realtime_bars ──→ [mpsc 4096] ──→ bridge ──→ [broadcast 4096] ──→ modules, state loop
    Polygon snapshot   ──→ [broadcast 4096] ──→ (supplementary, 15s interval)
    Yahoo fallback     ──→ [broadcast 4096] ──→ (always-on, 30s interval)

Supervision:
    TUI ──→ [mpsc 50] ──→ ExecCore (SupervisionCommand: Approve/Reject/ApproveAll/RejectAll)

State updates:
    ExecCore/modules ──→ watch::Sender<AppState> ──→ TRiMiNL (event-driven redraw)

Events:
    System ──→ [mpsc 1024] ──→ EventStore (dedicated OS thread, SQLite WAL)

Flow (L2/T&S):
    TUI ──→ [mpsc 32] ──→ FlowManager ──→ subscribe_tape/subscribe_depth ──→ AppState.flow
```

## 8-Step Verification Checklist

1. `cargo check --workspace` — all 20 crates compile clean
2. `cargo test --workspace` — 53 tests pass (33 original + 20 pipeline)
3. Channel capacities match config/talon.toml [channels] section
4. ExecCore processes intents from modules without blocking TUI
5. Supervision timeout (10s) auto-rejects pending intents
6. PendingIntents widget renders correctly with age highlighting at >5s
7. Fills widget shows P&L color-coded (cyan=profit, yellow=loss)
8. Portfolio persistence saves PortfolioSnapshot every 5s to events.db

## Signal-to-Fill Pipeline (Detail)

### Step 1: Quote Arrives
QuoteEvent enters the broadcast channel from IBKR bars, Polygon snapshots, or Yahoo fallback.

### Step 2: Module Processes Quote
Each strategy module (Firebird, Thunderbird, Taxi) receives the quote via `on_quote()`.
Returns `ScanResult { intents, approaching }`.

### Step 3: Intent Sent to ExecCore
OrderIntent is sent via the intent mpsc channel (capacity 256).

### Step 4: Risk Mesh Evaluation
ExecCore evaluates against: nosedive gate, concurrent positions, module allocation,
single position risk %, total exposure %.

### Step 5: Supervision Gate
- SupervisedAutonomy (Hatch modules): AutoExecute — bypasses approval
- DualControlStrict (0DTE/shorts): RequiresApproval always
- DualControl: Checks trust ledger for auto-trust

### Step 6: Pending Queue or Broker
- AutoExecute: submit immediately to broker
- RequiresApproval: add to PendingIntent queue, show in TUI
- Queue full (>10): reject immediately

### Step 7: Operator Decision
TUI shows pending intents with 'a'/'r'/Up/Down/'A'/'R' keybindings.
10s timeout auto-rejects if no decision made.

### Step 8: Fill Received
Broker returns FillEvent via broadcast channel.
Portfolio updated, Carousel calculates harvest, EventStore records.
