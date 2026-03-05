# TALON -- Architecture Specification

**System:** TALON (Trade Autonomously with Limited Override Necessity)
**Brand:** CashCache
**Version:** 3.2.0
**Author:** Atom / AX4
**Date:** 2026-03-02
**Classification:** Internal Engineering Reference

---

## 1. Mission

TALON is a tiered semi-autonomous trading terminal for the full retail capital
spectrum ($500 to $100K+). It provides automated execution under human
supervision with behavioral guardrails that prevent the most common retail
failure modes: overconcentration, revenge trading, and the "win, reinvest
everything, blow up" cycle.

1. **The trader owns the logic.** TALON provides the engine. The trader
   provides the judgment.
2. **Capital preservation is a first-class concern.** CashCache Vault breaks
   the reinvestment death spiral by extracting profits before the trader can
   touch them.
3. **Graduation is earned, not purchased.** Module access is gated by capital,
   account type, and track record -- not by subscription tier.
4. **The system acts; the human overrides.** The default state is execution
   within boundaries, not paralysis awaiting approval. Human intervention is
   friction, not flow -- the harder the override, the more destructive its
   potential.

### 1.1 Scope Boundary

TALON is a trading terminal. Three separate systems exist in the broader
CashCache ecosystem but are NOT part of this specification and are not required
for TALON to ship or deliver value:

| System | What It Is | TALON Integration Point | Behavior When Absent |
|---|---|---|---|
| Blackbird RAMjet | Tiered compute dispatch (CPU -> GPU -> FPGA) | `IntelligencePort<ComputeTarget>` | All compute runs on CPU. |
| MarketPIREP | Social sentiment platform (Sky Spotters, The Runway) | `IntelligencePort<SentimentState>` | Modules ignore sentiment entirely. |
| Blackbird Chine | Combinatorial overlap discovery | `IntelligencePort<StrategyProposal>` | No proposals surfaced. |

TALON defines ingestion interfaces for these systems (S10). It does not define,
build, or depend on their internals.

---

## 2. Architectural Rules

### 2.1 No Synchronous Intelligence Gates

Any external intelligence system (regime detection, sentiment, overlap
discovery) that integrates with TALON is an asynchronous observer publishing to
shared state. Strategy modules read the latest state at decision time. No
module ever blocks waiting for an external system.

```rust
pub trait IntelligencePort<S: Send + Sync>: Send + Sync {
    fn latest(&self) -> Option<&S>;
}
```

`None` = "no data yet, act conservatively." Modules degrade gracefully -- they
operate on their own signals when no external intelligence is available.

**Enforcement:** Compile-time. External system state types are not in module
dependency trees. Modules receive `Option<&S>` through the Governor layer,
never a direct port reference.

**Failure mode prevented:** Latency spikes and cascading hangs from blocking on
external compute during time-sensitive order decisions.

### 2.2 Strategy Modules Never Touch Brokers Directly

Modules emit `OrderIntent`s into a channel. The Governor consumes intents, runs
them through the risk mesh and stress multiplier, applies graduation checks,
and calls the broker on a dedicated blocking thread pool via
`tokio::task::spawn_blocking`. Modules are never blocked on network I/O.

**Enforcement:** Compile-time. `BrokerCommands` and `BrokerStreams` traits are
not in the module dependency tree. A module literally cannot construct a broker
call -- the types don't exist in its scope.

**Failure mode prevented:** Network I/O stalling signal generation. A slow
broker response cannot delay signal evaluation for other modules.

### 2.3 All Financial Arithmetic Uses rust_decimal

No `f64` in any calculation that touches money. Signal confidence values
(inherently approximate) use `f64`. The conversion boundary occurs at exactly
one point (`PositionSizer::calculate()`), with `NaN`/infinity producing
zero-size recommendations that the Governor rejects as no-ops.

**Enforcement:** Code review + clippy lint. `#[deny(clippy::float_arithmetic)]`
in modules that handle `Decimal` types.

**Failure mode prevented:** Floating-point precision bugs in position sizing,
P&L calculation, and settlement tracking. $0.01 rounding errors compound into
real money across thousands of trades.

### 2.4 The System's Boundaries Contract Under Stress

When realized drawdown deepens, the system does not just alert -- it reduces
its own operating envelope via the stress multiplier (S7.3). Fewer concurrent
positions, smaller sizes, shorter hold durations. The human can re-expand, but
only one tier at a time, with a cooldown and mandatory justification.

**Enforcement:** Runtime. The Governor applies the stress multiplier to every
numeric limit in the risk mesh before evaluating any order intent.

**Failure mode prevented:** The "doubling down after losses" spiral. Static
limits do not protect against the psychology of a trader at -8% who wants to
"make it back." Dynamic contraction forces the system to get smaller as the
situation gets worse.

---

## 3. System Tiers

| Tier | Name | Capital | Account Type | Primary Broker | Supervision |
|---|---|---|---|---|---|
| 1 | Hatch | $500 - $25K | Cash (-> Margin at $5K) | IBKR | Supervised Autonomy |
| 2 | Takeoff | $25K+ | Margin | IBKR | Dual-Control |
| 3 | Turbo | $30K+ | Margin | Cobra / CenterPoint | Dual-Control (Full) |

### 3.1 Hatch

The on-ramp. Every user starts here. Some stay permanently. Delivers full value
independently.

**Supervised Autonomy:** Bot operates within hard boundaries. Human sets the
boundaries, not the individual trades. Appropriate because Hatch strategies
are multi-day (time to notice and react), defined-risk (max loss known at
entry), and small capital relative to the operator's total wealth.

**Modules:** Firebird, Thunderbird, Taxi, CashCache Vault.

**Cash-to-margin at $5K:** Enables credit spreads for Siphon preview access.
Requires IBKR margin approval and "Limited" options permissions (Level 1).

### 3.2 Takeoff (Dual-Control)

Full intraday execution. PDT threshold ($25K) is the literal gateway.

**Dual-Control:** Bot proposes, human approves. Trust calibration (S8) grants
auto-execution for proven action classes over time.

**Modules:** Snapback, Climb (+ all Hatch modules).

### 3.3 Turbo (Capital Deployment Lattice)

Advanced options, short selling, premium strategies. Requires Cobra or
CenterPoint/Clear Street for professional locate inventory and DAS execution.

**Modules:** SAGE, ParaShort, Siphon, YoYo (+ all lower-tier modules).

Payload ($50K+, 30+ validated days on both Snapback and YoYo) is the final
module unlock within Turbo.

---

## 4. Modules

### 4.1 Module Map

| Module | Tier | Type | Strategy | Hold | Cash OK? |
|---|---|---|---|---|---|
| Firebird | Hatch | Options (long) | Oversold reversal | 2-14 days | Yes |
| Thunderbird | Hatch | Options (long) | Overextension fade | 2-10 days | Yes |
| Taxi | Hatch | Equity (long) | Technical swing | 3-15 days | Yes |
| CashCache Vault | Hatch | Equity (long) | Profit preservation | Indefinite | Yes |
| Snapback | Takeoff | 0DTE Options | Mean-reversion 0DTE | Minutes-hours | No |
| Climb | Takeoff | Equity (long) | Intraday momentum | Minutes | No |
| SAGE | Turbo | Options (Greeks) | Gamma exposure scalp | Hours-days | No |
| ParaShort | Turbo | Equity (short) | Parabolic fade | Hours-days | No |
| Siphon | Turbo | Options (spreads) | Theta decay farming | 7-45 days | No |
| YoYo | Turbo | 0DTE Options | Binary event 0DTE | Minutes-hours | No |
| Payload | Turbo | 0DTE Adaptive | Dynamic regime 0DTE | Minutes-hours | No |

### 4.2 Firebird -- Oversold Reversal

**Thesis:** Long options on liquid underlyings at oversold extremes. Catches the
bounce that mean-reversion predicts but retail traders are too scared to buy.

**Inputs:** RSI, volume divergence, S/R proximity, options chain data.

**Outputs:** `OrderIntent` with long call/put, defined stop/target/time-stop.

**Constraints:** Underlying $2B+ market cap, bid-ask <= $0.10, OI >= 500, daily
volume >= 100. Strike: ATM or 1 strike OTM. Expiration: 4-8 weeks.

**Exit:** Target (50% premium gain), time stop (50% DTE remaining), stop-loss
(50% premium loss). Cash account compatible -- premium paid from settled funds.

**Failure behavior:** If options chain data unavailable, module enters IDLE. If
liquidity filters eliminate all candidates, no signals generated (not an error).

### 4.3 Thunderbird -- Overextension Fade

**Thesis:** Long options fading violent overextension (parabolic runs,
gap-and-fails, climax tops). Shorter holds than Firebird because overextension
snaps back faster.

**Inputs:** Bollinger extremes, volume climax, momentum divergence.

**Distinction from Firebird:** Firebird = gradual oversold at established S/R.
Thunderbird = rapid violent overextension. Different signals, different timing.

**Constraints:** Same liquidity filters as Firebird. Expiration: 2-4 weeks
(shorter thesis). ATM or 1 strike OTM.

**Failure behavior:** Same as Firebird.

### 4.4 Taxi -- Equity Swing Loader

A taxi takes you from where you are to where you need to be. Taxi carries
traders across the gap between "$500 cash account" and "$25K margin account"
without requiring options knowledge. The structural bridge that makes the whole
system accessible.

**Thesis:** Multi-day swings on mid/large-cap equities ($2B+). Pullback-to-
support, breakout-with-volume, sector rotation momentum.

**Inputs:** Price action, volume, sector relative strength.

**Outputs:** `OrderIntent` with long equity, hard stop at entry.

**Constraints:** Long only. Hard stop at entry (1-2% risk). No averaging down.
Cash compatible -- T+1 settlement with 3-5 concurrent positions keeps capital
cycling. The system tracks both `settled` and `unsettled` cash to prevent
free-riding violations. A system that treats "sold $2K of stock" as "have $2K
to spend" will submit orders that get rejected for insufficient settled funds.

```rust
pub struct CashBalance {
    pub settled: Decimal,
    pub unsettled: Decimal,
    pub pending_settlement: Vec<SettlementEvent>,
}
```

**Failure behavior:** If fewer than 3 positions can be funded with settled cash,
module reduces scan universe rather than going idle.

### 4.5 CashCache Vault -- Profit Preservation

See `CASHCACHE_MODULE_SPEC_v0.2.0.md`. Summary here.

**Thesis:** Automated profit harvesting into a separate brokerage sub-account
the trading system cannot draw against. Breaks the "win, reinvest, blow up"
cycle structurally, not psychologically.

**Harvest rules:** When a position closes profitably, a configurable percentage
(default: 30%) of the realized gain is marked for harvest. Harvested funds
transfer to a separate sub-account. Transfers execute only against settled
cash -- unsettled funds are never harvested (prevents ghost harvests). A minimum
harvest threshold (gravity well) prevents uneconomical micro-transfers.

**Vault withdrawal:** Multi-step process with a 24-hour delay. Not a security
measure -- a behavioral one. The delay exists so the trader cannot impulsively
raid the vault during a drawdown.

**Wash sale tracking:** `WashGroup::ETFFamily` catches cross-ticker matches
(SPY/VOO/IVV/SPLG). 30-day lookback on all closed losing positions prevents
CashCache from creating wash sales during harvest rotation.

```rust
pub enum WashGroup {
    Ticker(Symbol),
    ETFFamily(Vec<Symbol>),
}
```

**IBKR isolation (three-layer defense):**

1. **Software isolation:** TALON cannot construct orders against Vault holdings.
   Different sub-account credentials.
2. **Margin isolation:** IBKR "Close-Only" sub-account designation prevents
   cross-margin. Configured during onboarding.
3. **Nuclear option ($25K+):** Separate IBKR account (Friends & Family) for
   true regulatory isolation.

**Failure behavior:** If IBKR sub-account unreachable, harvests queue locally.
On reconnection, queued harvests execute in FIFO order against settled cash.
If vault sub-account shares margin unexpectedly (config error), alert operator
and pause all harvests until isolation verified.

### 4.6 Snapback -- Mean-Reversion 0DTE

**Thesis:** SPX/SPY 0DTE mean-reversion. VWAP deviation + IV rank + time-of-day
regime.

**Constraints:** All positions closed by 3:45 PM EST. This is a non-negotiable
safety mechanism -- not subject to operator approval, not deferrable. Close
order escalation: limit -> aggressive limit (ask x 1.02, IOC) -> market order.
SPX preferred (European-style, cash-settled -- no accidental stock position
from exercise). SPY positions carry early assignment risk and physical
settlement.

**Supervision:** Dual-Control Strict (no auto-trust). 0DTE gamma risk is too
extreme for unsupervised execution regardless of track record.

**Failure behavior:** If close order does not fill within 60 seconds of first
attempt, escalate to market. If market order does not fill (halt), alert
operator with "0DTE POSITION AT RISK -- HALTED UNDERLYING" and queue fill for
halt resume. Clock synchronization to NTP required -- 3:45 PM means 3:45 PM.

### 4.7 Climb -- Intraday Momentum Breakout

**Thesis:** Nano/micro-cap ($50M-$2B, float 5M-50M shares) breakout above
pre-market high with volume.

**Inputs:** Pre-market scanner, real-time L2 data, volume confirmation.

**Outputs:** `OrderIntent` with long equity, trailing stop.

**Constraints:** Strictly intraday -- overnight nano/micro gap risk is
existential. Requires $25K margin (T+1 defeats momentum on cash). PDT
awareness: every Climb round-trip is a day trade. Below $25K equity =
module disabled.

```rust
pub struct PDTTracker {
    pub day_trades: VecDeque<NaiveDate>,
    pub account_equity: Decimal,
}

impl PDTTracker {
    pub fn can_day_trade(&self) -> bool {
        if self.account_equity >= Decimal::from(25_000) {
            return true;
        }
        let five_days_ago = business_days_ago(5);
        let count = self.day_trades.iter()
            .filter(|d| **d >= five_days_ago)
            .count();
        count < 3
    }
}
```

**Failure behavior:** If pre-market scanner unavailable, module enters IDLE. If
L2 data drops mid-trade, existing positions managed on L1 only with tighter
stops.

### 4.8 SAGE -- Gamma Exposure Scalping

**Thesis:** GEX flip level proximity + dealer positioning + RV/IV divergence on
SPX, SPY, QQQ, high-GEX names.

**Inputs:** Options chain for GEX computation (OI x gamma x contract
multiplier, summed across strikes) OR pre-computed GEX from third-party feed
(SpotGamma, Orats). IV rank for entry timing.

**Outputs:** Options spreads, delta-neutral hedges. Frequent adjustments.

**Constraints:** Atomic multi-leg order submission mandatory -- the broker must
support spread orders via API. IBKR `ComboLeg` or DAS TAPI spread orders. If
the broker does not support atomic spreads, SAGE is not viable on that broker.

```rust
pub enum OrderType {
    Single(LegOrder),
    Spread {
        legs: Vec<LegOrder>,
        net_debit_limit: Option<Decimal>,
        net_credit_limit: Option<Decimal>,
    },
}
```

**Supervision:** Dual-Control with auto-hedge within operator-defined bounds.
Delta-neutral hedging adjustments are pre-approved within the bounds; new
directional exposure requires approval.

**Failure behavior:** If options chain data stale (>30s), SAGE pauses signal
generation and dims TUI indicator. If GEX computation fails (missing OI data),
SAGE degrades to IV-rank-only mode with reduced confidence.

### 4.9 ParaShort -- Parabolic Fade

**Thesis:** Short equity after >100% move in <5 sessions. Volume divergence +
failed new high + bearish engulfing.

**Constraints:** Locate is a hard prerequisite. `locate_shares()` must succeed
BEFORE signal generation -- not at order time. Borrow cost included in
risk/reward calculation before entry. Hard stop above parabolic high.

**Supervision:** Dual-Control Strict (no auto-trust). Theoretically unlimited
loss. Forced cover protocol (S7.5) is mandatory.

**Failure behavior:** If locate fails, signal is discarded (logged as
LOCATE_FAILED, not as a rejection). If borrow cost exceeds configurable
threshold (default: 50% of expected profit), signal downgrades to WATCH
(surfaced in TUI but not actionable). If broker connection drops with open
short, see S12.4 emergency procedures.

### 4.10 Siphon -- Theta Decay Farming

**Thesis:** Credit spreads, iron condors, calendar spreads on range-bound
underlyings (ADX < 25, IV rank > 50th percentile).

**Inputs:** IV rank, ADX, historical range analysis, options chain.

**Outputs:** Multi-leg spread orders. Exit at 50% max credit capture or 200%
of credit received as stop.

**Constraints:** Atomic multi-leg submission required. Early assignment risk
monitoring on short legs (especially near ex-dividend). Margin requirement
scales with position count -- the system tracks aggregate Siphon margin against
the per-strategy ceiling.

**Preview at Hatch Margin ($5K+):** Vertical credit spreads only. Requires
IBKR options approval: Level 1 (Limited) for cash-secured puts, Level 2
(Standard) for full verticals. Approval checked at startup; insufficient level
disables module with actionable message.

**Failure behavior:** If early assignment detected (stock position appears
without corresponding order), surface to operator immediately. Do not
auto-liquidate -- operator may want to manage the resulting stock position.

### 4.11 YoYo -- Binary Event 0DTE

**Thesis:** 0DTE around FOMC/CPI/NFP. Pre-event straddles if IV underprices the
move. Post-event directional based on initial move direction.

**Constraints:** Same 3:45 PM hard close as Snapback. Event calendar
integration required. Position sizing accounts for the binary nature -- smaller
size, wider stops.

**Supervision:** Dual-Control Strict. Binary events are inherently
unpredictable.

**Failure behavior:** If event calendar unavailable, module enters IDLE with
"NO EVENT CALENDAR" in TUI. Manual override available via `:yoyo force-arm`.

### 4.12 Payload -- Dynamic Regime 0DTE

**Thesis:** Fuses Snapback + YoYo. Continuously selects strategy based on
regime + time + event calendar. The payload the whole system has been building
toward.

**Constraints:** Requires 30+ validated days on both Snapback AND YoYo. Both
predecessors must be independently profitable over the evaluation window.
$50K minimum.

**Supervision:** Dual-Control Strict. Adaptive behavior makes this the most
complex module -- no auto-trust regardless of track record.

**Failure behavior:** If either prerequisite module is disabled, Payload is
disabled. Payload never operates in conditions its components have not been
individually validated for.

---

## 5. Graduation System

### 5.1 Gates

| Transition | Capital | Account | Track Record | Broker |
|---|---|---|---|---|
| -> Hatch | $500 | Cash | None | IBKR verified |
| -> Hatch Margin | $5K | Margin | None (opt-in) | IBKR margin approved |
| Hatch -> Takeoff | $25K | Margin | 20 live + 10 any (paper=0.5x). >50% win rate on live. Max DD <20% | IBKR margin |
| Takeoff -> Turbo | $30K | Margin | 60+ trades across Takeoff modules. 10+ documented short theses | Cobra / CenterPoint active |
| Turbo -> Payload | $50K | Margin | 30+ days validated on Snapback + YoYo. Both profitable. | Turbo broker |

### 5.2 Behavior

**Not forced.** Meeting gates surfaces a proposal in TUI Zone C. User defers
indefinitely.

**Soft demotion** (within 10% of gate capital): Warning in TUI Zone A. Modules
remain active. Stress multiplier (S7.3) begins engaging.

**Hard demotion** (>10% below gate): Higher-tier modules paused, positions
exit-only. Re-graduation on capital recovery -- no track record
re-evaluation required.

**During demotion:** In-flight positions managed to exit. No new entries on
paused modules. CashCache Vault continues operating at all tiers.

---

## 6. Broker Architecture

### 6.1 Broker Table

| Broker | Role | Priority | API | Phase |
|---|---|---|---|---|
| IBKR | Primary all tiers + Vault home | P0 | TWS API + Client Portal REST | Month 1 |
| Alpaca | Hatch alt (API-first) | P1 | REST + WebSocket | Month 3 |
| Webull | Hatch alt (zero-cost) | P1-ALT | HTTP + GRPC + MQTT | Post-Austin |
| Cobra Trading | Turbo primary (shorts + locates) | P2 | DAS TAPI + DAS FIX 4.2 | Phase C |
| CenterPoint / Clear Street | Turbo fallback | P2-ALT | Clear Street REST + FIX 4.2 | If Cobra fails |

**Rejected:** SpeedTrader (redundant -- same DAS TAPI, worse locates).
NinjaTrader (futures-only, wrong asset class). **Deferred:** Tradier (clean
REST, $0.35/contract -- evaluate in v2).

### 6.2 Split Trait: BrokerCommands + BrokerStreams

```rust
/// Synchronous. Order lifecycle + account queries.
/// Runs on dedicated blocking thread pool via spawn_blocking.
pub trait BrokerCommands: Send + Sync {
    fn submit_order(&self, order: &OrderIntent) -> Result<OrderAck, BrokerError>;
    fn cancel_order(&self, id: &OrderId) -> Result<CancelAck, BrokerError>;
    fn modify_order(&self, id: &OrderId, m: &OrderModify) -> Result<ModifyAck, BrokerError>;
    fn positions(&self) -> Result<Vec<Position>, BrokerError>;
    fn account_snapshot(&self) -> Result<AccountSnapshot, BrokerError>;
    fn settled_cash_delta(&self, date: NaiveDate) -> Result<Decimal, BrokerError>;
    fn broker_id(&self) -> BrokerId;
    fn supports_short(&self, symbol: &Symbol) -> Result<ShortAvailability, BrokerError>;
    fn locate_shares(&self, symbol: &Symbol, qty: u64) -> Result<LocateResult, BrokerError>;
}

/// Asynchronous. Long-lived streaming connections.
/// Drop StreamHandle = cancel stream. RAII cancellation via oneshot channel.
#[async_trait]
pub trait BrokerStreams: Send + Sync {
    async fn subscribe_quotes(
        &self, symbols: &[Symbol], tx: Sender<QuoteEvent>,
    ) -> Result<StreamHandle, BrokerError>;
    async fn subscribe_fills(
        &self, tx: Sender<FillEvent>,
    ) -> Result<StreamHandle, BrokerError>;
    async fn subscribe_margin_events(
        &self, tx: Sender<MarginEvent>,
    ) -> Result<StreamHandle, BrokerError>;
}

/// RAII cancellation handle. Drop = cancel.
pub struct StreamHandle {
    _cancel: tokio::sync::oneshot::Sender<()>,
    join: tokio::task::JoinHandle<()>,
}
```

All `BrokerCommands` calls from async code go through the
`BrokerSessionManager` which wraps every call in
`tokio::task::spawn_blocking`:

```rust
impl BrokerSessionManager {
    pub async fn submit(
        &self, order: &OrderIntent,
    ) -> Result<OrderAck, BrokerError> {
        let commands = self.commands.clone();
        let order = order.clone();
        tokio::task::spawn_blocking(move || {
            commands.submit_order(&order)
        }).await.map_err(|e| BrokerError::RuntimePanic(e.to_string()))?
    }
}
```

The session manager holds
`HashMap<BrokerId, (Arc<dyn BrokerCommands>, Arc<dyn BrokerStreams>)>` and
routes operations based on module affinity and account type.

### 6.3 Multi-Broker Routing

At Turbo: simultaneous IBKR (CashCache Vault) + Cobra/CenterPoint (active).

| Operation | Routes To | Fallback |
|---|---|---|
| CashCache Vault harvests/transfers | IBKR (always) | None -- Vault is IBKR-only |
| Short sells + locates | Cobra / CenterPoint | CenterPoint / Cobra (reciprocal) |
| Long options (Turbo) | Cheapest available | IBKR |
| Everything else | Cheapest available | IBKR |

### 6.4 Pivot Triggers

| Condition | Action |
|---|---|
| Alpaca/Webull instability | IBKR sole Hatch broker |
| IBKR TWS API unworkable in Rust | Elevate Alpaca + Client Portal REST fallback |
| Cobra DAS TAPI problematic | Pivot to CenterPoint/Clear Street REST |
| FINRA PDT replacement approved | Reconfigure $25K boundary. Tier thresholds are config, not code. |

---

## 7. Supervision & Risk

### 7.1 Supervision Models

```rust
pub enum SupervisionModel {
    /// Hatch. Boundaries only. Bot executes freely within limits.
    SupervisedAutonomy,
    /// Takeoff/Turbo. Per-trade approval with trust graduation.
    DualControl,
    /// Short selling, 0DTE. No auto-trust. Every trade, every time.
    DualControlStrict,
}
```

**Kill switch:** Single keystroke (`K`) flattens any or all positions. Bypasses
all Governor logic. Market orders direct to broker. Logged but never gated.
Cannot be undone (positions are closed, not paused). TUI label: `EJECT`.

### 7.2 Account-Level Risk Parameters

| Parameter | Hatch | Takeoff | Turbo |
|---|---|---|---|
| Max single-position risk | 5% | 3% | 2% |
| Max total exposure | 60% | 80% | 80% |
| Max concurrent positions | 5 | 10 | 15 |
| Drawdown circuit breaker | -15% | -10% | -8% |
| Daily loss limit | 5% | 3% | 2% |

**Drawdown peak:** Rolling high-water mark of daily close net liquidation
(excluding CashCache Vault). Resets on explicit operator acknowledgment only.

### 7.3 Stress Multiplier

The Governor's most important mechanism. As realized drawdown from peak
deepens, the stress multiplier auto-contracts EVERY numeric limit in the risk
mesh:

| Drawdown from Peak | Stress Multiplier | Effect |
|---|---|---|
| 0-3% | 1.0 | Normal operation |
| 3-5% | 0.75 | All limits reduced 25% |
| 5-8% | 0.50 | All limits reduced 50% |
| 8% to circuit breaker | 0.25 (FLAMEOUT) | Minimal new risk. Trailing stops tightened. |
| Beyond circuit breaker | 0.0 (NOSEDIVE) | Halt. Exit-only. Operator must acknowledge. |

The multiplier applies multiplicatively to every numeric limit. At 0.50: if max
concurrent positions is 10, effective max is 5. If max single-position risk is
3%, effective is 1.5%.

**Human override:** The operator can expand the stress multiplier upward by ONE
tier at a time, with a 15-minute cooldown between overrides. Each override
requires a mandatory free-text justification that is logged, event-sourced, and
surfaced in end-of-day review. This is not a technical limitation -- it is a
behavioral speed bump.

```toml
[risk.stress]
tier_0_threshold_pct = 3.0
tier_1_multiplier = 0.75
tier_2_threshold_pct = 5.0
tier_2_multiplier = 0.50
tier_3_threshold_pct = 8.0
tier_3_multiplier = 0.25
override_cooldown_mins = 15
```

### 7.4 Flameout Protocol

Flameout engages when the stress multiplier hits 0.25 -- the final tier before
the circuit breaker halts all activity. It is the engine coughing before it
dies. The Governor automatically tightens trailing stops on all open positions
to preserve remaining capital:

**Profitable positions:** Trail to breakeven or current stop, whichever is
tighter. The position can no longer become a loss.

**Losing positions:** Tighten stop to 50% of original stop distance from
current price. A position with a $2.00 stop distance gets tightened to $1.00.

Flameout is not a module. It is a Governor protocol, sibling to Forced Cover
(S7.5). It fires automatically and is logged as a `FLAMEOUT` event with full
position state at the time of engagement.

```toml
[risk.flameout]
trigger_multiplier = 0.25
profitable_positions = "trail_to_breakeven"
losing_positions = "tighten_stop_50pct"
```

### 7.5 Nosedive -- Circuit Breaker

Nosedive is the event: the circuit breaker has tripped. Stress multiplier hits
0.0. All modules halt. Exit-only mode. No new entries of any kind.

The TUI enters full-screen takeover mode (same pattern as reconciliation in
S9.3). Zone A shows `NOSEDIVE` in pulsing amber. The operator cannot dismiss
the screen -- they must acknowledge, review the drawdown, and decide whether
to reset the circuit breaker or shut down for the day.

```
+-- NOSEDIVE -- CIRCUIT BREAKER TRIPPED ----------------------------+
| Drawdown: -10.2% from peak ($24,890 -> $22,349)                  |
| Flameout engaged at -8.1% -- stops tightened on 4 positions      |
| Time since Flameout: 16m 22s                                     |
|                                                                   |
| Open positions (exit-only):                                       |
|   NVDA  -$340  stop $136.80 (Flameout-tightened)     [Climb]     |
|   SPY 0DTE  -$180  EXIT 3:45PM                      [Snapback]   |
|                                                                   |
| [ACKNOWLEDGE + RESET]  [ACKNOWLEDGE + SHUT DOWN]  [DETAILS]      |
+-------------------------------------------------------------------+
```

Resetting the circuit breaker re-engages at stress tier 3 (0.25), not at
normal operation (1.0). The operator must override upward one tier at a time
with cooldowns and justifications.

### 7.6 Module-Level Risk Arbitration

Static allocation per module in `risk.toml`. The stress-adjusted account-level
ceiling is the hard gate -- the last module to request capital beyond the
ceiling is blocked. No module can exceed its own allocation even if account
headroom exists. Module-level limits never sum to exceed the account ceiling.

### 7.7 Forced Cover Protocol (Short Positions)

The ONLY code path bypassing Dual-Control. Evaluated on 1-min bars (not tick).

```toml
[risk.forced_cover]
trigger_pct = 10.0                # % adverse from entry
order_type = "aggressive_limit"   # Limit at ask x 1.02, IOC
retry_delay_secs = 30             # If unfilled, retry as market
halt_escalation_mins = 10         # After halt resume, time before market order
```

**On trigger:**

1. Submit aggressive limit (ask x 1.02, IOC).
2. If unfilled after 30s, submit market order.
3. Log full event with correlation span.
4. Persistent TUI banner: "FORCED COVER EXECUTED -- [symbol] [qty] @ [price]"
5. Push notification to operator.
6. Module enters review mode (no new signals until operator acknowledges).

**Halted stock:** Alert operator immediately, queue limit at halt price + 5%,
escalate to market if no operator response within 10 min of resume.

**Post-execution:** CashCache harvest (if pending) on the force-covered
position is cancelled. The forced cover fill is event-sourced with a
`ForcedCover` tag for reconciliation and audit.

### 7.8 Supervision-on-Timeout Defaults

In Dual-Control mode, if the operator does not respond within the configured
timeout:

| Action Class | Default on Timeout |
|---|---|
| New position entry | REJECT. Err toward capital preservation. |
| Stop-loss / protective exit | EXECUTE. Implicitly approved at entry time. |
| Forced cover (short) | EXECUTE. Bypasses timeout entirely. |
| 0DTE hard close (3:45 PM) | EXECUTE. Bypasses timeout entirely. |
| CashCache Vault harvest | EXECUTE. Settlement-gated, no risk. |

---

## 8. Trust Calibration

### 8.1 Trust Unit

Trust is earned per `(ModuleId, RegimeState, ActionClass)` tuple. Each
combination earns trust independently.

### 8.2 Trust Entry

```rust
pub struct TrustEntry {
    pub approvals: u32,
    pub rejections: u32,
    pub unique_days_of_week: HashSet<Weekday>,
    pub vix_buckets_seen: HashSet<VixBucket>,
    pub time_buckets_seen: HashSet<TimeBucket>,
}

pub enum VixBucket { Low, Normal, Elevated, High } // <15, 15-20, 20-30, 30+
pub enum TimeBucket { Open, MidMorning, Lunch, Afternoon, Close }

impl TrustEntry {
    pub fn qualifies_for_auto_trust(&self) -> bool {
        self.approvals >= 100
            && self.rejections == 0
            && self.unique_days_of_week.len() >= 4
            && self.vix_buckets_seen.len() >= 2
            && self.time_buckets_seen.len() >= 2
    }
}
```

### 8.3 Diversity Requirements

Prevent overfitting to a narrow market condition:

- >= 100 consecutive approvals with 0 rejections
- >= 4 unique days of the week
- >= 2 VIX buckets
- >= 2 time-of-day buckets

100 approvals all on Tuesday mornings in low-vol trending markets does not
prove the action class works in crisis conditions.

### 8.4 Trust Regime-Conditioning

Trust is regime-conditional. 100 approvals in TRENDING does not grant autonomy
in CRISIS. The trust ledger is keyed by `(ModuleId, RegimeState, ActionClass)`.

**When no regime system is connected** (`IntelligencePort<RegimeState>` returns
`None`), all trust entries are keyed to `RegimeState::Standalone`. Trust earned
in Standalone does NOT transfer when a regime system comes online -- the module
must re-earn trust under each detected regime.

### 8.5 Trust Revocation

A single rejection after auto-trust is granted causes immediate revocation for
that `(module, regime, action_class)` tuple. Counter resets to zero. Trust
re-earned from scratch.

This asymmetry is intentional. Easy to lose, hard to earn.

### 8.6 Trust Exclusions

Trust calibration is DISABLED for:

- All 0DTE modules (Snapback, YoYo, Payload) -- Dual-Control Strict
- ParaShort (short selling) -- Dual-Control Strict

These modules require operator approval on every trade, every time, regardless
of track record. A single bad 0DTE trade or uncovered short can be
catastrophic.

### 8.7 Trust Escalation UX

When qualifications are met, TUI Zone C surfaces an upgrade proposal:

```
+-- TRUST UPGRADE PROPOSAL --------------------+
| Climb qualifies for auto-trust               |
| in TRENDING regime, ENTRY action class       |
|                                              |
| Evidence: 100 approvals, 0 rejections        |
| Conditions: Mon-Fri, VIX 12-28, all times   |
|                                              |
| [GRANT AUTO-TRUST]  [NOT YET]  [DETAILS]    |
+----------------------------------------------+
```

The operator must explicitly grant. Auto-trust is never automatically applied.

---

## 9. Data Architecture

### 9.1 Sovereign Data Partitioning

Compile-time boundary. Sovereign types (positions, trades, identity, account
balances) implement no `Debug`/`Display`/`Serialize` by default. Access only
via `SovereignContext` guard that logs every access with caller identity and
purpose.

```rust
pub struct SovereignContext<'a> {
    pub caller: &'a str,
    pub purpose: &'a str,
    pub timestamp: DateTime<Utc>,
}

pub trait SovereignAccess<T> {
    fn read(&self, ctx: &SovereignContext) -> &T;
}
```

### 9.2 Persistence

| Data | Store | Access Pattern | Retention | Thread Model |
|---|---|---|---|---|
| Trade events, Vault events | SQLite WAL, event-sourced | Append-only, replay | Indefinite | Dedicated blocking writer thread |
| Market data (tick) | Ring buffer + daily archive | Stream in, batch flush | 30d full, 1yr bars | Async with periodic flush |
| Strategy parameters | TOML config, git-versioned | Read at startup, SIGHUP | Indefinite | Main thread |
| Sovereign data | Encrypted SQLite | Guarded read/write | Indefinite | Dedicated blocking writer thread |
| Trust ledger | SQLite (within events.db) | Append on approval/reject | Indefinite | Event store writer thread |
| Stress override log | SQLite (within events.db) | Append with justification | Indefinite | Event store writer thread |

**EventStore** runs on a dedicated OS thread -- never on the tokio executor.
Channel-backed: async code sends `StoreCommand::Append(event)`, the blocking
writer thread executes the INSERT.

```rust
pub struct EventStore {
    tx: std::sync::mpsc::Sender<StoreCommand>,
    _writer_thread: std::thread::JoinHandle<()>,
}

enum StoreCommand {
    Append(Event),
    Checkpoint,
    Shutdown,
}
```

WAL checkpoint: auto at 1000 pages, manual TRUNCATE during overnight batch
(0200 EST). `PRAGMA busy_timeout=5000`. `PRAGMA journal_mode=WAL`. Single
writer thread eliminates WAL contention entirely.

### 9.3 Position Reconciliation

Triggered on every startup and every broker reconnection during market hours.

**State machine:**

```rust
pub enum ReconciliationState {
    PullBrokerSnapshot,
    PullFillHistory { since: DateTime<Utc> },  // 30-min overlap for clock skew
    ReplayMissing { fills: Vec<BrokerFill> },
    Diff { broker: PositionSnapshot, local: PositionSnapshot },
    OperatorReview { discrepancies: Vec<Discrepancy> },
    Resolved,
}
```

**Resolution rules:**

| Discrepancy | Resolution | Auto or Manual |
|---|---|---|
| Quantity mismatch (broker != local) | Broker wins. Inject synthetic adjustment events. | Auto |
| Phantom position (local-only) | Freeze. Surface for operator review. | Manual |
| Orphaned position (broker-only) | Surface for operator claim or manual close. | Manual |
| Orphaned CashCache harvest | Debit cash buffer. Resume harvest protocol. | Auto |
| Unlogged forced cover (broker covered while system down) | Replay cover fill, close position. Surface alert -- the system's forced cover SHOULD have fired first. | Auto + Alert |
| Partial fill crash (submitted, partial fill, crash, rest filled while down) | Pull full fill history, replay all missing fills. | Auto |

**Hard rule:** No new positions until reconciliation reaches `Resolved`.
Full-screen TUI takeover during reconciliation. The operator cannot dismiss
it -- they must engage.

---

## 10. External System Integration Points

TALON does not build these systems. It defines the ports they plug into.

### 10.1 Port Interface

```rust
pub trait IntelligencePort<S: Send + Sync>: Send + Sync {
    fn latest(&self) -> Option<&S>;
}
```

**Backing store:** `Arc<ArcSwap<Option<S>>>` for high-read scenarios (lock-free
reads -- regime state published every few seconds, read every tick).
`Arc<RwLock<Option<S>>>` for moderate-read scenarios. Publisher runs in its own
task/thread. Consumer calls `latest()` synchronously -- never blocking, never
async.

### 10.2 Defined Ports

| Port | State Type | Producer | When None | When Some |
|---|---|---|---|---|
| Regime | `RegimeState` (Trending / Reverting / Crisis / LowLiq) | Blackbird Spike | Own signals, no weighting. Trust keyed to `Standalone`. | Regime-weighted sizing. Trust per regime. |
| Sentiment | `SentimentState` (Bullish / Bearish / Neutral + confidence) | MarketPIREP | Modules ignore sentiment. | Sentiment modifier on entry signals. |
| Belief | `BeliefState` (fused direction + uncertainty sigma) | Blackbird Spike | Modules operate independently. | Fused direction displayed in TUI. |
| Proposals | `StrategyProposal` (parameter combos) | Blackbird Chine | No proposals surfaced. | Proposals in TUI Zone C. |
| Compute | `ComputeTarget` (hardware dispatch) | Blackbird RAMjet | CPU-only. | GPU/FPGA dispatch. |

### 10.3 What This Means

TALON v1 ships with all ports returning `None`. Every module works. Every
strategy executes. Every risk control functions. Zero runtime overhead when
ports are unused -- `ArcSwap` reads of `None` are effectively free.

When an external system comes online: implement the port, register it with the
Governor. Modules automatically consume enriched state. No module code changes.
No recompilation. Blackbird flies at altitude gathering intelligence TALON
cannot reach alone. TALON operates independently, but sees further when
Blackbird is overhead.

---

## 11. TUI: The Cockpit

### 11.1 Layout

```
+---------------------------------------------------------------------+
|                        ZONE A: THE HORIZON                          |
|  * Health  * Clock  * Regime  * Belief  * VIX/SPX  * Broker dots   |
|  Stress multiplier indicator when engaged (x0.75 / x0.50 / x0.25)  |
|  Cruising Altitude indicator when eligible                          |
+--------------------------------+------------------------------------+
|     ZONE B: THE ARENA          |     ZONE C: THE BENCH             |
|                                |                                    |
|  Active positions              |  Module status cards               |
|  P&L bars + entry/stop/target  |  Signal queue (pending approval)   |
|  Time in trade                 |  Trust calibration counters        |
|  Module badge                  |  Regime -> module affinity         |
|                                |                                    |
+--------------------------------+------------------------------------+
|     ZONE D: THE VAULT          |     ZONE E: THE LOG               |
|                                |                                    |
|  CashCache total + sparkline   |  Event stream (last N)             |
|  Harvest pending               |  Trade lifecycle spans             |
|  Blocked instruments (wash)    |  Warnings / errors                 |
|  Stress override history       |  Stress multiplier changes         |
|                                |                                    |
+--------------------------------+------------------------------------+
```

### 11.2 Zone A -- The Horizon

Top strip. Always visible. Never occluded. Answers "is the world on fire?" in
one second.

- **System health:** Single glyph. Green/yellow/red.
- **Clock:** Market time (EST). Seconds visible.
- **Regime:** Single word. Saturation scales with confidence. `STANDALONE` when
  no regime system is connected (neutral gray).
- **Belief arrow:** Up/right/flat/down with uncertainty encoding (sharp = low
  sigma, fuzzy = high sigma). Dash when no fusion system connected.
- **Market pulse:** VIX + direction, SPX + % change, volume vs average.
- **Broker dots:** One per connected broker. Green/yellow/red. Blinking red =
  dead connection with open positions (EMERGENCY).
- **Stress indicator:** When stress multiplier < 1.0, shows `x0.75` / `x0.50`
  / `FLAMEOUT x0.25` in amber with increasing brightness. At `x0.0`
  (Nosedive), full-screen takeover replaces all zones.
- **Cruising Altitude:** When eligible (S11.10), shows `CRUISING ALTITUDE` in
  muted blue-green (vault color). Operator can step away.

### 11.3 Zone B -- The Arena

Each position is a horizontal bar:

```
 NVDA  ^ +2.3%  |=========...|  $+460  T 0:42:18  [Climb]     STOP: $138.20
 SPY   v -0.4%  |====.......|  $-82   T 1:15:03  [Snapback]  EXIT: 3:45PM
```

Progress bar: left end = stop loss, right end = target profit, fill level =
current P&L relative to both exits. One glance: "winning and approaching
target" vs "losing and approaching stop."

**Semantic brightness:** High-confidence positions at full saturation. Marginal
positions dimmer. Eye gravitates to brightest bars.

**0DTE positions** always show `EXIT: 3:45PM`. The hard close is the most
important information.

**Stress contraction:** When stress multiplier is active, a thin amber border
appears around Zone B indicating reduced capacity.

### 11.4 Zone C -- The Bench

```
+-- CLIMB ----------------------------------+
| Status: SCANNING                           |
| Signals: 3 gen / 2 appr / 1 rej          |
| Win rate (30d): 62%  Trust: 87/100        |
|                                           |
| >> PENDING: NVDA breakout $142.30         |
|    conf: 0.81  size: 17 shares            |
+-------------------------------------------+
```

Pending signals pulse with frequency increasing with urgency. Trust counter:
`87/100 (TRENDING)` -- when 100 reached, trust upgrade proposal appears.

### 11.5 Zone D -- The Vault

```
+-- CASHCACHE VAULT ---------------------+
| VAULT: $12,847.32                       |
| ........### (+$847.32 this month)       |
| Harvest pending: $42.18 (settling)      |
| Blocked: SPY, VOO, IVV (wash 18d)      |
| Last override: 3d ago (x0.75->1.0)     |
+-----------------------------------------+
```

The sparkline is the most psychologically important element in the TUI. On bad
days, the vault sparkline is visual evidence that the system worked. Money was
preserved. Muted blue-green. Not green (green = active P&L). Stable. Growing.

### 11.6 Zone E -- The Log

```
 14:32:07  OK  Climb      NVDA filled $142.28 x 17
 14:31:45  GO  Governor   NVDA approved (conf: 0.81, stress: x1.0)
 14:31:44  >>  Climb      NVDA breakout $142.30 (pending)
 14:30:12  $$  CashCache  Harvest +$28.40 from MSFT (settling)
 14:28:33  !!  Regime     REVERTING -> TRENDING (conf: 0.74)
 14:25:01  **  Stress     Multiplier engaged: x0.75 (DD: -3.2%)
```

Each event type has its own prefix + color. Governor decisions include stress
multiplier state for audit trail.

### 11.7 Color Philosophy

**No pure red (#FF0000) for losses.** Desaturated warm amber/rust. Pure red
triggers fight-or-flight.

**No pure green (#00FF00) for gains.** Muted teal/seafoam. Pure green triggers
dopamine and risk-seeking.

**Brightest elements are system states, not P&L.** Regime indicator, broker
dots, health glyph, stress indicator -- brightest. P&L numbers deliberately
subdued.

**Confidence-scaled saturation:** High confidence = vivid color. Low confidence
= washed out. Unknown/absent = neutral gray.

**Background:** Deep navy/charcoal. Not pure black (too harsh for 6+ hour
sessions). Not gray (too washed).

### 11.8 Keyboard Interaction

**Single-key (urgent):** `K` = EJECT (kill all). `Space` = approve pending.
`Esc` = reject pending. `Tab` = cycle zones.

**Two-key (important):** `C-r` = reset circuit breaker. `C-d` = toggle debug.
`C-s` = stress override (opens justification prompt).

**No three-key combos.** Complex actions use command mode (`:` prefix):

```
:approve nvda           # approve specific signal
:reject all             # reject all pending
:kill nvda              # flatten specific position
:vault status           # detailed CashCache Vault view
:trust climb            # trust calibration detail
:stress override        # override stress multiplier (with justification)
:stress history         # show override log
```

### 11.9 Adaptive Behavior

- **Quiet markets:** Zone B updates every 5s. Zone A every 10s.
- **Volatile markets:** Zone B every second. Zone A every second.
- **Crisis / Stress engaged:** Zone A regime fills top strip in pulsing amber.
  Zone B sorts by loss magnitude (worst first). Zone C suppresses new
  signals -- only existing position management visible.
- **Flameout (x0.25):** Zone A shows `FLAMEOUT x0.25`. Zone B shows
  tightened stops with `(FL)` suffix. Zone E logs all Flameout stop changes.
- **Nosedive (x0.0):** Zones B-E replaced with full-screen Nosedive panel
  (S7.5). No trading until acknowledged.
- **Reconciliation:** Zones B-E replaced with full-screen reconciliation panel.
  No trading until resolved.

### 11.10 Cruising Altitude

Cruising Altitude is a Governor-detected system mode, not a module. It engages
when the operator can safely step away -- make breakfast, journal trades, take
a break -- because all active positions are in modules that do not require
sub-minute attention.

**Eligibility:** Cruising Altitude is active when ALL of the following are true:

1. No module with `SupervisionModel::DualControl` or `DualControlStrict` has
   an active position.
2. No intraday module (Climb, Snapback, YoYo, Payload) is in `SCANNING` state.
3. All open positions are in Cruising Altitude-eligible modules.

```toml
[system.modes]
cruising_altitude_eligible_modules = [
    "firebird", "thunderbird", "taxi", "siphon", "cashcache"
]
```

**TUI behavior:** Zone A shows `CRUISING ALTITUDE` in muted blue-green (vault
color). Update rates drop to minimum. Zone C shows historical/review data
instead of pending signals. The cockpit dims because there is nothing to fly.

**Exit:** Any intraday module entering `SCANNING` state, any new Dual-Control
signal, or any stress multiplier engagement immediately exits Cruising Altitude
and restores normal TUI refresh rates.

### 11.11 None-State Display Rules

| Source | None Appearance |
|---|---|
| Regime system | `STANDALONE` in neutral gray |
| Belief fusion | Dash in neutral gray |
| Sentiment | Element absent (no placeholder) |
| Blackbird Chine proposals | Proposal area in Zone C absent |
| Broker connection | Dot turns red and blinks |
| Stale data (>30s) | Element dims, age in parentheses |

---

## 12. Operations

### 12.1 Development Environment

- **Machine:** NucBox (openSUSE Tumbleweed)
- **Language:** Rust (stable toolchain)
- **TUI:** Ratatui
- **Persistence:** SQLite WAL via `rusqlite`
- **Financial math:** `rust_decimal`
- **Async:** `tokio`
- **Config:** TOML, git-versioned
- **Logging:** `tracing` + `tracing-subscriber` + `tracing-appender`

### 12.2 Observability

Every signal -> Governor (risk mesh + stress check) -> graduation -> order ->
fill path carries a correlation span via `tracing`. The TUI debug panel (`C-d`)
shows recent trade lifecycle spans with timing per step. Stress multiplier
changes are traced as span events with the override justification text.
Flameout stop tightening events carry their own span with before/after stop
values.

### 12.3 Watchdog

`systemd` watchdog integration. TALON calls `sd_notify("WATCHDOG=1")` every
10 seconds. `WatchdogSec=30`. On timeout: systemd kills and restarts.
Reconciliation state machine (S9.3) handles position recovery on restart.

Supplementary: `talon-panic.sh` -- external emergency flatten script.
Authenticates directly to broker(s), submits market close-all, sends SMS.
Tested monthly on paper account.

### 12.4 Emergency Procedures

| Scenario | Automated | Manual Backup |
|---|---|---|
| Cobra drops, reconnects <60s | Auto-reconnect, reconcile | None needed |
| Cobra drops, no reconnect >60s | Exit-only mode, push notification | Cobra web: das.cobratrading.com |
| Cobra drops with open shorts | Push: "OPEN SHORTS -- COBRA DOWN", retry 10s x 30 | Cobra desk: **512-850-5022** |
| Total Cobra failure | Viable modules to IBKR. ParaShort dead. | Contact Cobra directly |
| IBKR TWS gateway crash | Auto-reconnect via Client Portal REST fallback | IBKR web trader |
| System crash with open positions | Reconciliation on restart (S9.3) | `talon-panic.sh` |
| System crash during 0DTE (after 3:30 PM) | `talon-panic.sh` triggered by watchdog timeout | Broker phone desk |

---

## 13. Build Plan

### Phase 0 -- Core Trading (Current)

- BrokerCommands + BrokerStreams traits
- IBKR implementation (TWS API)
- Firebird module (paper)
- Thunderbird module (paper)
- Taxi module (paper)
- CashCache Vault module (simulation)
- Supervised Autonomy supervision
- Governor with risk mesh + stress multiplier + Flameout protocol
- Position reconciliation state machine
- Watchdog (systemd)
- Observability (tracing)
- TUI (Ratatui) -- all five zones + stress indicator + Cruising Altitude
- SQLite persistence (event log, config)
- EventStore on dedicated writer thread

**Exit criteria:** All Hatch modules on IBKR paper. CashCache Vault
simulating harvests correctly. TUI operational with stress multiplier visible
and engaging under simulated drawdown. Flameout and Nosedive tested under
simulated conditions. Reconciliation tested against crash scenarios (S9.3
resolution table). Kill switch (EJECT) tested.

### Phase 1 -- Validation

- IBKR small-live ($500-$2K): Firebird, Thunderbird, Taxi
- CashCache Vault live harvesting (settled cash only)
- Alpaca BrokerCommands implementation (equities only -- validates trait)
- Graduation system (gates + proposals)
- Austin investor presentation prep from live data

### Phase 2 -- Takeoff Tier

- Dual-Control supervision model
- Snapback module (paper then live)
- Climb module (paper then live)
- Trust calibration (Phase 2 trust ledger)
- IBKR full-live ($25K+)

### Phase 3 -- Turbo Tier

- Cobra BrokerCommands + BrokerStreams (DAS TAPI)
- Multi-broker session manager
- SAGE, ParaShort, Siphon, YoYo modules
- Forced cover protocol (live-tested on paper first)
- Cobra live validation ($30K+)

### Phase 4 -- Evolution

- Payload module (adaptive 0DTE fusion)
- Webull BrokerCommands (zero-cost on-ramp)
- CenterPoint/Clear Street REST integration (Turbo fallback)

External system integration (Blackbird, MarketPIREP) is not on this roadmap.
Those systems ship on their own timelines and plug in via S10 ports when ready.

---

## 14. 3-Month Validation Plan (Austin)

**Month 1-2:** IBKR only. Build one broker well. Deploy Hatch modules on
paper. CashCache Vault simulation. Battle-test edge cases: partial fills,
connection drops, out-of-sequence events, crash recovery, stress multiplier
engagement under simulated drawdown, Flameout protocol under simulated stress
tier 3. TUI operational for daily use with Cruising Altitude detection working.

**Month 3:** IBKR small-live ($500-$2K). Alpaca implementation (validates trait
abstraction). CashCache Vault first real harvests. Cobra DAS TAPI trial (paper
only). Presentation prep from live data.

**Post-Austin:** Alpaca small-live. Cobra paper validation.

---

## 15. Configuration Structure

```
talon/
+-- config/
|   +-- system.toml             # Tiers, regulatory, paths, system modes
|   +-- brokers/
|   |   +-- ibkr.toml
|   |   +-- alpaca.toml
|   |   +-- cobra.toml          # Includes emergency contacts
|   |   +-- webull.toml
|   +-- modules/
|   |   +-- firebird.toml
|   |   +-- thunderbird.toml
|   |   +-- taxi.toml
|   |   +-- cashcache.toml
|   |   +-- snapback.toml
|   |   +-- climb.toml
|   |   +-- sage.toml
|   |   +-- parashort.toml
|   |   +-- siphon.toml
|   |   +-- yoyo.toml
|   |   +-- payload.toml
|   +-- risk.toml               # Per-tier risk + module allocations + stress + flameout
|   +-- graduation.toml         # Gates, track record thresholds
+-- data/
|   +-- events.db               # Event-sourced log (includes trust ledger)
|   +-- market.db               # Market data archive
|   +-- sovereign.db.enc        # Encrypted sovereign data
+-- backups/
    +-- daily/
```

---

## 16. Appendix

### 16.1 Broker Affinity Matrix

| Module | IBKR Cash | IBKR Margin | Alpaca | Webull | Cobra | CenterPoint |
|---|---|---|---|---|---|---|
| Firebird | Primary | Yes | Limited | Limited | Yes | Yes |
| Thunderbird | Primary | Yes | Limited | Limited | Yes | Yes |
| Taxi | Yes | Yes | Primary | Primary | Yes | Yes |
| CashCache Vault | Always | Always | No | No | No | No |
| Snapback | No | Primary | No | No | Yes | Yes |
| Climb | No | Primary | No | No | Yes | Yes |
| SAGE | No | Yes | No | No | Primary | Fallback |
| ParaShort | No | No | No | No | Primary | Fallback |
| Siphon | No | Preview | No | No | Primary | Fallback |
| YoYo | No | Yes | No | No | Primary | Fallback |
| Payload | No | No | No | No | Primary | Fallback |

### 16.2 Options Approval Level Requirements

| Module | Required Level |
|---|---|
| Firebird, Thunderbird | Level 2 (Standard) -- long calls/puts |
| Siphon (Hatch preview) | Level 1 (Limited) -- cash-secured puts |
| Siphon (full at Turbo) | Level 2 (Standard) -- verticals, calendars |
| SAGE | Level 2+ depending on strategy |
| Snapback, YoYo, Payload | Level 2 (Standard) -- long 0DTE |

Checked at startup. Insufficient level disables module with actionable message
("Request Level 2 options approval from IBKR to enable Siphon").

### 16.3 Brand Hierarchy

```
CashCache (brand / company)
+-- TALON Terminal (the trading system -- this spec)
|   +-- Hatch / Takeoff / Turbo (tiers)
|   +-- Firebird, Thunderbird, Taxi, Climb, Snapback, etc. (modules)
|   +-- CashCache Vault (profit preservation -- the brand's namesake)
+-- MarketPIREP (social sentiment platform -- separate spec)
|   +-- Sky Spotters (panelists)
|   +-- The Runway (discussion queue)
|   +-- Atop the Horns / Beneath the Claws (bull/bear consensus)
|   +-- Holding Pattern (low-confidence quarantine)
|   +-- Boneyard (discarded signals)
|   +-- Grounded (demoted panelists)
+-- Blackbird (intelligence & compute layer -- separate spec)
    +-- RAMjet (compute dispatch: CPU -> GPU -> FPGA)
    +-- Chine (combinatorial overlap discovery)
    +-- Spike (belief fusion: regime + sentiment + overlap -> unified state)
```

### 16.4 Aviation Naming Convention

The CashCache ecosystem uses an aviation metaphor throughout. TALON (the
terminal) is the bird of prey -- precise, autonomous within instinct,
overridable by the handler. System tiers follow the flight lifecycle: Hatch
(the bird emerges -- first capital, first trades), Takeoff (wheels leave the
ground -- $25K margin, intraday unlocked), and Turbo (afterburner -- full
module suite at Cobra). Modules use names that describe their action: Taxi
(ground movement before takeoff), Climb (altitude gain on momentum), Snapback
(recoil), Siphon (steady extraction), YoYo (up-and-down), Payload (the final
delivery). MarketPIREP uses aviation reporting terminology (PIREP = Pilot
Report). Blackbird references the SR-71 -- the intelligence platform that flies
at altitude above the tactical layer. Its subsystems are named after SR-71
components: RAMjet (the J58 turbo-ramjet engine that transitions from turbojet
to ramjet at speed -- compute dispatch transitions from CPU to GPU/FPGA at
demand), Chine (the sharp fuselage edges that generated unexpected lift from
vortices -- combinatorial discovery of unexpected patterns), and Spike (the
movable inlet cone that continuously adjusts to compress air optimally --
belief fusion that continuously adjusts to compress multiple data streams into
actionable intelligence).

System states use aviation terminology: Cruising Altitude (stable flight,
operator can step away), Flameout (engine failure protocol -- trailing stops
tighten as the system loses thrust), Nosedive (max drawdown -- the circuit
breaker trips), EJECT (kill switch -- flatten everything immediately),
Turbulence (future: volatility-regime classifier mode detected by Blackbird
Spike).

### 16.5 Regulatory Future-Proofing

**FINRA PDT reform (SR-FINRA-2025-017):** Filed Jan 2026. Projected mid-2026
approval. If PDT $25K minimum is replaced with risk-based intraday margin, the
Hatch -> Takeoff boundary shifts. All tier thresholds are in `system.toml`, not
hardcoded. CashCache Vault becomes more critical under dynamic margin (vault
survives margin calls on active account).

**Wash sale tracking:** Cross-ticker match via `WashGroup::ETFFamily`. The 30-
day window is defined by IRC Section 1091 and is unlikely to change, but the
list of "substantially identical" securities is config (not hardcoded).

### 16.6 Glossary

| Term | Definition |
|---|---|
| TALON | Trade Autonomously with Limited Override Necessity. The terminal. |
| CashCache | Brand name. Also the profit-preservation vault module. |
| CashCache Vault | Profit-preservation vault. Separate IBKR sub-account. |
| Hatch | Tier 1. $500-$25K. Cash/margin. Supervised Autonomy. The bird emerges. |
| Takeoff | Tier 2. $25K+. Margin. Dual-Control. Intraday unlocked. The system is airborne. |
| Turbo | Tier 3. $30K+. Margin at Cobra/CenterPoint. Full module suite. Afterburner engaged. |
| Governor | Behavioral containment layer between modules and execution. Applies risk mesh + stress multiplier + graduation. |
| Stress Multiplier | Dynamic risk contraction. Scales all limits by 0.0-1.0 based on drawdown depth. |
| Flameout | Governor protocol. Trailing stop tightening at stress multiplier x0.25. The engine coughing before it dies. |
| Nosedive | Event: circuit breaker tripped, max drawdown hit. Stress multiplier at x0.0. Exit-only until acknowledged. |
| Cruising Altitude | System mode: only swing/vault positions active, no intraday modules running. Operator can step away. |
| EJECT | Kill switch. Single keystroke (`K`). Flattens all positions via market orders. |
| Climb | Intraday momentum breakout module (Takeoff tier). Formerly Accelerader. |
| Taxi | Equity swing loader. Bridge from $500 cash to $25K margin. |
| Snapback | Mean-reversion 0DTE module. SPX/SPY. |
| YoYo | Binary event 0DTE module. FOMC/CPI/NFP. |
| Payload | Adaptive 0DTE fusion. Snapback + YoYo combined. Final unlock. |
| Siphon | Theta decay farming. Credit spreads, iron condors. |
| Forced Cover | Autonomous short-position risk breaker. Only code path bypassing Dual-Control. |
| Turbulence | Future: volatility-regime classifier mode. Detected by Blackbird Spike. |
| BrokerCommands | Sync trait for order lifecycle. Runs on blocking thread pool. |
| BrokerStreams | Async trait for streaming connections. Drop handle = cancel. |
| IntelligencePort | Generic trait for external system integration. `latest() -> Option<&S>`. |
| Blackbird | External intelligence ecosystem. RAMjet (compute), Chine (overlap), Spike (belief fusion). |
| Blackbird RAMjet | Compute dispatch subsystem. CPU -> GPU -> FPGA. Named for the SR-71 J58 turbo-ramjet. |
| Blackbird Chine | Combinatorial overlap discovery subsystem. Named for SR-71 fuselage chines. |
| Blackbird Spike | Belief fusion subsystem. Regime + sentiment + overlap -> unified state. Named for SR-71 inlet spike. |
| MarketPIREP | External social sentiment platform. Sky Spotters, The Runway, Holding Pattern. |
| Sky Spotters | MarketPIREP panelists who provide sentiment signals. |
| The Runway | MarketPIREP discussion queue. Signals line up before broadcast. |
| Holding Pattern | MarketPIREP: low-confidence data. Not rejected, not cleared. |
| Boneyard | MarketPIREP: discarded signals. |
| Grounded | MarketPIREP: demoted Sky Spotter. |
| Sovereign Data | Positions/trades/identity with compile-time access controls. |
| Gravity Well | CashCache Vault transfer threshold preventing micro-transfers. |
| Ghost Harvest | Bug: harvesting unsettled P&L. Prevented by settled cash gating. |
| PDT | Pattern Day Trader. FINRA $25K minimum for >3 day trades / 5 rolling days. |
| Wash Group | Set of substantially identical securities (e.g., SPY/VOO/IVV/SPLG). |

---

## Revision History

| Version | Date | Changes |
|---|---|---|
| 1.0.0-draft | 2026-03-01 | Initial spec with Conductor, PRESSbox, LOOM bundled inline. |
| 1.1.0 | 2026-03-01 | Audit response. BrokerGateway split, reconciliation, forced cover, CashCache defense, watchdog, expanded brokers, TUI philosophy. |
| 2.0.0 | 2026-03-01 | Decoupled external systems into separate specs. Introduced IntelligencePort. Consolidated audit fixes. |
| 3.0.0 | 2026-03-01 | Full rebrand: TR1M -> TALON. TrEStL -> Taxi. MR0DTEAN -> Snapback. BR0DTEAN -> YoYo. DR0DTEAN -> Payload. Range R0LEx -> Siphon. Conductor -> Blackbird Core. LOOM -> Blackbird Recon. PRESSbox -> MarketPIREP. Added stress multiplier (S7.3) with graduated drawdown contraction, override cooldown, and mandatory justification logging. Added S2.4 architectural rule. Supervision-on-timeout defaults (S7.6). Trust exclusions (S8.6). Module subsections with Thesis/Inputs/Outputs/Constraints/Failure per spec-structure.md. Brand hierarchy (S16.3) and aviation naming (S16.4). Options approval validation (S16.2). ASCII-clean per project standard. |
| 3.1.0 | 2026-03-02 | Accelerader -> Climb. Blackbird Core -> RAMjet, Blackbird Recon -> Chine, Blackbird Fuse -> Spike (SR-71 component naming). Added Flameout protocol (S7.4): trailing stop tightening at stress x0.25. Added Nosedive (S7.5): circuit breaker event with full-screen TUI takeover. Added Cruising Altitude (S11.10): Governor-detected safe-to-step-away mode. Added EJECT as kill switch label. Turbulence banked as future volatility-regime classifier mode. Expanded aviation naming convention (S16.4) with SR-71 subsystem rationale. Updated all cross-references, TUI mockups, and glossary. |
| 3.2.0 | 2026-03-02 | Tier renames: Jumpstart -> Hatch, DC -> Takeoff, CDL -> Turbo. Flight lifecycle naming: the bird hatches, taxis, takes off, climbs, hits afterburner. Updated all tier references, module map, graduation gates, broker table, risk parameters, supervision model comments, build plan phases, config, brand hierarchy, aviation naming, and glossary. |

---

*TALON ships without Blackbird, without MarketPIREP. It delivers full value on
CPU, without sentiment, without combinatorial discovery, without belief fusion.
Those systems plug in later via IntelligencePort when they are ready. The stress
multiplier protects the trader from their worst moments. Flameout tightens the
stops before the crash. The CashCache Vault proves, visually, every single day,
that the system works.*
