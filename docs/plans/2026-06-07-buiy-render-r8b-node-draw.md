# R8b: Hybrid Node-Draw Model Implementation Plan

**Date:** 2026-06-07
**Status:** landed
**Spec:** [specs/2026-06-03-buiy-render-pipeline-design/2026-06-06-render-node-draw-model-design.md](../specs/2026-06-03-buiy-render-pipeline-design/2026-06-06-render-node-draw-model-design.md) (DECIDED: Option C hybrid)
**Supersedes:** [plans/2026-06-03-buiy-render-r8-paint-clip-toplayer.md](2026-06-03-buiy-render-r8-paint-clip-toplayer.md) Task 8 only. Tasks 1–7 of that plan (`scissor_rect`, `clip_for_primitive` in `render/clip.rs`; `partition_top_layer` in `render/top_layer.rs`) are landed and consumed here without re-implementation.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Thread the per-entity clip AABB through `ExtractedNode` → `PackedInstance` → WGSL so the quad/shadow fragment shaders discard pixels outside the clip AABB (one draw, order-safe, no grouping). Top-layer members receive the full-view sentinel (`clip = None` → packed as `[NEG_INFINITY, INFINITY]` → discard never fires → they paint unclipped at the `painters_z` tail). Reserve the multi-pass node structure for R9 effect groups with a documented stub in `BuiyNode::run`.

**Architecture:** Per-instance fragment-discard clip (the decided hybrid). The original R8 Task 8 assumed a per-batch scissor side-table on R6's single-buffer draw with no per-draw loop — irreconcilable with the landed `prepare::pack_extracted_nodes → BuiyInstanceBuffers → BuiyNode::run draw(0..4, 0..quad_count)` architecture. The fragment-discard approach keeps one draw, preserves `painters_z` order verbatim (no grouping/re-sort — paint-order § 1.2 hard constraint), and reserves the multi-pass node for the top-layer/effect composite (R9).

**Depends on (all landed):** R5 (`render/extract.rs`), R6 (`render/prepare.rs`, `render/node.rs`, `render/buckets.rs`), R7 (`render/primitive.rs`, `render/shader.wgsl`, `render/shadow.wgsl`), R8 helpers (`render/clip.rs::clip_for_primitive`; `render/top_layer.rs::partition_top_layer`).

**Tier/Test reality:** HEADLESS for struct layout, packing, naga WGSL parse, extract field population, ordering logic. GPU `#[ignore]` for actual draw assertions (no wgpu adapter on this host/CI). All GPU tests mirror the existing `render_smoke.rs` `#[ignore]` idiom. **Verify the actual current shapes by reading the files — the snippets below are the intended end state; reconcile against reality before editing (R5/R6/R7 may differ in detail).**

---

## The gate (keep green at every commit)

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace -j 2
```

`cargo test --workspace` runs only non-`#[ignore]`d tests. GPU tests are gated; CI passes with no adapter. Use `-j 2` for the test step (this host link-OOMs at full parallelism under mold).

---

## Orientation: current shapes this plan changes

Read these first (paths from the worktree root) and confirm the exact current shapes before editing:

- `crates/buiy_core/src/render/extract.rs` — `ExtractedNode` (currently `{ entity, position, size, color }`, **no clip**); `extracted_node_for(entity, gt, layout, background, theme)`; `extract_buiy_nodes` query fan + `Changed` `Or`-set. **Adds `clip: Option<ClipRect>` + a `clip` param to `extracted_node_for`, and `Option<&ClipRect>`/`Option<&AncestorClip>`/`Option<&Stacking>` to the query.**
- `crates/buiy_core/src/render/instance.rs` — `PACKED_INSTANCE_STRIDE_BYTES = 36`; `PackedInstance` = 9 f32; `pack_instance`/`pack_extracted`; `packed_raw_stride_agrees` checks `[f32; 9]`. **→ stride 52, +`clip_min[2]`/`clip_max[2]`, raw `[f32; 13]`.**
- `crates/buiy_core/src/render/buckets.rs` — `InstanceBuckets` over `Vec<[f32; 9]>`; `packed_to_raw → [f32; 9]`. **All `[f32; 9]` → `[f32; 13]`.**
- `crates/buiy_core/src/render/prepare.rs` — `BuiyInstanceBuffers.quad: RawBufferVec<[f32; 9]>`; `pack_extracted_nodes → (Vec<[f32; 9]>, [f32; 12])`. **Both → `[f32; 13]`.**
- `crates/buiy_core/src/render/primitive.rs` — the instance `VertexBufferLayout` (stride 36, last attr `@location(5)` at offset 32). **+`@location(6) Float32x2`@36, `@location(7) Float32x2`@44; stride 52.**
- `crates/buiy_core/src/render/shader.wgsl` + `shadow.wgsl` — `Instance`/`VertexOut`/vertex/fragment. **+clip attrs, +`rect_center`/`clip_min`/`clip_max` interpolants, +fragment discard.**
- `crates/buiy_core/src/render/node.rs` — `BuiyNode::run` single `draw(0..4, 0..quad_count)`. **+R9 reserved stub comment.**
- `crates/buiy_core/src/render/clip.rs` — `clip_for_primitive(...)` (READ its exact signature) + `ClipRect`/`AncestorClip`. **Consumed, not changed.**
- `crates/buiy_core/tests/render_smoke.rs` — the `#[ignore]` GPU idiom + exact wording to mirror.

**Find every `ExtractedNode { ... }` struct literal to update** (Rust requires all fields; add `clip: None`): run `grep -rn "ExtractedNode {" crates/buiy_core/` before Task 1 — known sites: `tests/render_extract.rs`, `tests/render_buckets.rs` (`node()` helper), `tests/render_prepare.rs`.

### Clip coordinate space — exact derivation

The WGSL vertex computes the fragment's logical-px position. With `local_uv = uv*2-1` and `half_size = rect_size*0.5`:

```
logical = rect_pos + uv*rect_size = rect_center + local_uv*half_size   (rect_center = rect_pos + rect_size*0.5)
```

In the fragment, `frag_pos = rect_center + local_uv*half_size` is logical px, y-down, window-relative — the **same space as `ClipRect.min/.max`**. No view-uniform transform for the comparison (the view uniform affects only `@builtin(position)`). Discard, before the SDF/blur, in both shaders:

```wgsl
let frag_pos = in.rect_center + in.local_uv * in.half_size;
if any(frag_pos < in.clip_min) || any(frag_pos > in.clip_max) {
    return vec4<f32>(0.0, 0.0, 0.0, 0.0);
}
```

### Full-view sentinel

`ExtractedNode.clip == None` (no clip, or a top-layer member) packs to `clip_min = [NEG_INFINITY; 2]`, `clip_max = [INFINITY; 2]`. For any finite `frag_pos`, both `any(< NEG_INF)` and `any(> INF)` are `false` → discard never fires. Both are valid `bytemuck::Pod` f32.

### Top-layer ordering — why one draw suffices in v1

Layout sub-pass 6f places top-layer members at the **tail** of the root `painters_z`. `extract_buiy_nodes` walks in order → top-layer at the tail of `ExtractedNodes.nodes` → `pack_view` packs them last → the single draw emits them last (over everything prior). Their `clip = None` → sentinel → unclipped over the full view. No second draw in v1. The multi-pass extension (R9 per-`EffectGroup` targets) is reserved as a comment. `partition_top_layer` stays a landed helper for when top-layer needs an explicit separate pass (effects / ancestor-clip escape beyond the sentinel).

---

## Task 1 — `ExtractedNode` carries the per-primitive clip AABB (HEADLESS)

**Files:** `src/render/extract.rs`; tests `render_extract.rs`, `render_buckets.rs`, `render_prepare.rs`.

- [ ] **Write failing tests** (append to `tests/render_extract.rs`): `extracted_node_for_carries_clip_when_provided` (Some(clip)→carried, None→None) and `assemble_preserves_clip_per_entity`.
- [ ] Run, expect compile FAIL (`ExtractedNode` has no `clip`, `extracted_node_for` arity).
- [ ] **Impl** in `extract.rs`: add `pub clip: Option<ClipRect>` to `ExtractedNode` (doc: None = full-view sentinel; top-layer members always None per § 3.2). Add `clip: Option<&ClipRect>` param to `extracted_node_for` (before `theme`); set `clip: clip.copied()`. Extend the `extract_buiy_nodes` query fan with `Option<&ClipRect>`, `Option<&AncestorClip>`, `Option<&Stacking>` and the `Or<(…)>` set with `Changed<ClipRect>`, `Changed<AncestorClip>`. In the per-entity loop, compute `effective_clip`: top-layer (`stacking.map(|s| s.top_layer != TopLayer::None)`) → `None`; else `clip_for_primitive(false, clip_rect, ancestor_clip)`; pass `effective_clip.as_ref()`.
- [ ] Update **all** `ExtractedNode { … }` struct literals in tests (`grep -rn "ExtractedNode {"`) with `clip: None,`.
- [ ] `cargo test -p buiy_core` green. Run GATE. Commit: `feat(render): ExtractedNode carries clip AABB (Option<ClipRect>); top-layer sentinel`

---

## Task 2 — `PackedInstance` carries the clip AABB; stride 36 → 52 (HEADLESS)

**Files:** `src/render/instance.rs`, `buckets.rs`, `prepare.rs`; tests `render_instance.rs`, `render_buckets.rs`.

- [ ] **Write failing tests** (`tests/render_instance.rs`): `packed_instance_stride_is_52` (size_of PackedInstance == 52 == [f32;13]; const == 52), `pack_extracted_sets_clip_min_max_from_node_clip`, `pack_extracted_uses_full_view_sentinel_when_clip_absent` (±INFINITY), `packed_raw_stride_agrees_with_thirteen_floats`.
- [ ] Run, expect FAIL (stride 36).
- [ ] **Impl**: `PACKED_INSTANCE_STRIDE_BYTES = 52`; add `clip_min: [f32;2]`, `clip_max: [f32;2]` to `PackedInstance` (after `radius`). `pack_instance` (`DrawData` path, no clip) → sentinel. `pack_extracted` → `match node.clip { Some(c)=>(min,max), None=>(±INF) }`. `packed_raw_stride_agrees` checks `[f32;13]`. In `buckets.rs`, all `[f32;9]`→`[f32;13]` (`InstanceBuckets.batches`, `push`, `batches()`, `packed_to_raw` extended with the 4 clip floats). In `prepare.rs`, `quad: RawBufferVec<[f32;13]>` (+ `Default`), `pack_extracted_nodes → (Vec<[f32;13]>, [f32;12])`.
- [ ] Update existing stride/raw-type assertions in `render_instance.rs`/`render_buckets.rs` (36→52, `[f32;9]`→`[f32;13]`).
- [ ] `cargo test -p buiy_core` green. Run GATE. Commit: `feat(render): PackedInstance stride 36->52; [f32;13] raw; clip_min/clip_max packing`

---

## Task 3 — Vertex layout (stride 52) + WGSL clip attrs + fragment discard (HEADLESS naga + descriptor; GPU `#[ignore]`)

**Files:** `src/render/primitive.rs`, `shader.wgsl`, `shadow.wgsl`; tests `render_primitive_descriptor.rs`, `render_shader_wgsl.rs`, `render_smoke.rs`.

- [ ] **Write failing tests** (`render_primitive_descriptor.rs`): `instance_buffer_stride_is_52_with_clip_fields`, `instance_has_clip_min_at_location_6_offset_36` (Float32x2), `instance_has_clip_max_at_location_7_offset_44` (Float32x2). (`render_shader_wgsl.rs`): `quad_shader_with_clip_parses`, `shadow_shader_with_clip_parses` (naga parse + entry points).
- [ ] Run, expect FAIL (stride 36 in descriptor).
- [ ] **Impl vertex layout** (`primitive.rs`): instance `VertexBufferLayout` `array_stride: 52`, append `@location(6) Float32x2 @36`, `@location(7) Float32x2 @44`. Update the existing instance-stride descriptor test 36→52.
- [ ] **Impl WGSL** (both `shader.wgsl` and `shadow.wgsl`): add `clip_min`/`clip_max` to `Instance` at `@location(6)`/`(7)`; add `rect_center`, `clip_min`, `clip_max` to `VertexOut` (mind shadow's `@location(5)` is `blur` not `radius`); set them in the vertex fn; add the fragment discard (above) before the SDF/blur. VertexOut ends at 7 interpolants — within wgpu limits.
- [ ] `cargo test -p buiy_core --test render_shader_wgsl` + `--test render_primitive_descriptor` green.
- [ ] Add GPU `#[ignore]` `clip_aabb_pipeline_registers_with_stride_52` to `render_smoke.rs` (mirror the existing idiom + wording; assert `BuiyPipeline` registered under a real `RenderPlugin` — **adjust the exact plugin/resource API to whatever `render_smoke.rs` already uses so it compiles**).
- [ ] Run GATE. Commit: `feat(render): WGSL clip @location(6-7) + fragment discard; vertex layout stride 52`

---

## Task 4 — Top-layer composite pass in `BuiyNode::run` (HEADLESS logic + GPU `#[ignore]`)

Top-layer ordering is already correct (painters_z tail → packed last → drawn last; sentinel clip). This task documents the v1 single-draw rationale, reserves the R9 multi-pass extension point, and adds the headless sentinel test + GPU `#[ignore]` composite smoke.

**Files:** `src/render/node.rs`; tests `render_extract.rs`, `render_smoke.rs`.

- [ ] **Write tests** (`render_extract.rs`): `top_layer_entity_gets_none_clip_regardless_of_clip_rect` and `in_flow_clipped_entity_gets_clip_from_clip_for_primitive` (pure-logic guards on the Task-1 branch). Expect PASS (logic landed in Task 1).
- [ ] **Impl** (`node.rs`): after the single `pass.draw(...)`, add the v1-rationale + `R9 RESERVED` comment block (per-EffectGroup intermediate passes go here; v1 has no active second pass; paint order preserved verbatim).
- [ ] Add GPU `#[ignore]` `top_layer_composites_last_over_in_flow` to `render_smoke.rs` (mirror idiom; **adjust API to compile**).
- [ ] Confirm the `#[ignore]` test is collected but skipped (`cargo test -p buiy_core --test render_smoke 2>&1 | grep top_layer` → `ignored`).
- [ ] Run GATE. Commit: `feat(render): R9 multi-pass stub in BuiyNode::run; top-layer sentinel clip headless tests`

---

## Task 5 — Wire-up smoke + docs

**Files:** `tests/render_smoke.rs`, `docs/README.md`.

- [ ] Add final GPU `#[ignore]` `clip_aabb_full_wire_up_smoke` (full ClipRect→ExtractedNode→PackedInstance→WGSL path proven headlessly across Tasks 1–3; this stub proves wgpu accepts stride-52 + registers the pipeline on a real adapter — **adjust API to compile**).
- [ ] Update `docs/README.md` render-plans list with an R8b entry (status `[active]`/`[landed]` as appropriate): hybrid node-draw — per-instance clip AABB fragment-discard (stride 36→52, @location 6-7), top-layer full-view sentinel, R9 multi-pass stub.
- [ ] Run the full GATE. Commit: `docs(render): add R8b node-draw plan to docs index`

---

## Done criteria

- [ ] Gate green at every task boundary; all GPU tests `#[ignore]`d with `render_smoke.rs` wording.
- [ ] `PACKED_INSTANCE_STRIDE_BYTES == 52`; `PackedInstance` 52 bytes; raw `[f32; 13]` everywhere (`InstanceBuckets`, `BuiyInstanceBuffers.quad`, `pack_extracted_nodes`).
- [ ] Instance vertex `array_stride == 52`; `@location(6) Float32x2`@36; `@location(7) Float32x2`@44.
- [ ] Both shaders: `Instance` has `clip_min`/`clip_max` at `@location(6)/(7)`; `VertexOut` has `rect_center`/`clip_min`/`clip_max`; fragment discards outside `[clip_min, clip_max]`.
- [ ] `ExtractedNode.clip: Option<ClipRect>`; top-layer → `None`; in-flow → `clip_for_primitive(false, …)`; all struct literals updated.
- [ ] `BuiyNode::run` has the R9 reserved comment; one active draw in v1; `painters_z` order preserved verbatim (no render-side re-sort).
