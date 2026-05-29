**Date:** 2026-05-22
**Status:** active
**Subject:** iced — known unsolved problems and forward-looking gaps

# Open problems

This file is the forward-looking complement to [`critiques.md`](critiques.md). Critiques names structural choices iced has made and won't easily reverse; this file names gaps that may yet close, plus the dependencies that block their closure. Each item ties back to a Buiy decision where relevant.

## AccessKit integration

**Status:** absent. Issue [#552](https://github.com/iced-rs/iced/issues/552) open since **2020-10-05** (~5.5 years). No PR has a credible landing path.

**Blockers:**
1. The function-based `Theme` model does not carry accessibility metadata (no analog to ARIA name/role/value as cascade properties).
2. iced widgets do not expose a stable identity (no entity-equivalent); AccessKit nodes need stable identity to survive view-rebuilds across frames.
3. The single-Model architecture has no built-in observer for "the visible widget tree changed."
4. Héctor has not personally championed the feature; the COSMIC team has built bespoke AccessKit wiring in `libcosmic` rather than upstream.

**Implications for Buiy:** Buiy's AccessKit-first commitment (foundation [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md)) intentionally rejects iced's path. Buiy's decomposed-component model gives each widget a stable `Entity` identity; the AccessKit adapter consumes those entities directly. The Buiy foundation [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) Avoid row "Megacomponents that are BSN-hostile" applies in spirit here too — iced's `Theme` + `Widget` pair is the equivalent of a megacomponent for a11y purposes.

## Mobile target maturity

**Status:** no first-class support. README lists only Win/Mac/Linux/Web. No iOS or Android target in CI. No touch-first widget design. No soft-keyboard handling, swipe gestures, drawer/sheet patterns, bottom-tab navigation.

**Blockers:**
1. winit's iOS and Android support is in flight but not as polished as desktop. iced consumes winit; whatever winit doesn't deliver, iced doesn't deliver.
2. Touch primitives are absent from the widget API (only `mouse::Event` and `keyboard::Event` are first-class).
3. The single-`Application` trait assumes one bootstrap function; Android's `Activity` and iOS's `UIApplication` lifecycles need integration points iced has not built.
4. Mobile rendering performance (battery, refresh-rate-aware rendering) has not been tuned.

**Implications for Buiy:** Bevy supports Android and iOS natively. Buiy inherits this — foundation [README § 1.6](../../specs/2026-05-07-buiy-foundation/README.md) lists "Game and app, both" as a goal, which implies mobile must work. The relevant open question in the foundation spec is "Platform support staging" (foundation README § 5) — whether mobile is in v1 or staged. Iced's experience confirms this is non-trivial work, not free.

## WASM target completeness

**Status:** "Web" listed in README; reality is limited. Works via wgpu's WebGL2 backend (`webgl` feature). No clipboard parity, no native file dialog, no multi-window, no system-font enumeration, no AccessKit web-adapter integration.

**Blockers:**
1. wgpu's WebGPU support is gated behind browser feature flags as of 2026-05-22; WebGL2 remains the production-safe path.
2. The single-`Application` assumption is desktop-shaped; the browser's lifecycle (visibility-change, back/forward, history) needs adapters iced hasn't built.
3. Web a11y depends on AccessKit's web adapter, which iced doesn't use at all (see above).
4. Bundle-size concerns — a full iced WASM build is multi-megabyte; production web apps target sub-500KB.

**Implications for Buiy:** Bevy's WASM target is in scope per foundation README § 5 ("Bevy WASM target policy" is an open question). Buiy inherits Bevy's wgpu→WebGL/WebGPU pipeline, the same substrate as iced. Buiy's WASM story will look similar to iced's unless Buiy invests in web a11y bridging (which depends on AccessKit's web adapter maturing).

## Grid / Flexbox-like advanced layout

**Status:** iced has its own layout engine, intentionally simpler than Taffy. The 0.14 `grid` widget is a fixed-shape table, not a CSS-Grid solver. No subgrid, no anchor positioning, no `position: sticky`.

**Blockers:**
1. The layout engine is internal and intertwined with iced's `Element` lifecycle. Replacing it with Taffy would be a large refactor.
2. Adding Grid support directly to iced's layout would duplicate Taffy's work.
3. There is no public roadmap commitment to Grid layout.

**Implications for Buiy:** Buiy commits to Taffy directly (foundation [architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) and inherits Grid, block layout, float, named-line grid, etc. for free. This is a Validates row in [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) — bevy_ui's 3.5 years on Taffy confirms the choice.

## Animation / transition primitives

**Status:** 0.14 added the Animation API for application code (PR #2757). It provides interpolation primitives and easing curves but does not provide:

- CSS-style style-property transitions (`transition: background-color 0.2s ease`).
- Layout transitions (children appearing/disappearing animate position).
- Spring physics.
- Gesture-driven animations.
- Scroll-driven animations / view transitions.

**Blockers:**
1. The Elm-architecture single-`update` per-event model doesn't naturally express "interpolate this style property over time." Animations need a time-driver injected into the update loop.
2. Pre-0.14, the Subscription::frame() pattern was the only path; 0.14 added explicit animation primitives but didn't add transition syntax.
3. No CSS-property-binding analog exists in iced's `Theme` system.

**Implications for Buiy:** Foundation `buiy-animation-design` (foundation [README § 4](../../specs/2026-05-07-buiy-foundation/README.md)) is committed to web-platform parity for transitions, layout transitions, springs, reduced-motion gating. Iced's animation API is below Buiy's bar.

## Multi-window support depth

**Status:** multi-window landed in 0.12 (PR #1964). Each window has its own view projection but shares the single Model. Inter-window communication is via Messages.

**Open subtleties:**
1. **Per-window AccessKit adapter ownership.** iced doesn't have AccessKit, so this is moot for iced — but for any future a11y integration, the per-window adapter ownership pattern matters. Buiy commits to per-winit-WindowId adapter (foundation [architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)), same as bevy_ui's pattern.
2. **Cross-window drag-and-drop.** Not natively supported.
3. **Window-level theming.** Per-window themes are achievable but require manual plumbing.
4. **Inter-window state synchronization.** Manual via Messages; no built-in shared-state primitives beyond "it's all one Model anyway."

## WCAG 2.2 SC coverage

**Status:** approximately zero coverage. Without AccessKit + accessible name/role/value computation + focus tree, most success criteria are unverifiable. Some visual SCs (text contrast — 1.4.3, focus visibility — 2.4.7, reflow — 1.4.10) could be tested in iced apps via manual or screenshot-diff testing but no infrastructure exists.

**Implications for Buiy:** Foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) commits to a CI-gated + manual-release-gated WCAG-SC verification harness. This is a foundational *differentiator* between Buiy and iced.

## Drag-and-drop ergonomics

**Status:** native-OS drag-and-drop is supported (drop files onto an iced window), but in-window widget-to-widget drag-and-drop is manual. No reorder-by-drag list primitive, no sortable-list, no drop-zone widget. Each iced app that needs drag-and-drop builds it from low-level pointer events.

**Implications for Buiy:** Foundation `buiy-input-events-design` commits to drag-and-drop as a first-class concern including a11y-compatible alternatives (drag-as-keyboard-operation, screen-reader announcements). Iced has neither.

## Touch / gamepad input

**Status:** touch events exist via winit's Touch event; iced widgets do not consume them in a touch-first way (no hit-test enlargement, no long-press semantics, no scroll-by-touch tuned thresholds). Gamepad input is not exposed at all by iced; apps that need it use `gilrs` directly and route messages back to their `update`.

**Implications for Buiy:** Bevy has first-class gamepad support via `bevy_input::gamepad`. Buiy inherits this. Touch needs explicit design — foundation [interaction.md](../../specs/2026-05-07-buiy-foundation/interaction.md) is the placeholder.

## State management at scale (Model size limits)

**Status:** anecdotal reports from COSMIC, Cryptowatch, Halloy contributors suggest the Model + Message pattern starts producing maintenance friction around 200+ Message variants. No formal upper-limit; this is a Rust-compile-time question (large enums slow compilation) and a developer-ergonomics question (jump-to-definition on Message becomes essential).

**Mitigations in practice:** screen-level decomposition (each screen has its own Model + Message; root Model wraps Each.Message variants).

**Implications for Buiy:** Buiy's ECS model has no equivalent ceiling — adding a component is local. This is a structural advantage of the ECS choice that is hard to communicate in benchmarks.

## The Parley question — *will iced migrate?*

**Status:** as of 2026-05-22, iced uses cosmic-text (via cryoglyph). The Linebender ecosystem's Parley + Vello stack is iced's main alternative substrate. The brief that produced this folder claimed iced uses Parley; this is **false** as of 0.14.

**Open question:** will iced migrate to Parley?

Considerations:
- The cryoglyph fork (March 2025) suggests iced is committing *deeper* to cosmic-text, not migrating away. Forking glyphon means owning the cosmic-text→wgpu adapter, which is opposite of what a migration would do.
- COSMIC is also cosmic-text-based (System76 is the cosmic-text steward).
- Bevy migrated `bevy_text` from cosmic-text → Parley + swash (issue [#21765](https://github.com/bevyengine/bevy/issues/21765)). Iced is not following.

**Implications for Buiy:** Buiy commits to cosmic-text (foundation [text.md](../../specs/2026-05-07-buiy-foundation/text.md), informed by [`/home/user/buiy/docs/prior-art/cosmic-text/lessons.md`](../cosmic-text/lessons.md)). Buiy and iced share this substrate even as bevy_ui moves away. The cosmic-text constituency is shrinking from bevy_ui's exit but staying stable from iced + COSMIC. See [`history.md`](history.md) § "0.10 — text-engine switch."

## Theme tokenization

**Status:** the 0.14 Oklch palette generation is a single-palette computation: each `Theme` produces one set of colors. There is no semantic-token layer ("color.danger.bg.hovered" → concrete color via cascade) and no per-subtree token override.

**Open question:** will iced add a token layer above `Theme`?

**Implications for Buiy:** Foundation `buiy-theme-tokens-design` (foundation [README § 4](../../specs/2026-05-07-buiy-foundation/README.md)) commits to semantic tokens + per-subtree override + OS-preference binding. This is materially richer than iced.

## Single-maintainer bus-factor

**Status:** see [`critiques.md`](critiques.md) § "Single-maintainer bus-factor risk." Open in the sense that no public succession plan exists; financial-runway-via-Kraken delays but does not resolve the concern.

## Sources

- iced issue #552 (accessibility) — https://github.com/iced-rs/iced/issues/552
- iced 0.14.0 CHANGELOG — https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- PR #2757 (Animation API) — https://github.com/iced-rs/iced/pull/2757
- PR #1964 (multi-window) — https://github.com/iced-rs/iced/pull/1964
- Bevy issue #21765 (bevy_text → Parley) — https://github.com/bevyengine/bevy/issues/21765
- cryoglyph (iced's glyphon fork) — https://github.com/iced-rs/cryoglyph
- cosmic-text — https://github.com/pop-os/cosmic-text
- Linebender Parley — https://github.com/linebender/parley
- Buiy foundation accessibility — /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md
- Buiy foundation verification — /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/verification.md
