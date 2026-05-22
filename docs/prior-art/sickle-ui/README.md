**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — widget library on top of bevy_ui, declared obsolete by its maintainer after Bevy 0.15

# sickle_ui

`sickle_ui` was a third-party widget library for Bevy, sitting **on top of** `bevy_ui` (not parallel like `bevy_lunex`). It provided a fluent widget-builder DSL (`UiBuilder`), a theming system with pseudo-states (`Theme<T>` + `PseudoState`), a state-driven dynamic-style engine (`DynamicStyle` + `FluxInteraction`), and a fixed catalog of ~30 widgets covering inputs, layout containers, and menu primitives.

**Status — archived as of this writing.** The maintainer (UkoeHB, who took the last-published fork after the original `UmbraLuminosa/sickle_ui` repository went 404 / was removed) posted the following notice on the surviving fork's README:

> `sickle_ui` has been made obsolete by changes introduced in Bevy 0.15.0 and will not be publicly maintained. This is the last release, compatible with Bevy 0.14.2.

Pinned to Bevy **0.14.2**, last published version **0.4.0 on 2024-10-03** (~19 months stale at this Buiy doc's date of 2026-05-22). The library never crossed the Bevy 0.15 boundary, never integrated with `bevy_ui_widgets` (which landed in 0.17) or `bevy_feathers` (which landed in 0.17 / iterated in 0.18). The canonical `UmbraLuminosa/sickle_ui` GitHub repository returns **404**; the official `bevyengine/bevy-assets` UI listing contains no `sickle_ui.toml`.

It still matters as prior-art because (a) its **widget-builder DSL** is a clean Rust expression of the fluent-UI pattern other Bevy UI libs (haalka, cobweb, kayak) all converge on; (b) its **DynamicStyle + FluxInteraction** decomposition is the right shape for state-driven styling regardless of whose render pipeline runs underneath; (c) the `sickle_ui_scaffold` substrate was salvaged into `bevy_cobweb_ui` as `cob_sickle_ui_scaffold` — the design survived even though the project didn't.

## Key facts

| Field | Value |
|---|---|
| Crate | `sickle_ui` |
| Latest stable | **0.4.0** (2024-10-03) |
| Latest pre-release | (none — 0.4.0 is the last) |
| Bevy version pin | **0.14** (last known good: 0.14.2) |
| License | MIT OR Apache-2.0 |
| Total downloads (crates.io) | 15,120 |
| Recent downloads (90 days) | 517 |
| Companion crates | `sickle_macros` 0.4.0, `sickle_math` 0.4.0, `sickle_ui_scaffold` 0.4.0 (13,911 downloads) |
| Maintainer | UmbraLuminosa (original, repo deleted); UkoeHB hosts the surviving fork |
| Original repo | `https://github.com/UmbraLuminosa/sickle_ui` — **404, deleted** |
| Surviving fork | `https://github.com/UkoeHB/sickle_ui` (last-release archive) |
| Active fork? | `danec020/sickle_ui` (9 stars, last commit 2025-02-27, still on Bevy 0.14) |
| Listed in bevy-assets? | **No** (verified by `bevyengine/bevy-assets/Assets/UI/` directory listing 2026-05-22) |
| Documentation coverage | docs.rs reports **2.9% documented** |

## Staleness assessment — read this before considering sickle_ui for anything

Five independent signals converge on "do not adopt":

1. **Primary-source obsolescence notice.** The surviving fork's README declares the project obsolete. This is not inference; it is the maintainer's stated position. See [history.md § "The obsolescence notice"](history.md).
2. **The original repository is deleted.** `UmbraLuminosa/sickle_ui` returns 404; only forks remain. The `UmbraLuminosa` org itself still has one public repo (`Proof-of-Concept-Editor-in-Bevy`) but no `sickle_ui`.
3. **19 months since last release** (2024-10-03 → 2026-05-22) with no 0.5.0-pre branches, no Bevy-0.15+ migration commits on the surviving fork, no PRs trying to update against any subsequent Bevy minor (0.15, 0.16, 0.17, 0.18).
4. **Removed from official discovery.** No entry in `bevyengine/bevy-assets/Assets/UI/`. New Bevy users will not encounter sickle_ui through official channels.
5. **The ecosystem replaced it.** `bevy_ui_widgets` (headless primitives, Bevy 0.17+) and `bevy_feathers` (styled widget kit, Bevy 0.17+) now occupy the same niche as official Bevy crates. See [`../bevy-feathers/architecture.md`](../bevy-feathers/architecture.md) and [`../bevy-ui-widgets/`](../bevy-ui-widgets/) for the successors.

For an app committing to sickle_ui in 2026: you would be picking up an unmaintained Bevy-0.14 library, with a deleted upstream, no migration path forward, and a known-better official replacement two minor releases ahead. Buiy treats sickle_ui as **historical prior art**, not as an integration target.

## Table of contents

- [`architecture.md`](architecture.md) — how sickle_ui extends bevy_ui; plugin shape; the UiBuilder DSL; module layout; DynamicStyle + FluxInteraction.
- [`widgets.md`](widgets.md) — full enumeration of the ~30 widgets and their components/events/spawn extensions.
- [`api.md`](api.md) — the extension-trait DSL pattern, theme/PseudoTheme/PseudoState, custom-widget creation, BSN-compat assessment.
- [`integration.md`](integration.md) — `SickleUiPlugin` setup, feature flags, Bevy 0.14 pin, coexistence story.
- [`history.md`](history.md) — 0.1 → 0.4 timeline, the Bevy 0.15 cliff, the obsolescence notice, post-archive forks.
- [`distribution.md`](distribution.md) — license, MSRV-class info, governance (solo maintainer + bus factor), removal from bevy-assets.
- [`ecosystem.md`](ecosystem.md) — production usage (sparse), comparisons against bevy_feathers / bevy_ui_widgets / bevy_lunex / haalka / bevy_cobweb_ui / bevy_egui / Buiy.
- [`critiques.md`](critiques.md) — what doomed the project, accessibility gaps, BSN-hostility of the extension-trait DSL, and the lingering open problems.
- [`lessons.md`](lessons.md) — **the consult-this-when-designing decision file.** Validates / Avoid / Borrow for Buiy.
- [`glossary.md`](glossary.md) — sickle-specific terms.

## Glossary stub

Full glossary at [`glossary.md`](glossary.md). Quick reference:

- **`UiBuilder<E>`** — the heart of sickle_ui. A typed wrapper over `Commands` + an entity that exposes the widget-spawn vocabulary via extension traits. The fluent DSL anchor.
- **Extension trait (`Ui*Ext`)** — sickle's authoring pattern: each widget module ships a `Ui<WidgetName>Ext` trait implemented for `UiBuilder<Entity>`. Calling `ui.button(...)` is dispatched through one of these traits.
- **`FluxInteraction`** — sickle's pointer-state state machine: `None / PointerEnter / PointerLeave / Pressed / Released / PressCanceled / Disabled`. Replaces (and supersedes for sickle's purposes) Bevy's `Interaction`.
- **`DynamicStyle`** — a component that resolves visual styling against the current `PseudoStates` and `FluxInteraction` on every change. The state-on-style engine.
- **`PseudoState`** — enum of CSS-pseudo-class-equivalent states (e.g. `Checked`, `Open`, `Selected`, `OddChild`, plus folder-state and flex-direction variants). Drives `PseudoTheme<C>` lookup.
- **`Theme<C>` / `PseudoTheme<C>`** — typed theme component for component `C`; carries `DynamicStyleBuilder` closures keyed by `PseudoState` lists.
- **`UiContext`** — trait letting a widget expose named sub-entities (e.g. a slider's "bar", "handle") so themes can target them by string identifier.

## How to use this corpus

Each subsystem file is independently skimmable. If you are…

- **picking a Bevy UI library in 2026** — read [`README.md`](README.md) (this file) and [`critiques.md`](critiques.md). Conclusion: do not pick sickle_ui.
- **designing a Buiy widget DSL** — read [`api.md`](api.md), [`widgets.md`](widgets.md), [`lessons.md`](lessons.md). Borrow the `FluxInteraction` + `DynamicStyle` shape; avoid the extension-trait-on-UiBuilder dispatch pattern.
- **understanding why third-party Bevy UI is hard to keep alive** — read [`history.md`](history.md) and [`ecosystem.md`](ecosystem.md). Solo maintainer + Bevy minor releases as breaking-migration events = high abandonment risk.
- **deciding whether your app should migrate off sickle_ui** — read [`history.md`](history.md) and [`ecosystem.md`](ecosystem.md) (comparison table). Targets: `bevy_feathers` for editor-style UI; `bevy_cobweb_ui` (also archived as of 2026-01-13, ironically) for asset-driven UI; Buiy for production game/app UI with a11y.

## Framing disclosure

These docs are written from a Buiy-stance — Buiy is a parallel-to-bevy_ui UI stack committed to BSN-friendly decomposed components, AccessKit-first a11y, and tracking-latest-Bevy as a hard policy. Many "Implications for Buiy" notes frame sickle_ui's choices through that lens. In particular: sickle_ui *extends* `bevy_ui` (an inheritance model Buiy explicitly rejects in foundation [architecture.md § 2](../../specs/2026-05-07-buiy-foundation/architecture.md)) and its extension-trait DSL is the kind of authoring shape Buiy specifically wants to avoid because BSN-as-data-format cannot statically reach trait-dispatched method calls. Future readers auditing whether the parallel-stack and BSN-friendliness commitments are themselves correct should weigh the corpus accordingly: it's a learn-from-sickle_ui-into-Buiy-stance artifact, not a neutral catalog.

A second disclosure: this corpus describes a project that **as of the writing date is no longer actively maintained**, with a deleted upstream repository. Future versions of sickle_ui or any "sickle_ui 2" continuation should be evaluated independently of this corpus. If the project revives, archive this folder rather than updating it.

## Sources

- Crate index — https://crates.io/crates/sickle_ui
- Companion crate — https://crates.io/crates/sickle_ui_scaffold
- crates.io API — https://crates.io/api/v1/crates/sickle_ui
- docs.rs — https://docs.rs/sickle_ui/0.4.0/sickle_ui/
- Surviving fork (with obsolescence notice) — https://github.com/UkoeHB/sickle_ui
- Active fork (Bevy 0.14, no migration) — https://github.com/danec020/sickle_ui
- Original repo (404) — https://github.com/UmbraLuminosa/sickle_ui
- Maintainer org — https://github.com/UmbraLuminosa
- Bevy assets UI listing (no sickle_ui entry) — https://github.com/bevyengine/bevy-assets/tree/main/Assets/UI
- bevy_cobweb_ui successor (also archived) — https://github.com/UkoeHB/bevy_cobweb_ui
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling prior-art — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../bevy-feathers/architecture.md`](../bevy-feathers/architecture.md)
