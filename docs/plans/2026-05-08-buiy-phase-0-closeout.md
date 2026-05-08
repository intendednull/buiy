# Buiy Phase 0 Closeout Implementation Plan

**Date:** 2026-05-08
**Status:** landed
**Spec:** [specs/2026-05-07-buiy-foundation/README.md](../specs/2026-05-07-buiy-foundation/README.md)
**Realizes:** Deferred items from [plans/2026-05-07-buiy-phase-0-foundations.md](2026-05-07-buiy-phase-0-foundations.md) self-review (lines 2784–2790).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the three substantive deferrals identified in the Phase 0 self-review — render pipeline doesn't actually draw, AccessKit adapter is a snapshot-only stub, `bevy_picking` is an AABB shim — plus flip the cosmetic `[draft]` status tags to `[landed]`. After this plan, `examples/hello_button` paints real pixels via Buiy's pipeline, a real screen reader attached to the window receives an AccessKit tree, and pointer events flow through `bevy_picking::pointer::PointerHits` while `Hovered` stays a thin convenience layer on top.

**PR shape:** One branch, one PR. Tasks below each commit independently for review readability; they all land together.

**Architecture:**
- *Render:* Extend `ExtractedDraws` with the primary-window logical size. Create a static unit-quad vertex buffer at pipeline-init time and store it on `BuiyPipeline`. In the render-graph node, build a per-frame instance buffer from `ExtractedDraws.draws + window_size`, doing logical-pixel → clip-space conversion CPU-side per the shader's path (a). Issue one instanced `draw(0..4, 0..n)`.
- *A11y:* Split `a11y.rs` into a folder. Factor the `A11yNodeView → accesskit::Node` translation into a pure function so it is unit-testable without a winit window. Add a per-window `AccessKitAdapters: HashMap<WindowId, accesskit_winit::Adapter>` resource. Lifecycle systems react to `WindowCreated`/`WindowClosed` and use `bevy::winit::WinitWindows` to look up the raw winit window handle. A push system in `BuiySet::A11yUpdate` translates the `A11yTreeBuilder` snapshot into `accesskit::TreeUpdate` and pushes to every adapter.
- *Picking:* Split `picking.rs` into a folder. Add a system that reads `bevy_picking::pointer::PointerLocation` and produces `bevy_picking::backend::PointerHits` from Buiy's `ResolvedLayout` AABBs, registered via `App::add_systems(PreUpdate, ... .in_set(bevy_picking::PickSet::Backend))`. The existing `update_hovered` system rewires to read `PointerHits` so `Hovered` keeps its API.

**Tech Stack:** Rust 1.85, Bevy 0.18 (with `bevy_picking` feature added), Taffy 0.10, `accesskit` + `accesskit_winit`, `bytemuck` for `Pod` instance data.

**Bevy version note:** Bevy 0.18+ APIs shift between minors. Where this plan shows specific Bevy API calls — `bevy::winit::WinitWindows`, the exact shape of `bevy_picking::backend::PointerHits` and `PickSet::Backend`, `accesskit_winit::Adapter::new` signature — verify against the in-tree version before writing the code. Architectural decisions (per-window adapter map, instance-buffer per frame in the render-node, `Hovered` as a thin layer over `bevy_picking`) are stable; minor API name/import adjustments may be required.

**Out of scope (explicit deferrals to v0.x sub-specs):**
- Real-GPU pixel-diff CI gate. The two `#[ignore]`'d render smoke tests stay `#[ignore]`'d; CI lavapipe provisioning is `buiy-verification-design` work.
- Persistent vertex/instance buffers, atlasing, clip-path, filters, blend modes, top-layer compositing — `buiy-render-pipeline-design`.
- Multi-pointer arbitration, full backend priority registration, hit-target linter — `buiy-input-events-design`.
- ACCNAME 1.2 computation, full ARIA taxonomy expansion — `buiy-accessibility-design`.
- Forms, animation, devtools, BSN, multiple widgets, 3D-anchored UI — explicit Phase 0 non-goals.

---

## Spec coverage map

| Phase 0 self-review deferral | This plan task |
|---|---|
| Plan + `docs/README.md` index `[draft]` tags | Task 1 |
| Task 21 self-review checkboxes unticked | Task 1 |
| Workspace deps (`bytemuck`, `accesskit`, `accesskit_winit`, `bevy_picking` feature) | Task 2 |
| Render pipeline draws — instance-buffer construction left to v0.x | Task 3 |
| `accesskit_winit::Adapter` per-window wiring (`HashMap<WindowId, Adapter>`) | Task 4 |
| `bevy_picking::backend` registration, per-window filter | Task 5 |

---

## File structure

**Modified:**
- `Cargo.toml` (workspace) — add `bytemuck`, `accesskit`, `accesskit_winit` to `[workspace.dependencies]`; add `bevy_picking` to bevy's feature list.
- `crates/buiy_core/Cargo.toml` — depend on the new workspace entries; add a direct `bevy_picking`/`accesskit`/`accesskit_winit` access path through bevy.
- `crates/buiy_core/src/render/mod.rs` — extend `ExtractedDraws` with `window_size: Vec2`; extend `extract_buiy_draws` to populate it from the primary window.
- `crates/buiy_core/src/render/pipeline.rs` — add a static unit-quad `Buffer` to `BuiyPipeline`; create it at pipeline-init.
- `crates/buiy_core/src/render/node.rs` — build per-frame instance buffer; bind both buffers; issue draw call.
- `crates/buiy_core/src/render/shader.wgsl` — drop the resolved-by-this-plan TODO comment.
- `crates/buiy_core/src/lib.rs` — module declarations updated for the `a11y` and `picking` folder splits.
- `crates/buiy/src/lib.rs` — re-export `AccessKitAdapters` if it becomes part of the public surface (it does, so consumers can disable it).
- `examples/hello_button/Cargo.toml` — no edit required.
- `docs/plans/2026-05-07-buiy-phase-0-foundations.md` — flip `Status:` to `landed`; tick Task 21 checkboxes.
- `docs/README.md` — flip Phase 0 plan tag from `[draft]` → `[landed]`.

**Created:**
- `docs/plans/2026-05-08-buiy-phase-0-closeout.md` — this plan (already saved).
- `crates/buiy_core/src/render/instance.rs` — `InstanceData` POD + `to_instance(draw, window_size)` clip-space conversion.
- `crates/buiy_core/tests/render_instance.rs` — unit tests for instance-data layout + conversion math.
- `crates/buiy_core/src/a11y/mod.rs` — moves the contents of `a11y.rs` here; declares `translate` and `adapter` submodules.
- `crates/buiy_core/src/a11y/translate.rs` — pure `to_accesskit_node(view) -> accesskit::Node` + `build_tree_update(views, focused) -> accesskit::TreeUpdate`.
- `crates/buiy_core/src/a11y/adapter.rs` — `AccessKitAdapters` resource, `WindowCreated`/`WindowClosed` lifecycle systems, push-update system.
- `crates/buiy_core/tests/a11y_translate.rs` — unit tests for `translate.rs`.
- `crates/buiy_core/src/picking/mod.rs` — moves `picking.rs` content here; declares `backend` submodule.
- `crates/buiy_core/src/picking/backend.rs` — `bevy_picking` backend system, registered via `BuiyPickingBackendPlugin`.
- `crates/buiy_core/tests/picking_backend.rs` — integration tests for `PointerHits` emission.

**Deleted (after the moves above commit):**
- `crates/buiy_core/src/a11y.rs`
- `crates/buiy_core/src/picking.rs`

---

## Task 1: Cosmetic status flips

**Files:**
- Modify: `docs/plans/2026-05-07-buiy-phase-0-foundations.md:4` and `:2754-2758`
- Modify: `docs/README.md` (search for the `[draft]` tag on the Phase 0 plan entry)

This task has no code, only doc-status synchronization with the landed reality.

- [ ] **Step 1: Flip Phase 0 plan header**

In `docs/plans/2026-05-07-buiy-phase-0-foundations.md`, line 4:

```diff
-**Status:** draft
+**Status:** landed
```

- [ ] **Step 2: Tick Task 21 checkboxes**

In `docs/plans/2026-05-07-buiy-phase-0-foundations.md`, lines 2754–2758, replace each `- [ ]` with `- [x]`. The five steps (re-read foundation spec, `cargo doc`, `cargo deny check`, final `cargo test --workspace`, README index update) all happened across the readiness PRs (commits `1dfe84e`, `6b0ea9d`, `2312707`).

- [ ] **Step 3: Flip Phase 0 entry in `docs/README.md`**

Find the Phase 0 plan line under `### Foundation` → `**Plans**`:

```diff
-- [Phase 0 foundations](plans/2026-05-07-buiy-phase-0-foundations.md) — workspace, BuiyPlugin, system sets, minimal render/layout/a11y/focus/picking/theme, verification harness skeleton, hello-world Button. `[draft]`
+- [Phase 0 foundations](plans/2026-05-07-buiy-phase-0-foundations.md) — workspace, BuiyPlugin, system sets, minimal render/layout/a11y/focus/picking/theme, verification harness skeleton, hello-world Button. `[landed]`
```

- [ ] **Step 4: Add this plan to the index**

Same file `docs/README.md`, immediately under the Phase 0 entry, add:

```markdown
- [Phase 0 closeout](plans/2026-05-08-buiy-phase-0-closeout.md) — render-pipeline draws, AccessKit per-window adapter, `bevy_picking` backend; closes the three substantive deferrals from the Phase 0 self-review. `[draft]`
```

- [ ] **Step 5: Commit**

```bash
git add docs/plans/2026-05-07-buiy-phase-0-foundations.md docs/plans/2026-05-08-buiy-phase-0-closeout.md docs/README.md
git commit -m "docs: flip Phase 0 plan to landed, index closeout plan"
```

---

## Task 2: Add workspace dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/buiy_core/Cargo.toml`

The Phase 0 plan deferred these deps with the comment at workspace `Cargo.toml:36-40`: *"Phase 0 keeps the workspace dep set minimal: only crates that have a `use` in `src/` are listed. Speculative deps (accesskit, accesskit_winit, bevy_picking backend, image-compare for SSIM, thiserror for typed errors) return when the consuming task lands."* Closeout is when they land for the three named deps.

- [ ] **Step 1: Pre-flight verify the current workspace builds**

```bash
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && xvfb-run -a cargo test --workspace
```

Expected: PASS on every line. If anything fails before we touch the manifest, fix that first.

- [ ] **Step 2: Edit workspace `Cargo.toml`**

Modify the `[workspace.dependencies]` block. Two edits:

(a) Add `bevy_picking` to the bevy feature list:

```diff
-bevy = { version = "0.18", default-features = false, features = ["bevy_render", "bevy_core_pipeline", "bevy_winit", "bevy_window", "bevy_asset", "bevy_log", "x11", "wayland"] }
+bevy = { version = "0.18", default-features = false, features = ["bevy_render", "bevy_core_pipeline", "bevy_winit", "bevy_window", "bevy_asset", "bevy_log", "bevy_picking", "x11", "wayland"] }
```

(b) Append the three new deps under the bevy line:

```toml
# Phase 0 closeout deps (closeout plan 2026-05-08).
bytemuck = { version = "1", features = ["derive"] }
accesskit = "0.21"
accesskit_winit = "0.31"
```

(Version pins: pick whatever versions match Bevy 0.18's vendored `accesskit` re-export. If `bevy_a11y` re-exports a specific accesskit version, match it exactly to avoid duplicate `accesskit` in the dep graph. Run `cargo tree -p accesskit` after the edit to confirm a single version resolves; if not, adjust the pin.)

(c) Update the deferral comment (workspace `Cargo.toml:36-40`) to reflect what's now in vs. still out:

```diff
 # Phase 0 keeps the workspace dep set minimal: only crates that have a
 # `use` in `src/` are listed. Speculative deps (accesskit, accesskit_winit,
 # bevy_picking backend, image-compare for SSIM, thiserror for typed errors)
 # return when the consuming task lands. See review notes in
 # docs/plans/2026-05-07-buiy-phase-0-foundations.md.
+# Phase 0 closeout (2026-05-08) added: bytemuck (instance buffer POD),
+# accesskit + accesskit_winit (per-window adapter), bevy_picking feature
+# on bevy. Still deferred: image-compare (visual harness upgrade is v0.x
+# verification work), thiserror (no error-typing pressure yet).
```

- [ ] **Step 3: Edit `crates/buiy_core/Cargo.toml`**

Add the new deps so `buiy_core` can `use` them:

```diff
 [dependencies]
 bevy.workspace = true
 taffy.workspace = true
 tracing.workspace = true
+bytemuck.workspace = true
+accesskit.workspace = true
+accesskit_winit.workspace = true
```

- [ ] **Step 4: Verify the dep graph resolves cleanly**

```bash
cargo tree -p accesskit
cargo build --workspace
```

Expected: a single `accesskit` version in the tree, and a clean build. If two versions resolve, downgrade/upgrade the workspace `accesskit` pin to match Bevy's vendored version.

- [ ] **Step 5: Re-run cargo-deny**

```bash
cargo deny check
```

Expected: PASS. The new crates' licenses (Apache-2.0 / MIT for bytemuck and accesskit) are already on the allowlist; no `deny.toml` change should be needed. If deny flags a new license, add it explicitly with a one-line justification rather than relaxing the policy.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/buiy_core/Cargo.toml
git commit -m "build: add bytemuck, accesskit, accesskit_winit, bevy_picking feature"
```

---

## Task 3: Render pipeline draws real pixels

**Files:**
- Create: `crates/buiy_core/src/render/instance.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/src/render/pipeline.rs`
- Modify: `crates/buiy_core/src/render/node.rs`
- Modify: `crates/buiy_core/src/render/shader.wgsl` (one comment removal)
- Create: `crates/buiy_core/tests/render_instance.rs`

The shader's existing TODO at `shader.wgsl:7-13` resolves the unit-space question for us: pre-multiply by the inverse window size on the CPU before writing the instance buffer (path (a)). No bind-group layout change required; `pipeline.rs:60` `layout: vec![]` stays valid.

### Subtask 3a: Define instance data and conversion math

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/render_instance.rs`:

```rust
//! Unit tests for the instance-data layout and clip-space conversion. These
//! are pure-CPU tests; no GPU adapter required.

use bevy::prelude::*;
use buiy_core::render::DrawData;
use buiy_core::render::instance::{INSTANCE_STRIDE_BYTES, InstanceData, to_instance};

#[test]
fn instance_data_layout_matches_pipeline_descriptor() {
    // pipeline.rs declares the per-instance buffer with array_stride = 36.
    assert_eq!(
        std::mem::size_of::<InstanceData>(),
        INSTANCE_STRIDE_BYTES,
        "InstanceData stride must match pipeline.rs (2*4 + 2*4 + 4*4 + 1*4 = 36)"
    );
    assert_eq!(INSTANCE_STRIDE_BYTES, 36);
}

#[test]
fn to_instance_centers_origin_at_window_center() {
    // A rect at (0,0) of size (window) should map to clip rect_pos = (-1, +1)
    // (top-left in clip after y-flip) and rect_size = (2, 2).
    let window = Vec2::new(800.0, 600.0);
    let draw = DrawData {
        position: Vec2::ZERO,
        size: window,
        color: Color::WHITE,
        radius: 0.0,
    };
    let i = to_instance(&draw, window);
    assert!((i.rect_pos[0] - -1.0).abs() < 1e-6);
    assert!((i.rect_pos[1] - 1.0).abs() < 1e-6);
    assert!((i.rect_size[0] - 2.0).abs() < 1e-6);
    assert!((i.rect_size[1] - -2.0).abs() < 1e-6);
}

#[test]
fn to_instance_packs_color_in_linear_rgba() {
    let window = Vec2::new(100.0, 100.0);
    let draw = DrawData {
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
        color: Color::srgb(1.0, 0.0, 0.0),
        radius: 0.0,
    };
    let i = to_instance(&draw, window);
    let lin = LinearRgba::from(Color::srgb(1.0, 0.0, 0.0));
    assert!((i.color[0] - lin.red).abs() < 1e-5);
    assert!((i.color[1] - lin.green).abs() < 1e-5);
    assert!((i.color[2] - lin.blue).abs() < 1e-5);
    assert!((i.color[3] - lin.alpha).abs() < 1e-5);
}

#[test]
fn to_instance_radius_uses_min_window_dim() {
    // Radius is in clip-space units; pixel-to-clip uses 2.0 / min(window dims)
    // so the corner radius stays visually reasonable on non-square windows.
    let window = Vec2::new(1000.0, 500.0);
    let draw = DrawData {
        position: Vec2::ZERO,
        size: Vec2::splat(100.0),
        color: Color::WHITE,
        radius: 25.0,
    };
    let i = to_instance(&draw, window);
    let expected = 25.0 * (2.0 / 500.0);
    assert!((i.radius - expected).abs() < 1e-6);
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p buiy_core --test render_instance
```

Expected: compile error (`buiy_core::render::instance` does not exist).

- [ ] **Step 3: Implement `instance.rs`**

Create `crates/buiy_core/src/render/instance.rs`:

```rust
//! Per-instance data layout for the rounded-rect pipeline. The struct stride
//! must equal the per-instance `array_stride` declared in
//! `pipeline.rs::register` (currently 36). Phase 0 closeout converts logical
//! pixels → clip-space units on the CPU here, per the path (a) decision in
//! `shader.wgsl`'s former TODO comment.
//!
//! v0.x will replace this with a view uniform (`buiy-render-pipeline-design`),
//! at which point the conversion moves to the vertex stage and `InstanceData`
//! shrinks back to logical-pixel units.

use crate::render::DrawData;
use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};

/// Stride must match `pipeline.rs::register` instance-buffer layout (36 B).
pub const INSTANCE_STRIDE_BYTES: usize = 36;

/// One instance record. Fields match `Instance` in `shader.wgsl` 1:1.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Pod, Zeroable)]
pub struct InstanceData {
    /// Top-left in clip space (-1..+1, y up).
    pub rect_pos: [f32; 2],
    /// Width / height in clip space; height is negative because UI-space y is
    /// down-positive but clip-space y is up-positive (single y-flip lives in
    /// the size, not the position, so the shader's `rect_pos + uv * rect_size`
    /// remains correct top-down).
    pub rect_size: [f32; 2],
    /// Linear RGBA. Pipeline target is `Rgba8UnormSrgb`, so the GPU re-encodes
    /// to sRGB on write.
    pub color: [f32; 4],
    /// Corner radius in clip-space units. Phase 0 closeout uses
    /// `2.0 / min(window.x, window.y)` as the px→clip conversion to keep
    /// corners visually round on non-square windows; v0.x view uniform
    /// removes the approximation.
    pub radius: f32,
}

/// Convert one [`DrawData`] (logical-pixel UI space) into an [`InstanceData`]
/// (clip space) for the given window size in logical pixels.
pub fn to_instance(draw: &DrawData, window_size: Vec2) -> InstanceData {
    let inv_w = 2.0 / window_size.x;
    let inv_h = 2.0 / window_size.y;
    let inv_min = 2.0 / window_size.x.min(window_size.y);

    let rect_pos = [
        draw.position.x * inv_w - 1.0,
        // y-flip: UI top (px=0) → clip top (+1).
        1.0 - draw.position.y * inv_h,
    ];
    let rect_size = [draw.size.x * inv_w, -draw.size.y * inv_h];

    let lin = LinearRgba::from(draw.color);
    let color = [lin.red, lin.green, lin.blue, lin.alpha];

    InstanceData {
        rect_pos,
        rect_size,
        color,
        radius: draw.radius * inv_min,
    }
}
```

- [ ] **Step 4: Wire `instance` into `render/mod.rs`**

Add `pub mod instance;` near the existing `pub mod node;` / `pub mod pipeline;` lines in `crates/buiy_core/src/render/mod.rs`.

- [ ] **Step 5: Run the tests to verify they pass**

```bash
cargo test -p buiy_core --test render_instance
```

Expected: 4 tests pass.

### Subtask 3b: Extend `ExtractedDraws` with window size

- [ ] **Step 6: Modify `ExtractedDraws`**

In `crates/buiy_core/src/render/mod.rs`, extend the resource and the extract system:

```diff
 #[derive(Resource, Default, Clone)]
 #[non_exhaustive]
 pub struct ExtractedDraws {
     pub draws: Vec<DrawData>,
+    /// Logical-pixel size of the primary window this frame. Populated by the
+    /// extract system. Render-graph nodes use this to convert
+    /// `DrawData` (px) → `InstanceData` (clip) per the Phase 0 closeout
+    /// design. Zero on frames where no window exists; the render node
+    /// must skip drawing in that case.
+    pub window_size: Vec2,
 }
```

```diff
 fn extract_buiy_draws(
     mut commands: Commands,
     main_world_q: Extract<Query<(&Style, &ResolvedLayout), With<Node>>>,
     main_world_theme: Extract<Res<Theme>>,
+    main_world_windows: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
 ) {
     let mut draws = ExtractedDraws::default();
+    if let Ok(window) = main_world_windows.single() {
+        let res = window.resolution.size();
+        draws.window_size = Vec2::new(res.x, res.y);
+    }
     for (style, layout) in main_world_q.iter() {
```

(The `Query::single` API in Bevy 0.18 returns a `Result`; if the project's pinned bevy is on the older `.get_single()` form, swap accordingly. Verify with `cargo doc` if uncertain.)

- [ ] **Step 7: Run existing tests to confirm nothing regressed**

```bash
xvfb-run -a cargo test --workspace
```

Expected: all green. The new `window_size` field defaults to zero and the existing render smoke tests don't read it.

### Subtask 3c: Static unit-quad vertex buffer in `BuiyPipeline`

- [ ] **Step 8: Extend `BuiyPipeline`**

In `crates/buiy_core/src/render/pipeline.rs`, two edits.

(a) Add the buffer field:

```diff
-#[derive(Resource)]
-pub struct BuiyPipeline {
-    pub id: CachedRenderPipelineId,
-}
+#[derive(Resource)]
+pub struct BuiyPipeline {
+    pub id: CachedRenderPipelineId,
+    /// Static unit-quad vertex buffer (4 verts, TriangleStrip). Created once
+    /// at pipeline registration and reused every frame. Phase 0 closeout
+    /// scope: vertex emission order matches the `cull_mode: None` setting in
+    /// the descriptor; v0.x tightens to back-face culling.
+    pub vertex_buffer: bevy::render::render_resource::Buffer,
+}
```

(b) Build the buffer at the end of `register`:

```rust
// At top of file, add:
use bevy::render::renderer::RenderDevice;
use bevy::render::render_resource::{Buffer, BufferInitDescriptor, BufferUsages};
```

```rust
// Inside `register`, BEFORE `let id = pipeline_cache.queue_render_pipeline(descriptor);`:
let render_device = world.resource::<RenderDevice>();

// Unit quad in (pos, uv) interleaved layout, matching the vertex-buffer
// layout in `descriptor.vertex.buffers[0]`. TriangleStrip order: TL, BL,
// TR, BR — both triangles wind consistently, which the v0.x backface-cull
// tightening will rely on.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct QuadVertex { pos: [f32; 2], uv: [f32; 2] }

let quad: [QuadVertex; 4] = [
    QuadVertex { pos: [0.0, 0.0], uv: [0.0, 0.0] }, // TL
    QuadVertex { pos: [0.0, 1.0], uv: [0.0, 1.0] }, // BL
    QuadVertex { pos: [1.0, 0.0], uv: [1.0, 0.0] }, // TR
    QuadVertex { pos: [1.0, 1.0], uv: [1.0, 1.0] }, // BR
];

let vertex_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
    label: Some("buiy_unit_quad_vbo"),
    contents: bytemuck::cast_slice(&quad),
    usage: BufferUsages::VERTEX,
});
```

(c) Update the final `world.insert_resource` call:

```diff
-    let id = pipeline_cache.queue_render_pipeline(descriptor);
-    world.insert_resource(BuiyPipeline { id });
+    let id = pipeline_cache.queue_render_pipeline(descriptor);
+    world.insert_resource(BuiyPipeline { id, vertex_buffer });
```

- [ ] **Step 9: Build to verify**

```bash
cargo build -p buiy_core
```

Expected: clean build.

### Subtask 3d: Build instance buffer + draw call in `node.rs`

- [ ] **Step 10: Replace `BuiyNode::run` body**

In `crates/buiy_core/src/render/node.rs`, three changes.

(a) Imports at the top:

```diff
 use bevy::core_pipeline::core_2d::graph::{Core2d, Node2d};
 use bevy::ecs::query::QueryItem;
 use bevy::prelude::*;
 use bevy::render::{
     render_graph::{
         NodeRunError, RenderGraphContext, RenderGraphExt, RenderLabel, ViewNode, ViewNodeRunner,
     },
-    render_resource::{PipelineCache, RenderPassDescriptor},
+    render_resource::{BufferInitDescriptor, BufferUsages, PipelineCache, RenderPassDescriptor},
     renderer::RenderContext,
     view::ViewTarget,
 };

-use super::{ExtractedDraws, pipeline::BuiyPipeline};
+use super::{ExtractedDraws, instance::to_instance, pipeline::BuiyPipeline};
```

(b) Replace the body of `run` from the line that opens the pass through the closing of `Ok(())`. The new body builds the instance buffer, sets both vertex buffers, and issues the draw:

```rust
fn run<'w>(
    &self,
    _graph: &mut RenderGraphContext,
    render_context: &mut RenderContext<'w>,
    view_target: QueryItem<'w, '_, Self::ViewQuery>,
    world: &'w World,
) -> Result<(), NodeRunError> {
    let pipeline_cache = world.resource::<PipelineCache>();
    let buiy_pipeline = world.resource::<BuiyPipeline>();
    let Some(pipeline) = pipeline_cache.get_render_pipeline(buiy_pipeline.id) else {
        return Ok(());
    };
    let draws = world.resource::<ExtractedDraws>();
    if draws.draws.is_empty() || draws.window_size.x <= 0.0 || draws.window_size.y <= 0.0 {
        return Ok(());
    }

    // Pack instances. Phase 0 closeout: per-frame allocation; v0.x
    // (`buiy-render-pipeline-design`) introduces persistent buffers.
    let instances: Vec<_> = draws
        .draws
        .iter()
        .map(|d| to_instance(d, draws.window_size))
        .collect();

    let render_device = render_context.render_device();
    let instance_buffer = render_device.create_buffer_with_data(&BufferInitDescriptor {
        label: Some("buiy_instance_vbo"),
        contents: bytemuck::cast_slice(&instances),
        usage: BufferUsages::VERTEX,
    });

    let mut pass = render_context.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("buiy_pass"),
        color_attachments: &[Some(view_target.get_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_vertex_buffer(0, buiy_pipeline.vertex_buffer.slice(..));
    pass.set_vertex_buffer(1, instance_buffer.slice(..));
    pass.draw(0..4, 0..instances.len() as u32);
    Ok(())
}
```

(c) Update the file-level doc comment in `node.rs:20-23`:

```diff
-//! Phase 0 status: this node ships the *render-graph wiring* + a
-//! `set_render_pipeline` call so the pass exists and the pipeline is bound,
-//! but vertex / instance buffer construction is deferred to v0.x. Task 19
-//! (the e2e screenshot harness) is responsible for end-to-end verification.
+//! Phase 0 closeout (2026-05-08): this node now builds the instance buffer
+//! per-frame from `ExtractedDraws` and issues an instanced `draw(0..4, 0..n)`
+//! against the static unit-quad VBO held on `BuiyPipeline`. v0.x upgrades to
+//! persistent buffers + bind groups for filters / atlas
+//! (`buiy-render-pipeline-design`).
```

- [ ] **Step 11: Drop the resolved TODO from the shader**

In `crates/buiy_core/src/render/shader.wgsl`, delete lines 1–13 (the doc comment and `TODO(Task 11)` block) and replace with a one-line file-level comment:

```wgsl
// Buiy rounded-rect shader. Inputs are clip-space units; CPU-side
// conversion lives in `render::instance::to_instance`.
```

The `// see TODO above re: units` comment on the `Instance.rect_pos` line should also be removed.

- [ ] **Step 12: Run all tests + the still-`#[ignore]`'d render smoke locally**

```bash
xvfb-run -a cargo test --workspace
xvfb-run -a cargo test -p buiy_core --test render_smoke -- --ignored
```

Expected: workspace all green; the ignored render-smoke tests pass on a machine with a real GPU or lavapipe. CI runners without a GPU continue to skip them — that's the v0.x verification gate, not this plan's scope.

- [ ] **Step 13: Commit**

```bash
git add crates/buiy_core/src/render/ crates/buiy_core/tests/render_instance.rs
git commit -m "feat(buiy_core): render pipeline draws via per-frame instance buffer"
```

---

## Task 4: AccessKit per-window adapter

**Files:**
- Modify: `crates/buiy_core/src/lib.rs`
- Move: `crates/buiy_core/src/a11y.rs` → `crates/buiy_core/src/a11y/mod.rs`
- Create: `crates/buiy_core/src/a11y/translate.rs`
- Create: `crates/buiy_core/src/a11y/adapter.rs`
- Create: `crates/buiy_core/tests/a11y_translate.rs`
- Modify: `crates/buiy_core/tests/a11y.rs` (extend)
- Modify: `crates/buiy/src/lib.rs` (export `AccessKitAdapters`)

The architecture spec commits to **per-window** ownership: `HashMap<WindowId, accesskit_winit::Adapter>`. Per the user's pick on PR-shape question 2, this plan lands the full per-window map, not a primary-window-only shortcut.

### Subtask 4a: Module split (mechanical move)

- [ ] **Step 1: Move the file**

```bash
mkdir crates/buiy_core/src/a11y
git mv crates/buiy_core/src/a11y.rs crates/buiy_core/src/a11y/mod.rs
```

- [ ] **Step 2: Declare the new submodules**

At the top of `crates/buiy_core/src/a11y/mod.rs`, after the existing `use` block, add:

```rust
pub mod adapter;
pub mod translate;

pub use adapter::{AccessKitAdapters, AccessKitAdapterPlugin};
pub use translate::{build_tree_update, to_accesskit_node};
```

- [ ] **Step 3: Verify the workspace still builds**

```bash
cargo build --workspace
```

Expected: clean build. Existing `crates/buiy_core/tests/a11y.rs` still passes; this step is purely a move.

### Subtask 4b: Pure translation function (`translate.rs`)

- [ ] **Step 4: Write the failing tests**

Create `crates/buiy_core/tests/a11y_translate.rs`:

```rust
//! Unit tests for the pure `A11yNodeView` → AccessKit translation. No winit
//! window is required — this is the test-friendly seam between Buiy's a11y
//! tree and the OS-level adapter.

use bevy::prelude::*;
use buiy_core::a11y::{
    A11yNodeView, A11yRole,
    translate::{build_tree_update, to_accesskit_node},
};

#[test]
fn role_maps_to_accesskit_role() {
    let view = A11yNodeView {
        entity: Entity::PLACEHOLDER,
        role: A11yRole::Button,
        name: "Save".into(),
        description: String::new(),
        focusable: true,
    };
    let node = to_accesskit_node(&view);
    assert_eq!(node.role(), accesskit::Role::Button);
    assert_eq!(node.label(), Some("Save"));
}

#[test]
fn build_tree_update_emits_root_plus_children() {
    let views = vec![
        A11yNodeView {
            entity: Entity::from_raw(1),
            role: A11yRole::Button,
            name: "Save".into(),
            description: "Saves the document".into(),
            focusable: true,
        },
        A11yNodeView {
            entity: Entity::from_raw(2),
            role: A11yRole::Generic,
            name: "Container".into(),
            description: String::new(),
            focusable: false,
        },
    ];
    let update = build_tree_update(&views, None);
    // Root + 2 child nodes:
    assert_eq!(update.nodes.len(), 3);
    // Tree pointer is set with a stable root id:
    assert!(update.tree.is_some());
}

#[test]
fn focused_node_id_round_trips() {
    let views = vec![A11yNodeView {
        entity: Entity::from_raw(42),
        role: A11yRole::Button,
        name: "Focus me".into(),
        description: String::new(),
        focusable: true,
    }];
    // Caller passes the entity-derived NodeId for the focused node.
    let focused = Some(buiy_core::a11y::translate::node_id_for(Entity::from_raw(42)));
    let update = build_tree_update(&views, focused);
    assert_eq!(update.focus, focused.expect("focused id present"));
}
```

- [ ] **Step 5: Run tests to verify they fail**

```bash
cargo test -p buiy_core --test a11y_translate
```

Expected: compile error (`buiy_core::a11y::translate` is empty, `to_accesskit_node` etc. are undefined).

- [ ] **Step 6: Implement `translate.rs`**

Create `crates/buiy_core/src/a11y/translate.rs`:

```rust
//! Pure translation from Buiy's frame-built `A11yNodeView` snapshot into the
//! AccessKit data model. Keeping this module winit-free means we can
//! unit-test it without provisioning a real window.

use crate::a11y::{A11yNodeView, A11yRole};
use accesskit::{Node, NodeId, Role, Tree, TreeUpdate};
use bevy::prelude::Entity;

/// Stable AccessKit root node id. Every adapter pushes the same root so the
/// AT sees one tree per Buiy window. v0.x may key this off the `WindowId`
/// when multi-window-aware ATs become a target.
pub const ROOT_NODE_ID: NodeId = NodeId(0);

/// Convert a Bevy [`Entity`] into a stable [`NodeId`]. Entity::to_bits is
/// deterministic within a session, which is sufficient for AT consumption
/// (the AT doesn't compare across sessions).
pub fn node_id_for(entity: Entity) -> NodeId {
    // Avoid 0 (reserved for ROOT_NODE_ID). +1 is safe because Bevy never
    // produces an `Entity` whose bits are `u64::MAX`.
    NodeId(entity.to_bits().saturating_add(1))
}

/// Translate one [`A11yNodeView`] into an [`accesskit::Node`].
pub fn to_accesskit_node(view: &A11yNodeView) -> Node {
    let mut node = Node::new(role_to_accesskit(view.role));
    if !view.name.is_empty() {
        node.set_label(view.name.clone());
    }
    if !view.description.is_empty() {
        node.set_description(view.description.clone());
    }
    // Phase 0 closeout: focusable widgets get the AccessKit "focusable"
    // semantic. Full keyboard-action contract is widget-specific
    // (`buiy-widget-catalog-design`).
    if view.focusable {
        node.add_action(accesskit::Action::Focus);
    }
    node
}

fn role_to_accesskit(role: A11yRole) -> Role {
    match role {
        A11yRole::Generic => Role::GenericContainer,
        A11yRole::Button => Role::Button,
        A11yRole::Link => Role::Link,
        A11yRole::Image => Role::Image,
        A11yRole::Text => Role::Label,
        A11yRole::Heading => Role::Heading,
        A11yRole::Dialog => Role::Dialog,
        A11yRole::AlertDialog => Role::AlertDialog,
        A11yRole::Tooltip => Role::Tooltip,
    }
}

/// Build a full [`TreeUpdate`] containing a synthetic root plus one node
/// per [`A11yNodeView`]. Children are listed under the root in iteration
/// order; nesting is a v0.x topic (`buiy-accessibility-design`).
pub fn build_tree_update(views: &[A11yNodeView], focused: Option<NodeId>) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(views.len() + 1);

    // Children first — we still need to materialize their NodeIds before
    // we can list them under the root.
    let mut child_ids = Vec::with_capacity(views.len());
    for view in views {
        let id = node_id_for(view.entity);
        child_ids.push(id);
        nodes.push((id, to_accesskit_node(view)));
    }

    // Root.
    let mut root = Node::new(Role::Window);
    for cid in &child_ids {
        root.push_child(*cid);
    }
    nodes.insert(0, (ROOT_NODE_ID, root));

    TreeUpdate {
        nodes,
        tree: Some(Tree::new(ROOT_NODE_ID)),
        focus: focused.unwrap_or(ROOT_NODE_ID),
    }
}
```

(Verify exact AccessKit 0.21 API names — `Node::new`, `set_label`, `push_child`, `Tree::new` — against the docs of the pinned version. If any differ in the workspace's resolved AccessKit, swap the call names; the data model is stable.)

- [ ] **Step 7: Run translate tests to verify they pass**

```bash
cargo test -p buiy_core --test a11y_translate
```

Expected: 3 tests pass.

### Subtask 4c: Per-window adapter map (`adapter.rs`)

- [ ] **Step 8: Implement `adapter.rs`**

Create `crates/buiy_core/src/a11y/adapter.rs`:

```rust
//! Per-window AccessKit adapter ownership. Buiy maintains a
//! `HashMap<WindowId, accesskit_winit::Adapter>` resource and pushes
//! `TreeUpdate` payloads to every adapter each frame.
//!
//! Architecture: foundation spec architecture.md § 2.6, accessibility.md § 3.11.
//!
//! Test-friendliness note: we cannot `Adapter::new` without a real winit
//! window, so adapter creation is gated on the presence of a winit handle in
//! `bevy::winit::WinitWindows`. Tests that use `MinimalPlugins` (no winit)
//! exercise the lifecycle systems but observe an empty adapter map, which is
//! the correct behavior — the *pure* translation is covered by tests in
//! `tests/a11y_translate.rs`.

use crate::BuiySet;
use crate::a11y::{A11yTreeBuilder, FocusedEntity};
use crate::a11y::translate::{build_tree_update, node_id_for};
use accesskit_winit::Adapter;
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, WindowClosed, WindowCreated, WindowId};
use bevy::winit::WinitWindows;
use std::collections::HashMap;

/// Per-window AccessKit adapter map. NonSend because `Adapter` holds a
/// reference to a non-Send winit handle on some platforms.
#[derive(Default)]
pub struct AccessKitAdapters {
    pub adapters: HashMap<WindowId, Adapter>,
}

pub struct AccessKitAdapterPlugin;

impl Plugin for AccessKitAdapterPlugin {
    fn build(&self, app: &mut App) {
        app.init_non_send_resource::<AccessKitAdapters>()
            .add_systems(
                Update,
                (open_adapters, close_adapters)
                    .chain()
                    .before(BuiySet::A11yUpdate),
            )
            .add_systems(Update, push_tree_updates.in_set(BuiySet::A11yUpdate));
    }
}

fn open_adapters(
    mut events: EventReader<WindowCreated>,
    winit_windows: NonSend<WinitWindows>,
    mut adapters: NonSendMut<AccessKitAdapters>,
) {
    for ev in events.read() {
        let Some(winit_window) = winit_windows.get_window(ev.window) else {
            // Headless / test app — nothing to bind.
            continue;
        };
        let id = winit_window.id();
        if adapters.adapters.contains_key(&id) {
            continue;
        }
        // Adapter::new signature varies by accesskit_winit version; on 0.31
        // it takes (window, activation_handler, action_handler). Buiy ships
        // a minimal action handler that forwards Focus actions back into
        // `FocusedEntity` — keyboard/programmatic focus is the only action
        // Phase 0 closeout supports. v0.x widens this per
        // `buiy-input-events-design`.
        let adapter = Adapter::with_direct_handlers(
            winit_window,
            BuiyActivationHandler,
            BuiyActionHandler,
        );
        adapters.adapters.insert(id, adapter);
    }
}

fn close_adapters(
    mut events: EventReader<WindowClosed>,
    winit_windows: NonSend<WinitWindows>,
    mut adapters: NonSendMut<AccessKitAdapters>,
) {
    for ev in events.read() {
        let Some(winit_window) = winit_windows.get_window(ev.window) else {
            continue;
        };
        adapters.adapters.remove(&winit_window.id());
    }
}

fn push_tree_updates(
    builder: Res<A11yTreeBuilder>,
    focused: Res<FocusedEntity>,
    mut adapters: NonSendMut<AccessKitAdapters>,
) {
    if adapters.adapters.is_empty() {
        return;
    }
    let focused_id = focused.0.map(node_id_for);
    // Build once per frame; clone per adapter — `TreeUpdate` is cheap-ish
    // and the multi-window case is rare. v0.x can cache.
    let update = build_tree_update(builder.snapshot(), focused_id);
    for adapter in adapters.adapters.values_mut() {
        let cloned = TreeUpdateClone::from(&update).into();
        adapter.update_if_active(|| cloned);
    }
}

// `TreeUpdate` does not implement `Clone` directly in accesskit 0.21; this
// helper materializes a fresh `TreeUpdate` from the shared one without
// touching the original. Drop this helper once accesskit derives `Clone`.
struct TreeUpdateClone {
    nodes: Vec<(accesskit::NodeId, accesskit::Node)>,
    tree: Option<accesskit::Tree>,
    focus: accesskit::NodeId,
}

impl From<&accesskit::TreeUpdate> for TreeUpdateClone {
    fn from(u: &accesskit::TreeUpdate) -> Self {
        Self {
            nodes: u
                .nodes
                .iter()
                .map(|(id, n)| (*id, n.clone()))
                .collect(),
            tree: u.tree.clone(),
            focus: u.focus,
        }
    }
}

impl From<TreeUpdateClone> for accesskit::TreeUpdate {
    fn from(c: TreeUpdateClone) -> Self {
        accesskit::TreeUpdate {
            nodes: c.nodes,
            tree: c.tree,
            focus: c.focus,
        }
    }
}

struct BuiyActivationHandler;
impl accesskit_winit::ActivationHandler for BuiyActivationHandler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        // Adapter calls this on first AT connection. Buiy returns an empty
        // tree; the next `push_tree_updates` tick fills it. AT will see the
        // same root id and merge the update.
        Some(accesskit::TreeUpdate {
            nodes: vec![(crate::a11y::translate::ROOT_NODE_ID,
                accesskit::Node::new(accesskit::Role::Window))],
            tree: Some(accesskit::Tree::new(crate::a11y::translate::ROOT_NODE_ID)),
            focus: crate::a11y::translate::ROOT_NODE_ID,
        })
    }
}

struct BuiyActionHandler;
impl accesskit_winit::ActionHandler for BuiyActionHandler {
    fn do_action(&mut self, _request: accesskit::ActionRequest) {
        // Phase 0 closeout: actions arriving from the AT (e.g., screen-reader
        // "click") are dropped. v0.x routes them through bevy_picking events
        // per `buiy-input-events-design`. The hook exists so a real screen
        // reader can already enumerate the tree and announce widgets.
    }
}
```

(API verification reminder: `Adapter::with_direct_handlers`, `update_if_active`, the `ActivationHandler` / `ActionHandler` trait names, and the public field set on `TreeUpdate` are all on `accesskit_winit` 0.31. If a different patch resolves, name-swap may be required; the structure stays.)

- [ ] **Step 9: Add `AccessKitAdapterPlugin` to the top-level plugin**

In `crates/buiy/src/lib.rs`, two edits.

(a) Re-export:

```diff
 pub use buiy_core::{
     BuiySet, CorePlugin,
-    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder},
+    a11y::{A11yDescription, A11yLabel, A11yRole, A11yTreeBuilder, AccessKitAdapters, AccessKitAdapterPlugin},
```

(b) Compose into `BuiyPlugin::build`:

```diff
         app.add_plugins((
             CorePlugin,
             buiy_core::theme::ThemePlugin,
             buiy_core::a11y::A11yPlugin,
+            buiy_core::a11y::AccessKitAdapterPlugin,
             buiy_core::focus::FocusPlugin,
             buiy_core::layout::LayoutPlugin,
             buiy_core::picking::PickingPlugin,
             WidgetsPlugin,
         ));
```

(`AccessKitAdapterPlugin` slots immediately after `A11yPlugin` because it depends on `A11yTreeBuilder` and `FocusedEntity`. Plugin-build order doesn't enforce system-set order, but keeping declarations adjacent makes the dependency obvious.)

### Subtask 4d: Verify

- [ ] **Step 10: Add a smoke test for the adapter lifecycle**

Append to `crates/buiy_core/tests/a11y.rs`:

```rust
#[test]
fn adapter_plugin_loads_without_panic() {
    use bevy::prelude::*;
    use buiy_core::a11y::{AccessKitAdapterPlugin, AccessKitAdapters};
    use buiy_core::CorePlugin;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(buiy_core::a11y::A11yPlugin);
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_plugins(AccessKitAdapterPlugin);
    app.update();
    // No winit windows present → adapter map stays empty. The plugin still
    // installs its lifecycle systems without panicking. Real adapter
    // creation is exercised by running the `hello_button` example end-to-end.
    let adapters = app.world().get_non_send_resource::<AccessKitAdapters>();
    assert!(adapters.is_some(), "AccessKitAdapters resource present");
    assert!(
        adapters.unwrap().adapters.is_empty(),
        "no adapters created without winit windows"
    );
}
```

- [ ] **Step 11: Run all a11y-related tests**

```bash
cargo test -p buiy_core --test a11y --test a11y_translate
```

Expected: existing test (`tree_builder_emits_one_node_per_focusable_with_role_and_label`) still passes; new translate tests pass; new adapter smoke test passes.

- [ ] **Step 12: Smoke-run the example with a screen reader hint**

```bash
cargo run --example hello_button
```

Expected: window opens. On Linux with Orca running and AT-SPI enabled, Orca announces "Save, Button". (This is a *manual release-gate* check per `verification.md`; Phase 0 closeout merely verifies the wiring no longer panics.)

- [ ] **Step 13: Commit**

```bash
git add crates/buiy_core/src/a11y/ crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs crates/buiy_core/tests/a11y.rs crates/buiy_core/tests/a11y_translate.rs
git commit -m "feat(buiy_core): per-window AccessKit adapter map"
```

(`git mv` from Step 1 of subtask 4a was already committed implicitly via the move; if your local state didn't auto-commit the move, add the original `crates/buiy_core/src/a11y.rs` removal here too.)

---

## Task 5: `bevy_picking` backend

**Files:**
- Modify: `crates/buiy_core/src/lib.rs`
- Move: `crates/buiy_core/src/picking.rs` → `crates/buiy_core/src/picking/mod.rs`
- Create: `crates/buiy_core/src/picking/backend.rs`
- Create: `crates/buiy_core/tests/picking_backend.rs`
- Modify: `crates/buiy_core/tests/picking.rs` (no behavioral change; existing `hit_test` API stays)

### Subtask 5a: Module split

- [ ] **Step 1: Move the file**

```bash
mkdir crates/buiy_core/src/picking
git mv crates/buiy_core/src/picking.rs crates/buiy_core/src/picking/mod.rs
```

- [ ] **Step 2: Declare the new submodule**

In `crates/buiy_core/src/picking/mod.rs`, after the existing `use` block, add:

```rust
pub mod backend;

pub use backend::BuiyPickingBackendPlugin;
```

- [ ] **Step 3: Verify the workspace still builds**

```bash
cargo build --workspace && cargo test -p buiy_core --test picking
```

Expected: clean. Existing `hit_test` test still passes.

### Subtask 5b: Backend system

- [ ] **Step 4: Write the failing test**

Create `crates/buiy_core/tests/picking_backend.rs`:

```rust
//! `bevy_picking` backend integration test. Drives a fake `PointerLocation`
//! and asserts a `PointerHits` event fires for the entity under the pointer.

use bevy::prelude::*;
use bevy::math::FloatOrd;
use bevy::picking::backend::PointerHits;
use bevy::picking::pointer::{PointerId, PointerLocation, Location};
use buiy_core::{
    CorePlugin,
    components::{Node, ResolvedLayout, Style},
    picking::{PickingPlugin, BuiyPickingBackendPlugin},
};

#[test]
fn pointer_over_buiy_node_emits_hit() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);
    app.add_event::<PointerHits>();

    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ResolvedLayout {
                position: Vec2::new(10.0, 10.0),
                size: Vec2::new(100.0, 50.0),
            },
        ))
        .id();

    // Spawn a pointer at (50, 30). Real apps source this from winit; the
    // backend reads `PointerLocation` regardless of source.
    app.world_mut().spawn((
        PointerId::Mouse,
        PointerLocation::new(Location {
            target: bevy::picking::pointer::PointerTarget::Window(Entity::PLACEHOLDER),
            position: Vec2::new(50.0, 30.0),
        }),
    ));

    app.update();

    let hits = app.world_mut().resource_mut::<Events<PointerHits>>();
    let mut reader = hits.get_cursor();
    let any_hit = reader.read(&hits).any(|h| h.picks.iter().any(|(e, _)| *e == entity));
    assert!(any_hit, "Buiy backend emits a PointerHits for the entity under the cursor");
}
```

(Bevy 0.18 API verification: the exact module paths `bevy::picking::backend::PointerHits`, `bevy::picking::pointer::{PointerId, PointerLocation, Location}`, and `bevy::picking::pointer::PointerTarget` are stable as of 0.18 but the `PointerTarget` variant set may be `NormalizedRenderTarget`-flavored. If the resolved version exposes a different target type, adjust the test imports — the architecture is unchanged.)

- [ ] **Step 5: Run the test to verify it fails**

```bash
cargo test -p buiy_core --test picking_backend
```

Expected: compile error (`buiy_core::picking::BuiyPickingBackendPlugin` undefined).

- [ ] **Step 6: Implement `backend.rs`**

Create `crates/buiy_core/src/picking/backend.rs`:

```rust
//! Buiy's `bevy_picking` backend. Reads `PointerLocation` and produces
//! `PointerHits` from `ResolvedLayout` AABBs.
//!
//! Phase 0 closeout scope: per-pointer hits, top-most-by-area resolution
//! (matches the Phase 0 `hit_test` semantics in `mod.rs`). Multi-pointer
//! arbitration, pointer-target window filtering, and full backend priority
//! land in `buiy-input-events-design`.

use crate::components::ResolvedLayout;
use bevy::picking::backend::{HitData, PointerHits};
use bevy::picking::pointer::{PointerId, PointerLocation};
use bevy::picking::PickSet;
use bevy::prelude::*;

pub struct BuiyPickingBackendPlugin;

impl Plugin for BuiyPickingBackendPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PreUpdate, emit_picks.in_set(PickSet::Backend));
    }
}

fn emit_picks(
    pointers: Query<(&PointerId, &PointerLocation)>,
    nodes: Query<(Entity, &ResolvedLayout)>,
    mut output: EventWriter<PointerHits>,
) {
    for (pointer, location) in pointers.iter() {
        let Some(loc) = location.location() else {
            continue;
        };
        let cursor = loc.position;

        // Collect every Buiy node under the cursor, with its area as the
        // tie-break for "top-most".
        let mut hits: Vec<(Entity, f32)> = Vec::new();
        for (entity, layout) in nodes.iter() {
            if point_in_aabb(cursor, layout) {
                let area = layout.size.x * layout.size.y;
                hits.push((entity, area));
            }
        }
        if hits.is_empty() {
            continue;
        }
        // Smallest area = closest to the user (top of the stack). bevy_picking
        // expects depth-sorted; emit one HitData per entity with depth derived
        // from area rank.
        hits.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
        let picks: Vec<(Entity, HitData)> = hits
            .iter()
            .enumerate()
            .map(|(i, (e, _))| {
                (
                    *e,
                    HitData::new(
                        // Camera entity unknown to Buiy in Phase 0 closeout; the
                        // render-graph node draws into the active 2D camera's
                        // ViewTarget, but bevy_picking expects a camera ref for
                        // its own back-projection. v0.x sub-spec wires this.
                        Entity::PLACEHOLDER,
                        i as f32,
                        None,
                        None,
                    ),
                )
            })
            .collect();

        output.write(PointerHits::new(*pointer, picks, 0.0));
    }
}

fn point_in_aabb(point: Vec2, layout: &ResolvedLayout) -> bool {
    let max = layout.position + layout.size;
    point.x >= layout.position.x
        && point.x <= max.x
        && point.y >= layout.position.y
        && point.y <= max.y
}
```

(Verification reminder: `HitData::new` arity and the `PointerHits::new` priority float in Bevy 0.18 may differ from the call signatures shown. Confirm with `cargo doc -p bevy_picking --open` against the resolved version; the algorithm — collect + area-sort + emit one `HitData` per hit — does not change.)

- [ ] **Step 7: Run the test to verify it passes**

```bash
cargo test -p buiy_core --test picking_backend
```

Expected: PASS.

### Subtask 5c: Rewire `Hovered` over the new backend

- [ ] **Step 8: Replace `update_hovered` to consume `PointerHits`**

In `crates/buiy_core/src/picking/mod.rs`, replace the body of `update_hovered`:

```rust
fn update_hovered(
    mut hovered: ResMut<Hovered>,
    mut events: EventReader<bevy::picking::backend::PointerHits>,
) {
    // The top-most hit is at index 0 of `picks` (sorted ascending by depth in
    // `BuiyPickingBackendPlugin::emit_picks`). Multiple pointers: we honor the
    // most-recently-emitted hit and fall through to clearing if no events
    // arrive this frame.
    let mut latest: Option<Entity> = None;
    let mut saw_event = false;
    for ev in events.read() {
        saw_event = true;
        if let Some((entity, _)) = ev.picks.first() {
            latest = Some(*entity);
        } else {
            latest = None;
        }
    }
    if saw_event {
        hovered.0 = latest;
    }
    // No events this frame ⇒ leave `Hovered` as-is. Cursor-leaving-window
    // produces an empty `picks` event (see emit_picks) which clears it.
}
```

Update the imports at the top of `mod.rs`:

```diff
 use crate::components::ResolvedLayout;
 use bevy::ecs::query::QueryState;
 use bevy::prelude::*;
+use bevy::picking::backend::PointerHits;
```

Drop the `windows: Query<&Window>, layouts: Query<...>` parameters from `update_hovered` since the backend now drives it. The free `hit_test` function stays as-is — tests still call it directly, and the AABB algorithm is unchanged.

- [ ] **Step 9: Wire `BuiyPickingBackendPlugin` into the top-level plugin**

In `crates/buiy/src/lib.rs`, two edits.

(a) Re-export:

```diff
-    picking::Hovered,
+    picking::{BuiyPickingBackendPlugin, Hovered},
```

(b) Compose into `BuiyPlugin::build`, immediately after `PickingPlugin`:

```diff
             buiy_core::layout::LayoutPlugin,
             buiy_core::picking::PickingPlugin,
+            buiy_core::picking::BuiyPickingBackendPlugin,
             WidgetsPlugin,
         ));
```

- [ ] **Step 10: Run the full picking suite**

```bash
cargo test -p buiy_core --test picking --test picking_backend
```

Expected: existing `hit_test_returns_entity_under_point` still passes (proves the free function is intact); new backend test passes.

- [ ] **Step 11: Run the workspace and example**

```bash
xvfb-run -a cargo test --workspace
```

Expected: all green, no regressions in the existing e2e fixture.

- [ ] **Step 12: Commit**

```bash
git add crates/buiy_core/src/picking/ crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs crates/buiy_core/tests/picking_backend.rs
git commit -m "feat(buiy_core): bevy_picking backend with Hovered as thin layer"
```

---

## Task 6: Final verification

**Files:**
- None new. This task runs the project's check command and addresses any leftover.

- [ ] **Step 1: Run the full check command**

```bash
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

Expected: every line PASS. Fix any warning at root cause; do not relax `-D warnings`.

- [ ] **Step 2: Re-run cargo-deny**

```bash
cargo deny check
```

Expected: PASS.

- [ ] **Step 3: Visual smoke test**

```bash
cargo run --example hello_button
```

Expected: window opens with a visible "Save" button rectangle painted via Buiy's pipeline (Task 3 verification). Pressing Tab causes the focus ring to appear (existing Phase 0 focus). On Linux with AT-SPI active, a screen reader announces "Save, Button" (Task 4 verification — manual gate).

- [ ] **Step 4: Update CHANGELOG**

In `CHANGELOG.md`, under `## [Unreleased]`, add:

```markdown
### Added
- Render pipeline now produces real pixels for Buiy nodes (instance-buffer
  construction, clip-space conversion, draw call). Closes the Phase 0
  render deferral.
- Per-window AccessKit adapter map. Real screen readers attached to a
  Buiy window receive a tree update each frame. Closes the Phase 0 a11y
  deferral.
- `bevy_picking` backend. `Hovered` becomes a thin layer over the standard
  `PointerHits` event flow. Closes the Phase 0 picking deferral.
```

- [ ] **Step 5: Flip this plan's own status**

In `docs/plans/2026-05-08-buiy-phase-0-closeout.md` (this file), line 4:

```diff
-**Status:** draft
+**Status:** landed
```

In `docs/README.md`, flip this plan's index entry's `[draft]` → `[landed]`.

- [ ] **Step 6: Final commit**

```bash
git add CHANGELOG.md docs/plans/2026-05-08-buiy-phase-0-closeout.md docs/README.md
git commit -m "docs: changelog + status flips for Phase 0 closeout"
```

- [ ] **Step 7: Open the PR**

```bash
git push -u origin <branch>
gh pr create --title "Phase 0 closeout: render draws, AccessKit per-window, bevy_picking backend"
```

PR description should cite the three Phase 0 deferrals being closed and link to this plan.

---

## Self-review (plan author)

Cross-checked against the foundation spec self-review (foundations plan lines 2784–2790):

- ✅ `bevy_picking` real backend implementation — Task 5. Forced API adjustments: `PointerHits` is a `Message` (not `Event`) in Bevy 0.18 → `MessageWriter`/`MessageReader`; `PickingSystems::Backend` (not `PickSet::Backend`); `bevy::picking::PickingPlugin` had to be added to `BuiyPlugin` because it's the sole registrar of `PickingSystems` sets and `Messages<PointerHits>`.
- ✅ AccessKit `accesskit_winit::Adapter` per-window wiring — Task 4. **Architectural deviation from this plan's body:** Bevy 0.18 owns adapter creation (`accesskit_winit::Adapter::*` constructors all require `&ActiveEventLoop` which only exists inside the winit runner; bevy_winit creates adapters in `prepare_accessibility_for_window` and stores them in `ACCESS_KIT_ADAPTERS`). Buiy could not own a `HashMap<WindowId, Adapter>` resource as the plan body described. Implemented as a *bridge*: `push_tree_updates` reaches into `bevy::winit::accessibility::ACCESS_KIT_ADAPTERS` each frame and pushes Buiy's `TreeUpdate`. The per-window outcome (one `Adapter` per window, fed our tree) is preserved; ownership is delegated to bevy_winit. See `crates/buiy_core/src/a11y/adapter.rs` module doc-comment for the reasoning. The `AccessKitAdapters` resource was dropped during the Task 4 review loop as it was dead weight under the bridge model.
- ✅ Render pipeline draws (instance-buffer construction) — Task 3.
- ⚠️ Cosmic-text / glyph rendering — **explicitly out of scope** per "Out of scope" header. Closeout's hello_button still shows a colored rect with no glyph rendering; the architecture-proof bar remains the right one for Phase 0.
- ⚠️ BSN authoring / hot-reload — **explicitly out of scope**.
- ⚠️ Forms / animation / devtools / 3D-anchored UI — **explicitly out of scope**.

Each remaining deferral has a sub-spec named in the foundation roadmap.

**Placeholder scan:** No "TBD" / "TODO" / "implement later" inside steps. Three Bevy-version-note disclaimers (`bevy_picking` API arity, `accesskit_winit` 0.31 handler trait names, `bevy::winit::WinitWindows` accessor shape) are honest version-stability acknowledgements per the same pattern Phase 0's plan used.

**Type consistency:** Names referenced across tasks are consistent: `InstanceData`, `to_instance`, `INSTANCE_STRIDE_BYTES`, `ExtractedDraws`, `BuiyPipeline`, `BuiyNode`, `A11yNodeView`, `A11yTreeBuilder`, `AccessKitAdapters`, `AccessKitAdapterPlugin`, `to_accesskit_node`, `build_tree_update`, `node_id_for`, `ROOT_NODE_ID`, `BuiyPickingBackendPlugin`, `PickSet::Backend`, `PointerHits`, `PointerLocation`. The `Hovered` resource keeps its public type and constructor; only its update system body changes.

**Dependency-graph health:** `accesskit` is added as a workspace dep; the cargo-tree check in Task 2 Step 4 ensures we don't ship two versions of `accesskit` (Bevy's vendored copy + ours). If duplication is unavoidable, the plan instructs to match Bevy's version pin.

**Risk register:**
- Bevy 0.18 minor-version drift on `bevy_picking::backend` and `bevy::winit::WinitWindows` accessor. Mitigated by the version note in the header and per-task disclaimers.
- AccessKit 0.21 API name drift (e.g., `Adapter::with_direct_handlers` vs `Adapter::new`). Mitigated by the same disclaimer pattern; structure is stable.
- Per-frame instance-buffer allocation cost. Acknowledged in `node.rs` comments and deferred to `buiy-render-pipeline-design`. Phase 0 closeout's hello_button has 1 entity, so the allocation cost is negligible at the demo scale.

---

Plan complete and saved to `docs/plans/2026-05-08-buiy-phase-0-closeout.md`.
