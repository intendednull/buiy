# Render node-draw model — per-entity clip + composite passes (design)

**Status:** landed — decided 2026-06-07 as **Option C (hybrid)**, shipped via R8b
(+ R9). Per-instance fragment-discard clip + the reserved multi-pass node (top-layer
composite, then R9 effect-group passes). R8 Task 8 + R9 implemented against it.
Surfaced 2026-06-06 when R8's plan could not express the consumer draw coherently on
R6's single-buffer draw model.
**Owners:** render-pipeline.
**Related:** [architecture.md § 1.3 / § 2](architecture.md) (pillar 2: typed-primitive batched node + one top-layer composite pass), [clip-and-transform.md § A](clip-and-transform.md) (`ClipRect`/`AncestorClip` consumption), [paint-order-and-top-layer.md § 3 / § 4](paint-order-and-top-layer.md) (top-layer composite), [effect-compositor.md](effect-compositor.md) (R9), R6 (`render/prepare.rs`, `render/node.rs`), R8 (`scissor_rect`, `clip_for_primitive`, `partition_top_layer`).

## Problem

R6 landed `BuiyNode::run` as a **single** `draw(0..4, 0..quad_count)` against one
persistent quad instance buffer + the view uniform. That model cannot express the
three things the next phases require:

1. **Per-entity rectangular clip (R8).** Each painted entity is clipped to its
   `ClipRect` (Outline to its `AncestorClip`). `scissor_rect`/`clip_for_primitive`
   (landed in R8) produce the rects, but nothing consumes them in the draw.
2. **Top-layer composite (R8 §3/§4).** Top-layer members paint last at the root,
   after the normal pass — a second ordered pass.
3. **Effect-group off-screen targets (R9).** `opacity`/`isolation` groups render
   to an intermediate `Rgba16Float` target, then composite back — intermediate
   passes per group.

R8's plan assumed a "per-batch scissor side-table feeding `BuiyNode::run`" but did
not reconcile it with R6's single-buffer draw or with the paint-order constraint,
so the implementing agent correctly stopped rather than hack.

## The load-bearing constraint

**Paint order must be preserved.** `painters_z` is the compositing order; render
must not reorder overlapping painters (paint-order-and-top-layer.md § 1.2 — a hard
constraint). So any grouping/batching the draw does must be **paint-order-neutral**:
it may only coalesce runs that don't change the visible result.

## Options

### A. Per-instance clip in the shader (fragment discard)
Add the clip AABB (and, for Outline, the ancestor AABB) to `PackedInstance`; the
fragment discards / zero-alphas outside it. One draw, order preserved, no grouping.
- **Pro:** keeps the single-draw model; trivially paint-order-neutral; extends to
  the reserved rounded clip (`ClipRadius`) by adding the radius later; cheap.
- **Con:** grows the instance (+4 f32, +16 B/instance); does **not** by itself
  solve top-layer or effect-group composite (those still need extra passes).

### B. Scissor-grouped batches (hardware scissor)
`prepare` groups instances by clip rect into contiguous runs; the node issues
`set_scissor_rect` + a sub-range `draw` per run.
- **Pro:** true hardware clipping (free); matches the plan's "scissor side-table".
- **Con:** a scissor group can only coalesce **contiguous same-clip** runs without
  violating paint order — interleaved clips degrade to many tiny draws; grouping
  logic in prepare is non-trivial; still needs separate passes for composite.

### C. Hybrid (recommended): fragment-discard clip (A) + reserved multi-pass node
Use **A** for per-entity rectangular clip (order-preserving, one draw), and build
the **composite passes the architecture already reserves** (pillar 2: "one ordered
composite pass for the top layer"; R9's per-group intermediate targets) as explicit
extra sub-passes in `BuiyNode::run` — normal pass → top-layer pass → (R9) effect-group
target passes + composite. `partition_top_layer` (landed) already splits the tail.
- **Pro:** clip stays cheap + order-trivial; the pass structure is exactly what
  architecture § 2 + effect-compositor.md describe; each phase (R8 top-layer, R9
  effects) adds one well-scoped pass without re-touching the clip path.
- **Con:** per-instance clip data (as A); the multi-pass node grows in complexity
  (but that growth is the spec's intended shape, not incidental).

## Recommendation

**Option C.** It keeps clipping in the cheap, order-safe per-instance/fragment path,
reserves hardware scissor as a later optimization for large opaque clip regions if
profiling wants it, and realizes the top-layer + effect-group passes through the
multi-pass node structure architecture § 2 already commits to. Concretely:
1. R8 Task 8: thread `ClipRect`/`AncestorClip` (via `clip_for_primitive`) into the
   instance; discard in the quad/shadow fragment; add the top-layer composite pass
   driven by `partition_top_layer`.
2. R9: add per-`EffectGroup` intermediate targets + a composite step in the same node.

**Ratified 2026-06-07.** R8b ([2026-06-07-buiy-render-r8b-node-draw.md](../../plans/2026-06-07-buiy-render-r8b-node-draw.md))
implements this: the clip AABB is threaded `ExtractedNode` → `PackedInstance`
(stride 36 → 52) → the quad/shadow WGSL fragment discard, and the top-layer/effect
multi-pass extension point is reserved in `BuiyNode::run`. R9 adds the effect-group
passes against the same reserved structure.
