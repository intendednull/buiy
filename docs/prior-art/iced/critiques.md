**Date:** 2026-05-22
**Status:** active
**Subject:** iced — honest critique of architectural and feature gaps

# Critiques

This file enumerates iced's structural and pragmatic limitations as they appear from a 2026-05-22 outside-the-project vantage. Critiques are framed for Buiy designers: what iced does *not* solve, where its design choices lock in tradeoffs, and where the Buiy foundation spec explicitly diverges. Companion to [`open-problems.md`](open-problems.md) (forward-looking gaps that may still be fixed) and [`comparisons.md`](comparisons.md) (head-to-head against neighbours).

## No CSS-style cascade

Iced theming is **function-based**: a widget's `Style` is produced by a closure `impl Fn(&Theme, Status) -> Style`. Themes are Rust types implementing the `Theme` trait, not declarative token bundles. Consequences:

- No cross-widget cascade. Setting "all buttons in this container have danger styling" requires plumbing a custom theme variant through the component tree or wrapping every button in a `themed!` macro.
- No CSS variables / custom properties. Iced 0.14 added Oklch-based palette generation, but it computes one palette at build/load time, not at render time per subtree.
- No `@media` queries, no container queries, no `prefers-color-scheme` propagation beyond what the OS layer surfaces.

For Buiy (foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to **semantic-token theming** at the design layer + per-subtree token overrides) this is a structural mismatch: Buiy needs cascade-equivalent semantics for design-system token swaps; iced does not provide them.

## No Grid layout

The layout engine is iced's own (see Agent A's [`layout-engine.md`](layout-engine.md)) — not Taffy, not Stretch, not Yoga. It implements a Flexbox-flavored row/column model with `pad`/`align`/`spacing`, plus the `stack` widget for layered children, plus `pin`/`float` for positioned children added in 0.14.

What's missing:

- **No CSS Grid.** The 0.14 `grid` widget is a fixed-shape table — equally-sized cells, no `grid-template-columns` / `grid-template-rows` / `grid-area` / named lines / span / auto-flow.
- **No subgrid.**
- **No anchor positioning.**
- **No `position: sticky`.**
- **No container queries.**
- **No `position: fixed`** beyond the `stack` widget's layered model.

For Buiy (foundation goals 1 + 4: web-platform parity, parallel to bevy_ui, **Taffy substrate** per [architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) — iced's layout engine is intentionally simpler than Taffy. Taffy ships Grid (since 0.3), block layout (0.4), float (0.10). Buiy commits to Taffy and inherits Grid for free; iced has chosen the simpler ceiling.

## No native AccessKit integration

Issue [#552](https://github.com/iced-rs/iced/issues/552) (*"Implement accessibility support"*) has been **open since 2020-10-05**, ~5.5 years as of 2026-05-22. There is no PR with a credible path to landing.

What this means in practice:

- No screen-reader support on Windows (no UI Automation tree), macOS (no NSAccessibility tree), or Linux (no AT-SPI tree).
- No accessible name / role / value computation per widget.
- No `aria-live` analog.
- No focus indicator beyond the visual `:focus`-style rendering iced does internally.
- No keyboard-only navigation guarantees beyond what the application author manually wires up.

This is the single largest gap relative to Buiy's foundation goal 2 (*"WCAG 2.2 AA is the floor"*) and foundation [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md). Buiy's commitment to AccessKit-first is informed *negatively* by iced's experience: even with a 5+ year backlog and known production demand (COSMIC's accessibility work happens at libcosmic + custom-AccessKit-wiring level, not in iced), iced has not added it.

The COSMIC team has reportedly built AccessKit integration *outside* the iced repo and used it in COSMIC apps — but this code has not been upstreamed in any form Héctor has accepted.

## Elm-architecture verbosity at scale

The Elm Architecture (Model / Message / update / view) is type-safe and easy to reason about for small applications. At scale (say, COSMIC Files or Cryptowatch desktop) the `Message` enum balloons into hundreds of variants and the `update` function becomes a giant match statement.

Established mitigations (used by COSMIC and Halloy):

- **Component splitting** — each screen becomes its own `(Model, Message, update, view)` quadruple, composed via parent enums that carry child messages.
- **Effect/Task chains** — async work returns `Task<Message>` values that compose; this is structurally clean but adds another type-level layer.

What it still costs:

- Refactors touch many files. Adding a field to the global Model means threading through every `update` arm that needs it.
- IDE jump-to-definition on `Message` variants is the single most-used navigation pattern.
- The global Model is a single state tree; Bevy's ECS (and Buiy's) provides finer-grained reactivity and parallelism.

This is a *deliberate* tradeoff iced makes. Compare egui's stateless-rendering simplicity (no Model at all) or signal-based reactives (Floem, Xilem) for the alternative end of the spectrum. See [`comparisons.md`](comparisons.md).

## Single state tree

Following from Elm-architecture: iced apps have **one** Model. Multi-window apps (since 0.12) share one Model across windows. Plugins can't carve out their own state. Workspaces / tabs / multiple documents all live in the same global tree.

For Bevy/Buiy this is alien: ECS gives you O(many) entities with O(many) components and queries scope to whatever subset you need. Buiy components are per-entity, observable, decomposed — exactly the inverse of iced's single-Model.

## WASM support is limited

The iced README lists "the Web" as a supported platform; the reality is more constrained:

- Built via wgpu's WebGL2 backend (the `webgl` feature). WebGPU support exists but is gated behind browser feature flags in 2026-05-22 Chrome/Edge/Firefox.
- No clipboard integration parity.
- No native-file-dialog integration.
- No multi-window (browser tabs are not winit windows).
- No system-font enumeration via fontdb — fonts must be bundled in the WASM binary.
- No screen-reader integration even at the level of forwarding to the browser's accessibility tree (the AccessKit web adapter is a separate concern, and iced doesn't use AccessKit at all).

iced examples build for the web; production iced apps targeting the web are rare. Most iced apps in awesome-iced are desktop-only.

## Mobile support is limited

- No iOS or Android platform listed in the README or Cargo.toml's target spec.
- No touch-first widget designs (the widget set assumes mouse + keyboard).
- No soft-keyboard handling.
- No swipe / pinch / multi-touch gestures.
- No mobile-style navigation patterns (drawer, bottom-tab, modal sheet, pull-to-refresh).

Slint and Dioxus invest in mobile; iced has not. See [`comparisons.md`](comparisons.md).

## Slower than egui at small surfaces (verify-required)

Per third-party benchmarks (e.g. [Sven-Hendrik Haase's GUI benchmarks](https://github.com/svenhh/gui-benchmarks), various reddit-thread microbenchmarks from 2023-2024), iced has higher per-frame cost than egui on simple immediate-mode-style surfaces. This is structural: iced does layout solving + theme resolution + change detection per frame; egui rebuilds from scratch with a flat command list and skips most layout state.

iced's 0.14 *reactive rendering* (PR #2662) closes the gap by re-rendering only dirty regions. Comparable benchmarks against 0.14 have not yet circulated publicly; the gap is likely narrowed but not eliminated.

iced is faster than egui on **large, retained, mostly-static** UIs because incremental layout dominates over batched rebuilds at scale. The crossover point is around several hundred nodes per author observation; precise numbers are not published.

## "Iced owns its own renderer" — the maintenance cost

iced ships `iced_wgpu` + `iced_tiny_skia` + `cryoglyph` (its own glyphon fork since March 2025). This buys:

- Full control over the GPU pipeline.
- No dependency on system widget kits (GTK, Qt, NSView, HWND).
- Consistent rendering across platforms.

But it also costs:

- Every `wgpu` major bump (22.0 → 27.0 in 0.14) is a porting event. `wgpu` is itself pre-1.0 and breaks API every few months.
- The cryoglyph fork commits iced to maintaining its own text-rendering adapter. The reason for forking was Héctor wanting iced-specific control; the cost is permanent.
- Bug-for-bug rendering parity with web browsers (for the WASM target) is impossible because the underlying renderer is not the browser's.

For Buiy: Bevy already owns a wgpu render graph; Buiy plugs into it (foundation [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) rather than running a parallel render pipeline. Iced is the existence proof that owning the renderer is *possible* and *commercially viable* — but not free.

## No animation primitives beyond manual subscription (pre-0.14); thin in 0.14

Pre-0.14, animation was: `Subscription::frame()` produced a tick event, your `update` handler advanced an animation Model field, your `view` re-rendered. Every animated interaction was hand-rolled.

0.14 added the **Animation API for application code** (PR #2757) which gives proper interpolation primitives, easing curves, and a less verbose API. The 0.14 animation API still does not provide:

- CSS-style transitions on style properties (`transition: background-color 0.2s ease-in-out`).
- Layout transitions (smooth resize of a container as children appear/disappear).
- Spring physics primitives.
- Gesture-driven animations (drag-to-dismiss, swipe-to-reveal).
- Scroll-driven animations.

For Buiy (foundation [interaction.md § 3.7 Animation](../../specs/2026-05-07-buiy-foundation/interaction.md) — to be split into a dedicated sub-spec) this is a feature gap to close, not a pattern to copy.

## Release-cadence drift

The 0.13 → 0.14 gap was 14 months. The 0.12 → 0.13 gap was 7 months. Major-version releases land when Héctor decides they are ready, not on a schedule. For projects (like Buiy) depending on iced, the unpredictability is a planning cost — Bevy's quarterly cadence is the contrast.

This is also tied to the absence of a public roadmap with dates (see [`governance.md`](governance.md) § "RFC / decision process").

## Maintainer review style

Per long-running threads on the iced Zulip and the awesome-iced PR history, Héctor tends to rewrite or reject community PRs that don't fit his architectural taste. This is consistent with the philosophy chapter's stance on personal-project autonomy. Effects:

- Code quality is high and consistent.
- Contributor friction is high. Several long-running community widget proposals never landed in core.
- Community widgets ended up in `iced_aw` (community-led) rather than upstream — see [`ecosystem.md`](ecosystem.md).

## Single-maintainer bus-factor risk

Héctor is the only person with commit on `iced-rs/iced`. If he stops, iced stops. The COSMIC team has the second-most contributors but does not have commit. This is structurally identical to Bevy-cart and Iced-Héctor's joint risk.

The Kraken/Cryptowatch sponsorship gives Héctor financial runway; this does not fix the bus-factor concern, only delays it.

## Sources

- iced book, Philosophy chapter — https://book.iced.rs/philosophy.html
- iced issue #552 (accessibility) — https://github.com/iced-rs/iced/issues/552
- iced CHANGELOG — https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- PR #2662 (reactive rendering) — https://github.com/iced-rs/iced/pull/2662
- PR #2757 (Animation API) — https://github.com/iced-rs/iced/pull/2757
- PR #1697 (text shaping overhaul / iced_wgpu refactor) — https://github.com/iced-rs/iced/pull/1697
- Buiy foundation architecture — docs/specs/2026-05-07-buiy-foundation/architecture.md
- Buiy bevy-ui lessons — docs/prior-art/bevy-ui/lessons.md
