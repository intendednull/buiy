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

pub mod clip;
pub mod color;
pub mod components;
pub mod instance;
pub mod node;
pub mod pipeline;

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
            .add_systems(ExtractSchedule, extract_buiy_draws);
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
const MISSING_TOKEN_FALLBACK: Color = Color::srgb(1.0, 0.0, 1.0);

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

#[allow(clippy::type_complexity)]
fn extract_buiy_draws(
    mut commands: Commands,
    main_world_q: Extract<Query<(&Background, Option<&Border>, &ResolvedLayout), With<Node>>>,
    main_world_theme: Extract<Res<Theme>>,
    main_world_windows: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
) {
    let mut draws = ExtractedDraws::default();
    if let Ok(window) = main_world_windows.single() {
        let res = window.resolution.size();
        draws.window_size = Vec2::new(res.x, res.y);
    }
    for (background, border, layout) in main_world_q.iter() {
        let (color, missed) = resolve_token(&background.color, &main_world_theme);
        if missed {
            tracing::warn!(
                token = ?background.color,
                "missing theme color token; falling back to magenta sentinel"
            );
        }
        // Phase-0 parity: skip emitting a quad for a transparent fill
        // (`Background` absent OR `ColorToken::Transparent`), matching the
        // old empty-string skip.
        if color == Color::NONE {
            continue;
        }
        let radius = border.map(|b| uniform_radius_px(&b.radius)).unwrap_or(0.0);
        draws
            .draws
            .push(DrawData::new(layout.position, layout.size, color, radius));
    }
    // TODO(v0.x): reuse ExtractedDraws via ResMut + clear/extend instead of
    // reallocating the inner Vec each frame. See `buiy-render-pipeline-design`.
    commands.insert_resource(draws);
}
