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
#[derive(Resource, Default, Clone)]
pub struct ExtractedDraws {
    pub draws: Vec<DrawData>,
}

#[derive(Clone, Copy, Debug)]
pub struct DrawData {
    pub position: Vec2,
    pub size: Vec2,
    pub color: Color,
    pub radius: f32,
}

pub struct BuiyRenderPlugin;

impl Plugin for BuiyRenderPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ExtractedDraws>();
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

fn extract_buiy_draws(
    mut commands: Commands,
    main_world_q: Extract<Query<(&Style, &ResolvedLayout), With<Node>>>,
    main_world_theme: Extract<Res<Theme>>,
) {
    let mut draws = ExtractedDraws::default();
    for (style, layout) in main_world_q.iter() {
        let color = main_world_theme
            .color(&style.background_token)
            .unwrap_or(Color::WHITE);
        draws.draws.push(DrawData {
            position: layout.position,
            size: layout.size,
            color,
            radius: style.border_radius,
        });
    }
    commands.insert_resource(draws);
}
