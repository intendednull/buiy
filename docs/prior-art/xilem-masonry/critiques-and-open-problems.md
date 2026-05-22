**Date:** 2026-05-22
**Status:** active
**Subject:** Xilem + Masonry — honest critiques and unresolved open problems

# Critiques & open problems

This file is the no-marketing read of where Xilem + Masonry fall short, both relative to their own design ambitions and relative to a Buiy-shaped feature target. Honest tone, no hedging.

## Critiques

### 1. Pre-1.0 maturity, openly acknowledged

The README literally calls Xilem and Masonry "experimental." The 0.4.0 release notes call them "alpha-quality" and warn of ongoing breaking changes. This isn't a hidden critique — it's the project's own posture. The implication for any external evaluator: **do not depend on Xilem/Masonry for anything production-critical**, and expect 6–18-month migration cycles between minor releases.

For Buiy: this validates the spec's choice to *not* depend on either crate. Reference material only.

### 2. Small adoption beyond Linebender itself

Per [`ecosystem-comparisons.md`](ecosystem-comparisons.md): no verifiable third-party production deployments of Xilem or Masonry. The single non-trivial example (Placehero, Mastodon client) is in-tree and not generally available.

The download counts (~7.5K Xilem, ~17.7K Masonry) are pre-adoption-range for a UI framework — comparable to woodpecker_ui (~1K), substantially below Iced (~6-figure) or egui (~7-figure). The project's reach is the Linebender-aware Rust community, not the broader Rust application-development community.

**Counter:** the substrate crates (Vello, Parley, Kurbo) are widely adopted. The unbundling pays off there.

### 3. Linebender bandwidth split across many projects

Per [`distribution-governance.md`](distribution-governance.md): three named active leads (Raph Levien, Daniel McNab, Olivier Faure) plus rotating contributors. The same three names appear in:

- Xilem + Masonry development.
- Vello development (Raph especially).
- Parley development.
- Linebender governance / Zulip moderation / community work.
- Multiple smaller crates (Kurbo, Peniko, Color, etc.).

This means **any one of Xilem/Masonry doesn't have 3 full-time engineers** — it has 3 part-time leads who are also stewarding the rest of Linebender. The cadence math works out: ~10 months per Xilem minor release matches the ~10% of one lead's time available for Xilem-specific work after other commitments.

For Buiy: this is calibration data on what a small-collective-shipping-a-UI-framework can plausibly deliver per year. Buiy's own roadmap should not assume faster cadence than Linebender's without demonstrating a wider bus factor.

### 4. "Yet another Rust UI" reception risk

There are ~10 active Rust UI frameworks (Iced, Slint, egui, Dioxus, GPUI, Xilem, Masonry, Druid-legacy, bevy_ui, ratatui, plus smaller ones). The Rust UI fragmentation problem is real. Xilem's value proposition rests on:

- Best-in-class substrate (Vello + Parley).
- Cleanest reactive paradigm (per the paper's claims).
- Tight AccessKit integration.

**But:** these are all *substrate-level* arguments. End users tend to pick UI frameworks for *API ergonomics*, *third-party widget availability*, *theming flexibility*, *time-to-first-app*. Xilem doesn't yet have an obvious edge on any of those for end-user-app-developer onboarding. The risk is that Xilem gets locked into "the framework researchers respect but nobody ships."

This isn't an invalid criticism; it's a known dynamic. Buiy's positioning needs to be aware of the same risk (game-engine UI is a different niche, which helps).

### 5. The Flutter-style constraint-passing layout is becoming non-standard

The Rust UI ecosystem has largely consolidated on **Taffy** for layout: bevy_ui, woodpecker_ui, Dioxus desktop, Iced (in some configurations), kayak_ui (pre-abandonment). Slint has its own DSL-based layout. egui is per-frame manual.

Masonry's BoxConstraints-passing is the **Flutter / Druid lineage**, and it's increasingly the outlier in Rust. New CSS layout features (subgrid, container queries, anchor positioning, view-transitions) land in Taffy first; Masonry would need to implement each manually. This isn't fatal — Flutter ships a competitive UI on the same model — but it's a meaningful divergence from where the Rust ecosystem is heading.

For Buiy: this critique reinforces the Taffy commitment. See [`lessons.md`](lessons.md) Avoid row.

### 6. Documentation thin at the application level

Per [`distribution-governance.md`](distribution-governance.md): trait-level docs are good; application-building docs are scattered across blog posts; tutorials are minimal. A newcomer landing on Xilem has to reverse-engineer examples + Raph's blog series + monthly progress posts to assemble a mental model.

This is **typical pre-1.0**, but it shapes adoption. Iced has substantially better tutorials at a similar maturity level; egui has wide community resources; Slint has commercial docs. Xilem's docs gap is a real friction.

### 7. Single Apache-2.0 license

Per [`distribution-governance.md`](distribution-governance.md): Apache-2.0-only, not the MIT-OR-Apache-2.0 dual that Bevy + most of the Rust ecosystem uses. Limits cross-pollination with MIT-only downstream consumers.

For Buiy: this is a "noted but not deal-breaking" critique. Buiy is dual-licensed and won't lift code directly, only architectural patterns.

### 8. The release-notes "we plan to start keeping a changelog after this release" is concerning

The 0.4.0 release notes (2025-10-29) say "we plan to start keeping a changelog after this release" — meaning **no formal CHANGELOG existed for the first three minor releases**. Architectural changes between 0.1, 0.3, and 0.4 are documented in blog posts and release-page release notes, but not in a structured per-minor changelog.

This is a process gap. For projects integrating with Xilem/Masonry, version migrations are harder without a per-version cheat-sheet of breaking changes.

## Open problems

These are the unresolved technical / design questions Xilem + Masonry haven't yet answered:

### O1. Mobile target maturity

Android examples build as `cdylib` but the `accesskit_android` adapter is pre-1.0 ([`../accesskit/lessons.md`](../accesskit/lessons.md)). iOS isn't in scope at 0.4.0. The mobile story for Xilem is functionally **not addressed**.

For Buiy: same situation. Mobile is manual-release-gate per the foundation spec.

### O2. WASM / web target

`xilem_web` exists but uses the DOM, not Masonry/Vello. The Masonry-Vello-Parley path has no WASM target. So Xilem-on-web is architecturally a *different framework* than Xilem-on-desktop, with shared paradigm but different widget set.

A future where one Xilem codebase runs unchanged on desktop (Masonry-Vello) and web (DOM) requires either porting Vello to render in a `<canvas>` (architecturally large) or porting Masonry's widget contracts to the DOM (architecturally large). Neither is in flight at 0.4.0.

For Buiy: Bevy WASM target works for visual/input/layout; web a11y waits for AccessKit's web adapter. The story is roughly the same as Xilem's.

### O3. Theme / token system

Xilem has per-widget styling setters. There is **no theme primitive**, no semantic-token system, no light/dark variant binding, no OS-preference-driven variant switching. The 0.4.0 release added "styling properties" as a first-class widget concept, but the layer above (a theme that maps tokens to widget property values) doesn't exist.

For Buiy: theme is first-class (`buiy-theme-tokens-design`); this is a substantial Buiy advantage when it ships.

### O4. APG widget coverage

Masonry ships button, text input, label, slider, prose, flex, grid, list — call it ~15 widgets. The full WAI-ARIA APG catalog has ~60 patterns (combobox, listbox, tree, treegrid, menu, menubar, tabs, tab list, disclosure, alert, alertdialog, breadcrumb, carousel, dialog, link, radio group, switch, table, toolbar, tooltip, ...). Xilem's widget coverage is **~25% of APG**.

Adding the remaining 75% per APG keyboard contracts is a multi-engineer-year commitment. Linebender's cadence won't deliver this fast.

For Buiy: full APG coverage is the foundation goal (foundation [`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)). Even though Xilem won't get there soon, Buiy's commitment requires it.

### O5. Production usage / battle-testing

No commercial product ships on Xilem/Masonry that's verifiable. Without production deployment, edge cases in BiDi text editing, IME composition, accessibility-action-handling, multi-window focus management, drag-and-drop, etc. are likely uncovered.

This is the same situation as bevy_ui (no flagship game) and woodpecker_ui (no flagship app). The Bevy + Linebender ecosystems both lack the "this UI shipped to a million users" credibility signal.

### O6. Vello stability across hardware

Vello uses compute shaders. Compute-shader compatibility varies across GPUs (especially mobile GPUs, integrated GPUs, older drivers). Vello has worked through several rounds of "doesn't run on X" issues; there's no guarantee the next round won't surface.

For Buiy: Buiy's render pipeline is wgpu-based (potentially the same Vello-stability questions). The mitigation is to keep the renderer simpler than Vello's compute-shader-based path; trade off some capabilities for portability. The Buiy `buiy-render-pipeline-design` sub-spec is the place to make this tradeoff explicit.

### O7. Parley feature parity with cosmic-text

Parley has tighter AccessKit integration but lags cosmic-text on editor-shaped primitives (cursor management, IME composition, swappable backing strings). If Bevy 0.19's migration to Parley is followed by other text-editing-heavy projects, Parley will need to catch up. The catch-up timeline is unclear.

For Buiy: this is *Buiy's reason for choosing cosmic-text*. If Parley closes the editor-API gap, Buiy reconsiders. Not a high-likelihood event in the foundation timeline.

### O8. Reactive paradigm vs ECS data flow

Xilem's view-trees-as-pure-functions paradigm assumes the source-of-truth state is **one owned Rust value**. Bevy's ECS makes the source of truth **the world's component storage**, which isn't a single value. Bridging the two paradigms (ECS-as-reactive-source-of-truth) is non-trivial.

For Buiy: this is the *reason Buiy doesn't adopt the Xilem reactive paradigm in v1*. Bevy's observers + change detection are the v1 reactivity. A signal-style follow-up sub-spec (foundation [`README.md § 5`](../../specs/2026-05-07-buiy-foundation/README.md) open question) would have to design the ECS-reactive-bridge from scratch.

### O9. Multi-window focus / IME coordination

`masonry_winit` ships multi-window support. Whether focus transitions across windows, IME state ownership across windows, and accessibility-tree-per-window coordination work cleanly at scale is uncertain. The Placehero example doesn't stress multi-window.

For Buiy: foundation `buiy-window-and-surface-design` is an explicit sub-spec to address this; reference Linebender's implementation when sub-spec work begins.

### O10. Animation system

Xilem 0.4.0 ships a blinking text cursor as the showcase animation feature. There's no general animation primitive (no spring physics, no keyframes, no layout transitions, no reduced-motion gating). 0.5+ likely addresses this; not solved at 0.4.0.

For Buiy: animation is its own sub-spec (`buiy-animation-design`); Buiy ships first-class.

## Summary

Xilem + Masonry are the architectural reference point for "next-generation Rust UI substrate," but they are:

- Pre-1.0 with honest experimental labeling.
- Small-bandwidth (3 active leads sharing time across many crates).
- Slow-cadence (10 months per minor).
- Limited adoption beyond Linebender itself (substrate adoption higher).
- Feature-incomplete relative to a Buiy-shaped scope (theme, APG coverage, animation, i18n, devtools all missing).

The right Buiy posture is: study the substrate, study the Widget::accessibility shape, study the masonry_testing infrastructure, do not depend on the framework. See [`lessons.md`](lessons.md) for the codified decisions.

## Sources

- Xilem README + release notes: https://github.com/linebender/xilem
- Sibling files: [`distribution-governance.md`](distribution-governance.md), [`ecosystem-comparisons.md`](ecosystem-comparisons.md), [`history.md`](history.md), [`accessibility.md`](accessibility.md), [`text-and-rendering.md`](text-and-rendering.md), [`masonry-toolkit.md`](masonry-toolkit.md), [`xilem-architecture.md`](xilem-architecture.md)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)
- Cross-link: [`../woodpecker-ui/critiques.md`](../woodpecker-ui/critiques.md), [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)
