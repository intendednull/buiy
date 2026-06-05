# Buiy render — architecture

**Parent:** [README.md](README.md)

This file defines the structural skeleton the rest of the render spec relies on: how Buiy's paint passes attach to Bevy 0.18's render graph, where the main-world → render-world boundary sits, the typed-primitive node group and the single top-layer composite pass it contains, per-window node-group ownership, the persistent-buffer/per-frame-instance handoff that retires the Phase-0 hacks, the system-set order including the render-prep stage, and the crate home of the render-side components. Every sibling file ([component-model.md](component-model.md), [clip-and-transform.md](clip-and-transform.md), [paint-order-and-top-layer.md](paint-order-and-top-layer.md), [effect-compositor.md](effect-compositor.md), [atlas-and-text-seam.md](atlas-and-text-seam.md), [color-and-forced-colors.md](color-and-forced-colors.md)) assumes the seams pinned here.

Tier legend: **F** = foundation, **C** = core (reserved seam, deferred shader/body), **E** = extended (named only). See [foundation/README.md](../2026-05-07-buiy-foundation/README.md#tier-legend).

The spine is [README.md § 2](README.md#2-architectural-pillars-one-line-summaries) (pillars) and [§ 3](README.md#3-the-component-contract) (the component contract). This file elaborates the *mechanism* of pillars 1, 2, 3, and 5; it does not relitigate them. The component contract's names are authoritative and used verbatim.

## 1. Render-graph integration

### 1.1 Plugin shape: `BuiyPlugin::finish` adds `BuiyRenderPlugin`, whose `build` sees a live `RenderApp`

Buiy's render code lives behind one plugin, `BuiyRenderPlugin`. The load-bearing seam is that **`BuiyPlugin::finish` ADDS `BuiyRenderPlugin`** — **not** `BuiyPlugin::build`. Because `add_plugins` runs the new plugin's `build` synchronously, and `finish` hooks run in registration order *after* `RenderPlugin::finish` has created the `RenderApp`, by the time `BuiyPlugin::finish` runs `app.add_plugins(BuiyRenderPlugin)`, `BuiyRenderPlugin::build` sees a live `RenderApp`. This is already correct in Phase 0 ([`crates/buiy/src/lib.rs`](../../../crates/buiy/src/lib.rs) `BuiyPlugin::finish`): the `RenderApp` sub-app and the `PipelineCache` resource it reaches for are inserted by `RenderPlugin::finish`, and Bevy runs plugin `finish` hooks in registration order. Adding `BuiyRenderPlugin` from `BuiyPlugin::build` instead would run `BuiyRenderPlugin::build` against a `RenderApp` that does not yet exist.

The block below is **`BuiyRenderPlugin::build`** (the *inner* plugin's `build`) — it is correct precisely because the outer `BuiyPlugin` deferred the `add_plugins(BuiyRenderPlugin)` call to its own `finish`. The "register in `finish`, not `build`" rule governs the *outer* plugin's choice of where to add the inner one; it does **not** mean `BuiyRenderPlugin` must register its render-graph work in a `finish` hook — by the time the inner plugin's `build` runs, the `RenderApp` is already live.

```rust
pub struct BuiyRenderPlugin;

impl Plugin for BuiyRenderPlugin {
    fn build(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            // The extract -> prepare split (§3): `extract_buiy_nodes` (ExtractSchedule) builds
            // the CPU-side per-view ExtractedNodes (§3.1); `prepare_buiy_instances`
            // (RenderSystems::Prepare) writes the GPU BuiyInstanceBuffers (§3.2) and effect-group
            // targets — it MUST be a prepare-phase system because ViewTarget does not exist until
            // prepare_view_targets (RenderSystems::ManageViews, AFTER ExtractSchedule). Both store
            // their output as COMPONENTS on the per-view render entity (§4, R8), not global
            // resources, so no init_resource here.
            .add_systems(ExtractSchedule, extract_buiy_nodes)
            .add_systems(Render, prepare_buiy_instances.in_set(RenderSystems::Prepare));
        atlas::register(render_app);                  // atlas-and-text-seam.md
        compositor::register(render_app);             // effect-compositor.md — registers the
                                                      // composite PIPELINES + pooling resources ONLY;
                                                      // it does NOT add a separate competing ViewNode.
                                                      // Its passes run INSIDE BuiyNode::run's ordered
                                                      // passes in the same BuiyRenderLabel group (§2.3).
        node::register(render_app);                   // §2 — the node group
        pipeline::register(render_app);               // §1.4 — pipelines
    }
}
```

`app.get_sub_app_mut(RenderApp)` returns `Option`: if the app was built headless without a render sub-app, the early-return makes `BuiyRenderPlugin` a clean no-op. This is the same guard the Phase-0 plugin already uses and is what makes the component model, the render-prep passes (§5), and `ClipRect` geometry testable on CI runners that have no wgpu adapter (the headless constraint — see [verification.md](verification.md) and § 6 here).

> The `Plugin::finish` ordering contract — add `BuiyPlugin` *after* `DefaultPlugins` — is documented on `BuiyPlugin` itself and is a foundation-level invariant, not a render-spec decision. This spec inherits it.

### 1.2 The extract boundary: `ExtractSchedule` + `Extract<Query>`

Buiy follows Bevy's standard two-world split. The main world owns the ECS truth (layout output, render-side components); the render world owns GPU resources and the graph. Data crosses once per frame in `ExtractSchedule`, which runs after the main-world schedules complete — and, per the foundation pipeline, after `BuiySet::Render` (§5).

Extraction reads the main world through `Extract<SystemParam>`, the read-only cross-world accessor. The system signature is a fan of `Extract<Query<…>>` plus `Extract<Res<…>>`. The query below is an **illustrative subset, not exhaustive** — it omits e.g. `UiTransform.backface_visibility`, the off-screen content-visibility paint-skip marker ([paint-order-and-top-layer.md § 5.3](paint-order-and-top-layer.md)), and the reserved `Filter` / `MixBlendMode` / `BackdropFilter` effect components — each added as its tier lands. It does **not** omit `UserPreferences.forced_colors` ([color-and-forced-colors.md](color-and-forced-colors.md)): gate #11 (forced-colors) is **F-tier**, so the `Extract<Res<UserPreferences>>` accessor is in the v1 fan, not deferred. The fan binds `Option<&T>` for **every** author-set component (`Background` / `Border` / `BoxShadow` / `Opacity` / `Outline` / `CssVisibility` / the effect components — and `Stacking`, which a bare `Node` lacks until the `Style` bundle inserts it) because these components are inserted **independently** ([component-model.md § 1](component-model.md)); a non-`Option` `&T` query would silently drop every `Node` missing that component (e.g. an unstyled `Node` lacking `Stacking`):

```rust
fn extract_buiy_nodes(
    mut commands: Commands,                            // writes ExtractedNodes as a per-view component (§4, R8)
    nodes: Extract<Query<
        (
            Entity,
            &GlobalTransform,                          // pillar 5 — composed by layout, propagated by Bevy
            &ResolvedLayout,                           // box size (position folded into GlobalTransform)
            Option<&ClipRect>,                         // pillar 4 — written by WriteClipRects (clip-and-transform.md); Option
                                                       // for the same reason as Stacking/CssVisibility: WriteClipRects
                                                       // emits ClipRect only on nodes under a clipping ancestor, so a bare
                                                       // Node may lack it — absent => no ancestor clip => no scissor. Render
                                                       // branches on the Option, never requires it.
            Option<&Background>, Option<&Border>,      // component-model.md — author-set, all Option
            Option<&BoxShadow>, Option<&Outline>,
            Option<&Opacity>, Option<&EffectGroup>,    // effect-compositor.md — author-set, all Option
            Option<&CssVisibility>,                    // component-model.md § 12.1 — render-owned CSS visibility (absent => Visible)
            Option<&StackingContext>, Option<&Stacking>, // SC removed when an entity stops forming one (layout/systems.rs ~4058);
                                                       // Stacking is Option because a bare Node lacks the Style bundle that inserts it
                                                       // (components.rs `pub struct Node;` has no #[require]) — absent => Stacking::default()
                                                       // => TopLayer::None / no isolation, never silently dropped
        ),
        With<Node>,                                    // no `Without<Display::None>` — no such type exists; Display::None
                                                       // entities are absent from painters_z (paint-order § 5.1) so never reach paint
    >>,
    theme: Extract<Res<Theme>>,
    prefs: Extract<Res<UserPreferences>>,              // forced_colors — gate #11 is F-tier (color-and-forced-colors.md § 3)
    windows: Extract<Query<(Entity, &Window)>>,        // queries all windows (reserved structure, §4); v1
                                                       // still writes all Nodes to the PRIMARY view (D2)
) { /* §3.1: rebuild the per-frame instance set, Changed-gated; v1 targets the primary view (§4) */ }
```

`Display::None` is a variant of the layout `Display` enum, not a marker component; there is no `Without<Display::None>` filter to write (the layout passes filter the variant in Rust — no Bevy `Without<Display::None>` exists). Display-`None` entities never form a stacking context and never enter `StackingContext.painters_z`: the stacking pass's `display_none` guard in `stacking_context` / `painters_of` excludes them ([`crates/buiy_core/src/layout/systems.rs`](../../../crates/buiy_core/src/layout/systems.rs) `painters_of`, the `if display_none(node)` skip ~3970, not the sticky pass at :594), so the forward paint walk (paint-order § 5.1) never reaches them; the extract fan needs no None-skip clause. The exclusion is **doubly** load-bearing, and the second guard sits at the extract query itself: a `Display::None` entity has **no Taffy node**, hence **no `ResolvedLayout`**, and the fan binds `&ResolvedLayout` as a **required** (non-`Option`) term — so a `Display::None` entity fails the query and is dropped at extract regardless of whether it ever reached `painters_z`. The no-skip-clause argument therefore rests on the required `&ResolvedLayout` binding, not only on the downstream paint walk. Render reads content-visibility, when it needs it, via `Containment.content_visibility` ([`crates/buiy_core/src/layout/components.rs`](../../../crates/buiy_core/src/layout/components.rs)), and `StackingContext` is queried as `Option<&_>` because layout removes it from an entity that stops forming a context — paint walks the SC-root `painters_z`, so a member without its own `StackingContext` is still painted.

This replaced the temporary `Visual` that Phase 0's `extract_buiy_draws` originally read ([`crates/buiy_core/src/render/mod.rs`](../../../crates/buiy_core/src/render/mod.rs)). The `Visual` → `Background` + `Border` migration was a plan concern ([README § 3.3](README.md#33-the-visual-migration)); `extract_buiy_draws` now reads the real component model (`&Background, Option<&Border>`). Theme-token resolution against `Res<Theme>` stays in extract (it is cheap and main-world-authoritative); [color-and-forced-colors.md](color-and-forced-colors.md) owns the resolution rules. The forced-colors mechanism is concrete: a **main-world** system watches `UserPreferences.forced_colors` and, when it flips, swaps `Res<Theme>` to the forced/system-palette variant (the `Theme` carrying the forced palette) *before* extract runs, so extract resolves tokens against the already-forced `Theme` with no render-world special-casing — `color-and-forced-colors.md § 3` owns the palette-selection rules. Extracting `UserPreferences` alongside `Theme` lets render gate forced-colors-only behavior (e.g. system-color overrides for `Background` / `Border`) on the flag directly when token swap alone is insufficient.

**Extraction is one-directional and read-only.** It never mutates the main world. This is the render half of pillar 1 (immutable layout output, thin render consumer): the extract query reads layout-owned and render-owned components and writes only render-world state. It never re-sorts `StackingContext.painters_z`, never re-derives stacking, never recomputes geometry. Where render needs a derived value, a render-prep pass in the *main* world computes it first (§5, pillar 4).

### 1.3 Node placement: `ViewNode` + `ViewNodeRunner<BuiyNode>` in `Core2d`, after post-processing, before `Tonemapping`

The paint group is a `ViewNode` — a render-graph node parameterised by a per-view query, run once per view that matches. It is inserted into the `Core2d` sub-graph via the wrapper `ViewNodeRunner<BuiyNode>` (the bridge that turns a `ViewNode` into the graph's `Node` trait), with explicit edges pinning it after the 2D scene's post-processing window and before `Node2d::Tonemapping`:

```rust
#[derive(RenderLabel, Hash, PartialEq, Eq, Debug, Clone)]
pub struct BuiyRenderLabel;

pub(crate) fn register(render_app: &mut SubApp) {
    render_app
        .add_render_graph_node::<ViewNodeRunner<BuiyNode>>(Core2d, BuiyRenderLabel)
        // Pin Buiy inside the post-processing window but before tonemapping:
        // UI is NOT bloomed (crisp text/edges), and it PARTICIPATES in whatever
        // tonemapping/color-management the view enables (None by default for Camera2d).
        .add_render_graph_edges(
            Core2d,
            (
                Node2d::StartMainPassPostProcessing,
                BuiyRenderLabel,
                Node2d::Tonemapping,
            ),
        );
    // If a 2D bloom node is present, pin Buiy after it so UI is not bloomed.
    // This edge MUST NOT be added statically here: add_render_graph_edge ->
    // add_node_edge -> try_add_node_edge(..).unwrap() PANICS on an absent node
    // (verified bevy_render::render_graph::graph add_node_edge, ~line 405), and
    // Node2d::Bloom has no node in the base Core2d graph. So this edge is added
    // by code that runs AFTER the bloom plugin / gated on detecting the Bloom
    // node's presence, not in BuiyRenderPlugin::build:
    //   if render_graph_has_node(Core2d, Node2d::Bloom) {
    //       render_app.add_render_graph_edge(Core2d, Node2d::Bloom, BuiyRenderLabel);
    //   }
}
```

`add_render_graph_node` / `add_render_graph_edge` / `add_render_graph_edges` are the `RenderGraphExt` trait methods on `SubApp` (verified in `bevy_render::render_graph::app`). `BuiyRenderLabel` derives `RenderLabel` — this is the stable identity other nodes order against and the name pillar 2 reserves for the whole per-window node group (§4).

**Why before tonemapping, after the post-processing window.** The `Node2d` *enum* (`bevy_core_pipeline::core_2d::Node2d`, 0.18) declares `MsaaWriteback, StartMainPass, MainOpaquePass, MainTransparentPass, EndMainPass, Wireframe, StartMainPassPostProcessing, Bloom, PostProcessing, Tonemapping, Fxaa, Smaa, Upscaling, ContrastAdaptiveSharpening, EndMainPassPostProcessing`. **Enum declaration order is not graph order** — what orders the graph is the edge chain `Core2dPlugin` actually wires (verified in `core_2d/mod.rs`):

```
StartMainPass → MainOpaquePass → MainTransparentPass → EndMainPass
  → StartMainPassPostProcessing → Tonemapping → EndMainPassPostProcessing → Upscaling
```

The other variants — `MsaaWriteback`, `Wireframe`, `Bloom`, `PostProcessing`, `Fxaa`, `Smaa`, `ContrastAdaptiveSharpening` — are **reserved labels with no edges in the base Core2d graph** (verified: nothing in the installed `bevy_*-0.18` crates wires `Node2d::Bloom` / `Node2d::PostProcessing` for 2D); they are optional-plugin nodes that appear only when their plugin is added, attaching between `StartMainPassPostProcessing` and `Tonemapping`. **The load-bearing edge is therefore `BuiyRenderLabel → Node2d::Tonemapping`** — this is what guarantees Buiy paints into the shared view attachment *before* the `Tonemapping` node, so the UI is **positioned to participate in** whatever tonemapping / color-management the view enables. The "participates in tonemapping" framing has teeth only on the **opt-in HDR / non-`None`-tonemapper** path: on the **default** `Camera2d` view (`Tonemapping::None`) the `Tonemapping` node is a pass-through, so the edge is **inert** and the sRGB OETF runs at Buiy's own attachment write into the default `Rgba8UnormSrgb` view target — there is **no** default linear shared-working-target the UI blends into. The edge buys the *seam*, not a default behavior. The after-edge attaches from `Node2d::StartMainPassPostProcessing` (the start of the post-processing window, a node that always exists in the base graph), placing Buiy after the main 2D pass and inside the post-processing window. The decision: **Buiy paints inside the post-processing window, before tonemapping**, so UI rides the view's tonemapping/color-management seam but is **not bloomed** (crisp text/edges). "Not bloomed" is enforced by the **conditional** extra edge `Node2d::Bloom → BuiyRenderLabel` shown in the `register` block above. This edge is added by code that runs **after** the bloom plugin (or is gated on detecting the `Bloom` node's presence in the graph) — **not** statically in `BuiyRenderPlugin::build`: `add_render_graph_edge` → `add_node_edge` → `try_add_node_edge(..).unwrap()` **panics** on an absent node (verified `bevy_render::render_graph::graph::add_node_edge`), and `Node2d::Bloom` has no node in the base Core2d graph, so a static add would crash an un-bloomed app at startup. The instant a 2D bloom plugin is present its bloom node lands in this window and the (now safely addable) edge forces it to run first — bloom processes the scene, then Buiy paints on top, un-bloomed.

The consequence the foundation cares about: Buiy widgets write into the same view color attachment as 2D scene content and therefore **participate in** whatever tonemapping / color-management path the view enables (but skip bloom), rather than each managing its own color-space matching. The placement's value is the **seam** for opt-in HDR / wide-gamut — *not* a claim that v1 tonemaps the UI. By default it does not: `Camera2d` sets `Tonemapping::None` (verified, `core_2d/mod.rs:88` `register_required_components_with::<Camera2d, Tonemapping>(|| Tonemapping::None)`), so the `Tonemapping` node is a pass-through and the **default** 2D view is SDR pass-through — the UI is *not* tonemapped unless the app opts into a non-`None` tonemapper / HDR. Because the default view is SDR pass-through, the **gate-#9 authored-contrast guarantee holds unconditionally for the default view**: authored theme-token colors reach the surface unshifted. [color-and-forced-colors.md § 1.2](color-and-forced-colors.md) owns the linear-light-render / sRGB-store contract that this placement enables and cites these pinned edges; this section owns only the *graph edges* that make it true. **Open question (README § 5), narrowed:** only on the **opt-in-HDR / non-`None`-tonemapper** path does tonemapping shift authored token colors — whether such a view uses a UI-safe curve to keep the gate-#9 contrast guarantee end-to-end is owned by `buiy-verification-design` / color-management; the default-view guarantee needs no such gate.

This is the gpui / makepad convergence point ([prior-art/gpui/gpu-rendering.md](../../prior-art/gpui/gpu-rendering.md), [prior-art/makepad/gpu-rendering.md](../../prior-art/makepad/gpu-rendering.md)): a custom-shader 2D-UI pass attached to the host's render graph, after scene content, before final color resolve. It is also the deliberate departure from bevy_ui, whose render-graph node Buiy reuses *none* of — bevy_ui's renderer caps non-rect clipping, backdrop-filter, mix-blend-mode, isolation, and true top-layer compositing ([prior-art/bevy-ui/architecture.md](../../prior-art/bevy-ui/architecture.md) "Renderer caps"), which is exactly why Buiy owns the node.

### 1.4 Pipelines: `PipelineCache` + stable-UUID shaders

Each typed primitive (§2) owns a render pipeline, queued through the render world's `PipelineCache` at registration:

```rust
let id: CachedRenderPipelineId = pipeline_cache.queue_render_pipeline(descriptor);
```

`queue_render_pipeline` returns a `CachedRenderPipelineId` immediately; the cache compiles the pipeline asynchronously and `get_render_pipeline(id)` yields `None` until it is ready. The node's `run` early-returns `Ok(())` on `None` (the Phase-0 node already does this) — a not-yet-compiled pipeline produces a dropped frame, never a panic. `RenderDevice` (the wgpu device handle) backs buffer and bind-group creation.

**One pipeline per target format (a wgpu invariant, not a choice).** A wgpu `RenderPipeline`'s fragment `ColorTargetState.format` is fixed at pipeline creation and must equal the bound attachment's format, so a single pipeline cannot target both the view's `Rgba8UnormSrgb` (the `Camera2d` default) and the `Rgba16Float` effect-group targets ([effect-compositor.md § 2.2](effect-compositor.md)). Each typed-primitive pipeline (quad / shadow / glyph / path) is therefore a **`SpecializedRenderPipeline`** (`bevy_render::render_resource::SpecializedRenderPipeline`) keyed at least on the target `ColorTargetState` format, and Buiy builds each primitive for **both** formats it targets: the **view format** for the main pass into the shared attachment (§1.3), and `Rgba16Float` for the off-screen effect-group targets.

**`main_texture_format()` is the format/edge seam contract — owned here.** The view format Buiy specializes the main-pass pipeline against is `ViewTarget::main_texture_format()` (verified `bevy_render::view::ViewTarget::main_texture_format`, returning the `TextureFormat` of the view's main color attachment): it reports `TextureFormat::bevy_default()` (`Rgba8UnormSrgb`) for a non-HDR view and `ViewTarget::TEXTURE_FORMAT_HDR` (`Rgba16Float`) for an HDR view (`view.hdr`). The main-pass primitive pipeline's `ColorTargetState.format` is keyed off this value, and the effect-group pipeline variant is keyed off the fixed `Rgba16Float` group-target format. This is the single declared owner of the format seam; [color-and-forced-colors.md § 1.1](color-and-forced-colors.md) **references** `main_texture_format()` rather than re-deriving the view format.

This forks the **blend space**, and the seam is an accepted v1 approximation, bounded here: a child `quad` painting direct-to-window blends in the attachment's encoded (sRGB / SDR) space, while the *same* `quad` inside an effect group blends in linear (the `Rgba16Float` target). So wrapping content in `opacity: ~1` (which forms an effect group) subtly changes how that content blends. v1 accepts this; the only fully-correct alternative — paint all content into linear intermediates and resolve once at the end — is noted as the revisit path, not adopted now (it over-builds before a measured need).

Shaders are loaded into the render world's `Assets<Shader>` under stable weak `Handle::Uuid` handles, following the **Buiy render-asset UUID convention** already pinned in [`crates/buiy_core/src/render/pipeline.rs`](../../../crates/buiy_core/src/render/pipeline.rs): the prefix `0xB01A_01XX_..` ("BUIY 01"), trailing octet per asset. The octet assignments below are **normative** — these are the reserved trailing-octet values for the v1 primitive and composite pipelines (this is the only place the octets are enumerated; plans realize but do not renumber them), within the reserved range `0xB01A_0100_..` through `0xB01A_01FF_..`:

| Octet | Pipeline / shader | Tier |
|---|---|---|
| `..01` | quad / rounded-rect (Phase 0 owns this) | F |
| `..02` | shadow (closed-form SDF Gaussian) | F |
| `..03` | glyph-alpha (the [atlas-and-text-seam.md](atlas-and-text-seam.md) seam) | F |
| `..04` | path-SDF | C (reserved) |
| `..05` | top-layer / effect composite pipeline | F |

Each octet is documented in the `pipeline.rs` comment block as the shader lands. The **convention, the reserved range, and these specific assignments** are pinned here.

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

The `BoxShadow` *list-index* order (the order of multiple shadow entries within one element's shadow list, [component-model.md § 5](component-model.md)) and the shadow-under-quad *paint* order (every shadow draws before its caster's quad) are orthogonal: the former decides how an element's own stacked shadows layer among themselves, the latter places that whole shadow group beneath the caster.

### 2.3 One ordered top-layer composite pass

Inside the same `BuiyNode`, after the in-flow layers, runs exactly **one** ordered composite pass for the top layer — the elements (modal `<dialog>`, popovers, tooltips) that paint above all in-flow stacking contexts and are not clipped by ancestor overflow. "Exactly one composite pass" scopes the **top-layer relocation** pass specifically; the effect compositor adds its *own* per-`EffectGroup` composite draws (one per group RT, [effect-compositor.md § 3](effect-compositor.md)) — those are a separate mechanism counted separately, not a violation of the single-top-layer-pass commitment. This is the CSS top-layer the foundation commits to and bevy_ui lacks. The pass consumes `Stacking.top_layer` membership and the layout-provided top-layer order; it composites top-layer content (and the `::backdrop` model — an [open question, README § 5 #3](README.md#5-open-questions), proposed render-synthesized in [paint-order-and-top-layer.md](paint-order-and-top-layer.md)) over the in-flow result. [paint-order-and-top-layer.md](paint-order-and-top-layer.md) owns the ordering identity and the `::backdrop` decision; this section pins only that there is **one** composite pass and it lives inside the per-window node group, not a separate graph node.

The off-screen **effect-group** compositing (a render target per `EffectGroup` for group `opacity` / `isolation`) is a distinct mechanism that also executes **inside this same node group** — its passes run as ordered passes within `BuiyNode::run` (or a sibling pass in the same `BuiyRenderLabel` group), **not** as a separate competing `ViewNode`. `compositor::register` (§1.1) registers only the composite pipelines and pooling resources; it adds no graph node of its own ([effect-compositor.md § 3](effect-compositor.md) owns the in-node pass ordering and agrees on this). The two are kept separate as *mechanisms*, both behind the one `BuiyRenderLabel` node: top-layer is a *paint-order* relocation (same surface, different draw position); effect-group is an *intermediate-target* operation (off-screen RT, then composite). Pillar 2's rejection of a single grow-in-place node is precisely because it cannot express the intermediate targets the effect compositor needs; the rejection of a full slimming-paint sub-graph is that it over-builds before a concrete C-tier need. The node-group-with-one-composite-pass is the middle the brainstorming settled on.

**The single-`ViewNode` choice vs. the small-fixed-subgraph alternative — a live tradeoff, not foreclosed.** Folding the effect-group passes inside one `BuiyNode::run` means Buiy hand-rolls the inter-pass texture-state transitions (the render-pass begin/end and any layout transitions between writing a group target and sampling it) and manages target-reuse hazards manually inside `run()`, rather than letting the render graph schedule discrete nodes with declared edges and let Bevy's graph executor handle the transitions. The alternative — a small fixed sub-graph of a few named nodes (one per pass kind) — would push that bookkeeping onto the graph at the cost of more graph surface and inter-node target plumbing. v1 takes the single-node path because the pass count is small and fixed and the in-`run` ordering is explicit; the sub-graph remains the documented revisit if the manual transition/hazard management inside `run()` becomes the bug surface. This is a tradeoff worth re-opening on evidence, not a closed door.

**Target residency is pinned (so a parent never samples a recycled child target).** All effect-group render targets for the frame are acquired **up-front** — in the per-`EffectGroup` prepare pass (§5.2) / at the start of `run` — and held for the whole composite, then released after the run. A child group's target, acquired before its parent samples it, is therefore **not** recycled mid-run: the texture-cache `get` that hands out a pooled target marks it taken for the frame, and `update_texture_cache_system` (render `Cleanup`, present under `DefaultPlugins`) returns only **untaken** textures to the pool — so a target held by Buiy for the duration of the composite is never reclaimed underneath a later parent sample ([effect-compositor.md § 2.2 / § 3](effect-compositor.md) owns the pooling mechanism; this section pins the residency invariant).

## 3. The hybrid handoff (pillar 3)

Pillar 3 retires two Phase-0 stopgaps in one move: the per-frame `create_buffer_with_data` allocation in the node (`node.rs`) and the per-instance CPU y-flip / radius-px→clip approximation in [`crates/buiy_core/src/render/instance.rs`](../../../crates/buiy_core/src/render/instance.rs). The replacement is **persistent GPU buffers + atlas + a view uniform**, with the per-frame instance *set* rebuilt from a `Changed<T>`-gated extract. Damage tracking is ECS change-detection — the thing Buiy gets for free — and there are **no screen-space damage-rects in v1**.

### 3.1 Per-frame instance set, `Changed<T>`-gated

The per-view `ExtractedNodes` component (replacing the global `ExtractedDraws` resource) is rebuilt each frame from the extract query (§1.2), stored on the view's render entity (§4, R8). The query is gated so that the common steady-state frame — nothing changed — does near-zero work, mirroring the layout pipeline's `O(0)` steady-state contract ([layout/architecture.md § 9](../2026-05-08-buiy-layout-design/architecture.md#9-performance-contract)). The change-detection trigger set for a re-extract is the union of the paint-input components:

```rust
Or<(
    Changed<GlobalTransform>, Changed<ResolvedLayout>, Changed<ClipRect>,
    Changed<Background>, Changed<Border>, Changed<BoxShadow>, Changed<Outline>,
    Changed<Opacity>, Changed<EffectGroup>, Changed<StackingContext>, Changed<Stacking>,
    Changed<CssVisibility>, Changed<OffscreenAuto>, Changed<Containment>,
)>
```

The last three are paint-*skip* triggers, not paint-*value* triggers, but they belong in the same Or-set: a `visibility: hidden` flip (`CssVisibility`), an off-screen `content-visibility: auto` add/remove (`OffscreenAuto` — structural, exactly like `Stacking` / `StackingContext`), and a `content_visibility` toggle (`Containment`) each flip whether an entity paints at all. Without them a paint-skip flip never re-extracts and the prior frame's paint goes stale — change detection is the *only* damage signal ([component-model.md § 1](component-model.md)), so a skip transition that does not touch a tracked component is invisible to extract.

This is the v1 damage mechanism. A frame where no paint input changed re-extracts nothing and re-uploads nothing; the persistent buffers from the prior frame are re-bound and re-drawn. The runner-up *full immediate rebuild* was rejected as wasting the gate-#14 render-time budget; the runner-up *full retained scene* was rejected as premature invalidation complexity (the classic stale-paint bug). ECS change-detection sits between them: retained buffers, change-gated rebuild, no hand-rolled invalidation graph.

> **Bevy extract caveat (faithful to the engine).** `Extract<Query<…>>` reads the *main* world's change ticks. Because the render world's extract runs every frame, `Changed<T>` filters in extract see main-world mutation since the prior extract — which is the semantics we want. The Phase-0 extract already rebuilds an `ExtractedDraws` each frame; the target keeps the per-frame rebuild but the change-gating means an unchanged entity contributes its *cached* instance record rather than being re-resolved. The cached records are keyed by `Entity` inside each view's `ExtractedNodes` component (§4) so a partial re-extract patches only changed entities.

### 3.2 Persistent buffers + view uniform

`BuiyInstanceBuffers` holds one growable GPU buffer per primitive type, allocated once and reused frame-to-frame (grow-in-place on capacity overflow, never reallocated per frame). It is a **GPU product of the prepare phase, not of extract** — `prepare_buiy_instances` (`RenderSystems::Prepare`) builds and writes it, because the GPU view (and its `ViewTarget`) do not exist until `prepare_view_targets` runs in `RenderSystems::ManageViews`, *after* `ExtractSchedule`. It is **not** a global render-world resource: it is stored as a **component on the view's render entity** (the render-world view entity that also carries `ExtractedView` and, by the prepare phase, its `ViewTarget`; `RetainedViewEntity` is the stable Hash/Eq **key** into Bevy's main↔render view mapping that *identifies* this entity across frames — it is not itself an entity you attach components to), so each window's buffers grow independently and one window's instance count never sizes another's buffer (R8, §4). This per-view storage is a deliberate Buiy decision for per-window isolation (§4), not an idiom inherited from another crate. `prepare_buiy_instances` writes each view's changed instance slices (and the effect-group targets, §5.2) into that view's buffers via the queue — the GPU half of the extract(CPU per-view) → prepare(GPU buffers + targets, `ViewTarget` available) split (§3.1) — and `BuiyNode::run` reads the buffers off the same view entity its `ViewQuery` already resolved. This is the upgrade the Phase-0 node comments already name ("v0.x upgrades to persistent buffers + bind groups"), scoped per-view rather than as a single global growable buffer.

The per-instance CPU coordinate work moves to the GPU via a **view uniform**. Today `instance.rs` packs `rect_pos`/`rect_size` in clip space, baking the y-flip into a negative height and approximating corner-radius px→clip with `2.0 / min(window.x, window.y)` — both are explicitly Phase-0 stopgaps in the source. The target uploads a Buiy view uniform (logical-pixel → clip-space transform for the view; conceptually the same role as `bevy_render`'s `ViewUniform`, which exists at `bevy_render::view::ViewUniform`) and `InstanceData` shrinks back to **logical-pixel** units. The vertex shader applies the view transform; the radius stays in logical pixels and the SDF fragment evaluates in pixel space, removing the non-square-window approximation entirely. The atlas (a persistent `TextureAtlas`-backed GPU texture, [atlas-and-text-seam.md](atlas-and-text-seam.md)) is the third persistent resource, bound per atlas-backed batch.

### 3.3 What stays out of v1

No property trees ([pillar 7](README.md#2-architectural-pillars-one-line-summaries)): render paints the current frame's resolved values and relies on change-detection. The named revisit trigger is *animating opacity (or transform) without re-running layout* — until that lands, `ResolvedTransform` and `painters_z` are the only "trees," and they are layout's, read forward. No screen-space damage-rects: ECS change-detection is the only damage signal. [effect-compositor.md](effect-compositor.md) holds the gate-#15 memory budget for the persistent atlas + RT pool so "persistent" does not become "unbounded."

## 4. Per-window node-group ownership (cross-cutting § 3.18 F-tier)

Coexistence is **per-window** ([foundation/cross-cutting.md § 3.18](../2026-05-07-buiy-foundation/cross-cutting.md#318-compatibility-and-coexistence)): each window is owned by exactly one stack, and per-window state — render-graph node group, AccessKit adapter, picking backend filter, focus root — is keyed by winit `WindowId`. The render-graph node ordering on a Buiy window is owned unilaterally by Buiy's per-window node group.

Bevy gives this to us at the right granularity for free: a `ViewNode` runs **once per view**, and each window's camera is a distinct view. `BuiyNode::run` therefore already executes per-window. The structural commitment this spec makes is that all per-window render state — the instance buffers (§3.2), the prepared effect-group resources ([effect-compositor.md](effect-compositor.md), §5.2), the `ExtractedNodes`, the top-layer composite target, the view uniform — is stored as **components on the per-view render entity** (the render-world view entity that carries `ExtractedView`, and — once the prepare phase runs — `ViewTarget`; `RetainedViewEntity` is the Hash/Eq **key** into Bevy's main↔render view mapping that names this entity stably across frames, not an attach target itself) — *not* as global render-world `Resource`s that assume one Buiy window. The CPU-side `ExtractedNodes` is written in `ExtractSchedule`; the GPU-side buffers, effect-group targets, and the view uniform are written in the prepare phase (`RenderSystems::Prepare`), since `ViewTarget` does not exist until `prepare_view_targets` (`RenderSystems::ManageViews`, after `ExtractSchedule`). The atlas texture and its bind group are deliberately **not** in this per-view list: glyph/icon coverage is window-independent, so the atlas resource itself is legitimately global ([atlas-and-text-seam.md](atlas-and-text-seam.md)); a per-view *reference* to that shared atlas bind may sit on the view entity for draw-time convenience, but the atlas resource it points at is global, not per-window. This per-view-component storage is a **deliberate Buiy decision** — its rationale is per-window isolation (one window's instance count / target size never sizes another's) — not an idiom inherited from another crate. Extraction writes the CPU-side `ExtractedNodes` onto the render-world view entity (reached through Bevy's main↔render entity mapping, not by attaching to `RetainedViewEntity` and not "keyed alongside `ViewTarget`" — `ViewTarget` does not exist yet in `ExtractSchedule`). **For v1, extraction writes all `Node`s to the PRIMARY window's view entity** — matching the layout D2 simplification (the single global `TopLayerActivation` reading the primary window). The consequence at the surface: **v1 paints Buiy UI only into the primary window.** A second window's camera still produces a distinct view and `BuiyNode::run` still executes against it (the `ViewNode` runs once per view), but that view's `ExtractedNodes` is empty — it receives **no** extracted instances until the per-window partition below is wired — so the second window paints nothing Buiy. The per-view storage shape, the all-windows extract query (§1.2), and the per-window keying are **reserved structure** so a true per-window partition (partitioning the main-world `Node` instances by their target view/window onto each window's view entity) can be wired without re-architecting the node group — not a working multi-view partition shipping in v1. The extract query (§1.2) is written to read *all* windows (`Extract<Query<(Entity, &Window)>>`), not only `PrimaryWindow` as Phase 0 does, so the partition can be turned on without a query change — but v1 still resolves every `Node` to the primary view.

`BuiyNode::run` maps a view to its window/target through its `ViewQuery` (`(Entity, &ViewTarget, &ExtractedView)`): the matched `Entity` *is* the per-view render entity, so `run` reads that view's `BuiyInstanceBuffers` / `ExtractedNodes` / prepared effect-groups directly off it, and `&ExtractedView` (with the camera's target) identifies which window/surface this view paints into. No global lookup keyed by `WindowId` is needed at draw time — the graph already handed `run` the right view entity. `BuiyRenderLabel` names the whole per-window group; the typed-primitive batches and the one top-layer composite pass are members of it.

> **Reserved, not fully wired in v1 (D2).** Layout Phase 9 ships a single global `TopLayerActivation` resource that reads the primary window (the Phase-9 D2 simplification; `TopLayerActivation` is exported from `buiy_core::layout`). True per-window top-layer *routing* is owned by `buiy-window-and-surface-design` and is an [open question, README § 5 #1](README.md#5-open-questions). This spec **reserves** the per-window node-group structure — the keying, the per-view `ViewNode`, the all-windows extract — so the global-activation simplification is a tracked dependency that can be lifted without re-architecting the node group, not a baked-in single-window assumption. Do not design new render state that assumes one Buiy window.

## 5. System-set order and the render-prep stage

### 5.1 The foundation order, unchanged

Buiy's per-frame main-world work runs in `BuiySet`, chained in `Update` ([`crates/buiy_core/src/lib.rs`](../../../crates/buiy_core/src/lib.rs) `CorePlugin`):

```
BuiySet::Layout → Style → Input → Animate → Picking → A11yUpdate → Render
```

Extraction runs in `ExtractSchedule`, which executes after the main-world schedules — i.e. after `BuiySet::Render` — per [foundation/architecture.md § 2.8](../2026-05-07-buiy-foundation/architecture.md#28-module-organization). This spec adds no new top-level `BuiySet` variant; the render-prep work slots inside the existing chain.

### 5.2 The render-prep stage: between `Animate` and `Picking`

The render-prep passes run **after the last set that can mutate a paint/transform input but before `BuiySet::Picking`**, because picking (and extract) consume their results. They are pinned `.after(BuiySet::Animate).before(BuiySet::Picking)` — not `.after(BuiySet::Layout)` — because `Style`, `Input`, and `Animate` all run between `Layout` and `Picking` and any of them can still mutate a paint or transform input (an `Animate` system mutating `Opacity` / `UiTransform` / `Transform` is the load-bearing case). v1 may ship no `Animate` systems, but the committed edge must be `.after(Animate)` so the ordering stays correct the instant an `Opacity` / `UiTransform` / `Transform` animator lands. The full intra-frame `Update` ordering, against the real `BuiySet` chain ([`crates/buiy_core/src/lib.rs`](../../../crates/buiy_core/src/lib.rs)), is:

```
BuiySet::Layout  →  Style  →  Input  →  Animate
   →  Transform-compose (bridge)  →  { WriteClipRects, WriteEffectGroups }
   →  propagation (§5.3: mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms)
   →  BuiySet::Picking  →  BuiySet::A11yUpdate  →  BuiySet::Render  →  ExtractSchedule
```

- **The `Transform` compose** — `write_buiy_transform` is the **single authority** that writes each entity's Bevy `Transform.translation`; no other pass writes it (clip-and-transform.md § B.2/§ B.3). It is a **top-down accumulation walk**: per entity it composes the base `ResolvedLayout.position` **minus** the accumulated ancestor `ScrollOffset` (summed down the path through scroll containers), folded through `ResolvedTransform.matrix`, into one `Transform`. The walk seeds (re-runs) a subtree whenever `ResolvedLayout` **or** `ResolvedTransform` **or** any ancestor scroll-container `ScrollOffset` changed — the `ScrollDirty` top-down set. Because there is exactly one writer, "which write wins" is moot; scroll and transform are unified into this one pass rather than a second writer or a mutually-exclusive `Changed`-filter. Picking and the future 3D-anchored/RTT path need `GlobalTransform`; so the compose must precede propagation (§5.3), which must precede `BuiySet::Picking`.
- **`WriteClipRects`** — computes each entity's `ClipRect` from layout-owned inputs (overflow, scroll viewport, `Containment` PAINT boundary, the box), reading only layout output (pillar 4). Picking must test against `ClipRect` (a click outside an ancestor's clip does not hit), so the clip rects must exist before `BuiySet::Picking`. [clip-and-transform.md](clip-and-transform.md) owns the algorithm and the `Changed<ScrollOffset>` fast-recompute.
- **`WriteEffectGroups`** — writes the `EffectGroup` component (the `EffectReason`-carrying struct, [component-model.md § 10](component-model.md); R2) onto each entity that forms a group, reading the group-triggering inputs. The canonical EffectGroup-former predicate is owned in exactly one place — [effect-compositor.md § 1](effect-compositor.md) — as: an entity forms an `EffectGroup` iff any of [`Opacity < 1`, `Stacking.isolation == Isolation::Isolate`, `Filter` non-empty, `MixBlendMode != Normal`, `BackdropFilter` non-empty]. This bullet **cites** that predicate, it does not restate a variant. Note the isolation term reads the **layout-owned** `Stacking.isolation` field (`Isolation::Isolate`, the real variant — there is no standalone `Isolation` render component, and the spelling is never the lowercase keyword `isolation: isolate`); `Filter` / `MixBlendMode` / `BackdropFilter` are the reserved render-owned components. It runs in the same prep window as `WriteClipRects` (it depends on neither, so the two are unordered relative to each other) and before `BuiySet::Picking` / extract, so the per-frame instance set (§3.1) and the effect compositor see a settled `EffectGroup`.

These are render-*prep* in that render is their ultimate consumer, but they are layout-adjacent (they read layout output plus the render-owned effect components and write components the layout / picking / render contracts reach for). They run in the post-`Animate` window, *before* `BuiySet::Picking`. [clip-and-transform.md](clip-and-transform.md) pins the exact placement of `WriteClipRects` + the compose; this section pins the constraint: **the `Transform` compose, `WriteClipRects`, and `WriteEffectGroups` are pinned `.after(BuiySet::Animate).before(BuiySet::Picking)`** — after the last set (`Layout` / `Style` / `Input` / `Animate`) that can mutate a paint/transform input, and before picking/extract consume their results.

`WriteClipRects` and `WriteEffectGroups` both operate on **logical-px layout output** (`ResolvedLayout`, `ScrollOffset`, the overflow/`Containment` inputs and the group-trigger components) and **neither reads `GlobalTransform`** — so their order *relative to the propagation chain* (§5.3) is immaterial: they may run before or after `mark_dirty_trees → propagate_parent_transforms → sync_simple_transforms` with no correctness consequence, and the only edge they actually need is `.before(BuiySet::Picking)`. (The `Transform` compose is the exception that *does* couple to propagation — it writes the `Transform`s propagation consumes — which is why §5.3 pins the compose-before-propagation edge. The render-world per-`EffectGroup` prepare pass below *does* fold boxes through `GlobalTransform`, but it runs in `RenderSystems::Prepare`, after `GlobalTransform` is final, so it too is unaffected by the main-world prep order.)

**Per-`EffectGroup` prepare pass (render-world).** `EffectGroup` membership alone does not size an off-screen target — the compositor needs each group's painted bounds, a bucketed descriptor, and a post-order index. A prepare pass pinned to **`RenderSystems::Prepare`** (not an extract pass) computes, per `EffectGroup`: the **painted bounds** — [effect-compositor.md § 2.1](effect-compositor.md) is the single source of this formula: the group **root box** UNION its descendant `painters_z` boxes UNION the **ink expansion** that escapes the box (`BoxShadow` blur+spread, `Outline` `max(0, width + offset)`, and — when the reserved filters land — a `Filter` blur's bleed), every box folded through its `GlobalTransform` and the whole union clipped by the group's `ClipRect`; the **bucketed `TextureDescriptor`** (size from those bounds × the view `scale_factor`, [effect-compositor.md § 2.1](effect-compositor.md)); and the **post-order composite index**. It stores these in a per-view render-world store — a component on the view render entity, parallel to `ExtractedNodes` / `BuiyInstanceBuffers` (§4), not a global resource. Its inputs are the extracted group members (`EffectGroup`, `Stacking` / `StackingContext.painters_z`, `GlobalTransform`, `ClipRect`, `ResolvedLayout`) plus the view/window `scale_factor`. The effect-compositor `ViewNode` **reads this prepared per-view store** — it does *not* walk main-world ancestors from `&World` ([effect-compositor.md § 1.1 / § 2 / § 3](effect-compositor.md) cite this pass as the source of group bounds / descriptor / post-order). The **`Prepare`** pinning (not extract) is forced, matching [effect-compositor.md § 1.1](effect-compositor.md): sizing the target needs the view `scale_factor` *and* the bounds must fold through the **final** `GlobalTransform` (propagation completes in the main world this same frame, and the render-world copy is settled by prepare), and acquiring the pooled target sits next to `ViewTarget` — which does not exist until `prepare_view_targets` (`RenderSystems::ManageViews`, after `ExtractSchedule`). Extract-time sizing would read a not-yet-final transform and have no `ViewTarget` to allocate against, so the prepare phase is the only correct home.

### 5.3 The `TransformSystems::Propagate` wrinkle

Pillar 5 hands `GlobalTransform` ownership to Bevy's hierarchical propagation rather than re-implementing it. The wrinkle: in Bevy 0.18, `TransformPlugin` schedules propagation in **`PostUpdate`** inside the `TransformSystems::Propagate` set (verified in `bevy_transform::plugins` — the variant is exactly `TransformSystems::Propagate`, the only variant of that enum, run in `PostStartup` and `PostUpdate`). But Buiy needs `GlobalTransform` available to `BuiySet::Picking` in `Update`, *earlier* in the frame than `PostUpdate`.

The target state resolves this by **scheduling a Buiy-owned propagation run inside the `Update` post-layout window**, ordered after `write_buiy_transform` (the `Transform` compose, §5.2) and before `BuiySet::Picking`, and leaving Bevy's standard `PostUpdate` propagation in place as the canonical late pass. Concretely, Buiy reuses Bevy's three public propagation systems (it does not fork them) — `bevy_transform::systems::{mark_dirty_trees, propagate_parent_transforms, sync_simple_transforms}` (verified in `bevy_transform::systems`, the same three `TransformPlugin` chains into `TransformSystems::Propagate`) — chained in that exact order in `Update`, after `write_buiy_transform` and before `BuiySet::Picking`, so `GlobalTransform` is final for picking and for that same frame's `ExtractSchedule`. These `Update` copies are a **distinct scheduled instance**, *not* members of `TransformSystems::Propagate` (that set lives only in `PostUpdate`); Buiy's `Update` copies and Bevy's `PostUpdate` `TransformSystems::Propagate` are two separate runs of the same systems, ordered relative to each other only by `Update` preceding `PostUpdate`. [clip-and-transform.md § B](clip-and-transform.md) owns the concrete `Update` `SystemSet` chain and ordering edges; this section pins only the ordering constraint — `GlobalTransform` is final before `BuiySet::Picking` and before extract — and flags that the naive "let `PostUpdate` own it" placement is too late for picking.

**The double-propagation cost is bounded by an engine-global heuristic, not unconditionally cheap (correction).** Bevy clears change ticks once per `World::update` (frame end), *not* between schedules within a frame (verified: `World::clear_trackers` runs at the end of `App::update`, and `Update` → `PostUpdate` are schedules within the same world update). So if the app *also* runs Bevy's default `PostUpdate` `TransformSystems::Propagate` — e.g. a 3D scene or any app on `DefaultPlugins` — Buiy entities re-propagate there too. The cost of that re-propagation is **governed by the global `StaticTransformOptimizations` resource**, not by Buiy alone: `mark_dirty_trees` recomputes that resource's `enabled` flag on *every* call from the **whole-world** moving-entity ratio (default threshold 0.30, verified `bevy_transform::systems`). Two consequences follow, both engine-global rather than Buiy-scoped:

- **The subtrees Buiy touched in `Update` are re-walked in `PostUpdate` regardless of the static-tree flag — but via two different source reasons depending on the flag's value.** `mark_dirty_trees` computes `StaticTransformOptimizations.enabled` first (whole-world moving-entity ratio, threshold 0.30 default) and **early-returns before setting any `TransformTreeChanged` mark when the flag is `false`** (verified `bevy_transform::systems::mark_dirty_trees`: the `if !static_optimizations.enabled { return; }` at ~line 132 sits **before** the `set_changed()` walk at ~line 144). So the mechanism splits on the flag:
  - **`enabled == true` (the common case, <30% moving).** Buiy's `Update` `mark_dirty_trees` runs the `set_changed()` walk and marks each touched subtree's `TransformTreeChanged` up toward its root (keyed on `Changed<Transform>` — the `Transform`s `write_buiy_transform` just composed). `Update`'s `propagate_parent_transforms` only **reads** that mark (`Ref<TransformTreeChanged>`, `is_changed()` for its early-exit, verified ~line 360/371) — it never writes it. Because change ticks do **not** clear between `Update` and `PostUpdate` (`World::clear_trackers` runs only at frame end), those marks survive `is_changed() == true` into `PostUpdate`, whose `propagate_parent_transforms` sees them still set and bypasses its early-exit **for exactly Buiy's touched subtrees** — re-walking them while `enabled` lets it skip the static subtrees Buiy did *not* touch. Re-propagation cost is bounded by Buiy's changed-entity count.
  - **`enabled == false` (>30% moving — a co-existing dynamic 3D scene).** Buiy's `Update` `mark_dirty_trees` early-returns and sets **no** marks at all. But `propagate_parent_transforms`'s early-exit is `static_optimizations.enabled && !transform_tree.is_changed()` (verified ~line 371); with `enabled == false` the `&&` short-circuits, the early-exit is skipped, and `PostUpdate` re-propagates **all roots** unconditionally — Buiy's subtrees among them. The extra `Update` propagation degrades to **O(Buiy-tree size)** (in fact O(all roots)) regardless of how little of Buiy changed.

  Either way Buiy's subtrees are re-propagated in `PostUpdate` — via surviving marks when `enabled`, via the disabled-flag full walk when not.
- Buiy running `mark_dirty_trees` an extra time in `Update` itself **mutates this shared resource** (both the `Update` and the `PostUpdate` `mark_dirty_trees` take `ResMut<StaticTransformOptimizations>`) and adds a whole-world `count()` pass over moving entities, perturbing the engine-global heuristic for the rest of the frame. The two writers are **ordered only by `Update` preceding `PostUpdate`** — there is no explicit `.before`/`.after` edge between them; the `Update` write simply happens first by virtue of its schedule, and `PostUpdate`'s `mark_dirty_trees` recomputes the flag from scratch.

So the `Update` run is the authoritative one picking/extract read, and the `PostUpdate` re-run is idempotent *as to result*, but its *cost* rides a world-global flag. The escape hatch if this becomes a measured **gate #14** problem: a **Buiy-scoped propagation that does not touch the shared `StaticTransformOptimizations` resource** (propagate Buiy's subtree without calling the global `mark_dirty_trees`), adopted only on measurement, not speculatively. A UI-only app that does *not* add the propagation to `PostUpdate` (no `TransformPlugin` `PostUpdate` pass) pays the propagation once. The alternative of having render read `ResolvedTransform` directly was rejected by pillar 5 (re-implements propagation, diverges from picking/3D expectations); this is the mechanism that honors that rejection.

## 6. Crate placement of the render-side components

The render-side components ([README § 3.2](README.md#32-render-owned-this-spec-introduces)) land in `buiy_core` for v1, alongside layout — which is also in `buiy_core` today. The placement is constrained by [README § 3.2's crate-placement note](README.md#32-render-owned-this-spec-introduces) (foundation README § 5, **Crate-split refinement**):

> The reserved effect components (`Opacity`, `Filter`, `BackdropFilter`, `MixBlendMode`) must live where **both** layout sub-pass 6f and render can read them.

Layout sub-pass 6f forms stacking contexts and must read SC trigger 5 — the `opacity < 1` / `filter` / `mix-blend-mode` clause, i.e. **three** SC-trigger components (`Opacity`, `Filter`, `MixBlendMode`). `BackdropFilter` is **not** read by 6f: it forms an `EffectGroup`, not a stacking context, so it is not an SC trigger — it only shares the crate home of the three triggers because it carries the same `EffectGroup`-reason concerns. The dependency edge is therefore **layout → these (three) trigger components → render**. If the workspace later splits `buiy_render` out of `buiy_core` ([open question, README § 5 #5](README.md#5-open-questions); foundation README § 5, **Final crate split** / **Crate-split refinement**), this edge must not invert: layout cannot be made to depend on a `buiy_render` crate, because layout already owns the SC-formation pass that reads them and render is the *downstream* consumer of layout's `StackingContext` output.

The target home that holds under either outcome:

- **`Opacity`, `Filter`, `MixBlendMode`** — the three SC-trigger components — plus **`BackdropFilter`** (which shares this crate home for `EffectGroup` reasons, not because layout reads it) live in a layout-readable location. If the crate splits, they live in `buiy_core` (or a shared `buiy_components` leaf) that *both* `buiy_layout` and `buiy_render` depend on, never in `buiy_render` alone. They are render-*owned* in the sense that this spec defines their fields and render is their paint consumer, but the crate home of the three triggers is dictated by the layout-read constraint, not by who paints them.
- **`Background`, `Border`, `BoxShadow`, `Outline`, `EffectGroup`, `ClipRect`, `CssVisibility`** — pure paint/clip components with no layout-read requirement — may move into `buiy_render` cleanly if the split happens, since nothing in layout reads them. (`CssVisibility` is render-owned § 3.2 and paint-only — it lives in this group, not the layout-readable group.) (`ClipRect` is *written* by the `WriteClipRects` render-prep pass and *read* by both render and picking; it is produced from layout output but is not a layout *input*, so it does not create an inverting edge.)

This mirrors the layout spec's own crate-agnostic stance: layout commits to living in "either `buiy_core` or a future `buiy_layout`" and is silent on which ([layout/architecture.md § 7](../2026-05-08-buiy-layout-design/architecture.md#7-crate-placement)). The render spec takes the same posture for the paint components and the *non-inversion* posture for the three SC-trigger components (plus `BackdropFilter`, which rides along for `EffectGroup` reasons). Plans choose the crate; this spec pins only that the layout → components → render edge stays pointing one way.

## 7. Verification

The render-graph integration and component model are provable on the headless CI runners (no wgpu adapter) that gate every commit ([foundation Build & Test](../2026-05-07-buiy-foundation/README.md); [verification.md](verification.md)):

- **What is device-free vs. what rides the GPU e2e path.** The headless-provable set is: the **component model**, the **`ClipRect` geometry**, the **instance / coordinate math**, and the **main-world system schedule-membership and ordering** (the `BuiySet` chain, the render-prep placement, the `Update` propagation order — all in `Update`, no render sub-app needed). Graph node/edge registration is **NOT** device-free: registering `BuiyRenderLabel` into `Core2d` with its pinned edges (`Node2d::StartMainPassPostProcessing → BuiyRenderLabel → Node2d::Tonemapping`, §1.3) requires the `RenderApp` sub-app, and `RenderPlugin::build` only creates that sub-app after `block_on(initialize_renderer)`, which `.expect()`s a live wgpu adapter (verified `bevy_render::renderer::initialize_renderer` ~line 281; `bevy_render::lib` ~line 354). On a CI runner with no adapter the sub-app is never built and the early-return in `BuiyRenderPlugin::build` (§1.1) makes it a no-op — so the graph-node + edge assertions, and the `extract_buiy_nodes ∈ ExtractSchedule` / `prepare_buiy_instances ∈ RenderSystems::Prepare` render-world membership assertions, ride the `#[ignore]` GPU e2e path, not the headless majority ([verification.md § 2.1](verification.md) is the canonical owner of this split; this section agrees with it).
- **System order is pinned by test.** The `Transform` compose, `WriteClipRects`, and `WriteEffectGroups` running before `BuiySet::Picking`, and extract running after `BuiySet::Render`, are asserted by ordering tests in the realizing crate (cf. the layout chain's `tests/system_set_order.rs`). The propagation-in-`Update` ordering (§5.3) — the three `bevy_transform::systems` chained before `BuiySet::Picking` — is asserted there too, since the naive `PostUpdate`-only placement is the regression to catch.
- **`ClipRect` geometry rides gate #5** (layout-snapshot) — its values are derived from layout output and snapshot-comparable without paint. Hit-target geometry rides **gate #10**.
- **Pixel correctness rides gate #2** (visual-regression, golden image on a canonical CI GPU) — the only properties that *require* a GPU. Render-time rides **gate #14**; the persistent-atlas / RT-pool memory rides **gate #15** (RSS slope < 1 MB/min, atlas entries return to baseline). [verification.md](verification.md) owns the full gate mapping and the headless/GPU split; this section pins which property each layer proves so the headless majority is not blocked on GPU CI.

---

*Elaborates README pillars 1, 2, 3, 5 and the § 3.2 crate constraint. Open items deferred to README § 5 are flagged inline (#1 per-window top-layer routing / D2; #3 `::backdrop`; #5 crate split). Target state only; the Phase-0 → target migration lives in `docs/plans/`.*
