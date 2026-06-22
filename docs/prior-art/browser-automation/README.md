**Date:** 2026-06-18
**Status:** active
**Subject:** Browser automation protocols (CDP / WebDriver / WebDriver BiDi / Playwright) — folder overview, key facts, reading order

# Browser automation protocols — prior-art folder

## Framing disclosure

These documents are written from Buiy's **AccessKit-semantic-tree-first agent-surface stance** and are a *learn-from* artifact, not a neutral catalog of browser automation. Buiy is an AccessKit-first Bevy ECS UI framework that already authors an AccessKit semantic tree (role + name + state + actions), today output-only; the thesis these files serve is that that same tree is the right LLM-agent perception+control surface, made bidirectional by consuming AccessKit `ActionRequest`s through the existing `bevy_winit` channel. Every "Implications for Buiy" / "Why this matters" passage reads the evidence through that lens, and the cross-protocol "convergence" narrative is selected to test that thesis — a neutral survey would weight these systems differently. The evidence files stay factual (verified versions, dates, spec text); Buiy design *decisions* are confined to [lessons.md](lessons.md) as validates / borrow / avoid. Read accordingly.

## What this folder is

A reference dossier on the four protocols/toolkits that drive a real browser from outside it — Chrome DevTools Protocol (CDP), W3C WebDriver "classic", W3C WebDriver BiDi, and Playwright — plus enough Puppeteer/Selenium context to place them in lineage. The angle that matters to us is narrow: **how an external agent perceives and controls a live UI**. Over roughly a decade these systems converged on the same primitives — stable node identities, *semantic* (accessibility-tree / ARIA-role) locators rather than brittle pixel or DOM-path coordinates, synthetic input delivered through the browser's *real* event dispatch path, and screenshot-on-demand. That is the same shape a Buiy agent surface over the AccessKit tree wants. These files are an index plus factual evidence; the design takeaways for Buiy live in [lessons.md](lessons.md), not in the evidence files.

This is an **index, not a deep dive** — each sibling carries the detail.

## Key facts

| Protocol | Model | Transport | Standard? | Maintainer | Spec / repo |
|---|---|---|---|---|---|
| **CDP** | JSON-RPC-style, domain-partitioned (DOM, Accessibility, Input, Page, Overlay, Runtime, Target…); commands + server-pushed events | WebSocket | No — coupled to Chromium build, no cross-vendor governance | Google / Chrome DevTools team | [type-defs repo](https://github.com/ChromeDevTools/devtools-protocol) (BSD-3-Clause); [tot reference](https://chromedevtools.github.io/devtools-protocol/tot/) |
| **WebDriver classic** | Command→response, synchronous; no server-initiated events | HTTP + JSON | **W3C Recommendation, 05 June 2018** (Level 1) | W3C Browser Testing and Tools WG | [TR/webdriver1](https://www.w3.org/TR/webdriver1/) |
| **WebDriver BiDi** | Bidirectional; combines classic commands + CDP-style events | WebSocket | **W3C Working Draft, 1 June 2026** (Rec track, *not* CR, *not* Rec) | W3C Browser Testing and Tools WG | [TR/webdriver-bidi](https://www.w3.org/TR/webdriver-bidi/); [repo](https://github.com/w3c/webdriver-bidi) |
| **Playwright** | High-level API over a wire protocol; auto-waiting actionability; a11y-tree-first locators | (driver-internal; speaks CDP / BiDi under the hood) | No — library, not a standard | Microsoft | [repo](https://github.com/microsoft/playwright); npm `playwright` 1.61.0, Apache-2.0 |

Context, not protocols in their own right: **Puppeteer** (Node library, drives Chrome via CDP, Firefox via BiDi; npm `puppeteer` 25.1.0, Apache-2.0) and **Selenium** (the WebDriver lineage; W3C WebDriver is the formalization of Selenium's earlier wire protocol; npm `selenium-webdriver` 4.45.0, Apache-2.0). All version/date/license/URL facts verified 2026-06-18 against the W3C TR, the npm registry, or the canonical GitHub repo.

> Version churn: Playwright bumps roughly weekly and Puppeteer/Selenium move fast — re-check npm for the current version before citing. WebDriver BiDi is a *living* Working Draft on the Recommendation track; its module list grows and its status line may advance — re-check the live status line before writing anything stronger than "Working Draft".

## How to use this folder

Start with this README for the lay of the land, then read in the [reading order](#canonical-reading-order) below. The two protocol files ([cdp.md](cdp.md), [webdriver-bidi.md](webdriver-bidi.md)) are the load-bearing evidence; [playwright-locators.md](playwright-locators.md) and [actionability.md](actionability.md) are the highest-signal files for Buiy because they describe the *semantic-locator* and *can-I-act-yet?* problems an AccessKit agent surface faces directly. [open-problems.md](open-problems.md) records where these systems still struggle, and [lessons.md](lessons.md) is the only file that draws Buiy conclusions (as validates / borrow / avoid). [glossary.md](glossary.md) defines the cross-cutting vocabulary.

## Contents (siblings)

- [cdp.md](cdp.md) — Chrome DevTools Protocol: domain model, the Accessibility / DOM / Input / Page / Overlay domains, `getFullAXTree` and friends, why it is Chromium-coupled.
- [webdriver-bidi.md](webdriver-bidi.md) — WebDriver classic → BiDi: the request/response ancestor, the bidirectional successor, its modules (session, browser, browsingContext, emulation, network, script, storage, log, input, webExtension), `input.performActions`.
- [playwright-locators.md](playwright-locators.md) — `getByRole` / `getByLabel` / `getByText`, ARIA snapshots (`ariaSnapshot()`, `toMatchAriaSnapshot()`), the accessibility-tree-first locator model.
- [actionability.md](actionability.md) — Playwright's auto-waiting actionability checks (visible, stable, receives-events, enabled, editable; `attached` base check; `strict`; `force`).
- [open-problems.md](open-problems.md) — where browser automation still hurts: shadow DOM / iframes, flakiness, a11y-tree completeness, cross-vendor drift, automation detection, multi-driver arbitration.
- [lessons.md](lessons.md) — **Buiy implications only**: what to validate, what to borrow, what to avoid for an AccessKit agent surface.
- [glossary.md](glossary.md) — shared vocabulary across all of the above.

## Glossary stub

Full definitions live in [glossary.md](glossary.md). The terms you need before reading the evidence files:

- **AX tree / accessibility tree** — the semantic tree a browser exposes to assistive tech (role, name, state, actions); the perception surface these protocols increasingly target. Buiy's AccessKit tree is the direct analog.
- **Backend node id / stable node id** — a handle that survives across calls so an agent can re-reference a node without re-querying by selector.
- **Semantic locator** — finding an element by user-facing meaning (role + accessible name) instead of CSS/XPath/coordinates.
- **Actionability** — the precondition set an automation tool checks before acting (visible, stable, hit-testable, enabled).
- **Synthetic input through the real path** — dispatching input so it travels the browser's genuine event pipeline, not a side channel, so handlers fire as a user would trigger them.
- **BiDi** — bidirectional; a transport that lets the remote end push events, not just answer commands.

## Canonical reading order

1. **README.md** (this file) — orientation + key-facts table.
2. **[cdp.md](cdp.md)** — the event-rich, Chrome-only ancestor; establishes the domain/AX-tree vocabulary.
3. **[webdriver-bidi.md](webdriver-bidi.md)** — the cross-vendor W3C lineage: classic request/response, then the bidirectional convergence of classic + CDP.
4. **[playwright-locators.md](playwright-locators.md)** — the semantic-locator and ARIA-snapshot layer (the direct analog of a Buiy `getByRole`-over-AccessKit query).
5. **[actionability.md](actionability.md)** — the can-I-act-yet? gate that sits between a locator and a synthetic action.
6. **[open-problems.md](open-problems.md)** — the still-unsolved edges.
7. **[lessons.md](lessons.md)** — Buiy conclusions, last, once the evidence is in hand.

[glossary.md](glossary.md) is reference material — consult it as needed rather than reading it straight through.

## Why this matters for Buiy

Buiy already authors an AccessKit semantic tree (role + name + state + actions) — today output-only, for screen readers. Browser automation spent a decade discovering that this *same* tree is the right surface for programmatic perception and control: stable node ids, semantic (a11y / ARIA-role) locators, synthetic input through the real event path, and screenshot-on-demand. Playwright's `getByRole` / ARIA-snapshot model is the direct analog of querying Buiy's AccessKit tree, and WebDriver BiDi shows the bidirectional shape (consume action requests, push events back). The thesis these folders serve — make the AccessKit tree bidirectional by consuming `ActionRequest`s through the existing `bevy_winit` channel — is exactly the convergence point this whole industry arrived at independently. The consumer of that surface (an LLM agent harness) is out of scope for *this* folder; it would belong in a future `llm-agent-interface/` prior-art folder, not yet authored.

## Sources

- https://github.com/ChromeDevTools/devtools-protocol
- https://chromedevtools.github.io/devtools-protocol/tot/
- https://www.w3.org/TR/webdriver1/
- https://www.w3.org/TR/webdriver-bidi/
- https://github.com/w3c/webdriver-bidi
- https://github.com/microsoft/playwright
- https://registry.npmjs.org/playwright/latest
- https://registry.npmjs.org/puppeteer/latest
- https://registry.npmjs.org/selenium-webdriver/latest
