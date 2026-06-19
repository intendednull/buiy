**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — Bevy's producer-side accessibility plugin surface; the `AccessibilityNode` megacomponent case study that anchors Buiy's "no megacomponents" rule

# bevy_a11y

`bevy_a11y` is the official accessibility crate in the Bevy engine workspace (`bevyengine/bevy`). Structurally it is a **single-file glue crate**: it installs an activation gate (`AccessibilityRequested`), a management flag (`ManageAccessibilityUpdates`), a one-variant system set (`AccessibilitySystems::Update`), the `ActionRequest` event newtype, and the `AccessibilityNode(pub accesskit::Node)` megacomponent — and almost no logic. The real work lives in two sibling crates: per-window adapter ownership, tree-build, and `ActionRequest` plumbing in [`bevy_winit/src/accessibility.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs), and the producer-side per-widget role/label/value logic in [`bevy_ui/src/accessibility.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/accessibility.rs). When this folder says "bevy_a11y" architecturally, it means the producer-side accessibility plugin surface owned across those three crates, of which `bevy_a11y` is the smallest member but the API anchor. The corpus exists because **Buiy replaces bevy_a11y for its own windows** — Buiy is its own AccessKit producer, talking to `accesskit_winit` directly per window, with a fully decomposed component vocabulary — and bevy_a11y is the upstream design that decision is measured against (see foundation [`architecture.md` § 2.4 / § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)).

**Honest assessment.** bevy_a11y has been a default sub-plugin in `DefaultPlugins` since Bevy 0.10 (March 2023, PR #6874) and provides real, durable AccessKit integration with a sound activation-gating pattern (skip tree-build until an assistive technology attaches) that Buiy borrows. But its central component, `AccessibilityNode(pub accesskit::Node)`, is the prototypical BSN-hostile megacomponent: ~200 properties reachable only through `set_<field>()` / `clear_<field>()` method calls, no per-property change-detection, no compositional patching — issue [#17644](https://github.com/bevyengine/bevy/issues/17644) (viridia, 2025-02-02) named it directly. The "fix," PR [#24308](https://github.com/bevyengine/bevy/pull/24308) (merged 2026-05-21, milestone 0.19), decomposed exactly **one** field — `AccessibleLabel` mirrors into `AccessibilityNode` via component hooks — and the PR author called it `"not a 100% fix, but ... good enough to close the ticket."` The megacomponent still exists in its original shape on main HEAD as of 2026-05-22. The upstream decomposition trajectory is "split one field per release as the pain is hit"; Buiy's foundation spec commits to full decomposition from day one, which is why the two component vocabularies share zero names and will not converge.

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `bevy_a11y` (workspace crate inside `bevyengine/bevy`) |
| Shape | Single-file crate (`src/lib.rs`); resources + one system set + one megacomponent + one event newtype, no systems of its own |
| License | MIT OR Apache-2.0 |
| Latest stable | 0.18.1 |
| Pre-release | 0.19.0-rc.2 (2026-05-22) |
| Workspace HEAD | 0.19.0-dev |
| Default-plugin posture | Sub-plugin of `DefaultPlugins` since Bevy 0.10 (disable via `disable::<AccessibilityPlugin>()`) |
| AccessKit integration landed | Bevy 0.10, 2023-03-01, PR #6874 (Nolan Darilek / `ndarilek`) |
| Central component | `AccessibilityNode(pub accesskit::Node)` — the megacomponent (~200 method-gated properties) |
| BSN-hostility issue | [#17644](https://github.com/bevyengine/bevy/issues/17644) "Design of bevy_a11y is BSN-unfriendly" (viridia, 2025-02-02) |
| Partial fix | [#24308](https://github.com/bevyengine/bevy/pull/24308) `AccessibleLabel` (merged 2026-05-21, milestone 0.19) — decomposes the label field only; `AccessibilityNode` unchanged |
| `accesskit` pin | 0.21 (v0.18.1) → 0.24 (HEAD); verify per release |
| Adapter ownership | `accesskit_winit::Adapter` stored thread-local in `bevy_winit`, keyed by window `Entity` (the adapter is `!Send`) |
| Multi-window | Real and structural — one adapter / activation handler / action queue per window |
| Foundation stewardship | Bevy Foundation (Washington 501(c)(3)); a11y SME contributors + AccessKit / Pneuma Solutions relationship |

## Strengths

- **Sound activation gate.** `AccessibilityRequested` (atomic bool) short-circuits tree-build until an AT actually attaches, with `adapter.update_if_active(|| tree_update)` skipping the build closure entirely otherwise. This is the cost-amortisation lever Buiy borrows (see [`docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md)).
- **Real per-window multi-adapter support.** Each window gets its own adapter, activation handler, and action queue, keyed by entity — the structural pattern Buiy adopts (keyed by platform `WindowId` instead, to survive entity respawn).
- **Shipped, default-on, durable.** AccessKit has been wired since 0.10; the integration is not experimental.

## Weaknesses

- **The `AccessibilityNode` megacomponent.** One component holds every a11y property; properties are method-gated (`set_*` / `clear_*`), not public fields; change-detection is coarse (any setter dirties the whole node); BSN cannot author or patch per-property. The plugin even calls `allow_ambiguous_component::<AccessibilityNode>()` to silence Bevy's own ambiguity detector because multiple systems mutate the one component per frame.
- **Decomposition is incremental and lazy.** PR #24308 split out a single field (`AccessibleLabel`) and locked it into the megacomponent's lifecycle via `#[require(AccessibilityNode)]`. Role, value, description, bounds, all state flags, and all relations still flow through the megacomponent.
- **Single-occupant adapter slot.** `accesskit_winit::Adapter` accepts exactly one tree per window with no merge protocol — two producers structurally cannot share a window, forcing per-window coexistence (the only design AccessKit's shape allows; see [`coexistence.md`](coexistence.md)).
- **Owns almost nothing for focus.** Focus tracking lives in `bevy_input_focus`, not `bevy_a11y` (see [`focus-model.md`](focus-model.md)).

## Lessons for Buiy

There is no `lessons.md` in this folder; the consult-this-when-designing material lives in **[`component-model-incident.md`](component-model-incident.md)** (the #17644 / #24308 case study in depth), with the cross-folder borrow/avoid tables in [`docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) and [`docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) (Avoid row "Megacomponents that are BSN-hostile"). All Buiy-side statements below are **sourced to the foundation spec as target state, not decided here**:

- **Validates** — the activation-gate + per-window-adapter pattern. Buiy keeps both shapes (foundation [`architecture.md` § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)); #17644 is the load-bearing validation of the "small, public-fielded, observable, decomposed component" rule (foundation [`architecture.md` § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)).
- **Avoid** — the megacomponent shape (private-fielded, method-gated, bundle-everything) and the layer-over-bevy_a11y integration path (three indirection hops, two component vocabularies, a per-frame translation tax that earns nothing over talking to `accesskit_winit` directly).
- **Borrow** — the activation gate, the closure-form `update_if_active` build skip, and per-window adapter ownership. Buiy's target component set (`A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`, per foundation [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)) shares zero names with bevy_a11y's; the divergence is structural, not a reaction to #17644 being incompletely fixed.

## Reading order

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, strengths/weaknesses, lessons pointer, reading order. |
| [`component-model-incident.md`](component-model-incident.md) | **The consult-this-when-designing case study.** Issue #17644, PR #24308, and why Buiy still replaces bevy_a11y after the partial fix. |
| [`architecture.md`](architecture.md) | Structural shape: the single-file crate, the plugin, the system set, the activation gate, the megacomponent, per-window adapter ownership, tree-update push. |
| [`api.md`](api.md) | Public API surface (resources, components, events, system set, plugin) as of v0.18.1 + main HEAD. |
| [`focus-model.md`](focus-model.md) | Focus in the Bevy stack — what `bevy_a11y` owns (almost nothing), what `bevy_input_focus` owns, what `bevy_feathers` styles, where Buiy diverges. |
| [`coexistence.md`](coexistence.md) | bevy_a11y / Buiy per-window coexistence — adapter-slot single-occupancy, the suppression rule, no shared-window coordinator. |
| [`distribution.md`](distribution.md) | Distribution shape: crate features, default-plugin posture, MSRV, platform support. |
| [`ecosystem.md`](ecosystem.md) | Who actually depends on it, the download-vs-deployment disconnect, adjacent Bevy crates, other game-engine a11y stacks. |
| [`governance.md`](governance.md) | Stewardship within the Bevy Foundation, SME contributors, AccessKit / Pneuma Solutions relationship, future-direction signals. |
| [`history.md`](history.md) | Release history from Bevy 0.10 (March 2023) through 0.19.0-rc.2, with the #17644 → #24308 episode in focus. |
| [`comparisons.md`](comparisons.md) | Side-by-side with Buiy's planned model, peer AccessKit producers (egui, Slint, Freya, Xilem/Masonry, Godot), and non-AccessKit game-engine stacks (Unity, Unreal). |
| [`critiques.md`](critiques.md) | Honest critiques: the megacomponent legacy, post-#24308 reality, coverage gaps, multi-window concerns, performance unknowns, Wayland/X11 divergence, the deployment gap. |
| [`open-problems.md`](open-problems.md) | Unresolved questions the integration leaves on the floor, organized by the area each touches in the Buiy foundation spec. |

## Sources

- `bevy_a11y` lib.rs (main HEAD, 0.19.0-dev) — https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs
- `bevy_winit/src/accessibility.rs` (main HEAD) — https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs
- `bevy_ui/src/accessibility.rs` (main HEAD) — https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/accessibility.rs
- PR #6874 (AccessKit integration, Bevy 0.10, 2023-03) — https://github.com/bevyengine/bevy/pull/6874
- Issue #17644 (BSN-unfriendly, viridia, 2025-02-02) — https://github.com/bevyengine/bevy/issues/17644
- PR #24308 (Introduce AccessibleLabel, merged 2026-05-21, milestone 0.19) — https://github.com/bevyengine/bevy/pull/24308
- AccessKit project — https://accesskit.dev
- Buiy foundation — architecture §2.4, §2.6: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation — accessibility: [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
