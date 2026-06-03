# Buiy render — effect-group compositor

**Parent:** [README.md](README.md)

This file specifies pillar 6: the off-screen effect-group compositor. It owns the target state of `EffectGroup` ([README § 3.2](README.md#32-render-owned-this-spec-introduces)) — how an entity whose subtree must composite atomically gets an off-screen render target, how those targets are allocated, pooled, and evicted, how the per-`EffectGroup` results composite back into the per-window node group, and exactly what v1 carries (group `opacity` + `isolation`, both *correct*) versus what is reserved (`Filter` / `BackdropFilter` / `MixBlendMode`, component + boundary now, shader later).

It does **not** own: the typed-primitive batched node, the per-window node group placement in `Core2d`, or the extract/handoff — those are [architecture.md](architecture.md). It does **not** own paint order or top-layer membership — that is [paint-order-and-top-layer.md](paint-order-and-top-layer.md); the compositor *consumes* `StackingContext.painters_z` to know which entities paint into a group's target and in what order. It does **not** own the texture atlas — atlas allocation, warmup, and eviction are [atlas-and-text-seam.md](atlas-and-text-seam.md); this file reuses the *same eviction discipline* (the `frames_since_last_use` pool described below) but applies it to render targets, not glyph cells.

## 1. The identity: effect-group set ≡ effect-forming stacking-context set

The decisive, settled fact (pillar 6, [README § 2](README.md#2-architectural-pillars-one-line-summaries)): **the set of entities that need an off-screen render target is exactly the set of stacking-context triggers that are *effects*.** This is not a coincidence to be re-derived per feature — it is a structural property of compositing, observed independently by WebRender ([prior-art/servo-stylo/rendering.md](../../prior-art/servo-stylo/rendering.md) "Effects force off-screen passes") and Blink's effect property tree ([prior-art/blink/stacking-and-paint.md § 3](../../prior-art/blink/stacking-and-paint.md)): anything that can't be expressed in a single forward shader pass — `opacity < 1` over a group, `filter`, `backdrop-filter`, `mix-blend-mode`, isolation — *must* first rasterize its whole subtree to an intermediate target, then apply the effect to that target as a unit. Stacking-context boundaries **are** the GPU's natural render-pass boundaries.

Layout's stacking sub-pass 6f already enumerates the full SC-trigger union in one place ([layout stacking-and-top-layer.md § 2](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)). The compositor does **not** re-enumerate it. It consumes a *subset marker*: an entity carries `EffectGroup` iff it triggers an SC for an **effect** reason, i.e. one of:

- `Opacity` with `opacity < 1` (**v1, carried**);
- `Stacking { isolation: Isolation::Isolate, .. }` (**v1, carried**);
- `Filter` non-empty (**reserved**, component ships, no shader);
- `BackdropFilter` non-empty (**reserved**, component ships, no shader, plus a backdrop-sample seam — § 6);
- `MixBlendMode` other than `Normal` (**reserved**, component ships, no shader).

The triggers that form an SC but are *not* effects — positioned + `z_index`, non-identity `Transform`, `contain: paint`/`strict` — do **not** carry `EffectGroup`. They reorder and clip, both of which the forward typed-primitive pass already expresses (paint order via `painters_z`, clip via `ClipRect` from [clip-and-transform.md](clip-and-transform.md)); neither needs an intermediate target. This is the same separation Blink draws between its transform/clip trees and its effect tree, and the same one the layout prior-art insists 6f keep distinct: `isolation: isolate` is an effect boundary (→ `EffectGroup`) but does *not* clip, while `contain: paint` clips but is *not* an effect boundary (→ no `EffectGroup`) ([prior-art/blink/stacking-and-paint.md § 1.1](../../prior-art/blink/stacking-and-paint.md)).

### 1.1 Who writes `EffectGroup`

`EffectGroup` (**F**) is a render-owned marker, written by a render-prep system `WriteEffectGroups` that runs alongside `WriteClipRects` ([clip-and-transform.md](clip-and-transform.md)) reading only the effect components above. It is *derived state*, not author-set: an author sets `Opacity(0.5)`, and `WriteEffectGroups` inserts/removes the `EffectGroup` marker as the effect crosses the trigger threshold (`opacity` rising back to `1.0` removes it). Deriving it render-side (rather than having layout 6f emit it) keeps the marker on the side of the boundary that owns the compositor and avoids a second layout→render component edge; layout 6f independently reads the same `Opacity`/`Filter`/`MixBlendMode` components for its *SC-formation* clause (README § 4 unblock #2), so the two derivations share inputs but not the output.

```rust
/// Render-owned marker: this entity's subtree composites to its own off-screen
/// target before its effect (opacity / isolation / reserved filter|blend) applies.
/// Derived by `WriteEffectGroups`; never author-set.
#[derive(Component, Reflect, Debug, Clone, Copy)]
#[reflect(Component)]
pub struct EffectGroup {
    /// Which effect(s) made this a group, so the composite step knows what to apply.
    pub reason: EffectReason,
}

bitflags::bitflags! {
    /// One entity can carry several reasons at once (opacity<1 AND isolate).
    #[derive(Reflect, Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EffectReason: u8 {
        const OPACITY    = 1 << 0; // v1: carried
        const ISOLATION  = 1 << 1; // v1: carried
        const FILTER     = 1 << 2; // reserved: marks the group, no shader in v1
        const BACKDROP   = 1 << 3; // reserved: marks the group, needs backdrop sample (§ 6)
        const MIX_BLEND  = 1 << 4; // reserved: marks the group, no shader in v1
    }
}
```

The `reason` bitflags answer the composite step's only question: *what do I do with this group's target once it's filled?* In v1 the answer is "alpha-composite it at `opacity` (default `1.0`) with `SrcOver` blend." `ISOLATION` alone changes *nothing* about the composite math — an isolated group with `opacity == 1` composites identically to no group at all; isolation's whole effect is what it does to descendants' blending *within* the target (§ 5.2), not to the parent composite.

## 2. The off-screen target lifecycle

Each `EffectGroup` whose subtree is non-empty and not skipped (`Display::None`, `ContentVisibility::Hidden`, or an off-screen `Auto` — see [paint-order-and-top-layer.md](paint-order-and-top-layer.md)) acquires **one** off-screen color target per frame it paints. The lifecycle is acquire → paint subtree → composite → release, all within the render node's execution, all reusing Bevy's existing pooling primitive.

### 2.1 Sizing: painted bounds, not viewport

A group's target is sized to the **group's painted bounds** — the axis-aligned union of the group root's own box and every descendant's painted extent in the group's local space, *expanded* by ink that escapes the box: `BoxShadow` blur+spread radius, `Outline` width+offset, and (when the reserved filters land) a `Filter` blur's bleed. It is emphatically **not** sized to the window. A 24×24 icon at `opacity: 0.6` allocates a ~32×32 target (box + shadow ink), not a 4K render target. This is the single most important sizing decision for gate #15: viewport-sized intermediates are the classic way a compositor's memory blows past a flat budget the moment several groups are live.

Painted bounds come from layout's already-resolved geometry — the compositor does not measure. The group root's box is `ResolvedLayout { position, size }`; the descendant union is the bounding box of `painters_z` entries' boxes (each folded through the group-local `GlobalTransform`, [clip-and-transform.md](clip-and-transform.md)); ink expansion reads `BoxShadow`/`Outline`/`Filter` from [component-model.md](component-model.md). The result is a `URect` in physical pixels, snapped out to integer texel bounds. A group whose `contain: paint` also clips it (the root may be both an effect group *and* a paint-containment box) intersects the painted-bounds union with the group's own `ClipRect` first — clipped descendants cannot enlarge the target.

> **Open — sub-pixel/transform-scaled bounds calibration.** When a group root carries a `scale()` or non-axis-aligned `rotate()`, the *tight* painted bounds in target space versus the *device* bounds it composites into is a quality/memory tradeoff (render at target-local resolution then transform, vs. render pre-magnified). v1 sizes the target to the group-local untransformed bounds and applies the group transform at composite time; the magnified-quality variant is deferred. Flagged against [README § 5 #4](README.md#5-open-questions) (per-fixture perf/leak calibration).

### 2.2 Allocation + pooling: the `frames_since_last_use` pool

Targets are **pooled and reused**, never allocated fresh per group per frame in steady state. Buiy reuses Bevy 0.18's existing render-target pool — `bevy_render::texture::TextureCache` (`crates/.../bevy_render-0.18.1/src/texture/texture_cache.rs`) — rather than inventing a parallel one. `TextureCache::get(&render_device, descriptor)` returns a `CachedTexture { texture, default_view }`, reusing any non-`taken` texture whose `TextureDescriptor` matches exactly (same size, format, usage), or creating one on a miss. This is the right primitive because:

- **Descriptor-keyed reuse** means two groups that resolve to the same snapped size + format share the pool bucket and recycle each other's targets frame-to-frame. The compositor snaps painted bounds to a small set of **bucket sizes** (next-power-of-two per axis, capped) precisely so the descriptor matches across frames and across groups — a continuously-resizing target would defeat reuse by minting a fresh descriptor every frame.
- **Eviction is automatic and bounded.** `TextureCache::update()` (run by `update_texture_cache_system` in the render app's cleanup, which Buiy's per-window node group does **not** override) increments every entry's `frames_since_last_use`, marks all `taken = false`, and `retain`s only entries with `frames_since_last_use < 3`. So a target unused for 3 consecutive frames is dropped and its GPU memory returns. A group that stops being a group (`opacity` animates back to `1.0`, `EffectGroup` removed) frees its target within three frames with zero bespoke bookkeeping.

The target descriptor is fixed:

```rust
TextureDescriptor {
    label: Some("buiy_effect_group_target"),
    size: Extent3d { width, height, depth_or_array_layers: 1 }, // bucketed painted bounds
    mip_level_count: 1,
    sample_count: 1,
    dimension: TextureDimension::D2,
    format: view_target.main_texture_format(), // match the parent target's HDR/sRGB format
    usage: TextureUsages::RENDER_ATTACHMENT  // group subtree renders into it
         | TextureUsages::TEXTURE_BINDING,   // composite pass samples it
    view_formats: &[],
}
```

Matching `ViewTarget::main_texture_format()` (Bevy's `Rgba16Float` HDR target by default, [color-and-forced-colors.md](color-and-forced-colors.md)) keeps the group's pixels in the same linear-light space as the parent so the composite is a plain linear-space `SrcOver` — no per-group color conversion, and the whole tree still tonemaps once at the end (§ 4).

### 2.3 Why this makes gate #15 satisfiable

Gate #15 ([foundation verification.md gate #15](../2026-05-07-buiy-foundation/verification.md), [README § 5 #4](README.md#5-open-questions)) requires RSS slope `< 1 MB/min` after warmup and **atlas/target entries to return within ε of baseline** after a long-running fixture settles to idle. The pooling design satisfies it by construction, not by a leak-hunt:

1. **Bounded working set.** Live target memory is `Σ (bucketed painted-bounds area × bytes/texel)` over *currently-painting* groups, plus at most a few frames of slack from not-yet-evicted buckets. It is a function of how many groups are simultaneously on-screen, not of frame count or session length — so the slope is flat once the scene stops changing.
2. **Return-to-baseline.** When activity stops, no new `EffectGroup`s appear; within 3 frames every transiently-allocated target's `frames_since_last_use` reaches 3 and `update()` drops it. Bucket count returns to the steady-state working set, which *is* the baseline. The same `frames_since_last_use < 3` discipline backs the glyph atlas ([atlas-and-text-seam.md](atlas-and-text-seam.md)), so the gate's two clauses (RSS slope, atlas/target entries return) share one eviction model.
3. **No unbounded growth path.** Because sizing is painted-bounds (§ 2.1) not viewport, and reuse is descriptor-keyed, an adversarial fixture that opens and closes a thousand opacity groups over ten minutes never accumulates a thousand live targets — it recycles a handful of buckets. This is the explicit counter to WebRender's documented cost ("GPU memory for atlases and intermediate targets … a real ceiling," [prior-art/servo-stylo/rendering.md](../../prior-art/servo-stylo/rendering.md)).

Concrete per-fixture numbers (max live targets, bucket cap, ε) are owned by `buiy-verification-design` and calibrate over time ([README § 5 #4](README.md#5-open-questions)); this file commits the *mechanism* that keeps them achievable.

## 3. Composite pass ordering within the per-window node group

The compositor adds passes *inside* the per-window `BuiyRenderLabel` node group ([architecture.md](architecture.md)), which sits in `Core2d` after `Node2d::EndMainPass` and **before** `Node2d::Tonemapping` (verified against `bevy_core_pipeline-0.18.1` `graph::Node2d`: the order is `EndMainPass → StartMainPassPostProcessing → … → Tonemapping`). Everything the compositor does is therefore **pre-tonemapping**, in linear-light HDR space, so group output participates in the one final color-management/tonemap step ([color-and-forced-colors.md](color-and-forced-colors.md)).

Within one window's node execution, ordering is:

1. **Group subtree passes, innermost-first (bottom-up).** For each live `EffectGroup`, in an order that resolves children before parents (a group nested inside another group must be fully composited before its ancestor rasterizes, because the ancestor samples the child's *composited* result, not its raw subtree), the node:
   - acquires the group's pooled target (§ 2.2);
   - clears it transparent;
   - runs the typed-primitive batched pass ([architecture.md](architecture.md)) over the group's `painters_z` slice, into the group target instead of the window target. Nested `EffectGroup` descendants are *not* re-rasterized here — they appear as a single composited sample (step handled when their own, earlier, iteration ran).
2. **Group composite into parent target.** Each filled group target is drawn as one textured quad into its **parent target** (the next enclosing group's target, or the window's `ViewTarget::main_texture_view()` at the root), at the group's device position, applying the effect named by `EffectReason` — in v1, multiply sampled alpha by `Opacity::value` and blend `SrcOver`. This is the per-group equivalent of GPUI's "scene-level draw-order" composite ([prior-art/gpui/gpu-rendering.md](../../prior-art/gpui/gpu-rendering.md)), but with a real intermediate target so the group is alpha-composited as a unit (§ 5.1).
3. **Top-layer composite pass.** After the root window target holds the fully-composited normal-flow tree, the one ordered top-layer composite pass ([paint-order-and-top-layer.md](paint-order-and-top-layer.md)) draws top-layer entries (and their `::backdrop`) above it. Top-layer entries that are *themselves* effect groups (a modal at `opacity: 0.9`) composite their own target in step 1 like any other group, then draw in this step in top-layer order — the two mechanisms compose without special-casing.

The "bottom-up, children before parents" order is the compositing inverse of `painters_z`'s forward paint walk, and it is derivable from the same structure: a group's target depends on its descendant groups' targets, so a post-order traversal of the effect-group nesting (itself a sub-forest of the stacking tree `painters_z` encodes) yields the dependency-correct order. The compositor does not compute a new tree; it post-orders the `EffectGroup`-bearing entities by their position in the stacking nesting layout already produced.

```rust
/// Runs inside the per-window BuiyRenderLabel node group, between
/// Node2d::EndMainPass and Node2d::Tonemapping. Sketch of `ViewNode::run`.
fn run(&self, graph, render_context, (view_target, buiy_window): QueryItem, world) -> Result<(), NodeRunError> {
    // 1. innermost-first: each group's subtree → its pooled target
    for group in self.groups_post_order(world) {            // children before parents
        let target = texture_cache.get(device, group.descriptor()); // pooled (§ 2.2)
        render_context.command_encoder()... // clear + typed-primitive pass into `target`
    }
    // 2. composite each group target into its parent target, applying EffectReason
    for group in self.groups_post_order(world) {
        composite_quad(parent_target_of(group), group_target, group.opacity, SrcOver);
    }
    // 3. top-layer composite into the root window target (see paint-order-and-top-layer.md)
    // ... still pre-tonemapping; Node2d::Tonemapping consumes the result.
    Ok(())
}
```

## 4. v1 semantics: correct group opacity (not the rejected approximation)

The settled, user-approved decision (pillar 6 runner-up explicitly rejected): **v1 group opacity is correct, not a forward per-child approximation.**

- **Correct (shipped):** the whole group rasterizes to its target at full opacity, *then* the entire target is alpha-composited into the parent at the group's `opacity`. Overlapping children inside the group blend among themselves at full alpha first; the group's translucency applies once, to the composed result. A button label overlapping its background inside an `opacity: 0.5` group shows the *group* at half opacity — the overlap region does not double-darken.
- **Rejected (the approximation we do not ship):** multiplying each child's own alpha by the group `opacity` in the forward pass and skipping the intermediate target. This is cheaper (no off-screen pass, stays batched) but **wrong** wherever group descendants overlap: each overlapping pixel composites twice at reduced alpha, so the overlap shows through. CSS group opacity is defined as the former; the user chose correctness-from-day-one over the forward-pass speed, accepting the off-screen pass cost. This is recorded so the cost of the intermediate target is understood as *bought*, not accidental.

`isolation` in v1 is the second carried effect and is even simpler: it forms an `EffectGroup` (so its subtree rasterizes to an isolated target) but applies **no** parent-composite effect beyond `SrcOver` at `opacity` (default `1.0`). Its entire observable v1 behavior is the *isolation* itself — § 5.2.

## 5. What v1 actually guarantees

### 5.1 Group opacity over overlapping children

Given an `EffectGroup` with `EffectReason::OPACITY` and `Opacity(a)`, every entity in the group's `painters_z` slice paints into the group target at its own resolved alpha; the target is then composited into the parent once at `a`. The verification target (gate #2, [verification.md](verification.md)) is a golden fixture with two overlapping opaque children inside an `opacity: 0.5` parent: the overlap region must equal the single-layer color at 50%, **not** a doubled composite. That golden is the regression guard that the off-screen pass — not the rejected approximation — is what shipped.

### 5.2 Isolation creates an isolated group

`Isolation::Isolate` makes the group target the blending root for its descendants: any descendant `MixBlendMode` (a reserved C-tier effect) blends only against siblings *within* the isolated target, never against content painted behind the group in the parent. In v1, with no blend shader, isolation's *visible* effect is nil for a tree that uses only `SrcOver` — an isolated group of straight-alpha children composites identically to the same children un-isolated. The v1 guarantee is structural, not pixel-visible: the group **boundary** exists and the descendants rasterize into a separate target, so when `MixBlendMode` lands (§ 6) its blend is already correctly scoped without re-architecting. This is exactly why the compositor is built in v1 even though only opacity is pixel-observable: isolation reserves the blend-isolation seam.

## 6. Reserved seams: filter / backdrop-filter / mix-blend-mode

These three are **C-tier reserved** ([README § 3.2](README.md#32-render-owned-this-spec-introduces), foundation [visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling)). v1 ships their **component model** ([component-model.md](component-model.md): `Filter`, `BackdropFilter`, `MixBlendMode`) and their **`EffectGroup` boundary** (each sets its `EffectReason` bit and allocates a target exactly as opacity/isolation do), but **no shader**. The deferral is deliberate and the seam is shaped so adding the shader later needs no re-architecture:

- **`filter` (`EffectReason::FILTER`).** The group already rasterizes to its own target. A filter (blur, brightness, drop-shadow, …) is a fragment-shader pass applied to that target *before* the composite quad in step 2 — a new pass slotted between "target filled" and "target composited," reading the target, writing a filtered target (a second pooled allocation from the same `TextureCache`), then compositing. v1 already sizes the target with filter-blur ink expansion in painted bounds (§ 2.1) so the future blur has the margin it needs. **Adding a filter is: write the shader, insert one pass.** No change to allocation, pooling, ordering, or the marker.
- **`mix-blend-mode` (`EffectReason::MIX_BLEND`).** The composite quad in step 2 already chooses a blend op (`SrcOver` in v1). A blend mode is a *different blend op* on that same quad against the parent target, scoped correctly because isolation (§ 5.2) defines the blending root. **Adding a blend mode is: extend the composite blend-op selection.** The isolation boundary it needs already ships v1.
- **`backdrop-filter` (`EffectReason::BACKDROP`) — the one seam that needs more than v1's machinery.** A backdrop filter blurs the *content behind* the element, not the element's own subtree. So unlike `filter`, it cannot be expressed by sampling only the group's own target — it must **sample the parent target's region under the group** *before* the group's descendants paint over it, filter that sample, then composite the group on top. The seam this file reserves: the composite step has access to the parent target (it composites *into* it), and Bevy's `ViewTarget` already provides the ping-pong primitive this needs — `ViewTarget::post_process_write()` (verified in `bevy_render-0.18.1` `view/mod.rs`) hands a read-source/write-destination pair so a pass can sample what's already drawn while writing forward. **Adding backdrop-filter is: before step 1 rasterizes the backdrop group's subtree, sample the parent region (via the post-process-write source), run the filter shader into a backdrop scratch target, and use it as the group's cleared background instead of transparent.** The note to carry forward: backdrop-filter is the only reserved effect whose *input* is the parent rather than the subtree, so it is the one that must read the parent target — every other effect reads only its own group target. The component and `EffectReason::BACKDROP` bit ship v1 precisely so this ordering constraint (backdrop group composites against an already-painted parent region) is reserved now, not retrofitted.

No reserved effect changes the marker, the pool, the sizing, or the pass *structure*; each is "one shader + (for backdrop) one parent-sample." That is the test of the seam being right.

## Verification

How the claims here are proven, mapped to the CI gates ([foundation verification.md](../2026-05-07-buiy-foundation/verification.md), [verification.md](verification.md)):

- **Group-opacity correctness (gate #2 visual-regression).** The overlapping-children-under-`opacity:0.5` golden (§ 5.1) is the regression guard that the off-screen pass shipped and the rejected per-child approximation did not. A second golden with three nested opacity groups verifies the bottom-up composite order (§ 3).
- **`EffectGroup` derivation (gate #1 unit + gate #5 layout-snapshot adjacent).** `WriteEffectGroups` is headless-testable with no GPU: assert that `Opacity(0.5)` yields `EffectGroup { reason: OPACITY }`, `opacity` back to `1.0` removes the marker, `Isolation::Isolate` yields `ISOLATION`, and `contain: paint` / positioned-`z_index` yield **no** `EffectGroup` (the § 1 separation). This is the headless half — no wgpu adapter needed ([README goal #6](README.md#goals)).
- **Pooling + return-to-baseline (gate #15 RSS/atlas).** The long-running fixture opens and closes opacity groups, then idles; the assertion is RSS slope `< 1 MB/min` and live target-bucket count returning within ε of the steady-state working set after `< 3` frames of idle — satisfiable because eviction is the bounded `frames_since_last_use < 3` retain (§ 2.3). The bucket count is observable as the `TextureCache` entry count for the `buiy_effect_group_target` descriptor family.
- **No re-architecture for reserved effects (review-time, not a runtime gate).** The seam claims in § 6 are verified structurally: the `EffectReason` bits, the painted-bounds ink expansion, and the parent-target access in the composite step exist in v1 code, so the C-tier follow-up PRs add shaders and (for backdrop) one parent-sample without touching the marker, pool, sizing, or pass structure.
