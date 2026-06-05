# Effect-group formation (`WriteEffectGroups`) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the `WriteEffectGroups` render-prep pass that derives the `EffectGroup { reason: EffectReason }` marker on every entity that forms an off-screen compositing boundary, and removes it when none of the five formers hold.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [effect-compositor.md § 1](../specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md) (the canonical former predicate + who writes `EffectGroup`) and [component-model.md § 10](../specs/2026-06-03-buiy-render-pipeline-design/component-model.md) (the `EffectGroup` / `EffectReason` struct).
**Architecture:** `WriteEffectGroups` is a main-world render-prep system. It reads the five effect inputs (`Opacity`, `Stacking.isolation`, `Filter`, `MixBlendMode`, `BackdropFilter`), OR-s their reason bits, and inserts/removes a derived `EffectGroup` carrying that `EffectReason` flag set. It is scheduled alongside `WriteClipRects` in the render-prep window — `.after(BuiySet::Animate).before(BuiySet::Picking)` — so picking and the render extract see a current marker. This phase writes **only** the marker; per-group target sizing/allocation is a render-world Prepare pass owned by later phases (R6/R9).
**Tier/Test reality:** HEADLESS (unit/integration on CI). Every test in this plan runs under `App::new()` + `MinimalPlugins` with no wgpu adapter and no `RenderApp`. `WriteEffectGroups` touches only main-world ECS components and CPU predicate math — there is **nothing GPU here**, so there are **no `#[ignore]` GPU tests** in this phase.

---

## Cross-phase dependency (read before starting)

The five effect-input components and the `EffectGroup` / `EffectReason` output types are, per the spec, owned by the **render component-model phase** ([component-model.md §§ 6, 8, 10](../specs/2026-06-03-buiy-render-pipeline-design/component-model.md)). At the time this plan was authored **none of them exist in the codebase** (grep-confirmed: no `Opacity`, `Filter`, `MixBlendMode`, `BackdropFilter`, `EffectGroup`, or `EffectReason` anywhere). This plan is therefore written to be **self-contained**: Task 1 creates the four effect-input components, and Task 2 creates the `EffectReason` / `EffectGroup` output types, in a new `crates/buiy_core/src/render/effect.rs` module.

**If the component-model phase has already landed these types when you execute this plan**, do NOT duplicate them: skip Tasks 1–2's *type creation* (keep their tests as regression coverage, re-pointing imports at the existing definitions), and start the real work at Task 3. Verify with:

```sh
grep -rn "struct Opacity\|enum MixBlendMode\|struct EffectGroup\|struct EffectReason\|EffectReason:" crates/buiy_core/src/
```

If that prints definitions, the types exist — adapt imports and proceed from Task 3. If it prints nothing, build them here as written. Either way the `WriteEffectGroups` system (Tasks 3–8) is this phase's deliverable and is identical.

The spec uses `Length` (for `Filter`/`BackdropFilter`'s `FilterFn::Blur` and the radius/shadow fields) and `Angle` (for `FilterFn::HueRotate`). `Length` already exists in `crate::layout::types`. To keep this phase scoped to *effect-group formation* and not the full `FilterFn` surface, Tasks 1's `Filter` / `BackdropFilter` carry `Vec<FilterFn>` with a **minimal** `FilterFn` enum sufficient for the predicate (a non-empty list is all the former needs). The full `FilterFn` variant set + `Angle` stub are component-model's concern; if component-model lands first it supplies the richer enum and you import it instead.

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

## Task 1 — The four effect-input components (`Opacity`, `Filter`, `MixBlendMode`, `BackdropFilter`)

These are the predicate inputs `WriteEffectGroups` reads. `Stacking.isolation` (the fifth input) already exists in `layout::components` and needs nothing here.

**Files**
- Create: `crates/buiy_core/src/render/effect.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod effect;`)
- Modify: `crates/buiy_core/src/lib.rs` (re-export the new public types)
- Test: `crates/buiy_core/src/render/effect.rs` (inline `#[cfg(test)] mod tests`)

### Steps

- [ ] **Write the failing test.** Create `crates/buiy_core/src/render/effect.rs` with only the test module first so it fails to compile (RED = the types don't exist yet):

  ```rust
  //! Render-owned effect components and the derived `EffectGroup` marker.
  //!
  //! The four effect-input components here (`Opacity`, `Filter`,
  //! `MixBlendMode`, `BackdropFilter`) plus the layout-owned
  //! `Stacking.isolation` field are the inputs to the canonical
  //! effect-group-former predicate; `EffectGroup` / `EffectReason` (§ 2 of
  //! this module) are its output. Predicate + ownership:
  //! docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.
  //! Struct shapes: component-model.md §§ 6, 8, 10.

  use bevy::prelude::*;

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::layout::Length;

      #[test]
      fn opacity_default_is_one() {
          // CSS initial value `opacity: 1` — NOT the derived `Opacity(0.0)`.
          assert_eq!(Opacity::default().0, 1.0);
      }

      #[test]
      fn filter_default_is_empty() {
          assert!(Filter::default().0.is_empty());
      }

      #[test]
      fn backdrop_filter_default_is_empty() {
          assert!(BackdropFilter::default().0.is_empty());
      }

      #[test]
      fn mix_blend_mode_default_is_normal() {
          assert_eq!(MixBlendMode::default(), MixBlendMode::Normal);
      }

      #[test]
      fn filter_carries_a_function_list() {
          let f = Filter(vec![FilterFn::Blur(Length::px(4.0))]);
          assert_eq!(f.0.len(), 1);
      }
  }
  ```

- [ ] **Run it — expect FAIL (does not compile):**

  ```sh
  cargo test -p buiy_core --lib render::effect 2>&1 | tail -20
  ```

  Expected: compile errors `cannot find type/struct ` for `Opacity`, `Filter`, `BackdropFilter`, `MixBlendMode`, `FilterFn` — confirms RED. (Also add `pub mod effect;` to `render/mod.rs` now so the module is reachable, see next step; without it the test isn't even discovered.)

- [ ] **Minimal impl.** In `render/mod.rs` add `pub mod effect;` directly under the existing `pub mod node;` line group:

  ```rust
  pub mod effect;
  pub mod instance;
  pub mod node;
  pub mod pipeline;
  ```

  Then prepend the component definitions **above** the test module in `effect.rs`:

  ```rust
  use crate::layout::Length;

  /// Group opacity in `[0.0, 1.0]`. `1.0` (default, CSS initial) is a no-op;
  /// a value `< 1.0` forms an `EffectGroup` (`EffectReason::OPACITY`).
  /// Absent component == opaque.
  ///
  /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 6.
  #[derive(Component, Reflect, Clone, Copy, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct Opacity(pub f32);

  impl Default for Opacity {
      // Manual: a derived `Default` on a tuple `f32` gives `Opacity(0.0)`
      // (fully transparent), the wrong CSS initial value.
      fn default() -> Self {
          Opacity(1.0)
      }
  }

  /// Reserved filter-function value (minimal, predicate-sufficient subset).
  /// A non-empty `Filter`/`BackdropFilter` list is all the effect-group
  /// former needs; the full CSS `FilterFn` surface + `Angle` stub are
  /// component-model.md § 8's concern and land with the filter shader.
  ///
  /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
  #[derive(Reflect, Clone, PartialEq, Debug)]
  pub enum FilterFn {
      /// CSS `blur(<length>)`.
      Blur(Length),
  }

  /// C (reserved). Filter function list. Non-empty == forms an
  /// `EffectGroup` (`EffectReason::FILTER`) in v1; the filter shaders are
  /// deferred. Empty / absent == no filter.
  ///
  /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
  #[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct Filter(pub Vec<FilterFn>);

  /// C (reserved). Backdrop filter list — samples what is BEHIND the
  /// element. Non-empty == forms an `EffectGroup`
  /// (`EffectReason::BACKDROP_FILTER`) in v1; the backdrop-sampling shader
  /// is deferred. Buiy treats `backdrop-filter` as an effect-group former
  /// ONLY (it forms NO stacking context). Empty / absent == none.
  ///
  /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
  #[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct BackdropFilter(pub Vec<FilterFn>);

  /// C (reserved). Blend mode against the backdrop. Any value other than
  /// `Normal` forms an `EffectGroup` (`EffectReason::MIX_BLEND`) in v1; the
  /// blend shader is deferred. `Normal` (default) is a no-op.
  ///
  /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 8.
  #[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub enum MixBlendMode {
      #[default]
      Normal,
      Multiply,
      Screen,
      Overlay,
      Darken,
      Lighten,
      ColorDodge,
      ColorBurn,
      HardLight,
      SoftLight,
      Difference,
      Exclusion,
      Hue,
      Saturation,
      Color,
      Luminosity,
  }
  ```

  Re-export them from `crates/buiy_core/src/lib.rs`. Add a new `pub use` for the render effect types near the existing `pub use components::{...}` block:

  ```rust
  pub use render::effect::{BackdropFilter, Filter, FilterFn, MixBlendMode, Opacity};
  ```

  (Confirm `Length` is reachable as `crate::layout::Length` — it is re-exported there. If clippy flags an unused import in the test, gate it under `#[cfg(test)]` usage only.)

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --lib render::effect
  ```

  Expected: 5 passing tests.

- [ ] **Full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test --workspace
  ```

  ```sh
  git add -A && git commit -m "feat(render): add effect-input components (Opacity/Filter/MixBlendMode/BackdropFilter)"
  ```

---

## Task 2 — The `EffectReason` bitflags + `EffectGroup` marker

The output of the predicate. `EffectGroup` is **computed/derived** (never author-set, not serialized), so per [component-model.md § 10](../specs/2026-06-03-buiy-render-pipeline-design/component-model.md) it carries leaner derives — **no** `Reflect`, **no** `Default` (absence of the component == no group).

**Files**
- Modify: `crates/buiy_core/src/render/effect.rs`
- Modify: `crates/buiy_core/src/lib.rs` (re-export `EffectGroup`, `EffectReason`)
- Test: `crates/buiy_core/src/render/effect.rs` (extend the inline `tests` module)

### Steps

- [ ] **Write the failing test.** Add to the `tests` module in `effect.rs`:

  ```rust
  #[test]
  fn effect_reason_bits_are_disjoint_powers_of_two() {
      assert_eq!(EffectReason::OPACITY.bits(), 1);
      assert_eq!(EffectReason::ISOLATION.bits(), 2);
      assert_eq!(EffectReason::FILTER.bits(), 4);
      assert_eq!(EffectReason::BACKDROP_FILTER.bits(), 8);
      assert_eq!(EffectReason::MIX_BLEND.bits(), 16);
  }

  #[test]
  fn effect_reason_ors_multiple_reasons() {
      let r = EffectReason::OPACITY | EffectReason::ISOLATION;
      assert!(r.contains(EffectReason::OPACITY));
      assert!(r.contains(EffectReason::ISOLATION));
      assert!(!r.contains(EffectReason::FILTER));
  }

  #[test]
  fn effect_group_carries_its_reason() {
      let g = EffectGroup {
          reason: EffectReason::OPACITY,
      };
      assert_eq!(g.reason, EffectReason::OPACITY);
  }
  ```

- [ ] **Run it — expect FAIL (does not compile):**

  ```sh
  cargo test -p buiy_core --lib render::effect 2>&1 | tail -20
  ```

  Expected: `cannot find type EffectReason` / `EffectGroup`. RED confirmed.

- [ ] **Minimal impl.** Add to `effect.rs` (above the test module). The crate already depends on `bitflags = "2.11.1"` (see `Cargo.toml`); mirror the `ContainFlags` idiom in `layout/types.rs`:

  ```rust
  bitflags::bitflags! {
      /// Which effect(s) made an entity an `EffectGroup`. One entity can
      /// carry several at once (`OPACITY | ISOLATION`). The compositor reads
      /// this to choose the composite op without re-querying the five
      /// underlying components.
      ///
      /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
      #[derive(Clone, Copy, PartialEq, Eq, Debug)]
      pub struct EffectReason: u8 {
          const OPACITY         = 1;  // v1: carried
          const ISOLATION       = 2;  // v1: carried
          const FILTER          = 4;  // reserved: marks the group, no shader in v1
          const BACKDROP_FILTER = 8;  // reserved: marks the group, needs backdrop sample
          const MIX_BLEND       = 16; // reserved: marks the group, no shader in v1
      }
  }

  /// Render-owned, render-prep-DERIVED: this entity's subtree composites to
  /// its own off-screen target before its effect applies. Written by
  /// `WriteEffectGroups` (this module); NEVER author-set. NO `Reflect` /
  /// `Default` — absence of the component == no group.
  ///
  /// The canonical former predicate (any of: `Opacity < 1`,
  /// `Stacking.isolation == Isolation::Isolate`, non-empty `Filter`,
  /// non-`Normal` `MixBlendMode`, non-empty `BackdropFilter`) is owned by
  /// effect-compositor.md § 1.
  ///
  /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/component-model.md § 10.
  #[derive(Component, Clone, Copy, Debug)]
  pub struct EffectGroup {
      /// The OR of every reason that formed this group.
      pub reason: EffectReason,
  }
  ```

  Re-export from `lib.rs`, extending the Task 1 line:

  ```rust
  pub use render::effect::{
      BackdropFilter, EffectGroup, EffectReason, Filter, FilterFn, MixBlendMode, Opacity,
  };
  ```

- [ ] **Run it — expect PASS:**

  ```sh
  cargo test -p buiy_core --lib render::effect
  ```

  Expected: 8 passing tests in this module.

- [ ] **Full gate, then commit.**

  ```sh
  git add -A && git commit -m "feat(render): add EffectReason bitflags + derived EffectGroup marker"
  ```

---

## Task 3 — The former predicate as a pure function

Before wiring a system, isolate the decision into a pure, unit-testable function that maps the five inputs to an `Option<EffectReason>`. Pure CPU math, trivially headless. This is the single source of the predicate the system applies.

**Files**
- Modify: `crates/buiy_core/src/render/effect.rs`
- Test: `crates/buiy_core/src/render/effect.rs` (extend inline `tests`)

### Steps

- [ ] **Write the failing test.** Add to the `tests` module:

  ```rust
  use crate::layout::Isolation;

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
