//! Buiy render pipeline. Phase 0 ships the smallest end-to-end pass
//! (rounded rect with solid bg) wired into Bevy's render graph. Full
//! pipeline (top-layer compositing, clip-path, filters, blend modes,
//! atlasing) lives in `buiy-render-pipeline-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/visuals.md § 3.3 and
//! architecture.md § 2.3.

use crate::{
    components::{Node, ResolvedLayout, Visual},
    theme::Theme,
};
use bevy::prelude::*;
use bevy::render::{Extract, ExtractSchedule, RenderApp};

pub mod instance;
pub mod node;
pub mod pipeline;

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

fn extract_buiy_draws(
    mut commands: Commands,
    main_world_q: Extract<Query<(&Visual, &ResolvedLayout), With<Node>>>,
    main_world_theme: Extract<Res<Theme>>,
    main_world_windows: Extract<Query<&Window, With<bevy::window::PrimaryWindow>>>,
) {
    let mut draws = ExtractedDraws::default();
    if let Ok(window) = main_world_windows.single() {
        let res = window.resolution.size();
        draws.window_size = Vec2::new(res.x, res.y);
    }
    for (visual, layout) in main_world_q.iter() {
        let color = match main_world_theme.color(&visual.background_token) {
            Some(c) => c,
            None => {
                tracing::warn!(
                    token = %visual.background_token,
                    "missing theme color token; falling back to magenta sentinel"
                );
                MISSING_TOKEN_FALLBACK
            }
        };
        draws.draws.push(DrawData {
            position: layout.position,
            size: layout.size,
            color,
            radius: visual.border_radius,
        });
    }
    // TODO(v0.x): reuse ExtractedDraws via ResMut + clear/extend instead of
    // reallocating the inner Vec each frame. See `buiy-render-pipeline-design`.
    commands.insert_resource(draws);
}
