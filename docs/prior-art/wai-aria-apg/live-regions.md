**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — live regions (`aria-live`, `aria-atomic`, `aria-relevant`, `aria-busy`) and how Buiy's global announcer service + AccessKit `Live` enum implement the contract

# Live regions

ARIA live regions are how content updates reach screen readers asynchronously — without the user moving focus. APG covers the four live-region roles (`alert`, `status`, `log`, `timer`; plus deprecated `marquee`) and the four global live-region properties (`aria-live`, `aria-atomic`, `aria-relevant`, `aria-busy`). The Buiy foundation [`accessibility.md § 3.11 Live regions and announcements`](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to all of these (with `marquee` in tier E).

## The four live-region roles

| Role | Implicit live | Implicit atomic | Typical use |
|---|---|---|---|
| `alert` | `assertive` | `true` | Critical errors, warnings; interrupts the user immediately |
| `status` | `polite` | `true` | Status updates, success messages; waits for AT idle |
| `log` | `polite` | `false` | Chat / message log; appends to existing announcements |
| `timer` | `off` | (n/a) | Time-counting; updates frequent enough that AT does NOT auto-announce — apps poll explicitly |
| `marquee` (deprecated-leaning) | `off` | (n/a) | Scrolling text marquee |

**Implicit semantics matter.** An element with `role="alert"` does NOT need `aria-live="assertive"` set explicitly — the role implies it. Buiy emits the role only; AccessKit's `Live` enum is derived from the role + any explicit `aria-live` override.

## The live-region properties

### `aria-live`

`off` / `polite` / `assertive`. Controls **politeness**:

- **`off`** — no announcement on update
- **`polite`** — wait for AT idle (current speech finishes) then announce
- **`assertive`** — interrupt current speech immediately to announce

AT honour the politeness contract differently:
- NVDA / JAWS / Narrator queue polite announcements behind currently-speaking content
- VoiceOver / TalkBack treat polite as "queue if not currently speaking; drop otherwise"
- Some AT users configure verbosity that elides polite announcements entirely

**Buiy's default policy.** Prefer `polite` for status messages; reserve `assertive` for actual errors / warnings the user must hear. The verification harness gate 4 ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) captures announcement output for snapshot review.

### `aria-atomic`

`true` / `false`. Controls **what gets announced** on update:

- **`true`** — announce the ENTIRE live region content on any change (the live region is "atomic")
- **`false`** (default) — announce only the changed portion (per `aria-relevant`)

**Common error.** Setting `aria-atomic="true"` on a noisy live region (e.g. a chat log) causes AT to re-read the entire log on every new message. Use `aria-atomic="false"` (default) and let `aria-relevant="additions"` carry only new content.

### `aria-relevant`

Space-separated tokens: `additions` / `removals` / `text` / `all`. Controls **which types of mutation** are relevant:

- `additions` — new descendants added
- `removals` — descendants removed
- `text` — text node content changed
- `all` — equivalent to "additions removals text" (NOT to "additions text" alone)

Default is `additions text`.

**AccessKit gap.** AccessKit's node model carries `Live` and `is_live_atomic` but **does not** carry `aria-relevant`. Buiy must layer `aria-relevant` filtering on its own side — the global announcer service ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`accesskit/lessons.md § Borrow`](../accesskit/lessons.md)) decides which mutations warrant a push-and-announce vs silent update.

### `aria-busy`

`true` / `false`. Indicates a node (typically a container) is being **modified by the application** — AT should defer announcement until `aria-busy` flips back to `false`.

Common pattern:

1. Application sets `aria-busy="true"` on the live region
2. Application makes multiple mutations
3. Application sets `aria-busy="false"`
4. AT announces the final state once (per `aria-atomic` / `aria-relevant` filtering)

Buiy implements this in the global announcer's batch-update path: announcements queued during a `aria-busy` window are merged and flushed when busy clears.

## Mapping to AccessKit

| ARIA | AccessKit |
|---|---|
| `role="alert"` | `Role::Alert` with implicit `Live::Assertive` |
| `role="status"` | `Role::Status` with implicit `Live::Polite` |
| `role="log"` | `Role::Log` with implicit `Live::Polite` |
| `role="timer"` | `Role::Timer` with implicit `Live::Off` |
| `aria-live="polite"` | `Node::set_live(Live::Polite)` |
| `aria-live="assertive"` | `Node::set_live(Live::Assertive)` |
| `aria-live="off"` | `Node::set_live(Live::Off)` |
| `aria-atomic="true"` | `Node::set_is_live_atomic(true)` |
| `aria-relevant` | **not represented** — Buiy filters on its side |
| `aria-busy="true"` | `Node::set_is_busy(true)` |

## How Buiy implements live regions

The Buiy global **Announcer** service ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)) is a Bevy resource that:

1. Accepts announcement requests: `Announcer::announce(Politeness::Polite, "Saved.")` or via emitting changes to a live-region container
2. Maintains a per-window queue of pending announcements
3. On each `BuiySet::A11yUpdate` cycle, materialises the announcements into AccessKit tree updates — either by mutating an existing live-region node's value or by inserting a hidden `Role::Alert` / `Role::Status` node with the announcement text
4. Honours `aria-busy` windows: batches mutations while busy, flushes on busy-clear
5. Honours `aria-relevant`: filters which subtree mutations warrant a push, per the configured token set on each live region
6. Honours `aria-atomic`: when emitting via mutation, sets the entire container's value as the announcement; when emitting via insertion, just the inserted node carries the announcement

### Per-widget live-region usage

| Widget | Live region |
|---|---|
| `Alert` | implicit `role=alert`, assertive |
| `Status` | implicit `role=status`, polite |
| `Log` (e.g. chat) | implicit `role=log`, polite, `aria-relevant="additions"` |
| `Timer` | implicit `role=timer`, off (app polls if announcement needed) |
| `Toast` / `Snackbar` | constructs an Alert-or-Status announcement; WCAG 2.2.3 pause / stop / extend; auto-dismiss with the announcement timing decoupled from visual dismiss |
| `Carousel` (auto-rotating) | `aria-live="polite"` only while paused (per APG) |
| `Feed` | `aria-busy="true"` during item-loading; ARIA `feed` role; no `aria-live` directly |
| Form-validation errors | inline emit via `aria-describedby` to error message + Announcer polite announcement |
| Drag/drop announcements | polite announcements on drag start / drop / cancel ([WCAG 2.5.7](wcag-22-aa-mapping.md)) |

## Common pitfalls

| Pitfall | Mitigation |
|---|---|
| `aria-live` set ALONGSIDE a live-region role | Don't — the role implies it. Setting both can confuse AT |
| `aria-atomic="true"` on a noisy log | Use default (false) + `aria-relevant="additions"` |
| Adding the live region to the DOM AT runtime AND populating it in same tick | Some AT only fire if the node existed empty *before* the population. Pattern: create empty live region at app start; mutate to announce |
| `aria-live="assertive"` overuse | Reserve for critical interrupts; otherwise the user fatigues and disables AT |
| Live regions inside `aria-hidden` subtree | They don't announce — `aria-hidden` excludes from AT |
| Live regions in `inert` subtree | Same — `inert` removes from focus + AT |
| Forgetting `aria-relevant` filtering on chat logs | "All" causes re-read of removals; use `additions` only |
| Wayland session bounds wrong | (Tangential) — but Wayland's `winit::Window::inner_position()` returns `Err`; AT-SPI bounds reports may be wrong on Wayland sessions. Not a live-region issue per se but it affects how AT positions announcements visually |

## Open questions for Buiy

- **Multi-window announcement priority.** When two windows each have a polite announcement, what's the priority? Buiy's per-window adapter ownership ([`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md)) makes per-window queues natural; cross-window prioritisation isn't currently spec'd.
- **`aria-relevant="removals"` semantics.** When announcing a removal, AT must speak both "removed" and the text of the removed item — Buiy's announcer needs to capture the text *before* the removal happens. The implementation watches `Component::on_remove` observers.
- **Marquee.** Tier E in Buiy. Question: should Buiy emit `Role::Marquee` (AccessKit supports it via `Role::Marquee`) or fold it into the announcer? Decision deferred; the APG pattern is "deprecated-leaning" and Buiy doesn't need it for foundation.

## Sources

- ARIA 1.2 § 6.4 Live Region Properties: <https://www.w3.org/TR/wai-aria-1.2/#attrs_liveregions>
- APG patterns (alert, status, log, timer): each at `https://www.w3.org/WAI/ARIA/apg/patterns/<pattern>/`
- WCAG 4.1.3 Status Messages: <https://www.w3.org/WAI/WCAG22/Understanding/status-messages.html>
- AccessKit `Live` enum: <https://docs.rs/accesskit/0.24.0/accesskit/enum.Live.html>
- Buiy live regions and announcements: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy global announcer (architecture commitment): [`docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling files: [`roles-states-properties.md`](roles-states-properties.md), [`patterns-catalog.md`](patterns-catalog.md), [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md)
