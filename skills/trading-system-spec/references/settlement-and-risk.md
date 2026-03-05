# Settlement, Margin, and Risk Engineering

## Settlement Cycles

| Instrument | Settlement | Impact on Cash Accounts | Impact on Margin Accounts |
|---|---|---|---|
| US Equities | T+1 | Cannot reuse proceeds until next business day | Proceeds available immediately (borrowed against margin) |
| US Options | T+1 | Premium paid from settled funds. Proceeds locked T+1. | Premium from buying power. Proceeds immediate. |
| US Treasuries | T+1 | Locked T+1 | Available immediately |

### Cash Account Capital Recycling

A cash account with $5,000 can hold $5,000 in positions. Selling a $2,000 position frees $2,000 -- but not until T+1. The system must track:

```rust
pub struct CashBalance {
    pub settled: Decimal,        // Available for new purchases today
    pub unsettled: Decimal,      // Pending settlement (T+1)
    pub pending_settlement: Vec<SettlementEvent>,
}

pub struct SettlementEvent {
    pub amount: Decimal,
    pub settles_on: NaiveDate,   // Trade date + 1 business day
    pub source: OrderId,
}
```

**Free-riding violation:** Using unsettled funds to buy AND sell a security before the original sale settles. Results in 90-day account restriction (cash-only, no unsettled fund usage). The system must prevent this by blocking purchases that exceed `settled` balance.

### Margin Buying Power

Margin accounts have `buying_power = excess_liquidity x leverage_factor`. For Reg-T: 2x overnight, 4x intraday. For Portfolio Margin (IBKR, $110K+): risk-based, often 6x+.

The system should never use 100% of buying power. Implement a margin buffer:

```toml
[risk.margin]
max_buying_power_usage_pct = 90  # Never use more than 90% of available BP
margin_call_warning_pct = 95     # Alert operator at 95% usage
```

## Pattern Day Trader (PDT) Rule

**Rule:** >3 day trades in 5 rolling business days on a margin account with <$25K equity = PDT violation.

**Day trade definition:** Opening AND closing the same position in the same trading day. Opening Monday, closing Tuesday = NOT a day trade. Opening and closing Monday = day trade.

**Counting:**

```rust
pub struct PDTTracker {
    pub day_trades: VecDeque<NaiveDate>,  // Rolling 5-business-day window
    pub account_equity: Decimal,
}

impl PDTTracker {
    pub fn can_day_trade(&self) -> bool {
        if self.account_equity >= Decimal::from(25_000) {
            return true; // Exempt
        }
        let five_days_ago = business_days_ago(5);
        let count = self.day_trades.iter()
            .filter(|d| **d >= five_days_ago)
            .count();
        count < 3
    }
}
```

**Cash account exemption:** Cash accounts are NOT subject to PDT. They can day trade freely -- but each round-trip uses settled funds, which aren't available again until T+1.

**FINRA PDT reform (SR-FINRA-2025-017):** Filed January 2026. Proposes replacing $25K minimum with risk-based intraday margin. If approved (projected mid-2026), the PDT threshold becomes configurable. Design all tier boundaries as config values, not hardcoded constants.

## Wash Sale Prevention

**Rule:** Sell a security at a loss, buy a "substantially identical" security within 30 days before OR after the sale = wash sale. The loss is disallowed for tax purposes.

**Substantially identical:** Same ticker, same ETF tracking the same index (SPY/VOO/IVV are substantially identical), options on the same underlying within certain strike/expiration ranges.

**Engineering implementation:**

```rust
pub struct WashSaleTracker {
    /// Closed positions with losses, keyed by wash sale group
    pub loss_events: HashMap<WashGroup, Vec<LossEvent>>,
}

pub struct LossEvent {
    pub symbol: Symbol,
    pub close_date: NaiveDate,
    pub loss_amount: Decimal,
}

pub enum WashGroup {
    Ticker(Symbol),
    ETFFamily(Vec<Symbol>),  // e.g., [SPY, VOO, IVV, SPLG]
}

impl WashSaleTracker {
    pub fn is_blocked(&self, symbol: &Symbol, date: NaiveDate) -> bool {
        let group = self.wash_group(symbol);
        self.loss_events.get(&group)
            .map(|events| events.iter().any(|e| {
                let window_start = e.close_date - Duration::days(30);
                let window_end = e.close_date + Duration::days(30);
                date >= window_start && date <= window_end
            }))
            .unwrap_or(false)
    }
}
```

**Critical for profit-harvesting systems:** CashCache sells winning positions and buys broad ETFs. If it sells SPY at a loss and then buys VOO within 30 days, that's a wash sale. The wash sale tracker must use `WashGroup::ETFFamily` to catch cross-ticker matches.

## Position Reconciliation for Trading Systems

Trading system reconciliation has specific failure modes beyond generic state reconciliation:

### Crash Between Order and Fill

The system submits an order, then crashes before receiving the fill acknowledgment. On restart:

1. The broker has the filled position.
2. The system's event log has the order submission but no fill.
3. The reconciliation pulls broker fill history since last known fill timestamp.
4. The missing fill is replayed into the event log.
5. Position state is rebuilt from the event log.

### Partial Fill Crash

The system submits an order for 100 shares. 50 fill. System crashes. Remaining 50 fill while system is down.

1. On restart, broker shows 100 shares.
2. System event log shows 50-share fill.
3. Reconciliation finds the second 50-share fill in broker history.
4. Second fill is replayed. Position now shows 100 shares.

### CashCache Harvest Mid-Crash

CashCache initiates a multi-day liquidation. Day 1 sell order fills. System crashes. Day 2 sell order was never submitted.

1. On restart, broker shows partial position (Day 1 sold, Day 2 still held).
2. System event log shows harvest initiated, Day 1 fill recorded, Day 2 not submitted.
3. Reconciliation matches -- no discrepancy in positions.
4. CashCache resumes the harvest protocol from where it left off (Day 2 sell).

### Forced Cover While System Down

A short position triggers forced cover at the broker level (broker's own risk management, not the system's). The system was down.

1. On restart, broker shows flat position (covered).
2. System event log shows open short position.
3. Reconciliation finds the cover fill in broker history.
4. Cover is replayed. Position closed. CashCache pending harvest (if any) is cancelled.
5. **This is an `UnloggedForcedCover` discrepancy** -- it must be surfaced to the operator even though it's automatically resolvable, because it indicates the system's forced cover should have fired first.

## Options Approval Levels (IBKR)

| Level | Name | Strategies Allowed |
|---|---|---|
| 1 | Limited | Covered calls, protective puts, cash-secured puts |
| 2 | Standard | All Level 1 + long calls/puts, spreads (verticals, calendars, diagonals) |
| 3 | Full | All Level 2 + naked puts, naked calls, straddles, strangles |

The system must check approval level at startup:

```rust
pub fn validate_module_permissions(
    module: &Module,
    approval_level: OptionsApprovalLevel,
) -> Result<(), PermissionError> {
    match module.required_approval() {
        RequiredApproval::None => Ok(()),
        RequiredApproval::Level1 if approval_level >= Level1 => Ok(()),
        RequiredApproval::Level2 if approval_level >= Level2 => Ok(()),
        RequiredApproval::Level3 if approval_level >= Level3 => Ok(()),
        required => Err(PermissionError::InsufficientApproval {
            module: module.id(),
            required,
            actual: approval_level,
        }),
    }
}
```

Modules with insufficient approval are disabled at startup with a clear message explaining what the user needs to do (request approval upgrade from broker).
