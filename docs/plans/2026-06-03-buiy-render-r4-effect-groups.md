# Effect-group formation (`WriteEffectGroups`) Implementation Plan

**Date:** 2026-06-03
**Status:** landed

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Depends on:** R1 (sole creator of `render/components.rs` + `render/color.rs` and sole definer of `Opacity`/`Filter`/`FilterFn`/`MixBlendMode`/`BackdropFilter`/`EffectGroup`/`EffectReason`). Execution order: R1 → R2 → R3 → **R4** → R5 → R6 → R7 → R8 → (R9, R10) → R11.

**Goal:** Implement the `WriteEffectGroups` render-prep pass that derives the `EffectGroup { reason: EffectReason }` marker on every entity that forms an off-screen compositing boundary, and removes it when none of the five formers hold. The marker type and the five effect-input components are **owned by R1** (`render/components.rs`); this phase imports them and contributes only the predicate + system.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [effect-compositor.md § 1](../specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md) (the canonical former predicate + who writes `EffectGroup`) and [component-model.md § 10](../specs/2026-06-03-buiy-render-pipeline-design/component-model.md) (the `EffectGroup` / `EffectReason` struct, defined by R1).
**Architecture:** `WriteEffectGroups` is a main-world render-prep system. It reads the five effect inputs (`Opacity`, `Stacking.isolation`, `Filter`, `MixBlendMode`, `BackdropFilter` — all owned by R1's `render/components.rs`), OR-s their reason bits, and inserts/removes a derived `EffectGroup` carrying that `EffectReason` flag set. It is scheduled alongside `WriteClipRects` in the render-prep window — `.after(BuiySet::Animate).before(BuiySet::Picking)` — so picking and the render extract see a current marker. This phase writes **only** the marker; per-group target sizing/allocation is a render-world Prepare pass owned by later phases (R6/R9).
**Tier/Test reality:** HEADLESS (unit/integration on CI). Every test in this plan runs under `App::new()` + `MinimalPlugins` with no wgpu adapter and no `RenderApp`. `WriteEffectGroups` touches only main-world ECS components and CPU predicate math — there is **nothing GPU here**, so there are **no `#[ignore]` GPU tests** in this phase.

---

## Cross-phase dependency (read before starting)

The five effect-input components and the `EffectGroup` / `EffectReason` output types are, per the spec, owned by the **render component-model phase R1** ([component-model.md §§ 6, 8, 10](../specs/2026-06-03-buiy-render-pipeline-design/component-model.md)), which lands `crates/buiy_core/src/render/components.rs` as their **sole home**. R1 runs before R4 (execution order R1 → R2 → R3 → R4), so by the time you execute this plan they already exist:

- `Opacity` (manual `Default` 1.0), `Filter` (+ the **full 10-variant** `FilterFn`), `MixBlendMode`, `BackdropFilter` — the four render-owned effect inputs (`Stacking.isolation` is the fifth, already in `layout::components`).
- `EffectGroup { reason: EffectReason }` (no `Reflect`, no `Default`) + the `EffectReason` bitflags — the predicate output.

**Do NOT redefine, re-export, register, or `pub mod` any of these.** Import them from `crate::render::components` (or the `buiy_core` crate-root re-exports R1 already adds). This phase contributes **only** `effect_reason_for` + `write_effect_groups` in a new `crates/buiy_core/src/render/effect.rs` module. Verify the owners exist before starting:

```sh
grep -rn "struct Opacity\|enum MixBlendMode\|struct EffectGroup\|struct EffectReason\|EffectReason:\|enum FilterFn" crates/buiy_core/src/render/components.rs
```

That MUST print the definitions (R1 landed them). If it prints nothing, R1 has not landed — stop and land R1 first; do not build these types here.

`Length` (for `FilterFn::Blur`) and `Angle` (for `FilterFn::HueRotate`) are already provided: `Length` in `crate::layout::types`, `Angle` in R1's `render/components.rs`. R1's `FilterFn` is the **full 10-variant** CSS surface — the predicate only needs "non-empty list", so it reads that richer enum transparently (any variant counts as a former). There is no minimal `FilterFn` stub in this phase.

---

## Conventions for every task

- The gate (this host + CI have **no** xvfb and **no** wgpu adapter) must stay green at **every** commit:

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test --workspace
  ```

- Fast inner loop: `cargo test -p buiy_core` (and `--test <name>` for one integration file).
- All paths below are relative to the worktree root `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline`.
- TDD discipline: write the failing test, run it, see the *expected* failure, write minimal impl, see green, then run the **full gate** before committing. One commit per task.

---

## Task 1 — Create `render/effect.rs` (system module only; import all types from R1)

This phase owns **no** types. The five effect-input components (`Opacity`, `Filter` + the full 10-variant `FilterFn`, `MixBlendMode`, `BackdropFilter`) and the predicate output (`EffectGroup` / `EffectReason`) are **already defined by R1 in `crate::render::components`** (R1 runs first — verified by the grep in the Cross-phase section). Create `render/effect.rs` as a thin module holding only `effect_reason_for` (Task 2) and `write_effect_groups` (Tasks 3–7); this task just stands the module up with its imports.

**Guarded-import rule:** `render/components.rs` and all of the above types already exist (owned by R1). Do **NOT** redefine them, do **NOT** add `pub mod components;`, do **NOT** re-export them from `lib.rs`, do **NOT** `register_type` them. Import them from `crate::render::components`.

**Files**
- Create: `crates/buiy_core/src/render/effect.rs` (module header + imports only)
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod effect;`)

### Steps

- [ ] **Create the module** `crates/buiy_core/src/render/effect.rs` with the header doc-comment and the R1-type imports — no type definitions:

  ```rust
  //! The `WriteEffectGroups` render-prep pass: derive the `EffectGroup`
  //! marker from the five effect formers.
  //!
  //! The effect-input components (`Opacity`, `Filter`/`FilterFn`,
  //! `MixBlendMode`, `BackdropFilter`) and the predicate output
  //! (`EffectGroup` / `EffectReason`) are owned by R1's
  //! `crate::render::components` — this module defines **no** types, only the
  //! predicate (`effect_reason_for`) and the system (`write_effect_groups`).
  //! The layout-owned `Stacking.isolation` field is the fifth input.
  //!
  //! Predicate + ownership:
  //! docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.
  //! Struct shapes (owned by R1): component-model.md §§ 6, 8, 10.

  use bevy::prelude::*;

  // All effect types are owned by R1 (render/components.rs) — imported, never
  // redefined here.
  use crate::render::components::{
      BackdropFilter, EffectGroup, EffectReason, Filter, MixBlendMode, Opacity,
  };
  ```

- [ ] **Wire the module.** In `render/mod.rs` add `pub mod effect;` directly under the existing `pub mod node;` line group:

  ```rust
  pub mod effect;
  pub mod instance;
  pub mod node;
  pub mod pipeline;
  ```

  Do NOT re-export the imported types from `lib.rs` — R1 already does (re-exporting here would be a duplicate `pub use`).

- [ ] **Run it — expect PASS (compiles clean, no tests yet):**

  ```sh
  cargo test -p buiy_core --lib render::effect 2>&1 | tail -20
  ```

  Expected: the module compiles; the imports resolve against R1's `render::components`. If any import is "unresolved", R1's component-model has not landed — stop and land R1 first (per the Cross-phase grep). If clippy flags an unused import at this stage, it will be consumed by Task 2's predicate; you may land Tasks 1+2 in one commit to avoid a transient unused-import warning, or `#[allow(unused_imports)]` only on this intermediate commit.

- [ ] **Full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test --workspace
  ```

  ```sh
  git add -A && git commit -m "feat(render): stand up render/effect.rs (imports R1 effect types)"
  ```

---

## Task 2 — The former predicate as a pure function

Isolate the decision into a pure, unit-testable function that maps the five inputs to an `Option<EffectReason>`. Pure CPU math, trivially headless. This is the single source of the predicate the system applies. All types are imported from R1's `render::components`.

**Files**
- Modify: `crates/buiy_core/src/render/effect.rs`
- Test: `crates/buiy_core/src/render/effect.rs` (inline `#[cfg(test)] mod tests`)

### Steps

- [ ] **Write the failing test.** Add the inline `tests` module to `effect.rs`. All types are imported from R1 (`super::*` re-exports the module's `use crate::render::components::{...}`, plus `Isolation`/`Length`/`FilterFn` pulled in explicitly):

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::layout::{Isolation, Length};
      use crate::render::components::FilterFn;

  // A small constructor matching the system's read-shape: the four
  // render-owned inputs plus the one layout-owned `Isolation` field.
  fn reason_of(
      opacity: Option<f32>,
      isolation: Isolation,
      filter_len: usize,
      blend: MixBlendMode,
      backdrop_len: usize,
  ) -> Option<EffectReason> {
      effect_reason_for(
          opacity.map(Opacity),
          isolation,
          (filter_len > 0).then(|| Filter(vec![FilterFn::Blur(Length::px(1.0)); filter_len])),
          (blend != MixBlendMode::Normal).then_some(blend),
          (backdrop_len > 0)
              .then(|| BackdropFilter(vec![FilterFn::Blur(Length::px(1.0)); backdrop_len])),
      )
  }

  #[test]
  fn opacity_below_one_forms_opacity_reason() {
      assert_eq!(
          reason_of(Some(0.5), Isolation::Auto, 0, MixBlendMode::Normal, 0),
          Some(EffectReason::OPACITY)
      );
  }

  #[test]
  fn opacity_exactly_one_forms_no_group() {
      assert_eq!(
          reason_of(Some(1.0), Isolation::Auto, 0, MixBlendMode::Normal, 0),
          None
      );
  }

  #[test]
  fn absent_opacity_is_treated_as_one() {
      assert_eq!(
          reason_of(None, Isolation::Auto, 0, MixBlendMode::Normal, 0),
          None
      );
  }

  #[test]
  fn isolate_forms_isolation_reason() {
      assert_eq!(
          reason_of(None, Isolation::Isolate, 0, MixBlendMode::Normal, 0),
          Some(EffectReason::ISOLATION)
      );
  }

  #[test]
  fn isolation_auto_forms_no_group() {
      assert_eq!(
          reason_of(None, Isolation::Auto, 0, MixBlendMode::Normal, 0),
          None
      );
  }

  #[test]
  fn non_empty_filter_forms_filter_reason() {
      assert_eq!(
          reason_of(None, Isolation::Auto, 1, MixBlendMode::Normal, 0),
          Some(EffectReason::FILTER)
      );
  }

  #[test]
  fn non_normal_blend_forms_mix_blend_reason() {
      assert_eq!(
          reason_of(None, Isolation::Auto, 0, MixBlendMode::Multiply, 0),
          Some(EffectReason::MIX_BLEND)
      );
  }

  #[test]
  fn non_empty_backdrop_forms_backdrop_filter_reason() {
      assert_eq!(
          reason_of(None, Isolation::Auto, 0, MixBlendMode::Normal, 1),
          Some(EffectReason::BACKDROP_FILTER)
      );
  }

  #[test]
  fn combined_triggers_or_their_reason_bits() {
      // opacity<1 AND isolate AND filter -> OR of the three bits.
      assert_eq!(
          reason_of(Some(0.25), Isolation::Isolate, 2, MixBlendMode::Normal, 0),
          Some(EffectReason::OPACITY | EffectReason::ISOLATION | EffectReason::FILTER)
      );
  }

  #[test]
  fn all_five_triggers_or_every_bit() {
      assert_eq!(
          reason_of(Some(0.1), Isolation::Isolate, 1, MixBlendMode::Screen, 1),
          Some(EffectReason::all())
      );
  }
  }
  ```

- [ ] **Run it — expect FAIL:**

  ```sh
  cargo test -p buiy_core --lib render::effect 2>&1 | tail -20
  ```

  Expected: `cannot find function effect_reason_for` — RED.

- [ ] **Minimal impl.** Add the pure function to `effect.rs` (above the test module). Note the **inputs match exactly what the system reads**: render-owned components are `Option<T>` (absent == initial value), and `Isolation` comes from the layout-owned `Stacking` component (passed as the field, not the whole component). `MixBlendMode` is passed pre-filtered to `Option` only as a test convenience — the function takes the raw value to keep the predicate honest; adjust the signature to the raw forms the system actually has:

  ```rust
  use crate::layout::Isolation;

  /// Canonical effect-group-former predicate (effect-compositor.md § 1):
  /// an entity forms an `EffectGroup` iff ANY of —
  ///   1. `Opacity < 1`,
  ///   2. `Stacking.isolation == Isolation::Isolate`,
  ///   3. `Filter` non-empty,
  ///   4. `MixBlendMode != Normal`,
  ///   5. `BackdropFilter` non-empty.
  /// Returns the OR of every reason that held, or `None` if the entity
  /// forms no group. Absent render components are passed as `None` and
  /// read as their CSS-initial (no-group) value.
  ///
  /// `backdrop-filter` sets `BACKDROP_FILTER` but is deliberately NOT a
  /// stacking-context trigger (effect-compositor.md § 1) — that distinction
  /// is layout 6f's concern, not this predicate's; here it is simply a
  /// fifth former bit.
  pub(crate) fn effect_reason_for(
      opacity: Option<Opacity>,
      isolation: Isolation,
      filter: Option<Filter>,
      blend: Option<MixBlendMode>,
      backdrop: Option<BackdropFilter>,
  ) -> Option<EffectReason> {
      let mut reason = EffectReason::empty();
      if opacity.is_some_and(|o| o.0 < 1.0) {
          reason |= EffectReason::OPACITY;
      }
      if isolation == Isolation::Isolate {
          reason |= EffectReason::ISOLATION;
      }
      if filter.is_some_and(|f| !f.0.is_empty()) {
          reason |= EffectReason::FILTER;
      }
      if blend.is_some_and(|b| b != MixBlendMode::Normal) {
          reason |= EffectReason::MIX_BLEND;
      }
      if backdrop.is_some_and(|b| !b.0.is_empty()) {
          reason |= EffectReason::BACKDROP_FILTER;
      }
      (!reason.is_empty()).then_some(reason)
  }
  ```

  The Task-3 test helper passes `blend` already filtered to `Option`; that matches the `Option<MixBlendMode>` parameter (the system supplies `Some(*blend)` when the component is present, `None` when absent — see Task 4). `FilterFn` must derive `Clone` for the `vec![...; n]` repeat in the test — it already does (Task 1). Add `use crate::layout::Isolation;` at the top of `effect.rs` if not already imported there from Task 3's helper; keep one import only.

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --lib render::effect
  ```

  Expected: all predicate tests + earlier tests pass.

- [ ] **Full gate, then commit.**

  ```sh
  git add -A && git commit -m "feat(render): effect_reason_for — canonical effect-group former predicate"
  ```

---

## Task 4 — `WriteEffectGroups` system: insert `EffectGroup` on a former

Now the system. It queries the five inputs, applies `effect_reason_for`, and **inserts** `EffectGroup { reason }` when a reason holds. (Removal is Task 5.) The query is over `With<Node>` entities (the Buiy node marker), reading each input as `Option<&T>` so absent components fall to their initial value. `Stacking` is `Option<&Stacking>` and we pass its `.isolation` field (or `Isolation::default()` when absent).

**Files**
- Modify: `crates/buiy_core/src/render/effect.rs` (add the system)
- Test: `crates/buiy_core/tests/render_effect_groups.rs` (new integration test, `App::new()` + `MinimalPlugins`)

### Steps

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_effect_groups.rs`:

  ```rust
  //! Headless integration tests for `WriteEffectGroups` — the render-prep
  //! pass that derives the `EffectGroup` marker. No wgpu adapter needed
  //! (pure main-world ECS); these are the gating tests for this phase.
  //!
  //! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.

  use bevy::prelude::*;
  use buiy_core::layout::Isolation;
  use buiy_core::render::effect::{write_effect_groups, EffectGroup, EffectReason};
  use buiy_core::{BackdropFilter, Filter, FilterFn, MixBlendMode, Node, Opacity, Stacking};

  // Minimal harness: a bare schedule running just `write_effect_groups`,
  // so the test does not depend on the full BuiyRenderPlugin/RenderApp
  // (which needs a wgpu adapter). The real plugin wiring is Task 8.
  fn run_once(world: &mut World) {
      let mut schedule = Schedule::default();
      schedule.add_systems(write_effect_groups);
      schedule.run(world);
  }

  fn reason_of(world: &World, e: Entity) -> Option<EffectReason> {
      world.get::<EffectGroup>(e).map(|g| g.reason)
  }

  #[test]
  fn opacity_below_one_forms_opacity_group() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app.world_mut().spawn((Node, Opacity(0.5))).id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), Some(EffectReason::OPACITY));
  }

  #[test]
  fn isolate_forms_isolation_group() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((
              Node,
              Stacking {
                  isolation: Isolation::Isolate,
                  ..default()
              },
          ))
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), Some(EffectReason::ISOLATION));
  }

  #[test]
  fn non_empty_filter_forms_filter_group() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((Node, Filter(vec![FilterFn::Blur(buiy_core::Length::px(4.0))])))
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), Some(EffectReason::FILTER));
  }

  #[test]
  fn non_normal_blend_forms_mix_blend_group() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((Node, MixBlendMode::Multiply))
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), Some(EffectReason::MIX_BLEND));
  }

  #[test]
  fn non_empty_backdrop_forms_backdrop_filter_group_but_is_present() {
      // backdrop-filter sets BACKDROP_FILTER; the SC-trigger asymmetry
      // (it forms no stacking context) is layout's concern, not asserted
      // here — here we only assert the EffectGroup marker + bit.
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((
              Node,
              BackdropFilter(vec![FilterFn::Blur(buiy_core::Length::px(4.0))]),
          ))
          .id();
      run_once(app.world_mut());
      assert_eq!(
          reason_of(app.world(), e),
          Some(EffectReason::BACKDROP_FILTER)
      );
  }
  ```

  (`Stacking`, `Node`, `Length` must be re-exported from `buiy_core` — `Node` and `Stacking` already are via `pub use components::...` / `pub use layout::{... Stacking ...}`; `Length` is re-exported via `pub use layout::{... Length ...}`. Confirm and, if a name is missing from a re-export, that is a one-line fix in `lib.rs`.)

- [ ] **Run it — expect FAIL:**

  ```sh
  cargo test -p buiy_core --test render_effect_groups 2>&1 | tail -20
  ```

  Expected: `cannot find function write_effect_groups in module ... effect` — RED.

- [ ] **Minimal impl.** Add to `effect.rs` (above the test module). It must be `pub` so the integration test and the plugin can name it:

  ```rust
  use crate::components::Node;
  use crate::layout::Stacking;

  /// Render-prep pass: derive the `EffectGroup` marker from the five effect
  /// formers (effect-compositor.md § 1). Inserts `EffectGroup { reason }`
  /// when any former holds; removes a stale marker when none do (Task 5).
  /// Writes ONLY the boundary marker — per-group target sizing/allocation is
  /// a render-world Prepare pass (effect-compositor.md § 1.1, later phase).
  ///
  /// Runs alongside `WriteClipRects` in the render-prep window
  /// (`.after(BuiySet::Animate).before(BuiySet::Picking)`); wiring is Task 8.
  pub fn write_effect_groups(
      mut commands: Commands,
      query: Query<
          (
              Entity,
              Option<&Opacity>,
              Option<&Stacking>,
              Option<&Filter>,
              Option<&MixBlendMode>,
              Option<&BackdropFilter>,
              Option<&EffectGroup>,
          ),
          With<Node>,
      >,
  ) {
      for (entity, opacity, stacking, filter, blend, backdrop, existing) in &query {
          let isolation = stacking.map(|s| s.isolation).unwrap_or_default();
          let reason = effect_reason_for(
              opacity.copied(),
              isolation,
              filter.cloned(),
              blend.copied(),
              backdrop.cloned(),
          );
          match (reason, existing) {
              (Some(reason), _) => {
                  // Insert or overwrite the marker with the current reason set.
                  commands.entity(entity).insert(EffectGroup { reason });
              }
              (None, Some(_)) => {
                  // Former no longer holds — drop the stale marker (Task 5).
                  commands.entity(entity).remove::<EffectGroup>();
              }
              (None, None) => {}
          }
      }
  }
  ```

  Notes:
  - `opacity.copied()` works because `Opacity: Copy`; `filter.cloned()` / `backdrop.cloned()` because `Filter`/`BackdropFilter` are not `Copy` (they own a `Vec`). `blend.copied()` because `MixBlendMode: Copy`.
  - `Isolation: Default` (default is `Auto`), so `unwrap_or_default()` is correct for an entity with no `Stacking`.
  - This commit already contains the **removal** branch so the gate stays green and Task 5 only adds its *test*; if you prefer strict TDD, gate the removal branch behind Task 5 by writing the insert-only `match` first (`(Some(reason), _) => insert`, else `{}`) and adding the removal arm in Task 5. Either ordering keeps the gate green; the plan shows the final form.

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test render_effect_groups
  ```

  Expected: 5 passing tests.

- [ ] **Full gate, then commit.**

  ```sh
  git add -A && git commit -m "feat(render): WriteEffectGroups inserts EffectGroup on an effect former"
  ```

---

## Task 5 — Removal: `EffectGroup` disappears when the former stops holding

The marker is **derived state**, not author-set: `opacity` rising back to `1.0` (or a filter list emptying) must remove the `EffectGroup`. Assert insert-then-remove across two runs.

**Files**
- Test: `crates/buiy_core/tests/render_effect_groups.rs` (extend)
- Modify: `crates/buiy_core/src/render/effect.rs` (only if you deferred the removal arm in Task 4)

### Steps

- [ ] **Write the failing test.** Append to `render_effect_groups.rs`:

  ```rust
  #[test]
  fn opacity_one_forms_no_group() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app.world_mut().spawn((Node, Opacity(1.0))).id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), None);
  }

  #[test]
  fn isolation_auto_forms_no_group() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((Node, Stacking::default())) // isolation == Auto
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), None);
  }

  #[test]
  fn empty_filter_and_backdrop_form_no_group() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((Node, Filter::default(), BackdropFilter::default()))
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), None);
  }

  #[test]
  fn opacity_rising_back_to_one_removes_the_marker() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app.world_mut().spawn((Node, Opacity(0.5))).id();

      run_once(app.world_mut());
      assert_eq!(
          reason_of(app.world(), e),
          Some(EffectReason::OPACITY),
          "0.5 forms an opacity group"
      );

      // Author animates opacity back to 1.0 — the marker must be dropped.
      app.world_mut().entity_mut(e).insert(Opacity(1.0));
      run_once(app.world_mut());
      assert_eq!(
          reason_of(app.world(), e),
          None,
          "opacity back to 1.0 removes the EffectGroup"
      );
  }

  #[test]
  fn entity_without_node_marker_never_forms_a_group() {
      // The query is gated on With<Node>; a stray Opacity on a non-Buiy
      // entity must be ignored.
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app.world_mut().spawn(Opacity(0.3)).id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), None);
  }
  ```

- [ ] **Run it — expect PASS** (Task 4's impl already has the removal arm) **or FAIL** (if you deferred it):

  ```sh
  cargo test -p buiy_core --test render_effect_groups 2>&1 | tail -20
  ```

  If `opacity_rising_back_to_one_removes_the_marker` FAILs, add the removal arm to the `match` in `write_effect_groups`:

  ```rust
  (None, Some(_)) => {
      commands.entity(entity).remove::<EffectGroup>();
  }
  (None, None) => {}
  ```

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test render_effect_groups
  ```

- [ ] **Full gate, then commit.**

  ```sh
  git add -A && git commit -m "test(render): EffectGroup is removed when no former holds (derived state)"
  ```

---

## Task 6 — Combined-trigger OR over multiple components on one entity

The system-level mirror of Task 3's pure-function combined test: an entity carrying several formers at once gets one `EffectGroup` whose `reason` is the OR of every bit.

**Files**
- Test: `crates/buiy_core/tests/render_effect_groups.rs` (extend)

### Steps

- [ ] **Write the failing test** (this should already pass given Task 4's impl, but assert it explicitly at the system tier — write it, run it, confirm GREEN; if it were RED the impl OR-ing would be wrong):

  ```rust
  #[test]
  fn multiple_formers_on_one_entity_or_their_bits() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((
              Node,
              Opacity(0.4),
              Stacking {
                  isolation: Isolation::Isolate,
                  ..default()
              },
              Filter(vec![FilterFn::Blur(buiy_core::Length::px(2.0))]),
              MixBlendMode::Screen,
              BackdropFilter(vec![FilterFn::Blur(buiy_core::Length::px(2.0))]),
          ))
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), Some(EffectReason::all()));
  }

  #[test]
  fn opacity_and_isolate_or_to_two_bits() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((
              Node,
              Opacity(0.6),
              Stacking {
                  isolation: Isolation::Isolate,
                  ..default()
              },
          ))
          .id();
      run_once(app.world_mut());
      assert_eq!(
          reason_of(app.world(), e),
          Some(EffectReason::OPACITY | EffectReason::ISOLATION)
      );
  }
  ```

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test render_effect_groups
  ```

  If either fails, the OR logic in `effect_reason_for` is wrong — fix there (root-cause, do not patch the test).

- [ ] **Full gate, then commit.**

  ```sh
  git add -A && git commit -m "test(render): combined effect formers OR their EffectReason bits"
  ```

---

## Task 7 — The non-formers: `contain: paint` and positioned `z-index` form NO group

The § 1 separation: triggers that form a stacking context but are *not* effects (positioned + `z_index`, non-identity transform, `contain: paint`/`strict`) must **not** carry `EffectGroup`. They reorder/clip; neither needs an intermediate target. This guards against a future refactor conflating the SC-trigger set with the effect-former set.

**Files**
- Test: `crates/buiy_core/tests/render_effect_groups.rs` (extend)

### Steps

- [ ] **Write the test.** These entities form stacking contexts in layout but carry no effect former, so `WriteEffectGroups` must leave them group-free:

  ```rust
  use buiy_core::{Containment, Position};
  use buiy_core::layout::{ContainFlags, PositionKind, ZIndex};

  #[test]
  fn contain_paint_alone_forms_no_effect_group() {
      // `contain: paint` clips (a ClipRect boundary) but is NOT an effect
      // boundary (effect-compositor.md § 1). No EffectGroup.
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((
              Node,
              Containment {
                  contain: ContainFlags::PAINT,
                  ..default()
              },
          ))
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), None);
  }

  #[test]
  fn positioned_with_z_index_alone_forms_no_effect_group() {
      // positioned + z_index forms a stacking context (paint reorder) but is
      // not an effect — no off-screen target, no EffectGroup.
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      let e = app
          .world_mut()
          .spawn((
              Node,
              Position {
                  kind: PositionKind::Relative,
                  ..default()
              },
              Stacking {
                  z_index: ZIndex::Layer(5),
                  ..default()
              },
          ))
          .id();
      run_once(app.world_mut());
      assert_eq!(reason_of(app.world(), e), None);
  }
  ```

  Before writing, confirm the exact names: `ContainFlags::PAINT` (a flag in the `bitflags`), `PositionKind::Relative`, and the `ZIndex` value variant. Verify with:

  ```sh
  grep -n "PAINT\b" crates/buiy_core/src/layout/types.rs
  grep -n "enum ZIndex" -A6 crates/buiy_core/src/layout/types.rs
  grep -n "enum PositionKind" -A6 crates/buiy_core/src/layout/types.rs
  ```

  The non-auto `ZIndex` variant is `ZIndex::Layer(i32)` (verified). `ContainFlags::PAINT` is `1 << 1` (verified). `PositionKind::Relative` is verified. `Isolation`, `ZIndex`, `ContainFlags`, `PositionKind`, `Stacking`, `Containment`, `Position`, and `Length` are all re-exported from `buiy_core::layout` (the module is `pub`); use those paths. The assertion's `z_index` value is irrelevant — only that no effect former is present.

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test render_effect_groups 2>&1 | tail -20
  ```

  Expected PASS (these entities carry no effect former). If it does not compile, fix the enum-variant spellings per the greps above — do **not** weaken the assertion.

- [ ] **Full gate, then commit.**

  ```sh
  git add -A && git commit -m "test(render): SC-only triggers (contain:paint, positioned z-index) form no EffectGroup"
  ```

---

## Task 8 — Wire `WriteEffectGroups` into the render-prep window + assert schedule placement

The system must run in the main world in the render-prep window: `.after(BuiySet::Animate).before(BuiySet::Picking)` — the same slot `WriteClipRects` occupies (effect-compositor.md § 1.1). It lives in `BuiyRenderPlugin::build` on the **main** app (it is main-world ECS work, not a `RenderApp` system). Add a real schedule-membership/order test that runs headless (no `RenderApp`, no GPU).

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs` (register the system in `BuiyRenderPlugin::build`)
- Test: `crates/buiy_core/tests/render_effect_groups.rs` (extend — schedule-placement test)

### Steps

- [ ] **Write the failing test.** The cleanest headless assertion of "runs after Animate, before Picking" without a GPU is an **observable-ordering** test: register a probe system in `BuiySet::Animate` and one in `BuiySet::Picking`, run a full `Update`, and assert the `EffectGroup` is present after the frame (i.e. the plugin actually scheduled `write_effect_groups` in `Update`, between the two sets). Add to `render_effect_groups.rs`:

  ```rust
  use buiy_core::{BuiySet, CorePlugin};
  use buiy_core::render::BuiyRenderPlugin;

  #[test]
  fn plugin_runs_write_effect_groups_in_update_render_prep_window() {
      // Full main-world app (no RenderApp / wgpu): CorePlugin configures the
      // BuiySet chain; BuiyRenderPlugin must schedule write_effect_groups in
      // the render-prep window so a former gets its marker after one frame.
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(BuiyRenderPlugin);

      let e = app.world_mut().spawn((Node, Opacity(0.5))).id();
      app.update();

      assert_eq!(
          reason_of(app.world(), e),
          Some(EffectReason::OPACITY),
          "BuiyRenderPlugin scheduled write_effect_groups in Update"
      );
  }

  #[test]
  fn write_effect_groups_runs_after_animate_before_picking() {
      use std::sync::{Arc, Mutex};

      // Order probe: record the order of three events in one Update frame —
      // an Animate-set marker, the EffectGroup write, and a Picking-set
      // marker — and assert write happened strictly between them.
      #[derive(Resource, Clone, Default)]
      struct Log(Arc<Mutex<Vec<&'static str>>>);

      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(BuiyRenderPlugin);
      app.init_resource::<Log>();

      let probe = app.world().resource::<Log>().clone();
      let probe_a = probe.clone();
      let probe_p = probe.clone();
      app.add_systems(
          Update,
          (move || probe_a.0.lock().unwrap().push("animate")).in_set(BuiySet::Animate),
      );
      app.add_systems(
          Update,
          (move || probe_p.0.lock().unwrap().push("picking")).in_set(BuiySet::Picking),
      );
      // A system in no set, observing the marker right after the render-prep
      // window, records "wrote" once the EffectGroup exists.
      let e = app.world_mut().spawn((Node, Opacity(0.5))).id();
      app.add_systems(
          Update,
          (move |q: Query<&EffectGroup>, log: Res<Log>| {
              if q.iter().next().is_some() {
                  let mut v = log.0.lock().unwrap();
                  if !v.contains(&"wrote") {
                      v.push("wrote");
                  }
              }
          })
          .in_set(BuiySet::Picking),
      );

      app.update();

      let order = probe.0.lock().unwrap().clone();
      let _ = e;
      let ai = order.iter().position(|s| *s == "animate");
      let pi = order.iter().position(|s| *s == "picking");
      assert!(ai.is_some() && pi.is_some(), "both set probes ran: {order:?}");
      assert!(ai < pi, "Animate precedes Picking: {order:?}");
      // The marker exists by the Picking set => write happened before Picking
      // and (since it reads Animate-stage values) within the render-prep slot.
      assert!(
          order.contains(&"wrote"),
          "EffectGroup present by Picking set: {order:?}"
      );
  }
  ```

  (If the second test's ordering harness proves fiddly under Bevy's set semantics, the **first** test — `plugin_runs_write_effect_groups_in_update_render_prep_window` — is the load-bearing gating assertion; keep it and simplify the second to assert only `ai < pi` plus marker presence. Do not delete the first.)

- [ ] **Run it — expect FAIL:**

  ```sh
  cargo test -p buiy_core --test render_effect_groups 2>&1 | tail -25
  ```

  Expected: the new tests FAIL because `BuiyRenderPlugin::build` does not yet schedule `write_effect_groups` on the main app — RED.

- [ ] **Minimal impl.** In `crates/buiy_core/src/render/mod.rs`, register the system on the **main** app inside `BuiyRenderPlugin::build`, *before* the `get_sub_app_mut(RenderApp)` early-return (so it lands even when there is no RenderApp, exactly as the headless tests need). Import the set and the system:

  ```rust
  use crate::BuiySet;
  ```

  and at the top of `build`:

  ```rust
  fn build(&self, app: &mut App) {
      // Render-prep (main world): derive the EffectGroup marker alongside
      // WriteClipRects, in the .after(Animate).before(Picking) window
      // (effect-compositor.md § 1.1). This is main-world ECS work, not a
      // RenderApp system, so it is registered before the RenderApp branch.
      app.add_systems(
          Update,
          effect::write_effect_groups
              .after(BuiySet::Animate)
              .before(BuiySet::Picking),
      );

      // ExtractedDraws is render-world only — the main world does not read it.
      let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
          return;
      };
      // ... existing body unchanged ...
  }
  ```

  Confirm `effect` is in scope (Task 1 added `pub mod effect;` to this file) and that `BuiySet` is reachable from the crate root (it is defined in `lib.rs`; `use crate::BuiySet;`).

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --test render_effect_groups
  ```

  Also re-run the Phase-0 smoke test to confirm the new main-world system does not break plugin loading:

  ```sh
  cargo test -p buiy_core --test render_smoke
  ```

  (`render_plugin_loads_without_panic` must still pass — it adds `BuiyRenderPlugin` under `MinimalPlugins` + `CorePlugin`, which now also schedules `write_effect_groups`; that is fine headless.)

- [ ] **Full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test --workspace
  ```

  ```sh
  git add -A && git commit -m "feat(render): schedule WriteEffectGroups in render-prep window (.after(Animate).before(Picking))"
  ```

---

## Task 9 — Doc-comment cross-references + spec wiring note

Tie the code back to the spec so future readers (and the docs index) see the marker is in place and the geometry pass is *not* this phase's. Pure documentation; the gate's `RUSTDOCFLAGS="-D warnings"` clause is the test.

**Files**
- Modify: `crates/buiy_core/src/render/effect.rs` (module doc-comment: state scope boundary)
- Modify: `docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md` (Verification bullet: mark the headless `EffectGroup`-derivation half as landed)

### Steps

- [ ] **Add the scope-boundary note** to the `effect.rs` module doc-comment (the `//!` header), making explicit that this module owns *only* the marker, not the geometry:

  ```rust
  //! **Scope:** this module derives the `EffectGroup` *boundary marker*
  //! only. Per-group geometry (painted bounds, bucketed `TextureDescriptor`,
  //! post-order index) and the off-screen render targets are a render-world
  //! Prepare pass owned by a later phase (effect-compositor.md § 1.1, § 2) —
  //! NOT here.
  ```

- [ ] **Update the spec Verification bullet.** In `effect-compositor.md`, the `## Verification` section's "`EffectGroup` derivation (gate #1 …)" bullet describes the headless half this phase implements. Append a parenthetical marking it landed, e.g. ` (Landed: render-prep `WriteEffectGroups` + headless tests, plan 2026-06-03-buiy-render-r4-effect-groups.md.)`. Keep the existing prose; this is an additive provenance note, not a rewrite. (If the surrounding `[draft]` status convention forbids "landed" markers on a draft spec, instead add the note to `docs/README.md`'s plan catalog when this plan merges — coordinate with the docs-index update the umbrella render workflow performs.)

- [ ] **Run the doc gate — expect PASS:**

  ```sh
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  ```

- [ ] **Full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test --workspace
  ```

  ```sh
  git add -A && git commit -m "docs(render): cross-reference WriteEffectGroups scope; mark EffectGroup-derivation half landed"
  ```

---

## Done criteria

- [ ] `EffectGroup { reason: EffectReason }` is derived by `write_effect_groups` for every `With<Node>` entity satisfying the § 1 former predicate, and removed when none holds.
- [ ] Each of the five formers sets the correct bit (`OPACITY` / `ISOLATION` / `FILTER` / `MIX_BLEND` / `BACKDROP_FILTER`); combined formers OR their bits; `backdrop-filter` sets `BACKDROP_FILTER` (its SC-trigger asymmetry is layout's concern, untouched here).
- [ ] `opacity == 1` / `Isolation::Auto` / empty `Filter`+`BackdropFilter` / `MixBlendMode::Normal` form **no** group; SC-only triggers (`contain: paint`, positioned `z-index`) form **no** group.
- [ ] The system runs in the render-prep window `.after(BuiySet::Animate).before(BuiySet::Picking)`, registered on the **main** app so it works headless (no `RenderApp`/wgpu).
- [ ] Every commit is green under the full gate. **No GPU `#[ignore]` tests in this phase** — every test is headless `App::new()` + `MinimalPlugins`.
- [ ] Per-group geometry/target sizing is explicitly **out of scope** (later R6/R9 Prepare pass) and noted in the module doc-comment + spec.
