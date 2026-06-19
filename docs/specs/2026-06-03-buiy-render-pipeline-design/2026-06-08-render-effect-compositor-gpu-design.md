# Effect-compositor GPU orchestration — design note

**Date:** 2026-06-08
**Status:** landed (GPU-verify campaign Phase 4, item 5)
**Implements** effect-compositor.md §1.1/§2/§3 — the GPU dataflow R9 had left inert
and the campaign built. R9 shipped the pure math + structural seams (headless-tested)
but left `prepare_effect_groups` an empty body, the `BuiyNode::run` composite loops a
`let _ = group`, and extract emitting a FLAT node list with no group membership; this
note's "Dataflow"/"Node two-pass" design is what filled those in (the
`prepare_effect_groups` body, the group/composite passes, and per-view group
membership are now live — see the "As-landed deviation" note in the forks below and
[effect-compositor.md](effect-compositor.md)).

## Scope

Make group `opacity` (and `isolation`) composite **once** through an off-screen
`Rgba16Float` target — which made the 2 `render_compositor_gpu.rs` stubs real:
`group_opacity_overlap_is_single_layer_at_half`, `rt_pool_returns_to_baseline_after_idle`.
v1 carries the two SC-forming formers (opacity, isolation); `backdrop-filter`
(EffectGroup-but-not-SC) + filters stay reserved (no v1 shader).

## Decided forks (exploration blueprint, workflow `wucy4tq5e`)

1. **Device handles + instance ranges live on a SIBLING carrier**, not on the pure
   `PreparedEffectGroup` (which is `Copy+PartialEq+Debug`, asserted in headless
   `render_compositor.rs`; `CachedTexture` is neither and is render-only). Add
   `PreparedEffectTargets { targets: Vec<CachedTexture>, ranges: Vec<Range<u32>> }`
   zipped to `PreparedEffectGroups` by the post-order `index`. Keeps the math/GPU
   split the module is built around.
2. **Minimal per-view attach.** `BuiyNode`'s `ViewQuery` reads `PreparedEffectGroups`
   off the **view entity** (`node.rs:47,53`) — a resource shim is NEVER seen, so
   `prepare_effect_groups` MUST `insert` both carriers onto the render-world view
   entity (v1/D2: the primary view). Keep `BuiyInstanceBuffers` as the resource
   shim (the node reads instance sub-ranges from it by the range metadata). The full
   per-view-component flip of the buffers (architecture §4) is the correct end state
   but is the SEPARATE view-routing follow-up — pulling it in here couples two
   deferrals and destabilizes the working flat path. **Rejected-for-now:** the full
   flip (call it out in the commit body).
3. **Double-paint exclusion = contiguous-range partition in the one buffer.**
   `pack_view` emits instances ordered `[in-flow-non-group..][group A..][group B..]
   ...[top-layer..]`, recording each `[start,end)`. The flat draw becomes the
   non-group sub-range draw(s); each group pass draws its own sub-range into its RT.
   A group's members are a contiguous `painters_z` slice by construction
   (`StackingContext.painters_z`), so contiguity + paint-order hold naturally
   (paint-order §1.2). No second buffer, no shader-side group test.
4. **Acquire pooled targets in `prepare_effect_groups`** (`RenderSystems::Prepare`),
   NOT in `run` — `TextureCache::get` needs `&mut TextureCache` which a `ViewNode`
   (`world: &World`) cannot obtain (the `node.rs:88-96` precedent). The acquired
   `CachedTexture`s ride the sibling carrier into the node and are HELD for the whole
   run (residency rule — a child target must not be recycled before its parent
   samples it).
5. **Group nesting derived during the existing `assemble_context_tree` walk** —
   thread an `enclosing_group: Option<usize>`; when a context root is an `EffectGroup`
   member, push its index and recurse with it as the new enclosing group. An effect
   group IS a stacking context for both v1 formers, so the SC nesting the walk
   already encodes IS the post-order nesting — no second ancestor pass.

   > **As-landed deviation + follow-up (2026-06-09):** the implementation derived
   > membership/nesting from the `EffectGroup` marker + a `ChildOf` nearest-former
   > climb in extract, NOT from SC boundaries — at the time `opacity` formed **no**
   > stacking context (the deferred layout trigger-5), so "an effect group IS a
   > stacking context" did not hold. That trigger has since landed
   > (`forms_stacking_context`, follow-ups.md "Phase 9 render-side stacking-context
   > formers"): an `opacity < 1` / `filter` / `mix-blend-mode` former now forms a
   > `StackingContext`. The membership derivation **stays as-is** — the ChildOf
   > climb is SC-agnostic, remains correct, and also covers the
   > `backdrop-filter` former (EffectGroup-but-never-SC). The SC's contribution is
   > paint-order **atomicity**: a group's subtree is one contiguous `painters_z`
   > slice, so `pack_view_partitioned`'s single-range contiguity (fork 3) holds by
   > construction (the buckets.rs `debug_assert` stays as a tripwire; GPU
   > regression: `tests/render_group_contiguity_gpu.rs`).

## Dataflow (the spine extension)

- `ExtractedNode` gains `group: Option<usize>` (index into the per-view group list).
- A per-view `Vec<EffectGroupExtract>` (group entity, `EffectReason`, `opacity: f32`,
  `parent: Option<usize>`, root box folded through `GlobalTransform`, transitive
  descendant boxes, resolved-px ink terms, `ClipRect`) emitted alongside the flat
  node list. The extract query fan + `Or<Changed>` set gain `EffectGroup` + `Opacity`
  (`Changed<EffectGroup>`, `Changed<Opacity>`) — keep them in lockstep (the
  damage-gate firewall).
- `prepare_effect_groups` reads that list, composes `painted_bounds → bucket_extent
  → group_target_descriptor` per group, `post_order_indices` for composite order,
  `plan_allocation(RT_POOL_BUDGET_BYTES)` for degradation, acquires the pooled
  `Rgba16Float` targets, and inserts `PreparedEffectGroups` + `PreparedEffectTargets`
  on the view entity.

## Node two-pass (`BuiyNode::run`)

- **Step 1** (was `node.rs:107-115`): per group in post-order, begin a pass into its
  `Rgba16Float` target (clear transparent), bind the view uniform `@group(0)`, set the
  group's instance sub-range, draw with the **Quad@Rgba16Float** pipeline.
- **Step 2** (was `node.rs:171-178`): composite each target into its parent (a nested
  group's target, or `view_target.main_texture_view()` at root) via the NEW composite
  pipeline applying `opacity * SrcOver` in post-order — the GPU form of
  `composite_src_over` (compositor.rs:259). **Blend in the PARENT's space:** group
  targets are `Rgba16Float` linear; the window is `Rgba8UnormSrgb`. Getting the encode
  wrong shifts the overlap color and fails the golden.
- **Flat draw** (`node.rs:146`): draw ONLY the non-group instance range(s) — the
  double-paint TODO. A group member drawn both flat AND composited doubles the overlap.

## New pipelines

- **Quad@Rgba16Float** — the existing `BuiyPrimitiveKind::Quad` specialization keyed
  on `Rgba16Float` (the format is already a `BuiyPrimitiveKey` field); instantiate it
  via a render-world `SpecializedRenderPipelines<BuiyPrimitives>` cache (today only the
  `Rgba8UnormSrgb` view variant is queued). The step-1 group passes bind this.
- **Composite** (`composite.rs` + `composite.wgsl`, NEW) — a textured-quad pass
  sampling a group's `Rgba16Float` target, blending SrcOver into the parent with
  `sampled.a * opacity`. Registered in `compositor::register` (resources/pipelines
  only, NO graph node — effect-compositor.md §3; the composite runs inside `BuiyNode`).

## Risks → mitigations (heed these; the reviewer checks them)

- **False-green (the worst):** if `prepare_effect_groups` writes the carriers as a
  resource or onto the wrong entity, the node's `Option<&PreparedEffectGroups>` is
  `None`, the loops stay inert, and a no-panic test passes WHILE PAINTING NOTHING.
  → the GPU test MUST assert PIXELS (the overlap composited once at 0.5), never just
  "no panic".
- **Double-paint** → the partition exclusion; `group_opacity_overlap` fails loudly if
  wrong (overlap reads doubled/over-bright).
- **Target residency** → acquire ALL targets up-front in prepare, hold them on the
  sibling carrier for the whole run; `update_texture_cache_system` (render `Cleanup`)
  only reclaims untaken textures.
- **Paint-order** → the contiguous-range partition preserves `painters_z` (group
  members are a contiguous slice by construction).
- **System count** — `prepare_effect_groups` GAINING params keeps it ONE system, so
  `BUIY_RENDER_SYSTEM_COUNT = 2` (render_compositor_gpu.rs / render_prepare.rs) stays
  valid. If a NEW prepare system is added, bump it in lockstep.

## Tests (real, pixel-asserting — reuse `support::render_to_image`/`readback_rgba`)

- `group_opacity_overlap_is_single_layer_at_half`: an `Opacity(0.5)` parent over TWO
  overlapping opaque-red children; read back the overlap pixel — it must equal red
  composited ONCE at 0.5 over the backdrop (drive the expectation with
  `composite_src_over`), NOT `0.5*0.5` doubled; a non-overlap red pixel = the same
  0.5 red (proves no double-darken). This is the regression that the off-screen pass
  (not the rejected per-child approximation) shipped.
- `rt_pool_returns_to_baseline_after_idle`: churn opacity-group membership across
  frames (transient targets), then idle past the 3-frame TextureCache reclaim; assert
  the `buiy_effect_group_target` bucket count returns within ε of the steady-state
  working set (the mechanism, guaranteed by painted-bounds sizing + descriptor-keyed
  reuse + no bespoke eviction). ε/slope numbers defer to buiy-verification-design.

## Verification

Both stubs green on the RX 6700 XT via `--ignored`; headless gate stays green (the new
pipelines compile; the device-free compositor-math tests unchanged). No new deps.
`@group(0)` unchanged. The Quad@Rgba8UnormSrgb flat path unaffected for non-group frames.
