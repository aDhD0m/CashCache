# Supervision Models for Semi-Autonomous Trading

## The Spectrum

Trading systems exist on a spectrum from fully manual to fully autonomous. The most dangerous position is the middle: systems that are autonomous enough to cause harm but not sophisticated enough to prevent it. The graduated autonomy pattern manages this by starting conservative and earning trust.

```
MANUAL <--------------------------------------> AUTONOMOUS

  Human does       Human sets       Bot proposes,
  everything       boundaries,      human approves
                   bot executes     (Dual-Control)
                   within them
                   (Supervised
                   Autonomy)

  [Not in TR1M]    [Jumpstart]      [DC / CDL]
```

## Supervised Autonomy

**When:** Swing-length strategies, lower capital, slower failure modes.

The operator defines boundaries:
- Max position size (% of account)
- Max total exposure (% of account)
- Max concurrent positions
- Drawdown circuit breaker (% from HWM)
- Daily loss limit

The bot operates freely within boundaries. No per-trade approval. Appropriate when strategies are multi-day (time to notice and react), positions are defined-risk (max loss known at entry), and the capital at risk is small relative to the operator's total wealth.

```rust
pub struct BoundaryConfig {
    pub max_position_pct: Decimal,
    pub max_exposure_pct: Decimal,
    pub max_concurrent: u32,
    pub drawdown_breaker_pct: Decimal,
    pub daily_loss_limit_pct: Decimal,
}

pub fn check_boundaries(
    intent: &OrderIntent,
    portfolio: &Portfolio,
    config: &BoundaryConfig,
) -> Result<(), BoundaryViolation> {
    // Check all boundaries. Any violation = reject.
    // No partial violations. No "close enough" exceptions.
}
```

## Dual-Control

**When:** Intraday strategies, higher capital, faster failure modes.

Bot proposes, human approves. Every order intent is surfaced in the TUI with full context (signal confidence, risk parameters, position size, stop/target levels). The operator can:
- **Approve** -- order submitted as-is
- **Modify** -- adjust size, stop, or target before approving
- **Reject** -- signal discarded, logged for trust calibration

This is the default for new modules at any tier. A module earns less oversight through trust calibration.

### Exceptions to Dual-Control

Two categories of action bypass approval:

1. **Exit orders for existing positions** (stop-loss, target, time stop): These were implicitly approved when the entry was approved. The exit parameters were visible at entry time. Re-approving each exit adds latency without safety benefit.

2. **Forced cover on short positions**: This is a risk circuit breaker. The position has theoretically unlimited loss. Waiting for human approval when the position is moving against by 10%+ defeats the purpose.

Both exceptions must be fully specified: order type, price protection, fallback behavior, operator notification, event logging. See the trading-system-spec domain skill for forced cover protocol details.

## Trust Calibration (Graduated Autonomy)

Trust is earned per (module, regime, action class) tuple. 100 consecutive approvals with no rejections, across diverse market conditions, qualifies a module for auto-execution in that regime for that action class.

### Trust Entry Structure

```rust
pub struct TrustEntry {
    pub approvals: u32,
    pub rejections: u32,
    pub unique_days: HashSet<Weekday>,
    pub vix_buckets: HashSet<VixBucket>,
    pub time_buckets: HashSet<TimeBucket>,
}

pub enum VixBucket { Low, Normal, Elevated, High }  // <15, 15-20, 20-30, 30+
pub enum TimeBucket { Open, MidMorning, Lunch, Afternoon, Close }
```

### Diversity Requirements

Prevent overfitting to a narrow market condition:

- At least 4 unique days of the week (prevents "only works on Mondays" bias)
- At least 2 VIX buckets (prevents "only works in calm markets" bias)
- At least 2 time buckets (prevents "only works at open" bias)

### Trust Escalation

When qualifications are met, the TUI surfaces an upgrade proposal:

```
+- TRUST UPGRADE PROPOSAL --------------------------+
| Accelerader qualifies for auto-trust in TRENDING   |
|                                                     |
| Evidence: 100 approvals, 0 rejections               |
| Conditions: Mon-Fri, VIX 12-28, all time buckets    |
|                                                     |
| [GRANT AUTO-TRUST]  [NOT YET]  [DETAILS]            |
|-----------------------------------------------------+
```

The operator must explicitly grant auto-trust. It is never automatically applied.

### Trust Revocation

Any rejection after auto-trust is granted causes immediate revocation for that (module, regime, action class) tuple. The counter resets to zero. Trust must be re-earned from scratch.

This is intentionally harsh. A single rejection means the operator saw something the system didn't. The system should not assume it was a one-time mistake -- it should prove itself again.

### Trust Is Regime-Conditional

100 approvals in TRENDING does not grant auto-trust in CRISIS. Different regimes have fundamentally different market dynamics. A module that performs well in trending markets may be catastrophic in a crisis.

The trust ledger is keyed by `(ModuleId, RegimeState, ActionClass)`. Each combination earns trust independently.

### What "No Regime System Connected" Means for Trust

If no regime detection system is connected (`IntelligencePort<RegimeState>` returns None), all trust entries are keyed to `RegimeState::Standalone`. Trust earned in Standalone does not transfer when a regime system comes online -- the module must re-earn trust under each detected regime.

This prevents a dangerous shortcut: earning trust without regime awareness and then assuming that trust applies to all regimes.

## Kill Switch

Every supervision model includes a kill switch: a single-keystroke operation that flattens all positions immediately.

- Bypasses Dual-Control
- Bypasses trust calibration
- Submits market orders direct to broker
- Logs the event with timestamp and operator ID
- Cannot be undone (positions are closed, not paused)

The kill switch is the operator's ultimate override. Its existence is what makes graduated autonomy safe -- no matter how much trust the system has earned, the operator can always pull the plug instantly.

## Supervision Model Selection

| Strategy Characteristic | Recommended Supervision |
|---|---|
| Multi-day hold, defined risk, small capital | Supervised Autonomy |
| Intraday, larger capital, faster moves | Dual-Control |
| 0DTE options (extreme time decay) | Dual-Control Strict (no auto-trust) |
| Short selling (unlimited loss potential) | Dual-Control Strict (no auto-trust) |
| Profit preservation (CashCache) | Supervised Autonomy (boundary = harvest rules) |

"Strict" means the trust calibration system is disabled for that module. The operator must approve every trade, every time, regardless of track record. This is appropriate for strategies where a single bad trade can be catastrophic.
