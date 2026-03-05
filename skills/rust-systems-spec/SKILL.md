---
name: rust-systems-spec
description: "Generate production-grade Rust systems engineering specifications. Use this skill whenever the user asks to write a spec, architecture doc, system design, technical specification, or engineering reference for any Rust-based system -- especially those involving async runtimes, trait-based abstractions, event-sourced persistence, or hardware/network I/O boundaries. Also trigger when the user asks to audit, review, or revise an existing Rust system spec. This skill produces specs that a Rust developer can implement from directly, with trait definitions, error handling patterns, persistence strategies, operational runbooks, and failure mode analysis. Trigger aggressively: if the user mentions 'spec', 'architecture', 'system design', 'trait design', 'module spec', or 'engineering doc' in the context of Rust or systems programming, use this skill."
---

# Rust Systems Spec Generator

## Purpose

Produce engineering specifications for Rust systems that are implementable, auditable, and operationally complete. A good spec is one where a competent Rust developer can open it, read it top-to-bottom, and start writing code without asking clarifying questions. A bad spec is one that describes a vision but leaves the developer guessing about trait boundaries, error paths, persistence strategy, and what happens when things go wrong.

## When to Read Reference Files

Before writing any spec, read the reference files relevant to the task:

- **Always read:** `references/spec-structure.md` -- The canonical section ordering and what each section must contain.
- **If the system has async + sync boundaries:** `references/async-patterns.md` -- The split-trait pattern, blocking thread pool rules, and channel-backed persistence.
- **If the system has external integrations:** `references/integration-ports.md` -- The IntelligencePort pattern for clean decoupling of external systems.
- **If the system has a TUI or operator interface:** `references/tui-philosophy.md` -- Zone-based layout, color psychology, keyboard-first interaction, adaptive behavior.

## Core Principles

These are lessons learned from real spec-writing sessions. They exist because the opposite happened and caused problems.

### 1. Scope Discipline

The single most common spec failure is bundling systems that should be separate. Before writing, ask: "Can this system ship and deliver value without system X?" If yes, system X is external -- define a port for it, not its internals.

Signs you're bundling: the spec has sections describing the internals of a system the user won't build in the same phase. The spec has build phases where Phase D depends on Phase A but Phase A isn't useful alone. The word count is climbing past 800 lines with no sign of stopping.

The fix: extract the external system into its own spec. In the current spec, define only the integration port -- a trait with `fn latest() -> Option<&State>` or similar. Document what happens when the port returns `None` (the system must still work).

### 2. Trait Boundary Honesty

Every trait in the spec must have honest sync/async annotations.

If a method blocks on network I/O (broker API calls, database writes), it is synchronous and belongs in a trait that runs on a blocking thread pool. Do not annotate it `async` -- that forces the implementer to either block the async executor (violating the runtime) or wrap everything in `spawn_blocking` without guidance.

If a method manages long-lived connections (WebSocket streams, event subscriptions), it is async and must return a cancellation handle. Do not make it sync -- that forces the implementer to spawn internal tasks with no caller-visible handle.

Split traits when they contain both kinds. Name them clearly: `FooCommands` (sync) + `FooStreams` (async).

### 3. Failure Modes Are Not Optional

Every protocol, state machine, and integration point in the spec must have a "What goes wrong" section. This is not polish -- it is the spec. Developers implement happy paths from intuition. They implement error paths from specs. If the spec doesn't describe the error path, the developer will either skip it or guess.

Required failure mode coverage: what happens on startup before state is initialized, what happens when a dependency is unavailable, what happens when data is corrupt or inconsistent, what happens when the operator is absent.

### 4. The Operator Interface Is Architecture

If the system has a human operator, the interface design is a first-class architectural concern, not a cosmetic afterthought. The spec must describe: what information the operator needs, where it appears, how urgency is communicated visually, what the interaction model is (keyboard/mouse/voice), and how the interface adapts to system state changes.

Read `references/tui-philosophy.md` before writing any operator interface section.

### 5. No Dead Screens

Every data display element in the spec must have a documented data source. If a widget shows "regime state" but the regime detection system is external and might not be connected, the spec must say what the widget shows when the port returns `None`. "STANDALONE" or "UNKNOWN" or a dash -- but never undefined.

### 6. Configuration Over Code

Thresholds, limits, timeouts, feature flags, tier boundaries -- anything that might change based on operational experience -- must be in configuration files, not hardcoded. The spec should include a representative config file structure showing where each configurable value lives.

## Common Mistakes (Read Before Writing)

These are the top failures from real spec sessions. If you only read SKILL.md and skip the reference files, at least internalize these three:

1. **Bundling external systems.** If the spec is past 800 lines and still growing, you're probably describing the internals of a system that should be its own spec. Extract it. Define a port (`fn latest() -> Option<&S>`). Document None behavior. Move on. See `references/integration-ports.md`.

2. **Annotating sync traits as async.** If a method blocks on network I/O, it is sync. Put it in a sync trait on a blocking thread pool. If a method manages a long-lived stream, it is async and must return a cancellation handle. Never mix both in one trait. See `references/async-patterns.md`.

3. **Skipping failure modes.** Developers implement happy paths from intuition. They implement error paths from specs. If your spec doesn't describe what happens when a dependency is down, data is corrupt, or the operator is absent, the developer will skip it or guess wrong. Every protocol and state machine needs a "what goes wrong" section.

## Spec Writing Workflow

### Phase 1: Scope Interview

Before writing, establish:

1. **What ships together?** List the components. For each, ask: "Does this deliver value alone?" If yes, it's a system. If no, it's a module within a system.
2. **What's external?** List external dependencies. For each, define the integration port type. Document None behavior.
3. **Who operates it?** If human-operated, the TUI/UI section is mandatory.
4. **What's the persistence story?** Event-sourced? CRUD? In-memory only? This shapes everything.
5. **What are the async boundaries?** Where does blocking I/O happen? Where do long-lived streams live?

### Phase 2: Structural Draft

Follow the section ordering in `references/spec-structure.md`. Write all section headers first with one-line summaries. Present to the user for approval before filling in content. This prevents writing 500 lines in the wrong direction.

### Phase 3: Content Generation

Fill sections in dependency order -- foundational rules first, then components that depend on those rules. For each section:

- Write the happy path
- Write the failure modes
- Write the configuration
- Include code (trait definitions, struct layouts, enum variants) only when it clarifies something prose can't

### Phase 4: Audit Sweep

Before delivering, sweep the spec for these known failure patterns:

- [ ] Any `#[async_trait]` on a trait with sync method signatures?
- [ ] Any state machine missing its initialization / "not yet ready" state?
- [ ] Any risk/safety protocol that bypasses human oversight without specifying order type, fallback, notification, and logging?
- [ ] Any persistence layer on the async executor instead of a blocking thread?
- [ ] Any external system's internals described instead of just its port?
- [ ] Any module-level limits that can exceed the system-level ceiling when summed?
- [ ] Any display element without a documented data source and None behavior?
- [ ] Any configurable value hardcoded instead of in config?
- [ ] Any reconciliation scenario missing (crash between write and ack, partial operation, clock skew)?

If any box is unchecked, fix before delivering.

## Output Format

Deliver the spec as a single Markdown file. Use code blocks for trait definitions, struct layouts, config examples, and TUI mockups. Use tables for parameter matrices, module maps, and comparison data. Use prose for rationale, failure modes, and design philosophy.

Target length: 400-800 lines for a focused system spec. Under 400 usually means missing failure modes or operational detail. Over 800 usually means bundled external systems that should be extracted.
