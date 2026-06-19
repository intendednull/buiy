# Buiy — UI library foundation design

**Date:** 2026-05-07
**Status:** draft — "draft" here means this inventory is still accreting future feature areas as they graduate to their own specs; it does *not* mean the architecture is unsettled. The architectural foundation in [architecture.md](architecture.md) is stable.

## Purpose

Define the target shape of Buiy: a comprehensive UI library for the Bevy game engine, covering the modern web platform's UI feature set with full WCAG 2.2 AA accessibility, for both game and app UIs.

This spec is a **feature inventory and architectural foundation**, not an implementation plan. Each subsystem (layout, text, theming, a11y, widgets, etc.) graduates to its own design spec later. Implementation phasing lives in `docs/plans/`, not here.

The spec was written during a brainstorming session that consumed three deep-research reports (Bevy UI ecosystem, web platform feature inventory, accessibility deep-dive). Those reports are the substrate for the catalog and inform the architectural decisions.

## Children

This is a multi-file spec. The catalog is split across the children below; the parent (this README) holds goals, sub-spec roadmap, open questions, and references. Children inherit the parent's date.

- [architecture.md](architecture.md) — Architectural foundation: parallel UI stack, primitives we integrate directly, what Buiy owns, authoring (ECS + BSN), theming, a11y, reactivity, module organization, compatibility & policy.
- [visuals.md](visuals.md) — Document model & component hierarchy, layout, visual styling & rendering.
- [text.md](text.md) — Typography, text editing.
- [interaction.md](interaction.md) — Forms, events & input handling, animation & motion.
- [media-and-widgets.md](media-and-widgets.md) — Media & graphics, widget catalog (APG patterns).
- [accessibility.md](accessibility.md) — ARIA roles / states / properties, ACCNAME 1.2, focus management, keyboard interaction, screen-reader interop, user preferences, WCAG 2.2 SC enumeration table.
- [verification.md](verification.md) — Verification pipeline: CI gates, manual release gates, platform matrix, hot-reload trigger flow, tooling.
- [cross-cutting.md](cross-cutting.md) — i18n, state / data / reactivity, theming, devtools, 3D-anchored UI, compatibility & coexistence with bevy_ui.

Section numbers in cross-references map to children: 3.1–3.3 → visuals, 3.4–3.5 → text, 3.6–3.8 → interaction, 3.9–3.10 → media-and-widgets, 3.11 → accessibility, 3.12–3.14 + 3.16–3.18 → cross-cutting, 3.15 → verification.

## 1. Goals and non-goals

### Buiy's goals (the product)

1. **Comprehensive.** Feature parity with the modern web UI platform: HTML semantics, CSS layout / styling / animation surface area, ARIA roles and states, WAI-ARIA APG behavioral patterns, WCAG 2.2 success criteria, complex text (IME, BiDi, RTL, complex script shaping, emoji), the form-control set, drag-and-drop, clipboard, live regions. The web platform feature catalog produced during research is the master list we cull from, not an aspiration. Future web features (anchor positioning, container queries, view transitions, scroll-driven animations) are absorbable, not blocking.

2. **Accessible.** WCAG 2.2 AA is the floor. Every interactive widget ships with its APG keyboard contract, accessible name/role/value, focus management, AccessKit tree wiring. Forced-colors, reduced-motion, prefers-contrast, prefers-color-scheme are honored automatically from OS preferences.

3. **BSN-native.** Every Buiy component is small, public-fielded, observable, and decomposed by concern. No megacomponents, no private setters. BSN authoring works against Buiy components without adapter layers (the lesson of [bevy issue #17644](https://github.com/bevyengine/bevy/issues/17644)).

4. **Parallel to bevy_ui.** Buiy is a parallel UI stack — it integrates the same underlying primitives that bevy_ui uses (Taffy, cosmic-text, AccessKit, bevy_picking, Bevy's render graph) directly, with its own component model and render pipeline. The decision to go parallel rather than build on top of bevy_ui follows from the comprehensive-feature-parity goal: bevy_ui's renderer caps several capabilities (non-rect clipping, backdrop-filter, mix-blend-mode, isolation, true top layer) that web parity requires.

5. **Tracks Bevy.** Rolling latest-stable. No multi-version compatibility promise. Each Bevy minor release is a migration event for Buiy users.

6. **Game and app, both.** Buiy is the UI layer for anything built on Bevy. Productivity-app concerns (IME, complex text, screen readers, complex forms) and game concerns (gamepad nav, in-world UI anchoring, animation polish) are both in scope.

7. **Verifiable.** Every machine-testable claim Buiy makes (every widget behavior with a defined keyboard contract, every AccessKit tree shape, every theme variant's visual output, every layout primitive's resolved geometry, every machine-testable WCAG SC) is covered by automated tests that run in CI without a human approval gate. Claims that depend on human judgment, real OS subsystems, or physical devices (real-SR utterance verification, full OS-IME conformance, real-device mobile coverage, content-quality SCs, subjective visual quality) are documented as **manual release gates** with explicit owners, cadence, and release-blocking sign-off documents — not CI gates. [verification.md](verification.md) enumerates which tests live in CI vs at the manual release gate. Tradeoff acknowledgment: several user-experience claims (what an AT actually says, what real devices do, content quality, polish) sit at the manual tier; "fully automated verification pipeline" describes the CI tier specifically.

### Non-goals

- **Networking, persistence, routing/URL navigation, file system access, service workers, sandboxing.** UI is a presentation layer; data and transport are the consuming app's concern.
- **Game-side accessibility content** — audio description of gameplay, difficulty options, narrative aids, content warnings. Buiy provides the *UI primitives* (live regions, caption containers, settings widgets, remap UI); the game owns the substance.
- **A reactive component model with signals/computed/effects in v1.** Bevy's observers + change detection are the reactivity primitive. A signal-style layer is a follow-up sub-spec, not part of foundation.
- **Compatibility across Bevy minor versions.** Each Bevy minor release is a migration event.
- **Non-Bevy frontends.** No web target via WASM-without-Bevy, no SSR.
- **Replacing bevy_ui upstream.** Buiy stands parallel; bevy_ui and Buiy can both run in the same app (different windows).
- **Mixing Buiy and bevy_ui in the same UI tree (or same window).** See [cross-cutting.md § 3.18](cross-cutting.md).

### What this spec does

- Defines the architectural foundation: parallel to bevy_ui, BSN-friendly components, ECS + BSN authoring, token-based theming, AccessKit-first.
- Catalogs every feature/component by category, each tagged with a tier: **F** = foundation (without it nothing else works), **C** = core (any non-trivial UI needs it), **E** = extended (commonly needed but cuttable for a long time), **O** = out (explicitly excluded, with reason).
- Lists the subsystems that will receive their own design specs (Section 4 below).
- Records open questions for later resolution (Section 5 below).

### What this spec does NOT do

- Specify APIs in detail. Per-subsystem specs do that.
- Pick release phases or a timeline. Plans do that.
- Specify a single canonical UI style or design language. The default theme passes WCAG 2.2 AA; visual style is a theme concern.

### Tier legend

**F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason).

A small number of WCAG-tied items in [accessibility.md](accessibility.md) carry **dual tiers** of the form `F (AA) / C (AAA)`, where the conformance level (AA vs AAA) and the Buiy implementation tier differ. The convention applies only there; everywhere else tiers are single-valued.

## 4. Sub-spec roadmap

Each subsystem below graduates to its own design spec at `docs/specs/YYYY-MM-DD-<topic>-design.md` when it's that subsystem's turn to be designed. Each cites this foundation spec. Rows that have already graduated link to the spec on disk with their current status; the remaining rows are genuinely future and exist only as scope sketches here.

| Sub-spec | Scope |
|---|---|
| [`buiy-render-pipeline-design`](../2026-06-03-buiy-render-pipeline-design/README.md) | Render passes, top-layer compositing, clipping, filters, blend modes, atlasing, color management, render-graph node ordering. *(graduated — built: R1–R11 + the GPU-verify campaign all landed.)* |
| [`buiy-layout-design`](../2026-05-08-buiy-layout-design/README.md) | Taffy integration, anchor positioning, container queries, writing-mode integration. *(graduated — Phases 1–14 landed; remaining target features tracked in the spec.)* |
| [`buiy-text-rendering-design`](../2026-06-09-buiy-text-rendering-design/README.md) | cosmic-text integration, atlas management, font registration, fallback. *(graduated — built: text-rendering T1–T9 all landed.)* |
| text editing | IME composition, BiDi caret, undo/redo, multi-line, rich-text edit surface. *(graduated — realized as the [editing-and-ime.md](../2026-06-09-buiy-text-rendering-design/editing-and-ime.md) child of the text-rendering spec rather than a standalone spec, and built via the editing E1–E6 campaign.)* |
| `buiy-focus-model-design` | Focus tree, `:focus-visible`, traps, restoration, roving tabindex, gamepad spatial nav. |
| `buiy-accessibility-design` | AccessKit tree construction, decomposed components, ACCNAME 1.2, live regions, adapter ownership. |
| `buiy-theme-tokens-design` | Semantic tokens, theme assets, variants, OS-pref binding, contrast linter, APCA upgrade path. |
| `buiy-widget-catalog-design` | APG patterns shared infrastructure; per-widget specs nest as multi-file children. |
| `buiy-animation-design` | Transitions, keyframes, layout transitions, springs, reduced-motion gating. |
| `buiy-forms-design` | Form state machine, constraint validation, validation pseudo-classes, error-message model. |
| `buiy-input-events-design` | Pointer, keyboard, touch, gamepad, IME, drag-and-drop, `bevy_picking` backend registration + priority, drag a11y replacement contract. |
| `buiy-i18n-design` | BiDi, vertical writing, ICU, locale-aware formatters, calendar/numbering systems. |
| `buiy-3d-anchored-ui-design` | Billboards, worldspace UI, render-to-texture surface API, hit-testing. |
| [`buiy-verification-design`](../2026-06-15-buiy-verification-design/README.md) | Automated pipeline, harness API, WCAG-SC test fixtures + tolerances, CI matrix, manual release-gate cadences, perf baselines, `--accept` workflow. *(graduated — built: the `buiy_verify` harness landed.)* |
| `buiy-devtools-design` | Inspector, overlays, contrast checker, focus visualizer, theme editor. |
| `buiy-bsn-integration-design` | BSN authoring helpers, decomposed-component conventions, reflection-registration ergonomics, hot-reload semantics including component reload. |
| `buiy-asset-pipeline-design` | Theme assets, font assets, `.bsn` assets, icon atlases, vector assets, hot-reload semantics, asset GC, atlas-warmup strategy. |
| `buiy-coexistence-design` | (Conditional sub-spec — only if same-window coexistence with bevy_ui becomes required.) AccessKit-adapter coordinator, render-pass ordering across stacks, picking-backend priority across stacks, IME ownership, focus arbitration. |
| `buiy-window-and-surface-design` | Multi-window, render targets, render-to-texture contracts, off-screen rendering, fullscreen surface, top-layer per-window. |
| `buiy-clipboard-and-os-integration-design` | Clipboard, drag-drop OS interop, virtual keyboard hints, spellcheck OS bridge, system-color resolution, OS-preference plumbing. |

Each sub-spec gets one or more plans (`docs/plans/`) for implementation.

## 5. Open questions

- **Final crate split.** Single crate vs multi-crate workspace; if multi-crate, the exact partition. The spec commits to modular subsystems with clean boundaries; the partition can change.
- **Reactivity layer.** Observers + change detection only in v1. Whether to add a signal/computed/effect primitive in a follow-up sub-spec is open.
- **CSS-flavored stylesheet.** Never, or as a future layer above tokens? bevy_flair sets one precedent; the right answer depends on user demand.
- **Date/time pickers — Buiy-owned vs OS-delegated.** Buiy-owned per APG gives consistent visuals; OS-delegated is lighter. Spec defaults to Buiy-owned (consistency), but this is reversible.
- **WCAG 2.2 SC enforcement strategy.** Per-SC: automated CI check, runtime-honored constraint, or documented design constraint. The mapping table is owned by `buiy-verification-design`.
- **3D-anchored UI prioritization.** The renderer is ours and `Transform` works, so it's unblocked. Whether `buiy_3d` is concurrent with foundation work or strictly deferred is a planning choice.
- **Coexistence policy with `bevy_feathers` / `bevy_ui_widgets`.** Coexistence at the app level is committed; whether Buiy ships migration adapters from bevy_ui widgets is open.
- **Performance budgets — concrete numbers.** The CI gate ([verification.md § CI gate #14](verification.md)) is committed; the per-fixture *budget numbers* (target frame-time per fixture, allowed regression slack) calibrate over time and live in `buiy-verification-design`.
- **Platform support staging.** All platforms (Windows / macOS / Linux / Android / iOS / web) at v1, or staged?
- **Hot-reload of components (not just themes).** In scope as part of `buiy-bsn-integration-design`?
- **Render-to-texture surface API contract.** Feeds `buiy_3d`; the boundary is open.
- **Animation library substrate.** Roll our own springs, depend on `bevy_animation`, or wrap an existing crate?
- **OS spellchecker integration.** Where the OS exposes a spellchecker, Buiy uses it; where not, software fallback. The fallback library choice is open.
- **Real screen-reader testing in CI.** Currently out of CI (manual at release). If this becomes feasible (e.g., headless NVDA via vmnv tools), it becomes a CI gate.
- **AccessKit-adapter ownership when both stacks coexist same-window.** Currently the spec rules this out (per-window coexistence only). If demand arises, `buiy-coexistence-design` defines the coordinator.
- **AccessKit cadence policy decoupled from Bevy.** Whether AccessKit major releases between Bevy minors trigger a Buiy patch release with explicit semver, or are absorbed silently.
- **Reflection-registration ergonomics for BSN consumers.** Whether `register_type` calls are emitted by a derive macro on Buiy components, by a sub-plugin per crate, or by a single global plugin call.
- **Bevy WASM target policy.** Bevy supports WASM; the spec lists "non-Bevy frontends" as out, but Bevy-on-WASM is an in-scope Bevy target. Web a11y waits for AccessKit's web adapter; visual / input / layout work on WASM today. Whether v1 commits to WASM as a target platform is open.
- **Wayland vs X11 a11y differences.** AT-SPI behavior diverges between session types; whether Buiy ships Wayland-specific code paths or assumes AT-SPI parity is open.
- **APCA gate or advisory.** Currently APCA is advisory; WCAG 2 ratios are the gate. If WCAG 3 (which incorporates APCA) reaches recommendation status, the gate flips.
- **Real-device mobile CI staging.** [verification.md](verification.md) punts Android / iOS to manual release gate. Open question: budget and timeline for moving them into CI.
- **Crate-split refinement.** [architecture.md § 2.8](architecture.md) lists `buiy_core` as containing render + layout + focus + theme + a11y primitives. That may be too coarse; splitting into `buiy_render`, `buiy_a11y`, `buiy_layout`, `buiy_focus`, `buiy_theme` is open.

## References

- Bevy UI ecosystem report (research input, May 2026).
- Web platform UI feature catalog (research input, May 2026).
- Accessibility deep-dive (research input, May 2026).
- WAI-ARIA 1.2 — https://www.w3.org/TR/wai-aria-1.2/
- ARIA Authoring Practices Guide — https://www.w3.org/WAI/ARIA/apg/
- Accessible Name and Description Computation 1.2 — https://www.w3.org/TR/accname-1.2/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- AccessKit — https://accesskit.dev
- Bevy issue #17644 (`bevy_a11y` BSN-incompatibility, lesson source) — https://github.com/bevyengine/bevy/issues/17644
- Bevy discussion #14437 (BSN tracking) — https://github.com/bevyengine/bevy/discussions/14437
- Bevy discussion #16900 (Standard Headless Widgets) — https://github.com/bevyengine/bevy/discussions/16900
- Bevy issue #11100 (10 Challenges for Bevy UI Frameworks) — https://github.com/bevyengine/bevy/discussions/11100
