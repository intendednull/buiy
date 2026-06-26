**Date:** 2026-06-25
**Status:** active
**Subject:** Web/WASM rendering path of the Bevy + wgpu + winit stack — accessibility on the web

Accessibility in this stack splits cleanly into two layers, and only one of them works on the web:

- **The a11y TREE** — pure data: a snapshot of roles, labels, states, relations, focus (AccessKit's `TreeUpdate`). It is platform-neutral and compiles and builds correctly on wasm.
- **The platform SINK** — the adapter that pushes that tree into the OS/browser assistive-technology (AT) API. On native this is a real adapter; **on web it does not exist.**

The consequence is the headline fact of web a11y for canvas-rendered UI: a build can compile, build a *correct* tree every frame, and reach **no** browser AT — silently, with no panic or warning.

## AccessKit (0.24) platform adapters

AccessKit ships **five** platform adapters: **Windows** (UI Automation), **macOS** (NSAccessibility/AppKit), **Unix/Linux** (AT-SPI D-Bus), **Android** (`accesskit_android` 0.7.3), and **iOS** (`accesskit_ios` 0.1.0 — brand-new, "basic"). `accesskit_winit` 0.32 wires the desktop + Android adapters via target-gated deps (`accesskit_winit-0.32.2/Cargo.toml`: `accesskit_unix` :96, `accesskit_android` :111, `accesskit_macos` :116, `accesskit_windows` :119); **iOS shipped as a standalone `accesskit_ios` crate not yet integrated into `accesskit_winit`** (no `ios.rs` in its `platform_impl/`).

Only the **web/canvas adapter is unshipped** — it is listed under "Planned adapters," and there is no `accesskit_web` crate on crates.io (the API returns 404). One stale-source caveat: AccessKit's own "How it works" page still lumps the not-yet-available adapters verbatim as *"Android, iOS, web (for applications that render their own UI elements to a canvas)"* — but the Android and iOS adapters have since shipped, so **web is the only one genuinely missing**. The siblings in this corpus record the per-crate versions — see [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md) ("Web / DOM adapter — Not shipped") and [`../bevy-a11y/open-problems.md`](../bevy-a11y/open-problems.md) §O7. For canvas-rendered UI on the web, the one missing adapter is exactly the one that matters.

**The intended web design** (per long-standing AccessKit discussion, captured in [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)) is *not* a parallel JS tree: the web adapter would emit a **hidden DOM mirror with ARIA attributes** alongside the canvas, so web ATs (NVDA + Chrome, VoiceOver + Safari) read the shadow tree. Slint's web target reportedly uses this shadow-DOM approach; Bevy's nascent web-a11y thinking assumes the same.

## `accesskit_winit` (0.32) on web = no sink

Native Bevy creates one `accesskit_winit::Adapter` per window (in `bevy_winit`'s `prepare_accessibility_for_window`) and stores it in the `ACCESS_KIT_ADAPTERS` thread-local; producers push `TreeUpdate`s into it each frame. On wasm there is **no web backend** for `accesskit_winit` — the per-window slot holds only a null/no-op adapter (per the feasibility report). Whether the slot is empty or holds a no-op adapter, the net is identical: `update_if_active` is inert and the tree reaches no AT. The build looks "a11y-complete" and exposes nothing.

## How egui/eframe handle web a11y (the same wall)

egui integrates AccessKit (PR #2294), default-on in eframe, and gets real screen-reader support on **native** Windows/macOS. On the **web** it hits the same gap: AccessKit lacks a web backend, so eframe-on-web has no AccessKit a11y. eframe instead ships an **experimental, opt-in built-in screen reader** (enabled in the demo's "Backend" tab) — a text-to-speech read-out driven from egui's own state, not an integration with the browser's AT stack. (Status as of egui's documentation; this experimental reader has been in flux.) The takeaway: no mature Rust canvas UI has solved browser AT yet; everyone is waiting on the same upstream adapter or hand-rolling a DOM mirror.

## Path to real web a11y

Two routes, both substantial:

1. **Hidden-DOM / ARIA overlay**, driven by the producer's existing `TreeUpdate` output — render an off-screen DOM mirror with `aria-*` attributes that web ATs read, kept in sync with the canvas tree each frame. This is the architecturally "correct" web model (DOM-aligned ARIA, not a parallel tree).
2. **Wait on upstream** `accesskit_web` — which, when it ships, is expected to implement route 1 generically so producers reuse their tree unchanged.

## Implications for Buiy

- **The data/sink split is already there.** Buiy's tree builder, `crates/buiy_core/src/a11y/translate.rs`, is deliberately **winit-free pure data** — `build_tree_update` produces an AccessKit `TreeUpdate` with no platform dependency. The sink lives separately in `a11y/adapter.rs`, whose `push_tree_updates` loops over `ACCESS_KIT_ADAPTERS` and calls `update_if_active`. On wasm `translate.rs` compiles and builds a correct tree; `adapter.rs`'s loop runs against an empty / no-op adapter set and reaches no AT.
- **A future web sink reuses the tree builder verbatim.** Because the sink consumes a finished `TreeUpdate`, a hidden-DOM/ARIA web sink would consume the **same** `build_tree_update` output — exactly as the in-process driver (`a11y/inprocess.rs`) already consumes that tree for tests and LLM agents with no winit adapter at all. The data layer is web-ready; only the sink is missing. This is the payoff of the split.
- **"No AT" is not "no semantic tree."** Even web-AT-less, the same tree remains consumable in-process — by Buiy's test harness and by the bidirectional AccessKit agent interface — so a wasm build is still introspectable and drivable; it just doesn't reach a *browser* screen reader.
- **Ship-honesty is mandatory.** Any web milestone must disclose that a11y reaches no browser AT, silently. Buiy's documented stance already matches: web a11y is manual-release-gated and deferred until either upstream `accesskit_web` ships or Buiy writes a DOM sink (foundation `architecture.md §2.9`; [`../bevy-a11y/open-problems.md`](../bevy-a11y/open-problems.md) §O7). Web ships a11y when the sink exists, not before. See also [lessons](lessons.md).

## Sources

- AccessKit — How it works (shipped vs planned adapters; "Android, iOS, web (for applications that render their own UI elements to a canvas)"): https://accesskit.dev/how-it-works/
- AccessKit repository (platform-adapter status): https://github.com/AccessKit/accesskit
- egui — "Implement accessibility APIs via AccessKit" (PR #2294): https://github.com/emilk/egui/pull/2294
- egui / eframe README + docs (AccessKit lacks a Web backend; experimental built-in screen reader): https://github.com/emilk/egui and https://docs.rs/crate/eframe/latest
- `crates/buiy_core/src/a11y/translate.rs`, `crates/buiy_core/src/a11y/adapter.rs` (lines 23-82), `crates/buiy_core/src/a11y/inprocess.rs`
- `docs/reports/2026-06-25-wasm-browser-support-feasibility.md` (§6 "Accessibility reaches no screen reader on web")
- Sibling prior-art: [`../accesskit/platform-adapters.md`](../accesskit/platform-adapters.md), [`../bevy-a11y/open-problems.md`](../bevy-a11y/open-problems.md)
