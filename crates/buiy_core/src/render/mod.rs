//! Buiy render pipeline. Phase 0 ships the smallest end-to-end pass
//! (rounded rect with solid bg) wired into Bevy's render graph. Full
//! pipeline (top-layer compositing, clip-path, filters, blend modes,
//! atlasing) lives in `buiy-render-pipeline-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/visuals.md § 3.3 and
//! architecture.md § 2.3.

use crate::{
    components::{Node, ResolvedLayout},
    theme::Theme,
};
use bevy::prelude::*;
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderSystems};

pub mod atlas;
pub mod bridge;
pub mod buckets;
pub mod clip;
pub mod color;
pub mod components;
pub mod composite;
pub mod compositor;
pub mod effect;
pub mod extract;
pub mod forced_colors;
pub mod forced_colors_analyzer;
pub mod golden;
pub mod instance;
pub mod node;
pub mod pipeline;
pub mod prepare;
pub mod primitive;
pub mod top_layer;
pub mod view_uniform;
pub mod visibility;

pub use bridge::ScrollDirty;
pub use clip::write_clip_rects;
pub use color::{ColorToken, SystemColorKeyword};
pub use components::{
    AncestorClip, Angle, BackdropFilter, Background, Border, BorderSide, BoxShadow, ClipRadius,
    ClipRect, ComputedPaintSkip, Corners, CssVisibility, EffectGroup, EffectReason, Filter,
    FilterFn, LineStyle, MixBlendMode, OffscreenAuto, Opacity, Outline, Radius, Shadow, SkipReason,
};
pub use visibility::{node_skip_reason, write_paint_skip};

/// What the render world needs from the main world per frame: a list of
/// (rect, color, radius) tuples in window-local logical pixels, plus the
/// primary window size used to convert them to clip space.
///
/// Populated only by `extract_buiy_draws` inside `ExtractSchedule`; this
/// resource is not part of the main-world public API and external authors
/// should not construct it directly. `#[non_exhaustive]` keeps additions
/// (top-layer compositing, clip-path, filters, blend modes, atlasing per
/// `buiy-render-pipeline-design`) non-breaking.
#[derive(Resource, Default, Clone)]
#[non_exhaustive]
pub struct ExtractedDraws {
    pub draws: Vec<DrawData>,
    /// Logical-pixel size of the primary window this frame. Populated by the
    /// extract system. The live render path no longer reads it (the view
    /// uniform owns the logical → clip transform); it stays only for the
    /// Phase-0 `ExtractedDraws` carrier until the extract phase retires it.
    /// Zero on frames where no window exists.
    pub window_size: Vec2,
}

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

        // Main-world render-prep: clip computation runs between Animate and
        // Picking (architecture.md § 5.2) so picking + extract see settled
        // ClipRects. Runs on CI/headless — no RenderApp required.
        app.add_systems(
            Update,
            clip::write_clip_rects
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

        // Register author-set render components (reflection / BSN / inspectors)
        // in the MAIN world, before the RenderApp branch, so registration
        // happens even on headless hosts with no RenderApp (component-model.md
        // § 13). The computed ClipRect/AncestorClip/EffectGroup/
        // ComputedPaintSkip and the layout-owned OffscreenAuto are
        // deliberately NOT registered here.
        app.register_type::<components::Background>()
            .register_type::<components::Border>()
            .register_type::<components::BorderSide>()
            .register_type::<components::Corners>()
            .register_type::<components::Radius>()
            .register_type::<components::LineStyle>()
            .register_type::<components::BoxShadow>()
            .register_type::<components::Shadow>()
            .register_type::<components::Opacity>()
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
            .register_type::<components::TextColor>();

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
        }

        // ExtractedDraws is render-world only — the main world does not read it.
        // Initialization lives below inside the RenderApp branch.
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<ExtractedDraws>()
            .init_resource::<extract::ExtractedNodesView>()
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
            // The render-world glyph-instance list the text seam (unbuilt) fills
            // per frame; empty in v1 production (no in-crate glyph producer — the
            // seam is deferred), so the glyph draw is a no-op until text lands.
            // The GPU atlas tests play the producer and fill it directly.
            .init_resource::<prepare::ExtractedGlyphs>()
            // Phase-0 draw path (feeds node.rs today); retired by R6/R8 (the
            // node/instance rework) when node.rs reads the per-view
            // ExtractedNodes instead.
            .add_systems(ExtractSchedule, extract_buiy_draws)
            // The per-view extract rework (R5). architecture § 1.2/§ 3/§ 4.
            .add_systems(ExtractSchedule, extract::extract_buiy_nodes)
            // The prepare phase (R6): per-view persistent buffers + view
            // uniform, packed from R5's ExtractedNodes. ViewTarget exists in
            // RenderSystems::Prepare (architecture § 4), unlike in extract.
            .add_systems(
                Render,
                prepare::prepare_buiy_instances.in_set(RenderSystems::Prepare),
            )
            // Per-view VIEW-pass pipeline specialization (quad + glyph keyed on
            // the view's attachment format AND `Msaa` sample count — a bare
            // `Camera2d` defaults to `Msaa::Sample4`, so the window pass is 4x
            // and the 1x baseline ids cannot bind there). Inserts the
            // `BuiyViewPipelines` carrier on the view render entity for
            // `BuiyNode::run`. No explicit ordering vs the sibling Prepare
            // systems: neither reads the other's output. NOTE: both this and
            // `prepare_effect_groups` hold `ResMut<BuiySpecializedPipelines>`,
            // so the scheduler serializes them in an arbitrary (harmless)
            // order — each only inserts keys into the shared caches.
            .add_systems(
                Render,
                pipeline::prepare_buiy_view_pipelines.in_set(RenderSystems::Prepare),
            );
        // Render-graph node: graph TOPOLOGY only (add_render_graph_node + edges),
        // device-free, so it stays in build. The device-dependent pipeline init
        // (`pipeline::register`) runs in `finish` below — RenderDevice/PipelineCache
        // do not exist until RenderPlugin's own `finish` runs the renderer init.
        node::register(render_app);
        // Shared texture atlas (atlas-and-text-seam.md § 2): the render-world
        // BuiyAtlas + AtlasWarmupQueue resources plus the pre-paint
        // `warmup_atlas` drain in ExtractSchedule. Coverage/image primitives
        // (glyph/icon/gradient/mask) sample this one warehouse.
        atlas::register(render_app);
        // Compositor resources/pipelines (effect-compositor.md § 3): adds NO
        // render-graph node — the BuiyRenderLabel group + edges are owned by
        // node::register / architecture § 1.3; the composite passes run inside
        // BuiyNode::run. No-op until prepare_effect_groups lands (Task 9).
        compositor::register(render_app);
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
    }
}

/// Phase-0 parity helper: the uniform corner radius in logical px, read from
/// the top-left corner's x radius. Px-only via [`clip::px_or_zero`]; other
/// units resolve to `0` for now (paint-`Length` resolution is a later-phase
/// concern). A `Border`-less entity is square (radius 0).
fn uniform_radius_px(corners: &Corners) -> f32 {
    clip::px_or_zero(corners.top_left.x)
}

/// Build the [`DrawData`] for one node, or `None` when the resolved fill is
/// transparent (`Background` set to `ColorToken::Transparent`, or a token that
/// resolves to `Color::NONE`) — Phase-0 parity with the old empty-string skip.
/// Emits a `warn!` for each node whose named token misses (no per-session
/// dedup yet — fires every extract; the sentinel magenta is still drawn).
///
/// `position` is the bridge-composed `GlobalTransform.translation().truncate()`
/// (pillar 5: render reads `GlobalTransform`, not `ResolvedLayout.position` —
/// the bridge folds transform + ancestor scroll into it, clip-and-transform.md
/// § B.5); `size` still comes from `ResolvedLayout` (the bridge does not carry
/// size). Both are logical-px, y-down (§ B.4) — the y-flip lives in the GPU
/// view uniform, applied downstream.
///
/// Pure (no ECS params) so the transparent-skip and the `Border.radius`
/// (`Corners`) → quad-`radius` migration are unit-testable without a RenderApp
/// / wgpu adapter — the [`extract_buiy_draws`] system that wraps it needs one.
fn draw_for_node(
    background: &Background,
    border: Option<&Border>,
    position: Vec2,
    layout: &ResolvedLayout,
    theme: &Theme,
) -> Option<DrawData> {
    // The canonical §2.1 resolver (color::resolve_token) is the single owner of
    // token→Color mapping and of the missing-token `warn!` (via resolve_named),
    // so this path neither re-derives the miss nor re-emits the warn.
    let color = color::resolve_token(&background.color, theme);
    if color == Color::NONE {
        return None;
    }
    let radius = border.map(|b| uniform_radius_px(&b.radius)).unwrap_or(0.0);
    Some(DrawData::new(position, layout.size, color, radius))
}

/// Extract laid-out nodes into [`ExtractedDraws`]. Render reads
/// `GlobalTransform` for position (pillar 5 / clip-and-transform.md § B.5),
/// not `ResolvedLayout.position`: the `write_buiy_transform` bridge folds the
/// composed transform + ancestor scroll into `GlobalTransform`, which the
/// `BuiySet::RenderPrep` propagation chain finalizes in `Update` before this
/// extract runs. Coupling the query to `&GlobalTransform` means a bare
/// `Background` spawned without going through layout (so the bridge never
/// inserted a `Transform`/`GlobalTransform`) is dropped — acceptable: render
/// only paints laid-out entities, and the bridge writes a `Transform` (→
/// required `GlobalTransform`) on every `Node` carrying `ResolvedLayout`.
///
/// The other half of the § B.5 consumption contract is the per-primitive
/// backface cull bit: `backface-visibility: Hidden` culls a primitive when
/// its transformed normal faces away (the sign of the transformed z-basis of
/// `GlobalTransform`). That bit is read **directly** from the layout-owned
/// [`UiTransform::backface_visibility`](crate::UiTransform) — a one-bit flag,
/// so § B.5 introduces **no new render component** for it. The bridge does not
/// consume the bit; the cull itself is a paint-phase decision deferred to the
/// render-graph paint pass (no SDF cull is built this phase), at which point
/// the extract grows a `BackfaceVisibility` column read off `UiTransform`
/// alongside `GlobalTransform`. Pinning the *source* (layout's `UiTransform`,
/// not a new component) here keeps the contract fixed before the consumer
/// lands. Perspective / `Preserve3d` is the third § B.5 point: affine-
/// incompatible and C-tier deferred — pinned by the bridge's
/// `from_matrix`-drops-the-projective-row test, not here.
///
/// Token resolution re-reads `Res<Theme>` live every frame, so a theme swap
/// (light↔dark or the forced-colors variant) re-resolves all token-bearing
/// paint with no cached, theme-stamped buffer (color-and-forced-colors.md
/// § 2.3). The component-model phase replaces `Visual.background_token` with
/// `ColorToken`-bearing `Background`/`Border`/`Outline`/`BoxShadow`, resolved
/// via [`crate::render::color::resolve_token`]; the `theme.is_changed()` global
/// re-resolve signal then bypasses the per-entity `Changed<T>` short-circuit
/// on a theme-only switch.
#[allow(clippy::type_complexity)]
fn extract_buiy_draws(
    mut commands: Commands,
    main_world_q: Extract<
        Query<
            (
                &Background,
                Option<&Border>,
                &ResolvedLayout,
                &GlobalTransform,
            ),
            With<Node>,
        >,
    >,
    main_world_theme: Extract<Res<Theme>>,
    main_world_windows: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
) {
    let mut draws = ExtractedDraws::default();
    if let Ok(window) = main_world_windows.single() {
        let res = window.resolution.size();
        draws.window_size = Vec2::new(res.x, res.y);
    }
    for (background, border, layout, global) in main_world_q.iter() {
        let position = global.translation().truncate();
        if let Some(draw) = draw_for_node(background, border, position, layout, &main_world_theme) {
            draws.draws.push(draw);
        }
    }
    // TODO(v0.x): reuse ExtractedDraws via ResMut + clear/extend instead of
    // reallocating the inner Vec each frame. See `buiy-render-pipeline-design`.
    commands.insert_resource(draws);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn theme_with(token: &str, color: Color) -> Theme {
        let mut t = Theme::default();
        t.colors.insert(token.to_string(), color);
        t
    }

    fn layout(pos: Vec2, size: Vec2) -> ResolvedLayout {
        ResolvedLayout {
            position: pos,
            size,
        }
    }

    #[test]
    fn uniform_radius_reads_top_left_circular_px() {
        // The Phase-0 radius migration: `Border.radius` (`Corners`) → the
        // legacy single-`f32` quad radius via the top-left corner's x, px-only.
        assert_eq!(uniform_radius_px(&Corners::all(Radius::circular(6.0))), 6.0);
        assert_eq!(uniform_radius_px(&Corners::ZERO), 0.0);
        // Reads the top-left corner specifically, not any other corner: a
        // non-uniform `Corners` resolves to its top-left x, not top-right.
        assert_eq!(
            uniform_radius_px(&Corners {
                top_left: Radius::circular(4.0),
                top_right: Radius::circular(8.0),
                ..Corners::ZERO
            }),
            4.0
        );
    }

    #[test]
    fn transparent_background_emits_no_draw() {
        // `ColorToken::Transparent` → `Color::NONE` → skipped (no quad), the
        // Phase-0 empty-string parity. Theme is irrelevant on this path.
        let bg = Background {
            color: ColorToken::Transparent,
        };
        let l = layout(Vec2::ZERO, Vec2::splat(10.0));
        assert!(draw_for_node(&bg, None, Vec2::ZERO, &l, &Theme::default()).is_none());
    }

    #[test]
    fn tokened_background_with_border_carries_color_size_and_migrated_radius() {
        // The load-bearing extract path: resolve the token, take position from
        // the bridge-composed `GlobalTransform` (the `position` arg, distinct
        // from `ResolvedLayout.position` here to prove the source — pillar 5),
        // take size from `ResolvedLayout`, and migrate `Border.radius`
        // (`Corners`) → the single quad `radius` via the top-left x.
        let theme = theme_with("color.surface.secondary", Color::srgb(0.2, 0.3, 0.4));
        let bg = Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
        };
        let border = Border {
            radius: Corners::all(Radius::circular(6.0)),
            ..Default::default()
        };
        // `ResolvedLayout.position` is deliberately NOT the draw position: the
        // bridge's `GlobalTransform`-derived `position` arg wins.
        let l = layout(Vec2::new(10.0, 20.0), Vec2::new(30.0, 40.0));
        let global_position = Vec2::new(55.0, 66.0);

        let draw = draw_for_node(&bg, Some(&border), global_position, &l, &theme)
            .expect("opaque fill draws");
        assert_eq!(draw.color, Color::srgb(0.2, 0.3, 0.4));
        assert_eq!(draw.position, global_position);
        assert_eq!(draw.size, Vec2::new(30.0, 40.0));
        assert_eq!(draw.radius, 6.0);
    }

    #[test]
    fn missing_token_draws_magenta_sentinel_and_borderless_is_square() {
        // A miss still emits a quad (magenta sentinel, visible in screenshots),
        // and a `Border`-less node is square (radius 0).
        let bg = Background {
            color: ColorToken::Token(Cow::Borrowed("nope.not.here")),
        };
        let l = layout(Vec2::ZERO, Vec2::splat(10.0));
        let draw = draw_for_node(&bg, None, Vec2::ZERO, &l, &Theme::default())
            .expect("sentinel still draws");
        assert_eq!(draw.color, Color::srgb(1.0, 0.0, 1.0));
        assert_eq!(draw.radius, 0.0);
    }
}
