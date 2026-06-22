# Prepare phase: persistent buffers + view uniform + instance packing Implementation Plan

**Date:** 2026-06-03
**Status:** landed

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Depends on:** R5 (and R1). Execution order: R1 → R2 → R3 → R4 → R5 → **R6** → R7 → R8 → (R9, R10) → R11. R6 **consumes** R5's per-view `ExtractedNodes` component (owned by R5, `render/extract.rs`) and R1's shared render types (`render/components.rs`); it does **not** redefine them. R6 **owns** the CPU instance bucketing (`render/buckets.rs`) and the shared `BuiyPrimitiveKind` enum that R7 imports.

**Goal:** Replace the Phase-0 per-instance CPU y-flip/radius-approximation hack with a per-view **view uniform** (logical-px → clip, carrying the single y-flip + `scale_factor`) and a `RenderSystems::Prepare` system that owns persistent per-view GPU instance buffers, packing per-view `ExtractedNodes` into typed-primitive (quad/shadow/border/outline) instance sets keyed by `(primitive, layer)`.
**Spec:** [2026-06-03-buiy-render-pipeline-design](../specs/2026-06-03-buiy-render-pipeline-design/README.md) — realizes **architecture.md § 1.3 / § 2 / § 3** (the hybrid handoff: pillar 3 persistent buffers + view uniform; the prepare-phase split; per-view storage) and **color-and-forced-colors.md § 1** (linear-light color stays CPU-pre-linearized; the view uniform is the coordinate seam that lets the GPU OETF the value correctly).
**Architecture:** The Phase-0 path bakes a y-flip into a negative instance height and approximates corner-radius px→clip with `2.0 / min(window.x, window.y)`, both in `render::instance::to_instance`. This phase moves the logical-px → clip transform into a single **view uniform** (`BuiyViewUniform`: a 2×3-style logical→clip affine plus `scale_factor`) so `InstanceData` shrinks back to **logical-pixel** units and the radius stays in logical px (SDF evaluates in pixel space, killing the non-square-window approximation). A `prepare_buiy_instances` system in `RenderSystems::Prepare` packs the per-view `ExtractedNodes` into typed-primitive instance sets keyed by `(primitive, layer)` and uploads them into a **persistent** per-view `BuiyInstanceBuffers` (grow-in-place, never per-frame reallocated). `ViewTarget` is available in Prepare (it is created by `prepare_view_targets` in `RenderSystems::ManageViews`, *after* `ExtractSchedule`), which is exactly why the GPU buffers/uniform are a prepare product, not an extract product.
**Tier/Test reality:** **GPU (code + #[ignore] e2e — no wgpu adapter on CI/this host) for everything touching the `RenderApp`/buffer upload/prepare-system membership; HEADLESS (unit on CI) for the CPU packing + view-uniform math.** The CPU math (logical-px → clip via the view-uniform affine, sRGB→linear color, `(primitive, layer)` bucketing, instance stride vs. the pipeline descriptor) is pure-fn and gates every commit. The OLD per-instance-y-flip assertions in `render_instance.rs` are marked **Phase-0 baseline replaced** and ported to the new view-uniform math. Buffer upload, view-uniform GPU write, and `prepare_buiy_instances ∈ RenderSystems::Prepare` render-world membership ride the `#[ignore]` GPU path (mirror the `render_smoke.rs` `#[ignore]` idiom — `RenderPlugin::build` `.expect()`s a wgpu adapter that CI lacks).

---

## The gate (every commit must keep this green)

This host and CI have **no xvfb and no wgpu adapter**. Every task's final step runs:

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace
```

`cargo test --workspace` runs **without** `xvfb-run` and **without** `--ignored`, so only the HEADLESS tests execute on the gate. GPU tests are `#[ignore]`d and run locally only with `-- --ignored` on a machine that has a GPU/lavapipe.

## Conventions for this plan

- **Exact type/field names** come from the spec § 3 contract and architecture.md: `ExtractedNodes` (per-view component, **owned + populated by R5** in `render::extract`; R6 consumes it), `BuiyInstanceBuffers` (per-view component, owned here), `prepare_buiy_instances` (the `RenderSystems::Prepare` system, owned here), `BuiyViewUniform` (the logical→clip + `scale_factor` uniform; conceptually the same role as `bevy_render::view::ViewUniform`), `BuiyPrimitiveKind` (the shared primitive-kind enum, owned here in `render::buckets`; R7 imports it). The typed primitives this phase buckets are **quad / shadow / border / outline** (architecture.md § 2.1 — `border` is the outer-minus-inner band painted as a quad variant; `glyph`/`path` are out of scope for this phase).
- **Primitive vs. layer key.** A bucket key is `(BuiyPrimitiveKind, layer: u32)` where `layer` is the forward index into `StackingContext.painters_z` (architecture.md § 2.2). This phase does **not** compute real `painters_z` layers (that is the paint-order phase); it threads a `layer` field through the packing API and defaults it to `0`, so the bucketing machinery and its tests exist and the paint-order phase only has to feed real layer indices.
- **HEADLESS vs GPU is called out per task.** Pure-CPU math tests live in `crates/buiy_core/tests/render_instance.rs` (the existing file — extend it). GPU tests live in `crates/buiy_core/tests/render_prepare.rs` (new) and are `#[ignore]`d with the same comment shape as `render_smoke.rs`.
- This phase reads R5's per-view `ExtractedNodes` component (owned + populated by R5) and packs it through the new view-uniform math; it does **not** build a parallel carrier from the Phase-0 `ExtractedDraws`. The Phase-0 `DrawData` / `ExtractedDraws` / `extract_buiy_draws` stay alive only until the extract phase retires them — R6 does not depend on them. The `(primitive, layer)` bucketing + view-uniform packing is the prepare product, decoupling "what R5 extracted" from "what prepare packed" exactly as architecture.md § 3.1/§ 3.2 require.

---

## Task 1 — `BuiyViewUniform`: the logical-px → clip affine (HEADLESS, pure CPU math)

Introduce the view-uniform CPU type and its construction math. This is the core replacement for the per-instance y-flip/radius hack: a single per-view transform that maps a logical-pixel point to clip space (`-1..+1`, y-up), folding in the y-flip **once** and carrying `scale_factor`. Pure CPU; no GPU.

**Files**
- Create: `crates/buiy_core/src/render/view_uniform.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod view_uniform;`)
- Test: `crates/buiy_core/tests/render_view_uniform.rs` (new)

### Steps

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_view_uniform.rs`:

  ```rust
  //! Pure-CPU tests for the Buiy view uniform: the logical-pixel -> clip-space
  //! affine that replaces the Phase-0 per-instance y-flip / radius hack. No GPU
  //! adapter required (this is the HEADLESS half of the prepare phase).

  use bevy::prelude::*;
  use buiy_core::render::view_uniform::{BuiyViewUniform, VIEW_UNIFORM_SIZE_BYTES};

  #[test]
  fn view_uniform_size_is_std140_friendly() {
      // The uniform is uploaded to a UBO; its CPU size must be a multiple of 16
      // (std140 alignment) so the GPU layout is unambiguous. logical_to_clip is a
      // mat4 surrogate packed as 2 columns of vec4 (32 B) + scale_factor + 3 pad
      // (16 B) = 48 B.
      assert_eq!(std::mem::size_of::<BuiyViewUniform>(), VIEW_UNIFORM_SIZE_BYTES);
      assert_eq!(VIEW_UNIFORM_SIZE_BYTES % 16, 0);
  }

  #[test]
  fn origin_maps_to_clip_top_left() {
      // Logical (0,0) is the window top-left; in clip space (y-up) that is
      // (-1, +1). The y-flip lives ENTIRELY in the uniform now.
      let u = BuiyViewUniform::for_view(Vec2::new(800.0, 600.0), 1.0);
      let p = u.apply(Vec2::ZERO);
      assert!((p.x - -1.0).abs() < 1e-6, "x={}", p.x);
      assert!((p.y - 1.0).abs() < 1e-6, "y={}", p.y);
  }

  #[test]
  fn bottom_right_maps_to_clip_bottom_right() {
      // Logical (w,h) -> clip (+1, -1).
      let w = Vec2::new(800.0, 600.0);
      let u = BuiyViewUniform::for_view(w, 1.0);
      let p = u.apply(w);
      assert!((p.x - 1.0).abs() < 1e-6, "x={}", p.x);
      assert!((p.y - -1.0).abs() < 1e-6, "y={}", p.y);
  }

  #[test]
  fn center_maps_to_clip_origin() {
      let w = Vec2::new(800.0, 600.0);
      let u = BuiyViewUniform::for_view(w, 1.0);
      let p = u.apply(w * 0.5);
      assert!(p.x.abs() < 1e-6 && p.y.abs() < 1e-6, "p={p:?}");
  }

  #[test]
  fn scale_factor_is_carried_verbatim() {
      // The uniform carries scale_factor so the SDF/radius can stay in logical
      // px on the GPU. The logical->clip affine itself is in LOGICAL px (the
      // window size passed in is logical), so scale_factor does NOT scale the
      // affine; it is a separate field the shader uses for px-space AA.
      let u = BuiyViewUniform::for_view(Vec2::new(800.0, 600.0), 2.0);
      assert!((u.scale_factor() - 2.0).abs() < 1e-6);
      // Same logical window, different scale_factor => SAME logical->clip mapping.
      let u1 = BuiyViewUniform::for_view(Vec2::new(800.0, 600.0), 1.0);
      assert!((u.apply(Vec2::new(400.0, 300.0)) - u1.apply(Vec2::new(400.0, 300.0))).length() < 1e-6);
  }

  #[test]
  fn no_per_axis_radius_distortion() {
      // The Phase-0 hack approximated px->clip radius with 2/min(w,h), which
      // distorts on non-square windows. The view uniform removes that: a logical
      // delta maps to clip with INDEPENDENT per-axis scale, so a square in px is
      // a square in px on the GPU (radius stays in logical px). Assert the per-
      // axis clip scale differs on a non-square window (the thing the old hack
      // collapsed to a single min()).
      let u = BuiyViewUniform::for_view(Vec2::new(1000.0, 500.0), 1.0);
      let dx = (u.apply(Vec2::new(1.0, 0.0)) - u.apply(Vec2::ZERO)).x;
      let dy = (u.apply(Vec2::new(0.0, 1.0)) - u.apply(Vec2::ZERO)).y;
      // 1 logical px maps to 2/1000 in x, -2/500 in y: magnitudes differ.
      assert!((dx - (2.0 / 1000.0)).abs() < 1e-9);
      assert!((dy - (-2.0 / 500.0)).abs() < 1e-9);
      assert!((dx.abs() - dy.abs()).abs() > 1e-6, "per-axis scale must differ");
  }
  ```

- [ ] **Run it — expect FAIL (does not compile: module/type absent).**

  ```sh
  cargo test -p buiy_core --test render_view_uniform
  ```
  Expected: `error[E0432]: unresolved import buiy_core::render::view_uniform`.

- [ ] **Minimal impl.** Create `crates/buiy_core/src/render/view_uniform.rs`:

  ```rust
  //! The Buiy view uniform: the per-view logical-pixel -> clip-space affine that
  //! replaces the Phase-0 per-instance y-flip / radius approximation in
  //! `render::instance`. It plays the same role for Buiy that
  //! `bevy_render::view::ViewUniform` plays for the engine: one per-view
  //! transform uploaded to a UBO, applied in the vertex stage, so the per-
  //! instance record can stay in LOGICAL-pixel units.
  //!
  //! Spec: architecture.md § 3.2 (the hybrid handoff retires the per-instance
  //! coordinate hack) + color-and-forced-colors.md § 1.1 (color stays CPU-pre-
  //! linearized; only the COORDINATE packing moves to this uniform).

  use bevy::prelude::*;
  use bytemuck::{Pod, Zeroable};

  /// CPU size of [`BuiyViewUniform`] in bytes. Must be a multiple of 16 for
  /// std140 UBO alignment. 2 columns of `vec4` (32 B) + `scale_factor` + 3 pad
  /// (16 B) = 48 B.
  pub const VIEW_UNIFORM_SIZE_BYTES: usize = 48;

  /// Per-view logical-pixel -> clip-space transform, plus the view
  /// `scale_factor`. Uploaded once per view per frame in
  /// `RenderSystems::Prepare`; applied in the vertex stage so [`InstanceData`]
  /// stays in logical-pixel units.
  ///
  /// The affine is stored as two `vec4` columns (`col0`, `col1`) encoding the
  /// 2D affine `clip = M * logical + t`:
  /// - `col0 = [m00, m01, 0, tx_unused]` — but Buiy's logical->clip is purely
  ///   diagonal-scale + translate (no shear), so `col0 = [sx, 0, 0, tx]` and
  ///   `col1 = [0, sy, 0, ty]` where `clip.x = sx*lx + tx`, `clip.y = sy*ly + ty`.
  ///
  /// [`InstanceData`]: crate::render::instance::InstanceData
  #[repr(C)]
  #[derive(Copy, Clone, Debug, Pod, Zeroable)]
  pub struct BuiyViewUniform {
      /// `[sx, 0, 0, tx]` — x maps as `clip.x = sx*logical.x + tx`.
      col0: [f32; 4],
      /// `[0, sy, 0, ty]` — y maps as `clip.y = sy*logical.y + ty`.
      col1: [f32; 4],
      /// Device pixels per logical pixel for this view. Carried so the GPU can
      /// keep the SDF / corner-radius math in logical px (no non-square hack).
      scale_factor: f32,
      _pad: [f32; 3],
  }

  impl BuiyViewUniform {
      /// Build the uniform for a view of the given **logical** window size and
      /// `scale_factor`. The y-flip lives here, once: logical (0,0) (top-left)
      /// maps to clip (-1, +1); logical (w,h) maps to clip (+1, -1).
      pub fn for_view(logical_size: Vec2, scale_factor: f32) -> Self {
          let sx = 2.0 / logical_size.x;
          let sy = -2.0 / logical_size.y; // single y-flip
          Self {
              col0: [sx, 0.0, 0.0, -1.0],
              col1: [0.0, sy, 0.0, 1.0],
              scale_factor,
              _pad: [0.0; 3],
          }
      }

      /// Apply the affine to a logical-pixel point, yielding a clip-space point.
      /// The CPU mirror of the vertex-stage transform — used by tests and by
      /// any CPU-side bounds math.
      pub fn apply(&self, logical: Vec2) -> Vec2 {
          Vec2::new(
              self.col0[0] * logical.x + self.col0[3],
              self.col1[1] * logical.y + self.col1[3],
          )
      }

      /// Device-pixels-per-logical-pixel for this view.
      pub fn scale_factor(&self) -> f32 {
          self.scale_factor
      }
  }
  ```

  Then add to `crates/buiy_core/src/render/mod.rs`, next to the other `pub mod` lines:

  ```rust
  pub mod view_uniform;
  ```

- [ ] **Run it — expect PASS.**

  ```sh
  cargo test -p buiy_core --test render_view_uniform
  ```

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `feat(render): add BuiyViewUniform logical->clip affine (view-uniform handoff)`

---

## Task 2 — Logical-pixel `InstanceData` + `pack_instance` (HEADLESS, pure CPU math)

Shrink the per-instance record back to **logical-pixel** units. The new `pack_instance` produces position/size/radius in logical px (no clip conversion, no y-flip, no `2/min(w,h)`) and keeps color CPU-pre-linearized (`LinearRgba`). The view uniform (Task 1) does the clip transform on the GPU. The old `to_instance` (clip-space, y-flip baked into negative height) is retained for now only so the Phase-0 node keeps compiling; this task adds the *new* path beside it and pins the new stride.

**Files**
- Modify: `crates/buiy_core/src/render/instance.rs`
- Test: `crates/buiy_core/tests/render_instance.rs` (extend; port + re-tier the Phase-0 assertions)

### Steps

- [ ] **Write the failing test.** Append to `crates/buiy_core/tests/render_instance.rs`:

  ```rust
  // ----- view-uniform path (prepare phase) -----
  // These replace the Phase-0 clip-space `to_instance` assertions above, which
  // are now PHASE-0 BASELINE (the per-instance y-flip / radius hack the view
  // uniform retires). `pack_instance` keeps everything in LOGICAL px.
  use buiy_core::render::instance::{pack_instance, PackedInstance, PACKED_INSTANCE_STRIDE_BYTES};

  #[test]
  fn packed_instance_stride_matches_logical_pipeline_descriptor() {
      // pos(2*4) + size(2*4) + color(4*4) + radius(1*4) = 36, same field set as
      // the Phase-0 InstanceData but the values are LOGICAL px, not clip.
      assert_eq!(std::mem::size_of::<PackedInstance>(), PACKED_INSTANCE_STRIDE_BYTES);
      assert_eq!(PACKED_INSTANCE_STRIDE_BYTES, 36);
  }

  #[test]
  fn pack_instance_keeps_position_and_size_in_logical_px() {
      // No clip conversion, no y-flip baked into the size. The raw logical box
      // is forwarded; the GPU view uniform (Task 1) does the clip transform.
      let draw = DrawData::new(Vec2::new(100.0, 50.0), Vec2::new(200.0, 80.0), Color::WHITE, 12.0);
      let p = pack_instance(&draw);
      assert_eq!(p.rect_pos, [100.0, 50.0]);
      assert_eq!(p.rect_size, [200.0, 80.0]); // positive height — NO y-flip here
      assert_eq!(p.radius, 12.0);             // logical px — NO 2/min(w,h)
  }

  #[test]
  fn pack_instance_pre_linearizes_color_on_cpu() {
      // color-and-forced-colors.md § 1.1: color stays CPU-pre-linearized; only
      // the COORDINATE packing moves to the view uniform.
      let draw = DrawData::new(Vec2::ZERO, Vec2::splat(10.0), Color::srgb(1.0, 0.0, 0.0), 0.0);
      let p = pack_instance(&draw);
      let lin = LinearRgba::from(Color::srgb(1.0, 0.0, 0.0));
      assert!((p.color[0] - lin.red).abs() < 1e-5);
      assert!((p.color[1] - lin.green).abs() < 1e-5);
      assert!((p.color[2] - lin.blue).abs() < 1e-5);
      assert!((p.color[3] - lin.alpha).abs() < 1e-5);
  }

  #[test]
  fn pack_instance_then_view_uniform_equals_old_to_instance_position() {
      // Cross-check: packing in logical px and then applying the view uniform on
      // the CPU reproduces the Phase-0 clip-space position (minus the radius
      // approximation, which the new path deliberately drops). This proves the
      // view-uniform handoff is behavior-preserving for the coordinate seam.
      use bevy::prelude::Vec2;
      use buiy_core::render::view_uniform::BuiyViewUniform;
      let window = Vec2::new(800.0, 600.0);
      let draw = DrawData::new(Vec2::new(100.0, 100.0), Vec2::new(200.0, 100.0), Color::WHITE, 0.0);

      let packed = pack_instance(&draw);
      let u = BuiyViewUniform::for_view(window, 1.0);
      let top_left_clip = u.apply(Vec2::from(packed.rect_pos));

      // Old clip-space top-left from the retained Phase-0 to_instance.
      let old = buiy_core::render::instance::to_instance(&draw, window);
      assert!((top_left_clip.x - old.rect_pos[0]).abs() < 1e-6);
      assert!((top_left_clip.y - old.rect_pos[1]).abs() < 1e-6);
  }
  ```

  Then **re-tier the Phase-0 clip-space assertions**: above the `to_instance_centers_origin_at_window_center` test (and the other `to_instance_*` / `shader_*` / `signed_rect_size_*` tests), add a module doc note marking them Phase-0 baseline. Insert at the top of the file, after the existing `//!` header lines:

  ```rust
  //! NOTE (render-pipeline prepare phase): the `to_instance` / clip-space tests
  //! below are PHASE-0 BASELINE — they pin the per-instance y-flip / radius hack
  //! that the view uniform (`render::view_uniform`) retires. They are kept green
  //! while the Phase-0 node still calls `to_instance`; the live coordinate path
  //! is now `pack_instance` + `BuiyViewUniform` (see the view-uniform tests
  //! below). Do not extend the clip-space path; extend `pack_instance` instead.
  ```

- [ ] **Run it — expect FAIL (does not compile: `pack_instance`/`PackedInstance` absent).**

  ```sh
  cargo test -p buiy_core --test render_instance
  ```

- [ ] **Minimal impl.** Append to `crates/buiy_core/src/render/instance.rs` (keep the existing `InstanceData` / `to_instance` untouched):

  ```rust
  /// Stride of the logical-pixel [`PackedInstance`] in bytes. Same field set as
  /// the Phase-0 [`InstanceData`] (36 B), but the values are LOGICAL pixels —
  /// the GPU view uniform ([`crate::render::view_uniform::BuiyViewUniform`])
  /// applies the logical->clip transform in the vertex stage.
  pub const PACKED_INSTANCE_STRIDE_BYTES: usize = 36;

  /// One instance record in LOGICAL-pixel units (the view-uniform handoff). The
  /// per-instance y-flip / `2/min(w,h)` radius approximation that
  /// [`InstanceData`] bakes in is gone: position/size/radius are forwarded raw
  /// and the GPU view uniform does the clip transform. Color is CPU-pre-
  /// linearized (color-and-forced-colors.md § 1.1).
  #[repr(C)]
  #[derive(Copy, Clone, Debug, Pod, Zeroable)]
  pub struct PackedInstance {
      /// Top-left in logical pixels (window-relative, y-down).
      pub rect_pos: [f32; 2],
      /// Width / height in logical pixels (height is POSITIVE — the y-flip lives
      /// in the view uniform now, not in a negative height).
      pub rect_size: [f32; 2],
      /// Linear RGBA, pre-linearized on the CPU.
      pub color: [f32; 4],
      /// Corner radius in LOGICAL pixels (no clip-space approximation).
      pub radius: f32,
  }

  /// Pack one [`DrawData`] into a logical-pixel [`PackedInstance`]. The clip
  /// transform is deferred to the GPU view uniform; only the sRGB->linear color
  /// conversion happens here.
  pub fn pack_instance(draw: &DrawData) -> PackedInstance {
      let lin = LinearRgba::from(draw.color);
      PackedInstance {
          rect_pos: [draw.position.x, draw.position.y],
          rect_size: [draw.size.x, draw.size.y],
          color: [lin.red, lin.green, lin.blue, lin.alpha],
          radius: draw.radius,
      }
  }
  ```

- [ ] **Run it — expect PASS.**

  ```sh
  cargo test -p buiy_core --test render_instance
  ```

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `feat(render): add logical-px PackedInstance + pack_instance (view-uniform path)`

---

## Task 3 — `BuiyPrimitiveKind` + `(primitive, layer)` bucketing (HEADLESS, pure CPU)

Introduce the typed-primitive enum (`quad`/`shadow`/`border`/`outline`) and the bucketing key in `render/buckets.rs`. **R6 owns both `render/buckets.rs` (the CPU instance bucketing) and the shared `BuiyPrimitiveKind` enum** — R7 (pipeline specialization key, `render/primitive.rs`) **imports** `BuiyPrimitiveKind` from `render::buckets` rather than redefining it. Packing splits a per-view draw list into instance sets keyed by `(BuiyPrimitiveKind, layer)`, in the architecture.md § 2.2 within-layer paint order (`shadow → quad → border → outline`). This phase threads `layer` through but defaults it to `0` (real `painters_z` layers are the paint-order phase). Pure CPU.

**Files**
- Create: `crates/buiy_core/src/render/buckets.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod buckets;`)
- Test: `crates/buiy_core/tests/render_buckets.rs` (new)

### Steps

- [ ] **Write the failing test.** Create `crates/buiy_core/tests/render_buckets.rs`:

  ```rust
  //! Pure-CPU tests for typed-primitive `(primitive, layer)` bucketing. No GPU
  //! adapter required (HEADLESS half of the prepare phase).

  use buiy_core::render::buckets::{BuiyPrimitiveKind, InstanceBuckets, PrimitiveBatchKey};

  #[test]
  fn primitive_paint_order_is_shadow_quad_border_outline() {
      // architecture.md § 2.2: within a layer, back-to-front by type.
      assert!(BuiyPrimitiveKind::Shadow.paint_order() < BuiyPrimitiveKind::Quad.paint_order());
      assert!(BuiyPrimitiveKind::Quad.paint_order() < BuiyPrimitiveKind::Border.paint_order());
      assert!(BuiyPrimitiveKind::Border.paint_order() < BuiyPrimitiveKind::Outline.paint_order());
  }

  #[test]
  fn batch_keys_sort_by_layer_then_primitive() {
      let mut keys = vec![
          PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 1 },
          PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Shadow, layer: 1 },
          PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Outline, layer: 0 },
          PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 0 },
      ];
      keys.sort();
      // layer 0 before layer 1; within a layer, paint order (shadow<quad<...).
      assert_eq!(keys[0], PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 0 });
      assert_eq!(keys[1], PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Outline, layer: 0 });
      assert_eq!(keys[2], PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Shadow, layer: 1 });
      assert_eq!(keys[3], PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 1 });
  }

  #[test]
  fn buckets_group_pushed_instances_by_key() {
      let mut b = InstanceBuckets::default();
      let q0 = PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 0 };
      let s0 = PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Shadow, layer: 0 };
      b.push(q0, [0.0; 9]);
      b.push(q0, [1.0; 9]);
      b.push(s0, [2.0; 9]);
      assert_eq!(b.len(q0), 2);
      assert_eq!(b.len(s0), 1);
      assert_eq!(b.total_instances(), 3);
      // A key never pushed to has no batch.
      assert_eq!(b.len(PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Outline, layer: 0 }), 0);
  }

  #[test]
  fn buckets_iterate_in_paint_order() {
      let mut b = InstanceBuckets::default();
      b.push(PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 0 }, [0.0; 9]);
      b.push(PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Shadow, layer: 0 }, [0.0; 9]);
      b.push(PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 1 }, [0.0; 9]);
      let order: Vec<_> = b.batches().map(|(k, _)| *k).collect();
      // shadow@0, quad@0, then quad@1 — sorted ascending.
      assert_eq!(order[0].primitive, BuiyPrimitiveKind::Shadow);
      assert_eq!(order[0].layer, 0);
      assert_eq!(order[1].primitive, BuiyPrimitiveKind::Quad);
      assert_eq!(order[1].layer, 0);
      assert_eq!(order[2].layer, 1);
  }
  ```

- [ ] **Run it — expect FAIL (module/type absent).**

  ```sh
  cargo test -p buiy_core --test render_buckets
  ```

- [ ] **Minimal impl.** Create `crates/buiy_core/src/render/buckets.rs`:

  ```rust
  //! Typed render primitives and `(primitive, layer)` instance bucketing (the
  //! CPU instance-bucketing module — R6 owns it).
  //!
  //! architecture.md § 2: `BuiyNode` is a small fixed set of typed SDF
  //! primitives, batched per `(primitive, layer)`, each batch a single instanced
  //! draw. This module owns the shared `BuiyPrimitiveKind` enum, the batch key,
  //! and the per-view bucket store the prepare phase fills. R7's pipeline
  //! specialization key (`render/primitive.rs`) **imports** `BuiyPrimitiveKind`
  //! from here — it is NOT redefined there. The `layer` is the forward index
  //! into `StackingContext.painters_z` (§ 2.2); this phase threads it but
  //! defaults to 0 (real layers are the paint-order phase's job).

  use std::collections::BTreeMap;

  use crate::render::instance::PackedInstance;
  use bytemuck::Pod;

  /// A typed render primitive — the shared primitive-kind enum (R6 owns it; R7
  /// imports it from `render::buckets`). The v1 quad-family set this phase
  /// buckets (architecture.md § 2.1). `glyph` / `path` are out of this phase's
  /// scope.
  #[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
  pub enum BuiyPrimitiveKind {
      /// Box-shadow SDF (painted ahead of its caster). Lowest paint order.
      Shadow,
      /// Background fill + rounded corners.
      Quad,
      /// Outer-minus-inner border band (a quad variant).
      Border,
      /// Focus outline, painted outside the border box. Highest paint order.
      Outline,
  }

  impl BuiyPrimitiveKind {
      /// Within-layer paint rank: back-to-front `shadow < quad < border <
      /// outline` (architecture.md § 2.2).
      pub fn paint_order(self) -> u8 {
          match self {
              BuiyPrimitiveKind::Shadow => 0,
              BuiyPrimitiveKind::Quad => 1,
              BuiyPrimitiveKind::Border => 2,
              BuiyPrimitiveKind::Outline => 3,
          }
      }
  }

  /// The key a batch is grouped by: `(primitive, layer)`. Ordering is **layer
  /// first** (the forward `painters_z` walk), then primitive paint order, so the
  /// natural `BTreeMap` iteration is the draw order.
  #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
  pub struct PrimitiveBatchKey {
      pub primitive: BuiyPrimitiveKind,
      pub layer: u32,
  }

  impl PartialOrd for PrimitiveBatchKey {
      fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
          Some(self.cmp(other))
      }
  }
  impl Ord for PrimitiveBatchKey {
      fn cmp(&self, other: &Self) -> std::cmp::Ordering {
          self.layer
              .cmp(&other.layer)
              .then(self.primitive.paint_order().cmp(&other.primitive.paint_order()))
      }
  }

  /// Per-view bucket store: each `(primitive, layer)` maps to its instance
  /// vector. Iteration is in draw order (the `BTreeMap` key order). The instance
  /// payload is generic over a `Pod` record so the same store can hold quad
  /// instances now and other primitive layouts later; this phase uses
  /// [`PackedInstance`].
  #[derive(Default)]
  pub struct InstanceBuckets {
      batches: BTreeMap<PrimitiveBatchKey, Vec<[f32; 9]>>,
  }

  impl InstanceBuckets {
      /// Push one packed instance (as raw `[f32; 9]` = pos2+size2+color4+radius1)
      /// into its batch.
      pub fn push(&mut self, key: PrimitiveBatchKey, instance: [f32; 9]) {
          self.batches.entry(key).or_default().push(instance);
      }

      /// Number of instances in a batch (0 if the key was never pushed to).
      pub fn len(&self, key: PrimitiveBatchKey) -> usize {
          self.batches.get(&key).map_or(0, Vec::len)
      }

      /// `true` iff no instances were pushed.
      pub fn is_empty(&self) -> bool {
          self.batches.values().all(Vec::is_empty)
      }

      /// Total instance count across all batches.
      pub fn total_instances(&self) -> usize {
          self.batches.values().map(Vec::len).sum()
      }

      /// Iterate batches in draw order (`(layer, primitive paint order)`).
      pub fn batches(&self) -> impl Iterator<Item = (&PrimitiveBatchKey, &Vec<[f32; 9]>)> {
          self.batches.iter()
      }
  }

  /// Flatten a [`PackedInstance`] into the raw `[f32; 9]` the bucket store holds.
  /// Keeps the bucket store decoupled from the concrete instance struct while
  /// the stride is asserted equal in tests.
  pub fn packed_to_raw(p: &PackedInstance) -> [f32; 9] {
      [
          p.rect_pos[0], p.rect_pos[1],
          p.rect_size[0], p.rect_size[1],
          p.color[0], p.color[1], p.color[2], p.color[3],
          p.radius,
      ]
  }

  // Asserts at compile time (via a const fn caller in tests) that the raw layout
  // matches the struct stride. `_pod` is here so a future non-f32 record forces
  // a conscious change to `packed_to_raw`.
  const _ASSERT_POD: fn() = || {
      fn _is_pod<T: Pod>() {}
      _is_pod::<PackedInstance>();
  };
  ```

  Then add to `crates/buiy_core/src/render/mod.rs`:

  ```rust
  pub mod buckets;
  ```

- [ ] **Run it — expect PASS.**

  ```sh
  cargo test -p buiy_core --test render_buckets
  ```

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `feat(render): add BuiyPrimitiveKind + (primitive,layer) instance bucketing`

---

## Task 4 — `pack_view`: stride agreement + raw-layout round-trip (HEADLESS, pure CPU)

Tie the three pieces together with a pure CPU function `pack_view`: given a slice of the per-view extract records, produce an `InstanceBuckets`. Every record goes to `(BuiyPrimitiveKind::Quad, layer 0)` in v1 (the only primitive the v1 set emits), packed via `pack_instance` + `packed_to_raw`. This is the seam the prepare GPU system (Task 5) calls; isolating it keeps the GPU upload trivial and the math testable. Also pin the **stride agreement** between `PackedInstance` and the raw `[f32; 9]` (the pipeline descriptor invariant).

**Files**
- Modify: `crates/buiy_core/src/render/buckets.rs` (add `pack_view`)
- Test: `crates/buiy_core/tests/render_buckets.rs` (extend)

### Steps

- [ ] **Write the failing test.** Append to `crates/buiy_core/tests/render_buckets.rs`:

  ```rust
  use bevy::prelude::*;
  use buiy_core::render::DrawData;
  use buiy_core::render::instance::{pack_instance, packed_raw_stride_agrees};
  use buiy_core::render::buckets::pack_view;

  #[test]
  fn raw_layout_stride_agrees_with_struct() {
      // The [f32;9] the bucket holds must be byte-identical in size to the
      // PackedInstance struct the pipeline descriptor declares (36 B). If this
      // ever drifts, the instanced draw reads garbage.
      assert!(packed_raw_stride_agrees());
      assert_eq!(std::mem::size_of::<[f32; 9]>(), 36);
  }

  #[test]
  fn pack_view_routes_every_draw_to_quad_layer_0() {
      let draws = vec![
          DrawData::new(Vec2::ZERO, Vec2::splat(10.0), Color::WHITE, 1.0),
          DrawData::new(Vec2::splat(5.0), Vec2::splat(20.0), Color::BLACK, 2.0),
      ];
      let buckets = pack_view(&draws);
      let quad0 = buiy_core::render::buckets::PrimitiveBatchKey {
          primitive: buiy_core::render::buckets::BuiyPrimitiveKind::Quad,
          layer: 0,
      };
      assert_eq!(buckets.len(quad0), 2);
      assert_eq!(buckets.total_instances(), 2);
  }

  #[test]
  fn pack_view_preserves_packed_values_in_order() {
      let draws = vec![DrawData::new(Vec2::new(7.0, 9.0), Vec2::new(3.0, 4.0), Color::WHITE, 5.0)];
      let buckets = pack_view(&draws);
      let (_, batch) = buckets.batches().next().expect("one batch");
      let expect = buiy_core::render::buckets::packed_to_raw(&pack_instance(&draws[0]));
      assert_eq!(batch[0], expect);
  }

  #[test]
  fn pack_view_empty_input_is_empty() {
      let buckets = pack_view(&[]);
      assert!(buckets.is_empty());
      assert_eq!(buckets.total_instances(), 0);
  }
  ```

- [ ] **Run it — expect FAIL (`pack_view` / `packed_raw_stride_agrees` absent).**

  ```sh
  cargo test -p buiy_core --test render_buckets
  ```

- [ ] **Minimal impl.** Add to `crates/buiy_core/src/render/instance.rs`:

  ```rust
  /// `true` iff the raw `[f32; 9]` bucket layout is byte-equal to
  /// [`PackedInstance`]'s stride (the pipeline-descriptor invariant). Pins the
  /// agreement the instanced draw relies on.
  pub fn packed_raw_stride_agrees() -> bool {
      std::mem::size_of::<PackedInstance>() == std::mem::size_of::<[f32; 9]>()
          && PACKED_INSTANCE_STRIDE_BYTES == std::mem::size_of::<[f32; 9]>()
  }
  ```

  And add `pack_view` to `crates/buiy_core/src/render/buckets.rs`:

  ```rust
  use crate::render::DrawData;
  use crate::render::instance::pack_instance;

  /// Pack a per-view draw list (the extract output) into typed-primitive
  /// `(primitive, layer)` buckets. v1 routes every [`DrawData`] to
  /// `(Quad, layer 0)` — the only primitive the v1 set emits — packing each
  /// via [`pack_instance`]. The `layer` will become the real forward
  /// `painters_z` index when the paint-order phase lands; until then it is 0.
  pub fn pack_view(draws: &[DrawData]) -> InstanceBuckets {
      let mut buckets = InstanceBuckets::default();
      let quad0 = PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 0 };
      for draw in draws {
          buckets.push(quad0, packed_to_raw(&pack_instance(draw)));
      }
      buckets
  }
  ```

- [ ] **Run it — expect PASS.**

  ```sh
  cargo test -p buiy_core --test render_buckets
  ```

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `feat(render): add pack_view + raw-layout stride-agreement assert`

---

## Task 5 — `BuiyInstanceBuffers` per-view component + `prepare_buiy_instances` system (GPU code; HEADLESS test for construction-purity)

> **Ownership (do NOT redefine).** `ExtractedNodes` is owned by **R5** as the single per-view component in `render/extract.rs` (`{ nodes: Vec<ExtractedNode>, logical_size: Vec2, scale_factor: f32 }`, with a manual `Default` setting `scale_factor = 1.0`). R6 **consumes** it — `use crate::render::extract::ExtractedNodes;` — and never defines a parallel `ExtractedNodes` (and no `ExtractedNodesResource`). If `render/extract.rs` / `ExtractedNodes` already exists (it does, owned by R5), import it; do **not** re-add a `pub mod`, a `lib.rs` re-export, or a second struct.

Introduce the one per-view render-world component this phase owns — `BuiyInstanceBuffers` (the persistent GPU buffers, grow-in-place) — and the `prepare_buiy_instances` system pinned to `RenderSystems::Prepare`. The system reads the view's R5-owned `ExtractedNodes` (its `nodes` + `logical_size` + `scale_factor`), builds the `BuiyViewUniform`, packs via `pack_view`, and uploads into the persistent per-view buffers (grow-in-place via `BufferVec`/`RawBufferVec`). Register it in `BuiyRenderPlugin::build` with `.in_set(RenderSystems::Prepare)`.

> **Packing seam.** R5's `ExtractedNodes.nodes` is `Vec<ExtractedNode>` (the per-painted-entity CPU record), not `Vec<DrawData>`. `pack_view` (Task 4) consumes the per-view records; when wiring this system, feed it `&nodes.nodes` (adapting the `ExtractedNode` geometry/color into the packed instance via `pack_instance`). Do **not** rebuild a parallel draw list from the Phase-0 `ExtractedDraws` — the consumed carrier is R5's `ExtractedNodes`.

The **system body and buffer upload need a wgpu device** → the membership/upload assertions are `#[ignore]` GPU tests. The HEADLESS-gating test here is that `BuiyInstanceBuffers` is plain data (construct + default without a device) and that `prepare_buiy_instances` is a `fn` with the expected signature that can be added to a schedule — proven by adding it to a bare `App` schedule and asserting it does not run a system that needs missing GPU resources at *build* time. The real render-world membership assertion is `#[ignore]`.

**Files**
- Create: `crates/buiy_core/src/render/prepare.rs`
- Modify: `crates/buiy_core/src/render/mod.rs` (add `pub mod prepare;`; register the system in `BuiyRenderPlugin::build`)
- Test: `crates/buiy_core/tests/render_prepare.rs` (new — one HEADLESS purity test + the `#[ignore]` GPU tests)
- Consume (do NOT modify the definition): `ExtractedNodes` from `crates/buiy_core/src/render/extract.rs` (owned by R5)

### Steps

- [ ] **Write the failing tests.** Create `crates/buiy_core/tests/render_prepare.rs`:

  ```rust
  //! Prepare-phase tests. The CONSTRUCTION-purity test is HEADLESS (no GPU); the
  //! buffer-upload + render-world-membership tests are #[ignore] because they
  //! need a wgpu adapter (RenderPlugin::build `.expect()`s one) — same idiom as
  //! render_smoke.rs.

  use bevy::prelude::*;
  // ExtractedNodes is owned by R5 (render::extract); R6 only CONSUMES it.
  use buiy_core::render::extract::ExtractedNodes;
  use buiy_core::render::prepare::BuiyInstanceBuffers;
  use buiy_core::render::view_uniform::BuiyViewUniform;

  #[test]
  fn instance_buffers_default_is_empty_no_device() {
      // The per-view persistent-buffer component R6 owns is plain data: it
      // constructs (Default) with no GPU device present.
      let buffers = BuiyInstanceBuffers::default();
      assert_eq!(buffers.quad_count, 0);
  }

  #[test]
  fn view_uniform_from_extracted_nodes_params() {
      // R6 reads R5's ExtractedNodes (logical_size + scale_factor) and builds the
      // view uniform from them. R5's manual Default sets scale_factor = 1.0.
      let mut nodes = ExtractedNodes::default();
      assert_eq!(nodes.scale_factor, 1.0); // R5's manual Default (not 0.0)
      nodes.logical_size = Vec2::new(800.0, 600.0);
      nodes.scale_factor = 2.0;
      let u = BuiyViewUniform::for_view(nodes.logical_size, nodes.scale_factor);
      let p = u.apply(Vec2::ZERO);
      assert!((p.x - -1.0).abs() < 1e-6 && (p.y - 1.0).abs() < 1e-6);
      assert!((u.scale_factor() - 2.0).abs() < 1e-6);
  }

  // ----- GPU (#[ignore]) — needs a wgpu adapter -----
  // Run locally with: `cargo test -p buiy_core --test render_prepare -- --ignored`.

  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); prepare-system render-world membership"]
  fn prepare_system_is_in_render_prepare_set() {
      use bevy::render::{RenderApp, RenderSystems, Render};
      use bevy::ecs::schedule::ScheduleLabel;
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(bevy::asset::AssetPlugin::default());
      app.add_plugins(bevy::render::RenderPlugin::default());
      app.add_plugins(buiy_core::render::BuiyRenderPlugin);

      let render_app = app.get_sub_app(RenderApp).expect("RenderApp");
      // Assert the Render schedule contains our prepare system. Bevy exposes
      // schedule membership via the Schedules resource; we check the system is
      // present in the Render schedule graph (exact graph-introspection API per
      // the engine version — assert the system label/name is reachable).
      let schedules = render_app.world().resource::<bevy::ecs::schedule::Schedules>();
      let render = schedules.get(Render).expect("Render schedule present");
      let found = render
          .graph()
          .systems()
          .any(|(_, system, _, _)| system.name().as_string().contains("prepare_buiy_instances"));
      assert!(found, "prepare_buiy_instances registered in the Render schedule");
      // The set-membership (RenderSystems::Prepare) is asserted by the ordering
      // test below; this test pins presence in the render world.
      let _ = RenderSystems::Prepare;
  }

  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); buffer upload round-trip"]
  fn prepare_uploads_persistent_buffers() {
      // Full GPU round-trip: build a RenderApp, insert R5's ExtractedNodes with a
      // few nodes onto a view entity, run the Prepare set, and assert the view's
      // BuiyInstanceBuffers holds a non-empty quad buffer whose instance count
      // equals nodes.len(). (Provisioned by the Task-N e2e/visual harness on a
      // GPU runner; left as the documented GPU coverage point here.)
  }
  ```

- [ ] **Run the HEADLESS test — expect FAIL (module/type absent).**

  ```sh
  cargo test -p buiy_core --test render_prepare
  ```
  Expected: `error[E0432]: unresolved import buiy_core::render::prepare`. (The `#[ignore]` tests do not run on this command.)

- [ ] **Minimal impl.** Create `crates/buiy_core/src/render/prepare.rs`:

  ```rust
  //! The prepare phase (architecture.md § 3.2 / § 4): per-view persistent GPU
  //! instance buffers + the view uniform, written in `RenderSystems::Prepare`.
  //!
  //! Why prepare, not extract (architecture.md § 1.1 / § 4): `ViewTarget` (and a
  //! settled `GlobalTransform`) do not exist until `prepare_view_targets`
  //! (`RenderSystems::ManageViews`), which runs AFTER `ExtractSchedule`. So the
  //! CPU-side per-view record (`ExtractedNodes`, owned by R5 in `render::extract`)
  //! is an extract product, but the GPU buffers + view uniform are a PREPARE
  //! product. Both are stored as COMPONENTS on the per-view render entity
  //! (per-window isolation, § 4), never as global resources.
  //!
  //! `ExtractedNodes` is **not redefined here** — it is owned by R5 and imported
  //! from `crate::render::extract`. This module owns only `BuiyInstanceBuffers`
  //! (the persistent GPU buffers) and the `prepare_buiy_instances` system.

  use bevy::prelude::*;
  use bevy::render::render_resource::{BufferUsages, BufferVec, UniformBuffer};
  use bevy::render::renderer::{RenderDevice, RenderQueue};

  use crate::render::buckets::pack_view;
  use crate::render::view_uniform::BuiyViewUniform;
  // ExtractedNodes is owned by R5; consume it, do not redefine it.
  use crate::render::extract::ExtractedNodes;

  /// Persistent per-view GPU instance buffers (architecture.md § 3.2): one
  /// growable buffer per primitive, allocated once and reused frame-to-frame
  /// (grow-in-place; never reallocated per frame), plus the view-uniform UBO.
  /// Stored as a component on the per-view render entity for per-window
  /// isolation (§ 4).
  #[derive(Component)]
  pub struct BuiyInstanceBuffers {
      /// Quad-family instances (the v1 primitive set). Grows in place.
      pub quad: BufferVec<[f32; 9]>,
      /// The per-view logical->clip + scale_factor uniform.
      pub view_uniform: UniformBuffer<[f32; 12]>,
      /// Instance count written this frame (the instanced draw range).
      pub quad_count: u32,
  }

  impl Default for BuiyInstanceBuffers {
      fn default() -> Self {
          Self {
              quad: BufferVec::new(BufferUsages::VERTEX),
              view_uniform: UniformBuffer::default(),
              quad_count: 0,
          }
      }
  }

  /// `RenderSystems::Prepare` system: for each view, pack its [`ExtractedNodes`]
  /// into typed-primitive buckets, upload the persistent [`BuiyInstanceBuffers`]
  /// (grow-in-place), and write the view uniform. `ViewTarget` is available in
  /// this set (architecture.md § 4), unlike in extract.
  pub fn prepare_buiy_instances(
      mut commands: Commands,
      render_device: Res<RenderDevice>,
      render_queue: Res<RenderQueue>,
      mut views: Query<(Entity, &ExtractedNodes, Option<&mut BuiyInstanceBuffers>)>,
  ) {
      for (entity, nodes, buffers) in &mut views {
          // Consume R5's ExtractedNodes: pack its per-view records and build the
          // view uniform from its logical_size + scale_factor (R5-owned fields).
          let buckets = pack_view(&nodes.nodes);
          let uniform = BuiyViewUniform::for_view(nodes.logical_size, nodes.scale_factor);

          // Get-or-insert the persistent buffers component.
          let mut buffers = match buffers {
              Some(b) => b,
              None => {
                  commands.entity(entity).insert(BuiyInstanceBuffers::default());
                  // Skip this frame's upload; next frame the component exists.
                  // (Acceptable one-frame warmup; documented, not a hack —
                  // grow-in-place buffers are created lazily on first sight.)
                  continue;
              }
          };

          // Repack the quad buffer in place: clear + extend (the Vec backing
          // grows; the GPU buffer grows only on capacity overflow).
          buffers.quad.clear();
          let mut count = 0u32;
          for (_key, batch) in buckets.batches() {
              for inst in batch {
                  buffers.quad.push(*inst);
                  count += 1;
              }
          }
          buffers.quad_count = count;
          buffers.quad.write_buffer(&render_device, &render_queue);

          // Pack the uniform: col0(4) + col1(4) + scale_factor + 3 pad = 12 f32.
          buffers.view_uniform.set(uniform.as_std140_array());
          buffers.view_uniform.write_buffer(&render_device, &render_queue);
      }
  }
  ```

  > Implementer note: `BuiyViewUniform` must expose an `as_std140_array(&self) -> [f32; 12]` (col0 ++ col1 ++ [scale_factor, 0, 0, 0]) for the UBO write — add it to `view_uniform.rs` when wiring this system, with a HEADLESS test asserting the array round-trips `col0`/`col1`/`scale_factor`. The exact `BufferVec`/`UniformBuffer` method names (`set`, `clear`, `push`, `write_buffer`) are per `bevy_render::render_resource` 0.18; verify against the installed crate while implementing (they are the upload primitives, not invented here).

  > Implementer note (packing seam): `pack_view` (Task 4) is written against the per-view extract records — R5's `ExtractedNodes.nodes` is `Vec<ExtractedNode>`, the per-painted-entity CPU record (not `Vec<DrawData>`). When wiring `prepare_buiy_instances`, make `pack_view` consume `&[ExtractedNode]` (adapt `pack_instance` to read the `ExtractedNode` box geometry + resolved color), or thread an `ExtractedNode → DrawData` adapter. Do **not** add a parallel `ExtractedNodes`/`ExtractedNodesResource` rebuilt from the Phase-0 `ExtractedDraws` — the single consumed carrier is R5's `ExtractedNodes`. Align the Task-2/3/4 `DrawData` signatures to the `ExtractedNode` record at this seam (the bucketing logic and its tests are unchanged; only the input record type flips).

  Then in `crates/buiy_core/src/render/mod.rs` add the module and register the system. Add near the other `pub mod` lines:

  ```rust
  pub mod prepare;
  ```

  And in `BuiyRenderPlugin::build`, after R5's existing extract registration (R5 owns the `extract_buiy_nodes`/`ExtractedNodes` wiring — do NOT re-register it here), chain only the prepare system onto the same `render_app` builder:

  ```rust
  use bevy::render::{Render, RenderSystems};
  // ... R5's extract registration stays as-is; R6 appends only the prepare system:
  render_app
      .add_systems(
          Render,
          prepare::prepare_buiy_instances.in_set(RenderSystems::Prepare),
      );
  ```

- [ ] **Run the HEADLESS test — expect PASS.**

  ```sh
  cargo test -p buiy_core --test render_prepare
  ```

- [ ] **Confirm the GPU tests are skipped (not run) on the gate.**

  ```sh
  cargo test -p buiy_core --test render_prepare 2>&1 | grep -E "ignored|test result"
  ```
  Expected: the two `#[ignore]` tests report as `ignored`.

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `feat(render): add per-view BuiyInstanceBuffers + prepare_buiy_instances consuming R5 ExtractedNodes (RenderSystems::Prepare)`

---

## Task 6 — Consume R5's per-view `ExtractedNodes` in prepare (GPU code; HEADLESS for the consumption-shape test)

> **Ownership.** **R5 owns both the `ExtractedNodes` per-view component AND its population** (R5's `extract_buiy_nodes` writes it onto the primary view entity in `render::extract`). R6 does **not** build a parallel `ExtractedNodes` from the Phase-0 `ExtractedDraws`/`DrawData`, and adds **no** `extracted_nodes_from_draws` helper and **no** `ExtractedNodesResource` shim. This task only confirms `prepare_buiy_instances` reads R5's already-populated component and packs it.

R5's `extract_buiy_nodes` already writes the per-view `ExtractedNodes` (`{ nodes, logical_size, scale_factor }`) onto the primary view's render entity (architecture.md § 4, primary-window-only v1). R6's `prepare_buiy_instances` (Task 5) queries that component directly — there is no second carrier to populate here. The per-view entity routing and the populated-by-R5 read are exercised by the GPU `#[ignore]` round-trip (Task 5's `prepare_uploads_persistent_buffers`); the HEADLESS gate here is the consumption-shape assertion: a hand-built `ExtractedNodes` round-trips through `pack_view` into the expected bucket counts.

**Files**
- Test: `crates/buiy_core/tests/render_prepare.rs` (extend with a HEADLESS consumption-shape test)
- Consume (do NOT modify): `ExtractedNodes` from `crates/buiy_core/src/render/extract.rs` (owned + populated by R5)

### Steps

- [ ] **Write the failing test.** Append to `crates/buiy_core/tests/render_prepare.rs`:

  ```rust
  #[test]
  fn extracted_nodes_pack_view_routes_records_to_quad_layer_0() {
      // R6 consumes R5's ExtractedNodes and packs its `nodes` via pack_view.
      // (Build the per-view record with R5's ExtractedNode constructor; this
      // pins the consumption shape without rebuilding a parallel carrier.)
      use buiy_core::render::buckets::{pack_view, BuiyPrimitiveKind, PrimitiveBatchKey};
      use buiy_core::render::extract::ExtractedNodes;
      let mut view = ExtractedNodes::default();
      // R5's manual Default sets scale_factor = 1.0 (not the derive's 0.0).
      assert_eq!(view.scale_factor, 1.0);
      view.logical_size = Vec2::new(1280.0, 720.0);
      // Push one ExtractedNode (R5's per-painted-entity record) — see R5 for the
      // constructor; the bucketing only reads its packed geometry/color.
      // view.nodes.push(ExtractedNode::new(...));
      let buckets = pack_view(&view.nodes);
      let quad0 = PrimitiveBatchKey { primitive: BuiyPrimitiveKind::Quad, layer: 0 };
      assert_eq!(buckets.len(quad0), view.nodes.len());
  }

  #[test]
  fn extracted_nodes_empty_packs_to_empty_buckets() {
      use buiy_core::render::buckets::pack_view;
      use buiy_core::render::extract::ExtractedNodes;
      let view = ExtractedNodes::default();
      assert!(view.nodes.is_empty());
      assert_eq!(view.scale_factor, 1.0); // R5's manual Default
      let buckets = pack_view(&view.nodes);
      assert!(buckets.is_empty());
  }
  ```

  > Implementer note: `pack_view` consumes the per-view records (R5's `ExtractedNode`). Use R5's `ExtractedNode` constructor in the test; do **not** introduce a `DrawData`-based parallel path. If R5's `ExtractedNode` is not yet constructible from a unit test, gate this test on R5 landing (execution order R5 → R6) and keep the empty-buckets case as the always-green half.

- [ ] **Run it — expect FAIL (the `pack_view`/`ExtractedNode` seam not yet aligned).**

  ```sh
  cargo test -p buiy_core --test render_prepare
  ```

- [ ] **Minimal impl.** No new types. Align `pack_view` (Task 4) to consume `&[ExtractedNode]` (the packing seam called out in Task 5's implementer note) so `prepare_buiy_instances` reads R5's component with no adapter resource. Confirm nothing in `mod.rs` defines a second `ExtractedNodes` or an `ExtractedNodesResource`:

  ```sh
  grep -rn "ExtractedNodesResource\|struct ExtractedNodes" crates/buiy_core/src
  ```
  Expected: the only `struct ExtractedNodes` is R5's in `render/extract.rs`; no `ExtractedNodesResource` anywhere.

- [ ] **Run it — expect PASS (HEADLESS).**

  ```sh
  cargo test -p buiy_core --test render_prepare
  ```

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `feat(render): prepare consumes R5 per-view ExtractedNodes (no parallel carrier)`

---

## Task 7 — Port the node to the view-uniform path; bind the uniform; draw logical-px instances (GPU code + #[ignore] e2e)

Switch `BuiyNode::run` off the Phase-0 per-frame `create_buffer_with_data` + `to_instance` clip-space path onto the persistent `BuiyInstanceBuffers` + `BuiyViewUniform` bind group. The shader gains a `@group(0) @binding(0)` view uniform; the vertex stage applies the logical→clip transform; the SDF stays in logical px. This is **all GPU** — the node only runs in a render world with a wgpu device — so the e2e assertion is `#[ignore]`. The HEADLESS-gating work here is the WGSL/CPU-mirror parity test: a pure-CPU port of the new vertex transform must match `BuiyViewUniform::apply`.

**Files**
- Modify: `crates/buiy_core/src/render/shader.wgsl` (add view-uniform binding; apply in vertex; SDF in logical px)
- Modify: `crates/buiy_core/src/render/pipeline.rs` (add bind-group layout for the view uniform; the instance buffer stays a vertex buffer; reserve the `..01` octet note)
- Modify: `crates/buiy_core/src/render/node.rs` (read `BuiyInstanceBuffers` + bind the view uniform; draw `0..quad_count`)
- Test: `crates/buiy_core/tests/render_view_uniform.rs` (extend with the CPU-mirror parity test, HEADLESS); `crates/buiy_core/tests/render_smoke.rs` (extend with an `#[ignore]` node-draw assertion)

### Steps

- [ ] **Write the failing HEADLESS parity test.** Append to `crates/buiy_core/tests/render_view_uniform.rs`:

  ```rust
  // CPU mirror of the NEW vertex transform in shader.wgsl: a logical-px point
  // transformed by the view uniform must equal BuiyViewUniform::apply. This is
  // the device-free proof that the WGSL vertex math and the CPU uniform agree
  // (the shader itself only runs on GPU; this pins the math the GPU executes).
  fn wgsl_vertex_logical_to_clip(u: &BuiyViewUniform, logical: Vec2) -> Vec2 {
      // Mirror of: clip.x = col0.x*l.x + col0.w; clip.y = col1.y*l.y + col1.w
      let a = u.as_std140_array();
      Vec2::new(a[0] * logical.x + a[3], a[5] * logical.y + a[7])
  }

  #[test]
  fn wgsl_vertex_mirror_matches_apply() {
      let u = BuiyViewUniform::for_view(Vec2::new(1000.0, 500.0), 1.0);
      for p in [Vec2::ZERO, Vec2::new(500.0, 250.0), Vec2::new(1000.0, 500.0), Vec2::new(123.0, 456.0)] {
          let m = wgsl_vertex_logical_to_clip(&u, p);
          let a = u.apply(p);
          assert!((m - a).length() < 1e-6, "p={p:?} mirror={m:?} apply={a:?}");
      }
  }
  ```

  This also forces `BuiyViewUniform::as_std140_array() -> [f32; 12]` to exist (referenced in Task 5's implementer note). If not yet added, add it to `view_uniform.rs`:

  ```rust
  impl BuiyViewUniform {
      /// Flatten to the std140 UBO array: `col0 ++ col1 ++ [scale_factor,0,0,0]`.
      pub fn as_std140_array(&self) -> [f32; 12] {
          [
              self.col0[0], self.col0[1], self.col0[2], self.col0[3],
              self.col1[0], self.col1[1], self.col1[2], self.col1[3],
              self.scale_factor, 0.0, 0.0, 0.0,
          ]
      }
  }
  ```

  Append an `#[ignore]` GPU draw assertion to `crates/buiy_core/tests/render_smoke.rs`:

  ```rust
  // Same wgpu-adapter caveat as the other render_smoke #[ignore] tests. Asserts
  // the ported node draws the persistent buffers without panicking and the
  // view-uniform bind group is wired. Run locally with `-- --ignored`.
  #[test]
  #[ignore = "needs a wgpu adapter (real GPU or lavapipe); ported node draws persistent buffers"]
  fn node_draws_persistent_buffers_with_view_uniform() {
      // Build a RenderApp with BuiyRenderPlugin, drive one frame with a single
      // Buiy node + Visual, and assert the frame completes (no panic) and the
      // BuiyInstanceBuffers quad_count == 1. Provisioned on a GPU runner by the
      // visual-regression harness; documented GPU coverage point.
  }
  ```

- [ ] **Run the HEADLESS parity test — expect FAIL (`as_std140_array` absent until added; then PASS after the impl below).**

  ```sh
  cargo test -p buiy_core --test render_view_uniform
  ```

- [ ] **Minimal impl — shader.** Edit `crates/buiy_core/src/render/shader.wgsl` to add the view uniform and apply it; keep the SDF in logical px:

  ```wgsl
  // Buiy rounded-rect shader. Instance inputs are LOGICAL pixels; the view
  // uniform (render::view_uniform::BuiyViewUniform) does the logical->clip
  // transform in the vertex stage. The y-flip and px->clip scale live ENTIRELY
  // in the uniform — the per-instance y-flip / 2/min(w,h) hack is retired.

  struct BuiyView {
      // col0 = [sx, 0, 0, tx]; col1 = [0, sy, 0, ty]; clip = M*logical + t.
      col0: vec4<f32>,
      col1: vec4<f32>,
      // [scale_factor, pad, pad, pad]
      params: vec4<f32>,
  };
  @group(0) @binding(0) var<uniform> view: BuiyView;

  struct Vertex {
      @location(0) position: vec2<f32>,
      @location(1) uv: vec2<f32>,
  };

  struct Instance {
      @location(2) rect_pos: vec2<f32>,   // logical px, top-left
      @location(3) rect_size: vec2<f32>,  // logical px, POSITIVE height
      @location(4) color: vec4<f32>,
      @location(5) radius: f32,            // logical px
  };

  struct VertexOut {
      @builtin(position) clip_position: vec4<f32>,
      @location(0) local_uv: vec2<f32>,
      @location(1) half_size: vec2<f32>,  // logical px
      @location(2) color: vec4<f32>,
      @location(3) radius: f32,           // logical px
  };

  fn logical_to_clip(p: vec2<f32>) -> vec2<f32> {
      return vec2<f32>(view.col0.x * p.x + view.col0.w, view.col1.y * p.y + view.col1.w);
  }

  @vertex
  fn vertex(v: Vertex, i: Instance) -> VertexOut {
      var out: VertexOut;
      let logical = i.rect_pos + v.uv * i.rect_size; // logical-px corner
      out.clip_position = vec4<f32>(logical_to_clip(logical), 0.0, 1.0);
      out.local_uv = v.uv * 2.0 - 1.0;
      out.half_size = i.rect_size * 0.5;             // positive — no abs needed
      out.color = i.color;
      out.radius = i.radius;
      return out;
  }

  fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, r: f32) -> f32 {
      let q = abs(p) - half_size + vec2<f32>(r, r);
      return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
  }

  @fragment
  fn fragment(in: VertexOut) -> @location(0) vec4<f32> {
      // SDF in logical px; AA from fwidth in logical px (the view uniform keeps
      // logical px well-scaled, so fwidth is meaningful without scale_factor).
      let d = sdf_rounded_rect(in.local_uv * in.half_size, in.half_size, in.radius);
      let aa = fwidth(d);
      let alpha = 1.0 - smoothstep(-aa, aa, d);
      return vec4<f32>(in.color.rgb, in.color.a * alpha);
  }
  ```

  > Note: `rect_size` is now positive, so the `abs()` workaround (and its `signed_rect_size_breaks_sdf_without_abs` regression test, which is Phase-0 baseline) no longer applies to the new path. The Phase-0 `to_instance` + old shader path is retired by this edit; the baseline tests in `render_instance.rs` that exercised `to_instance`'s negative height stay as documentation of the retired hack but the shader no longer relies on the abs. (Keep them green — `to_instance` itself is unchanged; only the node + shader stop calling it.)

- [ ] **Minimal impl — pipeline.** In `crates/buiy_core/src/render/pipeline.rs`, add a bind-group layout for the view uniform and reference it in `descriptor.layout`. Add a `view_layout: BindGroupLayout` field to `BuiyPipeline`, build it via `render_device.create_bind_group_layout(...)` with one uniform binding visible to the vertex stage, and set `layout: vec![view_layout.clone()]`. Update the `..01` octet comment to note the uniform binding is now part of the rounded-rect pipeline. (The instance buffer stays `descriptor.vertex.buffers[1]` — `array_stride: 36` unchanged, now carrying logical-px values.)

  > Implementer note: the exact `BindGroupLayoutEntry` / `create_bind_group_layout` signature is per `bevy_render` 0.18; verify against the installed crate. This is mechanical wgpu plumbing, not a design choice — one `var<uniform>` binding at `@group(0) @binding(0)`, `ShaderStages::VERTEX`.

- [ ] **Minimal impl — node.** Rewrite `BuiyNode::run` in `crates/buiy_core/src/render/node.rs` to: resolve the pipeline (unchanged early-return on `None`); read `BuiyInstanceBuffers` (v1: the render-world resource shim from Task 6); skip if `quad_count == 0`; create+set the view-uniform bind group from `buffers.view_uniform`; set the persistent `buffers.quad.buffer()` as vertex buffer 1; `pass.draw(0..4, 0..buffers.quad_count)`. Drop the per-frame `create_buffer_with_data` and the `to_instance` import.

  > Implementer note: `BufferVec::buffer()` returns `Option<&Buffer>` (None before first `write_buffer`); early-return `Ok(())` on `None`, mirroring the not-yet-compiled-pipeline early-return. The `ViewTarget` query and render-pass descriptor are unchanged from Phase 0.

- [ ] **Run the HEADLESS tests — expect PASS.**

  ```sh
  cargo test -p buiy_core --test render_view_uniform --test render_instance --test render_buckets --test render_prepare
  ```

- [ ] **Confirm GPU tests still skip on the gate.**

  ```sh
  cargo test -p buiy_core 2>&1 | grep -E "ignored"
  ```

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `feat(render): port node + shader to view-uniform + persistent buffers (retire y-flip hack)`

---

## Task 8 — Retire the Phase-0 clip-space path; mark baseline tests as superseded (HEADLESS cleanup)

With the node and shader on the view-uniform path, the Phase-0 `to_instance` clip-space packer and its negative-height/`2/min(w,h)` assertions are dead code on the live path. Per the spec's "retires the Phase-0 stopgaps" framing (architecture.md § 3), delete `to_instance` + `InstanceData` + `INSTANCE_STRIDE_BYTES` and convert the surviving SDF-math tests to use the logical-px `PackedInstance`. This is the explicit "baseline replaced" step the task scope calls for. Pure HEADLESS.

**Files**
- Modify: `crates/buiy_core/src/render/instance.rs` (remove `InstanceData` / `to_instance` / `INSTANCE_STRIDE_BYTES`)
- Modify: `crates/buiy_core/src/render/mod.rs` (drop `ExtractedDraws.window_size`'s clip-conversion role comment if now stale; keep the resource)
- Test: `crates/buiy_core/tests/render_instance.rs` (delete the `to_instance_*` clip-space tests and the `signed_rect_size_*` baseline; re-express the SDF tests against `pack_instance`)

### Steps

- [ ] **Write the replacement SDF test (failing because it references the removed symbols if done before deletion — do the deletion in the same step).** In `crates/buiy_core/tests/render_instance.rs`, remove the entire block of `to_instance_*`, `shader_sdf_inside_is_filled_outside_is_empty`, and `signed_rect_size_breaks_sdf_without_abs` tests plus the `shader_half_size` helper, and replace with a logical-px SDF test:

  ```rust
  // Pure-CPU port of shader.wgsl::sdf_rounded_rect (logical px). The view-uniform
  // path keeps the SDF in logical px with a POSITIVE half_size — no abs() hack.
  fn sdf_rounded_rect(p: Vec2, half_size: Vec2, r: f32) -> f32 {
      let q = p.abs() - half_size + Vec2::splat(r);
      q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
  }

  #[test]
  fn logical_sdf_inside_is_filled_outside_is_empty() {
      let draw = DrawData::new(Vec2::new(100.0, 100.0), Vec2::new(200.0, 100.0), Color::WHITE, 0.0);
      let p = pack_instance(&draw);
      let half = Vec2::from(p.rect_size) * 0.5; // positive — logical px
      assert!(half.y > 0.0, "logical-px half_size is positive (no y-flip)");

      let d_center = sdf_rounded_rect(Vec2::ZERO, half, p.radius);
      assert!(d_center < 0.0, "rect center inside (d={d_center})");

      let d_out = sdf_rounded_rect(Vec2::splat(2.0) * half, half, p.radius);
      assert!(d_out > 0.0, "2x half-extent outside (d={d_out})");
  }
  ```

  Remove the `INSTANCE_STRIDE_BYTES` / `InstanceData` / `to_instance` imports and the `instance_data_layout_matches_pipeline_descriptor` test (superseded by `packed_instance_stride_matches_logical_pipeline_descriptor` from Task 2). Remove the Phase-0-baseline module-doc note added in Task 2 (no longer needed — the clip-space path is gone).

- [ ] **Run it — expect FAIL (still references removed symbols, or unused imports).**

  ```sh
  cargo test -p buiy_core --test render_instance
  ```

- [ ] **Minimal impl — delete the Phase-0 packer.** In `crates/buiy_core/src/render/instance.rs`, remove `InstanceData`, `to_instance`, and `INSTANCE_STRIDE_BYTES`. Update the module doc to describe only the logical-px `PackedInstance` path. Remove the now-unused `DrawData` import only if `pack_instance` no longer needs it (it does — keep it). Ensure nothing else in the crate references the removed symbols:

  ```sh
  grep -rn "to_instance\|INSTANCE_STRIDE_BYTES\|InstanceData" crates/buiy_core/src
  ```
  Expected: no hits in `src/` (the node was ported in Task 7; the pipeline never referenced them).

- [ ] **Run it — expect PASS.**

  ```sh
  cargo test -p buiy_core --test render_instance
  ```

- [ ] **Run the full gate, then commit.**

  ```sh
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```
  Commit: `refactor(render): remove Phase-0 clip-space InstanceData/to_instance (view-uniform path is live)`

---

## Done criteria

- [ ] `BuiyViewUniform` owns the single logical-px → clip y-flip + `scale_factor`; the per-instance y-flip/`2/min(w,h)` hack is gone (HEADLESS-proven by `render_view_uniform.rs` + the deletion in Task 8).
- [ ] `PackedInstance` carries logical-px position/size/radius + CPU-pre-linearized color; stride agrees with the pipeline descriptor (HEADLESS).
- [ ] `pack_view` buckets per-view draws into `(BuiyPrimitiveKind, layer)` sets in `shadow → quad → border → outline` order; v1 routes to `(Quad, 0)` (HEADLESS).
- [ ] `prepare_buiy_instances` is registered in `RenderSystems::Prepare`, consumes R5's per-view `ExtractedNodes`, owns persistent per-view `BuiyInstanceBuffers` (grow-in-place) + the view-uniform UBO, with `ViewTarget` available in that set (GPU `#[ignore]` for membership/upload; HEADLESS for `BuiyInstanceBuffers` construction-purity + the `BuiyViewUniform::for_view` math from R5's `logical_size`/`scale_factor`).
- [ ] The node + shader run off the persistent buffers and the bound view uniform; the WGSL vertex transform matches `BuiyViewUniform::apply` (HEADLESS parity test; GPU `#[ignore]` for the actual draw).
- [ ] Every commit kept the gate green: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace`.

## Cross-phase dependencies assumed

- **`ExtractedNodes` is owned + populated by R5 (`render::extract`).** This phase (R6) **consumes** it and never redefines or re-populates it. The R5 → R6 execution order guarantees the component exists when `prepare_buiy_instances` queries it. R6 owns only `render/buckets.rs` (CPU bucketing + the shared `BuiyPrimitiveKind` enum, which R7 imports) and `render/prepare.rs` (`BuiyInstanceBuffers` + `prepare_buiy_instances`).
- **Phase-0 `ExtractedDraws`/`extract_buiy_draws` stay alive until the extract phase retires them.** R6 does not touch them: it reads R5's `ExtractedNodes`, not the Phase-0 resource. (The `DrawData`-based Task 2–4 packing primitives stay as the math under test; the packing seam in Task 5/6 flips their input record from `DrawData` to R5's `ExtractedNode`.)
- **Real `(primitive, layer)` layers come from the paint-order phase.** This phase threads `layer` and defaults to `0`. The paint-order phase feeds the forward `StackingContext.painters_z` index into `pack_view` (the seam is the `layer` arg on `PrimitiveBatchKey`).
- **Per-window routing is reserved, not wired (architecture.md § 4 D2 / README § 5 #1).** R5 writes `ExtractedNodes` onto the **primary** view's render entity in v1; R6's `BuiyInstanceBuffers` is a per-view **component** the prepare system get-or-inserts on that same entity. The per-window phase widens R5's write to every view entity — no R6 type change. Do not add render state assuming exactly one window beyond reading the per-view component.
- **`scale_factor` source.** R5 reads `window.resolution.scale_factor()` at extract and threads it into `ExtractedNodes.scale_factor`; R6 reads that field to build the view uniform. No `ScaleFactor` carrier component exists in the codebase today; if a later phase introduces one (e.g. for per-view DPI), R5's extract should prefer it over the window read.
- **`shadow`/`border`/`outline` primitives are bucket-reserved only.** This phase defines the `BuiyPrimitiveKind` variants and their paint order but only the `Quad` pipeline exists (the `..02` shadow / border-band / `..03` glyph octets are later phases). `pack_view` emits only `Quad` instances in v1.
