**Date:** 2026-06-18
**Status:** active
**Subject:** Browser automation (CDP / WebDriver BiDi / Playwright) — the decision file: what to validate, avoid, and borrow for Buiy's agent interface

# Lessons for Buiy

Buiy already authors an AccessKit semantic tree (role + name + state + actions) but emits it output-only — it is the surface screen readers consume. The thesis these folders serve: that same tree is the right LLM-agent perception+control surface, made bidirectional by consuming AccessKit `ActionRequest`s through the existing `bevy_winit` channel. Browser automation is the closest-adjacent body of prior art: it is the field that has spent a decade learning *how to address, act on, and wait for* a live semantic UI tree from outside the application. This file extracts the load-bearing lessons. Decisions live here; the sibling evidence files stay factual.

See the folder map in [README.md](README.md). The AccessKit corpus that Buiy's tree comes from is [../accesskit/](../accesskit/); the role + accessible-name contract is [../wai-aria-apg/](../wai-aria-apg/). The *consumer* of this surface — an LLM agent harness — is out of scope here; it would belong in a future `llm-agent-interface/` prior-art folder, which **does not yet exist** (do not treat this as a live cross-link until that folder is authored).

---

## VALIDATES (the field independently arrived at Buiy's bets)

- **A11y-tree-first locators are the right *primary* address.** Playwright's most-recommended locator is `getByRole`, "the closest way to how users and assistive technology perceive the page"; it reads the accessibility tree, not raw DOM, and CSS/XPath are demoted to last resort because "the DOM can often change leading to non-resilient tests." Buiy can hand an agent the *same* role+name surface as its primary address space — no second tree to build. See [playwright-locators.md](playwright-locators.md).

- **Synthetic input through the real event path.** CDP `Input.dispatchMouseEvent`/`dispatchKeyEvent`/`insertText` and BiDi `input.performActions` drive the browser's actual input pipeline, not a side-channel that mutates state directly — so the same handlers, focus rules, and side effects fire as for a human. Buiy's analogue: consume AccessKit `ActionRequest`s through the existing `bevy_winit` channel, so an agent click is indistinguishable from a real one at the widget layer. See [actionability.md](actionability.md), [cdp.md](cdp.md).

- **Stable opaque node ids are the durable handle.** Every protocol pins nodes by an opaque, session-scoped id (CDP `backendDOMNodeId` as the durable join key, BiDi shared-id references) rather than by a path through structure. Resolvers re-find by *semantics*, but once resolved they address an id. Buiy's AccessKit `NodeId` already *is* this — a stable, retained-tree id — validating that the framework need not invent an addressing scheme. See [webdriver-bidi.md](webdriver-bidi.md), [glossary.md](glossary.md).

---

## BORROW (concrete mechanisms to lift)

- **Session-pinned ids mapped onto `AccessKit NodeId`.** Hand the agent the existing `NodeId` as the stable handle for a node within a session; do not mint a parallel id. This is the smallest possible addressing layer because the retained tree already owns it. (Evidence: BiDi/CDP node-reference model — [webdriver-bidi.md](webdriver-bidi.md).)

- **`getByRole` + ARIA-snapshot over the AccessKit tree.** Offer two read primitives: a role+name *query* (Playwright `getByRole`/CDP `Accessibility.queryAXTree`) and a compact, deterministic *snapshot* of the whole subtree (Playwright `ariaSnapshot()` YAML, including its `mode: 'ai'` and `boxes: true` variants). Buiy's tree already carries role/name/state — a YAML/JSON projection of it is the agent's primary perception payload. See [playwright-locators.md](playwright-locators.md).

- **Lazy, re-resolved-at-action-time locators.** Playwright locators are a *description of what to find*, re-evaluated on every action, not a held handle; this is precisely how it dodges stale-element exceptions when a component re-renders mid-action. Buiy mutates its tree every ECS frame, so an agent address MUST be a re-runnable query (role+name), resolved to a `NodeId` at action time — never a handle cached across frames. See [actionability.md](actionability.md), [open-problems.md](open-problems.md).

- **Actionability auto-waiting driven off the frame loop.** Playwright runs an action only when the target resolves to exactly one element that is visible, stable, receives events, and enabled — auto-waiting until then, with `strict` single-match and a `force: true` escape hatch. Buiy can evaluate the same gates against tree state once per frame and dispatch the `ActionRequest` only when they pass, with the same strict-single-match and force semantics. See [actionability.md](actionability.md).

- **Screenshot-on-demand + node highlight.** CDP pairs `Page.captureScreenshot` (pixels when semantics are insufficient) with `Overlay.highlightNode` (visually mark the node an id resolves to). Buiy already renders to a GPU texture and can read it back (`capture` example, render-to-texture path) — so an on-demand screenshot and a debug highlight of a resolved `NodeId` are cheap, and bridge the gap when the agent needs pixels. See [cdp.md](cdp.md).

- **The event-out direction: BiDi `session.subscribe` as the model for tree-change emission.** The command-in path (dispatch `ActionRequest`s) is the well-trodden half; the *event-out* half is the under-specified one, and BiDi is the cleanest prior art for it. BiDi keeps events *off* by default and delivers them only after the client opts in per module via `session.subscribe`, each event carrying `module.event` + params (see [webdriver-bidi.md](webdriver-bidi.md)). Lifted to Buiy, the open design questions the bidirectional thesis must answer are concrete:
  - **What to emit.** At minimum a *tree-changed* signal so an agent knows its last snapshot is stale; richer options mirror BiDi's per-module events — node added / removed / re-parented, focus moved, a node's state changed (checked/expanded/disabled), value changed, an action completed/rejected. These map directly onto the role+name+**state**+actions the tree already carries.
  - **At what granularity.** Per-node deltas vs. a coarse "tree dirty this frame" tick. Buiy already rebuilds/diffs the AccessKit tree each frame, so the diff it computes for screen readers is the natural event source — emit what changed, not the whole tree. A coalesced per-frame batch (one event carrying the frame's deltas) fits the ECS loop better than a stream of micro-events.
  - **How an agent subscribes.** A BiDi-style opt-in (subscribe to a set of event kinds, optionally scoped to a subtree) so a passive observer pays nothing and an active agent gets only the deltas it asked for — the same default-off discipline that keeps BiDi cheap.
  This is a *direction* to design, not a settled API; it is called out as the biggest remaining gap in the bidirectional thesis. See [webdriver-bidi.md](webdriver-bidi.md), [open-problems.md](open-problems.md).

---

## AVOID (anti-patterns the field paid for)

- **CDP domain sprawl + Chromium coupling.** CDP is a kitchen-sink RPC partitioned into DOM/Accessibility/Input/Page/Overlay/Runtime/Target/CSS/Console/Debugger/... versioned *with the Chromium build*, with no forward/backward-compat guarantee. Buiy should expose a SMALL curated command set (query, snapshot, invoke-action, screenshot, highlight — plus the subscribe/event surface above), not mirror its internals as a sprawling RPC. The whole reason WebDriver BiDi exists is to escape this coupling. See [cdp.md](cdp.md), [webdriver-bidi.md](webdriver-bidi.md).

- **Modeling a browser instead of the widget tree.** Browser protocols carry heavy structural machinery — frames, browsing contexts, realms, shadow DOM, cross-origin isolation (BiDi's `browsingContext`/`script` modules exist largely for this). Buiy has none of that: one process, one retained ECS tree. Importing frame/realm concepts would be modeling a problem Buiy does not have. The agent surface is the AccessKit tree, full stop. See [webdriver-bidi.md](webdriver-bidi.md), [open-problems.md](open-problems.md).

- **A second structural id space.** Browsers maintain *two* trees (DOM + accessibility) and must reconcile ids across them — friction Buiy can skip entirely because it has a single retained tree with one id space (`NodeId`). Do not introduce a DOM-like address alongside the semantic one; the AccessKit id is the only handle the agent needs. See [glossary.md](glossary.md).

- **Locking the contract to a fast-moving, unstandardized surface.** CDP changes whenever DevTools' needs change and is not specified in a shared public spec; that churn is a maintenance tax cross-vendor consumers (Firefox's CDP support, Puppeteer) paid for years. If Buiy's agent contract is to be durable, treat it as a versioned API with stability guarantees, closer in spirit to the W3C BiDi posture than to tip-of-tree CDP. See [cdp.md](cdp.md), [webdriver-bidi.md](webdriver-bidi.md).

---

## What the corpus does NOT settle (carry into the consuming spec)

Two design questions the browser literature genuinely leaves open for an in-engine surface, recorded so the spec does not assume they are answered:

1. **Multi-driver / human-vs-agent arbitration.** Browser automation assumes one driver and a tab the human is not touching; Buiy's agent surface shares one process, one input pipeline, and one frame loop with a live human. What happens when an `ActionRequest` interleaves with real input on the same `bevy_winit` channel — queue, reject, merge, contest focus, abort-on-human-keystroke — is unaddressed by the prior art. Flagged in [open-problems.md](open-problems.md) item 8.

2. **Doing better than sampled stability.** Playwright's "stable = two consecutive animation frames" is a sampling proxy. Buiy may be able to read a real *settled* signal from `ResolvedLayout` / its layout-dirty machinery instead of comparing bounds frame-to-frame — but whether that signal exists is a question for Buiy's layout internals, not the prior art. Flagged in [actionability.md](actionability.md) and [open-problems.md](open-problems.md) item 5.

---

## The one-line synthesis

Browser automation converged, after a decade, on exactly the surface Buiy already emits: a semantic role+name+state+actions tree, addressed by stable opaque ids, re-resolved by query at action time, gated by actionability, and driven through the real event path. Buiy's task is therefore *narrow* — make the existing AccessKit tree bidirectional (command-in via `ActionRequest`, event-out via a BiDi-style subscribe/delta surface) and wrap it in a small curated command set — not to build a browser-automation stack. The cautionary half is equally clear: do not inherit the browser's structural baggage (domains, frames, dual id spaces, version-coupled churn) that exists only because browsers are not Buiy — and do answer the two questions browsers never had to (multi-driver arbitration, and whether the engine can beat sampled readiness from the inside).

## Sources

- https://playwright.dev/docs/locators
- https://playwright.dev/docs/actionability
- https://playwright.dev/docs/api/class-locator
- https://playwright.dev/docs/api/class-elementhandle
- https://developer.chrome.com/blog/webdriver-bidi
- https://developer.chrome.com/blog/firefox-support-in-puppeteer-with-webdriver-bidi
- https://chromedevtools.github.io/devtools-protocol/tot/
- https://www.w3.org/TR/webdriver-bidi/
- https://github.com/ChromeDevTools/devtools-protocol
