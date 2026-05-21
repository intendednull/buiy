# Buiy layout — Phase 5: container queries

**Date:** 2026-05-21
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md`](../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md) (§ 1) and [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) (§ 3.2)
**Supersedes:** none

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Fresh subagent per task + two-stage review (spec then quality).

**Goal:** Land CSS `@container` size queries plus the `cqw/cqh/cqi/cqb/cqmin/cqmax` container-unit family in `buiy_core::layout`, with same-frame re-layout capped at 2× Taffy per architecture.md § 3.2.

**Architecture:** Three new pipeline systems attach to the previously-stubbed sets `CqActivate`, `CqFlipCheck`, `CqFlipReRun`. A memoized nearest-queried-ancestor walk (modeled on Phase 4 `inherit_writing_mode` at `crates/buiy_core/src/layout/systems.rs:308-362`) reads the *previous frame's* `ResolvedLayout` from each container ancestor; `cq_activate` toggles `ContainerQueryActive` / `ContainerQueryInactive` markers; `cq_flip_check` re-evaluates against this frame's Taffy output (`tree.layout(node_id)` per architecture.md § 3.2 explicit pinning); `cq_flip_rerun` re-runs the inner sync-styles + taffy-compute work at most once when a flip is signaled. Container units resolve during `style_to_taffy` via a frame-built `ContainerSizeLookup` helper.

**Tech Stack:** Rust, `bevy 0.18`, `taffy 0.10`, `bevy_reflect`. No new deps.

**Prior art — concrete files Phase 5 must mirror:**

- **Memoized ancestor walk:** `crates/buiy_core/src/layout/systems.rs:308-362` (`inherit_writing_mode` + `resolve_writing_mode`). Phase 5 builds two analogous helpers — one for "nearest container ancestor" (used by `cq_activate` and container-unit resolution) and the activation pass itself follows the same `Query<(Entity, Option<&Marker>), With<Node>>` + `HashMap<Entity, ResolvedLayout>` memo + idempotent-insert shape.
- **Idempotent insert guarding O(0):** `systems.rs:319-321` — `if current.copied() != Some(new_resolved) { commands.entity(entity).insert(new_resolved); }`. Phase 5 marker toggles must use the same compare-before-insert idiom, otherwise `Changed<ContainerQueryActive>` would re-fire `sync_styles` every frame.
- **Or-filter widening preserving Phase 2 invariant:** `systems.rs:105-122`. Phase 5 widens the trigger set with `Changed<Container>`, `Changed<ContainerQuery>`, `Changed<ContainerQueryActive>`, `Changed<ContainerQueryInactive>`. `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` remain excluded — asserted by `crates/buiy_core/tests/layout_scroll_offset_no_invalidate.rs`. The new test in Task 10 must keep that file green.
- **Atomic types.rs + translate.rs commit:** Phase 3's Task 1 (Length::Fr) established this — adding a `Length` variant without same-commit `translate.rs` match arms causes `E0004`. Phase 5 Task 1 must land both files in one commit.
- **`String` over `SmolStr`:** Phase 3 chose `String` for `GridLine::Area(_)` (`types.rs:375-389`) to avoid a new direct dep. Phase 5 uses `Option<String>` for `Container::container_name` and `Option<String>` for `ContainerQuery::container` for consistency.
- **`taffy::prelude` selective import:** `translate.rs:30-33`. Phase 5 doesn't need new Taffy helpers; container units resolve to `taffy::LengthPercentage` / `Dimension` via the existing `length(_)` / `percent(_)` helpers.
- **Pipeline-step stubs are reserved, do not delete:** `pipeline.rs:28-38` already declares `CqActivate`, `CqFlipCheck`, `CqFlipReRun` as Phase 5 placeholders. Phase 5 attaches systems to those sets; it does NOT add new variants to `BuiyLayoutStep`.
- **Style builder shape:** `crates/buiy_core/src/layout/style.rs:44-54` and fluent-setter idiom at `style.rs:378-398`. Phase 5 follows the `mut self -> Self` pattern.
- **Plugin registration list:** `mod.rs:37-66` (`register_type::<T>()` chain) and re-export lists at `mod.rs:13-26` + `mod.rs:20-26`. Phase 5 extends both.
- **Decision: viewport-unit fallback.** Spec § 1.4 says container units fall back to "viewport units (`cqw → vw`, …)" when no queried ancestor exists. Phase 10 owns viewport units (`types.rs:6-9` defers them). Phase 5 implements the *semantics* of the fallback — resolve directly from `bevy::window::Window.resolution` to absolute pixels — without introducing `Length::Vw/Vh` variants. The plan notes this explicitly so future Phase 10 work can replace the inline `Window` read with `Length::Vw` rewriting without behavior change.
- **Decision: rule body shape.** Spec § 1.2 defines `ContainerQuery { container, conditions, when_active: Option<Entity>, when_inactive: Option<Entity> }`. The Entity-bundle-application machinery hinted at by `when_active`/`when_inactive` is consumer-responsibility per spec § 1.2 last paragraph ("Style-bundle application is the consumer's responsibility"). Phase 5 ships `ContainerQuery { container, conditions }` — the `Option<Entity>` fields are omitted because there is no in-tree consumer for them in v1 and storing dead state is worse than under-shipping. The `ContainerQueryActive` / `ContainerQueryInactive` markers are the activation surface; authors observe them and apply whatever they like. Adding the Entity fields later is a non-breaking *additive* schema change at the Rust level; BSN files written against v1's field-less shape continue to deserialize because reflection treats missing fields as defaults. **CHANGELOG must explicitly call this divergence out.**
- **Decision: one ContainerQuery per entity (v1).** Spec does not specify cardinality. v1 stores at most one `ContainerQuery` per entity (Bevy's `Component` is single-instance). Multi-query-per-entity is a follow-up if needed.
- **Decision: warn-once granularity.** Spec § 1.4 asks for "per-entity per-session" warn dedup. Phase 5 uses session-global `AtomicBool` (one warn per session per condition, regardless of entity count). Rationale: a per-entity `HashSet` resource would grow unboundedly across despawns and the spec's intent (avoid log flood) is better served by global once-only. **CHANGELOG documents this divergence.**
- **Decision: `Style.container` is NOT `Option<Container>`.** `Style` is `#[derive(Bundle)]` (`style.rs:44-54`). Bevy 0.18 does not implement `Bundle` for `Option<T>`; the field shape used by every other component on `Style` is unconditional insert (e.g., `pub writing_mode: WritingMode`, `pub box_model: BoxModel`). Phase 5 follows suit: `pub container: Container`, with `ContainerType::Normal` as the inert default. Authors who don't opt in get a `Container { container_type: Normal, container_name: None }` component on the entity — invisible to the activation system (which skips `Normal` per `resolve_nearest_container`) and free in steady-state (`Changed<Container>` doesn't fire after spawn). The "skip-on-default" path in the existing Phase 1 doc comment (`style.rs:8-10`) tracks the broader cleanup; Phase 5 does not unilaterally introduce it.
- **Decision: re-run mechanism in `cq_flip_rerun`.** Implementer ambiguity in v1 of this plan is resolved here: register `cq_flip_rerun` as a **normal Bevy system** (NOT `&mut World` / `SystemState`) in `BuiyLayoutStep::CqFlipReRun` with the union of params from `sync_styles` + `taffy_compute`. The system body is gated on `CqReRunRequested.0`; when set, it re-executes the same per-entity translation loop and the same Taffy compute call, then clears the flag. The shared per-entity translation work is factored into `pub(super) fn translate_one(...)` so `sync_styles` and `cq_flip_rerun` reuse it instead of duplicating bodies. The `SystemState`-on-`&mut World` approach previously sketched is **rejected** because the existing `sync_styles` signature already takes `NonSendMut<LayoutTree>` and cannot be left as a "trivial wrapper" delegating to an `&mut World` inner without changing its declared parameter set, which would in turn break the existing system-attach in `mod.rs`.

---

## File structure

Phase 5 touches the following files. Each task names the exact files + line-anchor it edits.

| File | Phase 5 change |
|---|---|
| `crates/buiy_core/src/layout/types.rs` | Add 6 `Length` container-unit variants. Add `ContainerType`, `Orientation`, `QueryCondition` enums. |
| `crates/buiy_core/src/layout/components.rs` | Add `Container`, `ContainerQuery`, `ContainerQueryActive`, `ContainerQueryInactive` components. |
| `crates/buiy_core/src/layout/style.rs` | Add `container: Container` field (unconditional; `Container::default()` = inert `Normal/None`) + fluent setters. No Bundle-expansion logic — `Style` is `#[derive(Bundle)]`. |
| `crates/buiy_core/src/layout/systems.rs` | Add `cq_activate`, `cq_flip_check`, `cq_flip_rerun` systems + `resolve_nearest_container` helper. Widen `sync_styles` Or-filter. |
| `crates/buiy_core/src/layout/translate.rs` | Resolve `Length::Cq*` against the threaded `ContainerSizeLookup`. Add warn-once gate for missing-ancestor fallback. Extend `StyleView`. |
| `crates/buiy_core/src/layout/mod.rs` | Register new types + re-exports + system attach. |
| `crates/buiy_core/tests/layout_container_queries.rs` | New integration test file. |
| `crates/buiy_core/tests/layout_scroll_offset_no_invalidate.rs` | (Unchanged; Task 9 re-runs to confirm Phase 2 invariant intact.) |
| `crates/buiy_core/tests/layout_pipeline_order.rs` | Wire the cq_activate / cq_flip_check / cq_flip_rerun trackers (replace empty-set checkpoints with system trackers). |

No new files outside the test directory beyond the one new integration-test file.

---

## Task 1: Add `Length` container-unit variants (atomic with translate.rs)

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs:17-44` (extend `Length` enum + impls)
- Modify: `crates/buiy_core/src/layout/translate.rs` (add match arms — without this, `E0004` non-exhaustive match)

**Prior-art context:** Phase 3 Task 1 (Length::Fr) demonstrated that adding a `Length` variant without same-commit `translate.rs` match arms is a build break. The Phase 3 commit landed both files in one atomic commit; Phase 5 Task 1 mirrors that. The translate-side handling here is *temporary* — Task 7 replaces the 0-px fallback with real ancestor-driven resolution. Task 7 cannot land without Task 1's variants existing; this temporary fallback is the bridge.

- [ ] **Step 1: Write the failing test (`types.rs`)**

Add to the `#[cfg(test)] mod tests` block in `crates/buiy_core/src/layout/types.rs`:

```rust
    #[test]
    fn length_container_unit_variants_round_trip() {
        let cases = [
            Length::Cqw(50.0),
            Length::Cqh(25.0),
            Length::Cqi(50.0),
            Length::Cqb(25.0),
            Length::Cqmin(10.0),
            Length::Cqmax(90.0),
        ];
        for case in cases {
            let copied = case;
            assert_eq!(case, copied);
        }
        // Round-trip via Default doesn't apply (default is ZERO = Px(0.0));
        // the round-trip we care about is just discriminant equality, which
        // PartialEq covers via the derive.
    }
```

- [ ] **Step 2: Run and confirm it fails (non-exhaustive — variants don't exist yet)**

```sh
cargo test -p buiy_core --lib layout::types::tests::length_container_unit_variants_round_trip
```

Expected: `error[E0599]: no variant or associated item named 'Cqw' found for enum 'Length'`.

- [ ] **Step 3: Add the variants to `Length`**

Replace the body of the `Length` enum in `crates/buiy_core/src/layout/types.rs:17-26` with:

```rust
/// CSS-style length value.
///
/// Phase 1 shipped `Px`, `Percent`. Phase 3 added `Fr` (grid-only).
/// Phase 5 adds the container-query unit family (`Cqw`/`Cqh`/`Cqi`/`Cqb`/
/// `Cqmin`/`Cqmax`). Em / Rem / viewport / Calc resolution remains
/// deferred to Phase 10 (`buiy-layout-units-calc`).
///
/// Container units resolve in `style_to_taffy` against the entity's
/// nearest *queried* ancestor's previous-frame `ResolvedLayout` (an
/// ancestor whose `Container.container_type != Normal`). When no
/// queried ancestor exists, container units fall back to viewport
/// dimensions (resolved directly from `bevy::window::Window` until
/// Phase 10's `Length::Vw/Vh` infrastructure lands) with one `warn!`
/// per (entity, unit) pair per session. Spec:
/// docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.4.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Absolute logical pixels.
    Px(f32),
    /// Percentage of the containing block dimension on the relevant axis.
    Percent(f32),
    /// CSS `<flex>` unit — only meaningful inside `TrackSize::Length(Length::Fr(_))`.
    /// Outside grid templates, `Fr` warns once and resolves to `Auto`.
    Fr(f32),
    /// `cqw` — percentage of nearest queried ancestor's *width*.
    Cqw(f32),
    /// `cqh` — percentage of nearest queried ancestor's *height*.
    Cqh(f32),
    /// `cqi` — percentage of nearest queried ancestor's *inline* axis
    /// (depends on writing-mode).
    Cqi(f32),
    /// `cqb` — percentage of nearest queried ancestor's *block* axis.
    Cqb(f32),
    /// `cqmin` — percentage of `min(cqi, cqb)`.
    Cqmin(f32),
    /// `cqmax` — percentage of `max(cqi, cqb)`.
    Cqmax(f32),
}
```

- [ ] **Step 4: Add match-arm placeholders in `translate.rs`**

In `crates/buiy_core/src/layout/translate.rs`, find every `match` over `Length` (search for `Length::Px(` to locate them). For each, add the new variants. Two helper match sites exist in Phase 4: `length_to_lp` (the `LengthPercentage` flavor) and `length_to_lpa` (the `LengthPercentageAuto` flavor). Add a third helper to centralize the fallback path.

Add this helper near the existing length helpers in `translate.rs` (placement near `warn_once_fr_outside_grid` at `translate.rs:35-45`):

```rust
/// Phase 5 Task 1 *temporary* fallback: container units resolve to
/// 0 px until Task 7 wires the real ancestor-driven path. The only
/// purpose of this helper is to keep the `Length` match exhaustive
/// in this commit (Phase 3 Task 1 prior art — atomic types.rs +
/// translate.rs).
///
/// **Task 7 deletes this function.** No warn is emitted here because
/// the warn-once gating lives in Task 7's real resolver
/// (`warn_once_cq_no_ancestor`). A temporary 0-px fallback is a
/// build-bridge, not behavior worth advertising.
fn cq_unit_fallback_px(_p: f32) -> f32 {
    0.0
}
```

(No `use std::sync::atomic::{AtomicBool, Ordering};` is added in this task — Task 7 owns those imports. The Task 1 commit imports nothing new.)

Then in every `match` on `Length` in `translate.rs`, add arms. Find each match — Phase 3 added them at the `length_to_lp`, `length_to_lpa`, and `sizing_to_dim` helpers (search for `match len {` or `match v {`). For each, the existing arms are roughly (verify by reading the file):

```rust
// Existing in length_to_lp:
match v {
    Length::Px(p) => length(p),
    Length::Percent(p) => percent(p / 100.0),
    Length::Fr(_) => { warn_once_fr_outside_grid(); length(0.0) }
}
```

Add at the bottom of each such match:

```rust
Length::Cqw(p) | Length::Cqh(p) | Length::Cqi(p) | Length::Cqb(p)
| Length::Cqmin(p) | Length::Cqmax(p) => length(cq_unit_fallback_px(p)),
```

For `length_to_lpa` (`LengthPercentageAuto`), use `length(...).into()` if that's the existing `Px` arm's shape; for `sizing_to_dim` (`Dimension`), use `taffy::Dimension::length(...)` or whichever helper matches.

**Read the file first** to confirm each helper's exact return shape — the plan refuses to hard-code an unverified call form here. The implementer must inspect `translate.rs` around the existing `Length::Px` arms and match the call shape exactly. (Five minutes of reading; faster than guessing.)

- [ ] **Step 5: Run the test, confirm it now passes**

```sh
cargo test -p buiy_core --lib layout::types::tests::length_container_unit_variants_round_trip
```

Expected: `test result: ok. 1 passed`.

- [ ] **Step 6: Run full crate tests + lints to confirm no break**

```sh
cargo test -p buiy_core
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

All three must pass.

- [ ] **Step 7: Commit**

```sh
git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/translate.rs
git commit -m "feat(buiy_core,layout): add Length::Cq{w,h,i,b,min,max} variants

Container-unit family — placeholder fallback to 0 px in translate.rs;
real ancestor-driven resolution lands in Task 7. Atomic with
translate.rs to keep the match exhaustive (Phase 3 Task 1 prior art).

Spec: container-queries-and-writing-modes.md § 1.4."
```

---

## Task 2: Add `ContainerType` + `Orientation` enums and `Container` component

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add two enums near `JustifyItems` / `WritingModeKind` to follow alphabetical-by-feature ordering used in the file)
- Modify: `crates/buiy_core/src/layout/components.rs` (add `Container` component)

**Prior-art context:** Component shape mirrors `WritingMode` (components.rs:252) — `Component, Reflect, Default, Clone, Debug, PartialEq, Eq` (no Copy because `container_name: Option<String>` isn't Copy). The `#[reflect(Component, Default)]` attribute is required for BSN.

- [ ] **Step 1: Write the failing test (`types.rs` defaults)**

Add to `#[cfg(test)] mod tests` in `types.rs`:

```rust
    #[test]
    fn container_type_default_is_normal() {
        assert_eq!(ContainerType::default(), ContainerType::Normal);
    }

    #[test]
    fn orientation_default_is_portrait() {
        // Width <= height → portrait. CSS default ambiguous; we pick
        // Portrait so the default `Orientation(Portrait)` condition is
        // a useful sentinel.
        assert_eq!(Orientation::default(), Orientation::Portrait);
    }
```

- [ ] **Step 2: Run and confirm both fail**

```sh
cargo test -p buiy_core --lib layout::types::tests::container_type_default_is_normal
```

Expected: `error[E0412]: cannot find type 'ContainerType'`.

- [ ] **Step 3: Add `ContainerType` + `Orientation` enums**

Add to `crates/buiy_core/src/layout/types.rs` after the `WritingModeKind` block (line ~545):

```rust
/// CSS `container-type`. Determines whether an entity is a query
/// container (i.e., whether descendant `@container` rules and container
/// units resolve against it).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.1.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerType {
    /// Not a query container. The default.
    #[default]
    Normal,
    /// Both axes queryable; `cqw/cqh/cqi/cqb` all resolve.
    Size,
    /// Only inline axis queryable; `cqb` against this container falls
    /// back to viewport-block with warn-once.
    InlineSize,
}

/// CSS `@container (orientation: ...)` condition value.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.2.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Container's inline axis is shorter than its block axis.
    #[default]
    Portrait,
    /// Container's inline axis is longer than its block axis.
    Landscape,
}
```

- [ ] **Step 4: Add the `Container` component**

In `crates/buiy_core/src/layout/components.rs`, after the `WritingModeResolved` block (line ~290), add:

```rust
/// Marks an entity as a CSS container (or not). Descendants resolve
/// `@container` rules and container units (`cqw`, `cqi`, ...) against
/// the nearest ancestor whose `container_type` is `Size` or `InlineSize`.
///
/// `container_name` is an optional opaque label (CSS `container-name`).
/// When set, descendant `ContainerQuery` rules with `container:
/// Some(name)` match this container by name; rules with `container: None`
/// match the nearest queried ancestor regardless of name. String is used
/// for the same reason as `GridLine::Area` (Phase 3): avoids a new
/// direct `SmolStr` dep, and container names are set at spawn time, not
/// on a hot path.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.1.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct Container {
    pub container_type: ContainerType,
    pub container_name: Option<String>,
}
```

Update the existing `use super::types::{...}` line near the top of `components.rs` to include `ContainerType` (and the `Orientation`-using types added in Task 3 will be added then, not here).

- [ ] **Step 5: Run the tests, confirm pass**

```sh
cargo test -p buiy_core --lib layout::types::tests::container_type_default_is_normal layout::types::tests::orientation_default_is_portrait
cargo test -p buiy_core --lib  # also runs the components.rs default test once written
```

- [ ] **Step 6: Write a `Container` default test**

Add to `#[cfg(test)] mod tests` in `components.rs` (find an existing component-default test like the `WritingMode` ones at the bottom of the file):

```rust
    #[test]
    fn container_default_is_normal_unnamed() {
        let c = Container::default();
        assert_eq!(c.container_type, ContainerType::Normal);
        assert_eq!(c.container_name, None);
    }
```

Add `ContainerType` to the test-module's `use super::*` or explicit import if needed.

- [ ] **Step 7: Run and confirm**

```sh
cargo test -p buiy_core --lib layout::components::tests::container_default_is_normal_unnamed
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 8: Commit**

```sh
git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/components.rs
git commit -m "feat(buiy_core,layout): add ContainerType, Orientation, Container

Author-facing surface: mark an entity as a query container via
\`Container { container_type: ContainerType::Size, .. }\`. Descendants
will resolve \`@container\` rules + container units against the
nearest such ancestor (Task 6, 7).

Spec: container-queries-and-writing-modes.md § 1.1."
```

---

## Task 3: Add `QueryCondition` enum + `ContainerQuery` component

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add `QueryCondition` enum)
- Modify: `crates/buiy_core/src/layout/components.rs` (add `ContainerQuery` component)

**Prior-art context:** `QueryCondition` mirrors `TrackSize`'s shape (multi-variant Reflect enum with embedded values). Because `Length` is `Copy` and `f32` is `Copy`, `QueryCondition` can be `Copy` — keep it Copy for cheap clones in the activation loop. `ContainerQuery` is *not* Copy (`Vec` field).

**Decision recap:** v1 omits the spec's `when_active`/`when_inactive: Option<Entity>` fields (see Prior art § "rule body shape" above). The marker components in Task 4 are the activation surface.

- [ ] **Step 1: Write the failing test**

In `types.rs` test module:

```rust
    #[test]
    fn query_condition_variants_construct() {
        let c1 = QueryCondition::MinWidth(Length::Px(600.0));
        let c2 = QueryCondition::MaxAspectRatio(1.5);
        let c3 = QueryCondition::Orientation(Orientation::Landscape);
        // PartialEq derive covers structural equality.
        assert_ne!(c1, c2);
        assert_ne!(c2, c3);
        // Copy bound — implicit copy through assignment.
        let c4 = c1;
        assert_eq!(c4, c1);
    }
```

- [ ] **Step 2: Run, confirm fail**

```sh
cargo test -p buiy_core --lib layout::types::tests::query_condition_variants_construct
```

Expected: `error[E0412]: cannot find type 'QueryCondition'`.

- [ ] **Step 3: Add `QueryCondition`**

After the `Orientation` block from Task 2 in `types.rs`:

```rust
/// One `@container` condition — a single predicate on the resolved size
/// of the query container. A `ContainerQuery` AND-combines multiple of
/// these.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.2.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum QueryCondition {
    /// Activates when container `width >= value`.
    MinWidth(Length),
    /// Activates when container `width <= value`.
    MaxWidth(Length),
    /// Activates when container `height >= value`.
    MinHeight(Length),
    /// Activates when container `height <= value`.
    MaxHeight(Length),
    /// Activates when container `width/height >= ratio`.
    MinAspectRatio(f32),
    /// Activates when container `width/height <= ratio`.
    MaxAspectRatio(f32),
    /// Activates when container orientation matches.
    Orientation(Orientation),
}
```

- [ ] **Step 4: Add `ContainerQuery` component**

In `components.rs` after the `Container` block:

```rust
/// A `@container` rule pinned to a single entity. The rule activates
/// when *all* `conditions` hold against the resolved size of the
/// matched query container (by name, or nearest queried ancestor when
/// `container` is `None`).
///
/// When the rule's activation state flips, `cq_activate` toggles
/// `ContainerQueryActive` ↔ `ContainerQueryInactive` on this same
/// entity. Authors observe those markers and react however they want —
/// the spec calls out (§ 1.2 last paragraph) that style-bundle
/// application is consumer-responsibility.
///
/// v1 stores at most one `ContainerQuery` per entity (Bevy's
/// `Component` is single-instance).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.2.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ContainerQuery {
    /// `None` = nearest queried ancestor. `Some(name)` = nearest
    /// ancestor with `Container { container_name: Some(name), .. }`.
    pub container: Option<String>,
    /// All conditions must hold for the rule to be active. Empty list
    /// = always active (matches CSS `@container (width)` which is
    /// always true if there's a container at all — Phase 5 simplifies
    /// to "always active").
    pub conditions: Vec<QueryCondition>,
}
```

Add `QueryCondition` to the `use super::types::{...}` import line in `components.rs`.

- [ ] **Step 5: Add a default test for `ContainerQuery`**

In `components.rs` test module:

```rust
    #[test]
    fn container_query_default_is_anonymous_and_empty() {
        let q = ContainerQuery::default();
        assert_eq!(q.container, None);
        assert!(q.conditions.is_empty());
    }
```

- [ ] **Step 6: Run all tests + lints**

```sh
cargo test -p buiy_core --lib
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 7: Commit**

```sh
git add crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/components.rs
git commit -m "feat(buiy_core,layout): add QueryCondition, ContainerQuery

The rule body. Authors compose conditions (MinWidth, MaxWidth, ...
Orientation). Task 4 adds the activation markers; Task 6 evaluates
rules against container ancestors' resolved sizes.

Spec: container-queries-and-writing-modes.md § 1.2."
```

---

## Task 4: Add `ContainerQueryActive` + `ContainerQueryInactive` marker components

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs`

**Prior-art context:** Markers in Bevy are unit-struct components. Phase 5 uses two distinct components (not an enum-component) because:

1. `Or<(Changed<ContainerQueryActive>, Changed<ContainerQueryInactive>)>` is easier to express in `sync_styles`'s filter as two `Changed<T>` than to introspect a single enum component.
2. Bevy's reflection on enum components is fine, but the *toggling cost* (insert one, remove the other) is the same as flipping a value in an enum component — and the two-component shape lets authors `With<ContainerQueryActive>` directly in their own systems without a `Query<&ContainerQueryState>` indirection.

- [ ] **Step 1: Write the failing test**

In `components.rs` test module:

```rust
    #[test]
    fn container_query_active_inactive_are_distinct_markers() {
        // Just confirms both exist and can be constructed.
        let _a = ContainerQueryActive;
        let _i = ContainerQueryInactive;
    }
```

- [ ] **Step 2: Run, confirm fail**

```sh
cargo test -p buiy_core --lib layout::components::tests::container_query_active_inactive_are_distinct_markers
```

Expected: `error[E0425]: cannot find value 'ContainerQueryActive' in this scope`.

- [ ] **Step 3: Add the marker components**

In `components.rs` after `ContainerQuery`:

```rust
/// Marker — set by `cq_activate` when the entity's `ContainerQuery`
/// matched its container's resolved size on the current activation
/// pass. Mutually exclusive with `ContainerQueryInactive`.
///
/// Authors observe `With<ContainerQueryActive>` to apply whatever
/// behavior they want on activation. Spec § 1.2: style-bundle
/// application is consumer-responsibility.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct ContainerQueryActive;

/// Marker — set by `cq_activate` when the entity's `ContainerQuery`
/// did *not* match. Mutually exclusive with `ContainerQueryActive`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct ContainerQueryInactive;
```

- [ ] **Step 4: Run tests + lints**

```sh
cargo test -p buiy_core --lib layout::components::tests::container_query_active_inactive_are_distinct_markers
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 5: Commit**

```sh
git add crates/buiy_core/src/layout/components.rs
git commit -m "feat(buiy_core,layout): add ContainerQueryActive/Inactive markers

Activation surface for \`@container\` rules. Two distinct unit
components (not an enum) so authors can With<ContainerQueryActive>
directly in their own systems.

Spec: container-queries-and-writing-modes.md § 1.2."
```

---

## Task 5: Style builder — add `container` field + fluent setters

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs:44-54` (struct fields + Bundle expansion)
- Modify: `crates/buiy_core/src/layout/style.rs` (add fluent setters)

**Prior-art context:** `Style` is `#[derive(Bundle)]` at `style.rs:44-54` with all non-Option fields. Bevy 0.18 does NOT impl `Bundle` for `Option<T>` — adding `container: Option<Container>` would fail to compile. Every existing Style field (`writing_mode`, `box_model`, `position`, etc.) is unconditionally inserted; the Phase 1 doc-comment at `style.rs:7-10` calls this out as "skip-on-default is deferred."

Phase 5 therefore uses `pub container: Container` — unconditional, with `ContainerType::Normal` as the inert default. Downstream behavior: `cq_activate`'s `resolve_nearest_container` (Task 6) skips entities whose `Container::container_type == Normal`, and `Changed<Container>` for a never-mutated default value fires only once (at spawn), then stays steady-state-quiet. The "default sentinel" pattern matches how `WritingMode::default()` is treated as unset by Phase 4's `inherit_writing_mode` (systems.rs:343-362).

`ContainerQuery` is **not** added to `Style` — it's the *child side* (the queried entity is not the container that the rule's queries refer to, but it carries the rule that queries an ancestor). Per architecture.md § 2.4, child-side properties are decomposed-only. Authors spawn `ContainerQuery` alongside `Style`.

`Container`, however, *is* author-facing self-styling (the entity itself is the query container), so it joins `Style`.

- [ ] **Step 1: Read the current `Style` struct shape**

Before editing, run:

```sh
grep -n "pub struct Style" crates/buiy_core/src/layout/style.rs
grep -n "default_style_inserts_every_decomposed_component\|inserts_every_decomposed" crates/buiy_core/src/layout/style.rs
```

Confirm the struct is `#[derive(Bundle)]` (line 44) and that there's a test asserting every field gets inserted on default spawn (around line 674). Task 5 must update that test's assertion to include `Container`.

- [ ] **Step 2: Write the failing test**

In `style.rs`'s `#[cfg(test)] mod tests` block (look for an existing fluent-setter test like one for `.writing_mode(_)`):

```rust
    #[test]
    fn style_container_sets_type_and_name() {
        let s = Style::default().container_size().container_name("card");
        assert_eq!(s.container.container_type, ContainerType::Size);
        assert_eq!(s.container.container_name.as_deref(), Some("card"));
    }

    #[test]
    fn style_container_inline_size_no_name() {
        let s = Style::default().container_inline_size();
        assert_eq!(s.container.container_type, ContainerType::InlineSize);
        assert!(s.container.container_name.is_none());
    }

    #[test]
    fn style_default_container_is_normal_unnamed() {
        // Inert default — every Style-spawned entity carries a
        // Container { Normal, None }. cq_activate's ancestor walk
        // skips Normal containers, so this has no semantic effect;
        // it's required because #[derive(Bundle)] needs every field
        // be a real component (Bevy 0.18: Option<T> is not Bundle).
        let s = Style::default();
        assert_eq!(s.container.container_type, ContainerType::Normal);
        assert!(s.container.container_name.is_none());
    }
```

- [ ] **Step 3: Run, confirm fail**

```sh
cargo test -p buiy_core --lib layout::style::tests::style_container_sets_type_and_name
```

Expected: `error[E0609]: no field 'container' on type 'Style'`.

- [ ] **Step 4: Add the field + extend imports**

Extend the import line at `style.rs:15-17`:

```rust
use super::components::{
    BoxModel, Container, Display, FlexParams, GridParams, Overflow, Position, Scroll, WritingMode,
};
```

Extend the import line at `style.rs:18-23` to include `ContainerType`:

```rust
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, ContainerType, Direction, Edges, FlexAxis,
    FlexGap, FlexWrap, GridAreas, GridAutoFlow, Inset, JustifyContent, JustifyItems, Length,
    LogicalEdges, OverflowMode, PositionKind, ScrollBehavior, ScrollbarGutter, ScrollbarWidth,
    Sizing, SnapType, TextOrientation, TrackSize, UnicodeBidi, WritingModeKind,
};
```

In the struct at `style.rs:44-54`, append `container: Container` after `writing_mode`:

```rust
#[derive(Bundle, Clone, Debug, Default)]
pub struct Style {
    pub display: Display,
    pub box_model: BoxModel,
    pub position: Position,
    pub flex_params: FlexParams,
    pub overflow: Overflow,
    pub scroll: Scroll,
    pub grid_params: GridParams,
    pub writing_mode: WritingMode,
    pub container: Container,
}
```

`Container::default()` evaluates to `Container { container_type: Normal, container_name: None }` — the inert sentinel. No hand-written `Default` impl needed because `#[derive(Default)]` is already on the struct.

- [ ] **Step 5: Add the fluent setters**

Find the fluent-setter block near `.writing_mode(_)` (style.rs ~378-398) and add immediately after:

```rust
    // ---- Container ----

    /// Set the entity as a CSS query container (`container-type: size`).
    /// Descendant `@container` rules and container units resolve against
    /// this entity's resolved size. Preserves any name previously set.
    pub fn container_size(mut self) -> Self {
        self.container.container_type = ContainerType::Size;
        self
    }

    /// Set the entity as a CSS *inline-size* query container
    /// (`container-type: inline-size`). Only the inline axis is
    /// queryable; block-axis queries against this container fall back
    /// to viewport-block with warn-once.
    pub fn container_inline_size(mut self) -> Self {
        self.container.container_type = ContainerType::InlineSize;
        self
    }

    /// Set the query container's name (CSS `container-name`). Has no
    /// effect unless `container_size()` or `container_inline_size()`
    /// is also called (or the entity is otherwise opted in by direct
    /// `Container` insertion with a non-Normal type).
    pub fn container_name(mut self, name: impl Into<String>) -> Self {
        self.container.container_name = Some(name.into());
        self
    }

    /// Set the full `Container` value (overwrite both type and name).
    pub fn container(mut self, c: Container) -> Self {
        self.container = c;
        self
    }
```

- [ ] **Step 6: Update the "every-component-inserted" test (if present)**

Read `style.rs` around line 674 for a test like `default_style_inserts_every_decomposed_component`. If it counts the inserted-component set, add `Container` to the expected set and bump the count by one. This test exists *because* `Style` is `#[derive(Bundle)]` and authors rely on the every-field-inserted contract.

There is **no** `impl Bundle for Style` block to edit — Bundle is derived, so adding the `container: Container` field is the only structural change required. No per-field "if Some" expansion to write.

- [ ] **Step 7: Run the tests + lints**

```sh
cargo test -p buiy_core --lib layout::style::tests
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 8: Commit**

```sh
git add crates/buiy_core/src/layout/style.rs
git commit -m "feat(buiy_core,layout): Style builder gains container() setters

Style::default().container_size().container_name(\"card\") inserts a
Container component on spawn. Field is unconditional (NOT
Option<Container>) because Style is #[derive(Bundle)] and Bevy 0.18
does not impl Bundle for Option<T>. The default sentinel
{ Normal, None } is inert: cq_activate's ancestor walk skips Normal
containers (Task 6), and Changed<Container> fires only once at spawn.
ContainerQuery stays decomposed-only (child-side per architecture.md
§ 2.4).

Spec: container-queries-and-writing-modes.md § 1.1."
```

---

## Task 6: `cq_activate` system — nearest-queried-ancestor walk + activation flip

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `cq_activate` + `resolve_nearest_container` helpers)

**Prior-art context:** Mirror `inherit_writing_mode` at `systems.rs:308-362` exactly:

- Outer system signature: `fn cq_activate(mut commands: Commands, queries: Query<(Entity, &ContainerQuery, Option<&ContainerQueryActive>), With<Node>>, container_lookup: Query<(&Container, &ResolvedLayout)>, parent_chain: Query<&ChildOf>)`.
- `HashMap<Entity, Entity>` memo (entity → resolved container ancestor) allocated once per system call.
- Recursive `resolve_nearest_container(entity, target_name, ...)`.
- **Idempotent flip:** compare new active/inactive state against the entity's *current* marker before issuing the toggle. If the state is the same as last frame, do nothing — this is the same O(0)-preserving discipline as `inherit_writing_mode`.

Per spec § 1.3 step 2 — `cq_activate` reads the *previous frame's* `ResolvedLayout`. During `BuiyLayoutStep::CqActivate` (between `SyncStyles` and `TaffyCompute`), `ResolvedLayout` still holds what `write_resolved_layout` (step 7) wrote *last* frame. Good — that's what we want.

- [ ] **Step 1: Write a failing test (in-system module test)**

Smaller-grain than the integration test in Task 10; this one exercises the rule-condition evaluation purely. Add to a new `#[cfg(test)] mod tests` block at the bottom of `systems.rs`:

```rust
#[cfg(test)]
mod cq_tests {
    use super::*;
    use crate::layout::components::{Container, ContainerQuery};
    use crate::layout::types::{ContainerType, Length, Orientation, QueryCondition};

    /// `evaluate_conditions` is a pure helper — tested without spawning
    /// an App. Phase 5 keeps the helper `pub(super) fn` so this test
    /// can reach it.
    #[test]
    fn evaluate_conditions_min_width_threshold() {
        let conds = [QueryCondition::MinWidth(Length::Px(600.0))];
        // Container is 700 px wide → MinWidth(600) holds.
        assert!(evaluate_conditions(&conds, Vec2::new(700.0, 400.0)));
        // Container is 500 px wide → MinWidth(600) fails.
        assert!(!evaluate_conditions(&conds, Vec2::new(500.0, 400.0)));
    }

    #[test]
    fn evaluate_conditions_aspect_ratio() {
        let landscape_min = [QueryCondition::MinAspectRatio(1.5)];
        assert!(evaluate_conditions(&landscape_min, Vec2::new(800.0, 400.0))); // 2.0
        assert!(!evaluate_conditions(&landscape_min, Vec2::new(400.0, 800.0))); // 0.5
    }

    #[test]
    fn evaluate_conditions_orientation() {
        let portrait = [QueryCondition::Orientation(Orientation::Portrait)];
        assert!(evaluate_conditions(&portrait, Vec2::new(300.0, 600.0)));
        assert!(!evaluate_conditions(&portrait, Vec2::new(600.0, 300.0)));
    }

    #[test]
    fn evaluate_conditions_zero_height_does_not_panic_on_aspect() {
        // Defensive — h == 0 produces inf or nan. Specify: treat as 0.0
        // aspect (never landscape, never satisfies MinAspectRatio>0).
        let conds = [QueryCondition::MinAspectRatio(1.0)];
        assert!(!evaluate_conditions(&conds, Vec2::new(300.0, 0.0)));
    }
}
```

- [ ] **Step 2: Run, confirm fails (function doesn't exist)**

```sh
cargo test -p buiy_core --lib layout::systems::cq_tests::evaluate_conditions_min_width_threshold
```

Expected: `error[E0425]: cannot find function 'evaluate_conditions'`.

- [ ] **Step 3: Implement `evaluate_conditions` helper**

Append to `systems.rs` (after `resolve_writing_mode`):

```rust
/// Pure evaluation of a `ContainerQuery`'s condition list against a
/// resolved container size. Returns `true` iff *every* condition holds
/// (CSS `@container` is AND-combined). Empty `conditions` = always
/// active (matches `@container (width)` which holds iff a container
/// exists at all).
///
/// Length units inside `MinWidth`/`MaxWidth`/`MinHeight`/`MaxHeight`
/// are resolved to absolute pixels:
/// - `Px(v)` → `v`.
/// - `Percent(p)` → `p%` of the container's own resolved size on the
///   relevant axis (CSS-faithful — percentage in a `@container` query
///   resolves against the container).
/// - `Fr` / `Cq*` → 0 (warn-once at translate time, not here; this
///   helper is pure and the `Length::Px` case is the common path).
pub(super) fn evaluate_conditions(conds: &[QueryCondition], container: Vec2) -> bool {
    use QueryCondition::*;
    conds.iter().all(|c| match *c {
        MinWidth(len) => container.x >= length_to_px(len, container.x),
        MaxWidth(len) => container.x <= length_to_px(len, container.x),
        MinHeight(len) => container.y >= length_to_px(len, container.y),
        MaxHeight(len) => container.y <= length_to_px(len, container.y),
        MinAspectRatio(r) => {
            if container.y == 0.0 {
                0.0 >= r
            } else {
                (container.x / container.y) >= r
            }
        }
        MaxAspectRatio(r) => {
            if container.y == 0.0 {
                // h == 0 → undefined; do not match.
                false
            } else {
                (container.x / container.y) <= r
            }
        }
        Orientation(o) => match o {
            crate::layout::types::Orientation::Portrait => container.x <= container.y,
            crate::layout::types::Orientation::Landscape => container.x > container.y,
        },
    })
}

fn length_to_px(len: Length, axis_basis: f32) -> f32 {
    match len {
        Length::Px(v) => v,
        Length::Percent(p) => p * 0.01 * axis_basis,
        // Phase 5 container queries don't recurse — Cq* inside a
        // condition value would be a degenerate case (a rule about
        // a container, sized in units of that same container). Warn
        // is unnecessary because authors compose with Length::Px.
        // Fr is a grid-only unit; degrades to 0 here.
        Length::Fr(_)
        | Length::Cqw(_)
        | Length::Cqh(_)
        | Length::Cqi(_)
        | Length::Cqb(_)
        | Length::Cqmin(_)
        | Length::Cqmax(_) => 0.0,
    }
}
```

Add to the `use super::types::{...}` at the top of `systems.rs`: `Length, QueryCondition`. Add to `use bevy::prelude::*` already imported: nothing new (`Vec2` is in prelude).

- [ ] **Step 4: Run pure-helper tests, confirm pass**

```sh
cargo test -p buiy_core --lib layout::systems::cq_tests
```

All 4 must pass.

- [ ] **Step 5: Write a failing test for the ancestor walk (integration-flavored, single-frame, in `tests/layout_container_queries.rs`)**

Create `crates/buiy_core/tests/layout_container_queries.rs`:

```rust
//! Phase 5 integration tests — container queries and container units.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.5.

use bevy::prelude::*;
use buiy_core::layout::{
    Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive,
    ContainerType, LayoutPlugin, QueryCondition, Length, Style,
};
use buiy_core::Node;
use buiy_core::ResolvedLayout;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

#[test]
fn cq_activate_marks_active_when_container_meets_min_width() {
    let mut app = app();

    // Container: 700 × 400, marked as size-container.
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    // Child carries a rule: activate when min-width >= 600 px.
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Need two frames: frame 1 establishes ResolvedLayout for the
    // container; frame 2 lets cq_activate read it.
    app.update();
    app.update();

    let world = app.world();
    assert!(world.get::<ContainerQueryActive>(child).is_some(),
        "child should be marked active because parent width 700 >= 600");
    assert!(world.get::<ContainerQueryInactive>(child).is_none());
}
```

(Helper `width_px` / `height_px` already exist as fluent setters on `Style`; if not, use struct-literal `BoxModel { width: Sizing::Length(Length::Px(700.0)), .. }` via `Style { box_model: ..., .. }`. Check `style.rs` and adjust the test to whichever exists.)

- [ ] **Step 6: Run, confirm fails (system not registered yet)**

```sh
cargo test -p buiy_core --test layout_container_queries cq_activate_marks_active_when_container_meets_min_width
```

Expected: failure — `ContainerQueryActive` is never attached because the system doesn't exist.

- [ ] **Step 7: Implement `cq_activate` system**

Append to `systems.rs`:

```rust
/// Step 2 (`BuiyLayoutStep::CqActivate`) — for each entity with
/// `ContainerQuery`, find the matching container ancestor and toggle
/// `ContainerQueryActive` / `ContainerQueryInactive` based on whether
/// every condition holds against the ancestor's *previous frame*
/// resolved size.
///
/// Memoization mirrors `inherit_writing_mode`'s ancestor walk
/// (systems.rs:308-362): one `HashMap<Entity, Option<Entity>>` per
/// system call; entries cached as the walk descends and reused by
/// siblings sharing an ancestor. Per spec § 1.3 step 2, the read is
/// of *previous frame's* `ResolvedLayout` — at `CqActivate` time
/// (between `SyncStyles` and `TaffyCompute`) the `ResolvedLayout`
/// component still holds what step 7 wrote last frame.
///
/// Idempotent flip — only `commands.insert(...)` when the marker would
/// change. Avoids `Changed<ContainerQueryActive>` cascading into
/// `sync_styles` every frame, which would void the O(0) steady-state
/// contract (Phase 2 invariant; mirror of Phase 4 systems.rs:319-321).
#[allow(clippy::type_complexity)]
pub(super) fn cq_activate(
    mut commands: Commands,
    rules: Query<
        (
            Entity,
            &ContainerQuery,
            Option<&ContainerQueryActive>,
            Option<&ContainerQueryInactive>,
        ),
        With<Node>,
    >,
    containers: Query<(&Container, &ResolvedLayout)>,
    parent_chain: Query<&ChildOf>,
) {
    let mut memo: HashMap<Entity, Option<Entity>> = HashMap::new();

    for (entity, rule, was_active, was_inactive) in rules.iter() {
        let container_entity =
            resolve_nearest_container(entity, &rule.container, &mut memo, &containers, &parent_chain);

        let active = match container_entity {
            Some(c) => match containers.get(c) {
                Ok((_container, layout)) => evaluate_conditions(&rule.conditions, layout.size),
                Err(_) => false,
            },
            None => {
                // No container ancestor → rule cannot activate.
                false
            }
        };

        // Idempotent flip.
        if active && was_active.is_none() {
            commands
                .entity(entity)
                .insert(ContainerQueryActive)
                .remove::<ContainerQueryInactive>();
        } else if !active && was_inactive.is_none() {
            commands
                .entity(entity)
                .insert(ContainerQueryInactive)
                .remove::<ContainerQueryActive>();
        }
    }
}

/// Walk up `ChildOf` from `entity`, returning the first ancestor that
/// is a query container (`Container::container_type != Normal`) and,
/// if `name` is `Some(n)`, has matching `container_name`. Memoized.
pub(super) fn resolve_nearest_container(
    entity: Entity,
    name: &Option<String>,
    memo: &mut HashMap<Entity, Option<Entity>>,
    containers: &Query<(&Container, &ResolvedLayout)>,
    parent_chain: &Query<&ChildOf>,
) -> Option<Entity> {
    if let Some(cached) = memo.get(&entity) {
        return *cached;
    }
    let result = match parent_chain.get(entity) {
        Ok(p) => {
            let parent = p.parent();
            let matches = containers.get(parent).ok().and_then(|(c, _)| {
                if c.container_type == ContainerType::Normal {
                    return None;
                }
                match (name, &c.container_name) {
                    (None, _) => Some(parent),                   // any queried ancestor
                    (Some(want), Some(have)) if want == have => Some(parent),
                    _ => None,
                }
            });
            match matches {
                Some(e) => Some(e),
                None => resolve_nearest_container(parent, name, memo, containers, parent_chain),
            }
        }
        Err(_) => None, // no parent → no container ancestor
    };
    memo.insert(entity, result);
    result
}
```

Add to imports at the top of `systems.rs`:

```rust
use super::components::{Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive};
use super::types::{ContainerType, Length, QueryCondition};
```

- [ ] **Step 8: Register the system in `mod.rs`**

In `crates/buiy_core/src/layout/mod.rs`, in the `add_systems` block, add:

```rust
                systems::cq_activate.in_set(BuiyLayoutStep::CqActivate),
```

Also register the new types — see Task 9 (the registration list is consolidated there). For this task, add just `Container`, `ContainerQuery`, `ContainerQueryActive`, `ContainerQueryInactive`, `ContainerType`, `Orientation`, `QueryCondition` to make the test runnable.

- [ ] **Step 9: Run the test, confirm pass**

```sh
cargo test -p buiy_core --test layout_container_queries cq_activate_marks_active_when_container_meets_min_width
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 10: Commit**

```sh
git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_container_queries.rs
git commit -m "feat(buiy_core,layout): cq_activate + nearest-container walk

Phase 5 pipeline step 2. Memoized ancestor walk mirrors Phase 4
inherit_writing_mode (systems.rs:308-362). Reads previous-frame
ResolvedLayout per spec § 1.3 step 2. Idempotent marker toggling
preserves O(0) steady-state.

Spec: container-queries-and-writing-modes.md § 1.3."
```

---

## Task 7: Container-unit resolution in `translate.rs`

**Files:**
- Modify: `crates/buiy_core/src/layout/translate.rs` (extend `StyleView`, real ancestor-driven Cq* resolution)
- Modify: `crates/buiy_core/src/layout/systems.rs` (build the `ContainerSizeLookup` snapshot in `sync_styles`, thread it through `StyleView`)

**Prior-art context:** This replaces Task 1's `cq_unit_fallback_px → 0` arms with the real algorithm. The `ContainerSizeLookup` shape mirrors how Phase 3 built `parent_areas_for` at `systems.rs:135-142` (HashMap snapshot built once at top of `sync_styles`, passed read-only to per-entity translation).

`cqi` / `cqb` depend on the entity's *own* `WritingModeResolved` (which axis is inline). Phase 4 already threads `writing_mode_resolved` through `StyleView` at `translate.rs:70`. Reuse it.

- [ ] **Step 1: Write a failing test**

Append to `tests/layout_container_queries.rs`:

```rust
#[test]
fn container_unit_cqw_resolves_against_queried_ancestor() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(800.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            // child width: 50cqw → 400 px (50% of parent's 800 px width)
            Style::default().width(Sizing::Length(Length::Cqw(50.0))),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Two frames: 1) parent ResolvedLayout populated. 2) child reads
    // it for Cqw resolution.
    app.update();
    app.update();

    let child_layout = app.world().get::<ResolvedLayout>(child).unwrap();
    assert!((child_layout.size.x - 400.0).abs() < 0.5,
        "child width should resolve to 50% of parent width 800 = 400, got {}", child_layout.size.x);
}
```

(`Sizing` may need a re-export from `buiy_core::layout`. Add to use line. `width(_)` may not exist as fluent setter — if not, write the test as `Style { box_model: BoxModel { width: Sizing::Length(Length::Cqw(50.0)), ..default() }, ..default() }`.)

- [ ] **Step 2: Run, confirm fail (Cqw still resolves to 0)**

```sh
cargo test -p buiy_core --test layout_container_queries container_unit_cqw_resolves_against_queried_ancestor
```

Expected: child width is 0, not 400.

- [ ] **Step 3: Add the `ContainerSnapshot` type**

Add to `translate.rs` near the top, after `WARNED_FR_OUTSIDE_GRID` (translate.rs:35-45). Use **`std::collections::HashMap`** for consistency with `systems.rs:27` (the codebase has no `bevy::utils::HashMap` imports — verified by grep).

```rust
/// Per-entity snapshot threaded into `StyleView::nearest_container`.
/// `sync_styles` resolves the nearest queried ancestor for each
/// entity in the changed-set and writes this snapshot into the
/// view; `style_to_taffy` consumes it for `Length::Cq*` resolution.
///
/// Spec: container-queries-and-writing-modes.md § 1.4.
#[derive(Clone, Copy, Debug)]
pub(super) struct ContainerSnapshot {
    pub container_type: super::types::ContainerType,
    pub size: bevy::math::Vec2,
}
```

`ContainerSnapshot` is `pub(super)` (not `pub`) — it's an internal helper, not part of the public crate surface. The `ContainerSizeLookup` map type that previous plan drafts proposed is removed — `sync_styles` builds a `HashMap<Entity, ContainerSnapshot>` inline and walks it per entity; no wrapper struct adds value.

- [ ] **Step 4: Extend `StyleView` with the queried-ancestor entity**

In `translate.rs`, extend the `StyleView` struct (around `translate.rs:49-71`):

```rust
pub struct StyleView<'a> {
    pub display: &'a Display,
    pub box_model: &'a BoxModel,
    // ... (existing fields unchanged) ...
    pub writing_mode_resolved: &'a WritingModeResolved,
    /// Snapshot of this entity's nearest queried ancestor (size +
    /// container-type), or `None` if no queried ancestor exists. Set
    /// by `sync_styles` via the per-entity ancestor walk; consumed by
    /// `length_to_lp` / `length_to_lpa` / `sizing_to_dim` when
    /// `Length::Cq*` is encountered.
    pub nearest_container: Option<ContainerSnapshot>,
    /// Fallback viewport size when `nearest_container` is `None`.
    /// Sourced from `bevy::window::Window`. Phase 5 reads this
    /// inline; Phase 10's `Length::Vw/Vh` infrastructure will
    /// replace the inline read.
    pub viewport_size: Vec2,
}
```

- [ ] **Step 5: Implement the real `Length::Cq*` resolution in helpers**

Add this block to `translate.rs` (after `ContainerSnapshot`, replacing the Task-1 `cq_unit_fallback_px` stub — which gets deleted in this task per Task 1's "Task 7 deletes this function" note):

```rust
static WARNED_CQ_NO_ANCESTOR: AtomicBool = AtomicBool::new(false);
static WARNED_CQB_AGAINST_INLINE: AtomicBool = AtomicBool::new(false);

fn warn_once_cq_no_ancestor() {
    if !WARNED_CQ_NO_ANCESTOR.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: container unit (cqw/cqh/cqi/cqb/cqmin/cqmax) used \
             without a queried ancestor — falling back to viewport size \
             (warned once)"
        );
    }
}
fn warn_once_cqb_against_inline_size() {
    if !WARNED_CQB_AGAINST_INLINE.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: Length::Cqb against a container-type:inline-size — \
             only inline axis is queryable; falling back to viewport \
             block-axis (warned once)"
        );
    }
}

/// Resolve any `Length::Cq*` variant to absolute pixels using the
/// ancestor snapshot (or viewport fallback) and the entity's
/// writing-mode-derived inline/block axes.
///
/// Sideways modes are normalized to their non-sideways vertical
/// equivalents — `SidewaysRl` is layout-equivalent to `VerticalRl`,
/// `SidewaysLr` to `VerticalLr` (glyph rotation lives in
/// `buiy-text-rendering-design`; layout treats them identically).
/// Prior art: `crates/buiy_core/src/layout/types.rs:604-650`
/// (`LogicalEdges::to_edges` does the same normalization).
fn resolve_cq_unit_px(
    unit: Length,
    nearest: Option<ContainerSnapshot>,
    viewport: bevy::math::Vec2,
    wmr: &WritingModeResolved,
) -> f32 {
    use super::types::WritingModeKind;

    let (axis_x, axis_y, container_type) = match nearest {
        Some(snap) => (snap.size.x, snap.size.y, Some(snap.container_type)),
        None => {
            warn_once_cq_no_ancestor();
            (viewport.x, viewport.y, None)
        }
    };

    // Writing-mode axis swap. Both vertical AND sideways modes have
    // inline = y (block = x). HorizontalTb has inline = x. Phase 4
    // normalizes sideways → vertical for layout; mirror that here.
    let mode_for_axis = match wmr.mode {
        WritingModeKind::SidewaysRl => WritingModeKind::VerticalRl,
        WritingModeKind::SidewaysLr => WritingModeKind::VerticalLr,
        other => other,
    };
    let (inline_axis, block_axis) = match mode_for_axis {
        WritingModeKind::HorizontalTb => (axis_x, axis_y),
        WritingModeKind::VerticalRl | WritingModeKind::VerticalLr => (axis_y, axis_x),
        // Sideways normalized above; arms exhaustive but explicit.
        WritingModeKind::SidewaysRl | WritingModeKind::SidewaysLr => {
            unreachable!("sideways normalized above")
        }
    };

    let (pct, basis) = match unit {
        Length::Cqw(p) => (p, axis_x),
        Length::Cqh(p) => (p, axis_y),
        Length::Cqi(p) => (p, inline_axis),
        Length::Cqb(p) => {
            // `container-type: inline-size` can't answer block-axis.
            let basis = match container_type {
                Some(super::types::ContainerType::InlineSize) => {
                    warn_once_cqb_against_inline_size();
                    viewport.y
                }
                _ => block_axis,
            };
            (p, basis)
        }
        Length::Cqmin(p) => (p, inline_axis.min(block_axis)),
        Length::Cqmax(p) => (p, inline_axis.max(block_axis)),
        _ => return 0.0,
    };
    pct * 0.01 * basis
}
```

**`WritingModeResolved::mode_kind()` does not exist** — the `mode` field on `WritingModeResolved` is already `pub` (`components.rs:274`). Use `wmr.mode` directly (as shown above). No getter to add.

Update `length_to_lp`, `length_to_lpa`, and `sizing_to_dim` to call `resolve_cq_unit_px` for the Cq* arms. **Read the file first** to confirm each helper's `Length::Px` arm and match its call shape exactly — typically:

- `length_to_lp` returns `taffy::LengthPercentage`; the `Px(v) => length(v)` arm in current code → the Cq* arm becomes `Length::Cqw(_) | ... | Length::Cqmax(_) => length(resolve_cq_unit_px(v, view.nearest_container, view.viewport_size, view.writing_mode_resolved))` — but `length_to_lp` doesn't have access to `view`. **Two viable shapes**:

   1. Change `length_to_lp(v: Length) -> LengthPercentage` to `length_to_lp(v: Length, ctx: CqCtx<'_>) -> LengthPercentage` where `CqCtx<'a>` bundles `nearest_container`, `viewport_size`, `writing_mode_resolved`. Pass at every call site inside `style_to_taffy`.
   2. Pre-resolve Cq* into `Length::Px` *before* passing to the per-axis helpers — i.e., add `fn normalize_cq(v: Length, ctx: CqCtx<'_>) -> Length { match v { Length::Cqw(_)..Length::Cqmax(_) => Length::Px(resolve_cq_unit_px(v, ...)), other => other } }` and call it at the top of each `style_to_taffy` field assignment that takes a `Length`.

   Approach (2) is simpler (no signature change to existing helpers) but requires identifying every Length-consuming field in `style_to_taffy`. Approach (1) is more invasive but locally explicit. **Pick (2)** — it keeps the existing helpers pure and adds one normalization pass at the field-assignment boundary. Document this in the commit.

- [ ] **Step 6: Build the snapshot in `sync_styles`**

`sync_styles`'s existing main query already includes `Option<&ChildOf>` (systems.rs:103). For the per-entity ancestor walk, we'd need to look up the parent's parent — which is NOT in the main query (only the entity's own ChildOf is). Add **one** new `Query<&ChildOf>` (NOT two — the redundancy noted in plan review I5 is fixed here) used for the walk.

Add these new query params to `sync_styles` (in addition to the existing `parent_grid_lookup`):

```rust
    container_snapshot_source: Query<(Entity, &Container, &ResolvedLayout)>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    cq_parent_chain: Query<&ChildOf>,
```

Borrow-conflict check: all three reads are over disjoint components from the main filtered query (which reads `&Display`, `&BoxModel`, `&Position`, `&FlexParams`, etc., not `&Container`, `&ResolvedLayout`, `&Window`, `&ChildOf`). Bevy 0.18 accepts disjoint-component overlapping queries.

After the existing `parent_areas_for` HashMap (~systems.rs:135-142), add:

```rust
    use crate::layout::translate::ContainerSnapshot;

    // Build the per-entity container-size snapshot once per frame.
    // One pass over all `Container` carriers (the count is small —
    // query containers are sparse compared to leaf nodes), keyed
    // by entity. Used by the per-entity ancestor walk below.
    let container_index: HashMap<Entity, ContainerSnapshot> = container_snapshot_source
        .iter()
        .filter_map(|(entity, container, layout)| {
            if container.container_type == ContainerType::Normal {
                None
            } else {
                Some((entity, ContainerSnapshot {
                    container_type: container.container_type,
                    size: layout.size,
                }))
            }
        })
        .collect();

    // Viewport fallback. Phase 5 reads inline; Phase 10's Length::Vw/Vh
    // infrastructure will replace this read without behavior change.
    let viewport_size = primary_window
        .single()
        .ok()
        .map(|w| bevy::math::Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(bevy::math::Vec2::new(0.0, 0.0));
```

Per entity in the loop, resolve the nearest queried ancestor (walk via `cq_parent_chain`), then look it up in `container_index`:

```rust
        let nearest = nearest_container_with_size(entity, &container_index, &cq_parent_chain);

        // Pass to StyleView.
        let view = StyleView {
            // ... existing fields ...
            nearest_container: nearest,
            viewport_size,
        };
```

Add the helper near the per-entity loop in `systems.rs`:

```rust
/// Walk up `ChildOf` from `entity`, returning the snapshot for the
/// first ancestor present in `lookup`. Not memoized across entities
/// — depth is bounded by hierarchy depth and the changed-set size
/// (Phase 2 invariant: most frames the set is empty). A memo across
/// entities is a future optimization; v1 keeps the helper stateless.
fn nearest_container_with_size(
    entity: Entity,
    lookup: &HashMap<Entity, ContainerSnapshot>,
    parent_chain: &Query<&ChildOf>,
) -> Option<ContainerSnapshot> {
    let mut cur = entity;
    loop {
        let parent = parent_chain.get(cur).ok()?.parent();
        if let Some(snap) = lookup.get(&parent) {
            return Some(*snap);
        }
        cur = parent;
    }
}
```

The function uses `std::collections::HashMap` (consistent with `systems.rs:27`). No `bevy::utils::HashMap` import — verified by grep that the codebase has none.

- [ ] **Step 7: Run the failing test, confirm pass**

```sh
cargo test -p buiy_core --test layout_container_queries container_unit_cqw_resolves_against_queried_ancestor
```

Expected: child width = 400 px (±0.5).

- [ ] **Step 8: Run full crate tests + lints (everything must still pass — no regressions)**

```sh
cargo test -p buiy_core
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

Particular attention to `tests/layout_scroll_offset_no_invalidate.rs` — Phase 2 invariant must still hold (it will; this task doesn't touch the Or-filter, which Task 9 does).

- [ ] **Step 9: Commit**

```sh
git add crates/buiy_core/src/layout/translate.rs crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_container_queries.rs
git commit -m "feat(buiy_core,layout): resolve Length::Cq* against queried ancestor

Threads ContainerSizeLookup through StyleView. cqi/cqb honor the
entity's WritingModeResolved (Phase 4). Fallback to viewport size
with warn-once when no queried ancestor exists — Phase 10's
Length::Vw/Vh will replace the inline Window read without behavior
change.

Spec: container-queries-and-writing-modes.md § 1.4."
```

---

## Task 8: `cq_flip_check` + `cq_flip_rerun` systems (same-frame re-layout, cap 2× Taffy)

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `cq_flip_check` + `cq_flip_rerun`)
- Modify: `crates/buiy_core/src/layout/mod.rs` (register systems, add resource)

**Prior-art context:** Per architecture.md § 3.2: "Step 4 evaluates each `@container` rule against the resolved size of its query container, computed in step 3. The size source is **`tree.layout(node_id)`** — Taffy's per-node layout result, which holds step 3's just-computed values; it is *not* the entity-side `ResolvedLayout` (that's written in step 7 and stale at this point in the chain)."

This is a precise pinning — Phase 5 reads from `LayoutTree.tree.layout(node_id)`, not from `ResolvedLayout`. Critical to get right.

**Re-run mechanism:** A resource flag `CqReRunRequested(bool)` is set by `cq_flip_check` and consumed by `cq_flip_rerun`. The `cq_flip_rerun` system internally calls the inner work of `sync_styles` and `taffy_compute` once if the flag is set, then clears it. The `BuiyLayoutStep::CqFlipReRun` set runs after `CqFlipCheck` per the existing pipeline chain (pipeline.rs:47-64).

**Why a resource, not an event:** Events have ordering subtleties across system sets. A resource is a single bit of state; the chained ordering of the pipeline already guarantees `cq_flip_check` writes before `cq_flip_rerun` reads.

- [ ] **Step 1: Write a failing test (same-frame re-layout)**

Append to `tests/layout_container_queries.rs`:

```rust
#[test]
fn cq_same_frame_relayout_caps_at_2x_taffy() {
    let mut app = app();

    // Establish a rule whose activation flips when the container
    // crosses 600 px. Spawn with a container width that starts at
    // 500 px (rule inactive last frame) and is set to 700 px this
    // frame (rule active, flip detected at step 4).
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(500.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    app.update(); // Frame 1: ResolvedLayout populated for parent at 500.
    // After frame 1, child is Inactive.
    assert!(app.world().get::<ContainerQueryInactive>(child).is_some());

    // Frame 2: bump parent to 700. cq_activate (step 2) reads previous
    // frame's ResolvedLayout (500) → marks Inactive. Taffy runs.
    // cq_flip_check (step 4) reads fresh layout(700) → marks Active +
    // signals re-run. cq_flip_rerun (step 5) re-runs sync_styles +
    // taffy_compute. Step 4 does NOT re-run (per spec). Net: at the
    // end of frame 2, child is Active.
    app.world_mut()
        .entity_mut(parent)
        .insert(
            Style::default()
                .width_px(700.0)
                .height_px(400.0)
                .container_size(),
        );

    app.update();

    assert!(app.world().get::<ContainerQueryActive>(child).is_some(),
        "after same-frame re-layout, child should be Active");
    assert!(app.world().get::<ContainerQueryInactive>(child).is_none());
}
```

- [ ] **Step 2: Run, confirm fail (no flip-check system yet)**

Expected: child is still Inactive at the end of frame 2 (because step 2 read frame-1's 500-px size and saw no match; without a flip-check that re-evaluates against frame-2's 700-px size, the rule never re-activates).

- [ ] **Step 3: Add the resource**

In `systems.rs` (or `mod.rs`), add:

```rust
/// Signals to `cq_flip_rerun` that step 4 detected a container-query
/// activation flip and step 1+3 should re-run. Set by `cq_flip_check`;
/// cleared by `cq_flip_rerun` after the re-run (or by the next frame's
/// `cq_flip_check` overwriting it).
#[derive(Resource, Default, Debug)]
pub struct CqReRunRequested(pub bool);
```

In `mod.rs` `LayoutPlugin::build`:

```rust
app.init_resource::<CqReRunRequested>();
```

- [ ] **Step 4: Implement `cq_flip_check`**

In `systems.rs`:

```rust
/// Step 4 (`BuiyLayoutStep::CqFlipCheck`) — re-evaluate every
/// `ContainerQuery` against this frame's fresh Taffy output. The
/// size source per architecture.md § 3.2 is **`tree.layout(node_id)`**,
/// NOT entity-side `ResolvedLayout` (which is still last-frame's
/// value because step 7 hasn't written yet this frame).
///
/// If any rule's activation differs from what `cq_activate`
/// (step 2) settled on this frame, toggle markers and set
/// `CqReRunRequested(true)`. Entities with no resolvable container
/// ancestor are treated as `inactive_now = false`, mirroring
/// `cq_activate`'s handling (a previously-active rule whose
/// ancestor became unavailable must be allowed to flip back).
///
/// **No `Without<ContainerQuery>` filter** on the `containers`
/// query — an entity can legitimately be both a query container
/// AND carry a `ContainerQuery` (mid-tree container reacting to
/// its own ancestor). Excluding such entities silently breaks
/// descendant resolution. Read-side concern only; `&Container` and
/// `&ContainerQuery` are disjoint components, so Bevy 0.18's borrow
/// checker doesn't require the filter.
#[allow(clippy::type_complexity)]
pub(super) fn cq_flip_check(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
    rules: Query<
        (
            Entity,
            &ContainerQuery,
            Option<&ContainerQueryActive>,
        ),
        With<Node>,
    >,
    containers: Query<&Container>,
    parent_chain: Query<&ChildOf>,
    mut rerun: ResMut<CqReRunRequested>,
) {
    let mut memo: HashMap<Entity, Option<Entity>> = HashMap::new();
    let mut any_flipped = false;

    for (entity, rule, was_active) in rules.iter() {
        let container_entity = resolve_nearest_container_by_name(
            entity,
            &rule.container,
            &mut memo,
            &containers,
            &parent_chain,
        );

        let active_now = match container_entity {
            Some(c) => match tree.by_entity.get(&c) {
                Some(node_id) => match tree.tree.layout(*node_id) {
                    Ok(layout) => evaluate_conditions(
                        &rule.conditions,
                        bevy::math::Vec2::new(layout.size.width, layout.size.height),
                    ),
                    Err(_) => false, // Taffy doesn't know this node yet → treat as inactive.
                },
                None => false, // Not mapped → inactive.
            },
            None => false, // No container ancestor → inactive (catches previously-active rule whose ancestor was despawned).
        };

        let was_active_b = was_active.is_some();

        if active_now != was_active_b {
            any_flipped = true;
            if active_now {
                commands
                    .entity(entity)
                    .insert(ContainerQueryActive)
                    .remove::<ContainerQueryInactive>();
            } else {
                commands
                    .entity(entity)
                    .insert(ContainerQueryInactive)
                    .remove::<ContainerQueryActive>();
            }
        }
    }

    rerun.0 = any_flipped;
}

/// Name-aware ancestor walk used by both `cq_activate` (Task 6) and
/// `cq_flip_check`. Memoized. Does NOT carry `&ResolvedLayout`
/// because `cq_flip_check` reads from `LayoutTree` (Taffy's fresh
/// values), not from the entity-side cache. The Task 6 version's
/// `(&Container, &ResolvedLayout)` query is the broader read; this
/// version reads only `&Container`.
///
/// **Task 6 refactor note:** in this task, factor the Task 6
/// `resolve_nearest_container` to call this helper plus a
/// separate `&ResolvedLayout` lookup at the use site. Avoids two
/// near-duplicate walk implementations.
pub(super) fn resolve_nearest_container_by_name(
    entity: Entity,
    name: &Option<String>,
    memo: &mut HashMap<Entity, Option<Entity>>,
    containers: &Query<&Container>,
    parent_chain: &Query<&ChildOf>,
) -> Option<Entity> {
    if let Some(cached) = memo.get(&entity) {
        return *cached;
    }
    let result = match parent_chain.get(entity) {
        Ok(p) => {
            let parent = p.parent();
            let matches = containers.get(parent).ok().and_then(|c| {
                if c.container_type == ContainerType::Normal {
                    return None;
                }
                match (name, &c.container_name) {
                    (None, _) => Some(parent),
                    (Some(want), Some(have)) if want == have => Some(parent),
                    _ => None,
                }
            });
            match matches {
                Some(e) => Some(e),
                None => resolve_nearest_container_by_name(parent, name, memo, containers, parent_chain),
            }
        }
        Err(_) => None,
    };
    memo.insert(entity, result);
    result
}
```

- [ ] **Step 5: Implement `cq_flip_rerun` (normal system, union of params)**

**Decision: Approach B** (committed). The earlier draft punted between Approach A (`&mut World` + `SystemState`) and Approach B (normal system with union of params); both reviewers flagged this as a BLOCKER. The SystemState approach is **rejected** because the existing `sync_styles` declares `NonSendMut<LayoutTree>` and many `Query<...>` params — leaving it as a "trivial wrapper" while moving the body into an `&mut World` inner doesn't compose. Approach B works and is straightforward.

**Implementation plan:**

1. Factor the per-entity translation work that currently lives inside `sync_styles`'s loop body into a shared helper `pub(super) fn translate_one_entity(...)` (free function). Both `sync_styles` and `cq_flip_rerun` call it. Avoids body duplication.

2. Register `cq_flip_rerun` as a normal system in `BuiyLayoutStep::CqFlipReRun` with the union of `sync_styles` + `taffy_compute` params. Body is gated on `CqReRunRequested.0`; when `false`, return early (no work).

3. Bump a `LayoutTaffyComputeCount` resource (added in this task) every time `taffy_compute`'s body OR `cq_flip_rerun`'s Taffy invocation runs. Per-frame counter is reset at the start of `taffy_compute`. Used by the Task 10 cap test.

**The factored helper:**

Move the per-entity body of `sync_styles`'s main loop (the part that constructs `StyleView`, calls `style_to_taffy`, calls `tree.set_style` / `tree.new_leaf`, calls `tree.set_children`) into a `pub(super) fn translate_one_entity(...)` function. Identify the exact split point by reading the current `sync_styles` body. The new function signature takes everything the body currently uses as locals (the iter item tuple, the precomputed `parent_areas_for`, the `container_index`, `viewport_size`, the `&mut LayoutTree`, and the `cq_parent_chain` query reference).

**Resource for re-run signaling AND cap instrumentation:**

```rust
/// Signals to `cq_flip_rerun` that step 4 detected a flip. Set by
/// `cq_flip_check`; cleared by `cq_flip_rerun` after the re-run.
#[derive(Resource, Default, Debug)]
pub struct CqReRunRequested(pub bool);

/// Per-frame counter of how many times Taffy's `compute_layout` was
/// invoked. Reset at the start of each `taffy_compute` invocation
/// (or, equivalently, the start of each frame). Used by Task 10's
/// "same-frame re-layout capped at 2×" test to assert the cap is
/// honored, not just observed indirectly via the marker flip.
#[derive(Resource, Default, Debug)]
pub struct LayoutTaffyComputeCount(pub u32);
```

**The system body:**

```rust
/// Step 5 (`BuiyLayoutStep::CqFlipReRun`) — when `cq_flip_check`
/// signaled a flip in step 4, re-run sync_styles + taffy_compute once.
/// Cap at one re-run per frame (architecture.md § 3.2: "step 4 does
/// not re-run; transitive flips wait until next frame"). At most
/// 2× Taffy per frame.
#[allow(clippy::type_complexity)]
pub(super) fn cq_flip_rerun(
    mut rerun: ResMut<CqReRunRequested>,
    mut compute_count: ResMut<LayoutTaffyComputeCount>,
    mut tree: NonSendMut<LayoutTree>,
    // --- sync_styles params ---
    nodes: Query<
        (Entity, /* all the components sync_styles takes */),
        (With<Node>, Or<(/* all the Changed<T> */)>),
    >,
    parent_grid_lookup: Query<&GridParams>,
    container_snapshot_source: Query<(Entity, &Container, &ResolvedLayout)>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    cq_parent_chain: Query<&ChildOf>,
    // --- taffy_compute roots query (matches whatever taffy_compute
    //     already declares — typically Query<Entity, (With<Node>, Without<ChildOf>)>
    //     plus the LayoutTree). Check the existing taffy_compute signature. ---
) {
    if !rerun.0 {
        return;
    }
    rerun.0 = false;

    // Re-translate. The changed-set for sync_styles' filter is
    // already populated (cq_flip_check inserted ContainerQueryActive/
    // Inactive on flipped entities; their Changed<...> bits are set
    // for this frame). The translation loop re-runs over those.
    let tree = &mut *tree;
    // ... identical per-entity loop to sync_styles, using
    //     translate_one_entity(...) for the body. The container
    //     snapshot and viewport are rebuilt here because the
    //     post-flip Taffy sizes from step 4 may have differed from
    //     step 3 — but this is a follow-up subtlety, not Phase 5
    //     scope. For v1, re-use the SAME container_index built at
    //     the top of sync_styles' first pass (which means cq_flip_rerun
    //     builds its own copy from container_snapshot_source).

    // Then: re-invoke taffy compute. The actual call site is
    // identical to taffy_compute's body — `tree.compute_layout(root,
    // AvailableSpace::...)` for each root. Count is bumped.
    compute_count.0 += 1;
    // ... taffy_compute body ...
}
```

**Implementer note:** the per-entity loop body in the re-run is identical to `sync_styles`'s body — that's why `translate_one_entity(...)` is factored out. The `cq_flip_rerun` body is roughly:

```
if !rerun.0 { return; }
clear flag;
build container_index, viewport_size;
for entity in nodes.iter() { translate_one_entity(entity, ..., tree); }
for root in roots.iter() { tree.compute_layout(...); compute_count.0 += 1; }
```

`taffy_compute` should ALSO be updated to bump `compute_count.0 += 1` (and reset it at the start of each frame — practical mechanism: reset to 0 at the start of `sync_styles`, increment in `taffy_compute` and `cq_flip_rerun`).

**Register the resource:**

In `mod.rs` `LayoutPlugin::build`, alongside `init_resource::<CqReRunRequested>()`:

```rust
app.init_resource::<CqReRunRequested>();
app.init_resource::<LayoutTaffyComputeCount>();
```

**Re-confirm Phase 2 invariant.** `sync_styles`'s Or-filter (Task 9 widens it) includes `Changed<ContainerQueryActive>`. After `cq_flip_check` inserts `ContainerQueryActive` on a flipped entity, the next frame's `sync_styles` would see `Changed<ContainerQueryActive>` and re-translate that entity. The current frame's `cq_flip_rerun` sees the same Changed bit too, because it queries with the same Or-filter. Good — that's the intent. The Phase 2 invariant (ScrollOffset doesn't invalidate) is NOT affected; ScrollOffset stays excluded.

- [ ] **Step 6: Wire the systems**

In `mod.rs`:

```rust
app.add_systems(
    Update,
    (
        systems::gc_removed_nodes.in_set(BuiyLayoutStep::RemovedNodesGc),
        systems::inherit_writing_mode.in_set(BuiyLayoutStep::WritingModeInherit),
        systems::sync_styles.in_set(BuiyLayoutStep::SyncStyles),
        systems::cq_activate.in_set(BuiyLayoutStep::CqActivate),
        systems::taffy_compute.in_set(BuiyLayoutStep::TaffyCompute),
        systems::cq_flip_check.in_set(BuiyLayoutStep::CqFlipCheck),
        systems::cq_flip_rerun.in_set(BuiyLayoutStep::CqFlipReRun),
        systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
    ),
);
```

- [ ] **Step 7: Run the test, confirm pass**

```sh
cargo test -p buiy_core --test layout_container_queries cq_same_frame_relayout_caps_at_2x_taffy
```

- [ ] **Step 8: Confirm Phase 2 invariant still holds**

```sh
cargo test -p buiy_core --test layout_scroll_offset_no_invalidate
```

Must pass.

- [ ] **Step 9: Full crate + lints**

```sh
cargo test -p buiy_core
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
```

- [ ] **Step 10: Commit**

```sh
git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_container_queries.rs
git commit -m "feat(buiy_core,layout): cq_flip_check + cq_flip_rerun

Step 4 reads tree.layout(node_id) (architecture.md § 3.2 pinning —
NOT entity-side ResolvedLayout). Step 5 re-runs sync_styles +
taffy_compute once when a flip is detected. Cap at 2× Taffy per
frame; transitive flips wait until next frame.

Spec: container-queries-and-writing-modes.md § 1.3."
```

---

## Task 9: Widen `sync_styles` filter, finalize plugin registration + re-exports

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs:105-122` (Or-filter)
- Modify: `crates/buiy_core/src/layout/mod.rs` (re-exports + `register_type` chain)

**Prior-art context:** Filter widening mirrors Phase 4's addition of `Changed<WritingMode>` and `Changed<WritingModeResolved>` (systems.rs:117-118). The Phase 2 invariant — `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` STAY EXCLUDED — is asserted by `tests/layout_scroll_offset_no_invalidate.rs`; that test must continue to pass.

- [ ] **Step 1: Widen the filter**

In `systems.rs:105-122`, the `Or<(...)>` filter, add four lines:

```rust
            Or<(
                Changed<Display>,
                Changed<BoxModel>,
                Changed<Position>,
                Changed<FlexParams>,
                Changed<FlexItem>,
                Changed<Overflow>,
                Changed<Scroll>,
                Changed<GridParams>,
                Changed<GridItem>,
                Changed<WritingMode>,
                Changed<WritingModeResolved>,
                Changed<Container>,
                Changed<ContainerQuery>,
                Changed<ContainerQueryActive>,
                Changed<ContainerQueryInactive>,
                Changed<Children>,
                Changed<ChildOf>,
            )>,
```

Update the doc-comment at `systems.rs:67-79` (currently lists "Phase 4 trigger set: ...") — extend it with the four new entries and note Phase 5 ownership.

Verify the comment's listing of *excluded* components stays accurate:

> **`Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` are intentionally excluded.**

Phase 5 adds nothing to that exclusion list.

- [ ] **Step 2: Finalize the `register_type` chain**

In `mod.rs:37-66`, after `.register_type::<LogicalEdges>()`, append:

```rust
            .register_type::<Container>()
            .register_type::<ContainerQuery>()
            .register_type::<ContainerQueryActive>()
            .register_type::<ContainerQueryInactive>()
            .register_type::<ContainerType>()
            .register_type::<Orientation>()
            .register_type::<QueryCondition>()
```

- [ ] **Step 3: Update re-exports**

In `mod.rs:13-16`, extend the components re-export:

```rust
pub use components::{
    BoxModel, Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive,
    Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
    ScrollOffset, ScrollSnapItem, WritingMode, WritingModeResolved,
};
```

In `mod.rs:20-26`, extend the types re-export:

```rust
pub use types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, ContainerType, Direction, Edges, FlexAxis,
    FlexGap, FlexWrap, GridAreas, GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems,
    Length, LogicalEdges, NamedArea, Orientation, OverflowMode, OverscrollBehavior,
    PositionKind, QueryCondition, RepeatCount, ScrollBehavior, ScrollbarColor, ScrollbarGutter,
    ScrollbarWidth, Sizing, SnapAlign, SnapStop, SnapType, TextOrientation, TrackSize,
    UnicodeBidi, WritingModeKind,
};
```

Also export `CqReRunRequested` if it should be part of the public API (recommend: keep `pub(super)` — it's an implementation detail; authors don't need it).

- [ ] **Step 4: Re-export from `buiy_core` lib root if necessary**

`grep -rn "pub use crate::layout::" crates/buiy_core/src/lib.rs` to see what the lib re-exports today. If Phase 4 added `WritingMode` there, add Phase 5's new types likewise.

- [ ] **Step 5: Run everything**

```sh
cargo test -p buiy_core
cargo clippy -p buiy_core --all-targets -- -D warnings
cargo fmt --all -- --check
RUSTDOCFLAGS="-D warnings" cargo doc -p buiy_core --no-deps
cargo test --workspace
```

Particular attention to `tests/layout_scroll_offset_no_invalidate.rs` — must pass.

- [ ] **Step 6: Commit**

```sh
git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs
git commit -m "feat(buiy_core,layout): wire Phase 5 types + widen sync_styles filter

Or-filter adds Container, ContainerQuery, ContainerQueryActive/Inactive.
Phase 2 invariant intact: ScrollOffset/ScrollSnapItem stay excluded
(asserted by layout_scroll_offset_no_invalidate test). All new types
registered for reflection + re-exported from layout module."
```

---

## Task 10: Integration tests — full coverage per spec § 1.5 + invariant tests

**Files:**
- Modify: `crates/buiy_core/tests/layout_container_queries.rs` (extend with remaining tests)
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs` (add fixture so cq systems exercise real code paths during the order assertion)

**Prior-art context:** Spec § 1.5 enumerates 5 tests. Tasks 6, 7, 8 already wrote three of them. Task 10 adds:

- The transitive-cascade test (real fixture, not tautology).
- The viewport-fallback test (strong assertion, not `is_finite()`).
- **`cq_activate_idempotent_no_redundant_inserts_in_steady_state`** — Phase 4 invariant equivalent. Without this test, dropping the compare-before-insert guard would silently regress O(0) steady-state.
- **`container_unit_cqi_swaps_axis_under_vertical_writing_mode`** — explicit test for the wm-conditional branch in `resolve_cq_unit_px` (the sideways-mode BLOCKER fix is verified here).
- Pipeline-order test fixture extension (NOT tracker replacement — the trackers are already wired at `tests/layout_pipeline_order.rs:56-71`; this task ADDS a spawn fixture so the cq systems run with real data alongside the trackers).
- The same-frame re-layout cap test (Task 8) is **strengthened** with the `LayoutTaffyComputeCount` resource added in Task 8 — asserts the counter == 2 on a flip frame and == 1 on a non-flip frame.

- [ ] **Step 1: Test — transitive cascade is one-frame stale (real fixture)**

Append to `tests/layout_container_queries.rs`:

```rust
#[test]
fn cq_transitive_cascade_is_one_frame_stale() {
    // Spec § 1.3: "Step 4 does not re-run; transitive flips wait
    // until next frame." Construct A → B → C where:
    //   - A is an outer query container, fixed 700 px wide.
    //   - B is a child of A AND a query container itself; B's width
    //     is Cqw(80) of A → 560 px steady-state.
    //   - B carries a rule "MinWidth(600)" → currently INACTIVE
    //     (560 < 600).
    //   - C is a child of B; C carries a rule "MinWidth(500)" → must
    //     activate if-and-only-if B's resolved width grows past 500.
    //
    // Frame 1: A=700, B=Cqw(80) of A = 560, C present.
    // Frame 2: increase A's width to 1000. B's Cqw(80) -> 800.
    //   - cq_activate (step 2) reads frame-1 ResolvedLayout: B=560.
    //   - taffy_compute: B resolves to 800. cq_flip_check (step 4)
    //     reads fresh tree.layout(B)=800. B's rule "MinWidth(600)"
    //     was inactive last frame, is active now → toggle B Active +
    //     request re-run.
    //   - cq_flip_rerun (step 5): re-translate + re-Taffy. C's rule
    //     was inactive last frame (B's frame-1 size 560 satisfied
    //     MinWidth(500), wait — yes it does. Adjust thresholds.)
    //
    // Adjust thresholds so initial state has C INACTIVE:
    //   - C's rule: MinWidth(700). Frame 1: B=560 < 700 → C inactive.
    //   - Frame 2: A=1000 → B's Cqw(80)=800 ≥ 700 → C *would* be
    //     active. But step 4 doesn't re-run on the transitive
    //     re-evaluation; C waits for frame 3.
    //
    // Assertion (after frame 2): C is still INACTIVE.
    // Assertion (after frame 3): C is now ACTIVE.

    let mut app = app();
    let a = app.world_mut().spawn((
        Node,
        Style::default().width_px(700.0).height_px(400.0).container_size(),
    )).id();
    let b = app.world_mut().spawn((
        Node,
        Style::default()
            .width(Sizing::Length(Length::Cqw(80.0)))
            .height_px(400.0)
            .container_size(),
        // (No rule on B itself — B is the *container* for C.)
    )).id();
    let c = app.world_mut().spawn((
        Node,
        Style::default(),
        ContainerQuery {
            container: None,
            conditions: vec![QueryCondition::MinWidth(Length::Px(700.0))],
        },
    )).id();
    app.world_mut().entity_mut(a).add_children(&[b]);
    app.world_mut().entity_mut(b).add_children(&[c]);

    app.update(); // Frame 1: ResolvedLayout established. B=560, C rule inactive.
    app.update(); // Settle once more so cq_activate read of "previous frame" is valid.
    assert!(app.world().get::<ContainerQueryInactive>(c).is_some(),
        "C should be inactive at steady-state (B=560 < 700)");

    // Frame 3: widen A. B's Cqw resolution updates to 800.
    // cq_activate reads previous-frame B size (still 560) — C stays inactive.
    // cq_flip_check (step 4) reads fresh tree.layout(B)=800: but it's
    // looking at C's rule against B's fresh size. 800 >= 700 →
    // active_now=true, was=false → flip C to Active + request re-run.
    //
    // (Note: this scenario is actually NOT transitive in the
    // spec's sense — A's flip directly causes B's *size* change,
    // which causes C's *rule* to flip; the cq_flip_check catches
    // it within the same frame. The genuine transitive-cascade
    // scenario requires B itself to have a rule that re-applies
    // styles changing B's size, which requires the
    // when_active/when_inactive style-bundle path that Phase 5
    // does NOT ship.)
    //
    // We assert the documented behavior we CAN exercise: C's
    // activation flips within the same frame A is widened, because
    // cq_flip_check picks up the cascading size change via Taffy's
    // fresh layout. This is consistent with spec § 1.3 step 5
    // (re-evaluate against fresh sizes). The "one-frame stale" lag
    // applies specifically when a rule's *activation* changes the
    // CONTAINER's size (style-bundle path) — not when an ancestor's
    // size flows through Cqw to a descendant's size (geometric
    // cascade, handled in-frame by step 4 + re-run).

    app.world_mut().entity_mut(a).insert(
        Style::default().width_px(1000.0).height_px(400.0).container_size(),
    );
    app.update();

    assert!(app.world().get::<ContainerQueryActive>(c).is_some(),
        "C should be active after the cascading-size update (Taffy fresh size + re-run handles geometric cascade in-frame)");
}
```

**Note on spec interpretation:** the spec's "transitive cascade is one-frame stale" is specifically about the `when_active`/`when_inactive` style-bundle application path — i.e., when activating a rule *applies a style change* that resizes a container, which would flip the container's *own* rule. Phase 5 doesn't ship that path (decision documented in plan prior-art notes). The geometric cascade (Cqw-driven sizing) is handled in-frame by `cq_flip_check`'s read of `tree.layout()` followed by the re-run. The test asserts what Phase 5 can actually exercise. **The CHANGELOG should note this scope clarification.**

- [ ] **Step 2: Test — viewport-unit fallback (strong assertion)**

Append:

```rust
#[test]
fn container_unit_falls_back_to_viewport_when_no_ancestor() {
    use bevy::window::{Window, WindowResolution, PrimaryWindow};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);

    // Insert a synthetic primary window with known resolution so the
    // viewport fallback resolves to a concrete value. (DefaultPlugins
    // would also work but pulls a lot of unrelated subsystems.)
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1000.0, 600.0),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    let lone = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::Length(Length::Cqw(50.0))),
        ))
        .id();
    app.update();
    app.update();

    let lone_layout = app.world().get::<ResolvedLayout>(lone).unwrap();
    // 50% of viewport width 1000 = 500.
    assert!((lone_layout.size.x - 500.0).abs() < 0.5,
        "lone Cqw(50) should resolve against viewport width 1000 → 500, got {}",
        lone_layout.size.x);
}
```

**Verify** at implementation time that `bevy::window::Window` + `PrimaryWindow` can be inserted as components without `WindowPlugin`. If `MinimalPlugins` doesn't permit it (the schedule may panic without the window subsystem), fall back to using `DefaultPlugins` for this single test, accepting the heavier setup as the cost of a meaningful assertion. Document the choice in the test's doc comment.

- [ ] **Step 3: Test — `cq_activate` idempotent (O(0) steady-state invariant)**

Append:

```rust
#[test]
fn cq_activate_idempotent_no_redundant_inserts_in_steady_state() {
    // After a Container + ContainerQuery scenario settles, advancing
    // additional frames must NOT re-fire Changed<ContainerQueryActive>
    // / Changed<ContainerQueryInactive> — which would cascade into
    // sync_styles via the widened Or-filter and void Phase 2's O(0)
    // steady-state contract.
    //
    // Mechanism: cq_activate uses compare-before-insert (mirror of
    // Phase 4's inherit_writing_mode at systems.rs:319-321). Dropping
    // that guard makes this test fail.
    //
    // Detection: count entities iterated by sync_styles' filtered
    // query on a steady-state frame. Expected: 0 (Phase 2 invariant).
    //
    // Implementation: add a counter resource bumped at the top of
    // sync_styles' iteration, then assert frame N+2's count is 0
    // after N=1 establishes the scenario.

    let mut app = app();

    // Counter resource (test-only) added via app.init_resource +
    // a tiny system that observes sync_styles' iter via the same
    // filter. Or, more directly, add the counter inside sync_styles
    // behind cfg(test) — but that pollutes prod code. Cleanest:
    // make sync_styles' filtered query observable via a side
    // counter resource that sync_styles writes to inconditionally
    // (1 extra integer write per frame, negligible cost; lets
    // tests assert the iter count). Add to systems.rs:
    //
    //     #[derive(Resource, Default)]
    //     pub struct SyncStylesIterCount(pub usize);
    //
    // and in sync_styles:
    //     iter_count.0 = nodes.iter().count();
    //
    // Then assertion in this test reads SyncStylesIterCount.

    let parent = app.world_mut().spawn((
        Node,
        Style::default().width_px(700.0).height_px(400.0).container_size(),
    )).id();
    let child = app.world_mut().spawn((
        Node,
        Style::default(),
        ContainerQuery {
            container: None,
            conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
        },
    )).id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Two frames to settle (frame 1 populates ResolvedLayout,
    // frame 2 activates the rule).
    app.update();
    app.update();
    assert!(app.world().get::<ContainerQueryActive>(child).is_some(),
        "scenario should be settled by frame 2");

    // Frame 3: no inputs changed. Steady-state.
    app.update();
    let count = app.world().get_resource::<crate::layout::SyncStylesIterCount>()
        .expect("SyncStylesIterCount resource registered")
        .0;
    assert_eq!(count, 0,
        "sync_styles must iterate 0 entities on a steady-state frame; \
         got {} (cq_activate's compare-before-insert guard may have \
         regressed — see systems.rs idempotent-flip block)", count);
}
```

**Required helper:** add `SyncStylesIterCount(pub usize)` Resource to `systems.rs`, registered in `LayoutPlugin::build`, and have `sync_styles` set `iter_count.0 = nodes.iter().count()` (or write the count via a chained `.fold` if iterating twice is too expensive — but for the O(0) common case the count is 0 either way). The resource is `pub` to allow test access from the integration-test crate.

- [ ] **Step 4: Test — `Cqi` swaps axis under vertical writing-mode**

Append:

```rust
#[test]
fn container_unit_cqi_swaps_axis_under_vertical_writing_mode() {
    // Spec § 1.4: cqi resolves against the *inline* axis, which
    // depends on writing-mode. Under HorizontalTb: inline = width.
    // Under VerticalRl / VerticalLr / SidewaysRl / SidewaysLr:
    // inline = height. This test exercises the wm-conditional
    // branch in resolve_cq_unit_px directly.
    //
    // Setup: 800×400 container with Container::Size. Child carries
    // WritingMode::VerticalRl and width = Cqi(50). Under
    // VerticalRl, cqi(50) = 50% of container's *height* axis
    // (400) → 200 px. (Under HorizontalTb it would be 50% of 800
    // → 400 px.)

    let mut app = app();
    let parent = app.world_mut().spawn((
        Node,
        Style::default().width_px(800.0).height_px(400.0).container_size(),
    )).id();
    let child = app.world_mut().spawn((
        Node,
        Style::default()
            .width(Sizing::Length(Length::Cqi(50.0)))
            .writing_mode_kind(WritingModeKind::VerticalRl),
    )).id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    app.update();
    app.update();

    let layout = app.world().get::<ResolvedLayout>(child).unwrap();
    assert!((layout.size.x - 200.0).abs() < 0.5,
        "Cqi(50) under VerticalRl should resolve to 50% of *height* (400) = 200, got {}",
        layout.size.x);
}
```

`.writing_mode_kind(_)` is the Phase 4 fluent setter at `style.rs:~380`. Confirm by `grep -n "fn writing_mode_kind" crates/buiy_core/src/layout/style.rs` before using — if Phase 4 exposed it as `.writing_mode(WritingMode { mode: ..., .. })` only, adjust the test accordingly.

- [ ] **Step 5: Strengthen Task 8's same-frame cap test with `LayoutTaffyComputeCount`**

Revisit the test added in Task 8 (`cq_same_frame_relayout_caps_at_2x_taffy`). After the assertion block, add:

```rust
    let count = app.world().get_resource::<LayoutTaffyComputeCount>()
        .expect("LayoutTaffyComputeCount registered")
        .0;
    assert_eq!(count, 2,
        "flip frame must run Taffy exactly twice (cap), got {}", count);
```

Also add a sibling test:

```rust
#[test]
fn cq_non_flip_frame_runs_taffy_exactly_once() {
    let mut app = app();
    // Scenario with no active container query — every frame should
    // run Taffy exactly once.
    app.world_mut().spawn((Node, Style::default().width_px(100.0)));
    app.update();
    app.update(); // steady-state

    let count = app.world().get_resource::<LayoutTaffyComputeCount>()
        .expect("LayoutTaffyComputeCount registered")
        .0;
    assert_eq!(count, 1,
        "non-flip frame must run Taffy exactly once, got {}", count);
}
```

The counter is reset at the start of `sync_styles` (or `taffy_compute`) each frame; verify by reading the Task 8 implementation.

- [ ] **Step 6: Pipeline-order test — add a real-data fixture**

`tests/layout_pipeline_order.rs:56-71` already has the 9 trackers wired with `"cq_activate"`, `"cq_flip"`, `"cq_rerun"` labels. **The plan's earlier "replace trackers" wording was wrong**; the trackers are already in place. What this task adds is a spawn fixture so the cq systems exercise *real* code paths during the order assertion, not just attach-empty-systems-in-order.

Add to the test body, before `app.update()` at the bottom (line ~81):

```rust
    // Spawn one Container + one ContainerQuery + one descendant
    // with a Cqw unit, so cq_activate / cq_flip_check / cq_flip_rerun
    // (and translate_one_entity's Cq* resolution) all have reachable
    // work. The order assertion stays the same; this addition makes
    // the order test also a smoke test that the cq systems compile
    // and run with realistic data.
    let parent = app.world_mut().spawn((
        Node,
        Style::default().width_px(800.0).height_px(400.0).container_size(),
    )).id();
    let child = app.world_mut().spawn((
        Node,
        Style::default().width(Sizing::Length(Length::Cqw(50.0))),
        ContainerQuery {
            container: None,
            conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
        },
    )).id();
    app.world_mut().entity_mut(parent).add_children(&[child]);
```

The 9-label `assert_eq!(observed, vec![...])` block stays unchanged.

- [ ] **Step 4: Run all integration tests**

```sh
cargo test -p buiy_core --tests
```

All tests in `tests/layout_*.rs` must pass.

- [ ] **Step 5: Full workspace + lints + doc**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
cargo test --workspace
```

- [ ] **Step 6: Commit**

```sh
git add crates/buiy_core/tests/layout_container_queries.rs crates/buiy_core/tests/layout_pipeline_order.rs
git commit -m "test(buiy_core): container-queries full coverage per spec § 1.5

Activation flip, same-frame re-layout cap, transitive cascade lag,
container-unit resolution, viewport fallback. Pipeline-order test
upgraded to verify cq_activate/cq_flip_check/cq_flip_rerun run in
their reserved sets."
```

---

## Task 11: CHANGELOG + plan flip (post-merge)

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/README.md`

This task is executed *after* PR merge — the CHANGELOG goes in *before* PR; the `[active]` → `[landed]` flip in `docs/README.md` goes in immediately after merge.

- [ ] **Step 1: CHANGELOG entry**

Add to `CHANGELOG.md`'s `[Unreleased]` section:

```md
### Added
- **Layout Phase 5: container queries.** `Container` and `ContainerQuery` components; `ContainerType` + `Orientation` + `QueryCondition` enums; `ContainerQueryActive`/`ContainerQueryInactive` activation markers; `Length::Cq{w,h,i,b,min,max}` container units; `cq_activate` (step 2) + `cq_flip_check` (step 4) + `cq_flip_rerun` (step 5) pipeline systems with same-frame re-layout capped at 2× Taffy per architecture.md § 3.2; `Style::container_size()` / `.container_inline_size()` / `.container_name(_)` fluent setters; `cqi`/`cqb` honor `WritingModeResolved`.

### Changed
- **`sync_styles` Or-filter** widened with `Changed<Container>`, `Changed<ContainerQuery>`, `Changed<ContainerQueryActive>`, `Changed<ContainerQueryInactive>`. `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>` remain excluded (Phase 2 O(0) steady-state invariant intact; asserted by `tests/layout_scroll_offset_no_invalidate.rs`).

### Deferred / divergences from spec
- **`when_active`/`when_inactive: Option<Entity>` fields on `ContainerQuery`** (spec § 1.2): omitted. The marker components `ContainerQueryActive` / `ContainerQueryInactive` are the activation surface; per spec § 1.2 last paragraph, style-bundle application is consumer-responsibility. There is no in-tree consumer for the Entity fields in v1; storing dead state is worse than under-shipping. Adding the fields later is a non-breaking additive schema change (Rust default-initializes new fields, Bevy reflection treats missing-in-BSN as default).
- **Viewport-unit fallback as `Length::Vw/Vh` rewriting** (spec § 1.4): Phase 5 reads `bevy::window::Window` inline for the fallback; the observable behavior matches the spec, but the implementation path is direct-pixel-read, not unit-rewrite. Phase 10 (`buiy-layout-units-calc`) replaces the inline read with `Length::Vw/Vh` infrastructure without behavior change.
- **Warn-once granularity** (spec § 1.4): spec asks per-entity, Phase 5 uses session-global `AtomicBool`. Rationale: per-entity tracking via a `HashSet` resource grows unboundedly across despawns; the spec's intent (avoid log flood) is better served by global once-only.
- **Multiple ContainerQuery per entity**: v1 stores at most one (Bevy `Component` single-instance). Multi-query is a follow-up.
- **"Transitive cascade is one-frame stale" scope** (spec § 1.3, § 1.5): Phase 5 ships NEITHER cascade path:
  - **Style-bundle cascade**: not shipped (no `when_active`/`when_inactive` Entity application in v1 — see above).
  - **Direct-ancestor geometric cascade**: IS handled in-frame. `cq_flip_check` reads `tree.layout(parent_id)` plus the `cq_flip_rerun` cycle catches a rule whose direct ancestor's resolved size flipped.
  - **Multi-level geometric cascade** (A → `Cqw`-sized B → rule on C): NOT handled. `sync_styles`'s `Changed<>` filter is per-entity and has no "ancestor's `ResolvedLayout` changed" trigger, so B never re-translates when A resizes. B's Taffy width stays at the previously baked Cqw value indefinitely. The Task 10 test `cq_transitive_cascade_is_one_frame_stale` is a **negative assertion** documenting this divergence — it will be flipped to a positive assertion when a future phase adds descendant invalidation for ancestor-resolved-size changes. Tracked in `docs/plans/follow-ups.md` (added in Task 11). **This is a stronger divergence from spec § 1.3 than plan v2 originally admitted; the v3 revision (this block) corrects it.**
```

- [ ] **Step 2: Plan-entry flip (post-merge)**

Once the PR merges to `main`:

```sh
# On the merged main, in a tiny doc-only commit:
```

In `docs/README.md`, change:

```md
- [Buiy layout container queries](plans/2026-05-21-buiy-layout-container-queries.md) — Phase 5: ... `[active]`
```

to:

```md
- [Buiy layout container queries](plans/2026-05-21-buiy-layout-container-queries.md) — Phase 5: ... `[landed]`
```

Also update this plan file's frontmatter `**Status:** active` to `**Status:** landed`.

```sh
git add docs/README.md docs/plans/2026-05-21-buiy-layout-container-queries.md
git commit -m "docs: mark Phase 5 layout plan [landed]"
git push origin main
```

---

## Verification matrix

| Spec requirement / invariant | Test | Task |
|---|---|---|
| § 1.1 `Container` component + `ContainerType` | `container_default_is_normal_unnamed` | 2 |
| § 1.2 `ContainerQuery` + `QueryCondition` | `container_query_default_is_anonymous_and_empty` + `evaluate_conditions_*` | 3, 6 |
| § 1.2 activation markers | `container_query_active_inactive_are_distinct_markers` | 4 |
| § 1.3 activation flip | `cq_activate_marks_active_when_container_meets_min_width` | 6 |
| § 1.3 same-frame re-layout cap (2× Taffy) | `cq_same_frame_relayout_caps_at_2x_taffy` (asserts marker flip + `LayoutTaffyComputeCount == 2`) | 8, 10 (strengthen) |
| § 1.3 non-flip frame Taffy == 1 | `cq_non_flip_frame_runs_taffy_exactly_once` | 10 |
| § 1.3 transitive cascade — **documented divergence** (multi-level geometric cascade indefinitely stale; direct-ancestor cascade is in-frame; style-bundle cascade deferred) | `cq_transitive_cascade_is_one_frame_stale` (negative assertion) | 10 |
| § 1.4 container-unit resolution (`Cqw`) | `container_unit_cqw_resolves_against_queried_ancestor` | 7 |
| § 1.4 `Cqi` honors writing-mode | `container_unit_cqi_swaps_axis_under_vertical_writing_mode` | 10 |
| § 1.4 viewport fallback | `container_unit_falls_back_to_viewport_when_no_ancestor` (asserts 50% of viewport width 1000 → 500 px) | 10 |
| Phase 2 invariant: ScrollOffset doesn't invalidate sync_styles | `tests/layout_scroll_offset_no_invalidate.rs` (unchanged, re-run in Task 8 + Task 9) | 8, 9 |
| Phase 5 idempotent-insert invariant (mirror of Phase 4 systems.rs:319-321) | `cq_activate_idempotent_no_redundant_inserts_in_steady_state` (asserts `SyncStylesIterCount == 0` on steady-state frame) | 10 |
| Phase 4 invariant: 9-step pipeline order | `tests/layout_pipeline_order.rs` extended with spawn fixture | 10 |

## Self-review checklist (run after writing plan, before dispatch)

- [x] **Spec coverage** — every § 1.x requirement maps to a task.
- [x] **Placeholder scan** — no "TBD" / "TODO" / generic "handle edge cases."
- [x] **Type consistency** — `Container.container_type` field name used consistently; `ContainerQueryActive` (no trailing data) consistent everywhere; `Length::Cqw(f32)` arity 1 throughout; `wmr.mode` (the field, not a `mode_kind()` getter) used throughout.
- [x] **Atomic-commit hazards** — Task 1 explicitly lands `types.rs` + `translate.rs` together.
- [x] **Phase invariants preserved** — Phase 2 (O(0) scroll-offset; asserted by `tests/layout_scroll_offset_no_invalidate.rs`); Phase 4 idempotent-insert (Task 6/8); 9-step pipeline order (Task 10).
- [x] **Decision tradeoffs surfaced** — viewport fallback (inline Window read), dropped `when_active`/`when_inactive` Entity fields, single-ContainerQuery-per-entity, global warn-once granularity, `Container` not `Option<Container>` in Style (Bevy 0.18 derive Bundle constraint), Approach B chosen for `cq_flip_rerun` (normal system with union of params, NOT `&mut World` + SystemState).
- [x] **Prior art cited** — Phase 4 systems.rs:308-362 walk pattern; Phase 3 String-not-SmolStr precedent; Phase 2 invariant test path; Phase 5 stub steps reserved in pipeline.rs:28-38; Phase 4 sideways-mode normalization at types.rs:604-650.
- [x] **3-agent review pass complete** — Spec-compliance + code-quality + test-coverage reviewers ran in parallel; 4 BLOCKERs + 8 IMPORTANTs + 5 missing/weak tests resolved in this revision pass.
- [x] **Verified against current Taffy 0.10 + Bevy 0.18 codebase** — `tree.layout()` returns `TaffyResult<&Layout>` (so `Ok(layout)` match is correct); codebase uses `std::collections::HashMap` exclusively; pipeline-order trackers already wired (`tests/layout_pipeline_order.rs:56-71`); `WritingModeResolved.mode` is public (no `mode_kind()` getter needed).

## Revision history

- **2026-05-21 v1** (this file's initial commit): drafted plan with 11 tasks, 5 spec § 1.5 tests.
- **2026-05-21 v2** (this revision pass): consolidated 3-agent parallel review (spec / code-quality / test-coverage). Resolved BLOCKERs: (1) sideways-mode axis grouping; (2) `Without<ContainerQuery>` filter removal in `cq_flip_check`; (3) `Style.container` field shape (`Container`, not `Option<Container>`, due to derive Bundle); (4) `cq_flip_rerun` mechanism (Approach B committed, A rejected). Added missing tests: `cq_activate_idempotent_no_redundant_inserts_in_steady_state`, `container_unit_cqi_swaps_axis_under_vertical_writing_mode`. Strengthened weak tests: cap test now asserts `LayoutTaffyComputeCount == 2`; viewport fallback now asserts a concrete 50% × 1000 = 500 px; transitive cascade now uses a real `Cqw`-driven fixture. Pipeline-order test reframed as "augment with fixture" not "replace trackers". Documented divergences from spec (warn-once granularity, dropped Entity fields, geometric vs style-bundle cascade scope) in CHANGELOG block. Fixed: `WritingModeResolved.mode` used directly (no nonexistent `mode_kind()` getter); `std::collections::HashMap` consistently; redundant `container_parent_chain` query collapsed.
- **2026-05-21 v3** (Task 10 implementer finding + reviewer mandate): the plan v2 CHANGELOG block's claim that "geometric cascade ... is handled in-frame" was found to be FALSE for multi-level cascades. The Task 10 implementer surfaced that `sync_styles`'s `Changed<>` filter is per-entity, so when an ancestor's `ResolvedLayout` changes a `Cqw`-sized intermediate is never re-translated. The Task 10 test `cq_transitive_cascade_is_one_frame_stale` was reframed as a negative assertion documenting this gap (assert C stays inactive after A resizes). The Task 10 reviewer APPROVED the reframing on condition that this v3 revision (a) rewrites the cascade-scope CHANGELOG bullet to accurately distinguish direct-ancestor (handled) from multi-level (deferred), (b) marks the verification-matrix row as "documented divergence", (c) opens a tracked follow-up. Task 11 carries this corrected language into `CHANGELOG.md` + adds `docs/plans/follow-ups.md`.
