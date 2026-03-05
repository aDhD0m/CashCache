# TUI Design Philosophy for Real-Time Systems

## The Cockpit Principle

A real-time system TUI is not a dashboard. It is not a log viewer. It is the cockpit of an aircraft that is always in motion. Every pixel of screen real estate is expensive. Every color carries cognitive load. Every misplaced element costs a millisecond of confusion that, in a fast-moving system, can cost real outcomes.

## Three Simultaneous Jobs

The TUI must accomplish all three at once:

### 1. Ambient Awareness (Peripheral Vision)

One-second glance answers: "Is the system healthy? Are things winning or losing? Is anything screaming for attention?" This is done through color, brightness, and spatial position -- not text. The operator should not need to read any words to get this answer.

### 2. Surgical Precision (Focused Attention)

When the operator focuses on a specific element, every piece of information needed for a decision must be within that focal area. No scrolling. No tab-switching. No "where was that number?" Information architecture must match decision architecture.

### 3. Emotional Regulation (Psychological Design)

Colors, motion, and layout directly affect the operator's emotional state. The interface must produce calm, focused decision-making -- not dopamine hits or stress spikes.

## Zone-Based Layout

Organize the TUI into a fixed grid where every zone maps to a cognitive priority. Nothing is randomly placed.

### Zone Design Template

```
+---------------------------------------------------------------------+
|                        ZONE A: STATUS BAR                          |
|  System health * Clock * Global state * Connection indicators      |
|--------------------------------+------------------------------------|
|     ZONE B: PRIMARY FOCUS      |     ZONE C: COMMUNICATION         |
|                                |                                    |
|  The main operational data     |  System-to-operator messages       |
|  that the operator watches     |  Pending decisions / approvals     |
|  during active periods         |  Status of automated components    |
|                                |                                    |
|--------------------------------+------------------------------------|
|     ZONE D: ANCHOR             |     ZONE E: LOG                   |
|                                |                                    |
|  The "always good news"        |  Event stream (most recent)        |
|  element -- psychological       |  Scrollable, last N always visible |
|  stabilizer during bad         |                                    |
|  periods                       |                                    |
|--------------------------------+------------------------------------+
```

### Zone A -- Status Bar (Top, Always Visible)

First thing the eye sees. Answers the ambient awareness question.

Design rules:
- Single-glyph system health indicator (*). Color only: green/yellow/red.
- Clock in the operational timezone, not system timezone. Seconds visible.
- Global state as a single word with confidence-scaled color saturation. High confidence = vivid. Low confidence = washed out.
- Connection indicators: one dot per external service. Green/yellow/red. Blinking red = critical (disconnected with active state).
- No charts. No graphs. Numbers only. Charts waste status bar space.

### Zone B -- Primary Focus (Center-Left, Largest)

Where the operator's eyes live during active periods. The most information-dense zone.

Design rules:
- Each active item is a horizontal bar with fixed-width fields. The eye anchors on the left (identifier), scans right (status, metrics, exit condition).
- Include a **progress bar** for each item showing position relative to good/bad exits. Left end = worst outcome. Right end = best outcome. Fill level = current state. One element, three data points.
- Use **semantic brightness**: items the system is most confident about render at full saturation. Marginal items render dimmer. The eye naturally gravitates to the brightest items.

### Zone C -- Communication (Center-Right)

Where the system talks to the operator. Pending decisions, component status, trust/autonomy indicators.

Design rules:
- Each component has a status card with consistent layout. Every card looks the same.
- Pending decisions **pulse** -- subtle brightness oscillation for peripheral attention. Pulse frequency increases with urgency.
- Trust/autonomy counters show progress toward escalation thresholds.

### Zone D -- Psychological Anchor (Bottom-Left)

The "always good news" element. During bad periods, this zone provides visual evidence that the system is working over time.

Design rules:
- Sparkline showing long-term trend (growing savings, increasing success rate, etc.)
- Muted, calm color (not the same color used for active wins). Should feel like a savings account balance, not a trading position.
- This zone never shows alarming data. It is the emotional anchor.

### Zone E -- Event Log (Bottom-Right)

Scrollable event stream. Most recent at top. Last 5 always visible without scrolling.

Design rules:
- Each event type has a unique glyph + color. After a week, the operator reads by glyph without reading text.
- Consistent format: `timestamp  glyph  source  description`
- Severity determines prominence: errors are bright, routine events are dim.

## Color Philosophy

### Emotional Palette (for systems where operator emotional state matters)

**No pure red (#FF0000) for negative states.** Pure red triggers fight-or-flight. Use desaturated warm amber/rust instead. Amber says "pay attention" without screaming "DANGER."

**No pure green (#00FF00) for positive states.** Pure green triggers dopamine and risk-seeking. Use muted teal/seafoam instead. Teal says "going well" without encouraging overcommitment.

**Brightest elements = system state, not outcome metrics.** Global state indicator, connection health, and process health should be the brightest elements. Outcome numbers (P&L, throughput) are deliberately subdued. Operator attention should be on system health and decision quality.

**Background:** Deep navy/charcoal. Not pure black (too harsh for 6+ hour sessions). Not gray (too washed, low contrast).

**Borders:** Subtle, thin, low-contrast. The grid structure should be felt, not seen.

### Confidence-Scaled Saturation

A powerful technique for encoding confidence in color:

- High-confidence states render in vivid, saturated color.
- Low-confidence states render in washed-out, desaturated versions of the same hue.
- Unknown/absent states render in neutral gray.

The operator's eye perceives "vivid = certain" and "washed = uncertain" without reading numbers.

## Interaction Model

### Keyboard-First

Real-time system operators keep hands on keyboard. Mouse movement costs 200-500ms of motor context switching.

**Tier 1 -- Single key (urgent):** Kill all, approve, reject, cycle focus. These happen under pressure.

**Tier 2 -- Two key (important):** Reset circuit breaker, toggle debug, open detail view. Important but not panic-mode.

**Tier 3 -- Never three keys.** If an action needs three keys, it belongs in command mode.

**Command mode (vim-style `:` prefix):** For complex operations that benefit from explicit naming. `:kill nvda`, `:approve all`, `:status cashcache`.

### No Mouse Interaction

Not "mouse is secondary." Mouse is absent. If the TUI requires mouse interaction for any operation, the design is wrong.

## Adaptive Behavior

The TUI breathes with the system's operational tempo.

- **Low activity:** Longer update intervals. Calm visual rhythm. Feels relaxed because the system is relaxed.
- **High activity:** Shorter update intervals. Faster visual rhythm. Matches operational urgency.
- **Crisis:** Zone A dominates with pulsing state indicator. Zone B sorts by severity. Zone C suppresses new proposals -- only active state management visible. Narrows operator focus to what matters.
- **Recovery/Reconciliation:** Full-screen takeover. No normal operations until resolved. The operator cannot dismiss this -- they must engage with it.

## None-State Display Rules

Every display element that depends on an external data source must have a documented appearance when that source returns None:

| Source State | Display Behavior |
|---|---|
| Connected, data available | Normal rendering |
| Connected, no data yet | "WARMING UP" or spinner, dim |
| Not connected | "STANDALONE" or dash, neutral gray |
| Error | Glyph only ([!]), amber, no misleading data |

Never display stale data without indicating staleness. If the last update was > N seconds ago, dim the element and show the age.
