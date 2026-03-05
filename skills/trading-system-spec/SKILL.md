---
name: trading-system-spec
description: "Domain knowledge for engineering financial trading systems -- broker APIs, options mechanics, settlement rules, margin/cash account constraints, regulatory requirements (PDT, wash sales, options approval levels), position reconciliation for trading, and supervision models for semi-autonomous execution. Use this skill whenever the user is building, specifying, or auditing a trading system, algo trading platform, order management system, or any software that submits orders to brokerages. Also trigger when the user asks about broker API capabilities, trading account types, settlement mechanics, or regulatory constraints on automated trading. This skill is a domain companion to rust-systems-spec -- use both together when the system is a Rust-based trading platform. Trigger aggressively: if the conversation involves brokers, orders, fills, positions, margin, options, short selling, or trade execution in an engineering context, use this skill."
---

# Trading System Domain Knowledge

## Purpose

Provide the domain expertise that a systems engineer needs when building trading software. This is not a trading education resource -- it's the engineering-relevant subset of trading knowledge that affects system design decisions. The difference: a trader needs to know "what is a credit spread." An engineer needs to know "credit spreads require margin accounts with Level 2+ options approval, and the max loss is defined at entry, and both legs must be submitted atomically or you have naked short risk for the duration between leg fills."

## When to Read Reference Files

- **Always read:** `references/broker-landscape.md` -- Current broker API capabilities, integration difficulty, and selection criteria.
- **If options are involved:** `references/options-engineering.md` -- Options-specific constraints that affect system design.
- **If the system handles money/positions:** `references/settlement-and-risk.md` -- Settlement cycles, margin mechanics, wash sales, position reconciliation patterns.
- **If the system has human oversight:** `references/supervision-models.md` -- The graduated autonomy pattern for semi-autonomous trading.

## Critical Domain Rules

These are the rules that, if violated, cause real-money bugs or regulatory violations.

### 1. Settlement Is Not Instant

Equities settle T+1 (trade date + 1 business day). Options settle T+1. Cash accounts cannot reuse unsettled funds for new purchases (free-riding violation). This means a cash account with $5K can hold at most $5K in settled positions -- but if the system sells a position, that $5K is locked until tomorrow.

**Engineering impact:** Cash account strategies must track both `available_cash` (what can be used today) and `settled_cash` (what has cleared). A system that treats "sold $2K of stock" as "have $2K to spend" will submit orders that get rejected for insufficient settled funds.

### 2. Margin Is Not Free Money

Margin accounts borrow from the broker to increase buying power. The broker can liquidate positions at any time to meet margin requirements (margin call). This happens automatically with no warning during volatile markets. IBKR auto-liquidates at 3:58 PM if maintenance margin isn't met.

**Engineering impact:** Any system using margin must track `excess_liquidity` (cushion above margin requirement) and implement a margin buffer (never use >90% of available margin). Systems that vault profits (CashCache pattern) must understand that sub-accounts may share margin unless configured for isolation.

### 3. PDT Rule Is Binary

Pattern Day Trader: >3 day trades in 5 rolling business days on a margin account with <$25K equity. Violation = 90-day account restriction. No warnings, no grace period.

**Engineering impact:** Systems operating between $2K-$25K on margin must count day trades and hard-block at 3. Cash accounts are exempt. The $25K threshold is config, not code -- FINRA may change it (SR-FINRA-2025-017 filed Jan 2026).

### 4. Wash Sales Are Automated Tax Traps

Sell a security at a loss, buy a "substantially identical" security within 30 days before or after = wash sale. The loss is disallowed for tax purposes and added to the cost basis of the new position. Brokers report wash sales to the IRS automatically.

**Engineering impact:** Any system that harvests losses or rotates between similar instruments (SPY/VOO/IVV) must maintain a 30-day lookback of closed positions per security and block re-entry. This is especially critical for profit-harvesting vaults that might sell a position and then buy a near-identical ETF.

### 5. Options Approval Is Tiered

Brokers grant options trading in levels (IBKR: Limited, Standard, Full). Higher levels unlock riskier strategies. The system cannot assume a user has Level 3 just because they have a margin account.

**Engineering impact:** The system must check options approval level before offering strategies. A module that generates iron condor signals is useless if the user only has Level 1 (covered calls/protective puts). This check should happen at startup and on config change, not at order submission time.

### 6. Short Selling Has Unique Failure Modes

Shorting borrows shares from the broker's inventory. The locate must be confirmed before the sell order. Borrows can be recalled at any time (forced buy-in). Hard-to-borrow fees are charged daily and can exceed the position's profit potential.

**Engineering impact:** Short selling modules need a `locate_shares()` call that succeeds BEFORE order generation. Borrow cost must be factored into the risk/reward calculation. The system needs a forced-cover protocol for adverse moves that bypasses normal approval (because the position has theoretically unlimited loss). Forced covers are the most dangerous autonomous action a trading system can take -- they require full specification (order type, price protection, halted stock handling, operator notification).

### 7. 0DTE Options Decay to Zero

Zero-days-to-expiration options lose all time value by close. A 0DTE position that is +200% at 2:00 PM can be -100% by 3:30 PM. The position MUST be closed before expiration or it exercises/expires, potentially creating unintended stock positions.

**Engineering impact:** 0DTE modules need a hard clock-based exit (e.g., 3:45 PM for SPX). This exit is not optional, not subject to operator approval, and not deferrable. It is a safety mechanism. The system must handle the case where the close order doesn't fill (wide spreads in the final 15 minutes) -- retry with market order as last resort.

## Common Mistakes (Read Before Writing)

These are the top failures when specifying trading systems:

1. **Treating margin as free buying power.** Systems that use 100% of available margin will get auto-liquidated during volatile markets. IBKR liquidates at 3:58 PM with no warning. Always implement a margin buffer (90% max usage). See `references/settlement-and-risk.md`.

2. **Submitting spread legs separately.** If the short leg fills before the long leg, the operator has naked short exposure -- potentially unlimited risk. Multi-leg orders must be submitted atomically as a single spread order. If the broker API doesn't support spreads, the strategy is not viable on that broker. See `references/options-engineering.md`.

3. **Treating broker API crates/libraries as verified.** A crate existing on crates.io is not evidence it works for your use case. Before committing to a broker integration library, verify: last commit date, open issue count, whether it supports the specific features you need (combo orders, streaming fills, sub-accounts). The fallback is always raw protocol implementation, which is significantly harder.

## Module Archetypes

Trading modules tend to fall into a few categories with shared engineering patterns:

### Long Options (Cash Account Compatible)

Examples: Firebird (oversold reversal), Thunderbird (overextension fade)

- Entry: defined premium (max loss = premium paid)
- Exit: target profit %, time stop (% of DTE), stop-loss (% of premium)
- Settlement: T+1, but option premium is paid upfront from settled cash
- Risk: max loss is known at entry. No margin call risk. Cash-friendly.
- Key engineering: options chain selection (strike, expiration, liquidity filters)

### Long Equity Swing (Cash Account Compatible)

Examples: TrEStL (technical swing)

- Entry: market/limit order on equity
- Exit: hard stop ($ or %), trailing stop, time-based exit
- Settlement: T+1 creates capital recycling constraint on cash accounts
- Risk: position risk = stop distance x shares. No leverage on cash.
- Key engineering: track settled vs. available cash. 3-5 concurrent positions cycle capital.

### Intraday Momentum (Margin Required)

Examples: Accelerader (breakout momentum)

- Entry: breakout above level with volume confirmation
- Exit: minutes. Trail or target. Must close by EOD.
- Settlement: intraday = no settlement issue on margin. Cash = each round-trip uses one day of settled funds.
- Risk: fast moves, slippage, halts. PDT counting critical.
- Key engineering: pre-market scanner integration, real-time level 2 data, sub-second order submission

### 0DTE Options (Margin Required)

Examples: MR0DTEAN (mean-reversion), BR0DTEAN (binary event), DR0DTEAN (adaptive)

- Entry: SPX/SPY 0DTE calls/puts based on intraday signal
- Exit: target, stop, or HARD CLOSE at 3:45 PM
- Risk: can go from +200% to -100% in minutes. Gamma risk is extreme.
- Key engineering: the 3:45 PM hard close is a non-negotiable safety mechanism. Clock synchronization matters. The system must handle "can't fill close order" (wide spreads, halt) with escalation to market order.

### Short Equity (CDL Broker Required)

Examples: ParaShort (parabolic fade)

- Entry: short sell after locate confirmation
- Exit: cover order (buy to close). Hard stop above entry.
- Risk: theoretically unlimited loss. Forced buy-in possible. Borrow recall.
- Key engineering: `locate_shares()` is a hard gate. Forced cover protocol is mandatory. Borrow cost monitoring. Emergency cover via broker phone desk as documented backup.

### Premium Selling (Margin Required)

Examples: SAGE (gamma scalp), Range R0LEx (theta farming)

- Entry: sell options premium (credit spreads, iron condors)
- Exit: buy back at target credit capture (50%) or stop (200% of credit)
- Risk: defined risk on spreads, but early assignment risk on short legs. Margin requirement scales with position.
- Key engineering: atomic multi-leg order submission (don't sell the short leg without the long leg). Greeks computation for position management. Adjustment logic for breached strikes.

## Broker Selection Criteria for System Design

When specifying which broker(s) a system supports, evaluate:

1. **API type:** REST, WebSocket, FIX, proprietary TCP (DAS TAPI). REST is easiest from Rust. FIX is standard but verbose. DAS TAPI is proprietary TCP and harder to integrate.
2. **Order types:** Market, limit, stop, stop-limit, bracket, OCO, trailing. Not all brokers support all types via API.
3. **Real-time data:** Quote streaming (level 1), depth of book (level 2), options chains, GEX data. Delivery mechanism (WebSocket, MQTT, proprietary socket).
4. **Short inventory:** In-house lending vs. third-party locates. Locate API availability.
5. **Account minimums:** Range from $0 (Alpaca, Webull) to $30K (Cobra, CenterPoint).
6. **Options chain API:** Not all equity APIs include options. Some charge extra for options data.
7. **Paper trading:** API-accessible paper trading is essential for validation. Some brokers only offer paper via GUI.

See `references/broker-landscape.md` for the current evaluation of specific brokers.
