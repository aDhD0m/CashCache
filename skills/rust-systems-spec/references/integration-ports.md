# External System Integration Ports

## The Core Problem

When specifying a system, there's a gravitational pull to describe every system it will eventually interact with. If your trading terminal will someday use GPU compute, you start writing GPU dispatch logic. If it will someday consume sentiment data, you start writing the sentiment pipeline. This leads to specs that are 50% future systems that don't ship with v1 -- bloating the document, confusing the build plan, and creating phantom dependencies.

## The Rule

If a system can ship and deliver value without system X, then system X is external. Define a port for it, not its internals.

## The Port Pattern

```rust
/// Generic read-only port for consuming external intelligence.
/// The external system publishes state on its own schedule.
/// The consumer reads whatever's available. None = not connected.
pub trait IntelligencePort<S: Send + Sync>: Send + Sync {
    fn latest(&self) -> Option<&S>;
}
```

## How to Document a Port in a Spec

For each external system, the spec needs exactly four things:

### 1. What the port is

The trait definition and the state type it carries.

```rust
pub struct RegimeState {
    pub regime: Regime,        // Trending, Reverting, Crisis, LowLiquidity
    pub confidence: f64,       // 0.0-1.0
    pub since: DateTime<Utc>,  // When this regime was detected
}
```

### 2. Who produces it

One line naming the expected producer. Don't describe the producer's internals.

> **Producer:** Regime detection system (separate spec). Could be rule-based, ML-based, or manual operator override.

### 3. What happens when it's None

This is the most important part. Every consumer of the port must have documented None behavior. This ensures the system works without the external system.

> **When None:** Modules operate on their own signals without regime weighting. Position sizing uses baseline (no regime-adjusted multiplier). TUI regime indicator shows "STANDALONE."

### 4. What changes when it's Some

How the system's behavior improves when the external system is connected.

> **When Some:** Modules weight signals by regime affinity. Position sizing scales with regime confidence. TUI regime indicator shows the regime name with confidence-scaled saturation.

## Port Table Template

| Port | State Type | Producer | When None | When Some |
|---|---|---|---|---|
| Regime | `RegimeState` | Regime detection system | Own signals, no weighting | Regime-weighted sizing |
| Sentiment | `SentimentState` | PRESSbox | Ignore sentiment | Sentiment modifier on signals |
| Fusion | `BeliefState` | Signal fusion layer | Independent operation | Fused direction + uncertainty |
| Proposals | `StrategyProposal` | LOOM | No proposals | Proposals surfaced to operator |
| Compute | `ComputeTarget` | Conductor | CPU-only | GPU/FPGA dispatch |

## Common Mistakes

### Describing the producer's architecture

```markdown
<!-- BAD: This is PRESSbox's spec, not yours -->
### Sentiment Integration

PRESSbox uses a dual-encoder architecture with BERT-base for financial text
and a custom CNN for social media. The dialectic loop runs Scout and Judge
models in alternation with an optional Adversary for manipulation detection.
The temporal-contextual feature vector is 768-dimensional...
```

```markdown
<!-- GOOD: Just the port -->
### Sentiment Integration

TR1M defines `IntelligencePort<SentimentState>`. When PRESSbox (separate spec)
publishes sentiment, modules consume it as a signal modifier. When disconnected,
modules ignore sentiment entirely.
```

### Making the port a hard dependency

```rust
// BAD: panics if regime system isn't connected
let regime = regime_port.latest().expect("regime system must be running");

// GOOD: graceful degradation
let regime_weight = match regime_port.latest() {
    Some(state) => state.confidence,
    None => 1.0, // baseline weight, no regime adjustment
};
```

### Defining build phases that depend on external systems

```markdown
<!-- BAD: Phase 2 can't start until external system exists -->
### Phase 2
- Deploy sentiment analysis pipeline
- Integrate regime detection
- Build GPU dispatch layer

<!-- GOOD: External systems plug in whenever they're ready -->
### Phase 2
- Deploy DC tier modules
- Validate trust calibration
- (External systems plug into IntelligencePort when available)
```

## Implementation Notes

The typical backing store for a port is `Arc<ArcSwap<Option<S>>>` or `Arc<RwLock<Option<S>>>`:

- `ArcSwap` for high-read, low-write scenarios (regime state published every few seconds, read every tick). Lock-free reads.
- `RwLock` for moderate-read, moderate-write scenarios. Simpler API, tiny contention if publishes are <= 1/second.

The publisher runs in its own task/thread and calls `port.publish(new_state)`. The consumer calls `port.latest()` synchronously -- never blocking, never async.
