# Broker Landscape for Algorithmic Trading Systems

## Evaluation Date: March 2026

Broker capabilities change. Re-verify before making integration decisions.

## Broker Evaluation Matrix

### Interactive Brokers (IBKR)

**Role:** Primary broker for most retail algo systems. Institutional-grade API, widest instrument coverage.

| Dimension | Assessment |
|---|---|
| API | TWS API (Java-native, Rust via `ibapi` crate or raw TCP), Client Portal REST, FIX |
| Equities | Full US markets + international. Fractional shares. |
| Options | Full chain access. All strategies. Level 1-3 approval. |
| Short selling | Retail inventory. Decent ETB list. Not best-in-class for HTB. |
| Margin | Portfolio margin available ($110K+). Reg-T standard. |
| Minimum | $0 (cash), no minimum for margin but PDT rules apply. |
| Commissions | $0.0005-$0.0035/share equity, $0.15-$0.65/contract options |
| Paper trading | Full API-accessible paper environment. Separate credentials. |
| Data | Real-time L1/L2 via TWS socket. Historical bars. Options chains. |
| Sub-accounts | Yes. Used for CashCache isolation pattern. Close-Only designation available. |
| Rust integration | `ibapi` crate exists (community). TWS API is TCP + custom protocol. |
| Gotchas | Auto-liquidation at 3:58 PM on margin calls. TWS requires running gateway process. Client Portal has rate limits. Sub-accounts share margin by default. |

**Verdict:** P0 -- primary for all tiers. Build this integration first.

### Alpaca

**Role:** API-first broker for developers. Clean REST/WebSocket. Commission-free equities.

| Dimension | Assessment |
|---|---|
| API | REST + WebSocket. Well-documented. OAuth2. |
| Equities | US markets. Fractional shares. |
| Options | Yes (added 2023). Basic chain access. |
| Short selling | Limited inventory. Not suitable for CDL-grade shorting. |
| Margin | Standard Reg-T. |
| Minimum | $0 |
| Commissions | $0 equities, $0.015/contract options |
| Paper trading | Full API-accessible paper. Same endpoints, different base URL. |
| Data | Real-time via WebSocket. Historical bars. Options chains. |
| Rust integration | REST = trivial from Rust (`reqwest` + `serde`). WebSocket = `tokio-tungstenite`. |
| Gotchas | Options fills can be slow. Not the deepest liquidity pool. No direct market access. |

**Verdict:** P1 -- Jumpstart alternative. API-first path for developers who want the cleanest integration.

### Webull

**Role:** Zero-cost entry. Commission-free with modern multi-protocol API.

| Dimension | Assessment |
|---|---|
| API | HTTP + GRPC + MQTT (OpenAPI launched 2024, updated Q2 2025) |
| Equities | US markets. Fractional shares. |
| Options | Chain access via API. |
| Short selling | Limited. Not suitable for CDL. |
| Margin | Standard Reg-T. $2K minimum for margin. |
| Minimum | $0 (cash) |
| Commissions | $0 equities and options |
| Paper trading | Available via API. |
| Data | Real-time via MQTT (low-latency streaming). REST for snapshots. |
| Rate limits | 150 requests/10 seconds (trading). 1K+/sec (market data via MQTT). |
| Rust integration | REST/GRPC = straightforward from Rust. MQTT = `rumqttc` crate. |
| Gotchas | Rate limits may choke under DC/CDL intraday volumes. No DMA. |

**Verdict:** P1-ALT -- Jumpstart alternative for users wanting zero-cost entry. Not suitable above Jumpstart tier due to rate limits.

### Cobra Trading

**Role:** CDL-grade short selling. Professional DAS execution platform.

| Dimension | Assessment |
|---|---|
| API | DAS TAPI (proprietary TCP) + DAS FIX (FIX 4.2). |
| Equities | US markets via DAS. 70+ routing destinations. |
| Options | Yes via DAS. |
| Short selling | Best-in-class with Wedbush + IBKR dual clearing. Excellent HTB access. |
| Margin | PDT $30K minimum. 4% margin rate. |
| Minimum | $30K ($50K for foreign accounts) |
| Commissions | $0.002-$0.004/share (volume tiered). $1 minimum. |
| Paper trading | DAS Trader Pro demo available. TAPI access may require live account. |
| Data | Via DAS platform. Real-time L1/L2. |
| Rust integration | DAS TAPI = proprietary TCP protocol. Must reverse-engineer or use DAS documentation. Harder than REST. |
| Gotchas | $125/month DAS platform fee (waived at 250K shares/month). TAPI documentation may be limited. TCP connection drops require robust reconnection logic. |
| Emergency contacts | Trading desk: 512-850-5022 (Austin, TX). Support: support@cobratrading.com. Hours: 6AM-8PM EST. Web platform: das.cobratrading.com. |

**Verdict:** P2 -- CDL primary for short selling. DAS TAPI integration is the hardest broker integration in the system.

### CenterPoint Securities / Clear Street Active Trading

**Role:** CDL fallback. Same locate quality as Cobra with a modernizing API.

| Dimension | Assessment |
|---|---|
| API | DAS TAPI + Sterling API + Clear Street REST (rolling out post-acquisition). FIX 4.2. |
| Equities | US markets. 30+ routing options. |
| Options | Yes. |
| Short selling | Best-in-class. In-house lending firm. 5,000+ ETB symbols. |
| Margin | PDT $30K minimum. |
| Minimum | $30K |
| Commissions | $0.001-$0.004/share (volume tiered). |
| Paper trading | 14-day platform trial available. |
| Data | Via DAS/Sterling platform. |
| Rust integration | Clear Street REST API = modern, documented at docs.clearstreet.io. Much easier than DAS TAPI. FIX 4.2 available for institutional flows. |
| Gotchas | Clear Street REST may not yet be available for all retail active trader accounts. Verify before committing. Platform fees: $120-$150/month. |

**Verdict:** P2-ALT -- CDL fallback. If Cobra TAPI is problematic, CenterPoint/Clear Street REST API is the pivot. Same locates, easier API.

## Rejected Brokers

### SpeedTrader

DAS-based (same platform as Cobra and CenterPoint). $10K-$25K minimum. 25+ routing options. 4 third-party locate services.

**Rejection reason:** Redundant. Same DAS TAPI integration as Cobra, but worse locate inventory and no differentiated capability. If we're building DAS TAPI for Cobra, SpeedTrader adds zero incremental value.

### NinjaTrader

Futures-focused (CME, CBOT, NYMEX). C# API. Excellent for futures algo trading.

**Rejection reason:** Wrong asset class. Equities/options system. No equity execution capability. Revisit only if futures expansion becomes a priority.

### Tradier

Clean REST API. $0.35/contract options. No platform fees. $0 minimum.

**Deferred reason:** Strong candidate for lightweight options execution at Jumpstart tier. Lacks DMA, limited short inventory. Evaluate in v2 when operational data from IBKR exists to assess whether cost savings justify a fourth broker integration.

## Broker Selection Decision Tree

```
Is the user building a cash account system?
|-- Yes -> IBKR (most complete) or Alpaca (cleanest API) or Webull (zero cost)
|-- No -> Margin account
    |-- Does the system short sell?
    |   |-- Yes -> Cobra (primary) or CenterPoint (fallback)
    |   |-- No -> IBKR (most complete)
    |-- Does the system need sub-accounts for capital isolation?
        |-- Yes -> IBKR (only broker with flexible sub-account structure)
        |-- No -> Choose by API preference
```

## Multi-Broker Architecture Pattern

Systems that span multiple tiers often need simultaneous broker connections:

- **Capital preservation account** (CashCache pattern): Always at IBKR. Separate sub-account or separate account for isolation.
- **Active trading account**: IBKR at lower tiers, Cobra/CenterPoint at CDL tier.
- **Routing rules**: Shorts -> CDL broker. Vault operations -> IBKR. Everything else -> cheapest or most capable.

The `BrokerSessionManager` holds `HashMap<BrokerId, (Box<dyn BrokerCommands>, Box<dyn BrokerStreams>)>` and routes operations based on module affinity and account type.
