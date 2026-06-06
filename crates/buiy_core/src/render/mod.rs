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
use bevy::render::{Extract, ExtractSchedule, RenderApp};

pub mod bridge;
pub mod clip;
pub mod color;
pub mod components;
pub mod effect;
pub mod extract;
pub mod instance;
pub mod node;
pub mod pipeline;

pub use bridge::ScrollDirty;
pub use clip::write_clip_rects;
pub use color::{ColorToken, SystemColorKeyword};
pub use components::{
    AncestorClip, Angle, BackdropFilter, Background, Border, BorderSide, BoxShadow, ClipRadius,
    ClipRect, Corners, CssVisibility, EffectGroup, EffectReason, Filter, FilterFn, LineStyle,
    MixBlendMode, OffscreenAuto, Opacity, Outline, Radius, Shadow,
};

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
    /// extract system. Render-graph nodes use this to convert
    /// `DrawData` (px) → `InstanceData` (clip) per the Phase 0 closeout
    /// design. Zero on frames where no window exists; the render node
    /// must skip drawing in that case.
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

        // Register author-set render components (reflection / BSN / inspectors)
        // in the MAIN world, before the RenderApp branch, so registration
        // happens even on headless hosts with no RenderApp (component-model.md
        // § 13). The computed ClipRect/AncestorClip/EffectGroup and the
        // layout-owned OffscreenAuto are deliberately NOT registered here.
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
            .register_type::<color::SystemColorKeyword>();

        // ExtractedDraws is render-world only — the main world does not read it.
        // Initialization lives below inside the RenderApp branch.
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<ExtractedDraws>()
            .init_resource::<extract::ExtractedNodesView>()
            // Phase-0 draw path (feeds node.rs today); retired by R6/R8 (the
            // node/instance rework) when node.rs reads the per-view
            // ExtractedNodes instead.
            .add_systems(ExtractSchedule, extract_buiy_draws)
            // The per-view extract rework (this phase). architecture § 1.2/§ 3/§ 4.
            .add_systems(ExtractSchedule, extract::extract_buiy_nodes);
        // Phase 0: render-graph node + pipeline initialization.
        // The actual pipeline + node wiring lives in pipeline.rs and node.rs.
        node::register(render_app);
        pipeline::register(render_app);
    }
}

/// Sentinel color for missing theme tokens (magenta = "missing", visible at a
/// glance in screenshots). The accompanying `warn!` surfaces the typo'd token
/// name in dev. Phase 0 has a small, known token set; v0.x can promote to an
/// `error!` once tokens are typed.
pub(crate) const MISSING_TOKEN_FALLBACK: Color = Color::srgb(1.0, 0.0, 1.0);

/// Resolve a [`ColorToken`] against the active theme. Returns the resolved
/// `Color` and whether a named token *missed* (so the caller can emit one
/// `warn!`). Mirrors the Phase-0 `Visual.background_token` resolution:
/// `Transparent` → `Color::NONE`; `Token(name)` → `Theme::color(name)` or
/// the magenta sentinel on miss; `CurrentColor` / `SystemColor(_)` use the
/// v1 fallback (theme default foreground / system-color map — owned by
/// color-and-forced-colors.md; here they route through `Theme::color` of the
/// fallback token and sentinel-on-miss).
pub fn resolve_token(token: &ColorToken, theme: &Theme) -> (Color, bool) {
    // Each named token maps to the theme key to look up; the lookup-or-sentinel
    // step is shared. `currentColor`'s v1 fallback is the theme default
    // foreground token (color-and-forced-colors.md § 2.0); `SystemColor`'s
    // map is owned by buiy-theme-tokens-design and misses (sentinel) until it
    // lands.
    let name = match token {
        ColorToken::Transparent => return (Color::NONE, false),
        ColorToken::SystemColor(_) => return (MISSING_TOKEN_FALLBACK, true),
        ColorToken::Token(name) => name.as_ref(),
        ColorToken::CurrentColor => "color.text.primary",
    };
    match theme.color(name) {
        Some(c) => (c, false),
        None => (MISSING_TOKEN_FALLBACK, true),
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
    let (color, missed) = resolve_token(&background.color, theme);
    if missed {
        tracing::warn!(
            token = ?background.color,
            "missing theme color token; falling back to magenta sentinel"
        );
    }
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
