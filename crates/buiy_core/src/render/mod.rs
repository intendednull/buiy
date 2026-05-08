//! Buiy render pipeline. Phase 0 ships the smallest end-to-end pass
//! (rounded rect with solid bg) wired into Bevy's render graph. Full
//! pipeline (top-layer compositing, clip-path, filters, blend modes,
//! atlasing) lives in `buiy-render-pipeline-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/visuals.md § 3.3 and
//! architecture.md § 2.3.

use crate::{
    components::{Node, ResolvedLayout, Style},
    theme::Theme,
};
use bevy::prelude::*;
use bevy::render::{Extract, ExtractSchedule, RenderApp};

pub mod node;
pub mod pipeline;

/// What the render world needs from the main world per frame: a list of
/// (rect, color, radius) tuples in window-local logical pixels.
///
/// `#[non_exhaustive]` — the full pipeline (top-layer compositing,
/// clip-path, filters, blend modes, atlasing per
/// `buiy-render-pipeline-design`) will add fields here. The render
/// world does not re-export this for main-world consumers, so the only
/// out-of-crate audience is render-graph plugin authors who already
/// know to update through `..Default::default()` shims.
#[derive(Resource, Default, Clone)]
#[non_exhaustive]
pub struct ExtractedDraws {
    pub draws: Vec<DrawData>,
}

/// One rectangle in the Phase 0 render queue. Marked `#[non_exhaustive]`
/// because the full pipeline (clip-path, filters, blend modes, etc.)
/// will add per-draw fields pre-1.0. `Default` is derived so external
/// authors can construct via the `..Default::default()` shim referenced
/// in `ExtractedDraws` above.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub struct DrawData {
    pub position: Vec2,
    pub size: Vec2,
    pub color: Color,
    pub radius: f32,
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
    main_world_q: Extract<Query<(&Style, &ResolvedLayout), With<Node>>>,
    main_world_theme: Extract<Res<Theme>>,
) {
    let mut draws = ExtractedDraws::default();
    for (style, layout) in main_world_q.iter() {
        let color = match main_world_theme.color(&style.background_token) {
            Some(c) => c,
            None => {
                tracing::warn!(
                    token = %style.background_token,
                    "missing theme color token; falling back to magenta sentinel"
                );
                MISSING_TOKEN_FALLBACK
            }
        };
        draws.draws.push(DrawData {
            position: layout.position,
            size: layout.size,
            color,
            radius: style.border_radius,
        });
    }
    // TODO(v0.x): reuse ExtractedDraws via ResMut + clear/extend instead of
    // reallocating the inner Vec each frame. See `buiy-render-pipeline-design`.
    commands.insert_resource(draws);
}
