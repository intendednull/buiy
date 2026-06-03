# Buiy render — architecture

**Parent:** [README.md](README.md)

This file defines the structural skeleton the rest of the render spec relies on: how Buiy's paint passes attach to Bevy 0.18's render graph, where the main-world → render-world boundary sits, the typed-primitive node group and the single top-layer composite pass it contains, per-window node-group ownership, the persistent-buffer/per-frame-instance handoff that retires the Phase-0 hacks, the system-set order including the render-prep stage, and the crate home of the render-side components. Every sibling file ([component-model.md](component-model.md), [clip-and-transform.md](clip-and-transform.md), [paint-order-and-top-layer.md](paint-order-and-top-layer.md), [effect-compositor.md](effect-compositor.md), [atlas-and-text-seam.md](atlas-and-text-seam.md), [color-and-forced-colors.md](color-and-forced-colors.md)) assumes the seams pinned here.

Tier legend: **F** = foundation, **C** = core (reserved seam, deferred shader/body), **E** = extended (named only). See [foundation/README.md](../2026-05-07-buiy-foundation/README.md#tier-legend).

The spine is [README.md § 2](README.md#2-architectural-pillars-one-line-summaries) (pillars) and [§ 3](README.md#3-the-component-contract) (the component contract). This file elaborates the *mechanism* of pillars 1, 2, 3, and 5; it does not relitigate them. The component contract's names are authoritative and used verbatim.

## 1. Render-graph integration

### 1.1 Plugin shape: register in `finish`, against the `RenderApp` sub-app

Buiy's render code lives behind one plugin, `BuiyRenderPlugin`, composed by `BuiyPlugin::finish` — **not** `build`. This is load-bearing and already correct in Phase 0 ([`crates/buiy/src/lib.rs`](../../../crates/buiy/src/lib.rs) `BuiyPlugin::finish`): the `RenderApp` sub-app and the `PipelineCache` resource it reaches for are inserted by `RenderPlugin::finish`, and Bevy runs plugin `finish` hooks in registration order. Registering render work in `build` would reach for a `RenderApp` that does not yet exist.

```rust
pub struct BuiyRenderPlugin;

impl Plugin for BuiyRenderPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<ExtractedNodes>()       // §3.1 — replaces ExtractedDraws
            .init_resource::<BuiyInstanceBuffers>()   // §3.2 — persistent buffers
            .add_systems(ExtractSchedule, extract_buiy_nodes)
            .add_systems(Render, prepare_buiy_instances.in_set(RenderSystems::Prepare));
        atlas::register(render_app);                  // atlas-and-text-seam.md
        compositor::register(render_app);             // effect-compositor.md
        node::register(render_app);                   // §2 — the node group
        pipeline::register(render_app);               // §1.4 — pipelines
    }
}
```

`app.get_sub_app_mut(RenderApp)` returns `Option`: if the app was built headless without a render sub-app, the early-return makes `BuiyRenderPlugin` a clean no-op. This is the same guard the Phase-0 plugin already uses and is what makes the component model, the render-prep passes (§5), and `ClipRect` geometry testable on CI runners that have no wgpu adapter (the headless constraint — see [verification.md](verification.md) and § 6 here).

> The `Plugin::finish` ordering contract — add `BuiyPlugin` *after* `DefaultPlugins` — is documented on `BuiyPlugin` itself and is a foundation-level invariant, not a render-spec decision. This spec inherits it.

### 1.2 The extract boundary: `ExtractSchedule` + `Extract<Query>`

Buiy follows Bevy's standard two-world split. The main world owns the ECS truth (layout output, render-side components); the render world owns GPU resources and the graph. Data crosses once per frame in `ExtractSchedule`, which runs after the main-world schedules complete — and, per the foundation pipeline, after `BuiySet::Render` (§5).

Extraction reads the main world through `Extract<SystemParam>`, the read-only cross-world accessor. The system signature is a fan of `Extract<Query<…>>` plus `Extract<Res<…>>`:

```rust
fn extract_buiy_nodes(
    mut extracted: ResMut<ExtractedNodes>,
    nodes: Extract<Query<
        (
            Entity,
            &GlobalTransform,                          // pillar 5 — composed by layout, propagated by Bevy
            &ResolvedLayout,                           // box size (position folded into GlobalTransform)
            &ClipRect,                                 // pillar 4 — written by WriteClipRects (clip-and-transform.md)
            Option<&Background>, Option<&Border>,      // component-model.md
            Option<&BoxShadow>, Option<&Outline>,
            Option<&Opacity>, Option<&EffectGroup>,    // effect-compositor.md
            &StackingContext, &Stacking,               // paint order + top-layer membership
        ),
        (With<Node>, Without<Display::None marker>),   // skip rules: paint-order-and-top-layer.md
    >>,
    theme: Extract<Res<Theme>>,
    windows: Extract<Query<(Entity, &Window)>>,        // all windows, not just primary — §4
) { /* §3.1: rebuild the per-frame instance set, Changed-gated */ }
```

This replaces Phase 0's `extract_buiy_draws`, which reads the temporary `Visual` ([`crates/buiy_core/src/render/mod.rs`](../../../crates/buiy_core/src/render/mod.rs)). The `Visual` → `Background` + `Border` migration is a plan concern ([README § 3.3](README.md#33-the-visual-migration)); the target extract reads the real component model. Theme-token resolution against `Res<Theme>` stays in extract (it is cheap and main-world-authoritative); [color-and-forced-colors.md](color-and-forced-colors.md) owns the resolution rules and the forced-colors swap.

**Extraction is one-directional and read-only.** It never mutates the main world. This is the render half of pillar 1 (immutable layout output, thin render consumer): the extract query reads layout-owned and render-owned components and writes only render-world state. It never re-sorts `StackingContext.painters_z`, never re-derives stacking, never recomputes geometry. Where render needs a derived value, a render-prep pass in the *main* world computes it first (§5, pillar 4).

### 1.3 Node placement: `ViewNode` + `ViewNodeRunner<BuiyNode>` in `Core2d`, after `EndMainPass`

The paint group is a `ViewNode` — a render-graph node parameterised by a per-view query, run once per view that matches. It is inserted into the `Core2d` sub-graph via the wrapper `ViewNodeRunner<BuiyNode>` (the bridge that turns a `ViewNode` into the graph's `Node` trait), with an explicit edge ordering it after `Node2d::EndMainPass`:

```rust
#[derive(RenderLabel, Hash, PartialEq, Eq, Debug, Clone)]
pub struct BuiyRenderLabel;

pub(crate) fn register(render_app: &mut SubApp) {
    render_app
        .add_render_graph_node::<ViewNodeRunner<BuiyNode>>(Core2d, BuiyRenderLabel)
        .add_render_graph_edge(Core2d, Node2d::EndMainPass, BuiyRenderLabel);
}
```

`add_render_graph_node` / `add_render_graph_edge` are the `RenderGraphExt` trait methods on `SubApp` (verified in `bevy_render::render_graph::app`). `BuiyRenderLabel` derives `RenderLabel` — this is the stable identity other nodes order against and the name pillar 2 reserves for the whole per-window node group (§4).

**Why after `Node2d::EndMainPass`, before tonemapping.** The verified `Node2d` ordering in `Core2d` is `StartMainPass → MainOpaquePass → MainTransparentPass → EndMainPass → … → Tonemapping → …`. Attaching the Buiy edge from `EndMainPass` places Buiy paint *after* the 2D scene's main pass but *before* `Node2d::Tonemapping`. The consequence the foundation cares about: Buiy widgets write into the same view color attachment as 2D scene content and therefore participate in the same HDR / tonemapping / color-management path when it is enabled, rather than each managing its own color-space matching. [color-and-forced-colors.md](color-and-forced-colors.md) owns the linear-light-render / sRGB-output contract that this placement enables; this section owns only the *graph edge* that makes it true.

This is the gpui / makepad convergence point ([prior-art/gpui/gpu-rendering.md](../../prior-art/gpui/gpu-rendering.md), [prior-art/makepad/gpu-rendering.md](../../prior-art/makepad/gpu-rendering.md)): a custom-shader 2D-UI pass attached to the host's render graph, after scene content, before final color resolve. It is also the deliberate departure from bevy_ui, whose render-graph node Buiy reuses *none* of — bevy_ui's renderer caps non-rect clipping, backdrop-filter, mix-blend-mode, isolation, and true top-layer compositing ([prior-art/bevy-ui/architecture.md](../../prior-art/bevy-ui/architecture.md) "Renderer caps"), which is exactly why Buiy owns the node.

### 1.4 Pipelines: `PipelineCache` + stable-UUID shaders

Each typed primitive (§2) owns a render pipeline, queued through the render world's `PipelineCache` at registration:

```rust
let id: CachedRenderPipelineId = pipeline_cache.queue_render_pipeline(descriptor);
```

`queue_render_pipeline` returns a `CachedRenderPipelineId` immediately; the cache compiles the pipeline asynchronously and `get_render_pipeline(id)` yields `None` until it is ready. The node's `run` early-returns `Ok(())` on `None` (the Phase-0 node already does this) — a not-yet-compiled pipeline produces a dropped frame, never a panic. `RenderDevice` (the wgpu device handle) backs buffer and bind-group creation; the descriptor's `ColorTargetState` matches the view target format so Buiy composites into the shared attachment (§1.3).

Shaders are loaded into the render world's `Assets<Shader>` under stable weak `Handle::Uuid` handles, following the **Buiy render-asset UUID convention** already pinned in [`crates/buiy_core/src/render/pipeline.rs`](../../../crates/buiy_core/src/render/pipeline.rs): the prefix `0xB01A_01XX_..` ("BUIY 01"), trailing octet per asset. Phase 0 owns `..01` (rounded-rect). This spec's target allocates one octet per primitive shader and per compositor/effect shader — e.g. `..01` quad/rounded-rect, `..02` shadow (SDF Gaussian), `..03` glyph-alpha (the [atlas-and-text-seam.md](atlas-and-text-seam.md) seam), `..04` path-SDF, `..05` the top-layer/effect composite — each documented in the `pipeline.rs` comment block as it lands. The exact octet assignment is an implementation/plan detail; the **convention and reserved range** (`0xB01A_0100_..` through `0xB01A_01FF_..`) are pinned here.

## 2. The typed-primitive batched node + one top-layer composite pass (pillar 2)

`BuiyNode` is not one shader — it is a small fixed set of typed SDF primitives, batched, plus one ordered composite pass, all inside the single `BuiyRenderLabel` graph node. This is the shape two production Rust GPU UIs converged on (gpui's ~8 primitive types each with its own pipeline and SDF fragment evaluation; makepad's `DrawQuad`/`DrawText` instanced primitives). Buiy adopts the reduced primitive set, not arbitrary triangle meshes.

### 2.1 The primitive set

| Primitive | Tier | What it paints | SDF / atlas |
|---|---|---|---|
| `quad` | F | Background fill + border band + rounded corners (per-corner elliptical radius). | SDF rounded-rect; border = outer-minus-inner SDF band. |
| `shadow` | F | `BoxShadow` entries (multiple, inset, spread, blur). Painted ahead of its caster so it blurs independently. | Closed-form Gaussian-blurred-rect SDF (one draw per shadow, no convolution pass). |
| `glyph` | F (seam) | Single text glyph: samples a single-channel **alpha** atlas, multiplies by per-instance color (the alpha-as-color trick). | [atlas-and-text-seam.md](atlas-and-text-seam.md) owns the atlas + the glyph-alpha primitive; `buiy-text-rendering-design` owns shaping. |
| `path` | C (reserved) | Filled arbitrary 2D shape (drop-down arrows, icons, `clip-path` shapes). | SDF-AA filled path. Component/pipeline seam ships; the C-tier shader is deferred. |

`Outline` (focus indicator) paints as a `quad` variant outside the border box, never clipped by the element's own `ClipRect` ([component-model.md](component-model.md) owns the rule; the primitive is the existing quad pipeline with the clip rect suppressed).

```rust
#[derive(Default)]
pub struct BuiyNode;

impl ViewNode for BuiyNode {
    // ViewTarget gives the color attachment; the rest is read from the render world.
    type ViewQuery = (Entity, &'static ViewTarget, &'static ExtractedView);
    fn run<'w>(&self, graph, ctx, view, world) -> Result<(), NodeRunError> {
        // 1. Resolve this view's window/node-group state (§4).
        // 2. For each (primitive, layer) batch: bind pipeline, bind view-uniform + atlas,
        //    set the persistent instance buffer slice (§3.2), issue one instanced draw.
        // 3. Run the single ordered top-layer composite pass (§2.3).
    }
}
```

### 2.2 Batching: one draw per (primitive, layer)

Instances are grouped by `(primitive type, paint layer)` and each group renders as a single instanced draw — gpui's batching shape, which lets a frame painting thousands of primitives issue single-digit draw calls. The "layer" is the forward walk of `StackingContext.painters_z` ([paint-order-and-top-layer.md](paint-order-and-top-layer.md)): the render node consumes the already-sorted order, never re-sorts it (pillar 1). Within a layer, primitives draw back-to-front by type in the fixed order shadow → quad → glyph → path so a glyph paints over its background quad and a shadow under its caster. Atlas-backed primitives share one atlas bind across all instances in the batch ([atlas-and-text-seam.md](atlas-and-text-seam.md)).

### 2.3 One ordered top-layer composite pass

Inside the same `BuiyNode`, after the in-flow layers, runs exactly **one** ordered composite pass for the top layer — the elements (modal `<dialog>`, popovers, tooltips) that paint above all in-flow stacking contexts and are not clipped by ancestor overflow. This is the CSS top-layer the foundation commits to and bevy_ui lacks. The pass consumes `Stacking.top_layer` membership and the layout-provided top-layer order; it composites top-layer content (and the `::backdrop` model — an [open question, README § 5 #3](README.md#5-open-questions), proposed render-synthesized in [paint-order-and-top-layer.md](paint-order-and-top-layer.md)) over the in-flow result. [paint-order-and-top-layer.md](paint-order-and-top-layer.md) owns the ordering identity and the `::backdrop` decision; this section pins only that there is **one** composite pass and it lives inside the per-window node group, not a separate graph node.

The off-screen **effect-group** compositing (a render target per `EffectGroup` for group `opacity` / `isolation`) is a distinct mechanism that also lives behind this node but is owned by [effect-compositor.md](effect-compositor.md). The two are kept separate: top-layer is a *paint-order* relocation (same surface, different draw position); effect-group is an *intermediate-target* operation (off-screen RT, then composite). Pillar 2's rejection of a single grow-in-place node is precisely because it cannot express the intermediate targets the effect compositor needs; the rejection of a full slimming-paint sub-graph is that it over-builds before a concrete C-tier need. The node-group-with-one-composite-pass is the middle the brainstorming settled on — do not re-open it.

## 3. The hybrid handoff (pillar 3)

Pillar 3 retires two Phase-0 stopgaps in one move: the per-frame `create_buffer_with_data` allocation in the node (`node.rs`) and the per-instance CPU y-flip / radius-px→clip approximation in [`crates/buiy_core/src/render/instance.rs`](../../../crates/buiy_core/src/render/instance.rs). The replacement is **persistent GPU buffers + atlas + a view uniform**, with the per-frame instance *set* rebuilt from a `Changed<T>`-gated extract. Damage tracking is ECS change-detection — the thing Buiy gets for free — and there are **no screen-space damage-rects in v1**.

### 3.1 Per-frame instance set, `Changed<T>`-gated

The render-world `ExtractedNodes` resource (replacing `ExtractedDraws`) is rebuilt each frame from the extract query (§1.2). The query is gated so that the common steady-state frame — nothing changed — does near-zero work, mirroring the layout pipeline's `O(0)` steady-state contract ([layout/architecture.md § 9](../2026-05-08-buiy-layout-design/architecture.md#9-performance-contract)). The change-detection trigger set for a re-extract is the union of the paint-input components:

```rust
Or<(
    Changed<GlobalTransform>, Changed<ResolvedLayout>, Changed<ClipRect>,
    Changed<Background>, Changed<Border>, Changed<BoxShadow>, Changed<Outline>,
    Changed<Opacity>, Changed<EffectGroup>, Changed<StackingContext>, Changed<Stacking>,
)>
```

This is the v1 damage mechanism. A frame where no paint input changed re-extracts nothing and re-uploads nothing; the persistent buffers from the prior frame are re-bound and re-drawn. The runner-up *full immediate rebuild* was rejected as wasting the gate-#14 render-time budget; the runner-up *full retained scene* was rejected as premature invalidation complexity (the classic stale-paint bug). ECS change-detection sits between them: retained buffers, change-gated rebuild, no hand-rolled invalidation graph.

> **Bevy extract caveat (faithful to the engine).** `Extract<Query<…>>` reads the *main* world's change ticks. Because the render world's extract runs every frame, `Changed<T>` filters in extract see main-world mutation since the prior extract — which is the semantics we want. The Phase-0 extract already rebuilds an `ExtractedDraws` each frame; the target keeps the per-frame *resource* but the change-gating means an unchanged entity contributes its *cached* instance record rather than being re-resolved. The cached records are keyed by `Entity` in `ExtractedNodes` so a partial re-extract patches only changed entities.

### 3.2 Persistent buffers + view uniform

`BuiyInstanceBuffers` is a render-world resource holding one growable GPU buffer per primitive type, allocated once and reused frame-to-frame (grow-in-place on capacity overflow, never reallocated per frame). A `prepare_buiy_instances` system in the `Render` schedule's `RenderSystems::Prepare` set writes the changed instance slices into these buffers via the queue, and the node binds slices of them. This is the upgrade the Phase-0 node comments already name ("v0.x upgrades to persistent buffers + bind groups").

The per-instance CPU coordinate work moves to the GPU via a **view uniform**. Today `instance.rs` packs `rect_pos`/`rect_size` in clip space, baking the y-flip into a negative height and approximating corner-radius px→clip with `2.0 / min(window.x, window.y)` — both are explicitly Phase-0 stopgaps in the source. The target uploads a Buiy view uniform (logical-pixel → clip-space transform for the view; conceptually the same role as `bevy_render`'s `ViewUniform`, which exists at `bevy_render::view::ViewUniform`) and `InstanceData` shrinks back to **logical-pixel** units. The vertex shader applies the view transform; the radius stays in logical pixels and the SDF fragment evaluates in pixel space, removing the non-square-window approximation entirely. The atlas (a persistent `TextureAtlas`-backed GPU texture, [atlas-and-text-seam.md](atlas-and-text-seam.md)) is the third persistent resource, bound per atlas-backed batch.

### 3.3 What stays out of v1

No property trees ([pillar 7](README.md#2-architectural-pillars-one-line-summaries)): render paints the current frame's resolved values and relies on change-detection. The named revisit trigger is *animating opacity (or transform) without re-running layout* — until that lands, `ResolvedTransform` and `painters_z` are the only "trees," and they are layout's, read forward. No screen-space damage-rects: ECS change-detection is the only damage signal. [effect-compositor.md](effect-compositor.md) holds the gate-#15 memory budget for the persistent atlas + RT pool so "persistent" does not become "unbounded."

## 4. Per-window node-group ownership (cross-cutting § 3.18 F-tier)

Coexistence is **per-window** ([foundation/cross-cutting.md § 3.18](../2026-05-07-buiy-foundation/cross-cutting.md#318-compatibility-and-coexistence)): each window is owned by exactly one stack, and per-window state — render-graph node group, AccessKit adapter, picking backend filter, focus root — is keyed by winit `WindowId`. The render-graph node ordering on a Buiy window is owned unilaterally by Buiy's per-window node group.

Bevy gives this to us at the right granularity for free: a `ViewNode` runs **once per view**, and each window's camera is a distinct view. `BuiyNode::run` therefore already executes per-window. The structural commitment this spec makes is that all per-window render state — the instance buffers (§3.2), the atlas binds, the top-layer composite target, the view uniform — is *keyed by the view's window*, not held as global singletons that assume one Buiy window. The extract query (§1.2) reads *all* windows (`Extract<Query<(Entity, &Window)>>`), not only `PrimaryWindow` as Phase 0 does, so the instance set partitions per window. `BuiyRenderLabel` names the whole per-window group; the typed-primitive batches and the one top-layer composite pass are members of it.

> **Reserved, not fully wired in v1 (D2).** Layout Phase 9 ships a single global `TopLayerActivation` resource that reads the primary window (the Phase-9 D2 simplification; `TopLayerActivation` is exported from `buiy_core::layout`). True per-window top-layer *routing* is owned by `buiy-window-and-surface-design` and is an [open question, README § 5 #1](README.md#5-open-questions). This spec **reserves** the per-window node-group structure — the keying, the per-view `ViewNode`, the all-windows extract — so the global-activation simplification is a tracked dependency that can be lifted without re-architecting the node group, not a baked-in single-window assumption. Do not design new render state that assumes one Buiy window.

## 5. System-set order and the render-prep stage

### 5.1 The foundation order, unchanged

Buiy's per-frame main-world work runs in `BuiySet`, chained in `Update` ([`crates/buiy_core/src/lib.rs`](../../../crates/buiy_core/src/lib.rs) `CorePlugin`):

```
BuiySet::Layout → Style → Input → Animate → Picking → A11yUpdate → Render
```

Extraction runs in `ExtractSchedule`, which executes after the main-world schedules — i.e. after `BuiySet::Render` — per [foundation/architecture.md § 2.8](../2026-05-07-buiy-foundation/architecture.md#28-module-organization). This spec adds no new top-level `BuiySet` variant; the render-prep work slots inside the existing chain.

### 5.2 The render-prep stage: between layout output and `Picking`

Two render-prep passes must run **after layout output is final but before `BuiySet::Picking`**, because picking consumes their results:

- **`WriteClipRects`** — computes each entity's `ClipRect` from layout-owned inputs (overflow, scroll viewport, `Containment` PAINT boundary, the box), reading only layout output (pillar 4). Picking must test against `ClipRect` (a click outside an ancestor's clip does not hit), so the clip rects must exist before `BuiySet::Picking`. [clip-and-transform.md](clip-and-transform.md) owns the algorithm and the `Changed<ScrollOffset>` fast-recompute.
- **The `Transform` compose** — layout composes `ResolvedLayout.position` + `ResolvedTransform.matrix` into a Bevy `Transform` (pillar 5). Picking and the future 3D-anchored/RTT path need `GlobalTransform`; so the compose must precede propagation, which must precede `BuiySet::Picking`.

Both are render-*prep* in that render is their ultimate consumer, but they are layout-adjacent (they read only layout output and write components layout's contract reaches for). They run at the tail of `BuiySet::Layout` / head of the post-layout window, *before* `BuiySet::Picking`. [clip-and-transform.md](clip-and-transform.md) pins their exact intra-`BuiySet` placement; this section pins the constraint: **`WriteClipRects` and the `Transform` compose are done before `Picking`, and after layout's `ResolvedLayout` / `ResolvedTransform` are final.**

### 5.3 The `TransformSystems::Propagate` wrinkle

Pillar 5 hands `GlobalTransform` ownership to Bevy's hierarchical propagation rather than re-implementing it. The wrinkle: in Bevy 0.18, `TransformPlugin` schedules propagation in **`PostUpdate`** inside the `TransformSystems::Propagate` set (verified in `bevy_transform::plugins` — the variant is exactly `TransformSystems::Propagate`, the only variant of that enum, run in `PostStartup` and `PostUpdate`). But Buiy needs `GlobalTransform` available to `BuiySet::Picking` in `Update`, *earlier* in the frame than `PostUpdate`.

The target state resolves this by **scheduling a Buiy-owned propagation run inside the `Update` post-layout window**, ordered after the `Transform` compose (§5.2) and before `BuiySet::Picking`, and leaving Bevy's standard `PostUpdate` propagation in place as the canonical late pass. Concretely, Buiy reuses Bevy's propagation systems (it does not fork them) but configures an additional ordered invocation in `Update` so `GlobalTransform` is fresh for picking and for that same frame's extract. The intra-frame double-propagation is cheap (Bevy's propagation is change-gated; the `PostUpdate` pass re-runs only entities whose `Transform` changed after the `Update` pass, which for a layout-driven UI is normally none). [clip-and-transform.md](clip-and-transform.md) owns the precise system-config wiring (which propagation systems, what ordering edges against `TransformSystems::Propagate`); this section pins the **requirement** — `GlobalTransform` is final before `BuiySet::Picking` and before extract — and flags that the naive "let `PostUpdate` own it" placement is too late for picking. The alternative of having render read `ResolvedTransform` directly was rejected by pillar 5 (re-implements propagation, diverges from picking/3D expectations); this is the mechanism that honors that rejection.

## 6. Crate placement of the render-side components

The render-side components ([README § 3.2](README.md#32-render-owned-this-spec-introduces)) land in `buiy_core` for v1, alongside layout — which is also in `buiy_core` today. The placement is constrained by [README § 3.2's crate-placement note](README.md#32-render-owned-this-spec-introduces) (foundation §5 #28):

> The reserved effect components (`Opacity`, `Filter`, `BackdropFilter`, `MixBlendMode`) must live where **both** layout sub-pass 6f and render can read them.

Layout sub-pass 6f forms stacking contexts and must read SC trigger 5 — the `opacity < 1` / `filter` / `mix-blend-mode` clause — i.e. it reads these render-owned components. The dependency edge is therefore **layout → these components → render**. If the workspace later splits `buiy_render` out of `buiy_core` ([open question, README § 5 #5](README.md#5-open-questions); foundation §5 #1/#28), this edge must not invert: layout cannot be made to depend on a `buiy_render` crate, because layout already owns the SC-formation pass that reads them and render is the *downstream* consumer of layout's `StackingContext` output.

The target home that holds under either outcome:

- **`Opacity`, `Filter`, `BackdropFilter`, `MixBlendMode`** — the four SC-trigger-5 effect components — live in a layout-readable location. If the crate splits, they live in `buiy_core` (or a shared `buiy_components` leaf) that *both* `buiy_layout` and `buiy_render` depend on, never in `buiy_render` alone. They are render-*owned* in the sense that this spec defines their fields and render is their paint consumer, but their crate home is dictated by the layout-read constraint, not by who paints them.
- **`Background`, `Border`, `BoxShadow`, `Outline`, `EffectGroup`, `ClipRect`** — pure paint/clip components with no layout-read requirement — may move into `buiy_render` cleanly if the split happens, since nothing in layout reads them. (`ClipRect` is *written* by the `WriteClipRects` render-prep pass and *read* by both render and picking; it is produced from layout output but is not a layout *input*, so it does not create an inverting edge.)

This mirrors the layout spec's own crate-agnostic stance: layout commits to living in "either `buiy_core` or a future `buiy_layout`" and is silent on which ([layout/architecture.md § 7](../2026-05-08-buiy-layout-design/architecture.md#7-crate-placement)). The render spec takes the same posture for the paint components and the *non-inversion* posture for the four shared effect components. Plans choose the crate; this spec pins only that the layout → components → render edge stays pointing one way.

## 7. Verification

The render-graph integration and component model are provable on the headless CI runners (no wgpu adapter) that gate every commit ([foundation Build & Test](../2026-05-07-buiy-foundation/README.md); [verification.md](verification.md)):

- **Registration is unit-testable headless.** That `BuiyRenderPlugin` registers `BuiyRenderLabel` into `Core2d` with an edge from `Node2d::EndMainPass`, that `extract_buiy_nodes` is in `ExtractSchedule`, and that `prepare_buiy_instances` is in `RenderSystems::Prepare`, are asserted by building the render sub-app and inspecting the graph/schedule — no GPU required, the same way Phase 0's node registration is exercised.
- **System order is pinned by test.** `WriteClipRects` and the `Transform` compose running before `BuiySet::Picking`, and extract running after `BuiySet::Render`, are asserted by ordering tests in the realizing crate (cf. the layout chain's `tests/system_set_order.rs`). The `TransformSystems::Propagate`-in-`Update` ordering (§5.3) is asserted there too, since the naive `PostUpdate`-only placement is the regression to catch.
- **`ClipRect` geometry rides gate #5** (layout-snapshot) — its values are derived from layout output and snapshot-comparable without paint. Hit-target geometry rides **gate #10**.
- **Pixel correctness rides gate #2** (visual-regression, golden image on a canonical CI GPU) — the only properties that *require* a GPU. Render-time rides **gate #14**; the persistent-atlas / RT-pool memory rides **gate #15** (RSS slope < 1 MB/min, atlas entries return to baseline). [verification.md](verification.md) owns the full gate mapping and the headless/GPU split; this section pins which property each layer proves so the headless majority is not blocked on GPU CI.

---

*Elaborates README pillars 1, 2, 3, 5 and the § 3.2 crate constraint. Open items deferred to README § 5 are flagged inline (#1 per-window top-layer routing / D2; #3 `::backdrop`; #5 crate split). Target state only; the Phase-0 → target migration lives in `docs/plans/`.*
