//! Buiy render pipeline. Phase 0 ships the smallest end-to-end pass
//! (rounded rect with solid bg) wired into Bevy's render graph. Full
//! pipeline (top-layer compositing, clip-path, filters, blend modes,
//! atlasing) lives in `buiy-render-pipeline-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/visuals.md § 3.3 and
//! architecture.md § 2.3.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, Render, RenderApp, RenderSystems};

pub mod atlas;
pub mod blur;
pub mod bridge;
pub mod buckets;
pub mod clip;
pub mod color;
pub mod components;
pub mod composite;
pub mod compositor;
pub mod counters;
pub mod effect;
pub mod extract;
pub mod forced_colors;
pub mod forced_colors_analyzer;
pub mod golden;
pub mod icon_producer;
pub mod icon_raster;
pub mod instance;
pub mod node;
pub mod pipeline;
pub mod prepare;
pub mod primitive;
pub mod raster;
pub mod top_layer;
pub mod view_uniform;
pub mod visibility;

pub use counters::{RenderWorkCounters, record_text_work_counters};

pub use bridge::ScrollDirty;
pub use clip::write_clip_rects;
pub use color::{ColorToken, SystemColorKeyword};
pub use components::{
    AncestorClip, Angle, BackdropFilter, Background, Border, BorderSide, BoxShadow, ClipRadius,
    ClipRect, ComputedPaintSkip, Corners, CssVisibility, EffectGroup, EffectReason, Filter,
    FilterFn, Icon, LineStyle, MixBlendMode, OffscreenAuto, Opacity, Outline, Radius, Shadow,
    SkipReason,
};
pub use raster::RasterImage;
pub use visibility::{node_skip_reason, write_paint_skip};

/// One rectangle in the Phase 0 render queue. Marked `#[non_exhaustive]`
/// because the full pipeline (clip-path, filters, blend modes, etc.)
/// will add per-draw fields pre-1.0.
///
/// External callers construct via [`DrawData::new`]; the `#[non_exhaustive]`
/// attribute prevents struct-literal construction from outside the crate so
/// new fields added here remain non-breaking per SemVer.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct DrawData {
    pub position: Vec2,
    pub size: Vec2,
    pub color: Color,
    pub radius: f32,
}

impl DrawData {
    /// Construct a [`DrawData`] with all Phase 0 fields.
    ///
    /// Using a constructor rather than struct literal syntax is required by the
    /// `#[non_exhaustive]` attribute when constructing from outside the crate.
    /// Future fields are added here with default values to remain non-breaking.
    pub fn new(position: Vec2, size: Vec2, color: Color, radius: f32) -> Self {
        Self {
            position,
            size,
            color,
            radius,
        }
    }
}

/// Signed distance from `p` to a rounded rectangle centered at the origin with
/// half-extents `half_size` and corner radius `r` (negative inside, positive
/// outside). Units are logical px with a POSITIVE half-extent — the
/// view-uniform path retired the Phase-0 y-flip / abs hack.
///
/// This is the **single CPU twin of `shader.wgsl::sdf_rounded_rect`**: the SDF
/// oracle (`buiy_verify::reftest`) and the render-side unit tests all import it,
/// so the Rust ports cannot silently drift from *each other*. It does **not** by
/// itself pin the Rust↔WGSL numeric agreement — WGSL is not CPU-executable, so
/// that leg is caught by the CI lavapipe SDF cross-check; this fn only DRYs the
/// CPU side (2026-06-18 audit: a DRY nit, not new drift protection).
pub fn sdf_rounded_rect(p: Vec2, half_size: Vec2, r: f32) -> f32 {
    let q = p.abs() - half_size + Vec2::splat(r);
    q.max(Vec2::ZERO).length() + q.x.max(q.y).min(0.0) - r
}

pub struct BuiyRenderPlugin;

impl Plugin for BuiyRenderPlugin {
    fn build(&self, app: &mut App) {
        // Main-world forced-colors selection runs in BuiySet::Style, before the
        // BuiySet::Render extract reads Theme (color-and-forced-colors.md § 3.1).
        // Registered unconditionally — it has no RenderApp dependency, so it
        // must land before the RenderApp guard's early `return` below.
        app.init_resource::<crate::render::forced_colors::PrePreferenceTheme>()
            .add_systems(
                Update,
                crate::render::forced_colors::apply_forced_colors_theme
                    .in_set(crate::BuiySet::Style),
            );

        // Main-world render-prep: clip computation reads GlobalTransform (C1),
        // so it MUST run AFTER the bridge's propagation chain (sync_simple_transforms
        // is its last system, lib.rs) — and before Picking, so picking + extract
        // see settled ClipRects (architecture.md § 5.2). Runs on CI/headless —
        // no RenderApp required.
        app.add_systems(
            Update,
            clip::write_clip_rects
                .after(bevy::transform::systems::sync_simple_transforms)
                .after(crate::BuiySet::Animate)
                .before(crate::BuiySet::Picking),
        );

        // Render-prep (main world): derive the EffectGroup marker alongside
        // WriteClipRects, in the .after(Animate).before(Picking) window
        // (effect-compositor.md § 1.1). Main-world ECS work, not a RenderApp
        // system, so it is registered before the RenderApp branch — it lands
        // even on headless hosts with no RenderApp.
        app.add_systems(
            Update,
            effect::write_effect_groups
                .after(crate::BuiySet::Animate)
                .before(crate::BuiySet::Picking),
        );

        // Render-prep (main world): the subtree visibility-suppression walk
        // (paint-order-and-top-layer.md § 5.3 / § 5.4) writes the computed
        // ComputedPaintSkip marker across each CssVisibility::Hidden /
        // OffscreenAuto subtree, in the same Animate→Picking window as the
        // clip / effect passes, so extract reads a settled marker. Headless-
        // safe for the same reason as its siblings.
        app.add_systems(
            Update,
            visibility::write_paint_skip
                .after(crate::BuiySet::Animate)
                .before(crate::BuiySet::Picking),
        );

        // Debug-only pick-occlusion coherence (4b-scope): DEBUG-PANIC on a
        // hand-authored transparent `.top_layer()` `Node` that lacks
        // `Pickable::IGNORE` (the invisible-occluder bug class). Scheduled HERE, in
        // `Last`, because this plugin owns `write_paint_skip` above — so a closed
        // (`Display::None` / `CssVisibility::Hidden`) overlay has a settled
        // `ComputedPaintSkip` by `Last` and is correctly excluded (no false
        // positive on a hidden overlay). Fail-loud, NOT a silent auto-`IGNORE`: the
        // `buiy_view` reconciler's auto-ignore-at-construction stays the ONLY place
        // a transparent top-layer container is auto-repaired; every other surface
        // (hand-authored core/widgets/`bsn!`) must fix its own tree, loudly. The
        // system is read-only (`Query<EntityRef>`), so it is MT-executor sound.
        // Compiled out of release builds entirely.
        #[cfg(debug_assertions)]
        app.add_systems(
            bevy::app::Last,
            crate::picking::coherence::assert_no_transparent_top_layer_occluder,
        );

        // Register author-set render components (reflection / BSN / inspectors)
        // in the MAIN world, before the RenderApp branch, so registration
        // happens even on headless hosts with no RenderApp (component-model.md
        // § 13). The computed ClipRect/AncestorClip/EffectGroup/
        // ComputedPaintSkip and the layout-owned OffscreenAuto are
        // deliberately NOT registered here.
        app.register_type::<components::Background>()
            // Gradient / layered fills (parity Wave B1) — the sibling
            // decomposed component + its layer enum and value types.
            .register_type::<components::BackgroundLayers>()
            .register_type::<components::BackgroundLayer>()
            .register_type::<components::LinearGradient>()
            .register_type::<components::RadialGradient>()
            .register_type::<components::ColorStop>()
            .register_type::<components::Border>()
            .register_type::<components::BorderSide>()
            .register_type::<components::Corners>()
            .register_type::<components::Radius>()
            .register_type::<components::LineStyle>()
            .register_type::<components::BoxShadow>()
            .register_type::<components::Shadow>()
            .register_type::<components::Opacity>()
            .register_type::<components::QuadAlpha>()
            .register_type::<components::Outline>()
            .register_type::<components::Filter>()
            .register_type::<components::BackdropFilter>()
            .register_type::<components::MixBlendMode>()
            .register_type::<components::FilterFn>()
            .register_type::<components::Angle>()
            .register_type::<components::CssVisibility>()
            .register_type::<components::ClipRadius>()
            .register_type::<color::ColorToken>()
            .register_type::<color::SystemColorKeyword>()
            .register_type::<components::Icon>()
            // The raster (textured-quad) canvas primitive (the drawing-canvas
            // seam). Author-set, so registered here in the main world.
            .register_type::<raster::RasterImage>()
            .register_type::<components::TextColor>()
            // T7: the caret-color tier-1 override (decoration-and-paint
            // § 6.2). CaretVisual/SelectionVisual are machinery state and
            // deliberately NOT registered.
            .register_type::<components::CaretColor>();

        // Buiy's WGSL shaders are MAIN-world assets: `AssetPlugin` owns
        // `Assets<Shader>` in the main world; the render world only receives the
        // extracted GPU mirror (which the `PipelineCache` resolves the handle
        // against). Load them into the MAIN world here — NOT into the render
        // world (the render world has no `Assets<Shader>` resource, so a
        // render-world insert panics at build). Guarded to the real render path:
        // only when a RenderApp AND the asset store both exist, so the headless
        // gate (MinimalPlugins, no AssetPlugin/RenderApp) is unaffected.
        if app.get_sub_app(RenderApp).is_some()
            && app
                .world()
                .get_resource::<Assets<bevy::shader::Shader>>()
                .is_some()
        {
            bevy::asset::load_internal_asset!(
                app,
                pipeline::shader_handle(),
                "shader.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            bevy::asset::load_internal_asset!(
                app,
                pipeline::shadow_shader_handle(),
                "shadow.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            // The coverage-glyph (alpha-as-color) shader (octet ..03). Loaded
            // into the MAIN world exactly like the quad/shadow shaders — the
            // glyph pipeline (primitive.rs::specialize for Glyph) resolves this
            // handle through the PipelineCache's extracted GPU mirror.
            bevy::asset::load_internal_asset!(
                app,
                pipeline::coverage_shader_handle(),
                "coverage.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            // The effect-group composite shader (octet ..05). Loaded into the
            // MAIN world like the quad/shadow/coverage shaders; the composite
            // pipeline (composite.rs::specialize) resolves this handle through the
            // PipelineCache's extracted GPU mirror.
            bevy::asset::load_internal_asset!(
                app,
                composite::composite_shader_handle(),
                "composite.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            // The border/outline BAND shader (octet ..06, styling-f-tier.md
            // § 2.3 — C6-a feeds the OUTLINE channel). Loaded into the MAIN world
            // like its siblings; the band pipeline (primitive.rs
            // `BuiyBandPipeline::specialize`) resolves this handle through the
            // PipelineCache's extracted GPU mirror.
            bevy::asset::load_internal_asset!(
                app,
                pipeline::band_shader_handle(),
                "band.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            // The background-gradient shader (octet ..07, parity Wave B1).
            // Loaded into the MAIN world like its siblings; the gradient pipeline
            // (primitive.rs `BuiyGradientPipeline::specialize`) resolves this
            // handle through the PipelineCache's extracted GPU mirror.
            bevy::asset::load_internal_asset!(
                app,
                pipeline::gradient_shader_handle(),
                "gradient.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            // The backdrop-blur shader (octet ..08, parity Wave B4). Loaded into
            // the MAIN world like its siblings; the blur pipeline
            // (blur.rs `BlurPipeline::specialize`) resolves this handle through
            // the PipelineCache's extracted GPU mirror.
            bevy::asset::load_internal_asset!(
                app,
                blur::blur_shader_handle(),
                "blur.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            // The raster (textured-quad) shader (octet ..09, the drawing-canvas
            // seam). Loaded into the MAIN world like its siblings; the raster
            // pipeline (raster.rs `BuiyRasterPipeline::specialize`) resolves this
            // handle through the PipelineCache's extracted GPU mirror.
            bevy::asset::load_internal_asset!(
                app,
                raster::raster_shader_handle(),
                "raster.wgsl",
                bevy::shader::Shader::from_wgsl
            );
            // The rounded box-shadow shader (F4b-6, octet ..0A) — the blurred
            // rounded-rect coverage + the crisp 3D-press edge.
            bevy::asset::load_internal_asset!(
                app,
                pipeline::rounded_shadow_shader_handle(),
                "rounded_shadow.wgsl",
                bevy::shader::Shader::from_wgsl
            );
        }

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<extract::ExtractedNodesView>()
            // #2 Stage C: the entity->slot index, rebuilt on Full extract (the
            // foundation for in-place Patch overwrites).
            .init_resource::<extract::RetainedNodeIndex>()
            // #2 Stage C3b: the Full-vs-Patch damage tag (prepare/Stage D reads it).
            .init_resource::<extract::NodeDamage>()
            // The per-view effect-group carrier `prepare_effect_groups` reads as a
            // non-Option `Res`. `extract_buiy_nodes` overwrites it every frame
            // (both paths), but init it here for the same reason as
            // `ExtractedNodesView`: the resource must exist before the first
            // Prepare even if extract were ever skipped/reordered.
            .init_resource::<extract::ExtractedEffectGroups>()
            // The persistent per-frame instance buffers (R6). Device-free to
            // construct (`RawBufferVec`/`UniformBuffer` allocate their GPU buffers
            // lazily on first `write_buffer`), so it is initialized here in `build`
            // rather than lazily in `prepare`. Initializing it up front lets
            // `prepare_buiy_instances` skip its re-upload on an unchanged frame and
            // RETAIN the prior buffer (architecture.md § 3.1 damage retention),
            // instead of the old one-frame warmup that returned without uploading.
            .init_resource::<prepare::BuiyInstanceBuffers>()
            // The cumulative upload counters `prepare_buiy_instances` records
            // (the `RtPoolStats` observable idiom): the caret-blink GPU damage
            // test reads them to pin "a blink frame re-uploads the glyph
            // buffer ONLY" (decoration-and-paint § 6.3; verification § 1.3).
            .init_resource::<prepare::BufferUploadStats>()
            // The per-tier "repacked from source this frame" bits (the H6 fix):
            // written unconditionally by `prepare_buiy_instances`, read by
            // `prepare_effect_groups` instead of re-deriving `is_changed()` —
            // one source of truth for the fold/merge gates.
            .init_resource::<prepare::PreparedDamage>()
            // Deterministic per-frame work-unit counters (perf-final P0b) — the
            // host-independent measurement gate. Registered here AND in the
            // `buiy_bench_support` harness via the same `RenderWorkCounters` type.
            .init_resource::<counters::RenderWorkCounters>()
            // The render-world glyph-instance list, filled per frame by
            // `text::extract_buiy_glyphs` (registered by `BuiyTextPlugin`, T4).
            // Kept `init_resource`'d here so the prepare gate works even if the
            // text plugin is absent (the glyph draw is then a no-op).
            .init_resource::<prepare::ExtractedGlyphs>()
            // Text's quad-tier carrier (underline/overline, T6; selection T7),
            // filled by `text::extract_buiy_glyphs`; init'd here so the prepare
            // gate works even if the text plugin is absent — the
            // `ExtractedGlyphs` rationale, verbatim.
            .init_resource::<extract::ExtractedTextQuads>()
            // Vector-icon carriers (parity Wave B3): the icon coverage-instance
            // list + the touch-key set, filled by `extract_buiy_icons` below.
            // Icons ride the SAME coverage pipeline as text glyphs (their record
            // IS a `GlyphAlphaInstance`) but through their OWN carrier/buffer/draw
            // so the wholesale-rebuilt glyph carrier stays decoupled (§ 3.5).
            .init_resource::<icon_producer::ExtractedIcons>()
            .init_resource::<icon_producer::ResidentIconKeys>()
            // Raster (textured-quad canvas) carriers (the drawing-canvas seam):
            // the per-frame extracted list + the persistent instance buffer.
            // Init'd here so the prepare/draw work even standalone; the device-
            // owning `RasterGpu` (the Nearest sampler) inits in `finish`.
            .init_resource::<raster::ExtractedRasters>()
            .init_resource::<raster::RasterBuffers>()
            // The per-view extract rework (R5). architecture § 1.2/§ 3/§ 4.
            .add_systems(ExtractSchedule, extract::extract_buiy_nodes)
            // The raster canvas extract: mirror every `RasterImage` layout node
            // into `ExtractedRasters` (independent of the node/glyph carriers).
            .add_systems(ExtractSchedule, raster::extract_buiy_rasters)
            // P0b: record atlas_touch_ops/resident_keys AFTER the glyph producer
            // refreshes `ResidentTextKeys` (the audit-#5 blind-spot counter).
            .add_systems(
                ExtractSchedule,
                counters::record_text_work_counters.after(crate::text::extract_buiy_glyphs),
            )
            // The vector-icon producer (parity Wave B3): rasterizes + atlas-inserts
            // each `Icon` and emits a tinted coverage instance. `.after(maintain_atlas)`
            // so inserts/touches use the just-advanced atlas frame clock (the
            // `text::extract_buiy_glyphs` precedent).
            .add_systems(
                ExtractSchedule,
                icon_producer::extract_buiy_icons.after(atlas::maintain_atlas),
            )
            // The prepare phase (R6): per-view persistent buffers + view
            // uniform, packed from R5's ExtractedNodes. ViewTarget exists in
            // RenderSystems::Prepare (architecture § 4), unlike in extract.
            .add_systems(
                Render,
                prepare::prepare_buiy_instances.in_set(RenderSystems::Prepare),
            )
            // The raster canvas prepare: copy `ExtractedRasters` into the
            // persistent `RasterBuffers` + upload (its own carrier, no ordering
            // vs the other Prepare systems — none reads the other's output).
            .add_systems(
                Render,
                raster::prepare_buiy_rasters.in_set(RenderSystems::Prepare),
            )
            // Per-view VIEW-pass pipeline specialization (quad + glyph keyed on
            // the view's attachment format AND `Msaa` sample count — a bare
            // `Camera2d` defaults to `Msaa::Sample4`, so the window pass is 4x
            // and the 1x baseline ids cannot bind there). Inserts the
            // `BuiyViewPipelines` carrier on the view render entity for
            // the `buiy_pass` system. No explicit ordering vs the sibling Prepare
            // systems: neither reads the other's output. NOTE: both this and
            // `prepare_effect_groups` hold `ResMut<BuiySpecializedPipelines>`,
            // so the scheduler serializes them in an arbitrary (harmless)
            // order — each only inserts keys into the shared caches.
            .add_systems(
                Render,
                pipeline::prepare_buiy_view_pipelines.in_set(RenderSystems::Prepare),
            );
        // Render pass: registers the `buiy_pass` system into the `Core2d`
        // schedule (`Core2dSystems::EarlyPostProcess`), device-free, so it stays
        // in build. The device-dependent pipeline init (`pipeline::register`)
        // runs in `finish` below — RenderDevice/PipelineCache do not exist until
        // RenderPlugin's own `finish` runs the renderer init.
        node::register(render_app);
        // Shared texture atlas (atlas-and-text-seam.md § 2): the render-world
        // BuiyAtlas + AtlasWarmupQueue resources plus the pre-paint
        // `warmup_atlas` drain in ExtractSchedule. Coverage/image primitives
        // (glyph/icon/gradient/mask) sample this one warehouse.
        atlas::register(render_app);
        // Compositor resources/pipelines (effect-compositor.md § 3): adds NO
        // pass system — the `buiy_pass` system is owned by node::register; the
        // composite passes run straight-line inside `buiy_pass` (one shared
        // RenderContext encoder). No-op until prepare_effect_groups lands (Task 9).
        compositor::register(render_app);
        // Backdrop-blur (parity Wave B4): the device-free blur specialization
        // cache. The device-owning `BlurPipeline` inits in `finish`
        // (`blur::register_gpu`). Adds no pass system — the blur runs
        // straight-line inside `buiy_pass` (the same one-encoder discipline as
        // the effect-group composites).
        blur::register(render_app);
    }

    /// Device-dependent render-world setup. MUST run in `finish`, not `build`:
    /// `RenderPlugin` inserts `RenderDevice` / `PipelineCache` into the render
    /// world during ITS `finish` (the async `initialize_renderer`), so they are
    /// absent at `build` time. `pipeline::register` creates the view bind-group
    /// layout + the unit-quad vertex buffer and queues the pipeline through the
    /// `PipelineCache` — all of which need the device.
    fn finish(&self, app: &mut App) {
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        pipeline::register(render_app);
        // The device-owning atlas half (`AtlasGpu`). MUST run AFTER
        // `pipeline::register` so `BuiyPipeline.atlas_layout` exists when the
        // first `prepare_atlas_textures` builds the coverage bind group, and in
        // `finish` (not `build`) because `AtlasGpu::from_world` needs the
        // `RenderDevice` that `RenderPlugin::finish` materializes.
        atlas::register_gpu(render_app);
        // The device-owning composite half (`CompositePipeline`: bind-group
        // layouts, sampler, unit-quad VBO). `finish` for the same reason — its
        // `FromWorld` needs the `RenderDevice`.
        composite::register_gpu(render_app);
        // The device-owning blur half (`BlurPipeline`: bind-group layouts, the
        // linear clamp sampler, the unit-quad VBO). `finish` for the same reason
        // as the composite half — its `FromWorld` needs the `RenderDevice`.
        blur::register_gpu(render_app);
        // The device-owning raster half (`RasterGpu`: the Nearest sampler every
        // raster `@group(1)` bind group uses). `finish` because its `FromWorld`
        // needs the `RenderDevice`. The `@group(1)` layout is reused from
        // `BuiyPipeline::atlas_layout` (built in `pipeline::register` above).
        render_app.init_resource::<raster::RasterGpu>();
    }
}
