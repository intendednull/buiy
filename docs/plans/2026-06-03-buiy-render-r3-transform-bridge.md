# Transform / GlobalTransform Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fold `ResolvedLayout.position` − accumulated ancestor `ScrollOffset`, composed with `ResolvedTransform.matrix`, into a single Bevy `Transform` via one top-down render-prep writer (`write_buiy_transform`), then schedule Bevy's three propagation systems in `Update` so `GlobalTransform` is final before picking and extract.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes [clip-and-transform.md § B](../specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md) (the bridge, the propagation scheduling, the coordinate contract, perspective/backface consumption), README pillar 5, and unblocks follow-up "`Transform`/`GlobalTransform` bridge".
**Architecture:** A new render-prep system `write_buiy_transform` is the *single* authority over each laid-out entity's `Transform.translation`. It is a top-down `Children` walk that carries the running ancestor-scroll sum, composes `base = position − acc` lifted to a translation `Mat4`, then `base * ResolvedTransform.matrix`, and writes one `Transform` per entity (seeded by a `ScrollDirty` resource unioning `Changed<ResolvedLayout>`, `Changed<ResolvedTransform>`, and `Changed<ScrollOffset>`-on-a-scroll-ancestor). Bevy's `mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms` are then chained in `Update` after the writer and before `BuiySet::Picking`. The y-down→y-up flip and logical→physical scale stay out of the bridge (they live in the GPU view uniform); the whole bridge stays in logical-px, y-down, window-relative space. `Preserve3d`/perspective is affine-incompatible and C-tier deferred (noted, not built); `backface_visibility` is consumed only as a per-primitive render flag (render reads `UiTransform` directly — no new component).
**Tier/Test reality:** HEADLESS. Every gating test runs on this host + CI with `App::new() + MinimalPlugins + TransformPlugin + CorePlugin + LayoutPlugin` — no wgpu adapter, no `RenderApp`. There is **no** GPU-only `#[ignore]` test in this phase: the bridge is pure CPU math + ECS schedule membership/order, and propagation is exercised by adding `TransformPlugin` (which populates the three propagation systems) to the headless harness. The one GPU-dependent assertion the spec names (gate #2 golden coordinate-contract image) belongs to the render-graph/e2e phase, not here; this plan only pins the *CPU* coordinate contract (translation stays y-down, no flip in the bridge).

---

## Conventions for every task

- **The gate** (run after each task's impl step; must stay green before commit):
  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test --workspace
  ```
  This host and CI have **no xvfb and no wgpu adapter**. Do not add any test that constructs a `RenderApp` or `bevy::render::RenderPlugin` without `#[ignore]`. Every test in this plan is adapter-free.
- **Crate placement.** The bridge is render-prep but reads only layout output + writes a Bevy `Transform`. Per the spec it is a *main-world* system. Put the system and its `ScrollDirty` resource in a **new module** `crates/buiy_core/src/render/bridge.rs` (a sibling of the existing `render/node.rs` etc.), re-exported from `render/mod.rs`. The scheduling (adding the systems to `Update`) goes in `CorePlugin` (`crates/buiy_core/src/lib.rs`) because the bridge runs in the main world `Update` schedule alongside `BuiySet`, **not** in the `RenderApp` branch of `BuiyRenderPlugin` (that branch early-returns headless, which would make the bridge dead on CI — wrong).
- **Coordinate contract (pinned, do not flip).** `Transform.translation = (position.x − acc.x, position.y − acc.y, 0.0)` in logical px, y-down. No y-flip, no scale. Asserted by tests.
- **Type/name fidelity.** Use exactly: `write_buiy_transform`, `ScrollDirty`, `ResolvedLayout`, `ResolvedTransform`, `ScrollOffset`, `UiTransform`, `BackfaceVisibility`, `Overflow::is_scroll_container`, `bevy_transform::systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms}`, `BuiySet::Animate`, `BuiySet::Picking`.

---

## Task 0 — Add a `BuiySet::RenderPrep` system set between `Animate` and `Picking`

**Why first:** the spec pins the bridge + propagation `.after(BuiySet::Animate).before(BuiySet::Picking)`. `BuiySet` today has no render-prep slot; the bridge and the three propagation systems need a stable home that ordering tests can assert. We add one new internal set variant `RenderPrep` to `BuiySet` (the spec § 5.1 says "adds no new *top-level* variant" for the render *graph*; this is a fine-grained ordering anchor inside the existing chain — name it `RenderPrep` and chain it Animate → RenderPrep → Picking). This keeps the bridge's home explicit and test-pinnable.

**Files**
- Modify: `crates/buiy_core/src/lib.rs` (add `RenderPrep` to `BuiySet`, chain it)
- Modify: `crates/buiy/src/lib.rs` (no change needed unless re-export breaks — verify only)
- Test: `crates/buiy_core/tests/system_set_order.rs` (extend)

Steps:

- [ ] **Write the failing test.** Append to `crates/buiy_core/tests/system_set_order.rs`:
  ```rust
  #[test]
  fn render_prep_runs_between_animate_and_picking() {
      // The render-prep stage (write_buiy_transform + the three propagation
      // systems) is pinned `.after(BuiySet::Animate).before(BuiySet::Picking)`
      // per clip-and-transform.md § B.2.1 / architecture.md § 5.2.
      let idx = set_indices(&[BuiySet::Animate, BuiySet::RenderPrep, BuiySet::Picking]);
      assert!(idx[0] < idx[1], "Animate must precede RenderPrep");
      assert!(idx[1] < idx[2], "RenderPrep must precede Picking");
  }
  ```
- [ ] **Run it — expect FAIL (does not compile):**
  ```sh
  cargo test -p buiy_core --test system_set_order render_prep_runs_between_animate_and_picking
  ```
  Expected: compile error `no variant named RenderPrep found for enum BuiySet`.
- [ ] **Minimal impl.** In `crates/buiy_core/src/lib.rs`, add the variant and chain it. Edit the enum:
  ```rust
  /// Top-level system sets for Buiy. Order: Layout → Style → Input → Animate
  /// → RenderPrep → Picking → A11yUpdate → Render.
  #[derive(SystemSet, Debug, Clone, Copy, Eq, PartialEq, Hash)]
  pub enum BuiySet {
      Layout,
      Style,
      Input,
      Animate,
      /// Render-prep stage (clip-and-transform.md § B.2.1): the `Transform`
      /// compose bridge + Bevy's propagation chain run here so
      /// `GlobalTransform` is final before `Picking` and `ExtractSchedule`.
      RenderPrep,
      Picking,
      A11yUpdate,
      Render,
  }
  ```
  And in `CorePlugin::build`'s `configure_sets`, insert `BuiySet::RenderPrep` between `Animate` and `Picking`:
  ```rust
              .configure_sets(
                  Update,
                  (
                      BuiySet::Layout,
                      BuiySet::Style,
                      BuiySet::Input,
                      BuiySet::Animate,
                      BuiySet::RenderPrep,
                      BuiySet::Picking,
                      BuiySet::A11yUpdate,
                      BuiySet::Render,
                  )
                      .chain(),
              );
  ```
- [ ] **Run pass:**
  ```sh
  cargo test -p buiy_core --test system_set_order
  ```
  Both `buiy_sets_run_in_documented_order` and the new test pass.
- [ ] **Run the full gate.** Fix the doc comment on `BuiySet` to list all 8 variants so `RUSTDOCFLAGS="-D warnings"` stays clean.
- [ ] **Commit:** `feat(core): add BuiySet::RenderPrep between Animate and Picking`

---

## Task 1 — `ScrollDirty` resource: the top-down re-run seed set

**What:** A render-prep resource `ScrollDirty(HashSet<Entity>)` — the single re-run trigger feeding the single writer (spec § B.2/§ B.3). It is the render-prep analogue of layout's `ContainerSizeDirty` (`crates/buiy_core/src/layout/systems.rs`). This task introduces the type + a seeding system `seed_scroll_dirty` that populates it each frame and an `Default`/empty steady state. The walk (Task 2) drains it.

The seed rule (spec § B.2 last bullet, § B.3): `ScrollDirty` ⊇ { every entity whose `ResolvedLayout` changed } ∪ { every entity whose `ResolvedTransform` changed } ∪ { every entity in the subtree of a scroll-container whose `ScrollOffset` changed }. For this task seed only the **direct** `Changed<ResolvedLayout>` / `Changed<ResolvedTransform>` entities and the scroll-container entities whose `ScrollOffset` changed; the *subtree expansion* of a scroll-offset change is folded into the Task 2 walk (the walk already descends `Children` from any seeded ancestor, re-translating the whole subtree), so seeding the scroll container itself is sufficient.

**Files**
- Create: `crates/buiy_core/src/render/bridge.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod bridge;` + re-export `ScrollDirty`)
- Test: `crates/buiy_core/tests/render_transform_bridge.rs` (new)

Steps:

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_transform_bridge.rs`:
  ```rust
  //! Phase R3 — the Transform / GlobalTransform bridge (clip-and-transform.md § B).
  //! All tests are HEADLESS: MinimalPlugins + TransformPlugin + CorePlugin +
  //! LayoutPlugin, no wgpu adapter, no RenderApp.

  use bevy::prelude::*;
  use buiy_core::{
      CorePlugin, Node, ResolvedLayout,
      layout::{LayoutPlugin, Length, ScrollOffset, Sizing, Style},
      render::bridge::ScrollDirty,
  };

  /// HEADLESS harness for the bridge: TransformPlugin populates the three
  /// propagation systems CorePlugin chains in Update (§ B.2.1), so reading
  /// GlobalTransform after `update()` is meaningful.
  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::transform::TransformPlugin);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);
      app
  }

  #[test]
  fn scroll_dirty_is_empty_in_steady_state() {
      let mut app = app();
      app.world_mut().spawn((
          Node,
          Style {
              box_model: buiy_core::BoxModel {
                  width: Sizing::Length(Length::Px(50.0)),
                  height: Sizing::Length(Length::Px(50.0)),
                  ..Default::default()
              },
              ..Default::default()
          },
      ));
      // Frame 1: spawn frame — everything Changed, so ScrollDirty is non-empty.
      app.update();
      // Frame 2: nothing mutated — ScrollDirty must be empty (the seed observed
      // no Changed inputs).
      app.update();
      let dirty = app.world().resource::<ScrollDirty>();
      assert!(
          dirty.0.is_empty(),
          "steady-state frame must leave ScrollDirty empty, got {:?}",
          dirty.0
      );
  }
  ```
- [ ] **Run it — expect FAIL (does not compile):**
  ```sh
  cargo test -p buiy_core --test render_transform_bridge
  ```
  Expected: `unresolved import buiy_core::render::bridge::ScrollDirty`.
- [ ] **Minimal impl.** Create `crates/buiy_core/src/render/bridge.rs`:
  ```rust
  //! The `Transform` / `GlobalTransform` bridge (render-prep, main world).
  //!
  //! `write_buiy_transform` is the SOLE writer of a laid-out entity's Bevy
  //! `Transform.translation`. It folds `ResolvedLayout.position`, the
  //! accumulated ancestor `ScrollOffset`, and the optional composed
  //! `ResolvedTransform.matrix` into one `Transform`; Bevy's propagation
  //! chain (scheduled by `CorePlugin` in `Update`) then owns the resulting
  //! `GlobalTransform`. Render reads `GlobalTransform`, never `ResolvedLayout`.
  //!
  //! The bridge stays in logical-px, y-down, window-relative space: the
  //! y-down → y-up flip and the logical → physical scale live in the GPU
  //! view uniform, never here (clip-and-transform.md § B.4). A flip in the
  //! bridge would double-apply against the view uniform.
  //!
  //! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § B.

  use crate::components::{ResolvedLayout, ResolvedTransform};
  use crate::layout::{Overflow, ScrollOffset};
  use bevy::prelude::*;
  use std::collections::HashSet;

  /// Render-prep re-run seed for `write_buiy_transform` — the single trigger
  /// feeding the single writer (§ B.2/§ B.3). Seeded each frame by
  /// `seed_scroll_dirty` from the union of `Changed<ResolvedLayout>`,
  /// `Changed<ResolvedTransform>`, and `Changed<ScrollOffset>` on a
  /// scroll-container, then drained top-down by the walk. Empty in steady
  /// state — the render-prep analogue of layout's `ContainerSizeDirty`.
  #[derive(Resource, Default, Debug)]
  pub struct ScrollDirty(pub HashSet<Entity>);

  /// Render-prep — repopulate `ScrollDirty` for this frame. Cleared, then
  /// seeded with every entity whose `ResolvedLayout` or `ResolvedTransform`
  /// changed, plus every scroll-container whose `ScrollOffset` changed (the
  /// walk expands that container's subtree). Steady-state frames leave it
  /// empty.
  pub fn seed_scroll_dirty(
      mut dirty: ResMut<ScrollDirty>,
      changed_layout: Query<Entity, Changed<ResolvedLayout>>,
      changed_transform: Query<Entity, Changed<ResolvedTransform>>,
      changed_scroll: Query<(Entity, &Overflow), Changed<ScrollOffset>>,
  ) {
      dirty.0.clear();
      dirty.0.extend(changed_layout.iter());
      dirty.0.extend(changed_transform.iter());
      for (e, overflow) in changed_scroll.iter() {
          if overflow.is_scroll_container() {
              dirty.0.insert(e);
          }
      }
  }
  ```
  In `crates/buiy_core/src/render/mod.rs`, add after the existing `pub mod` lines:
  ```rust
  pub mod bridge;
  ```
- [ ] **Run pass:**
  ```sh
  cargo test -p buiy_core --test render_transform_bridge
  ```
  Note: `seed_scroll_dirty` is not yet scheduled, so `ScrollDirty` will not be initialized as a resource unless we add it. **This test will still fail** with "Resource ScrollDirty does not exist" — that is expected; wire the resource + system in Task 3 (scheduling). To keep this task's gate green, also do the scheduling-resource init here: in `CorePlugin::build` add `app.init_resource::<crate::render::bridge::ScrollDirty>();` and schedule `seed_scroll_dirty.in_set(BuiySet::RenderPrep)`. (Full chain wiring is Task 3; this minimal wiring makes `ScrollDirty` exist + steady-state-empty.)
  Concretely add to `CorePlugin::build`, after `configure_sets`:
  ```rust
          app.init_resource::<crate::render::bridge::ScrollDirty>();
          app.add_systems(
              Update,
              crate::render::bridge::seed_scroll_dirty.in_set(BuiySet::RenderPrep),
          );
  ```
  Re-run — passes (`ScrollDirty` empty on frame 2).
- [ ] **Run the full gate.** `seed_scroll_dirty` is `pub`; document it (done above) so rustdoc `-D warnings` is clean.
- [ ] **Commit:** `feat(render): add ScrollDirty seed resource for the transform bridge`

---

## Task 2 — `write_buiy_transform`: the top-down single-writer compose

**What:** The core bridge system. Top-down `Children` walk from each root (`With<Node>, Without<ChildOf>`), carrying `acc: Vec2` (running ancestor scroll sum, starts zero). Per entity it composes `base = Mat4::from_translation((position − acc).extend(0.0))`, then `composed = base * matrix` if `ResolvedTransform` present else `base`, and inserts `Transform::from_matrix(composed)` (change-gated). The walk descends a subtree iff it is in `ScrollDirty` (or an ancestor was) — mirroring `inherit_writing_mode` / `transform_composition` top-down shapes. The child's `acc` is the parent's `acc` plus this node's own `ScrollOffset` iff this node is a scroll container.

Insert `Transform` via `Commands` (not `Query<&mut Transform>`) because `Node` does not carry a `Transform` yet, and `Transform` `#[require(GlobalTransform, TransformTreeChanged)]` — inserting it pulls the companions in for free, so propagation has everything it needs.

**Files**
- Modify: `crates/buiy_core/src/render/bridge.rs`
- Test: `crates/buiy_core/tests/render_transform_bridge.rs`

Steps:

- [ ] **Write the failing tests.** Append to `crates/buiy_core/tests/render_transform_bridge.rs`:
  ```rust
  use buiy_core::ResolvedTransform;

  #[test]
  fn plain_layout_translation_equals_position_y_down_no_flip() {
      // A single 50×50 box at the root resolves to position (0,0); its
      // Transform.translation is (0,0,0) — y-down, NO flip (the flip is in
      // the GPU view uniform, § B.4).
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((
              Node,
              Style {
                  box_model: buiy_core::BoxModel {
                      width: Sizing::Length(Length::Px(50.0)),
                      height: Sizing::Length(Length::Px(50.0)),
                      ..Default::default()
                  },
                  ..Default::default()
              },
          ))
          .id();
      app.update();
      let pos = app.world().get::<ResolvedLayout>(e).unwrap().position;
      let t = app.world().get::<Transform>(e).expect("bridge wrote Transform");
      assert_eq!(t.translation, pos.extend(0.0));
      assert_eq!(t.translation.z, 0.0);
  }

  #[test]
  fn transform_folds_resolved_transform_matrix_into_translation_path() {
      // A translate(15,25) box: ResolvedTransform.matrix is a pure
      // translation, so the composed Transform equals
      // from_translation(position) * matrix. With position (0,0) the
      // resulting translation is (15,25,0).
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((Node, Style::default().translate_px(15.0, 25.0)))
          .id();
      app.update();
      let pos = app.world().get::<ResolvedLayout>(e).unwrap().position;
      let rt = app.world().get::<ResolvedTransform>(e).unwrap().matrix;
      let t = app.world().get::<Transform>(e).unwrap();
      let expected =
          Transform::from_matrix(Mat4::from_translation(pos.extend(0.0)) * rt);
      assert_eq!(t.translation, expected.translation);
      // translation component equals position + (15,25)
      assert!((t.translation.x - (pos.x + 15.0)).abs() < 1e-4);
      assert!((t.translation.y - (pos.y + 25.0)).abs() < 1e-4);
  }
  ```
  (`Style::default().translate_px(..)` is the builder used by `tests/layout_transforms.rs`; reuse it.)
- [ ] **Run — expect FAIL:** `cargo test -p buiy_core --test render_transform_bridge` — `Transform` is absent (`bridge wrote Transform` panics) because `write_buiy_transform` does not exist yet / is not scheduled.
- [ ] **Minimal impl.** Append to `crates/buiy_core/src/render/bridge.rs`:
  ```rust
  /// Render-prep — the SOLE writer of a laid-out entity's Bevy `Transform`.
  /// Top-down `Children` walk: per entity compose
  ///   `base = from_translation(position − accumulated_ancestor_scroll)`
  /// then `base * ResolvedTransform.matrix` (identity fast-path when the
  /// optional `ResolvedTransform` is absent), into ONE `Transform`. The walk
  /// descends only into `ScrollDirty`-seeded subtrees (§ B.2/§ B.3), so a
  /// steady-state frame visits no entities. No y-flip / scale here (§ B.4).
  ///
  /// Inserting `Transform` (via `Commands`) pulls in its required
  /// `GlobalTransform` + `TransformTreeChanged` companions, which Bevy's
  /// propagation chain (scheduled by `CorePlugin`) then consumes.
  ///
  /// Spec: clip-and-transform.md § B.2 / § B.3.
  #[allow(clippy::type_complexity)]
  pub fn write_buiy_transform(
      mut commands: Commands,
      roots: Query<Entity, (With<crate::components::Node>, Without<ChildOf>)>,
      layout: Query<(
          &ResolvedLayout,
          Option<&ResolvedTransform>,
          Option<&ScrollOffset>,
          Option<&Overflow>,
      )>,
      children: Query<&Children>,
      existing: Query<Option<&Transform>>,
      dirty: Res<ScrollDirty>,
  ) {
      for root in roots.iter() {
          walk(root, Vec2::ZERO, false, &mut commands, &layout, &children, &existing, &dirty);
      }
  }

  /// One node of the top-down walk. `acc` is the running ancestor scroll sum;
  /// `ancestor_seeded` is true once any ancestor (or this node) is in
  /// `ScrollDirty`, forcing the whole subtree to recompose (a parent-box or
  /// ancestor-scroll change shifts every descendant translation).
  #[allow(clippy::too_many_arguments)]
  fn walk(
      entity: Entity,
      acc: Vec2,
      ancestor_seeded: bool,
      commands: &mut Commands,
      layout: &Query<(
          &ResolvedLayout,
          Option<&ResolvedTransform>,
          Option<&ScrollOffset>,
          Option<&Overflow>,
      )>,
      children: &Query<&Children>,
      existing: &Query<Option<&Transform>>,
      dirty: &ScrollDirty,
  ) {
      let seeded = ancestor_seeded || dirty.0.contains(&entity);

      // Compose this node's translation and push the child-facing scroll acc.
      let mut child_acc = acc;
      if let Ok((resolved, resolved_transform, scroll, overflow)) = layout.get(entity) {
          if seeded {
              let base = Mat4::from_translation((resolved.position - acc).extend(0.0));
              let composed = match resolved_transform {
                  Some(rt) => base * rt.matrix,
                  None => base,
              };
              let new_t = Transform::from_matrix(composed);
              // Change-gate: only write when the translation actually moved
              // (steady-state frames recompose nothing because the walk does
              // not descend unseeded subtrees, but a seeded subtree whose
              // node didn't move still skips the structural op here).
              let unchanged = existing
                  .get(entity)
                  .ok()
                  .flatten()
                  .is_some_and(|prev| prev.translation == new_t.translation && prev.rotation == new_t.rotation && prev.scale == new_t.scale);
              if !unchanged {
                  commands.entity(entity).insert(new_t);
              }
          }
          // A scroll container adds its own offset to the child-facing acc.
          if overflow.is_some_and(|o| o.is_scroll_container()) {
              if let Some(off) = scroll {
                  child_acc = acc + Vec2::new(off.x, off.y);
              }
          }
      }

      if let Ok(kids) = children.get(entity) {
          for &child in kids {
              walk(child, child_acc, seeded, commands, layout, children, existing, dirty);
          }
      }
  }
  ```
  Schedule it in `CorePlugin::build` chained **after** `seed_scroll_dirty`, in `RenderPrep` (replace the single-system add from Task 1):
  ```rust
          app.add_systems(
              Update,
              (
                  crate::render::bridge::seed_scroll_dirty,
                  crate::render::bridge::write_buiy_transform,
              )
                  .chain()
                  .in_set(BuiySet::RenderPrep),
          );
  ```
- [ ] **Run pass:** `cargo test -p buiy_core --test render_transform_bridge`. All four tests pass.
- [ ] **Run the full gate.** Resolve any clippy on the helper (the `#[allow]`s above pre-empt `too_many_arguments` / `type_complexity`).
- [ ] **Commit:** `feat(render): write_buiy_transform single-writer Transform compose`

---

## Task 3 — Schedule Bevy's propagation chain in `Update` after the bridge

**What:** Chain `bevy_transform::systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms}` (the three public systems `TransformPlugin` chains into `PostUpdate`) in `Update`, **after** `write_buiy_transform`, in `BuiySet::RenderPrep` (which is `.before(BuiySet::Picking)` by Task 0). This makes `GlobalTransform` final before picking + extract. Bevy's own `PostUpdate` chain (added by `TransformPlugin`) stays in place as the canonical late pass — we add a *distinct* `Update` instance (spec § B.2.1).

These systems live in `bevy_transform`; they are **not** in `CorePlugin`'s default dependency surface only as a plugin — but the functions are `pub` (`bevy::transform::systems::{...}`). `CorePlugin` does not add `TransformPlugin` (the harness / `BuiyPlugin` does), so the `Update` copies require `Transform`/`GlobalTransform` types to exist; they do (always present in `bevy_transform`, a core dep). The `Update` copies run regardless of whether `TransformPlugin` is present — but they only *do* something when entities carry `Transform`, which the bridge inserts.

**Files**
- Modify: `crates/buiy_core/src/lib.rs` (extend the `RenderPrep` chain)
- Test: `crates/buiy_core/tests/render_transform_bridge.rs` (GlobalTransform-final test) and `crates/buiy_core/tests/system_set_order.rs` (propagation-order pin)

Steps:

- [ ] **Write the failing tests.**
  In `crates/buiy_core/tests/render_transform_bridge.rs` append:
  ```rust
  #[test]
  fn global_transform_is_final_after_update_no_postupdate_needed() {
      // After ONE app.update(), GlobalTransform must already equal the
      // composed Transform — proving the Update propagation chain ran before
      // PostUpdate (the picking/extract window reads it in Update).
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((Node, Style::default().translate_px(10.0, 20.0)))
          .id();
      app.update();
      let t = *app.world().get::<Transform>(e).unwrap();
      let gt = app.world().get::<GlobalTransform>(e).expect("GlobalTransform present");
      // Root entity: GlobalTransform == GlobalTransform::from(Transform).
      assert_eq!(gt.translation(), t.translation);
  }

  #[test]
  fn nested_transforms_compose_through_global_transform() {
      // Parent translate(100,0), child translate(0,50): the child's
      // GlobalTransform translation is the composed (100,50,0) once Bevy's
      // propagation runs in Update.
      let mut app = app();
      let parent = app
          .world_mut()
          .spawn((Node, Style::default().translate_px(100.0, 0.0)))
          .id();
      let child = app
          .world_mut()
          .spawn((Node, Style::default().translate_px(0.0, 50.0)))
          .id();
      app.world_mut().entity_mut(parent).add_child(child);
      app.update();
      let parent_pos = app.world().get::<ResolvedLayout>(parent).unwrap().position;
      let child_local = app.world().get::<ResolvedLayout>(child).unwrap().position;
      let gt = app.world().get::<GlobalTransform>(child).unwrap();
      // Child global = parent local (pos + 100,0) composed with child local
      // (pos + 0,50). Buiy translations are pure (no rotation/scale here), so
      // the global translation is the sum of the two local translations.
      let expected_x = (parent_pos.x + 100.0) + (child_local.x + 0.0);
      let expected_y = (parent_pos.y + 0.0) + (child_local.y + 50.0);
      assert!((gt.translation().x - expected_x).abs() < 1e-3, "x: {} vs {}", gt.translation().x, expected_x);
      assert!((gt.translation().y - expected_y).abs() < 1e-3, "y: {} vs {}", gt.translation().y, expected_y);
  }
  ```
  In `crates/buiy_core/tests/system_set_order.rs` append a system-order pin. The three propagation systems are added as plain functions in `Update`; assert they sit after the bridge and before `BuiySet::Picking` by locating their nodes in the toposort. Add:
  ```rust
  #[test]
  fn propagation_chain_runs_in_update_before_picking() {
      // The three bevy_transform propagation systems are chained in Update,
      // after write_buiy_transform, before BuiySet::Picking (§ B.2.1). They
      // are a DISTINCT Update instance, not PostUpdate's TransformSystems set.
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.update();

      let schedules = app.world().resource::<Schedules>();
      let schedule = schedules.get(Update).unwrap();
      let graph = schedule.graph();
      let toposort = graph.dependency().get_toposort().unwrap();

      // Find the named systems by their type name in the toposort.
      let name_pos = |needle: &str| -> usize {
          toposort
              .iter()
              .position(|n| {
                  if let NodeId::System(_) = n {
                      graph
                          .systems
                          .iter()
                          .find(|(k, ..)| NodeId::System(*k) == *n)
                          .map(|(_, sys, ..)| sys.name().to_string().contains(needle))
                          .unwrap_or(false)
                  } else {
                      false
                  }
              })
              .unwrap_or_else(|| panic!("system containing {needle:?} not found in Update toposort"))
      };

      let bridge = name_pos("write_buiy_transform");
      let mark = name_pos("mark_dirty_trees");
      let prop = name_pos("propagate_parent_transforms");
      let sync = name_pos("sync_simple_transforms");
      let picking = set_indices(&[BuiySet::Picking])[0];

      assert!(bridge < mark, "bridge before mark_dirty_trees");
      assert!(mark < prop, "mark_dirty_trees before propagate_parent_transforms");
      assert!(prop < sync, "propagate before sync_simple_transforms");
      assert!(sync < picking, "propagation chain before BuiySet::Picking");
  }
  ```
  > Note: the exact `graph.systems` iteration API may differ slightly across Bevy 0.18 patch versions. If `graph.systems` is not directly iterable, fall back to the simpler assertion that the **RenderPrep set** precedes **Picking** (already covered by Task 0) and assert propagation ran by the `global_transform_is_final_after_update...` behavior test, then drop the toposort-name probe. The behavior test (`GlobalTransform` final after `Update`) is the load-bearing gate; the name-probe is a belt-and-suspenders pin. Keep whichever compiles cleanly under `-D warnings`.
- [ ] **Run — expect FAIL:** `cargo test -p buiy_core --test render_transform_bridge` — `GlobalTransform present` panics (no propagation scheduled in `Update`; `MinimalPlugins`+`TransformPlugin` only propagates in `PostUpdate`, and our app reads after a single `update()` which *does* run `PostUpdate`… so confirm the failure is actually the **nested** test, where without an `Update` chain the child global is stale relative to picking). If the harness's `PostUpdate` masks the failure, temporarily assert the order test first (it fails to compile/find the systems) to drive the impl.
- [ ] **Minimal impl.** In `crates/buiy_core/src/lib.rs`, import the three systems and extend the `RenderPrep` chain:
  ```rust
  use bevy::transform::systems::{
      mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms,
  };
  ```
  ```rust
          app.add_systems(
              Update,
              (
                  crate::render::bridge::seed_scroll_dirty,
                  crate::render::bridge::write_buiy_transform,
                  // Bevy's three public propagation systems, chained in
                  // dependency order (clip-and-transform.md § B.2.1). A DISTINCT
                  // Update instance — NOT PostUpdate's TransformSystems::Propagate
                  // set — so GlobalTransform is final before Picking + extract.
                  mark_dirty_trees,
                  propagate_parent_transforms,
                  sync_simple_transforms,
              )
                  .chain()
                  .in_set(BuiySet::RenderPrep),
          );
  ```
  > These three systems run in `Update` even without `TransformPlugin`; they are inert until an entity has a `Transform`. With `TransformPlugin` *also* present (the harness, `BuiyPlugin`), Bevy's `PostUpdate` chain re-propagates — accepted cost, documented in spec § B.2.1.
- [ ] **Run pass:** `cargo test -p buiy_core --test render_transform_bridge` and `cargo test -p buiy_core --test system_set_order`.
- [ ] **Run the full gate.**
- [ ] **Commit:** `feat(core): chain bevy propagation in Update after the transform bridge`

---

## Task 4 — Scroll translation folds into the bridge

**What:** Prove the spec § B.3 invariant: a scroll-container ancestor's `ScrollOffset` subtracts from its descendants' translation (content scrolls up as offset grows), via the *same* `write_buiy_transform` walk — no second writer. Also prove the scroll-only frame re-runs the translation walk but not layout (the existing layout `LayoutTaffyComputeCount` invariant is already covered by `layout_scroll_offset_no_invalidate.rs`; here assert the *translation* moves and `ResolvedLayout` does not).

**Files**
- Test: `crates/buiy_core/tests/render_transform_bridge.rs`
- (No impl change expected — the walk already folds `acc`. If a test fails, fix the walk.)

Steps:

- [ ] **Write the failing test.** Append:
  ```rust
  #[test]
  fn scroll_offset_folds_into_descendant_translation() {
      use buiy_core::{BoxModel, Overflow, OverflowMode};
      let mut app = app();
      // Scroll container (overflow-y: scroll) with one child.
      let container = app
          .world_mut()
          .spawn((
              Node,
              Style {
                  box_model: BoxModel {
                      width: Sizing::Length(Length::Px(100.0)),
                      height: Sizing::Length(Length::Px(100.0)),
                      ..Default::default()
                  },
                  overflow: Overflow { y: OverflowMode::Scroll, ..Default::default() },
                  ..Default::default()
              },
              ScrollOffset::default(),
          ))
          .id();
      let child = app
          .world_mut()
          .spawn((
              Node,
              Style {
                  box_model: BoxModel {
                      width: Sizing::Length(Length::Px(50.0)),
                      height: Sizing::Length(Length::Px(50.0)),
                      ..Default::default()
                  },
                  ..Default::default()
              },
          ))
          .id();
      app.world_mut().entity_mut(container).add_child(child);
      app.update();

      let gt_before = app.world().get::<GlobalTransform>(child).unwrap().translation();
      let layout_before = app.world().get::<ResolvedLayout>(child).unwrap().position;

      // Scroll down by 30px: child content moves UP by 30 (translation.y −30).
      app.world_mut().get_mut::<ScrollOffset>(container).unwrap().y = 30.0;
      app.update();

      let gt_after = app.world().get::<GlobalTransform>(child).unwrap().translation();
      let layout_after = app.world().get::<ResolvedLayout>(child).unwrap().position;

      assert!(
          (gt_after.y - (gt_before.y - 30.0)).abs() < 1e-3,
          "child translation must fold in −scroll: before {} after {}",
          gt_before.y,
          gt_after.y
      );
      // ResolvedLayout is byte-stable across scroll (§ A.4 / layout invariant).
      assert_eq!(layout_before, layout_after, "scroll must not move ResolvedLayout");
  }
  ```
- [ ] **Run — expect FAIL (or PASS).** If `write_buiy_transform`'s `acc` folding is correct it may already pass; if the scroll-only frame did not re-seed `ScrollDirty` (because only `ScrollOffset` changed and the child is unseeded), the child translation will be stale and the test fails. Run:
  ```sh
  cargo test -p buiy_core --test render_transform_bridge scroll_offset_folds_into_descendant_translation
  ```
- [ ] **Minimal impl (if failing).** The failure is the subtree-expansion gap: `seed_scroll_dirty` seeds only the *container*, and `walk`'s `ancestor_seeded` propagation covers its descendants — verify `walk` passes `seeded` (not `false`) into child recursion. It does (Task 2). If the container itself carries no `ResolvedLayout` change, confirm `seed_scroll_dirty` inserted the container (it does, via `changed_scroll` + `is_scroll_container`). No new code expected; if a gap remains, the fix is to ensure the container's own subtree is walked because the container is in `ScrollDirty`. Re-run to green.
- [ ] **Run the full gate.**
- [ ] **Commit:** `test(render): scroll offset folds into descendant translation via the bridge`

---

## Task 5 — `backface_visibility` consumption note + render reads `GlobalTransform`

**What:** Two spec-pinned consumption points (§ B.5):
1. `backface_visibility` is a **per-primitive render flag** — render reads `UiTransform.backface_visibility` directly; **no new component**. This phase does not build the SDF cull (that is the render-graph paint phase), but it pins the contract that render reads `GlobalTransform` (not `ResolvedLayout`) for spatial data and `UiTransform.backface_visibility` for the cull bit. Migrate the Phase-0 extract to read `GlobalTransform.translation()` for position instead of `ResolvedLayout.position`, proving render now consumes the bridge output.
2. Document that perspective / `transform-style: Preserve3d` are affine-incompatible (C-tier deferred): `Transform::from_matrix` decomposes to TRS and drops the projective row, so the bridge never transports perspective. Add a unit test asserting the affine part survives and (constructed manually) the projective row is dropped by `Transform::from_matrix`.

**Files**
- Modify: `crates/buiy_core/src/render/mod.rs` (`extract_buiy_draws` reads `GlobalTransform` instead of `ResolvedLayout.position`)
- Test: `crates/buiy_core/tests/render_transform_bridge.rs` (affine-survives/projective-dropped CPU test); existing `render_smoke.rs` unaffected.

Steps:

- [ ] **Write the failing tests.** Append the affine/projective CPU test (no GPU, no app):
  ```rust
  #[test]
  fn from_matrix_drops_projective_perspective_row_keeps_affine() {
      // clip-and-transform.md § B.2 / § B.5: Transform::from_matrix decomposes
      // to TRS and drops any projective row, so a perspective term in the
      // ResolvedTransform Mat4 cannot survive the bridge — perspective must
      // ride a separate render-side channel (C-tier, deferred). This test
      // pins WHY the bridge is Flat-only.
      let mut m = Mat4::from_translation(Vec3::new(7.0, 0.0, 0.0));
      // Inject a projective term in the w-row (a true perspective row).
      m.w_axis.w = 1.0;
      m.z_axis.w = -0.01; // perspective on z
      let t = Transform::from_matrix(m);
      // The affine translation survives.
      assert!((t.translation.x - 7.0).abs() < 1e-4);
      // Reconstructing the Mat4 from the decomposed TRS has NO projective row
      // (w-row is (0,0,0,1)) — the perspective term is gone.
      let round_trip = t.compute_matrix();
      assert_eq!(round_trip.z_axis.w, 0.0, "projective z-perspective dropped");
      assert_eq!(round_trip.w_axis, Vec4::new(0.0, 0.0, 0.0, 1.0));
  }
  ```
  For the extract migration, add a focused test that the extract reads the bridge output. Because `extract_buiy_draws` runs in the `RenderApp` `ExtractSchedule` (GPU-gated), do **not** test it through the render app. Instead assert the *contract* at the source: after the bridge runs, `GlobalTransform.translation().truncate()` equals what extract should use. Add:
  ```rust
  #[test]
  fn render_spatial_source_is_global_transform_not_resolved_layout() {
      // Pillar 5: render reads GlobalTransform, not ResolvedLayout, for
      // position. With a transform present, GlobalTransform.translation
      // differs from ResolvedLayout.position by exactly the transform — so a
      // consumer reading ResolvedLayout would paint in the wrong place.
      let mut app = app();
      let e = app
          .world_mut()
          .spawn((Node, Style::default().translate_px(40.0, 0.0)))
          .id();
      app.update();
      let layout = app.world().get::<ResolvedLayout>(e).unwrap().position;
      let gt = app.world().get::<GlobalTransform>(e).unwrap().translation().truncate();
      assert!((gt.x - (layout.x + 40.0)).abs() < 1e-3);
      assert_ne!(gt, layout, "render must read GlobalTransform, not ResolvedLayout");
  }
  ```
- [ ] **Run — expect FAIL/PASS:** `cargo test -p buiy_core --test render_transform_bridge`. The `from_matrix...` and `render_spatial_source...` tests should compile and pass against the implemented bridge (they assert spec-fixed math). If `from_matrix` projective handling differs, adjust the assertion to Bevy's actual decomposition (the *point* — affine survives, projective dropped — is invariant).
- [ ] **Minimal impl — migrate the extract to `GlobalTransform`.** In `crates/buiy_core/src/render/mod.rs`, change `extract_buiy_draws` to read `GlobalTransform` for position (keep `ResolvedLayout` for `size`, which the bridge does not carry):
  ```rust
  use crate::components::{Node, ResolvedLayout, Visual};
  use bevy::prelude::*; // GlobalTransform is in the prelude
  ```
  ```rust
  fn extract_buiy_draws(
      mut commands: Commands,
      main_world_q: Extract<
          Query<(&Visual, &ResolvedLayout, &GlobalTransform), With<Node>>,
      >,
      main_world_theme: Extract<Res<Theme>>,
      main_world_windows: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
  ) {
      // ... window size unchanged ...
      for (visual, layout, global) in main_world_q.iter() {
          // ... color resolution unchanged ...
          draws.draws.push(DrawData {
              // Render reads GlobalTransform for position (pillar 5), not
              // ResolvedLayout.position — the bridge folds transform + scroll
              // into GlobalTransform. Size still comes from ResolvedLayout
              // (the bridge does not carry size). Logical-px, y-down (§ B.4);
              // the y-flip lives in the view uniform, applied downstream.
              position: global.translation().truncate(),
              size: layout.size,
              color,
              radius: visual.border_radius,
          });
      }
      commands.insert_resource(draws);
  }
  ```
  > This couples the extract query to `&GlobalTransform`. Entities without a `Transform`/`GlobalTransform` (e.g. a bare `Visual` spawned without going through layout) are dropped from the extract — acceptable: render only paints laid-out entities, and the bridge inserts `Transform` (→ `GlobalTransform`) on every `Node` with `ResolvedLayout`. Note in the doc comment that this requires the bridge to have run (`BuiySet::RenderPrep` before extract, which runs after `BuiySet::Render` in `ExtractSchedule`).
- [ ] **Run pass.** `cargo test -p buiy_core` (the whole crate, including `render_smoke.rs` — its `render_plugin_loads_without_panic` still passes; the extract change is type-level only and the `#[ignore]` GPU tests are unaffected).
- [ ] **Run the full gate.** The `examples/hello_button` and `buiy_widgets` may need the bridge to run for the button to position; verify `cargo build --workspace` is clean (the extract now needs `GlobalTransform`, which `BuiyPlugin` provides via the bridge + propagation). If `hello_button` relied on the Phase-0 `ResolvedLayout`-only path, confirm it still renders by building it: `cargo build --example hello_button`.
- [ ] **Commit:** `feat(render): extract reads GlobalTransform; pin backface/perspective consumption`

---

## Task 6 — Wire the bridge into `BuiyPlugin`'s `TransformPlugin` dependency + doc the harness contract

**What:** `BuiyPlugin` (the meta-crate) composes `CorePlugin` (which schedules the bridge + the `Update` propagation copies) but the **app** must supply `TransformPlugin` so `Transform`/`GlobalTransform` propagate canonically in `PostUpdate` and the companion types are registered for reflection. `DefaultPlugins` includes `TransformPlugin`; `MinimalPlugins` does not. Document this in `BuiyPlugin`'s rustdoc (mirroring the existing `InputPlugin` requirement note) and assert that a `BuiyPlugin`-on-`MinimalPlugins+InputPlugin+TransformPlugin` app produces a final `GlobalTransform` in `Update`.

**Files**
- Modify: `crates/buiy/src/lib.rs` (rustdoc note on `BuiyPlugin`; no new plugin add — the bridge lives in `CorePlugin`)
- Test: `crates/buiy/tests/transform_bridge_integration.rs` (new, in the meta-crate)

Steps:

- [ ] **Write the failing test.** Create `crates/buiy/tests/transform_bridge_integration.rs`:
  ```rust
  //! Integration: BuiyPlugin's bridge produces a final GlobalTransform in
  //! Update (before Picking) when the app supplies TransformPlugin.
  //! HEADLESS — no DefaultPlugins, no RenderApp.

  use bevy::prelude::*;
  use buiy::{BuiyPlugin, Node, ResolvedLayout};
  use buiy_core::layout::Style;

  #[test]
  fn buiy_plugin_bridge_finalizes_global_transform_in_update() {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::input::InputPlugin); // BuiyPlugin requires it
      app.add_plugins(bevy::transform::TransformPlugin); // supplies propagation companions
      app.add_plugins(BuiyPlugin);

      let e = app
          .world_mut()
          .spawn((Node, Style::default().translate_px(12.0, 34.0)))
          .id();
      app.update();

      let layout = app.world().get::<ResolvedLayout>(e).unwrap().position;
      let gt = app
          .world()
          .get::<GlobalTransform>(e)
          .expect("bridge + propagation produced GlobalTransform");
      assert!((gt.translation().x - (layout.x + 12.0)).abs() < 1e-3);
      assert!((gt.translation().y - (layout.y + 34.0)).abs() < 1e-3);
  }
  ```
- [ ] **Run — expect FAIL or PASS.** `cargo test -p buiy --test transform_bridge_integration`. If `BuiyPlugin` already pulls everything through `CorePlugin` it passes; if `Style::default().translate_px` is not re-exported at the `buiy` crate root, fix the import (use `buiy_core::layout::Style`). Drive any compile failure to a real assertion failure first.
- [ ] **Minimal impl — rustdoc note.** In `crates/buiy/src/lib.rs`, extend the `BuiyPlugin` doc's "Required Bevy plugins" section:
  ```rust
  /// `BuiyPlugin` also requires `bevy::transform::TransformPlugin` for the
  /// `Transform`/`GlobalTransform` bridge (clip-and-transform.md § B):
  /// `CorePlugin` schedules `write_buiy_transform` + a `Update` copy of
  /// Bevy's `mark_dirty_trees → propagate_parent_transforms →
  /// sync_simple_transforms` so `GlobalTransform` is final before
  /// `BuiySet::Picking` and extract. `DefaultPlugins` includes
  /// `TransformPlugin`; on `MinimalPlugins` add it explicitly. Without it the
  /// companion `Transform` registrations / `PostUpdate` canonical pass are
  /// absent (the `Update` copies still run, but the late canonical pass and
  /// reflection registration come from `TransformPlugin`).
  ```
- [ ] **Run pass + full gate.**
- [ ] **Commit:** `docs(buiy): document TransformPlugin requirement for the bridge; integration test`

---

## Task 7 — Docs: mark the bridge follow-up landed + update the catalog

**What:** The spec's user-level guideline requires doc updates to ship *with* the change. Reflect that the `Transform`/`GlobalTransform` bridge follow-up is realized: update `docs/plans/follow-ups.md` (mark the bridge row landed, referencing this plan + the implementing commits), and add this plan to the `docs/README.md` catalog under the render-pipeline area. Do **not** edit the spec children's design text (they describe target state); only flip status/catalog rows.

**Files**
- Modify: `docs/plans/follow-ups.md` (mark "Bevy `Transform` ownership bridge (`GlobalTransform` write)" landed)
- Modify: `docs/README.md` (add this plan to the catalog)
- Modify: `docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md` (optional: add a one-line "[landed]" marker at § B header pointing to this plan — only if the spec's existing convention uses such markers; otherwise skip)

Steps:

- [ ] **Read first.** Open `docs/plans/follow-ups.md` and `docs/README.md`; find the exact row/line for the transform-bridge follow-up and the render-pipeline plan section. Use the `organizing-buiy-docs` skill conventions (catalog grouping by area, dated `YYYY-MM-DD-<name>` filenames).
- [ ] **Edit `follow-ups.md`.** Mark the "`Transform`/`GlobalTransform` bridge" entry landed with a pointer to `docs/plans/2026-06-03-buiy-render-r3-transform-bridge.md` and the realizing commit range. Preserve the file's existing status syntax (e.g. `[landed]` / strikethrough) — match what the other landed rows use.
- [ ] **Edit `docs/README.md`.** Add a catalog line under the render-pipeline plans area:
  `- [2026-06-03-buiy-render-r3-transform-bridge](plans/2026-06-03-buiy-render-r3-transform-bridge.md) — Transform/GlobalTransform bridge (clip-and-transform § B): single-writer compose + Update propagation chain.`
- [ ] **Verify the gate** (docs-only change still runs the gate to catch broken intra-doc rustdoc links in any `//!`-referenced path):
  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    cargo test --workspace
  ```
- [ ] **Commit:** `docs(render): mark Transform/GlobalTransform bridge landed; catalog the plan`

---

## Cross-phase dependencies assumed

- **`WriteClipRects` / clip phase (clip-and-transform.md § A).** That sibling phase adds `write_clip_rects` (and `ClipRect`/`AncestorClip`) in the **same `BuiySet::RenderPrep` window**, ordered `.after(write_buiy_transform)` (spec: "after the bridge's `write_buiy_transform` (§ B.2, so `Transform` is composed)"). This plan creates `BuiySet::RenderPrep` (Task 0); the clip phase reuses it and chains its system after the bridge. No conflict — the two writers touch disjoint components (`Transform` vs `ClipRect`).
- **Effect-group phase (`WriteEffectGroups`).** Also lives in `RenderPrep`, unordered relative to the bridge (architecture § 5.2). Reuses Task 0's set.
- **Render-graph / extract phase.** Owns the GPU view uniform that carries the y-flip + logical→physical scale + projection (§ B.4). This plan deliberately keeps the bridge y-down/no-flip; the view-uniform work is a separate phase. The extract migration in Task 5 (`GlobalTransform`-sourced position) hands that phase logical-px y-down coordinates exactly as the contract requires.
- **`OffscreenAuto` / `Display::None` pruning (paint-order § 5).** The bridge walk currently does **not** prune `Display::None` subtrees (unlike `write_clip_rects`, which the spec says shares paint-order's skip predicates). For the transform bridge this is benign — composing a `Transform` for a `Display::None` entity is harmless (it is excluded from `painters_z` upstream and never painted). If a later phase requires the bridge to skip pruned subtrees for cost, it is an additive change to `walk` (read `Display`, skip-descend). Not taken here to avoid speculative coupling.
- **Perspective / `Preserve3d` render-side channel (§ B.5, C-tier).** Deferred and only *noted* (Task 5's CPU test pins *why* the bridge cannot transport perspective). No render-side perspective channel is built in this phase.
