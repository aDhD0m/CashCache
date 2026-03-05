# Options Engineering Constraints

## Options Chain Selection

Options are not like equities -- you can't just "buy AAPL." You must select:

1. **Expiration date** -- When the contract expires. Affects time decay rate.
2. **Strike price** -- The price at which the option can be exercised.
3. **Type** -- Call (right to buy) or Put (right to sell).

The combination (underlying, expiration, strike, type) uniquely identifies a contract. The system must select from the available chain, not construct arbitrary parameters.

### Liquidity Filters

Illiquid options have wide bid-ask spreads that destroy strategy profitability. Minimum filters before an option contract is tradeable:

```toml
[options.liquidity]
min_open_interest = 500          # Contracts outstanding
max_bid_ask_spread = 0.10        # Dollars, not percentage
min_underlying_market_cap = 2e9  # $2B -- penny stock options are untradeable
min_daily_volume = 100           # Contracts traded today
```

These filters are applied before signal generation, not at order time. A module should never generate a signal on an illiquid contract.

### Strike Selection Strategies

| Strategy | Strike Selection | Rationale |
|---|---|---|
| Directional (Firebird, Thunderbird) | ATM or 1 strike OTM | Best delta exposure per dollar |
| 0DTE (MR0DTEAN, BR0DTEAN) | ATM or 1 strike OTM | Maximum gamma. Further OTM = too cheap, too illiquid. |
| Credit spreads (Range R0LEx) | Short leg at desired delta (0.20-0.30), long leg 1-2 strikes further | Defines max risk and credit received |
| Protective (hedging) | OTM puts at 5-10% below current price | Insurance, not profit |

### Expiration Selection

| Strategy | Expiration | Rationale |
|---|---|---|
| 0DTE | Today | The whole point |
| Short-term directional | 2-4 weeks | Enough time for thesis, minimize theta decay |
| Swing (Firebird, Thunderbird) | 4-8 weeks | Slower theta, more time for reversal |
| Credit spreads | 30-45 DTE | Peak theta decay curve for premium sellers |
| Calendar spreads | Near: 2-4 weeks, Far: 6-8 weeks | Time spread exploitation |

## Greeks That Matter for System Design

### Delta (Delta)

Directional exposure. +1.0 = stock equivalent. The system uses delta to calculate notional exposure for risk limits.

```rust
pub fn notional_exposure(position: &OptionsPosition) -> Decimal {
    position.delta * position.quantity * position.contract_multiplier * position.underlying_price
}
```

### Gamma (Gamma)

Rate of delta change. Critical for 0DTE (gamma explodes near expiration) and for SAGE (gamma exposure scalping). High gamma = the position's directional exposure changes rapidly with price movement.

**Engineering impact:** High-gamma positions need more frequent monitoring. The system should increase check frequency for positions with |gamma| > threshold.

### Theta (Theta)

Time decay per day. Negative for long options (you lose money each day), positive for short options (you earn money each day).

**Engineering impact:** Range R0LEx and credit spread strategies profit from theta. The system must track theta P&L separately from directional P&L. A position can be directionally losing but theta-profitable.

### Vega (Vega)

Sensitivity to implied volatility. Long options benefit from rising IV. Short options benefit from falling IV.

**Engineering impact:** IV rank (current IV relative to its 52-week range) is a primary entry signal for premium-selling strategies. The system must compute or fetch IV rank for scanner/signal purposes.

## Atomic Multi-Leg Orders

Spread strategies require multiple legs submitted atomically. If the system submits the short leg before the long leg fills, the operator has naked short option exposure -- potentially unlimited risk.

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

**Rule:** Multi-leg orders are always submitted as a single spread order to the broker. The broker handles atomic fill-or-cancel for all legs. If the broker doesn't support spread orders via API, the strategy is not viable on that broker.

**IBKR:** Supports combo/spread orders via TWS API (`ComboLeg`).
**Alpaca:** Limited spread support -- verify before relying on it.
**Cobra/CenterPoint (DAS):** Spread orders supported via DAS TAPI.

## Early Assignment Risk

American-style options (all US equity options) can be exercised at any time before expiration. Short options positions face early assignment risk. This is most likely when:

- Short call is deep ITM near ex-dividend date (call holder exercises to capture dividend)
- Short put is deep ITM (put holder exercises to sell stock at inflated price)
- Near expiration when time value approaches zero

**Engineering impact:** The system must handle unexpected assignment events. An assignment converts an options position into a stock position (100 shares per contract). If the system has a short call assigned, it now has -100 shares (short stock) without having gone through the locate/borrow process.

Handling: treat assignment as a fill event. If the resulting stock position violates risk limits, surface to operator immediately. Do not auto-liquidate the stock position -- the operator may want to manage it.

## Exercise and Expiration

ITM options at expiration are automatically exercised by the broker (OCC rule). This creates stock positions that may:
- Exceed the account's buying power
- Create margin calls
- Create unintended overnight exposure

**Engineering impact:** 0DTE systems MUST close all positions before the exercise cutoff. For SPX (European-style, cash-settled), this is less dangerous (cash settlement, no stock position). For SPY (American-style, physically settled), an unclosed ITM option results in a stock position.

The hard close time (default 3:45 PM for 0DTE) exists specifically to prevent this. It is not subject to operator override. It is a safety mechanism.

## GEX (Gamma Exposure) for SAGE

Dealer gamma exposure creates predictable hedging flows. When dealers are short gamma (negative GEX), they hedge by buying dips and selling rips -- amplifying moves. When long gamma (positive GEX), they hedge by selling dips and buying rips -- dampening moves.

The GEX flip level (where dealer gamma goes from positive to negative) is a key level for intraday price action.

**Data source:** GEX is not directly available from most broker APIs. It must be computed from the options chain (open interest x gamma x contract multiplier, summed across all strikes) or sourced from a third-party data provider (SpotGamma, Orats, etc.).

**Engineering impact:** SAGE needs either: (a) a GEX computation engine that processes the full options chain, or (b) a data feed integration for pre-computed GEX levels. This is a significant data pipeline requirement -- specify which approach the system uses.
