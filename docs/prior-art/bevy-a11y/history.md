**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_a11y — release history from Bevy 0.10 (March 2023) through 0.19.0-rc.2 (May 2026), with the issue #17644 → PR #24308 episode in focus

# History

`bevy_a11y` is one of the longer-lived non-trivial subsystem crates in the Bevy workspace — three years and change of production integration, three AccessKit majors absorbed, and one canonical design incident (BSN-unfriendly megacomponent) that informs Buiy's most load-bearing a11y design choice ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md)).

This file traces the timeline and pins the incident.

## Phase 1 — Bevy 0.10 (2023-03-06): initial integration

PR [#6874](https://github.com/bevyengine/bevy/pull/6874) "Integrate AccessKit" by **Nolan Darilek (`ndarilek`)** opened 2022-12-06, merged by Bors 2023-03-01, shipped in Bevy 0.10 on 2023-03-06.

The PR introduced:

- The new `bevy_a11y` crate housing AccessKit integration code that did not need winit.
- The `AccessibilityNode(pub Node)` component (newtype over `accesskit::Node`).
- The `Focus` resource carrying the single focused `NodeId` (later subsumed into AccessKit's per-update `Tree.focus`).
- `AccessibilityRequested` and `ManageAccessibilityUpdates` resources for the activation gate and the "is the ECS responsible for tree updates?" flag.
- `ActionRequest` event channel wrapping `accesskit::ActionRequest`.
- The `AccessibilityPlugin` registering the above.
- A `Label` component "for marking text specifically as a label for UI controls" — distinct from the later `AccessibleLabel` (see Phase 4).
- The `accesskit_winit` adapter wiring into `bevy_winit`'s window-creation flow.

The release notes credit Nolan Darilek as the contributor and acknowledge **`mwcampbell`** (Matt Campbell, AccessKit author) for reviewing the integration and helping reduce the dependency footprint to improve compile times and binary size. The release marketed Bevy as "the first general-purpose game engine with first-class accessibility support."

The shape Darilek shipped — `AccessibilityNode(pub Node)` with `Deref` / `DerefMut` to `accesskit::Node` — is the same shape `bevy_a11y` 0.18.1 still has on 2026-05-22. Nothing about the megacomponent has structurally changed in three years; what changed is the surrounding ecosystem's expectations (BSN's reflection-driven authoring model), not the component.

## Phase 2 — Bevy 0.11 through 0.16 (2023-07 → 2024-11): iterative additions

The 0.10 → 0.16 stretch is iterative maintenance, not restructuring:

- **0.11 (2023-07)** — AccessKit dependency bump (PR [#8655](https://github.com/bevyengine/bevy/pull/8655), `ndarilek`).
- **0.12 (2023-11)** — Routine AccessKit + `accesskit_winit` bumps. The crate's public surface is unchanged.
- **0.13 (2024-02)** — Continued bumps; `Focus` resource semantics refined as AccessKit's `Tree.focus` model stabilized.
- **0.14 (2024-07)** — Bumps; the migration guide notes minor breaking changes from AccessKit's evolving `Role` and `Action` enums.
- **0.15 (2024-11)** — Notable: **`bevy_a11y` stopped re-exporting `accesskit`**. Migration guide entry: consumers must now add `accesskit` as a direct dependency if they need its types. This nudged downstream code toward depending on AccessKit directly rather than through Bevy — closer to Buiy's "talk to AccessKit directly" posture, but the megacomponent surface on `bevy_a11y`'s side remained.
- **0.16 (2025-04)** — Continued AccessKit version absorption; release notes pin `accesskit 0.19` then `0.20` line.

PR [#16234](https://github.com/bevyengine/bevy/pull/16234) (`ndarilek`, 2024-11) is a representative example of the cadence: a one-line AccessKit + `accesskit_winit` bump with no public-API impact, repeated approximately once per minor.

Throughout this phase the `AccessibilityNode` component definition is unchanged: a single newtype wrapper around `accesskit::Node` with `Deref` / `DerefMut`. Setters on the inner `Node` are AccessKit's own (`set_disabled()`, `clear_disabled()`, `set_role(Role)`, `set_label(&str)`, etc.) — none of which are component-derived public fields, none of which BSN's reflection-driven patch model can reach.

## Phase 3 — Bevy 0.17 (2025-09-30): bevy_feathers lands, bevy_a11y stays

Bevy 0.17 introduced **`bevy_feathers`** (tooling-focused widget set) and **`bevy_ui_widgets`** (headless widget primitives), both calling out accessibility as a first-class concern in the release notes. The feathers gallery example exercises the AccessKit integration on real widgets.

However, the release notes do **not** mention any rework of `bevy_a11y` itself. The mention is downstream:

> "Bevy Feathers includes accessibility features with built-in screen reader and assistive technology support."

The integration mechanic remained: each feathers widget inserts an `AccessibilityNode(Node)` whose internal `accesskit::Node` is mutated through AccessKit's own setters via component hooks. The pattern works for production code that knows it; it does not work for BSN-merged authoring, which is the gap issue #17644 names.

Bevy 0.17 also bumped AccessKit; v0.17.3 (2025-11-17) carries `accesskit 0.21`.

## Phase 4 — Issue #17644 (2025-02-02): the BSN-unfriendly megacomponent incident

[Issue #17644](https://github.com/bevyengine/bevy/issues/17644), opened 2025-02-02 by `viridia`, title: **"Design of bevy_a11y is BSN-unfriendly."**

> *Note: the brief preamble for this folder dated the issue "2026-02-02." Verified date is **2025-02-02** per the issue page — a year earlier. Corrected.*

The issue's central claim, paraphrased from the body:

- **BSN's design philosophy** is that components are ordinary properties that can be merged and patched. Multiple BSN templates contribute to a single entity by overlaying values, and the merge result is the final component state.
- `AccessibilityNode` violates this by wrapping `accesskit::Node` and exposing properties only through method calls. `aria-disabled` is set via `node.set_disabled()` (no argument) and cleared via `node.clear_disabled()` — not by setting a `disabled: bool` field that BSN could overlay.
- All accessibility properties live on a single ECS component. A BSN template that wants to set only the role cannot do so without read-modify-write logic that runs after merge.

viridia's framing quote (verified):

> "Because of this, I can well imagine wanting to merge together multiple BSN templates, each of which has opinions about various accessibility attributes."

The proposed solution in the issue body: "create our own idiomatic API, and then provide a transformation from that API into the AccessKit structure." This is the same posture Buiy commits to in [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md): decomposed Buiy components → `TreeUpdate` transformation in `BuiySet::A11yUpdate`. The Buiy spec is a stronger commitment than the issue's wording — Buiy decomposes by concern (`A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations`) rather than mirroring AccessKit's flat property bag, and it owns the AccessKit adapter outright rather than layering through `bevy_a11y`.

The issue was closed 2026-05-21 by PR #24308 (see next section). Whether the closure represents an adequate fix is treated separately in [`critiques.md`](critiques.md).

For the canonical telling of the component-model side of this incident, see the sibling [`component-model-incident.md`](component-model-incident.md).

## Phase 5 — PR #24308 (2026-05-15 opened, 2026-05-21 merged): the additive fix

[PR #24308](https://github.com/bevyengine/bevy/pull/24308) "Introduce `AccessibleLabel` component" by `viridia`, opened 2026-05-15, merged 2026-05-21, targets Bevy 0.19 (the next minor after 0.18.1).

> *Note: the brief preamble described PR #24308 as "the fix" that decomposed the megacomponent. Verified content: **PR #24308 is an additive change introducing a single new component, not a megacomponent decomposition.** Corrected here and in [`component-model-incident.md`](component-model-incident.md) and [`critiques.md`](critiques.md).*

What PR #24308 actually does (verified against the PR's file diff):

- Adds `AccessibleLabel(pub String)` as a new component in `bevy_ui::accessibility` (not in `bevy_a11y` itself).
- The component is `#[require(AccessibilityNode)]` — inserting it auto-inserts the megacomponent.
- Hooks `on_insert = on_label_inserted` and `on_remove = on_label_removed` synchronise the label string into the underlying `accesskit::Node`'s label via AccessKit's setter.
- Marked `#[component(immutable)]` so BSN-style overlaying of the label string is well-defined.
- Updates the feathers gallery example to use the new component.

The PR closes issue #17644 because the *one* property the issue most directly named (the label) is now BSN-overlayable. The PR's own description notes this is partial and references [issue #20524](https://github.com/bevyengine/bevy/issues/20524) as the broader follow-up. **The megacomponent itself — `AccessibilityNode(pub Node)` — is unchanged.** Every other ARIA property (role, description, states, relations, value, live region, …) is still set through AccessKit's method-style API on the inner `Node`.

The shape of this fix is part of what motivates Buiy's "replace `bevy_a11y` for Buiy windows" choice. Buiy decomposes upfront; `bevy_a11y` is decomposing piecemeal over what will likely be several Bevy minors, and during that interval the BSN-authoring experience for non-label properties is unchanged.

## Phase 6 — Bevy 0.18 (2026-01-13) → 0.18.1 (2026-03-04) → 0.19.0-rc.1 (2026-05-13) → 0.19.0-rc.2 (2026-05-22)

- **0.18.0** (2026-01-13): AccessKit bump to `accesskit 0.21`. No `bevy_a11y` public-API changes in the release notes.
- **0.18.1** (2026-03-04): Patch release; `bevy_a11y` follows the workspace version bump. AccessKit dependency unchanged.
- **0.19.0-rc.1** (2026-05-13): Pre-release on the `main` branch path. `bevy_a11y` Cargo.toml on `main` shows the AccessKit pin bumped to `accesskit 0.24` (AccessKit shipped 0.22 / 0.23 / 0.24 in a three-week burst at the start of 2026, then `accesskit_winit 0.33.0` + `accesskit_ios 0.1.0` on 2026-05-11).
- **0.19.0-rc.2** (2026-05-22): Same-day-as-this-folder pre-release. PR #24308 (the `AccessibleLabel` fix) targets the 0.19 milestone; the rc.2 is the first published artifact that contains it.

> *Note: the brief preamble listed "0.18.1 (2026-05-13)." Verified dates: 0.18.1 was 2026-03-04; 2026-05-13 is the `0.19.0-rc.1` date. Corrected.*

The `AccessibilityNode` newtype is unchanged from 0.10 through 0.19.0-rc.2. The change between Bevy 0.10 and Bevy 0.19 is the AccessKit version it wraps and the surrounding ecosystem (feathers widgets, BSN-friendly companion components like `AccessibleLabel`); it is not a structural change to the component-model surface that the issue #17644 incident named.

## Sources

- Bevy 0.10 release notes: https://bevy.org/news/bevy-0-10/
- PR [#6874](https://github.com/bevyengine/bevy/pull/6874) "Integrate AccessKit" (`ndarilek`, opened 2022-12-06, merged 2023-03-01).
- PR [#8655](https://github.com/bevyengine/bevy/pull/8655) "Bump `accesskit` and `accesskit_winit`" (Bevy 0.11 cycle).
- PR [#16234](https://github.com/bevyengine/bevy/pull/16234) "Bump accesskit and accesskit_winit" (Bevy 0.16 cycle).
- Bevy 0.15 migration guide entry on dropping the `accesskit` re-export: https://bevy.org/learn/migration-guides/0-14-to-0-15/.
- Bevy 0.17 release notes (feathers + a11y mention): https://bevy.org/news/bevy-0-17/.
- Issue [#17644](https://github.com/bevyengine/bevy/issues/17644) "Design of bevy_a11y is BSN-unfriendly" (`viridia`, opened 2025-02-02, closed 2026-05-21 by #24308).
- PR [#24308](https://github.com/bevyengine/bevy/pull/24308) "Introduce `AccessibleLabel` component" (`viridia`, opened 2026-05-15, merged 2026-05-21, target 0.19).
- crates.io `bevy_a11y` version history: https://crates.io/crates/bevy_a11y.
- `bevy_a11y` HEAD `src/lib.rs`: https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs.
- Sibling files: [`distribution.md`](distribution.md), [`component-model-incident.md`](component-model-incident.md), [`api.md`](api.md), [`critiques.md`](critiques.md), [`open-problems.md`](open-problems.md).
- AccessKit folder cross-reference: [`prior-art/accesskit/history.md`](../accesskit/history.md), [`prior-art/accesskit/lessons.md`](../accesskit/lessons.md).
- Buiy foundation cross-references: [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md), [`architecture.md § 2.6`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`cross-cutting.md § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md).
