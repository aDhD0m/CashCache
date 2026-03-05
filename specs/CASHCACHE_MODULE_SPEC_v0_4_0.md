# CashCache -- Module Engineering Specification

**Module:** CashCache  
**Tier:** TALON Hatch (Base Package)  
**Version:** 0.4.0  
**Author:** Atom / AX4  
**Date:** 2026-03-02  
**Classification:** Internal Engineering Reference  
**Revision:** Tier name alignment with TALON_ARCHITECTURE_v3.2.0. Jumpstart -> Hatch, DC -> Takeoff, CDL -> Turbo.  

---

## 1. Purpose

CashCache is an automated profit-preservation module that intercepts a configurable percentage of realized trading gains and routes them into a write-protected portfolio of stable instruments. Its function is to mechanically prevent the most common retail trading failure mode: winning, reinvesting everything, drawing down, and blowing up.

CashCache is not a trading strategy. It is an anti-fragility layer that grows in proportion to the trader's success and becomes increasingly difficult to destroy as the account matures.

---

## 2. Design Principles

- **Preservation over performance.** CashCache optimizes for capital protection, not returns. It buys boring assets on purpose.
- **Behavioral enforcement over behavioral suggestion.** The system does not "recommend" that users save profits. It does it automatically. Disabling requires deliberate action.
- **Impulse resistance.** Liquidating CashCache positions requires a multi-day confirmation protocol. There is no same-day withdrawal path.
- **Separation of concerns.** CashCache operates as a walled-off portfolio with its own view, its own accounting, and its own broker residency (separate IBKR sub-account). Active trading modules cannot access CashCache capital.
- **Structural anti-fragility.** Tax avoidance is prophylactic (wash sale group blocking), not reactive. Execution is context-aware (safe-list vs. custom tickers). Stop-losses are instrument-class-aware (indices ride through, individual names don't).

---

## 3. Profit Harvesting

### 3.1 Trigger Mechanism

CashCache harvesting is **account-type-dependent:**

**Cash Accounts (TALON Hatch, pre-$25K):**
- Harvesting parallels trade settlement.
- At end of each trading day, the system calculates net realized P&L from all closed positions.
- When those trades settle (typically between 0000-0400 EST, determined by the clearing house), a percentage of the previous day's positive net realized P&L is routed to the CashCache allocation queue.
- If net realized P&L for the day is zero or negative, no harvest occurs.
- Settlement timing is broker-dependent. The system queries settlement status via the `BrokerGateway` interface and triggers harvest only after confirmation.

**Margin Accounts (TALON Takeoff, TALON Turbo):**
- Harvesting occurs on **every closed winning trade**, immediately upon fill confirmation of the closing order.
- The harvest amount is calculated from the realized gain of that specific trade (not net daily P&L).
- Losing trades do not trigger any CashCache action (no clawback, no reduction).

### 3.2 Settled Cash Gating (Ghost Harvest Prevention)

**CRITICAL:** No harvest event triggers until the `BrokerGateway` confirms the capital is in settled, withdrawable status. This prevents "phantom" harvests from busted trades, late-day dividend adjustments, or pending corporate actions.

```rust
fn calculate_harvestable_amount(
    &self,
    broker: &dyn BrokerGateway,
    account_type: &AccountType,
    settlement_date: Option<NaiveDate>,
) -> Result<Decimal, CashCacheError> {
    match account_type {
        AccountType::Cash => {
            // Gate on settlement confirmation
            let settled = broker.settled_cash_delta(
                settlement_date.ok_or(CashCacheError::NoSettlementDate)?
            )?;
            Ok(settled.max(Decimal::ZERO))
        },
        AccountType::Margin => {
            // Gate on "Available for Withdrawal" delta
            let snapshot = broker.account_snapshot()?;
            let confirmed_gain = snapshot.available_for_withdrawal
                - snapshot.previous_available_for_withdrawal;
            Ok(confirmed_gain.max(Decimal::ZERO))
        },
    }
}
```

If a busted trade adjustment occurs after harvest, the system detects the discrepancy at the next reconciliation cycle and debits the CashCache cash buffer (not existing positions). If the buffer goes negative (harvest was already deployed), the next harvest cycle absorbs the deficit before any new deployment occurs. A negative buffer is logged as a critical warning.

### 3.3 Harvest Percentage

The user selects one of four harvest modes during CashCache setup. The mode can be changed at any time, but changes take effect on the next trading day (not retroactively).

**Mode 1: Fixed Percentage**
- User sets a fixed harvest rate (e.g., 20%).
- Enforced minimum: 5%. The system will not allow a harvest rate below 5% to prevent users from effectively disabling CashCache without explicitly turning it off.
- No upper bound enforced (user can set 50%+ if desired).

**Mode 2: Tiered -- Inverse Account Size (DEFAULT)**
- Harvest percentage decreases as the active trading account grows.
- Rationale: Small accounts need proportionally more protection. As the account matures, the trader has demonstrated competence and the active capital base needs room to compound.

| Active Account Value | Harvest Rate |
|---|---|
| $0 - $2,000 | 30% |
| $2,001 - $5,000 | 25% |
| $5,001 - $10,000 | 20% |
| $10,001 - $25,000 | 15% |
| $25,001 - $50,000 | 12% |
| $50,001+ | 10% |

- Thresholds are evaluated at the time of each harvest event based on the active account's net liquidation value (excluding CashCache).
- The enforced minimum rate is 5%, regardless of account size. Users can customize the tier thresholds and rates but cannot set any tier below 5%.

**Mode 3: Tiered -- Win Streak**
- Harvest percentage increases with each consecutive winning trade.
- Rationale: Addresses the momentum trading failure mode where a hot streak leads to overconfidence, oversizing, and a catastrophic loss. The system automatically diverts more capital to safety as the streak extends.

| Consecutive Wins | Harvest Rate |
|---|---|
| 1 (single win) | 10% |
| 2 | 15% |
| 3 | 20% |
| 4 | 25% |
| 5+ | 30% |

- A losing trade resets the streak counter to 0.
- A break-even trade (within a configurable tolerance, default $1) does not reset the streak but does not increment it.
- The enforced minimum rate is 5%.

**Mode 4: Off / Disabled**
- CashCache does not harvest any profits.
- Existing CashCache positions are retained and remain write-protected.
- The multi-day liquidation protocol still applies to any existing holdings.
- The system logs a warning on each trading session that CashCache is disabled.

### 3.4 Harvest Calculation Examples

**Cash Account, Inverse Account Size mode, $3,000 account:**

```
Day's closed trades:
  AAPL 250C: +$180
  MSFT 420P: -$60
  Net realized P&L: +$120

Tier: $2,001-$5,000 -> 25% harvest rate
Harvest amount: $120 x 0.25 = $30.00
$30.00 added to CashCache allocation queue at settlement
(only after BrokerGateway confirms settled cash)
```

**Margin Account, Win Streak mode, 3rd consecutive win:**

```
Closed trade:
  NVDA momentum breakout: +$450

Streak: 3 -> 20% harvest rate
Harvest amount: $450 x 0.20 = $90.00
$90.00 added to CashCache allocation queue immediately
(only after BrokerGateway confirms available_for_withdrawal delta)
```

---

## 4. Destination Instruments

### 4.1 Pre-Approved Safe List

CashCache maintains a system-curated list of instruments eligible for purchase. The default list includes:

**Broad Market ETFs:**
- SPY (S&P 500)
- QQQ (Nasdaq 100)
- VTI (Total US Stock Market)
- VOO (Vanguard S&P 500)
- IVV (iShares S&P 500)

**Dividend / Stability ETFs:**
- SCHD (Schwab US Dividend Equity)
- VIG (Vanguard Dividend Appreciation)
- DVY (iShares Select Dividend)

**Bond / Fixed Income ETFs:**
- BND (Vanguard Total Bond Market)
- AGG (iShares Core US Aggregate Bond)
- TLT (iShares 20+ Year Treasury)

**Blue Chip Equities:**
- AAPL, MSFT, GOOG, AMZN, JNJ, PG, KO, JPM, BRK.B, UNH

The safe list is maintained as a configuration file and can be updated by the system operator (not the end user) to add or remove instruments based on market conditions, delistings, or corporate events.

### 4.2 User Customization

- Users may add custom tickers to their CashCache portfolio beyond the safe list.
- Custom additions are subject to the following constraints:
  - **Concentration limit at purchase time:** No single instrument may exceed 30% of total CashCache portfolio value at the time of a new purchase into that instrument. If a purchase would breach this limit, the system redirects the allocation to the user's next-priority instrument or holds it in the CashCache cash buffer.
  - **Diversification warning:** If the user's CashCache portfolio contains fewer than 3 distinct instruments, the system displays a persistent warning. This is advisory only -- not enforced.
  - **No options, no leveraged/inverse ETFs, no OTC/penny stocks.** Custom tickers must be exchange-listed equities or ETFs with a market cap above $1B. The system validates against these criteria at addition time.
- Users may remove instruments from their personal CashCache watchlist at any time. Removing an instrument does not trigger a sale -- it only prevents future purchases.

### 4.3 Allocation Priority

When a harvest event produces capital for deployment, the system allocates across the user's selected instruments using a **round-robin weighted distribution:**

- The user assigns a weight (percentage) to each instrument in their CashCache portfolio. Weights must sum to 100%.
- Default for new users: equal-weight across SPY, QQQ, and SCHD (33.3% each).
- Allocation respects the concentration limit (tier-dependent, see S9.1). If round-robin would breach the limit on a specific instrument, excess is redistributed to remaining instruments proportionally.

---

## 5. Prophylactic Wash Sale Avoidance

### 5.1 Problem Statement

CashCache defaults to SPY and QQQ -- the same instruments users trade 0DTE options on in TALON Takeoff modules (Snapback, YoYo). If a trader loses $500 on SPY 0DTE options and CashCache buys SPY shares within 30 days, the loss is disallowed under IRS wash sale rules. Over a year of high-frequency 0DTE trading, the user could face a massive tax bill on "phantom" gains from constantly deferred losses.

### 5.2 Solution: Substantially Identical Group Blocking

Rather than tracking wash sales after the fact, CashCache prevents them from occurring by blocking purchases of instruments that are "substantially identical" to anything recently traded in the active account.

**Active Symbol Blocklist:**
- The system maintains a rolling 30-day window of all instruments traded in the active account (equities and options, including the underlying of any traded options).
- Any instrument on this list is blocked from CashCache purchase.

**Substantially Identical Groups:**
- Blocking is group-aware. If any member of a group is actively traded, ALL members of that group are blocked from CashCache purchase.

```rust
/// Substantially identical instrument groups for wash sale avoidance.
/// If ANY member of a group is actively traded, ALL members are blocked.
pub static WASH_SALE_GROUPS: &[&[&str]] = &[
    &["SPY", "VOO", "IVV", "SPLG"],        // S&P 500 trackers
    &["QQQ", "QQQM"],                       // Nasdaq 100 trackers
    &["VTI", "ITOT", "SPTM"],              // Total US market trackers
    &["BND", "AGG", "SCHZ"],               // US aggregate bond trackers
    &["TLT", "VGLT", "SPTL"],             // Long-term treasury trackers
    &["SCHD", "VIG", "DVY", "DGRO"],      // Dividend equity trackers
];
```

**Substitution Logic:**
- When CashCache attempts to purchase a blocked instrument, the system does NOT auto-substitute another member of the same group (because the entire group is blocked).
- Instead, allocation weight is redistributed to the next non-blocked instrument in the user's weighted list.
- If ALL of the user's weighted instruments are blocked, capital remains in the CashCache cash buffer. The user is warned: "All your CashCache instruments are currently blocked due to active trading in related securities. Your harvest will be held in cash until a clean instrument becomes available."

**Individual Equities:**
- Individual equities (AAPL, MSFT, etc.) do not have substitution groups. If AAPL is blocked, allocation skips to the next weighted instrument. No attempt to substitute AAPL with another tech stock -- individual stocks are never "substantially identical" to each other for IRS purposes.

### 5.3 User Warning

During CashCache setup, the system displays: "CashCache automatically avoids purchasing instruments you've recently traded to prevent wash sale conflicts. If you actively trade broad market ETFs (SPY, QQQ, etc.) and their equivalents, CashCache will hold your harvest in cash until a clean instrument is available."

### 5.4 Cross-ETF Trading Edge Case

If a user trades both SPY options *and* VOO options in their active account, the entire S&P 500 tracker group is blocked regardless. The group-blocking mechanism is inherently recursive -- there is no substitution chain to break because substitution never moves within a blocked group.

Users who actively trade across multiple ETF families may find that CashCache defaults to individual blue chip equities and bond ETFs as the only available purchase targets. This is an acceptable outcome -- it forces diversification across asset classes when the user's active trading is concentrated in equity indices.

---

## 6. Purchase Mechanism

### 6.1 Execution Strategy

CashCache uses context-aware execution based on instrument liquidity:

**Safe List Instruments (SPY, QQQ, SCHD, VOO, IVV, etc.):**
- Execution strategy: `MarketImmediate`
- Market orders. These instruments have penny-wide spreads and billions in daily volume. Limit order complexity is not justified.

**Custom Tickers (user-added, off safe list):**
- Execution strategy: `PassiveAggressive`
- Limit order at NBBO midpoint + 0.3%, valid for 30 minutes.
- If unfilled after 30 minutes, convert to market order.
- If the 30-minute window expires during a halt, cancel and retry next session.

```rust
pub enum ExecutionStrategy {
    /// Market order, immediate. For highly liquid safe-list instruments.
    MarketImmediate,
    /// Limit at midpoint + tolerance, convert to market after timeout.
    /// For custom tickers with potentially thin liquidity.
    PassiveAggressive {
        tolerance_bps: u32,    // default: 30 (0.3%)
        timeout_minutes: u32,  // default: 30
    },
}
```

### 6.2 Hybrid Deployment

When the CashCache allocation queue contains deployable capital (>= $10 minimum threshold), the system executes a hybrid purchase:

1. **Immediate buy (50% of allocation):** Executed during the next regular trading session (0930-1600 EST) using the instrument's assigned execution strategy. If allocation occurs overnight, the order queues for market open.
2. **DCA remainder (50% of allocation):** Split into equal daily purchases over the following N trading days (N is tier-dependent, see S9.1). Each daily tranche is executed within the first 30 minutes of the trading session.

### 6.3 Minimum Allocation Threshold

- The minimum deployable amount is **$10**.
- If a harvest event produces less than $10, the amount is held in the **CashCache cash buffer** -- an uninvested cash balance within the CashCache sub-account.
- The cash buffer is checked at each subsequent harvest event. If buffer + new harvest >= $10, the combined amount is deployed.
- The cash buffer earns no interest and is not invested. It is simply a queue.

### 6.4 Fractional Shares

- CashCache uses fractional share purchases where the broker supports them (IBKR supports fractional trading on most listed equities and ETFs).
- If a user-added custom ticker does not support fractional purchases at the broker, the system queues capital in the cash buffer until a whole share is affordable. The user is warned at the time of adding the ticker.

### 6.5 Order Failures

| Failure | Behavior |
|---|---|
| Market/limit order rejected by broker | Retry once at next session. If retry fails, return to cash buffer + notify user. |
| Instrument halted | Skip instrument, allocate to next in weight order. Retry halted instrument next session. |
| Insufficient funds (cash buffer discrepancy) | Halt purchases, reconcile cash buffer with broker balance. Log critical error. |
| Fractional shares unavailable for instrument | Queue in cash buffer until whole share affordable. Warn user. |
| PassiveAggressive limit unfilled + market conversion fails | Return to cash buffer. Notify user. Log execution failure with market conditions. |

---

## 7. Write Protection & Liquidation Protocol

### 7.1 System-Level Write Protection

- No active trading module (Firebird, Thunderbird, Climb, SAGE, ParaShort, Siphon, Snapback, YoYo, Payload) may access, liquidate, borrow against, or reference CashCache positions or cash buffer for any purpose.
- This is enforced at the **broker infrastructure level**: CashCache resides in a separate IBKR sub-account (see S8). Active modules authenticate against a different account. The separation is physical, not logical.
- Active modules literally cannot construct an order that touches CashCache holdings because they do not have credentials to the CashCache sub-account.

### 7.2 Catastrophic Auto-Liquidation

- The system may auto-liquidate CashCache positions **only** under catastrophic conditions:
  - The active trading account receives a margin call that cannot be satisfied by the active account's own liquidation AND both accounts are at the same broker with linked margin.
  - This scenario should be rare-to-impossible under the default configuration (S8.4 Collateral Opt-Out), which isolates CashCache from margin calculations.
- If triggered, the system logs a critical alert and pauses all active trading until human review.

### 7.3 Human Liquidation -- Multi-Day Confirmation Protocol

There is **no same-day liquidation path** for CashCache positions. The following protocol is the only mechanism for voluntary withdrawal:

**Day 1 -- Request Submission (any time):**
1. User navigates to the CashCache management interface.
2. User specifies the liquidation scope using one of the following:
   - Percentage of total CashCache portfolio (e.g., "liquidate 25%")
   - Specific shares of a specific instrument (e.g., "sell 10 shares of SPY")
   - Percentage of a specific instrument (e.g., "sell 50% of my AAPL position")
   - Total liquidation ("liquidate all")
3. User types a written confirmation (not a checkbox -- actual text input, e.g., "I confirm I want to liquidate 25% of my CashCache portfolio").
4. The request enters **PENDING** state. No orders are submitted.
5. System displays: "Liquidation request submitted. You must reconfirm between Day 2 0400 EST and Day 3 2000 EST, or this request will be automatically cancelled."

**Day 2-3 -- Extended Reconfirmation Window (Day 2 0400 EST through Day 3 2000 EST):**
1. The system presents the pending liquidation request with full details (instruments, estimated quantities, estimated proceeds at current market prices).
2. User must actively reconfirm by pressing a confirmation button and re-entering their written confirmation.
3. Upon reconfirmation, the system queues sell orders for the next regular trading session using the appropriate execution strategy (MarketImmediate for safe list, PassiveAggressive for custom tickers).
4. If the user does not reconfirm by Day 3 2000 EST, the request is **automatically cancelled**. No partial execution. The system logs the cancellation.

**Rationale for 72-hour window (vs. original 16-hour Day 2 only):**
- Eliminates the single-point-of-failure where a medical emergency, travel, or internet outage during a single 16-hour window causes automatic cancellation.
- The 40-hour reconfirmation window spans two full trading days, providing multiple natural opportunities to act.
- The behavioral barrier (sleep on it, come back tomorrow) is preserved -- the user still cannot execute same-day.

**Post-Execution:**
- Liquidation proceeds are deposited into the **active trading account's cash balance**, not the CashCache cash buffer.
- The system logs the full liquidation event including: original request timestamp, reconfirmation timestamp, execution prices, proceeds, and the user's written confirmations.

### 7.4 Cooling Period

- After a **successful liquidation execution**, the user cannot submit another liquidation request for **48 hours** (2 full trading days).
- A cancelled request (missed reconfirmation window or user-cancelled) does **not** trigger the cooling period. The user can immediately re-submit.
- This prevents rapid sequential liquidations that would circumvent the spirit of the multi-day protocol.

---

## 8. Broker Residency & Transitions

### 8.1 Sub-Account Partitioning

CashCache resides in a **separate IBKR sub-account**, not a logical partition within the active trading account. This is a Day 1 architectural decision driven by three requirements:

1. **Margin isolation:** The IBKR risk engine cannot use CashCache equity as margin collateral for active trades unless explicitly configured otherwise.
2. **Ghost harvest prevention:** CashCache's cash balance is independently verifiable against the sub-account's settled cash, with no cross-contamination from active trading P&L.
3. **Cobra graduation:** When the active account moves to Cobra, the IBKR sub-account remains independently operational with no ACAT transfer required.

### 8.2 Broker Assignment Table

| User Tier | Active Account Broker | CashCache Broker | CashCache Account Type |
|---|---|---|---|
| Hatch | IBKR | IBKR | Separate sub-account |
| Takeoff | IBKR | IBKR | Separate sub-account |
| Takeoff/Turbo | Cobra Trading | IBKR | Independent account |

### 8.3 Dual BrokerGateway Sessions

When a user operates at Cobra for active trading:
- The TALON system maintains **two active `BrokerGateway` connections**: one to IBKR (CashCache operations) and one to Cobra (active trading operations).
- The `BrokerGateway` manager maintains a `HashMap<BrokerId, Box<dyn BrokerGateway>>` with independent authentication sessions.
- CashCache always addresses `broker_sessions[IBKR]` regardless of which broker the active modules are using.

### 8.4 Collateral Opt-Out

```toml
[cashcache.margin_isolation]
exclude_from_margin_calculation = true  # DEFAULT
# If true (default): CashCache sub-account equity does NOT contribute
# to active account buying power or margin calculations.
#
# If false: CashCache holdings may increase active account leverage.
# WARNING: This undermines the preservation mandate. The vault becomes
# collateral for the behavior it's designed to hedge. Enable only if
# you fully understand the implications.
```

This is enforced at the IBKR account configuration level. Linked sub-accounts can be configured with independent margin treatment.

### 8.5 Cobra-to-IBKR Transfer: Gravity Well Protocol

For TALON Takeoff/Turbo users at Cobra, harvest amounts accumulate in a **Cobra-side buffer** (logical, tracked by TALON, not a separate broker account) before transfer.

**Transfer trigger:**

```rust
fn should_transfer(cobra_buffer: Decimal, transfer_fee: Decimal) -> bool {
    let fee_threshold = transfer_fee / Decimal::new(5, 3); // fee < 0.5% of transfer
    let floor = Decimal::new(100, 0); // $100 minimum
    cobra_buffer >= fee_threshold.max(floor)
}
```

- The transfer fee must be less than 0.5% of the transfer amount, with a $100 floor to prevent micro-transfers.
- If ACH is free (some broker configurations), the $100 floor alone governs.
- The Cobra-side buffer is logged and visible in the CashCache view as "Pending Transfer: $X.XX (awaiting threshold)."
- Transfer method (ACH or wire) is user-configured during Cobra onboarding.
- The CashCache allocation queue holds harvested amounts until the transfer clears at IBKR, then deploys per the standard purchase mechanism.

---

## 9. Account Size Scaling

### 9.1 Scaling Parameters

| Parameter | $0-$2K | $2K-$10K | $10K-$25K | $25K-$50K | $50K+ |
|---|---|---|---|---|---|
| Default harvest mode | Inverse Account Size | Inverse Account Size | Inverse Account Size | Inverse Account Size | Inverse Account Size |
| DCA spread (days) | 3 | 5 | 5 | 7 | 10 |
| Concentration limit | 40% | 30% | 25% | 20% | 15% |
| Min instruments (warning) | 2 | 3 | 3 | 4 | 5 |
| Cash buffer max | $50 | $100 | $250 | $500 | $1,000 |

### 9.2 Cash Buffer Overflow

If the CashCache cash buffer exceeds the tier-appropriate maximum, the system force-deploys the excess on the next trading session using the standard hybrid purchase mechanism, regardless of the $10 minimum threshold.

### 9.3 Threshold Evaluation

- Scaling thresholds are evaluated against the **active trading account's net liquidation value**, not the CashCache portfolio value.
- Thresholds are re-evaluated at the start of each trading session (0930 EST). Mid-session account value changes do not trigger re-evaluation.

---

## 10. Visibility & Reporting

### 10.1 Separate Portfolio View

CashCache is displayed in a **completely separate interface panel** from the active trading portfolio. It is never commingled with active P&L, active positions, or active account metrics.

The CashCache view displays:
- **Total CashCache value** (positions + cash buffer + Cobra-side pending transfer if applicable)
- **Cash buffer balance** (uninvested, pending deployment)
- **Cobra-side pending transfer** (if user is at Cobra tier)
- **Position list:** instrument, quantity, cost basis, current value, unrealized P&L, holding period (days), wash-sale-blocked status
- **Harvest history:** chronological log of all harvest events with source trade, harvest mode, rate applied, and amount
- **Growth chart:** CashCache total value over time (simple line chart)
- **Pending liquidation requests** (if any)
- **Blocked instruments:** list of instruments currently blocked by wash sale avoidance, with expected unblock date (30 days from last active trade)

### 10.2 Active Portfolio Integration

The active trading interface displays a single summary line:
- "CashCache: $X,XXX.XX" -- total value, no breakdown, no positions.
- This line is non-interactive. Clicking it navigates to the full CashCache view.
- Active P&L calculations (daily, weekly, monthly) **exclude** CashCache entirely.

### 10.3 Combined View (Optional)

A combined "Total Account" view is available showing:
- Active account value + CashCache value = Total value
- This view is read-only and exists solely for the user to see their complete financial picture.
- It is never the default view. The user must deliberately navigate to it.

---

## 11. Dividend Handling

Dividend handling is **user-configurable** with two options:

**Option A: Reinvest within CashCache (DRIP) -- DEFAULT**
- Dividends received on CashCache holdings are retained within the CashCache sub-account.
- Dividend proceeds are added to the CashCache cash buffer and deployed via the standard purchase mechanism (hybrid: 50% immediate + 50% DCA).
- Reinvested dividends are allocated according to the user's current instrument weights (not necessarily back into the dividend-paying instrument).
- Wash sale group blocking applies to dividend reinvestment.

**Option B: Route to Active Trading Capital**
- Dividends received on CashCache holdings are transferred to the active trading account's cash balance.
- This option is useful for users who want CashCache to generate supplemental trading capital without liquidating positions.
- The transfer occurs on dividend payment date with no delay.

The user can switch between options at any time. Changes take effect on the next dividend payment.

---

## 12. Exit Strategy

### 12.1 Voluntary Exit

All voluntary exits follow the multi-day confirmation protocol defined in S7.3. There are no exceptions.

### 12.2 Stop-Loss: Instrument-Class Bifurcation

The system maintains **per-holding stop-losses** that vary by instrument class:

**Broad Market ETFs (SPY, QQQ, VTI, VOO, IVV, BND, AGG, TLT, SCHD, VIG, DVY, and all instruments in WASH_SALE_GROUPS):**
- **No stop-loss.** These instruments are held through any drawdown.
- Rationale: A 35% decline in SPY indicates a generational market event. SPY has recovered from every such event in history. Selling at the bottom is the opposite of the "buy and hold" thesis for diversified instruments.

**Individual Equities (AAPL, MSFT, GOOG, etc.):**
- **-35% stop-loss from cost basis.**
- Evaluated at market close each day, not intraday (prevents flash crash triggers).
- Executed at next market open using `PassiveAggressive` execution strategy (not market order -- liquidity may be thin on a -35% name).

**User-Added Custom Tickers:**
- **-25% stop-loss from cost basis.**
- Tighter threshold because custom tickers have not been vetted to the same standard as the safe list.
- Same evaluation and execution mechanics as individual equities.

### 12.3 Stop-Loss Mechanics

- The stop-loss is measured from **average cost basis per share**, not from any trailing high.
- When triggered:
  - The position is queued for liquidation at next market open.
  - Proceeds go to the **CashCache cash buffer** (not the active account).
  - The system logs a critical alert and notifies the user.
  - The liquidated instrument is temporarily removed from the user's allocation weights. The user must manually re-add it if they want to resume purchasing.
  - If the position was held less than 1 year, the system logs: "This sale will be treated as a short-term capital loss. Consult a tax professional."
- The stop-loss thresholds are system-enforced and not user-configurable. They are safety mechanisms, not trading parameters.

### 12.4 Full CashCache Shutdown

If the user disables CashCache entirely (Mode 4: Off/Disabled):
- No new harvests occur.
- Existing positions are retained and remain write-protected.
- The multi-day liquidation protocol remains active for existing positions.
- The stop-losses remain active.
- To fully unwind CashCache, the user must disable harvesting AND submit a "liquidate all" request through the multi-day protocol.

---

## 13. Tax Awareness

### 13.1 Scope

CashCache implements **lightweight tax awareness** -- enough to avoid obvious tax inefficiency, not enough to constitute tax optimization or advice.

### 13.2 Long-Term Capital Gains Preference

- The system tracks cost basis and holding period for every CashCache lot (specific identification method).
- CashCache's design inherently favors long-term capital gains treatment: positions are held indefinitely under normal operation, and the multi-day liquidation protocol discourages frequent selling.

### 13.3 Prophylactic Wash Sale Avoidance

See S5. CashCache prevents wash sales structurally rather than tracking them retroactively. The Active Symbol Blocklist and Substantially Identical Group system ensure CashCache never purchases an instrument that would trigger a wash sale with recent active trading activity.

### 13.4 Disclaimer

CashCache is not a tax advisor. The system does not provide tax advice, tax projections, or tax-optimized liquidation recommendations. Users are responsible for their own tax compliance. The system provides cost basis and holding period data for export to tax preparation software.

---

## 14. Initialization

### 14.1 First-Time Setup

When a user enables CashCache for the first time:

1. **Mode selection:** User selects a harvest mode (Fixed, Inverse Account Size, Win Streak, or Disabled). Default: Inverse Account Size.
2. **Instrument selection:** User reviews the pre-approved safe list and selects instruments + weights. Default: SPY 33.3%, QQQ 33.3%, SCHD 33.4%.
3. **Dividend handling:** User selects DRIP or route-to-active. Default: DRIP.
4. **Wash sale warning:** System displays the cross-ETF trading warning (S5.3).
5. **Optional seed:** The system calculates a recommended initial seed:
   - Recommendation: 5-10% of current active account net liquidation value.
   - Displayed as: "We recommend seeding CashCache with $X-$Y from your current balance to establish your safety net. You can skip this and let CashCache grow from future profits."
   - User can accept the recommendation, enter a custom seed amount, or skip (CashCache starts empty).
   - If the user seeds, the amount is immediately transferred to the CashCache sub-account and deployed via the standard hybrid purchase mechanism.

### 14.2 Existing Account Migration

For users enabling CashCache on an account with existing trading history:
- The system does not retroactively harvest from past trades.
- Only trades closed after CashCache activation are subject to harvesting.
- The optional seed mechanism (S14.1) is the only way to capitalize CashCache from existing funds.

---

## 15. Future-Proofing: FINRA PDT Rule Replacement

### 15.1 Context

As of January 2026, FINRA has filed proposed amendments (SR-FINRA-2025-017) to replace the $25,000 Pattern Day Trader minimum with a risk-based intraday maintenance-margin framework. The SEC published the filing in the Federal Register on January 14, 2026. Approval is projected for mid-to-late 2026, with implementation potentially in late 2026 or early 2027.

### 15.2 Impact on CashCache

**Tier boundary shift:**
- The current $25,000 PDT threshold is the hard gate between TALON Hatch and TALON Takeoff. If replaced with dynamic intraday margin, sub-$25K accounts may gain day trading access subject to risk-based buying power limits.
- CashCache's Inverse Account Size tiers (S3.3) are calibrated around the $25K boundary. Re-calibration may be required.

**Increased relevance at lower tiers:**
- Dynamic intraday margin means sub-$25K accounts face real-time margin calls, concentration-based leverage reduction, and potential forced liquidation during volatile sessions.
- CashCache becomes **more critical, not less**, in a post-PDT world. The write-protected vault directly addresses the new risk: even if a dynamic margin call wipes the active account, CashCache (in a separate IBKR sub-account) is untouched.

**Volatility-adjusted harvesting (v2):**
- If risk-based intraday margin tightens buying power on volatile names, a new harvest mode could increase the harvest rate when the active account is deploying into high-volatility or high-concentration positions.
- The data pipeline should capture intraday margin utilization metrics from day one to enable backtesting.

### 15.3 Architectural Preparation

1. **Tier thresholds are configuration, not code.** The $25K boundary is defined in a configuration file, not hardcoded.

2. **Harvest mode extensibility.** Implemented as a trait, not a fixed enum:

    ```rust
    pub trait HarvestMode: Send + Sync {
        fn calculate_harvest(
            &self,
            realized_gain: Decimal,
            account_snapshot: &AccountSnapshot,
            streak_state: &StreakState,
        ) -> Decimal;

        fn mode_name(&self) -> &str;
        fn min_rate(&self) -> Decimal; // enforced minimum, always >= 0.05
    }
    ```

3. **Margin event integration hook.** The `BrokerGateway` trait includes a margin event subscription channel. CashCache monitors for intraday margin warnings, maintenance deficiency alerts, and forced liquidation events. Currently only catastrophic auto-liquidation uses this channel; future versions can implement proactive responses.

4. **Buying power tracking.** The system logs intraday buying power utilization as a time series from day one, even before CashCache uses it.

5. **Regulatory configuration block:**

    ```toml
    [regulatory]
    pdt_rule_active = true
    min_day_trading_equity = 25000
    margin_framework = "fixed"  # "fixed" or "dynamic"

    [regulatory.dynamic_margin]
    # Populated when margin_framework = "dynamic"
    # Parameters TBD based on final SEC-approved rule text
    ```

6. **Collateral Opt-Out awareness.** If post-PDT risk-based margin engines evaluate total account equity across linked sub-accounts, the Collateral Opt-Out toggle (S8.4) must be verified against the broker's updated margin engine. This is monitored via the regulatory status dashboard.

---

## 16. Data Model

### 16.1 Event Sourcing

CashCache uses an **event-sourced** persistence model. The current state (`CashCacheState`) is a projection derived from an append-only event log. This ensures:
- Perfect state reconstruction from any point in history
- Immutable audit trail for all financial operations
- Recovery from database corruption by replaying events

```rust
/// All state-changing operations are recorded as immutable events.
pub enum CashCacheEvent {
    // Harvest lifecycle
    HarvestReceived(HarvestEvent),
    HarvestReconciled {
        harvest_id: Uuid,
        adjustment: Decimal,  // negative if busted trade
        reason: String,
    },

    // Purchase lifecycle
    PurchaseQueued {
        allocation_id: Uuid,
        instruments: Vec<(Symbol, Decimal)>,  // symbol, amount
        strategy: ExecutionStrategy,
    },
    PurchaseExecuted {
        allocation_id: Uuid,
        symbol: Symbol,
        quantity: Decimal,
        price: Decimal,
        fees: Decimal,
        timestamp: DateTime<Utc>,
    },
    PurchaseFailed {
        allocation_id: Uuid,
        symbol: Symbol,
        reason: String,
        returned_to_buffer: Decimal,
    },

    // Liquidation lifecycle
    LiquidationRequested(LiquidationRequest),
    LiquidationReconfirmed {
        request_id: Uuid,
        confirmation_text: String,
        timestamp: DateTime<Utc>,
    },
    LiquidationCancelled {
        request_id: Uuid,
        reason: CancellationReason,
    },
    LiquidationExecuted {
        request_id: Uuid,
        fills: Vec<(Symbol, Decimal, Decimal)>,  // symbol, qty, price
        total_proceeds: Decimal,
    },

    // Stop-loss
    StopLossTriggered {
        symbol: Symbol,
        instrument_class: InstrumentClass,
        cost_basis: Decimal,
        trigger_price: Decimal,
        threshold: Decimal,  // -0.35 or -0.25
    },
    StopLossExecuted {
        symbol: Symbol,
        quantity: Decimal,
        price: Decimal,
        proceeds: Decimal,
        holding_period_days: u32,
    },

    // Transfers (Cobra graduation)
    TransferInitiated {
        from_broker: BrokerId,
        amount: Decimal,
        method: TransferMethod,
    },
    TransferConfirmed {
        transfer_id: Uuid,
        settled_at: DateTime<Utc>,
    },

    // Dividends
    DividendReceived {
        symbol: Symbol,
        amount: Decimal,
        mode: DividendMode,
    },

    // Wash sale avoidance
    InstrumentBlocked {
        symbol: Symbol,
        group: Option<String>,
        blocked_until: DateTime<Utc>,
        reason: String,
    },
    InstrumentUnblocked {
        symbol: Symbol,
    },

    // Configuration
    ConfigChanged {
        field: String,
        old_value: String,
        new_value: String,
        timestamp: DateTime<Utc>,
    },
}
```

### 16.2 State Projection

```rust
/// CashCache current state -- always derived from fold(events).
/// A materialized view in SQLite caches this for fast reads.
/// If the projection is suspect, rebuild from events.
pub struct CashCacheState {
    pub portfolio_id: Uuid,
    pub user_id: Uuid,
    pub broker: BrokerId,                     // always IBKR in v1
    pub sub_account_id: String,               // IBKR sub-account identifier
    pub cash_buffer: Decimal,                 // uninvested cash at IBKR
    pub cobra_side_buffer: Decimal,           // pending transfer from Cobra
    pub harvest_mode: Box<dyn HarvestMode>,
    pub dividend_mode: DividendMode,
    pub instruments: Vec<CashCacheInstrument>,
    pub blocked_symbols: HashMap<Symbol, DateTime<Utc>>,  // symbol -> unblock date
    pub pending_liquidation: Option<LiquidationRequest>,
    pub last_liquidation_executed_at: Option<DateTime<Utc>>,  // for cooling period
    pub win_streak_count: u32,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub last_harvest_at: Option<DateTime<Utc>>,
}

/// Individual instrument allocation within CashCache
pub struct CashCacheInstrument {
    pub symbol: Symbol,
    pub weight: Decimal,            // target allocation weight (sums to 1.0)
    pub quantity: Decimal,          // shares held (fractional)
    pub cost_basis: Decimal,        // average cost per share
    pub first_purchase: DateTime<Utc>,
    pub is_custom: bool,            // true if user-added (not from safe list)
    pub instrument_class: InstrumentClass,
    pub execution_strategy: ExecutionStrategy,
}

pub enum InstrumentClass {
    BroadMarketETF,     // no stop-loss
    IndividualEquity,   // -35% stop-loss
    CustomTicker,       // -25% stop-loss
}

/// Liquidation request (multi-day protocol)
pub struct LiquidationRequest {
    pub id: Uuid,
    pub requested_at: DateTime<Utc>,
    pub scope: LiquidationScope,
    pub day1_confirmation_text: String,
    pub reconfirmation_window_start: DateTime<Utc>,  // Day 2 0400 EST
    pub reconfirmation_window_end: DateTime<Utc>,    // Day 3 2000 EST
    pub reconfirmed: bool,
    pub reconfirmed_at: Option<DateTime<Utc>>,
    pub reconfirmation_text: Option<String>,
    pub status: LiquidationStatus,
    pub execution_fills: Option<Vec<(Symbol, Decimal, Decimal)>>,
}

pub enum LiquidationScope {
    PercentOfTotal(Decimal),
    SpecificShares { symbol: Symbol, quantity: Decimal },
    PercentOfSymbol { symbol: Symbol, percent: Decimal },
    LiquidateAll,
}

pub enum LiquidationStatus {
    PendingReconfirmation,
    Confirmed,
    Cancelled(CancellationReason),
    Executing,
    Executed,
    Failed(String),
}

pub enum CancellationReason {
    WindowExpired,
    UserCancelled,
    SystemError(String),
}

pub enum DividendMode {
    ReinvestInCashCache,
    RouteToActiveAccount,
}

pub enum TransferMethod {
    ACH,
    Wire,
}
```

### 16.3 Persistence

- Event log stored in SQLite with WAL mode enabled.
- Every event is append-only and immutable.
- Materialized state projection is cached in a separate SQLite table, rebuilt on startup and after every event.
- The database is backed up daily to a user-configurable location.
- Schema migrations are versioned and applied automatically on system startup.

---

## 17. Error Handling

### 17.1 Harvest Failures

| Failure | Behavior |
|---|---|
| Settlement data unavailable | Skip harvest for this cycle, retry next cycle. Log warning. |
| Broker API timeout during harvest calculation | Retry 3x with exponential backoff. If all fail, skip cycle. Log warning. |
| Negative cash buffer (ghost harvest detected) | Absorb deficit from next harvest cycle. Log critical warning. |
| Busted trade adjustment after harvest | Debit cash buffer by adjustment amount. If buffer goes negative, next harvest absorbs deficit first. |

### 17.2 Purchase Failures

| Failure | Behavior |
|---|---|
| Order rejected by broker | Retry once at next session. If retry fails, return to cash buffer + notify user. |
| Instrument halted | Skip instrument, allocate to next in weight order. Retry halted instrument next session. |
| Insufficient funds (cash buffer discrepancy) | Halt purchases, reconcile cash buffer with broker balance. Log critical error. |
| Fractional shares unavailable | Queue in cash buffer until whole share affordable. Warn user. |
| PassiveAggressive limit unfilled + market conversion fails | Return to cash buffer. Notify user. Log with market conditions. |
| All weighted instruments wash-sale-blocked | Hold in cash buffer. Warn user. |

### 17.3 Liquidation Failures

| Failure | Behavior |
|---|---|
| Reconfirmation window expired | Auto-cancel. Log. No cooling period applied. User can re-submit. |
| Sell order rejected | Retry once. If retry fails, return to Pending, extend window by 24 hours. Notify user. |
| Partial fill | Accept partial, resubmit remainder at next session. |

### 17.4 Stop-Loss Failures

| Failure | Behavior |
|---|---|
| End-of-day price data unavailable | Skip evaluation for this day. Log warning. Never assume price. |
| Liquidation order rejected | Retry at next market open. If 3 consecutive failures, notify user and flag for manual review. |
| PassiveAggressive limit unfilled on stop-loss | Convert to market after timeout. If market also fails, retry next session. |

### 17.5 Transfer Failures (Cobra-to-IBKR)

| Failure | Behavior |
|---|---|
| ACH/wire rejected | Hold in Cobra-side buffer. Retry at next threshold trigger. Notify user. |
| Transfer initiated but not confirmed within 5 business days | Flag as stuck. Notify user. Do not re-initiate (avoid double transfer). |

---

## 18. Testing Requirements

### 18.1 Unit Tests

- Harvest calculation for each mode at each tier boundary (edge cases: exact threshold values, $0 P&L, negative P&L, single-dollar gains).
- Settled cash gating: verify harvest does not fire on unsettled P&L.
- Ghost harvest recovery: simulate busted trade, verify buffer deficit absorption.
- Concentration limit enforcement (at limit, over limit, single-instrument portfolio).
- Multi-day liquidation protocol state machine (all transitions: Pending -> Confirmed -> Executed, Pending -> Cancelled, Pending -> Confirmed -> Failed -> retry).
- Extended reconfirmation window: verify Day 2 0400 through Day 3 2000 boundaries.
- Cooling period: verify 48hr block after execution, no block after cancellation.
- Win streak counter (increment, reset on loss, break-even tolerance).
- Cash buffer accumulation, overflow, and negative buffer handling.
- DCA scheduling (weekends, holidays, market closures).
- Stop-loss bifurcation: verify no trigger on BroadMarketETF, -35% on IndividualEquity, -25% on CustomTicker.
- Stop-loss evaluation at close only (not intraday).
- ExecutionStrategy selection: MarketImmediate for safe list, PassiveAggressive for custom.
- Wash sale group blocking: verify entire group blocked when one member traded.
- Wash sale recursive check: verify substitution never moves within a blocked group.
- Wash sale 30-day window expiry.
- Event sourcing: verify state reconstruction from empty by replaying all events.

### 18.2 Integration Tests

- Full harvest-to-purchase pipeline on IBKR paper account.
- Sub-account isolation: verify active account cannot submit orders against CashCache sub-account.
- Broker transition simulation: harvest at Cobra, Gravity Well accumulation, transfer to IBKR, deploy at IBKR.
- Fractional share purchase and position tracking accuracy over 30+ transactions.
- Concurrent harvest events (multiple trades closing simultaneously on margin account).
- PassiveAggressive execution: verify limit-to-market conversion after timeout.

### 18.3 Behavioral Tests

- Verify active trading modules cannot construct orders against CashCache positions (attempt and confirm rejection at BrokerGateway level).
- Verify same-day liquidation is impossible regardless of UI manipulation.
- Verify disabling CashCache does not liquidate existing positions.
- Verify the 48-hour cooling period is enforced after successful execution.
- Verify cooling period is NOT enforced after cancelled requests.
- Verify event log immutability (attempt to modify historical events, confirm rejection).

---

## 19. Dependencies

| Dependency | Purpose | Required Version |
|---|---|---|
| IBKR TWS API | Broker connectivity for CashCache sub-account | Latest stable |
| SQLite | Event log + materialized state persistence | 3.x via `rusqlite` |
| `rust_decimal` | Financial arithmetic (no floating point) | Latest stable |
| `chrono` | Timestamp and trading calendar operations | Latest stable |
| `uuid` | Entity identification | v4 |
| `tokio` | Async runtime for broker API calls | Latest stable |

---

## 20. Open Questions

1. **IBKR sub-account creation automation.** Can TALON programmatically create a linked sub-account via the IBKR API, or does this require manual setup through Client Portal? If manual, the onboarding flow needs a step-by-step guide.
2. **Cobra ACH fee structure.** What are Cobra's ACH transfer fees? This determines the Gravity Well threshold economics. If ACH is free, the $100 floor is the only constraint.
3. **IBKR margin engine behavior with linked sub-accounts.** Does IBKR's risk engine evaluate margin across linked sub-accounts by default, or are they independent? This determines whether the Collateral Opt-Out toggle requires API configuration or just account structure.
4. **Wash sale group maintenance.** Who maintains the WASH_SALE_GROUPS list as new ETFs launch? This is an ongoing operational responsibility. Consider pulling from an external data source (e.g., ETF overlap databases) rather than hardcoding.
5. **International users.** The spec assumes US tax law, FINRA regulation, and US-listed instruments. International users may have different settlement cycles, tax regimes, and instrument availability. Scope for v2.
6. **CashCache-as-collateral post-PDT.** If the SEC approves dynamic margin and brokers evaluate total account equity across sub-accounts, the Collateral Opt-Out may require broker-level configuration that isn't currently supported. Monitor IBKR announcements post-approval.

---

## Revision History

| Version | Date | Changes |
|---|---|---|
| 0.1.0-draft | 2026-03-01 | Initial specification |
| 0.2.0 | 2026-03-01 | Incorporated Dialectical Analysis critique: PassiveAggressive execution for custom tickers, Gravity Well transfer protocol, prophylactic wash sale avoidance with group blocking, extended 72hr reconfirmation window, instrument-class-bifurcated stop-losses, event sourcing persistence, settled cash gating (Ghost Harvest prevention), Collateral Opt-Out toggle, IBKR sub-account partitioning |
| 0.3.0 | 2026-03-02 | Name alignment with TALON_ARCHITECTURE_v3.1.0: TR1M -> TALON throughout. Module renames: Accelerader -> Climb, MR0DTEAN -> Snapback, BR0DTEAN -> YoYo, DR0DTEAN -> Payload, Range R0LEx -> Siphon. Write protection module list updated to current names. |
| 0.4.0 | 2026-03-02 | Tier name alignment with TALON_ARCHITECTURE_v3.2.0: Jumpstart -> Hatch, DC -> Takeoff, CDL -> Turbo. Updated all tier references in harvest rules, broker assignment, Gravity Well, PDT boundary, and wash sale context. |
