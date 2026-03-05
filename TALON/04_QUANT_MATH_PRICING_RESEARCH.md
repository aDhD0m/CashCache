# 04 -- Quantitative Math, Pricing Models, and Academic Research

Audited: 2026-03-03 | Source: Exhaustive Resource Registry, Section 4

---

## Audit Summary

| Resource | Status | TALON Relevance |
|----------|--------|-----------------|
| avhz/RustQuant | VALID -- premier Rust quant lib | HIGH -- Greeks, stochastics, autodiff |
| arxiv.org q-fin archive | VALID -- academic papers | Medium -- algorithm validation source |
| carlobortolan/quantrs | VALID -- fast option pricing | HIGH -- Black-Scholes at 0.0556 us |
| docs.rs/market-maker-rs | VALID -- Avellaneda-Stoikov | Low -- market making, not TALON's domain |
| awesome-rust (vicanso) | VALID -- curated list | Low -- discovery resource |
| awesome-quant (wilsonfreitas) | VALID -- curated list | Low -- discovery resource |

---

## Resource Details

### 1. RustQuant -- Premier Quantitative Finance Library

- **URL:** https://github.com/avhz/RustQuant
- **Files:** .rs, .toml, .csv
- **License:** MIT (verify -- common for academic Rust crates)

**Module Coverage:**

| Module | Contents | TALON Use |
|--------|----------|-----------|
| autodiff | Automatic differentiation | Greeks computation (delta, gamma, theta, vega) |
| stochastics | Stochastic process simulation | Monte Carlo pricing, volatility modeling |
| instruments | Option/bond/swap definitions | Contract modeling |
| data | Time series, .csv/.json readers | Historical data ingestion |
| math | Linear algebra, optimization | Signal generation, regression |
| statistics | Distributions, hypothesis tests | Strategy validation |
| trading | Portfolio, risk metrics | Performance analytics |

**Autodiff for Greeks:**

RustQuant's autodiff module computes derivatives through the computational
graph automatically. This is superior to finite-difference Greeks because:
- No epsilon parameter to tune
- Machine-precision accuracy
- Computes all Greeks in a single forward/backward pass

For SAGE (gamma exposure scalping), accurate real-time gamma computation
is critical. Finite-difference gamma (delta of delta) accumulates errors.
Autodiff gamma is exact to machine precision.

**Stochastic Processes Available:**

- Geometric Brownian Motion (GBM) -- stock price simulation
- Ornstein-Uhlenbeck -- mean-reverting processes
- Heston model -- stochastic volatility
- SABR model -- volatility smile calibration
- Jump-diffusion (Merton, Kou) -- tail risk modeling
- Cox-Ingersoll-Ross -- interest rate modeling

**TALON Application:**

For SAGE's gamma exposure analysis, RustQuant provides the mathematical
foundation. The autodiff module can compute Greeks without the numerical
instability of finite differences. The stochastic process library enables
Monte Carlo validation of strategy edge.

For Firebird (oversold reversals) and Thunderbird (overextension fades),
the statistics module provides distribution fitting and hypothesis testing
to validate mean-reversion signals.

**Caveat:** RustQuant is an academic library, not a production trading
dependency. Use it for offline computation (backtesting, signal research)
and pre-computed lookup tables. Do not call RustQuant in the hot path
of live order generation -- the allocation patterns are not optimized
for sub-millisecond latency.

---

### 2. quantrs -- Ultra-Fast Option Pricing

- **URL:** https://github.com/carlobortolan/quantrs
- **Files:** .rs, .toml

**Benchmark Claims:**

| Model | Execution Time |
|-------|---------------|
| Black-Scholes analytical | 0.0556 us |
| Binomial tree (100 steps) | ~10 us |
| Monte Carlo (10K paths) | ~1 ms |

**Models Implemented:**

- Black-Scholes-Merton (European options)
- Binomial tree (American options, early exercise)
- Monte Carlo simulation (exotic payoffs)

**TALON Application:**

The 0.0556 us Black-Scholes execution time means TALON can compute
theoretical values for an entire options chain (hundreds of strikes)
in under 1ms. This is fast enough for real-time signal generation
in SAGE and Range R0LEx.

For American options (early exercise), the binomial tree at ~10 us
per option is acceptable for pre-market scans but too slow for
tick-by-tick repricing of large chains.

**Integration Pattern:**

```rust
// Conceptual -- verify against actual quantrs API
use quantrs::options::{BlackScholes, OptionType};

let bs = BlackScholes {
    spot: 150.0,
    strike: 155.0,
    rate: 0.05,
    volatility: 0.25,
    time_to_expiry: 30.0 / 365.0,
    option_type: OptionType::Call,
};

let price = bs.price();
let delta = bs.delta();
let gamma = bs.gamma();
let theta = bs.theta();
let vega = bs.vega();
```

---

### 3. arxiv.org q-fin Archive -- Academic Research

- **URL:** https://arxiv.org/archive/q-fin
- **Files:** .pdf

**Relevant Sub-archives:**

| Archive | Full Name | TALON Relevance |
|---------|-----------|-----------------|
| q-fin.CP | Computational Finance | Pricing algorithms, numerical methods |
| q-fin.MF | Mathematical Finance | Stochastic calculus, volatility models |
| q-fin.PM | Portfolio Management | Risk/reward optimization |
| q-fin.RM | Risk Management | VaR, CVaR, tail risk |
| q-fin.ST | Statistical Finance | Regime detection, correlation |
| q-fin.TR | Trading and Microstructure | Order flow, market impact |

**Key Papers for TALON Modules:**

For SAGE (gamma scalping):
- Search: "gamma scalping" OR "delta hedging" OR "gamma exposure"
- Focus on q-fin.TR and q-fin.CP

For Firebird/Thunderbird (mean reversion):
- Search: "mean reversion" AND "options" OR "oversold"
- Focus on q-fin.ST and q-fin.MF

For ParaShort (parabolic fades):
- Search: "short selling" AND "momentum reversal"
- Focus on q-fin.TR

**Validation Protocol:**

Academic papers are the ONLY acceptable source for validating that a
trading strategy has theoretical edge. The pipeline is:

1. Find paper with mathematical proof or empirical evidence
2. Replicate the core result in Rust (using RustQuant or quantrs)
3. Backtest against historical data (using Databento or local OHLCV)
4. Paper trade for minimum 30 days
5. Live trade at minimum position size

Marketing material, blog posts, and YouTube videos are NOT valid
sources of evidence for strategy validation. This is a hard rule
from the user preferences.

---

### 4. market-maker-rs -- Avellaneda-Stoikov Market Making

- **URL:** https://docs.rs/market-maker-rs
- **Files:** .rs, .html

Implements the Avellaneda-Stoikov optimal market making model with
inventory risk management.

**TALON Application:** Low direct relevance. TALON modules are
directional (long, short, spread) not market-making. However,
the inventory risk management math applies to position sizing
in multi-module concurrent operation. If TALON ever adds a
market-making module, this is the starting point.

---

### 5. Curated Lists (Discovery Resources)

**awesome-rust (vicanso):**
- **URL:** https://github.com/vicanso/awesome-rust
- Curated list including RustQuant, stochastic-rs, and other
  high-performance libraries. Use as a discovery starting point,
  not as a dependency decision.

**awesome-quant (wilsonfreitas):**
- **URL:** https://github.com/wilsonfreitas/awesome-quant
- Links to finalytics (financial data analysis) and RunMat
  (MATLAB-syntax array math in Rust). The MATLAB-syntax crate
  is interesting for prototyping quant formulas but should not
  be used in production TALON code.

---

## Dependency Decision Matrix

For TALON's quantitative needs, the recommended stack:

| Need | Crate | Reason |
|------|-------|--------|
| Option pricing (hot path) | quantrs | Sub-microsecond Black-Scholes |
| Greeks computation (offline) | RustQuant autodiff | Machine-precision, no epsilon tuning |
| Volatility modeling | RustQuant stochastics | Heston, SABR calibration |
| Statistical validation | RustQuant statistics | Distribution fitting, hypothesis tests |
| Performance metrics | barter (statistics module) | Sharpe, Sortino, drawdown |

Do NOT depend on RustQuant in the hot path. Pre-compute Greeks tables
at market open, update on 5-second intervals, and serve from cache.
