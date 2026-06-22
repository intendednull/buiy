**Date:** 2026-06-18
**Status:** active
**Subject:** Chrome DevTools Protocol (CDP) — the domain-partitioned inspect+drive model, and its accessibility/input/screenshot surface for agents

# Chrome DevTools Protocol (CDP)

CDP is the wire protocol the Chrome DevTools front-end speaks to a Chromium back-end, and the same protocol Puppeteer (and historically Playwright's Chromium path) use to drive the browser. It is maintained by the Google / Chrome DevTools team as part of Chromium. It is **not** a W3C or standards-track protocol: it is versioned and coupled to the Chromium build (tip-of-tree vs. stable channels), with no cross-vendor governance.

The public type-definition repo is `github.com/ChromeDevTools/devtools-protocol` (**BSD-3-Clause**), published as the `devtools-protocol` npm package. Its README is explicit that it does *not* track definition/implementation issues — those go to crbug.com under DevTools — and that it mirrors/publishes the definitions rather than being the authoritative source. The browseable tip-of-tree reference is `chromedevtools.github.io/devtools-protocol/tot/`.

## Architecture: one socket, JSON-RPC, domain-partitioned

The transport is a single persistent **WebSocket** carrying language-agnostic, JSON-RPC-style messages. Each message is a `{id, method, params}` command that resolves to a `{id, result}` response, or a server-pushed `{method, params}` **event**. Because the connection is persistent and bidirectional, CDP can stream events (DOM mutations, navigations, console messages) — the property that later generalized cross-vendor as WebDriver BiDi (see [webdriver-bidi.md](webdriver-bidi.md)).

Methods and events are grouped into **domains**: DOM, Accessibility, Input, Page, Overlay, Runtime, Target, CSS, Console, Debugger, and more. Most domains are gated by an explicit `Domain.enable` / `Domain.disable` pair: you turn a domain on before its commands/events are live, and turning it off lets the back-end stop maintaining that machinery. This enable/disable gating is the cost-control knob — a back-end keeps the accessibility tree, DOM node map, or input plumbing warm only for the domains a client has explicitly opened.

The agent-relevant domains:

## Accessibility — the semantic-tree read surface

The Accessibility domain exposes the browser's computed AX tree (the same tree assistive technology consumes):

- **`Accessibility.getFullAXTree`** — fetches the entire AX tree for the root Document. Optional `depth` (max depth) and `frameId`. Returns an array of **AXNode** objects.
- **`Accessibility.getPartialAXTree`** — fetches an AX node and partial subtree for a given DOM node (`nodeId` / `backendNodeId` / `objectId`; optional `fetchRelatives`).
- **`Accessibility.queryAXTree`** — searches a DOM node's AX subtree for nodes matching a given **accessible name and/or role**. This is the role+name query primitive that user-facing locators are built on (cf. Playwright's `getByRole`; see [playwright-locators.md](playwright-locators.md)).
- **`Accessibility.getChildAXNodes`** — fetches children of a node by `AXNodeId`. This makes traversal **lazy**: a client can walk the tree incrementally instead of paying for the full snapshot, which matters because the full AX tree of a real page is large.

An **AXNode** carries `nodeId` (an `AXNodeId`), `ignored` / `ignoredReasons`, `role`, `chromeRole`, `name`, `description`, `value`, a `properties` array (focusable, disabled, expanded, checked, ...), `parentId` / `childIds` for tree navigation, `backendDOMNodeId`, and `frameId`. The `role` + `name` + state-bearing `properties` are exactly the semantic fields an LLM agent needs to perceive "what is on this page and what can I do to it."

## The backendDOMNodeId join — one identity across inspect + control

The load-bearing design feature: an AXNode's `backendDOMNodeId` links it back to its DOM element, and `DOM.getDocument` / `DOM.querySelector` mint and return the same `backendNodeId`. So the *same node identity* threads through inspection (Accessibility, DOM, CSS) and control (Input targets a position; Overlay highlights a `backendNodeId`; `getPartialAXTree`/`getBoxModel` accept a `backendNodeId`). A `backendNodeId` is the **durable join key**: a stable handle the back-end issues, distinct from the ephemeral `nodeId` the DOM domain hands out per session — it is the id that survives across domains and recomputes. (The `AXNodeId` is the handle for AX-tree traversal — `getChildAXNodes` takes one — but the protocol does not guarantee it as stable as `backendDOMNodeId`: AX node ids can be regenerated when the tree is recomputed, so the durable correlation is via `backendDOMNodeId`, not `AXNodeId`.) This single cross-domain identity — perceive a node in the AX tree, then act on the very same node — is the property Buiy already has natively (one AccessKit `NodeId` per entity).

## Input — synthetic events through the real pipeline

- **`Input.dispatchMouseEvent`** — dispatches a mouse event to the page (coordinates, button, modifiers, `clickCount`).
- **`Input.dispatchKeyEvent`** — dispatches a key event (press/release/raw, modifiers, optional editing commands).
- **`Input.insertText`** — "emulates inserting text that doesn't come from a key press, for example an emoji keyboard or an IME" — the non-keyboard text-entry path.

The key property: these inject input **low in the browser's event pipeline, not via JavaScript `dispatchEvent`**. Events from `EventTarget.dispatchEvent()` carry `isTrusted = false` and cannot be forged to `true`; many sites' security logic ignores untrusted events. CDP-dispatched input arrives as **trusted** (`isTrusted = true`), indistinguishable from genuine hardware input, because it enters below the JS sandbox. This is *why* automation tools route input through CDP rather than synthesizing DOM events — it drives the same hit-testing, focus, and default-action machinery a real user would. (The Buiy analogue: drive AccessKit `ActionRequest`s through the real `bevy_winit` channel rather than calling widget handlers directly — same "real event pipeline, not handler calls" principle.)

## Page / Overlay — screenshot and highlight

- **`Page.captureScreenshot`** — `format` (png/jpeg/webp, default png), `quality` (jpeg only), and a **`clip`** Viewport (`x`/`y`/`width`/`height`/`scale`) to capture a single region instead of the whole page. `captureBeyondViewport` and `fromSurface` are experimental flags. This is the raw pixel-perception channel — full-page or clipped to one node's box.
- **`Overlay.highlightNode`** — highlights a node (by `nodeId` / `backendNodeId` / `objectId`) using a `HighlightConfig` with separate `contentColor` / `paddingColor` / `borderColor` / `marginColor`, i.e. the full **box model**, plus `showInfo`, `showRulers`, `showAccessibilityInfo`. This is what paints the inspector's box-model overlay; for an agent it gives a verifiable visual of "which node did I resolve."

## Cost: Chromium coupling and version churn

CDP's power is also its liability. It is Chromium-only and proprietary — no Firefox/WebKit equivalent of the same surface. It tracks the Chromium build, so the protocol drifts release-to-release; methods are added, marked experimental, or change shape on tip-of-tree, and there is no stability contract across versions (the type-defs repo even disclaims being authoritative). The domain count is large and still growing — "domain sprawl" — so a client that leans on many domains is exposed to churn across all of them. The cross-vendor convergence answer is WebDriver BiDi ([webdriver-bidi.md](webdriver-bidi.md)), which reuses the persistent-WebSocket + events shape under W3C governance.

## Trust and automation detection

Two security-adjacent facts an agent-surface designer should hold together. First, CDP input is **trusted** (above): the whole reason to route through CDP is that synthesized input is indistinguishable from hardware input at the `isTrusted` boundary, so site logic that gates on `isTrusted` cannot tell an automation click from a human one. Second — and in tension with that — *attaching* the CDP perception surface is itself observable: enabling the `Runtime` domain has detectable side effects that anti-bot scripts watch for to fingerprint automation (see [open-problems.md](open-problems.md) item 1). So CDP is simultaneously *un*detectable at the input layer and *detectable* at the attach layer. For Buiy this trust/detection axis mostly dissolves — there is no untrusted-event boundary to forge past and no external attach to observe, because the agent and the widget tree share one process — but the *concept* (a real-input path vs. a side channel, and an arbitration story for who is driving) carries over; the multi-driver question Buiy must answer is flagged in [open-problems.md](open-problems.md).

## Implications for Buiy

- **Borrow the stable cross-domain identity.** CDP's `backendDOMNodeId` join — one node identity spanning perceive and act — is the right shape, and Buiy gets it for free: an AccessKit `NodeId` per entity already names the node in the semantic tree; making the tree bidirectional means acting on that same id. No DOM/AX dual mapping to maintain, and no AX-vs-DOM id-stability mismatch to manage.
- **Borrow `queryAXTree`'s role+name lookup** as the agent's primary locator primitive, and **borrow screenshot + highlight** as the verification channel (render the resolved node's box, capture pixels) — cheap to provide from a renderer Buiy already owns.
- **Avoid domain sprawl and vendor coupling.** CDP's cost is many enable/disable-gated surfaces tracking one engine's build. Buiy should expose *one* surface — the AccessKit tree it already authors — rather than a parallel protocol partition, and lean on AccessKit's cross-platform contract (see [../accesskit/](../accesskit/)) instead of inventing a Chromium-style versioned protocol.

See [actionability.md](actionability.md) for what "the action actually landed" requires beyond dispatching input, and [lessons.md](lessons.md) for the validates/borrow/avoid distillation.

## Sources

- CDP tip-of-tree reference: https://chromedevtools.github.io/devtools-protocol/tot/
- Accessibility domain: https://chromedevtools.github.io/devtools-protocol/tot/Accessibility/
- Input domain: https://chromedevtools.github.io/devtools-protocol/tot/Input/
- Page domain (captureScreenshot): https://chromedevtools.github.io/devtools-protocol/tot/Page/
- Overlay domain (highlightNode): https://chromedevtools.github.io/devtools-protocol/tot/Overlay/
- devtools-protocol repo (license, README scope): https://github.com/ChromeDevTools/devtools-protocol
- devtools-protocol npm package: https://www.npmjs.com/package/devtools-protocol
- Event.isTrusted (trusted vs synthetic events): https://developer.mozilla.org/en-US/docs/Web/API/Event/isTrusted
- Why CDP input is trusted (background): https://dev.to/ms_74/why-faking-real-browser-events-doesnt-work-4pp1
