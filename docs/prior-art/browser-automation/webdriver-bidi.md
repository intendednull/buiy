**Date:** 2026-06-18
**Status:** active
**Subject:** WebDriver classic vs WebDriver BiDi — the W3C cross-vendor browser-control standard, and why a long-lived duplex channel changes what's possible

# WebDriver classic vs WebDriver BiDi

The W3C Browser Testing and Tools Working Group owns two protocols for remote
control of browsers. **WebDriver classic** is the older, HTTP request/response
standard (the formalization of Selenium's wire protocol). **WebDriver BiDi** is
its bidirectional WebSocket successor — the cross-vendor convergence of classic
and the CDP-style evented model (see [cdp.md](cdp.md)). They are not
either/or: a single session typically runs both at once.

## WebDriver classic

- **Standardization:** W3C **Recommendation**, published **05 June 2018**
  (`https://www.w3.org/TR/webdriver1/`). Level 1 is the stable Recommendation;
  the `/TR/webdriver/` landing now points at a Level 2 **Working Draft** (status
  line dated 28 May 2026) — cite Level 1 (`/TR/webdriver1/`) when you mean the
  cemented standard.
- **Transport:** a platform- and language-neutral **HTTP wire protocol**.
  Endpoints map to commands; the remote end reads an HTTP request, matches it to
  a defined endpoint, processes it, and returns a JSON-bodied response with a
  status code.
- **Strictly synchronous.** Pure command→response. There is **no
  server-initiated push** — the client cannot be notified of a console message,
  a network request, or a navigation as it happens; it can only ask and wait.
  This request/response ceiling is the structural limit BiDi and CDP remove.
- **Per-browser driver.** The client talks HTTP to a driver process
  (chromedriver, geckodriver, …) that translates to the browser's internal
  control surface. Cross-vendor by virtue of every vendor shipping a conformant
  driver to the shared spec.
- **Interaction model — act on an element handle.** You locate an element (e.g.
  `Find Element`, returning an opaque element reference) and then issue
  high-level commands against that handle: `Element Click`, `Element Send Keys`,
  `Element Clear`. The driver performs its own actionability checks before
  acting (see [actionability.md](actionability.md)).
- **Actions API (low-level input).** For anything the high-level commands can't
  express — drag, chorded keys, precise pointer paths, wheel scroll — classic
  defines an **Actions** model: multiple typed *input sources* (null for
  synchronization, keyboard, pointer, wheel) whose action lists are dispatched
  together by `Perform Actions`, and torn down by `Release Actions`. This same
  source-and-tick model is what BiDi carries forward as `input.performActions`.

## WebDriver BiDi

- **Standardization status:** W3C **Working Draft**, status line **"W3C Working
  Draft, 1 June 2026"** (`https://www.w3.org/TR/webdriver-bidi/`). It is on the
  Recommendation track but is **not** a Candidate Recommendation and **not** a
  Recommendation. It is a living standard; re-check the status line before
  citing, as it may advance.
- **Transport:** a **bidirectional WebSocket** carrying JSON messages, with a
  symmetric three-shape vocabulary:
  - **command** (client→browser): `{ "id": 1, "method": "module.command",
    "params": {…} }` — the numeric `id` lets many commands be in flight at once.
  - **result** (browser→client): `{ "id": 1, "result": {…} }` — matched back by `id`.
  - **event** (browser→client, unsolicited): `{ "type": "event", "method":
    "log.entryAdded", "params": {…} }` — delivered only after the client opts in
    via `session.subscribe`.
- **Module-namespaced like CDP, but standardized.** Methods read
  `module.command`, the same shape as CDP's `Domain.command`. The 2026-06-01
  draft defines exactly ten modules: **session, browser, browsingContext,
  emulation, network, script, storage, log, input, webExtension** (the list has
  grown over the draft's life). Named in this folder's scope:
  `browsingContext.navigate`, `browsingContext.captureScreenshot` (returns a
  Base64 PNG; full-page / OOPIF support has been added over time),
  `input.performActions` (the classic multi-device source/tick sequence model),
  `script.evaluate`/`callFunction`, and the `log` module's `entryAdded` event.
- **Explicitly evented.** The spec's own framing: BiDi "extends WebDriver by
  introducing bidirectional communication. In place of the strict
  command/response format of WebDriver, this permits events to stream from the
  user agent to the controlling software, better matching the evented nature of
  the browser DOM."
- **Cross-vendor, runs alongside classic.** Implemented in **Chrome and
  Firefox**; the typical deployment runs BiDi **and** classic concurrently over
  the same WebDriver session, so a client can keep classic's stable command
  surface and layer BiDi events/low-latency on top. (Mechanically, BiDi has its
  own session lifecycle — every command except `session.new`/`session.status`
  needs an active BiDi session.) Puppeteer added BiDi support to reach Firefox.
- **Extensible by external specs.** Other W3C/WHATWG specs can define their own
  BiDi modules/commands, so the protocol grows without a single vendor's build
  controlling it — the governance contrast with CDP, which is coupled to the
  Chromium build and has no cross-vendor process (see [cdp.md](cdp.md)).

## The throughline: a duplex channel buys events + low latency

Classic, CDP, and BiDi differ less in *what* you can ask than in *how* the
channel works:

| | WebDriver classic | CDP | WebDriver BiDi |
|---|---|---|---|
| Transport | HTTP req/resp | WebSocket | WebSocket |
| Server push (events) | none | yes | yes (after `subscribe`) |
| Message namespacing | endpoint paths | `Domain.command` | `module.command` |
| Governance | W3C Recommendation | Chromium-coupled, single vendor | W3C Working Draft, cross-vendor |
| Low-level input | Actions API | `Input.dispatch*` | `input.performActions` |

The HTTP-only design forces clients to **poll** for state and can't surface
asynchronous browser activity (console, network, navigation) as it occurs. A
**long-lived, bidirectional channel** is what lets the browser *push* those
events and lets many commands pipeline at low latency. CDP proved the model
within one vendor; BiDi is the cross-vendor, standardized version of the same
idea — and folds in classic's element-handle and Actions semantics so it can
replace classic rather than sit beside it forever.

## Implications for Buiy

The classic→BiDi arc is direct evidence for Buiy's bidirectional-channel thesis.
Classic could already *act on* a semantic element handle (`Element Click`), but
being HTTP-only it was perception-poor and push-incapable — exactly the
output-only posture Buiy's AccessKit tree is in today. BiDi's whole reason to
exist is to make the same control surface **duplex**: commands in, events out,
over one persistent connection. For Buiy that maps onto consuming AccessKit
`ActionRequest`s back through the existing `bevy_winit` channel and emitting tree
change events — turning an output-only semantic tree into a BiDi-shaped
command/result/event surface. The **event-out direction is the under-specified
half** of that thesis (BiDi details it via `session.subscribe` + per-module
events; the Buiy equivalent — what tree-change events to emit, at what
granularity, how an agent subscribes — is sketched in [lessons.md](lessons.md)
and flagged as open in [open-problems.md](open-problems.md)). See
[actionability.md](actionability.md) for the element-readiness checks both
protocols share, and [glossary.md](glossary.md) for terms.

## Sources

- WebDriver (Level 1) — W3C Recommendation, 05 June 2018: https://www.w3.org/TR/webdriver1/
- WebDriver — current TR landing (Level 2 Working Draft, 28 May 2026): https://www.w3.org/TR/webdriver/
- WebDriver BiDi — W3C Working Draft, 1 June 2026: https://www.w3.org/TR/webdriver-bidi/
- WebDriver BiDi repo: https://github.com/w3c/webdriver-bidi
- MDN — WebDriver BiDi modules and message shapes: https://developer.mozilla.org/en-US/docs/Web/WebDriver/Reference/BiDi/Modules
- browsingContext.captureScreenshot full-page discussion: https://github.com/w3c/webdriver-bidi/issues/384
