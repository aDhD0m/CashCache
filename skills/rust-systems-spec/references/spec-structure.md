# Spec Structure Reference

Canonical section ordering for Rust systems engineering specifications. Every section is listed with its purpose and what it must contain. Not every spec needs every section -- but if you skip one, know why.

## Quick Navigation

- S1: Mission (Required)
- S2: Architectural Rules (Required)
- S3: System Tiers / Modes (If applicable)
- S4: Components / Modules (Required)
- S5: Graduation / Progression (If applicable)
- S6: External Interfaces (Required)
- S7: Supervision & Risk (If applicable)
- S8: Trust / Autonomy Escalation (If applicable)
- S9: Data Architecture (Required)
- S10: External System Integration Points (If applicable)
- S11: Operator Interface (If human-operated)
- S12: Operations (Required)
- S13: Build Plan (Required)
- S14: Validation Plan (If applicable)
- S15: Configuration Structure (Required)
- S16: Appendix
- Line Budget Guidance (end of file)

## Section Order

### 1. Mission (Required)

3-5 sentences. What the system does, who it serves, what principles govern its design. No implementation detail. A non-technical stakeholder should understand this section.

Include a **Scope Boundary** subsection listing what the system is NOT. Name external systems explicitly. For each, state the integration point (trait/port) and the system's behavior when the external system is absent.

**Anti-pattern:** Mission sections that describe features instead of purpose. "TR1M supports 11 trading modules across 3 tiers" is a feature list. "TR1M is a tiered trading terminal that prevents the most common retail failure modes" is a mission.

### 2. Architectural Rules (Required)

The invariants that every contributor must internalize. These are the rules that, if violated, create systemic bugs rather than local bugs. Typically 2-5 rules.

Each rule must specify:
- What it forbids or requires
- Why (the failure mode it prevents)
- How it's enforced (compile-time, runtime, code review)

**Example rules:**
- "No synchronous intelligence gates in the execution path" (prevents latency spikes from blocking on external compute)
- "All financial arithmetic uses rust_decimal" (prevents floating-point precision bugs in money calculations)
- "Strategy modules never touch brokers directly" (prevents network I/O blocking the signal generation path)

### 3. System Tiers / Modes (If applicable)

If the system operates in distinct modes or serves users at different capability levels, define them here. Table format:

| Tier | Name | Entry Criteria | Capabilities | Supervision Model |

For each tier, one paragraph explaining the rationale and key constraints.

### 4. Components / Modules (Required)

The functional units of the system. Start with a summary table:

| Component | Tier/Mode | Type | Purpose | Key Constraint |

Then one subsection per component. Each subsection contains:
- **Thesis:** One sentence explaining what the component does and why it exists.
- **Inputs:** What data it consumes and from where.
- **Outputs:** What it produces (events, orders, state updates).
- **Constraints:** Hard limits, dependencies, account requirements.
- **Failure behavior:** What happens when this component's inputs are unavailable or its dependencies fail.

Components that have their own detailed spec (separate document) get a short summary here with a cross-reference to the full spec.

### 5. Graduation / Progression (If applicable)

If the system has a progression model (user advancement, feature unlocking, trust escalation), define the gates and transitions. Table format:

| Transition | Prerequisites | Track Record | Gate Type |

Specify: Can users be demoted? Is progression forced or optional? What happens to in-flight work during demotion?

### 6. External Interfaces (Required)

How the system communicates with the outside world. Split into:

#### 6.1 Command Interfaces (Sync)

Traits for request/response operations. Include full trait definition in Rust. Specify which thread pool they run on. Document every error variant in `Result<T, E>`.

#### 6.2 Streaming Interfaces (Async)

Traits for long-lived connections. Include full trait definition. Specify cancellation mechanism (drop handle, explicit cancel, timeout). Document reconnection behavior.

#### 6.3 Multi-Instance Routing (If applicable)

If the system connects to multiple instances of the same interface (e.g., multiple brokers), document the routing rules. Table format:

| Data/Operation | Routes To | Fallback |

### 7. Supervision & Risk (If applicable)

For systems with human operators or safety constraints:

- **Supervision model:** What the system does autonomously vs. what requires approval.
- **Risk parameters:** Table of configurable limits per tier/mode.
- **Override protocols:** Anything the system does autonomously that bypasses normal approval. For each: trigger condition, order type / action type, fallback behavior, operator notification, event logging.

Override protocols are safety-critical. They require the most detailed specification in the entire document. Include the config file section, default values, and every edge case (halted inputs, unreachable dependencies, absent operator).

### 8. Trust / Autonomy Escalation (If applicable)

If the system gradually grants more autonomy to automated components:

- What is the trust unit (action class, component, regime)?
- What conditions must be met for escalation?
- What diversity requirements prevent overfitting?
- Can trust be revoked? Under what conditions?

Include the data structure that tracks trust state.

### 9. Data Architecture (Required)

#### 9.1 Data Classification

If the system handles sensitive data, define the classification boundary. What's sovereign (restricted access, encrypted, audit-logged) vs. public (market data, config)?

#### 9.2 Persistence

Table format:

| Data | Store | Access Pattern | Retention | Thread Model |

For event-sourced systems: describe the EventStore pattern (channel-backed, dedicated blocking thread, checkpoint strategy). For SQL/SQLite: specify WAL mode behavior, busy_timeout, checkpoint triggers.

#### 9.3 Reconciliation (If stateful)

For any system that maintains state that could diverge from an external source of truth:

- When reconciliation triggers (startup, reconnection, periodic)
- The state machine (step by step)
- Resolution rules for each discrepancy type (which source wins, what requires human review)
- The hard rule: can the system operate during reconciliation, or does it halt?

### 10. External System Integration Points (If applicable)

For systems designed to consume external intelligence or services that don't ship with the system:

```rust
pub trait IntelligencePort<S: Send + Sync>: Send + Sync {
    fn latest(&self) -> Option<&S>;
}
```

Table of defined ports:

| Port Type | State Type | Expected Producer | Behavior When None |

Emphasize: the system ships with all ports returning None and delivers full value. External systems are enrichment, not prerequisites.

### 11. Operator Interface (If human-operated)

See `references/tui-philosophy.md` for the full treatment. At minimum:

- Layout diagram (ASCII) with zone names and purposes
- Per-zone: what data appears, why it's in that position, what the operator learns from a glance
- Color philosophy (what colors mean, what's deliberately avoided)
- Interaction model (keyboard map, command mode)
- Adaptive behavior (how the interface responds to system state changes)

### 12. Operations (Required)

- **Development environment:** OS, toolchain, key dependencies with versions.
- **Observability:** Logging framework, span correlation, what's traced.
- **Process supervision:** Watchdog, heartbeat, restart behavior.
- **Emergency procedures:** Per-dependency failure playbook. Table format:

| Scenario | Automated Response | Manual Backup |

Include phone numbers, URLs, support hours for external services. These save lives (metaphorically) during real incidents.

### 13. Build Plan (Required)

Ordered phases. Each phase lists what's built, what's validated, and the exit criteria. No phase should depend on systems not yet built.

The build plan should be achievable by the team described in the Mission section. If the plan requires 3 senior Rust developers but the team is one person, the plan is wrong.

### 14. Validation Plan (If applicable)

The near-term concrete plan (1-3 months) with specific milestones. Distinct from the build plan -- validation is about proving the system works with real data / real money / real users, not about building features.

### 15. Configuration Structure (Required)

Show the actual directory tree and representative config file contents. Every configurable value mentioned in the spec should appear in a config file somewhere in this section.

### 16. Appendix

Catch-all for:
- Affinity matrices (which components work with which external interfaces)
- Enum definitions
- Naming conventions
- Regulatory context
- Glossary

## Line Budget Guidance

| Section | Typical Lines | Notes |
|---|---|---|
| Mission + Scope | 30-50 | Short and sharp |
| Architectural Rules | 20-40 | Few rules, well-explained |
| Tiers | 20-40 | Table + one paragraph each |
| Components | 100-200 | Scales with component count |
| External Interfaces | 40-80 | Trait definitions + routing |
| Risk & Supervision | 40-80 | Tables + override protocols |
| Data Architecture | 40-80 | Tables + reconciliation |
| Integration Points | 20-40 | Port definitions + None behavior |
| Operator Interface | 80-150 | Layout + zones + color + interaction |
| Operations | 40-60 | Environment + observability + emergency |
| Build Plan | 30-50 | Phases with exit criteria |
| Config + Appendix | 40-80 | Directory tree + glossary |
| **Total** | **400-800** | |
