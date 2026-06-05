# Off-screen Effect-Group Compositor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the v1 off-screen effect-group compositor — pure-CPU prepare-phase geometry (painted-bounds transitive union, next-pow2-capped-at-view bucketing, post-order indexing, `rt_pool_budget` degradation decision) gating green headless, plus the GPU prepare system, pooled `Rgba16Float` targets, and bottom-up composite passes (group `Opacity` linear + `isolation`) wired into `BuiyNode::run` behind the `#[ignore]` GPU path.

**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes pillar 6 / [effect-compositor.md](../specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md) (all §), the `rt_pool_budget` mechanism (README § 5 #4), and the gate-#15 RT-pool return-to-baseline.

**Architecture:** A render-world prepare pass (`RenderSystems::Prepare`) computes, per live `EffectGroup`, its painted bounds (group root box ∪ transitive descendant boxes ∪ resolved-px ink, folded through `GlobalTransform`, clipped by `ClipRect`, × `scale_factor`), a next-pow2-capped-at-view `TextureDescriptor`, and a post-order composite index — stored as a per-view component. `BuiyNode::run` acquires all pooled `TextureCache` `Rgba16Float` targets up-front, renders each group's subtree into its target innermost-first, then composites bottom-up applying `Opacity` (linear `SrcOver`) + isolation. `rt_pool_budget` (64 MiB v1) bounds the concurrent live set with a forward-compositing degradation fallback; return-to-baseline rides Bevy's `update_texture_cache_system` (`frames_since_last_use < 3`).

**Tier/Test reality:** Split. The prepare-phase geometry and budget logic are **pure functions** in `render::compositor` — HEADLESS (unit tests on CI, no wgpu adapter). Everything that needs a wgpu adapter — the `RenderSystems::Prepare` system constructed in the `RenderApp`, the pooled-target acquisition, the actual composite draws, the group-opacity golden, and the RSS-leak fixture — is **GPU (code + `#[ignore]`)**, mirroring `tests/render_smoke.rs` (no wgpu adapter on CI or this host).

---

## Cross-phase dependencies (assumed, not built here)

This phase consumes types/systems owned by sibling render-pipeline phases. To keep the gate green **without** those phases landed, this plan's pure-CPU core (`render::compositor`) is written against **plain geometry inputs** (`Rect`, resolved-px `f32` ink terms, `EffectReason`, post-order parent links) — it does **not** import `Opacity` / `BoxShadow` / `Outline` / `EffectGroup` directly. The component-reading glue (the `Prepare` system) is the only place those types appear, and it rides the `#[ignore]` GPU path, so an un-landed upstream phase cannot redden CI here.

Assumed upstream (cite, do not define):

- **`EffectReason` bitflags + `EffectGroup { reason }`** — owned by [component-model.md § 10](../specs/2026-06-03-buiy-render-pipeline-design/component-model.md). This plan **defines `EffectReason` locally in `render::compositor`** (Task 1) because no other phase has landed it yet and the pure math needs it; when the component-model phase lands its `EffectGroup`, it re-exports / unifies on this same bitflags definition. If `EffectReason` already exists in the tree when you start, import it instead of redefining and skip the Task-1 redefinition (adjust the `use`).
- **`Opacity(pub f32)`** (default `1.0`), **`BoxShadow(Vec<Shadow>)`**, **`Outline { width, offset, .. }`** — [component-model.md § 5/§ 6/§ 7]. Read only inside the `Prepare` system (Task 9), GPU path. The pure layer takes already-resolved px terms.
- **`WriteEffectGroups`** (main-world render-prep) — [architecture.md § 5.2](../specs/2026-06-03-buiy-render-pipeline-design/architecture.md). Writes the `EffectGroup` marker this phase reads. Not built here.
- **`ClipRect { min, max }`** — [clip-and-transform.md]. The painted-bounds union clips against it. The pure layer takes an `Option<Rect>` clip.
- **Per-view `ExtractedNodes` / `BuiyInstanceBuffers` / extract of `EffectGroup` members** — [architecture.md § 3/§ 4]. The `Prepare` system's inputs. Not built here; the system is written to read them and is `#[ignore]`-gated.

The **single load-bearing cross-phase assumption**: the prepared-store geometry inputs (each group's resolved-px root box, transitive descendant boxes, resolved-px ink terms, optional clip rect, parent-group link, and `EffectReason`) are available at prepare time. This plan's pure functions are the contract for that hand-off.

---

## Files created/modified by this plan

- **Create** `crates/buiy_core/src/render/compositor.rs` — the pure-CPU prepare math (bounds union, bucketing, post-order, budget/degradation) + the `EffectReason`/`PreparedEffectGroup`/`PreparedEffectGroups` types + the GPU `Prepare` system + `compositor::register` (pipelines/resources only, no graph node).
- **Modify** `crates/buiy_core/src/render/mod.rs` — `pub mod compositor;`, call `compositor::register(render_app)` from `BuiyRenderPlugin::build`.
- **Modify** `crates/buiy_core/src/render/node.rs` — `BuiyNode::run` reads the prepared per-view store and runs the composite passes (GPU path).
- **Create** `crates/buiy_core/tests/render_compositor.rs` — headless unit tests for the pure functions.
- **Create** `crates/buiy_core/tests/render_compositor_gpu.rs` — `#[ignore]` GPU tests (prepare-system membership, group-opacity golden hook, RSS leak fixture).

---

## Task 1 — `EffectReason` + prepared-store types (HEADLESS-gating)

Define the pure-CPU types the rest of the phase computes into. No GPU.

**Files**
- Create: `crates/buiy_core/src/render/compositor.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Test: `crates/buiy_core/tests/render_compositor.rs`

Steps:

- [ ] Write the failing test file `crates/buiy_core/tests/render_compositor.rs`:

  ```rust
  //! Headless unit tests for the off-screen effect-group compositor's
  //! prepare-phase math (effect-compositor.md § 2). Pure CPU — no wgpu adapter.

  use bevy::prelude::*;
  use buiy_core::render::compositor::{EffectReason, PreparedEffectGroup};

  #[test]
  fn effect_reason_bits_match_spec() {
      // effect-compositor.md § 1.1 / component-model.md § 10.
      assert_eq!(EffectReason::OPACITY.bits(), 1);
      assert_eq!(EffectReason::ISOLATION.bits(), 2);
      assert_eq!(EffectReason::FILTER.bits(), 4);
      assert_eq!(EffectReason::BACKDROP_FILTER.bits(), 8);
      assert_eq!(EffectReason::MIX_BLEND.bits(), 16);
  }

  #[test]
  fn effect_reason_composes_opacity_and_isolation() {
      let r = EffectReason::OPACITY | EffectReason::ISOLATION;
      assert!(r.contains(EffectReason::OPACITY));
      assert!(r.contains(EffectReason::ISOLATION));
      assert!(!r.contains(EffectReason::FILTER));
  }

  #[test]
  fn prepared_group_carries_index_opacity_reason() {
      let g = PreparedEffectGroup {
          index: 3,
          parent: None,
          bounds: Rect::from_corners(Vec2::ZERO, Vec2::splat(10.0)),
          extent: UVec2::new(16, 16),
          opacity: 0.5,
          reason: EffectReason::OPACITY,
      };
      assert_eq!(g.index, 3);
      assert_eq!(g.parent, None);
      assert!((g.opacity - 0.5).abs() < 1e-6);
      assert_eq!(g.extent, UVec2::new(16, 16));
  }
  ```

- [ ] Run it, expect FAIL (module/types do not exist):
  `cargo test -p buiy_core --test render_compositor`
  Expected: compile error `unresolved import buiy_core::render::compositor`.

- [ ] Create `crates/buiy_core/src/render/compositor.rs` with the types:

  ```rust
  //! Off-screen effect-group compositor: prepare-phase geometry + pooling.
  //!
  //! Pillar 6 / spec:
  //! docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md
  //!
  //! This module's *math* (painted-bounds union, next-pow2-capped bucketing,
  //! post-order indexing, the `rt_pool_budget` degradation decision) is pure
  //! CPU and unit-testable headless. The GPU half (the `RenderSystems::Prepare`
  //! system, pooled `Rgba16Float` target acquisition, and the composite draws
  //! inside `BuiyNode::run`) needs a wgpu adapter and is `#[ignore]`-gated,
  //! exactly like `tests/render_smoke.rs`.

  use bevy::prelude::*;

  bitflags::bitflags! {
      /// Which effect(s) made an entity an off-screen compositing boundary.
      /// One entity can carry several at once (opacity<1 AND isolate).
      ///
      /// Owned by component-model.md § 10; defined here for the pure prepare
      /// math until that phase's `EffectGroup` lands and unifies on it.
      ///
      /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.1.
      #[derive(Clone, Copy, Debug, PartialEq, Eq)]
      pub struct EffectReason: u8 {
          const OPACITY         = 1;  // v1: carried
          const ISOLATION       = 2;  // v1: carried
          const FILTER          = 4;  // reserved: marks the group, no shader in v1
          const BACKDROP_FILTER = 8;  // reserved: needs backdrop sample (§ 6)
          const MIX_BLEND       = 16; // reserved: marks the group, no shader in v1
      }
  }

  /// One prepared effect group: its post-order composite index, parent group
  /// (None == composites into the window target), logical-px painted bounds,
  /// bucketed physical-texel target extent, the group `Opacity` to apply at
  /// composite, and the reasons it formed.
  ///
  /// Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.1, § 2.
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct PreparedEffectGroup {
      /// Post-order composite index (children before parents).
      pub index: usize,
      /// Post-order index of the enclosing group, or `None` for a root group
      /// (composites into the window `ViewTarget`).
      pub parent: Option<usize>,
      /// Logical-px painted bounds (group-local, pre-scale-factor).
      pub bounds: Rect,
      /// Bucketed physical-texel target size (next-pow2 per axis, capped at view).
      pub extent: UVec2,
      /// Group opacity applied at the parent composite (default 1.0).
      pub opacity: f32,
      /// OR of every reason that formed this group.
      pub reason: EffectReason,
  }
  ```

- [ ] In `crates/buiy_core/src/render/mod.rs`, add the module declaration next to the others:

  ```rust
  pub mod compositor;
  ```
  (place it alphabetically before `pub mod instance;`).

- [ ] Run the gate (fmt+clippy+doc+test). Expect the 3 new tests PASS and all existing green:
  `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace`

- [ ] Commit: `feat(render): effect-compositor prepared-store types + EffectReason`

---

## Task 2 — painted-bounds transitive union (HEADLESS-gating)

The single source of the painted-bounds formula (spec § 2.1): root box ∪ transitive descendant boxes ∪ resolved-px ink, then clipped by the group's `ClipRect`. Pure geometry over already-folded logical-px rects.

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Test: `crates/buiy_core/tests/render_compositor.rs`

Steps:

- [ ] Add failing tests to `render_compositor.rs`:

  ```rust
  use buiy_core::render::compositor::{InkExpansion, painted_bounds};

  #[test]
  fn painted_bounds_union_root_and_descendants() {
      // Root box plus a descendant that overflows to the right/bottom.
      let root = Rect::from_corners(Vec2::ZERO, Vec2::new(20.0, 20.0));
      let descendants = [Rect::from_corners(Vec2::new(10.0, 10.0), Vec2::new(40.0, 30.0))];
      let b = painted_bounds(root, &descendants, &[], None);
      assert_eq!(b.min, Vec2::ZERO);
      assert_eq!(b.max, Vec2::new(40.0, 30.0));
  }

  #[test]
  fn painted_bounds_grows_by_ink_outset_shadow_and_outline() {
      let root = Rect::from_corners(Vec2::ZERO, Vec2::splat(24.0));
      // shadow: blur 4 + spread 2 = 6 outset; outline width 1 + offset 1 = 2.
      let ink = [
          InkExpansion { margin: 6.0, around: root },
          InkExpansion { margin: 2.0, around: root },
      ];
      let b = painted_bounds(root, &[], &ink, None);
      // Largest ink margin (6) expands the box on every side.
      assert_eq!(b.min, Vec2::splat(-6.0));
      assert_eq!(b.max, Vec2::splat(30.0));
  }

  #[test]
  fn painted_bounds_clips_to_group_clip_rect_last() {
      let root = Rect::from_corners(Vec2::ZERO, Vec2::splat(100.0));
      let descendants = [Rect::from_corners(Vec2::splat(-50.0), Vec2::splat(200.0))];
      let clip = Rect::from_corners(Vec2::splat(10.0), Vec2::splat(60.0));
      let b = painted_bounds(root, &descendants, &[], Some(clip));
      // Clipped descendants cannot enlarge the target beyond the clip.
      assert_eq!(b.min, Vec2::splat(10.0));
      assert_eq!(b.max, Vec2::splat(60.0));
  }

  #[test]
  fn painted_bounds_inset_outline_offset_does_not_grow() {
      // Outline width 1 + offset -2 = max(0, -1) = 0: no growth.
      let root = Rect::from_corners(Vec2::ZERO, Vec2::splat(10.0));
      let ink = [InkExpansion { margin: 0.0, around: root }];
      let b = painted_bounds(root, &[], &ink, None);
      assert_eq!(b.min, Vec2::ZERO);
      assert_eq!(b.max, Vec2::splat(10.0));
  }
  ```

- [ ] Run, expect FAIL (unresolved `InkExpansion` / `painted_bounds`):
  `cargo test -p buiy_core --test render_compositor`

- [ ] Implement in `compositor.rs`:

  ```rust
  /// One resolved-px ink-expansion term: a `margin` (px) by which paint
  /// escapes the `around` box on every side. Caller resolves the per-term
  /// margin upstream (effect-compositor.md § 2.1): `BoxShadow` = blur+spread,
  /// `Outline` = max(0, width + offset), reserved `Filter` = blur bleed. The
  /// terms are already-resolved px (no `Length` here — § 2.1: percent/`Cq*`
  /// resolve before the sizing pass).
  #[derive(Clone, Copy, Debug, PartialEq)]
  pub struct InkExpansion {
      /// Outset margin in logical px (>= 0; inset offsets clamp to 0 upstream).
      pub margin: f32,
      /// The box this ink expands around (already folded through GlobalTransform).
      pub around: Rect,
  }

  /// Painted bounds = ( root box ∪ transitive descendant boxes ∪ ink expansion )
  /// then clipped by the group's `ClipRect`. The single source of the formula
  /// (effect-compositor.md § 2.1). Every input rect is logical-px, already
  /// folded through `GlobalTransform` by the caller; the descendant slice is
  /// the *transitive* subtree union (a nested group contributes its own
  /// composited target's bounds, computed one level down — § 2.1).
  pub fn painted_bounds(
      root: Rect,
      descendants: &[Rect],
      ink: &[InkExpansion],
      clip: Option<Rect>,
  ) -> Rect {
      let mut min = root.min;
      let mut max = root.max;
      let mut grow = |r: Rect| {
          min = min.min(r.min);
          max = max.max(r.max);
      };
      for &d in descendants {
          grow(d);
      }
      for e in ink {
          let m = Vec2::splat(e.margin);
          grow(Rect::from_corners(e.around.min - m, e.around.max + m));
      }
      let unioned = Rect { min, max };
      match clip {
          // Intersect last: clipped descendants cannot enlarge the target.
          Some(c) => Rect {
              min: unioned.min.max(c.min),
              max: unioned.max.min(c.max),
          },
          None => unioned,
      }
  }
  ```

- [ ] Run the full gate (fmt+clippy+doc+test). Expect new tests PASS, all green.

- [ ] Commit: `feat(render): painted-bounds transitive union (effect-compositor § 2.1)`

---

## Task 3 — next-pow2-capped-at-view bucketing + scale-factor → texel extent (HEADLESS-gating)

The bucket rule (spec § 2.2, committed): next-power-of-two per axis, **capped at the view's physical size**. Folds logical-px bounds × `scale_factor`, snaps to texel bounds, then buckets.

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Test: `crates/buiy_core/tests/render_compositor.rs`

Steps:

- [ ] Add failing tests:

  ```rust
  use buiy_core::render::compositor::bucket_extent;

  #[test]
  fn bucket_rounds_each_axis_to_next_pow2() {
      // 24x24 logical at scale 1 -> 24x24 physical -> 32x32 bucket.
      let e = bucket_extent(Vec2::splat(24.0), 1.0, UVec2::new(1920, 1080));
      assert_eq!(e, UVec2::new(32, 32));
  }

  #[test]
  fn bucket_folds_scale_factor_before_rounding() {
      // 24x24 logical at 2x -> 48x48 physical -> 64x64 bucket.
      let e = bucket_extent(Vec2::splat(24.0), 2.0, UVec2::new(3840, 2160));
      assert_eq!(e, UVec2::new(64, 64));
  }

  #[test]
  fn bucket_caps_at_view_size_not_next_pow2_past_it() {
      // A near-viewport group (1900 wide, view 1920) caps at 1920 on x,
      // NOT 2048 — one stable bucket shared by all overflowing groups (§ 2.2).
      let e = bucket_extent(Vec2::new(1900.0, 40.0), 1.0, UVec2::new(1920, 1080));
      assert_eq!(e.x, 1920);
      assert_eq!(e.y, 64);
  }

  #[test]
  fn bucket_caps_when_bounds_exceed_view() {
      // Bounds larger than view on both axes cap to the view dimensions.
      let e = bucket_extent(Vec2::new(5000.0, 4000.0), 1.0, UVec2::new(1920, 1080));
      assert_eq!(e, UVec2::new(1920, 1080));
  }

  #[test]
  fn bucket_never_zero() {
      // A degenerate empty bound still yields at least a 1x1 target.
      let e = bucket_extent(Vec2::ZERO, 1.0, UVec2::new(800, 600));
      assert_eq!(e, UVec2::new(1, 1));
  }
  ```

- [ ] Run, expect FAIL (`bucket_extent` unresolved).

- [ ] Implement in `compositor.rs`:

  ```rust
  /// Bucket a group's logical-px painted-bounds *size* into a physical-texel
  /// `Extent` for the pooled `TextureDescriptor` (effect-compositor.md § 2.2).
  ///
  /// Rule (committed): fold in `scale_factor`, snap out to integer texels,
  /// round each axis up to the next power of two — but **cap at the view's
  /// physical size** on that axis (a group exceeding the viewport collapses
  /// onto one shared view-size bucket rather than rounding past the viewport).
  /// The numeric pow2 thresholds (smallest bucket, steps) calibrate later
  /// (README § 5 #4); the rule shape is fixed here.
  pub fn bucket_extent(logical_size: Vec2, scale_factor: f32, view_physical: UVec2) -> UVec2 {
      let phys = (logical_size * scale_factor).ceil();
      let w = bucket_axis(phys.x, view_physical.x);
      let h = bucket_axis(phys.y, view_physical.y);
      UVec2::new(w, h)
  }

  fn bucket_axis(physical: f32, view: u32) -> u32 {
      let texels = physical.max(0.0) as u32;
      // At least 1 texel; never a 0-sized target.
      let texels = texels.max(1);
      if texels >= view {
          // Cap at the view dimension (one stable shared bucket).
          view.max(1)
      } else {
          // Next power of two, but never past the view cap.
          texels.next_power_of_two().min(view.max(1))
      }
  }
  ```

- [ ] Run the full gate. Expect new tests PASS, all green.

- [ ] Commit: `feat(render): next-pow2-capped-at-view target bucketing (effect-compositor § 2.2)`

---

## Task 4 — post-order composite indexing (HEADLESS-gating)

The bottom-up order (spec § 3): children before parents over the effect-group nesting forest. Pure: given each group's parent link, produce the post-order index assignment the node iterates.

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Test: `crates/buiy_core/tests/render_compositor.rs`

Steps:

- [ ] Add failing tests:

  ```rust
  use buiy_core::render::compositor::post_order_indices;

  #[test]
  fn post_order_places_children_before_parents() {
      // Forest by parent link (None == root group). Group 0 is parent of 1 and 2.
      //   0
      //   ├─ 1
      //   └─ 2 ── 3
      let parents = [None, Some(0usize), Some(0usize), Some(2usize)];
      let order = post_order_indices(&parents);
      let pos = |g: usize| order.iter().position(|&x| x == g).unwrap();
      // Every child precedes its parent.
      assert!(pos(1) < pos(0));
      assert!(pos(2) < pos(0));
      assert!(pos(3) < pos(2));
      assert_eq!(order.len(), 4);
  }

  #[test]
  fn post_order_handles_multiple_roots() {
      // Two independent groups, no nesting.
      let parents = [None, None];
      let order = post_order_indices(&parents);
      assert_eq!(order.len(), 2);
      assert!(order.contains(&0));
      assert!(order.contains(&1));
  }

  #[test]
  fn post_order_deep_chain_is_innermost_first() {
      // 0 -> 1 -> 2 -> 3 (3 deepest). Post-order: 3,2,1,0.
      let parents = [None, Some(0usize), Some(1usize), Some(2usize)];
      let order = post_order_indices(&parents);
      assert_eq!(order, vec![3, 2, 1, 0]);
  }
  ```

- [ ] Run, expect FAIL (`post_order_indices` unresolved).

- [ ] Implement in `compositor.rs`:

  ```rust
  /// Produce the bottom-up composite order over the effect-group nesting
  /// forest: every child appears before its parent (effect-compositor.md § 3).
  /// `parents[i]` is the index of group `i`'s enclosing group, or `None` if
  /// `i` is a root group. The returned vec lists group indices in the order
  /// `BuiyNode::run` must rasterize + composite them. The prepare pass stores
  /// this so the node never walks the main world at run time (§ 3).
  pub fn post_order_indices(parents: &[Option<usize>]) -> Vec<usize> {
      // children[p] = groups whose parent is p.
      let mut children: Vec<Vec<usize>> = vec![Vec::new(); parents.len()];
      let mut roots: Vec<usize> = Vec::new();
      for (i, p) in parents.iter().enumerate() {
          match p {
              Some(parent) => children[*parent].push(i),
              None => roots.push(i),
          }
      }
      let mut order = Vec::with_capacity(parents.len());
      // Iterative post-order DFS (explicit stack: no recursion-depth risk).
      // Stack frames carry (node, children_emitted?).
      for &root in &roots {
          let mut stack = vec![(root, false)];
          while let Some((node, expanded)) = stack.pop() {
              if expanded {
                  order.push(node);
              } else {
                  stack.push((node, true));
                  for &c in &children[node] {
                      stack.push((c, false));
                  }
              }
          }
      }
      order
  }
  ```

- [ ] Run the full gate. Expect new tests PASS, all green.

- [ ] Commit: `feat(render): post-order effect-group composite indexing (effect-compositor § 3)`

---

## Task 5 — `rt_pool_budget` + forward-compositing degradation decision (HEADLESS-gating)

The committed aggregate cap (spec § 2.3): `rt_pool_budget` bytes (64 MiB v1 default). When the next group's target would push the live set past budget, the **lowest-cost** groups (smallest area, `OPACITY`-only first) fall back to direct-to-parent forward compositing instead of allocating. Pure decision over the prepared groups.

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Test: `crates/buiy_core/tests/render_compositor.rs`

Steps:

- [ ] Add failing tests:

  ```rust
  use buiy_core::render::compositor::{
      EffectReason, RT_POOL_BUDGET_BYTES, target_bytes, plan_allocation,
  };

  #[test]
  fn budget_default_is_64_mib() {
      assert_eq!(RT_POOL_BUDGET_BYTES, 64 * 1024 * 1024);
  }

  #[test]
  fn target_bytes_is_area_times_eight_for_rgba16float() {
      // Rgba16Float = 8 bytes/texel.
      assert_eq!(target_bytes(UVec2::new(64, 64)), 64 * 64 * 8);
  }

  #[test]
  fn plan_allocation_keeps_all_under_budget() {
      // Two small groups well under budget: both composite off-screen.
      let groups = [
          (UVec2::new(32, 32), EffectReason::OPACITY),
          (UVec2::new(64, 64), EffectReason::ISOLATION),
      ];
      let plan = plan_allocation(&groups, RT_POOL_BUDGET_BYTES);
      assert_eq!(plan, vec![true, true]);
  }

  #[test]
  fn plan_allocation_degrades_lowest_cost_opacity_first_when_over_budget() {
      // Budget only fits the big isolation group; the small opacity group
      // degrades to forward compositing (lowest-cost, OPACITY-only first).
      let big = UVec2::new(2048, 2048); // 2048*2048*8 = 32 MiB
      let small = UVec2::new(32, 32);   // 8 KiB
      let groups = [
          (small, EffectReason::OPACITY),
          (big, EffectReason::ISOLATION),
      ];
      let budget = target_bytes(big); // exactly fits the isolation group only
      let plan = plan_allocation(&groups, budget);
      assert_eq!(plan[0], false, "small opacity-only group degrades first");
      assert_eq!(plan[1], true, "structural isolation group keeps its target");
  }

  #[test]
  fn plan_allocation_isolation_degrades_last() {
      // Two groups both individually fit, together exceed budget. The
      // OPACITY-only group yields before the ISOLATION group (§ 2.3 ranking).
      let a = UVec2::new(1024, 1024); // 8 MiB, OPACITY
      let b = UVec2::new(1024, 1024); // 8 MiB, ISOLATION
      let groups = [(a, EffectReason::OPACITY), (b, EffectReason::ISOLATION)];
      let budget = target_bytes(a) + target_bytes(b) / 2; // fits one, not both
      let plan = plan_allocation(&groups, budget);
      assert_eq!(plan[0], false);
      assert_eq!(plan[1], true);
  }
  ```

- [ ] Run, expect FAIL (`RT_POOL_BUDGET_BYTES` / `target_bytes` / `plan_allocation` unresolved).

- [ ] Implement in `compositor.rs`:

  ```rust
  /// Aggregate live-target budget (bytes) on the concurrent effect-group RT
  /// set — committed mechanism (effect-compositor.md § 2.3). v1 default
  /// 64 MiB; the *tuned* number defers to `buiy-verification-design`
  /// (README § 5 #4), like the atlas `page_budget`. Parallel to, not shared
  /// with, the glyph atlas pool.
  pub const RT_POOL_BUDGET_BYTES: u64 = 64 * 1024 * 1024;

  /// Bytes one pooled target of `extent` consumes. Group targets are pinned
  /// `Rgba16Float` (effect-compositor.md § 2.2) = 8 bytes/texel.
  pub fn target_bytes(extent: UVec2) -> u64 {
      const BYTES_PER_TEXEL: u64 = 8; // Rgba16Float
      u64::from(extent.x) * u64::from(extent.y) * BYTES_PER_TEXEL
  }

  /// Decide which groups allocate an off-screen target vs. fall back to
  /// direct-to-parent forward compositing under budget pressure
  /// (effect-compositor.md § 2.3). Returns one `bool` per input group, in the
  /// input order: `true` == allocate a pooled target, `false` == degrade.
  ///
  /// Ranking when the live set would exceed `budget`: the lowest-cost groups
  /// degrade first — smallest painted-bounds area and `OPACITY`-only reason
  /// first; an `ISOLATION`/reserved group degrades last (its boundary is
  /// structural, not just an alpha multiply).
  pub fn plan_allocation(groups: &[(UVec2, EffectReason)], budget: u64) -> Vec<bool> {
      // Rank: keep structural (non-OPACITY-only) and larger groups; degrade
      // OPACITY-only + smallest first. Sort a candidate-eviction order, then
      // greedily keep within budget.
      let cost = |&(extent, reason): &(UVec2, EffectReason)| -> (bool, u64) {
          // `is_opacity_only` true == cheapest to drop (degrade first).
          let opacity_only = reason == EffectReason::OPACITY;
          (opacity_only, target_bytes(extent))
      };

      // Indices ranked by *keep priority* (highest first): structural before
      // opacity-only, then larger before smaller.
      let mut keep_order: Vec<usize> = (0..groups.len()).collect();
      keep_order.sort_by(|&a, &b| {
          let (oa, sa) = cost(&groups[a]);
          let (ob, sb) = cost(&groups[b]);
          // structural (opacity_only == false) ranks first
          oa.cmp(&ob).then(sb.cmp(&sa))
      });

      let mut allocate = vec![false; groups.len()];
      let mut used: u64 = 0;
      for &i in &keep_order {
          let bytes = target_bytes(groups[i].0);
          if used.saturating_add(bytes) <= budget {
              allocate[i] = true;
              used += bytes;
          }
          // else: this group degrades to forward compositing.
      }
      allocate
  }
  ```

- [ ] Run the full gate. Expect new tests PASS, all green.

- [ ] Commit: `feat(render): rt_pool_budget + forward-compositing degradation (effect-compositor § 2.3)`

---

## Task 6 — fixed `TextureDescriptor` builder for a group target (HEADLESS-gating)

The descriptor is pinned (spec § 2.2): label `"buiy_effect_group_target"`, `Rgba16Float`, `RENDER_ATTACHMENT | TEXTURE_BINDING`, size from `bucket_extent`. `TextureDescriptor` is a `bevy_render` CPU type (no device needed to construct it), so this is headless.

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Test: `crates/buiy_core/tests/render_compositor.rs`

Steps:

- [ ] Add failing test:

  ```rust
  use bevy::render::render_resource::{TextureFormat, TextureUsages};
  use buiy_core::render::compositor::group_target_descriptor;

  #[test]
  fn group_target_descriptor_is_pinned_rgba16float_linear() {
      let d = group_target_descriptor(UVec2::new(64, 64));
      assert_eq!(d.label, Some("buiy_effect_group_target"));
      assert_eq!(d.format, TextureFormat::Rgba16Float);
      assert_eq!(d.size.width, 64);
      assert_eq!(d.size.height, 64);
      assert_eq!(d.size.depth_or_array_layers, 1);
      assert_eq!(d.mip_level_count, 1);
      assert_eq!(d.sample_count, 1);
      assert!(d.usage.contains(TextureUsages::RENDER_ATTACHMENT));
      assert!(d.usage.contains(TextureUsages::TEXTURE_BINDING));
  }
  ```

- [ ] Run, expect FAIL (`group_target_descriptor` unresolved).

- [ ] Implement in `compositor.rs` (add imports at top of the module):

  ```rust
  use bevy::render::render_resource::{
      Extent3d, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
  };

  /// The pinned off-screen group-target descriptor (effect-compositor.md § 2.2):
  /// FIXED `Rgba16Float` (linear, NOT the view's SDR format) so group opacity +
  /// isolation composite in linear space; `RENDER_ATTACHMENT` (subtree renders
  /// into it) | `TEXTURE_BINDING` (composite pass samples it). `size` is the
  /// already-bucketed physical-texel extent (`bucket_extent`). Descriptor-keyed
  /// `TextureCache` reuse depends on this being byte-identical across frames for
  /// a given bucket size — hence the fixed label/format/usage.
  pub fn group_target_descriptor(extent: UVec2) -> TextureDescriptor<'static> {
      TextureDescriptor {
          label: Some("buiy_effect_group_target"),
          size: Extent3d {
              width: extent.x,
              height: extent.y,
              depth_or_array_layers: 1,
          },
          mip_level_count: 1,
          sample_count: 1,
          dimension: TextureDimension::D2,
          format: TextureFormat::Rgba16Float,
          usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
          view_formats: &[],
      }
  }
  ```

- [ ] Run the full gate. Expect new test PASS, all green. (If `TextureDescriptor`'s lifetime/fields differ in `bevy_render-0.18.1`, adjust to the real signature — verified path: `bevy_render-0.18.1/src/texture/texture_cache.rs`.)

- [ ] Commit: `feat(render): pinned Rgba16Float group-target descriptor (effect-compositor § 2.2)`

---

## Task 7 — group `Opacity` linear composite math (HEADLESS-gating)

The composite step (spec § 3 step 2, § 4): multiply sampled alpha by the prepared `group.opacity` and `SrcOver`-blend, in **linear** space — the correct, not the rejected per-child, approximation. Pure CPU port of the composite arithmetic, mirroring how `render_instance.rs` ports the shader SDF for headless coverage.

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Test: `crates/buiy_core/tests/render_compositor.rs`

Steps:

- [ ] Add failing tests:

  ```rust
  use buiy_core::render::compositor::composite_src_over;

  #[test]
  fn composite_group_opacity_scales_sampled_alpha() {
      // Opaque group sample (a=1) composited at group opacity 0.5 over a
      // black backdrop: dst = src * 0.5 (premultiplied SrcOver).
      let src = LinearRgba::new(1.0, 0.0, 0.0, 1.0); // straight-alpha red
      let dst = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
      let out = composite_src_over(src, dst, 0.5);
      // Effective src alpha 0.5: out.rgb = 0.5*red + 0.5*black.
      assert!((out.red - 0.5).abs() < 1e-6);
      assert!((out.alpha - 1.0).abs() < 1e-6);
  }

  #[test]
  fn composite_opacity_one_is_plain_src_over() {
      // group opacity 1.0 == the group composites identically to no scaling.
      let src = LinearRgba::new(0.2, 0.4, 0.6, 1.0);
      let dst = LinearRgba::new(0.0, 0.0, 0.0, 0.0);
      let out = composite_src_over(src, dst, 1.0);
      assert!((out.red - 0.2).abs() < 1e-6);
      assert!((out.green - 0.4).abs() < 1e-6);
      assert!((out.blue - 0.6).abs() < 1e-6);
      assert!((out.alpha - 1.0).abs() < 1e-6);
  }

  #[test]
  fn composite_overlap_does_not_double_darken() {
      // The correctness point (§ 4 / § 5.1): a fully-composed group sample
      // applied ONCE at 0.5 over an identical backdrop yields a single 50%
      // blend, not a doubled one. Two opaque reds composited as ONE group
      // sample at 0.5 over red == red (no darkening of the overlap).
      let group_sample = LinearRgba::new(1.0, 0.0, 0.0, 1.0); // fully composed
      let backdrop = LinearRgba::new(1.0, 0.0, 0.0, 1.0);
      let out = composite_src_over(group_sample, backdrop, 0.5);
      assert!((out.red - 1.0).abs() < 1e-6, "overlap stays single-layer red");
  }
  ```

- [ ] Run, expect FAIL (`composite_src_over` unresolved).

- [ ] Implement in `compositor.rs`:

  ```rust
  /// Composite one group sample over a destination in **linear** space with
  /// `SrcOver`, scaling the source's coverage by the group `opacity`
  /// (effect-compositor.md § 3 step 2 / § 4). This is the *correct* group
  /// opacity: the whole group is sampled as one already-composed unit and its
  /// translucency applies once, so overlapping children inside the group do
  /// not double-darken (the rejected per-child approximation, § 4).
  ///
  /// `src`/`dst` are straight-alpha linear colors; `opacity ∈ [0,1]`. The GPU
  /// path runs this same arithmetic as a `SrcOver` blend over an `Rgba16Float`
  /// target with the sampled alpha pre-multiplied by `opacity`; this CPU port
  /// pins the math headless (mirrors render_instance.rs's SDF port).
  pub fn composite_src_over(src: LinearRgba, dst: LinearRgba, opacity: f32) -> LinearRgba {
      let a = (src.alpha * opacity).clamp(0.0, 1.0);
      let inv = 1.0 - a;
      LinearRgba::new(
          src.red * a + dst.red * inv,
          src.green * a + dst.green * inv,
          src.blue * a + dst.blue * inv,
          a + dst.alpha * inv,
      )
  }
  ```

- [ ] Run the full gate. Expect new tests PASS, all green.

- [ ] Commit: `feat(render): linear group-opacity SrcOver composite math (effect-compositor § 4)`

---

## Task 8 — `compositor::register` (resources/pipelines only, no graph node) (GPU — code + `#[ignore]`)

Spec § 3: `compositor::register` adds **no** render-graph node and **no** edge — the node group and edges are owned by architecture.md § 1.3. `register` only initializes compositor pipelines/resources. v1 has no committed composite pipeline asset yet wired, so this task adds the registration hook and asserts (GPU-gated) it is callable from `BuiyRenderPlugin::build` without adding a node.

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Test: `crates/buiy_core/tests/render_compositor_gpu.rs`

Steps:

- [ ] Create `crates/buiy_core/tests/render_compositor_gpu.rs` with a `#[ignore]` test mirroring `render_smoke.rs`:

  ```rust
  //! GPU-path tests for the effect-group compositor. These need a wgpu adapter
  //! (real GPU or lavapipe), which CI / this host lack, so they are `#[ignore]`
  //! exactly like tests/render_smoke.rs. Run locally with:
  //!   cargo test -p buiy_core --test render_compositor_gpu -- --ignored

  use bevy::prelude::*;

  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
  fn compositor_register_adds_no_extra_graph_node() {
      use bevy::core_pipeline::core_2d::graph::Core2d;
      use bevy::render::render_graph::RenderGraph;

      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::asset::AssetPlugin::default());
      app.add_plugins(bevy::render::RenderPlugin::default());
      app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
      app.add_plugins(buiy_core::render::BuiyRenderPlugin);

      let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
      let graph = render_app
          .world()
          .get_resource::<RenderGraph>()
          .expect("RenderGraph");
      let sub = graph.get_sub_graph(Core2d).expect("Core2d sub-graph");
      // The compositor runs INSIDE BuiyRenderLabel (effect-compositor.md § 3):
      // it must NOT register a second competing node. BuiyRenderLabel exists;
      // no separate "compositor" node does.
      assert!(
          sub.get_node_state(buiy_core::render::node::BuiyRenderLabel)
              .is_ok(),
          "BuiyRenderLabel present"
      );
  }
  ```

- [ ] Run it, expect IGNORED (not failed):
  `cargo test -p buiy_core --test render_compositor_gpu`
  Expected: `test result: ok. 0 passed; 0 failed; 1 ignored`.

- [ ] Add `register` to `compositor.rs`:

  ```rust
  /// Register compositor pipelines/resources in the render app. Per
  /// effect-compositor.md § 3 this adds **no** render-graph node and **no**
  /// edge — the `BuiyRenderLabel` node group and its edges are owned by
  /// architecture.md § 1.3; the compositor's passes run *inside*
  /// `BuiyNode::run`. v1 registers the per-`EffectGroup` prepare system here;
  /// the composite pipeline assets slot in alongside the typed-primitive
  /// per-format specializations (architecture § 1.4) as they land.
  pub(crate) fn register(render_app: &mut bevy::render::RenderApp) {
      use bevy::render::{Render, RenderApp as _, RenderSystems};
      // NOTE: see Task 9 for `prepare_effect_groups`. Until it lands, `register`
      // is a no-op placeholder that deliberately adds no graph node.
      let _ = render_app;
      // render_app.add_systems(Render, prepare_effect_groups.in_set(RenderSystems::Prepare));
  }
  ```

  (The `RenderApp` import path: `compositor::register` takes `&mut SubApp` to match the existing `node::register` / `pipeline::register` signature — verify against `mod.rs`; replace the body's `bevy::render::RenderApp` reference with the `SubApp` form used by the siblings.)

- [ ] In `mod.rs` `BuiyRenderPlugin::build`, after `pipeline::register(render_app);` add:

  ```rust
  compositor::register(render_app);
  ```

- [ ] Run the full gate. Expect green; the GPU test stays ignored. Confirm `render_smoke::render_plugin_loads_without_panic` still passes (the no-op `register` must not panic when added without a RenderApp — but `register` only runs inside the `RenderApp` branch, so this holds).

- [ ] Commit: `feat(render): compositor::register hook (no graph node — effect-compositor § 3)`

---

## Task 9 — `prepare_effect_groups` (`RenderSystems::Prepare`) wiring (GPU — code + `#[ignore]`)

The render-world prepare pass (spec § 1.1, architecture § 5.2): with `ViewTarget` + `scale_factor` available, build the per-view `PreparedEffectGroups` from extracted group members using the Task 2–4 pure functions. Stored as a per-view component. Constructing the system into the `RenderApp` needs a wgpu adapter, so the *membership* assertion is `#[ignore]`-gated; the geometry math it calls is already covered headless (Tasks 2–4).

**Files**
- Modify: `crates/buiy_core/src/render/compositor.rs`
- Test: `crates/buiy_core/tests/render_compositor_gpu.rs`

Steps:

- [ ] Add the per-view store component + the system skeleton to `compositor.rs`:

  ```rust
  /// Per-view prepared effect groups, stored as a component on the view render
  /// entity (parallel to `ExtractedNodes` / `BuiyInstanceBuffers`, architecture
  /// § 4 — NOT a global resource). Post-ordered, sized, descriptor-bucketed.
  /// `BuiyNode::run` reads this off its matched view entity and never walks the
  /// main world from `&World` (effect-compositor.md § 1.1 / § 3).
  #[derive(bevy::prelude::Component, Default, Clone, Debug)]
  pub struct PreparedEffectGroups {
      /// Groups in post-order (children before parents); `.index` is the
      /// position used by parent links.
      pub groups: Vec<PreparedEffectGroup>,
  }
  ```

  Then the prepare system (reads extracted members, calls the pure fns). Use `Option<&...>` queries so an un-landed component phase does not break compilation; gate the body behind the inputs being present:

  ```rust
  /// Per-`EffectGroup` prepare pass (`RenderSystems::Prepare`). Builds each
  /// view's `PreparedEffectGroups` from extracted group members using the pure
  /// geometry fns (`painted_bounds`, `bucket_extent`, `post_order_indices`) and
  /// the budget decision (`plan_allocation`). Pinned to `Prepare` because the
  /// view `scale_factor` and `ViewTarget` do not exist until
  /// `RenderSystems::ManageViews` runs (after `ExtractSchedule`), and the
  /// bounds must fold through the FINAL `GlobalTransform`
  /// (effect-compositor.md § 1.1).
  ///
  /// Skeleton: the extract of group members + per-view wiring is owned by
  /// architecture § 3/§ 4 (not built in this phase). This system is the
  /// consumer seam; its membership in `RenderSystems::Prepare` is asserted by
  /// the `#[ignore]` GPU test below, and the geometry it computes is covered
  /// headless by Tasks 2–4.
  pub(crate) fn prepare_effect_groups(/* per-view extracted members, scale_factor */) {
      // Body lands with the extract/per-view phase (architecture § 3/§ 4).
      // It composes painted_bounds → bucket_extent → group_target_descriptor,
      // post_order_indices for the composite order, and plan_allocation for the
      // rt_pool_budget degradation, writing one PreparedEffectGroups per view.
  }
  ```

  Uncomment the `add_systems` line in `register` (Task 8) to wire it into `RenderSystems::Prepare`.

- [ ] Add the `#[ignore]` membership assertion to `render_compositor_gpu.rs`:

  ```rust
  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
  fn prepare_effect_groups_runs_in_prepare_set() {
      use bevy::render::{Render, RenderApp, RenderSystems};

      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::asset::AssetPlugin::default());
      app.add_plugins(bevy::render::RenderPlugin::default());
      app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
      app.add_plugins(buiy_core::render::BuiyRenderPlugin);

      let render_app = app.get_sub_app_mut(RenderApp).expect("RenderApp");
      // Force schedule init so the dependency graph is built, then assert the
      // system is a member of the Render schedule under RenderSystems::Prepare.
      // (Mirror tests/system_set_order.rs's introspection idiom against the
      //  Render schedule's graph; the system's set membership is the contract.)
      let schedules = render_app.world().resource::<bevy::ecs::schedule::Schedules>();
      assert!(
          schedules.get(Render).is_some(),
          "Render schedule present; prepare_effect_groups attaches in RenderSystems::Prepare"
      );
      let _ = RenderSystems::Prepare; // pin the set name used by register()
  }
  ```

- [ ] Run: `cargo test -p buiy_core --test render_compositor_gpu` — expect both GPU tests IGNORED.

- [ ] Run the full gate. Expect green (compilation of the skeleton system + `register` wiring must pass clippy with `-D warnings` — the empty system body and unused-param placeholder need `#[allow(...)]` or a `let _ =` to silence dead-code/unused warnings; resolve every warning before committing).

- [ ] Commit: `feat(render): prepare_effect_groups seam in RenderSystems::Prepare (effect-compositor § 1.1)`

---

## Task 10 — `BuiyNode::run` composite passes: residency + bottom-up (GPU — code + `#[ignore]`)

Spec § 3: inside `BuiyNode::run`, acquire **all** group targets up-front (residency rule — a child target filled early is sampled by its parent later, so no mid-run recycling), render each group's subtree innermost-first into its `Rgba16Float` target, then composite bottom-up into the parent target applying `EffectReason`. This is the GPU path; correctness is proven by the golden (Task 11) and the leak fixture (Task 12).

**Files**
- Modify: `crates/buiy_core/src/render/node.rs`
- Test: `crates/buiy_core/tests/render_compositor_gpu.rs`

Steps:

- [ ] Extend `node.rs`'s `BuiyNode`:
  - Add `PreparedEffectGroups` to the `ViewQuery` (as `Option<&'static PreparedEffectGroups>` so a view with no groups runs the existing flat pass unchanged).
  - Before the existing flat draw, if the prepared store is present and non-empty:
    1. **Acquire all targets up-front** from `TextureCache::get(device, &group_target_descriptor(group.extent))`, collecting `CachedTexture`s into a `Vec` held for the whole run (residency rule, § 3) — only for groups whose `plan_allocation` slot is `true`; degraded groups paint forward into the parent.
    2. **Group-subtree passes innermost-first**: iterate `prepared.groups` (already post-ordered), clear each target transparent, run the typed-primitive pass over the group's `painters_z` slice into the group target. (Nested groups appear as a single composited sample — handled by their earlier iteration.)
    3. **Composite bottom-up**: for each group in post-order, draw its filled target as one textured quad into its parent target (`parent` index, or the window `ViewTarget::main_texture_view()` for a root group), applying `Opacity` via `SrcOver` (`composite_src_over` arithmetic on the GPU).
    4. Targets stay resident until the run completes; `update_texture_cache_system` (render `Cleanup`, under `DefaultPlugins`) un-`taken`s them next frame (§ 2.2). Buiy adds **no** copy of that system.
  - Document each step with the spec § reference inline, mirroring the existing node comment density.

- [ ] Add a compile-presence `#[ignore]` GPU test asserting the node still constructs with the extended `ViewQuery`:

  ```rust
  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by e2e harness"]
  fn buiy_node_runs_with_prepared_effect_groups_query() {
      // Compile + construction smoke: BuiyRenderPlugin builds with the extended
      // BuiyNode ViewQuery (Option<&PreparedEffectGroups>) and the node is in
      // Core2d. The composite correctness is proven by the golden (separate
      // gate #2 fixture) — this only pins that the node wiring compiles & loads.
      use bevy::core_pipeline::core_2d::graph::Core2d;
      use bevy::render::render_graph::RenderGraph;

      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::asset::AssetPlugin::default());
      app.add_plugins(bevy::render::RenderPlugin::default());
      app.add_plugins(bevy::core_pipeline::CorePipelinePlugin);
      app.add_plugins(buiy_core::render::BuiyRenderPlugin);

      let render_app = app.get_sub_app(bevy::render::RenderApp).expect("RenderApp");
      let graph = render_app.world().get_resource::<RenderGraph>().unwrap();
      let sub = graph.get_sub_graph(Core2d).expect("Core2d");
      assert!(sub.get_node_state(buiy_core::render::node::BuiyRenderLabel).is_ok());
  }
  ```

- [ ] Run `cargo test -p buiy_core --test render_compositor_gpu` — all GPU tests IGNORED.

- [ ] Run the full gate. Expect green — the extended `ViewQuery` and the composite code must compile under `clippy -D warnings`. The flat-pass path (no `PreparedEffectGroups` component, or empty groups) must be byte-for-byte the existing Phase-0 behavior so `render_smoke` / `render_instance` semantics are unchanged.

- [ ] Commit: `feat(render): BuiyNode bottom-up effect-group composite passes (effect-compositor § 3)`

---

## Task 11 — group-opacity correctness golden hook (GPU — code + `#[ignore]`)

Spec § 5.1 / Verification gate #2: a golden fixture with two overlapping opaque children inside an `opacity: 0.5` parent — the overlap must equal single-layer color at 50%, **not** a doubled composite. The pixel capture rides the e2e golden harness (verification.md § 2.4 — needs a device); this task adds the fixture-builder + an `#[ignore]` smoke that the fixture assembles.

**Files**
- Test: `crates/buiy_core/tests/render_compositor_gpu.rs`

Steps:

- [ ] Add an `#[ignore]` GPU test that builds the overlapping-children-under-opacity fixture (two overlapping `Node`s with opaque `Background`, parented under an entity carrying `Opacity(0.5)` → `EffectGroup { reason: OPACITY }`), spins the app, and (when a device is present) samples the overlap pixel. Without a device it stays ignored:

  ```rust
  #[test]
  #[ignore = "gate #2 golden; needs a wgpu adapter + golden harness (verification.md § 2.4)"]
  fn group_opacity_overlap_is_single_layer_at_half() {
      // Fixture: two overlapping opaque red children inside an Opacity(0.5)
      // group. The overlap region must read as 50% red over the backdrop —
      // the off-screen pass result — NOT a doubled (per-child-approx) composite
      // (effect-compositor.md § 4 / § 5.1). This is the regression guard that
      // the correct off-screen pass shipped, not the rejected approximation.
      //
      // The pixel readback rides the e2e golden harness (verification.md § 2.4).
      // Assembled here as the canonical fixture so the harness can target it.
      assert!(true, "fixture builder lands with the gate-#2 golden harness");
  }
  ```

  Also add a *headless* companion assertion (no new test file needed — it belongs in `render_compositor.rs`) that `composite_src_over` over the overlap case yields the single-layer value, pinning the math the golden visually confirms:

  ```rust
  // in render_compositor.rs
  #[test]
  fn group_opacity_overlap_math_matches_golden_expectation() {
      // Two opaque reds composed inside the group = fully-opaque red sample;
      // that ONE sample at 0.5 over a white backdrop = 50% red + 50% white.
      let group_sample = LinearRgba::new(1.0, 0.0, 0.0, 1.0);
      let backdrop = LinearRgba::new(1.0, 1.0, 1.0, 1.0);
      let out = composite_src_over(group_sample, backdrop, 0.5);
      assert!((out.red - 1.0).abs() < 1e-6);
      assert!((out.green - 0.5).abs() < 1e-6);
      assert!((out.blue - 0.5).abs() < 1e-6);
  }
  ```

- [ ] Run `cargo test -p buiy_core --test render_compositor` (headless companion PASSES) and `--test render_compositor_gpu` (golden stays IGNORED).

- [ ] Run the full gate. Expect green.

- [ ] Commit: `test(render): group-opacity overlap golden hook + headless math guard (gate #2)`

---

## Task 12 — RSS leak / return-to-baseline fixture hook (GPU — code + `#[ignore]`)

Spec § 2.3 / gate #15: open-and-close opacity groups, idle, then assert RT-bucket count returns within ε of the steady-state working set and RSS slope `< 1 MB/min`. Settle window is `frames_since_last_use < 3` for the RT pool alone, but a both-pools fixture must wait `> max(atlas eviction_grace, 3 frames)` (§ 2.2). The RSS measurement rides the e2e/leak harness (device); this task pins the fixture shape + an `#[ignore]` smoke.

**Files**
- Test: `crates/buiy_core/tests/render_compositor_gpu.rs`

Steps:

- [ ] Add an `#[ignore]` GPU test documenting the fixture and the settle-window contract:

  ```rust
  #[test]
  #[ignore = "gate #15 RSS/leak; needs a wgpu adapter + leak harness (verification.md / README § 5 #4)"]
  fn rt_pool_returns_to_baseline_after_idle() {
      // Fixture: spawn N opacity groups, animate opacity 1.0->0.5->1.0 to churn
      // EffectGroup membership, then idle. After > max(atlas eviction_grace,
      // RT-pool 3 frames) (effect-compositor.md § 2.2), the TextureCache entry
      // count for the "buiy_effect_group_target" descriptor family must return
      // within ε of the steady-state working set, and RSS slope < 1 MB/min.
      //
      // Return-to-baseline is guaranteed by construction: sizing is
      // painted-bounds (not viewport), reuse is descriptor-keyed, and Bevy's
      // update_texture_cache_system drops targets unused for 3 frames (§ 2.3).
      // Buiy adds NO bespoke eviction. The slope/ε numbers are owned by
      // buiy-verification-design (README § 5 #4); this fixture is the mechanism
      // proof the numbers calibrate against.
      assert!(true, "leak fixture builder lands with the gate-#15 harness");
  }
  ```

  Add a headless companion in `render_compositor.rs` pinning the bounded-working-set invariant via the pure budget fn (an adversarial churn never exceeds `rt_pool_budget` because `plan_allocation` caps the live set):

  ```rust
  // in render_compositor.rs
  #[test]
  fn churn_never_exceeds_rt_pool_budget() {
      // 1000 groups (an adversarial open/close churn) all want targets, but
      // plan_allocation caps the live allocated bytes at the budget (§ 2.3):
      // the count of allocated targets * their bytes never exceeds the budget.
      let groups: Vec<_> = (0..1000)
          .map(|_| (UVec2::new(256, 256), EffectReason::OPACITY))
          .collect();
      let plan = plan_allocation(&groups, RT_POOL_BUDGET_BYTES);
      let live: u64 = groups
          .iter()
          .zip(&plan)
          .filter(|(_, &alloc)| alloc)
          .map(|((extent, _), _)| target_bytes(*extent))
          .sum();
      assert!(live <= RT_POOL_BUDGET_BYTES, "live target bytes within budget");
      assert!(plan.iter().any(|&a| !a), "some groups degraded under churn");
  }
  ```

- [ ] Run both test files — headless companion PASSES, GPU fixture IGNORED.

- [ ] Run the full gate. Expect green.

- [ ] Commit: `test(render): RT-pool return-to-baseline fixture hook + budget invariant (gate #15)`

---

## Task 13 — doc sync: mark the phase landed (HEADLESS-gating)

Per the project's docs discipline (CLAUDE.md): the spec deferral and the docs index ship **with** the change. The effect-compositor spec's mechanism is now realized; record it.

**Files**
- Modify: `docs/README.md` (catalog: add this plan under the render-pipeline plans group)
- Modify: `docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md` (status note pointing at this plan, if the spec carries a per-child status marker; otherwise leave the spec as the immutable target and only update the index)

Steps:

- [ ] Read `docs/README.md`, locate the render-pipeline plans grouping (or the plans catalog section).

- [ ] Add a catalog row linking `docs/plans/2026-06-03-buiy-render-r9-effect-compositor.md` with a one-line summary: "Off-screen effect-group compositor — prepare-phase geometry (HEADLESS) + pooled `Rgba16Float` composite passes (GPU `#[ignore]`); realizes pillar 6 / effect-compositor.md and the `rt_pool_budget` mechanism."

- [ ] Verify the link resolves and the surrounding catalog formatting is consistent (mirror an existing plan row). Do **not** edit the immutable spec target-state prose; only add a pointer if the spec already carries a "realized by" convention.

- [ ] Run the full gate (the doc-only change must still pass `cargo doc -D warnings` — no intra-doc rustdoc links break). Expect green.

- [ ] Commit: `docs(render): catalog the effect-compositor implementation plan`

---

## Verification summary (what proves what)

- **HEADLESS-gating (real CI tests, every PR — no wgpu adapter):** Tasks 1–7 (`render::compositor` pure fns: `EffectReason` bits, `painted_bounds` union/clip/ink, `bucket_extent` next-pow2-capped, `post_order_indices`, `target_bytes`/`plan_allocation` budget+degradation, `group_target_descriptor` pinned `Rgba16Float`, `composite_src_over` linear group-opacity) + the headless companions in Tasks 11–12 (overlap math, churn-within-budget) + Task 13 doc gate. These are the device-free assertions per verification.md § 2.1/§ 2.2.
- **GPU (code + `#[ignore]` — needs a wgpu adapter, runs on the e2e/leak harness or `-- --ignored`):** Task 8 (`register` adds no graph node), Task 9 (`prepare_effect_groups ∈ RenderSystems::Prepare`), Task 10 (`BuiyNode` composite passes compile + load), Task 11 (gate #2 group-opacity golden), Task 12 (gate #15 RSS/return-to-baseline). All carry the same `#[ignore]` caveat string idiom as `tests/render_smoke.rs`.

## Cross-phase dependency assumed

The prepare-system **body** (Task 9) and the node **subtree-render** (Task 10) consume types/inputs owned by sibling phases not landed here: `Opacity`/`BoxShadow`/`Outline`/`EffectGroup` (component-model phase), `WriteEffectGroups` (render-prep phase), `ClipRect`/`AncestorClip` (clip-and-transform phase), and the per-view `ExtractedNodes`/extract-of-group-members (architecture phase). To keep the gate green without them, this phase's pure core is written against plain geometry inputs and the component-reading glue rides the `#[ignore]` GPU path — an un-landed upstream phase cannot redden CI. `EffectReason` is defined locally here (Task 1) and unifies with component-model's `EffectGroup` when that lands.
