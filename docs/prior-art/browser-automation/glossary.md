**Date:** 2026-06-18
**Status:** active
**Subject:** Browser automation — glossary of protocol, accessibility-tree, locator, and actionability terms used across this folder

# Glossary — browser automation terms

Concise definitions for the vocabulary used throughout
[browser-automation/](README.md). Grouped loosely: wire protocols, the
accessibility-tree surface, locators / matching, and actionability. Each entry
cross-links the sibling file that treats it in depth.

## Protocols and transports

**CDP (Chrome DevTools Protocol)** — Google/Chromium's language-agnostic
JSON-RPC-style protocol over a **WebSocket**, partitioned into *domains* (DOM,
Accessibility, Input, Page, Overlay, Runtime, Target, …). Not a W3C standard;
versioned with the Chromium build. See [cdp.md](cdp.md).

**Domain** — CDP's unit of namespacing. Each domain (e.g. `Accessibility`,
`Input`, `DOM`) exposes a set of *commands* you call and *events* you subscribe
to. Method names are `Domain.method`, e.g. `Accessibility.getFullAXTree`,
`Input.dispatchMouseEvent`. See [cdp.md](cdp.md).

**WebDriver classic** — W3C **Recommendation** (Level 1, published 05 June 2018,
`https://www.w3.org/TR/webdriver1/`). Cross-vendor, HTTP request/response with
JSON bodies; synchronous command→response, **no server-initiated events**. The
formalization of Selenium's earlier wire protocol. (Note: the bare
`/TR/webdriver/` landing now resolves to a Level 2 Working Draft; cite
`/TR/webdriver1/` for the cemented 2018 Recommendation.) See
[webdriver-bidi.md](webdriver-bidi.md).

**WebDriver BiDi** — W3C **Working Draft** (dated 1 June 2026,
`https://www.w3.org/TR/webdriver-bidi/`) — *not* a Candidate Recommendation. A
bidirectional **WebSocket** protocol that combines classic WebDriver's
cross-vendor command set with CDP-style events. Modules in the 2026-06-01 draft:
session, browser, browsingContext, emulation, network, script, storage, log,
input, webExtension. See [webdriver-bidi.md](webdriver-bidi.md).

**Actions API** — WebDriver's low-level input model: ordered *action sequences*
across input sources (pointer, key, wheel) dispatched as synthesized OS-level
input rather than synthetic DOM events. Exposed in classic via the `/actions`
endpoint and in BiDi via `input.performActions`. See
[webdriver-bidi.md](webdriver-bidi.md).

**`input.performActions`** — The BiDi `input` module command that executes an
Actions-API payload (the tick-based pointer/key/wheel sequences) against a given
browsing context. The BiDi analogue of classic WebDriver's `/actions`. See
[webdriver-bidi.md](webdriver-bidi.md).

## Accessibility-tree surface

**`backendNodeId` / `backendDOMNodeId`** — A CDP-internal, stable integer handle
for a DOM node that is independent of the JS-visible `nodeId`/object id, and the
**durable join key** across CDP domains. AX nodes reference their DOM node by
`backendDOMNodeId`, letting a client correlate an accessibility node with the
element it describes without a round-trip through the page's JS context. See
[cdp.md](cdp.md).

**AXNode / `AXNodeId`** — CDP's accessibility-tree node (returned by the
`Accessibility` domain) and its id. An `AXNode` carries `role`, `name`, and
state properties (`focusable`, `disabled`, `expanded`, `checked`, …) plus child
ids — the browser's computed accessibility tree, the same one screen readers
consume. The `AXNodeId` is the handle for AX-tree traversal (`getChildAXNodes`
takes one) but is not guaranteed as stable as `backendDOMNodeId`; it can be
regenerated when the tree is recomputed, so durable correlation goes through
`backendDOMNodeId`. See [cdp.md](cdp.md).

**`getFullAXTree`** — `Accessibility.getFullAXTree`, the CDP command that fetches
the entire accessibility tree for the root Document (optional `depth`,
`frameId`), returning a flat array of `AXNode`s. The bulk-read primitive that a
full-page "semantic snapshot" of the browser is built on. See [cdp.md](cdp.md).

**Accessible name** — The string label a UI element exposes to assistive tech
(and to semantic locators), computed by the W3C **Accessible Name and
Description Computation** ("accname") algorithm: a priority walk over
`aria-labelledby` → `aria-label` → native labeling (`<label>`, `alt`, `<legend>`,
…) → text content. `getByRole(role, { name })` matches against it. See
[playwright-locators.md](playwright-locators.md) and [../wai-aria-apg/](../wai-aria-apg/).

**ARIA snapshot** — A serialized (YAML in Playwright) representation of the
accessibility tree under a node — roles, names, and nesting — used for
assertions (`toMatchAriaSnapshot`) and as a compact textual page model.
Playwright's `ariaSnapshot()` gained `{ boxes: true }` (bounding boxes) and
`{ mode: 'ai' }` around v1.59–1.60. See [playwright-locators.md](playwright-locators.md).

## Locators and matching

**Element handle** — A direct, possibly-stale reference to one DOM node obtained
from a prior query (Puppeteer's `ElementHandle`, classic WebDriver's element
reference). Contrast with a *locator*, which is re-resolved on each use. See
[playwright-locators.md](playwright-locators.md).

**Locator** — Playwright's lazy, *re-resolving* element reference: a description
(selector/role/text) that is matched fresh each time an action runs, so it
survives DOM churn that would stale an element handle. The unit auto-waiting and
strict mode attach to. See [playwright-locators.md](playwright-locators.md).

**`getByRole`** — Playwright's accessibility-tree-first locator: match an element
by its ARIA/HTML **role** (and optionally accessible `name`, plus state filters
like `checked`, `expanded`). The recommended, user-facing way to locate — it
targets the same semantics assistive tech sees. See
[playwright-locators.md](playwright-locators.md).

**Strict mode** — Playwright's default: an action errors if the locator resolves
to more than one element, forcing the test to disambiguate rather than silently
act on the first match. `force` does not relax this. See
[playwright-locators.md](playwright-locators.md).

## Actionability

**Actionability** — Playwright's pre-action gate: it runs an action only when the
locator resolves to exactly one element that is **visible, stable (same bounding
box across two animation frames), receives events (hit-targetable), and enabled**
(plus **editable** for `fill`; `attached` is the base existence check). See
[actionability.md](actionability.md).

**Auto-waiting** — Playwright polling the actionability checks until they pass
(or the timeout fires) before each action, instead of the caller inserting fixed
sleeps. Removes most explicit waits and the flakiness they paper over. See
[actionability.md](actionability.md).

**`force`** — `{ force: true }` "bypasses the actionability checks" (the spec's
wording) and acts anyway; default `false`. In practice it drives the input
without waiting on the gate (visibility / stability / receives-events) — an
escape hatch that trades safety for control. See [actionability.md](actionability.md).

**Hit-targetable** — The condition the *receives-events* check verifies: at the
element's action point, a hit-test of that pixel returns the element itself (or a
descendant), i.e. nothing — an overlay, modal, or transparent layer — would
intercept the click. The geometric counterpart to "visible." See
[actionability.md](actionability.md) and [open-problems.md](open-problems.md).

**Set-of-marks (SoM)** — A *vision*-agent technique (not part of these
protocols): overlay numbered bounding boxes on a screenshot so an LLM can refer
to an element by its integer label, which the harness maps back to pixel
coordinates. Listed here as the screenshot-first alternative to consuming the
accessibility tree directly — a recurring contrast in
[open-problems.md](open-problems.md) and [lessons.md](lessons.md). (Origin:
Yang et al., "Set-of-Mark Prompting," arXiv:2310.11441.)

## Sources

- CDP reference (tip-of-tree): https://chromedevtools.github.io/devtools-protocol/tot/
- CDP Accessibility domain (`getFullAXTree`, AXNode, backendDOMNodeId): https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/
- WebDriver (Level 1) — W3C Recommendation, 05 June 2018: https://www.w3.org/TR/webdriver1/
- WebDriver BiDi — W3C Working Draft (1 June 2026): https://www.w3.org/TR/webdriver-bidi/
- Playwright actionability: https://playwright.dev/docs/actionability
- Playwright locators (`getByRole`, strict mode): https://playwright.dev/docs/locators
- Playwright ARIA snapshots: https://playwright.dev/docs/aria-snapshots
- W3C Accessible Name and Description Computation 1.2: https://www.w3.org/TR/accname-1.2/
- Set-of-Mark Prompting (Yang et al., 2023): https://arxiv.org/abs/2310.11441
