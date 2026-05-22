**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — Validates / Avoid / Borrow decision file for Buiy

# Lessons for Buiy

**This is the consult-this-when-designing decision file.** The other files in this corpus are evidence; this file is the synthesis. **Freya is not a substrate Buiy could be built on** — Freya's Skia C++ dependency, Dioxus reactivity coupling, and desktop-only platform reach are structurally incompatible with Buiy's wgpu + Bevy ECS + cross-platform commitments. The lessons here are about (a) which Freya design choices Buiy can borrow at the *shape* level, (b) which structural pitfalls Buiy must avoid by virtue of Bevy-not-Skia substrate, and (c) what the existence of Freya tells us about the viability of solo-maintainer Rust GUI work.

## Top-of-file: two findings that reframe Buiy decisions

### 1. Dioxus's reactivity primitives are reusable beyond Dioxus core — but importing them costs the Dioxus coupling

Freya is the existence proof: a non-Dioxus renderer (Skia + Torin + AccessKit, none of which Dioxus owns) successfully consumes Dioxus signals + `rsx!` + components + scope-tree-reconciliation as a *library*. The reactivity layer works outside Dioxus's official renderer matrix.

The cost: **the whole Dioxus 0.6.x API surface flows through into Freya** — every Dioxus breaking change is a Freya migration, and Freya's release cadence is constrained by Dioxus's. Two pre-1.0 ecosystems composed multiplicatively.

**Restated rule for Buiy:** Foundation [§ 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md#27-reactivity) defers signals to a future sub-spec. **Good call.** If Buiy ever ships signals (foundation [open question § Reactivity layer](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)), build them directly on Bevy ECS using the *shape* validated by Dioxus + Leptos ([`../dioxus/lessons.md`](../dioxus/lessons.md) Borrow #1: `Signal<T>: Copy` via `generational-box`) — don't depend on Dioxus the crate. Freya shows the shape works; Freya also shows the import cost.

### 2. Skia gives you a massive renderer-feature surface for free — and then you can't go anywhere else

Freya's CSS-platform feature surface (rounded corners, gradients, shadows, blur, blend modes, SVG, color emoji, BiDi text) is essentially *"whatever Skia hands us."* That's a colossal feature-surface delivery in 3.5 years of solo work — Marc Espín did not have to implement any of those primitives.

The cost: **Skia owns the substrate.** Freya cannot become a Bevy plugin, cannot deeply integrate with non-UI rendering work, cannot port to mobile/WASM without inheriting Skia's mobile/WASM build complexity, and cannot intervene per-glyph or per-shaping-run in text. The renderer is a black box from the embedder's perspective.

**Restated rule for Buiy:** Buiy's wgpu-via-Bevy-render-graph commitment (foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly)) pays a *higher implementation cost* per primitive — every gradient, every shadow, every blur is a custom shader Buiy writes — and gets back **substrate composability**: 3D-anchored UI ([foundation cross-cutting § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)), per-pixel control, mobile/WASM as Bevy ships them, pure-Rust audit surface. The list of Skia primitives Freya exposes ([`skia-rendering.md`](skia-rendering.md)) is Buiy's wgpu-shader checklist.

## Validates

These Buiy design choices are confirmed by Freya's experience:

- **AccessKit as the native a11y substrate** (foundation [§ 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md#26-accessibility-accesskit-first)). Freya, Slint, Floem, Iced, egui, Bevy (via `bevy_a11y`), GPUI all depend on AccessKit. The de-facto-standard claim is real. See [`accessibility.md`](accessibility.md) and [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md).
- **Token-style theming with subtree overrides** (foundation [§ 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system)). Freya's `use_theme`/`use_init_theme` pattern is the React-Context analog; Buiy's `Theme` component on a subtree is the BSN analog. Both shapes work; Buiy's token-asset-with-OS-pref-binding extension is strictly more capable.
- **Single-substrate commitment** (foundation [non-goal § 1.3](../../specs/2026-05-07-buiy-foundation/README.md)). Freya is desktop-only and 3.5 years in still does not have mobile/WASM. Single-substrate-on-Bevy-platform-matrix is the realistic ambition; multi-renderer ambition (per Dioxus) burns years.
- **AccessKit-first applied uniformly across all targets** ([`../dioxus/lessons.md`](../dioxus/lessons.md) Validates #2 — webview-AT-for-free is unsustainable). Freya's native target requires AccessKit; Buiy targets Bevy's all-native platforms; the same AccessKit-first commitment applies.
- **No multi-version backward-compat promise** (foundation [§ 1.5](../../specs/2026-05-07-buiy-foundation/README.md)). Freya 0.3 → 0.4 is a substantial rewrite (PR #1351). Pre-1.0 Rust UI frameworks rewrite substantial portions of themselves between minors — promising back-compat is a trap.
- **CSS-flavored property naming where the concept maps cleanly.** Freya's `padding`, `background`, `corner_radius`, `main_align`, `cross_align` mirror CSS for user familiarity. Buiy adopts CSS-aligned names in BSN components where the mapping is clean.
- **Solo-maintainer Rust UI projects can be active long-term.** 3.5 years of nights-and-weekends from one person, currently rc.19. That's a real existence proof — Buiy's commitment can sustain similar discipline (with the explicit goal of avoiding bus-factor-1).

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Skia C++ substrate.** ~500K lines of C++, 20–40MB binary tax, CMake + Clang + Python build chain, opaque-to-embedder text and rendering. | [`skia-rendering.md`](skia-rendering.md), [`critiques.md § 1`](critiques.md) | Buiy commits to wgpu via Bevy's render graph + custom shaders. Pure Rust, audits cleanly, ports to Bevy's platform matrix including WASM. Foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly). |
| **Single-paradigm marriage to an external reactivity ecosystem.** Freya is married to Dioxus 0.6.x; Dioxus breaking changes flow into Freya unavoidably. | [`reactive-model.md`](reactive-model.md), [`critiques.md § 3`](critiques.md) | Foundation [§ 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md#27-reactivity) — Bevy observers + change detection only in v1. If signals ship, they're Buiy-owned, not a Dioxus dep. |
| **Own layout engine.** Torin is a one-consumer crate; Marc bears its entire maintenance. Excluded from Taffy's ecosystem improvements (subgrid, container queries, anchor positioning). | [`layout-and-styling.md`](layout-and-styling.md), [`critiques.md § 6`](critiques.md) | Foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly) — Taffy. Shares maintenance with Blitz, bevy_ui, Servo, and the rest of the Rust ecosystem. |
| **Stringly-typed styling props.** Runtime parse errors, no IDE autocomplete on values, no refactor safety. | [`layout-and-styling.md`](layout-and-styling.md), [`critiques.md § 7`](critiques.md) | Foundation [§ 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class) — BSN-typed components with `Reflect`-based asset hydration. Compile-time + IDE wins. |
| **Theme-as-Rust-struct.** No hot-reload, no OS-pref auto-binding, no asset-pipeline integration, no designer workflow. | [`layout-and-styling.md § Themability`](layout-and-styling.md), [`critiques.md § 8, § 11`](critiques.md) | Foundation [§ 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md#25-theming-token-based-design-system) — token assets, hot-reloadable, OS-pref-driven variants. |
| **Single-maintainer governance.** Marc Espín is the sole strategic owner; design discussions live in Discord; no documented succession. | [`distribution.md § Governance`](distribution.md), [`critiques.md § 4`](critiques.md) | Foundation [open question § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) — Buiy governance must structure for multiple committers from day one. Design discussions live in `docs/specs/`, not chat. |
| **Pre-1.0 with no ship-date commitment + rc churn for months.** Freya 0.4 has been in rc since 2026-02 with no public 0.4.0 commit-by date. Users either pin a stable 0.3.x missing all the new work or chase rcs. | [`history.md`](history.md), [`critiques.md § 2`](critiques.md) | Buiy versioning must commit to documented stability boundaries per sub-spec. Each foundation sub-spec graduates with its own version-stability commitment. |
| **AccessKit-as-dep-only-not-as-CI-claim.** Freya has the AccessKit dependency but the per-widget APG conformance and ACCNAME 1.2 compliance are not independently verified. | [`accessibility.md`](accessibility.md), [`critiques.md § 9`](critiques.md) | Foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) — APG-conformance + a11y-tree-snapshot tests in CI for every widget. The AccessKit claim is the *verified* claim. |
| **Reliance on Subsecond-or-equivalent without an explicit coverage matrix.** Freya inherits Subsecond hot-reload from Dioxus without documenting which Freya constructs hot-reload and which require restart. | [`critiques.md § 10`](critiques.md), [`../dioxus/open-problems.md § Hot-reload`](../dioxus/open-problems.md) | Foundation [open question § Hot-reload of components](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions) and `buiy-bsn-integration-design` — explicit coverage matrix. |
| **No public flagship app validation.** 33K downloads, 3.5 years, no production-app showcase. Edge cases unstress-tested. | [`ecosystem.md § Production users`](ecosystem.md), [`critiques.md § 5`](critiques.md) | Foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) — synthetic-app harness in CI exercising real production patterns. Doesn't replace real users but de-risks the unknown-unknowns. |
| **Misattribution of substrate.** Pre-corpus brief said Freya uses cosmic-text; Freya in fact uses Skia textlayout. Common community misattribution. | [`README.md § Brief corrections`](README.md), [`skia-rendering.md`](skia-rendering.md), [`../cosmic-text/lessons.md` line 35](../cosmic-text/lessons.md) | When citing Buiy's dependency lineage in any spec, verify the workspace `Cargo.toml` on the date of citation. Don't inherit substrate claims from older sources. |
| **Treating Torin as a borrow target.** Torin is custom-shape, not flexbox-spec-compliant, no subgrid/grid/container-queries. | [`layout-and-styling.md`](layout-and-styling.md) | Foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly) — Taffy. Don't borrow Torin's API shape; borrow CSS-aligned naming from Freya's *element attributes*, which are CSS-named regardless of the underlying engine. |

## Borrow

Concrete primitives worth studying (and possibly adapting into Buiy's own layers):

1. **The Skia primitive set as a wgpu-shader checklist.** Freya exposes (rounded clip, linear/radial/conic gradients, drop shadow, inner shadow, backdrop blur, color filter, blend mode, SVG render, color emoji, BiDi text, rotation/scale/skew). If Buiy's `buiy-render-pipeline-design` ships a wgpu shader for each, Buiy has visual parity with Freya. See [`skia-rendering.md`](skia-rendering.md).

2. **CSS-aligned element attribute names.** Freya's `padding`, `corner_radius`, `background`, `direction: vertical`, `main_align`, `cross_align`, `shadow`, `opacity`, `rotate`, `cursor`, `blend_mode`, `backdrop_blur`. Where Buiy's BSN components carry equivalent concepts, the field names should match CSS / Freya naming to ease user transition. (Don't copy the stringly-typed parsing — keep the names.) See [`layout-and-styling.md § CSS-flavored styling props`](layout-and-styling.md).

3. **Hook-as-renderer-state-accessor pattern.** Freya's `use_focus` / `use_theme` / `use_animation` / `use_canvas` / `use_node` exposes Freya-renderer state to reactive components via a per-renderer hook namespace. If Buiy adds signals (open question), the same shape — Buiy-renderer-specific accessors for focus / theme / layout / animation state — composes cleanly with Bevy ECS. See [`reactive-model.md § Freya-specific hooks`](reactive-model.md).

4. **Subsecond-style hot-reload at sync points** — already captured in [`../dioxus/lessons.md`](../dioxus/lessons.md) Borrow #6. Freya is the existence proof that Subsecond works in a Skia-backed app, not just web/desktop-webview. Buiy's BSN hot-reload story will lean on the same sync-point primitive.

5. **`use_canvas` escape hatch.** Freya's `use_canvas` hook gives a component a raw Skia `Canvas` for custom paint when the declarative model is insufficient. Buiy's equivalent — a Bevy component with a custom render-pass callback — should be on the foundation menu somewhere (likely `buiy-render-pipeline-design`). The escape hatch matters; pure declarative UIs always need one.

6. **Single-maintainer-quality discipline.** Despite the bus-factor risk, Marc Espín's 3.5-year delivery quality is impressive for the substrate complexity. Lessons in *what one disciplined maintainer can sustainably commit to* are calibrating data for Buiy's resourcing decisions. (Cross-link: Slint's ~10-person team for analogous scope.)

7. **`rsx!`-with-renderer-specific-elements pattern.** Freya reuses Dioxus's macro tokenizer + parser but injects a Freya-specific element type registry (`rect`, `label`, `paragraph` instead of `div`, `span`, `p`). This proves the macro tokenizer is extensible if Buiy ever wants to host a `rsx!`-derived DSL with Buiy-component-typed elements. Not part of foundation, but architecturally interesting.

8. **AccessKit + winit version pin coordination.** Freya pins `accesskit = 0.24.0` + `accesskit_winit = 0.32.0` together. These two crates must move together; AccessKit major bumps in either trigger coordinated upgrades. Buiy's `buiy-accessibility-design` sub-spec ([`README.md § 4`](../../specs/2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap)) should adopt the same coordinated-pin discipline.

9. **Workspace-as-internal-modularity.** Freya ships ~10 workspace crates (`freya-core`, `freya-engine`, `freya-elements`, `freya-components`, `freya-hooks`, `freya-winit`, `torin`, etc.) re-exported through a single `freya` meta-crate. Buiy's foundation [§ 2.8 Module organization](../../specs/2026-05-07-buiy-foundation/architecture.md#28-module-organization) lists the same shape. Validates the meta-crate-over-workspace pattern.

## How to use this file

When designing a Buiy feature:

1. **Find the row in `Avoid`** that names a pitfall close to your design. Read the linked file for the original incident.
2. **Find the entry in `Borrow`** that names a primitive close to what you're designing. Read the linked file for the shape, then adapt for Buiy's wgpu + Bevy ECS substrate.
3. **Promote any decision into a Buiy spec** under `docs/specs/` — this file is for capturing what we learn from Freya, not for encoding Buiy's own decisions.

## Sources

- All sibling files in this folder.
- Freya repo — https://github.com/marc2332/freya
- Freya site — https://freyaui.dev/
- Freya docs.rs — https://docs.rs/freya/latest/freya/
- Cross-references: [`../dioxus/lessons.md`](../dioxus/lessons.md), [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md), [`../accesskit/lessons.md`](../accesskit/lessons.md), [`../taffy/lessons.md`](../taffy/lessons.md), [`../slint/lessons.md`](../slint/lessons.md).
- Buiy foundation — [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md), [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md).
