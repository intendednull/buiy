**Date:** 2026-06-18
**Status:** active
**Subject:** Playwright actionability — condition-based auto-waiting (attached/visible/stable/receives-events/enabled/editable) before driving a node, force bypass, strict single-match

# Playwright actionability

## What actionability is

In Playwright an action (`click`, `fill`, `check`, `hover`, ...) is not a single imperative call against a pre-resolved handle. It is a loop:

1. **Locate** — resolve the locator to element(s) (see [playwright-locators.md](playwright-locators.md)).
2. **Wait until actionable** — poll a set of *conditions* on the target element.
3. **Dispatch real input** — only once all required conditions hold, send the actual pointer/keyboard event at the action point.

If the conditions do not all hold, Playwright retries — re-evaluating from the top — until they do or the action times out (`TimeoutError`). This replaces the arbitrary `sleep(500)` / hard-wait pattern with **condition-based waiting**: the test states *what must be true of the element* before acting, not *how long to guess*. Playwright's own best-practices doc pushes the same discipline for assertions: web-first `await expect(locator).toBeVisible()` retries, whereas a manual `expect(await locator.isVisible()).toBe(true)` "won't wait a single second, it will just check the locator is there and return immediately."

This is the single biggest reliability lesson in the whole browser-automation corpus (recorded as such in [lessons.md](lessons.md)).

## The actionability checks (verbatim definitions)

From the actionability reference:

- **Attached** — the base existence check: the element is present in the DOM. All other checks build on this.
- **Visible** — the element "has non-empty bounding box and does not have `visibility:hidden` computed style." Note `opacity:0` *is* visible; `display:none` and zero-size are not.
- **Stable** — the element "has maintained the same bounding box for at least two consecutive animation frames." This is how it waits out animations/transitions instead of sleeping.
- **Receives events (hit target)** — the element "is the hit target of the pointer event at the action point." Playwright hit-tests the action point and confirms no overlay (modal, toast, sticky header) intercepts the pointer.
- **Enabled** — not disabled: no `[disabled]` attribute, not inside a disabled `<fieldset>`, not a descendant of `[aria-disabled=true]`.
- **Editable** — enabled *and* not readonly (`[readonly]` or `[aria-readonly=true]` on supported roles).

Which checks apply depends on the action:

| Action | Visible | Stable | Receives events | Enabled | Editable |
|---|---|---|---|---|---|
| click / dblclick / tap / check / uncheck / setChecked | ✓ | ✓ | ✓ | ✓ | — |
| hover / dragTo | ✓ | ✓ | ✓ | — | — |
| fill / clear | ✓ | — | — | ✓ | ✓ |
| selectOption | ✓ | — | — | ✓ | — |
| screenshot | ✓ | ✓ | — | — | — |

(`press` / `type` target the focused element after focusing; `fill` is the editable-checked path for setting input values.)

## Force bypass and trial

`force: true` — "Whether to bypass the actionability checks. Defaults to `false`." The docs describe it as bypassing the actionability checks generally; in practice it drives the input without waiting on the gate (visibility / stability / receives-events) — the escape hatch for cases where the framework's model of "actionable" disagrees with reality (e.g. a deliberately-overlapped element). It trades reliability for control; the default is the safe condition-gated path. (The "skips the *non-essential* checks" gloss used elsewhere in this folder is an interpretation, not the spec's wording — the spec says "bypass the actionability checks.")

`trial: true` — "this method only performs the actionability checks and skips the action." A dry run: confirm the element *would* be actionable without dispatching input. Useful as a pure readiness probe.

## Strict single-match (a separate gate, no retry)

Locators are **strict**: "all operations on locators that imply some target DOM element will throw an exception if more than one element matches." Important nuance — a strict-mode violation is thrown *immediately*; it is **not** part of the retry loop. The error reads `strict mode violation: locator('...') resolved to N elements`. Auto-waiting waits for *zero → one* (attachment), but *two-or-more* is treated as a test bug, not a transient condition to wait out ([microsoft/playwright#30363](https://github.com/microsoft/playwright/issues/30363)). Opting out is explicit (`.first()` / `.nth()`), and Playwright discourages it in favor of a locator that uniquely identifies one element.

The split matters: ambiguity is a *spec* error surfaced loudly and fast; not-yet-ready is a *timing* condition retried silently.

## Tolerating a node that moved or despawned between read and act

Because the action loop re-evaluates from locate → check on each retry, a locator is a *recipe* re-run against current DOM, not a snapshot pointer. An element that detaches and re-attaches, or moves, is simply re-located and re-checked on the next iteration — the stale handle problem (act on an element captured a frame ago, now gone) is structurally avoided. The default action timeout is `0` (no timeout) but is typically set via `actionTimeout` in config or `setDefaultTimeout`; the bound is on the *whole loop*, not a fixed sleep.

## Implications for Buiy

Buiy's analogue of the DOM-mutation problem is the **frame loop**: nodes are ECS entities that are spawned, despawned, and re-laid-out across frames, and layout results (`ResolvedLayout`) settle over one or more frames. An agent that reads the AccessKit tree on frame N and acts on frame N+k faces the same read/act skew Playwright's loop absorbs.

The mapping is direct — actionability becomes a poll across frames before dispatching an AccessKit `ActionRequest`:

- **Attached** → the target entity still exists this frame.
- **Laid-out / visible** → the node has a `ResolvedLayout` with non-empty bounds and is not hidden (display/visibility analogue).
- **Stable** → same resolved bounds across two consecutive frames — wait out enter/layout animations rather than guessing a frame count.
- **Hit-targetable** → a `hit_test` / picking query at the action point returns *this* node, not an overlay (top-layer, modal, tooltip). Buiy already has the stacking + top-layer machinery this would query.
- **Enabled** → the AccessKit node's disabled state is clear (the tree already carries role+name+**state**+actions).

The loop tolerance carries over: re-resolve the entity each frame so a node that despawned/moved between agent read and act is simply re-found or times out, never acted on stale. `force` maps to a bypass that drives the `ActionRequest` without the gate; strict single-match maps to refusing an ambiguous target (multiple nodes for one query) loudly rather than picking one.

Two open questions a consuming spec must settle, not this evidence file: (1) whether "two consecutive frames" is the right stability window for an ECS render loop, or whether Buiy can do *better* than sampling — i.e. whether `ResolvedLayout` (or the layout-dirty machinery) already carries a *settled* signal an agent could read directly instead of comparing bounds frame-to-frame; this is unknown from the prior art and must be checked against Buiy's layout internals. (2) How the timeout/force knobs are exposed to an agent. Both are flagged in [open-problems.md](open-problems.md) (items 5 and 8). Recorded as a borrow/validate in [lessons.md](lessons.md).

## Sources

- Playwright actionability reference: https://playwright.dev/docs/actionability
- Playwright Locator API (force / trial / timeout / strict): https://playwright.dev/docs/api/class-locator
- Playwright best practices (web-first assertions, auto-waiting, avoid hard waits): https://playwright.dev/docs/best-practices
- Playwright locators guide (strict mode, `.first()`/`.nth()`/`.or()`): https://playwright.dev/docs/locators
- microsoft/playwright#30363 (strict mode does not auto-wait on multiple matches): https://github.com/microsoft/playwright/issues/30363
