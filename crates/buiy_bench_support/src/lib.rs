//! Shared adapterless shape→layout→extract harness + scenes for buiy_core
//! performance measurement.
//!
//! **Dev-only.** This crate is consumed ONLY as a dev-dependency (buiy_core's
//! criterion bench, the dhat alloc-budget test, the iai-callgrind bench), so it is
//! never in a production dependency graph — wasm-safe by construction.
//!
//! ## Why this crate exists (perf-final Phase 0a)
//!
//! The canonical `TextExtractHarness` lives in
//! `crates/buiy_core/tests/support/extract_harness.rs`, which a `benches/` target
//! cannot `mod` (it compiles only for the integration-test crate). The bench
//! therefore *replicated* the harness; adding a dhat test + an iai bench would make
//! it a TRIPLE copy. This crate is the single shared home so criterion / dhat / iai
//! all drive ONE harness (and, from Phase 0b, ONE counter-registration list).
//!
//! The harness drives the SAME production systems the real RenderApp does
//! (`extract_buiy_glyphs`, `extract_buiy_nodes`, `maintain_atlas`, the atlas
//! resources, `bevy::render::{ExtractSchedule, MainWorld}`) — it is HEADLESS (no
//! wgpu adapter, no `RenderApp`): the render side is a bare `World` carrying only
//! the CPU resources the producers touch; prepare/queue/draw never exist, so
//! nothing requests a GPU.

pub mod mvu_scenes;

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::atlas::{AtlasConfig, BuiyAtlas, maintain_atlas};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::extract::{
    ExtractedEffectGroups, ExtractedNodesView, ExtractedTextQuads, NodeDamage, RetainedNodeIndex,
    extract_buiy_nodes,
};
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::render::{RenderWorkCounters, record_text_work_counters};
use buiy_core::text::{
    BuiySwashCache, BuiyTextPlugin, FontKeyInterner, FontSize, GlyphMetaCache, ResidentTextKeys,
    SharedFontSystem, Text, extract_buiy_glyphs,
};
use buiy_core::{CorePlugin, Node};

/// The adapterless shape→layout→extract harness. The main `App` carries the full
/// headless text + layout pipeline (no `RenderPlugin`); the render `World` carries
/// only the CPU resources the extract producers read.
pub struct PipelineHarness {
    /// The main-world app (text + layout pipeline). `pub` so benches/tests can
    /// spawn into it and drive `app.update()` directly.
    pub app: App,
    /// The bare render world (CPU extract resources only — no device).
    pub render: World,
    extract: Schedule,
}

impl Default for PipelineHarness {
    fn default() -> Self {
        Self::new()
    }
}

impl PipelineHarness {
    pub fn new() -> Self {
        let mut app = App::new();
        // BuiyRenderPlugin's MAIN-world half registers headless; its render half
        // is guarded on a RenderApp that never exists here (no adapter).
        app.add_plugins(MinimalPlugins)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(CorePlugin)
            .add_plugins(LayoutPlugin)
            .add_plugins(BuiyTextPlugin::default())
            .add_plugins(BuiyRenderPlugin);
        // Synthetic primary window — the glyph + node producers read scale/presence
        // through a Query (the latency-test idiom).
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(1280, 1024),
                ..Default::default()
            },
            PrimaryWindow,
        ));

        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut render = World::new();
        render.insert_resource(BuiyAtlas::new(AtlasConfig::default()));
        render.init_resource::<ExtractedGlyphs>();
        render.init_resource::<ExtractedTextQuads>();
        // Audit #2 node-extract path: the per-view carrier + effect groups
        // `extract_buiy_nodes` overwrites each dirty frame (init'd so the
        // gate-skip path on a clean frame has a resident resource to retain).
        render.init_resource::<ExtractedNodesView>();
        render.init_resource::<ExtractedEffectGroups>();
        render.init_resource::<RetainedNodeIndex>();
        render.init_resource::<NodeDamage>();
        // P0b: the deterministic work-unit counters, registered here so the gate
        // tests can read them — the SAME `RenderWorkCounters` the real RenderApp
        // registers (one type, one registration list).
        render.init_resource::<RenderWorkCounters>();
        render.init_resource::<FontKeyInterner>();
        render.init_resource::<ResidentTextKeys>();
        render.init_resource::<GlyphMetaCache>();
        render.init_resource::<BuiySwashCache>();
        render.insert_resource(fonts);
        render.init_resource::<MainWorld>();

        let mut extract = Schedule::new(ExtractSchedule);
        // `record_text_work_counters` runs AFTER `extract_buiy_glyphs` (reads the
        // refreshed `ResidentTextKeys`); `extract_buiy_nodes` sets its own counts.
        extract.add_systems(
            (
                maintain_atlas,
                extract_buiy_glyphs,
                record_text_work_counters,
                extract_buiy_nodes,
            )
                .chain(),
        );

        Self {
            app,
            render,
            extract,
        }
    }

    /// One full pipeline pass: main-world Update (TextSync → measure/shape →
    /// TextCommit → layout) then the extract step against the live main world.
    pub fn frame(&mut self) {
        self.app.update();
        self.extract_only();
    }

    /// The extract step alone — swap the live main world into the render world's
    /// `MainWorld` slot, run `ExtractSchedule`, swap it back (bevy_render's own
    /// extract dance, minus the renderer).
    pub fn extract_only(&mut self) {
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
        self.extract.run(&mut self.render);
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
    }

    pub fn glyph_count(&self) -> usize {
        self.render.resource::<ExtractedGlyphs>().glyphs.len()
    }
}

/// A representative large UI: `paragraphs` wrapped text blocks (a heading + a
/// long body each) in a padded flex column — hundreds of nodes and a multi-
/// thousand-glyph shaping workload, the shape that stresses the per-frame hot
/// path. Returns the populated, NOT-yet-updated harness.
pub fn build_large_scene(paragraphs: usize) -> PipelineHarness {
    let mut h = PipelineHarness::new();

    // A wrapped body string long enough that each paragraph shapes into many
    // lines at the column width below — the cosmic-text shaping cost is what we
    // want dominant in the measurement.
    const BODY: &str = "The quick brown fox jumps over the lazy dog. Shaped by \
        cosmic-text at the committed wrap width, rasterized once per (font, size, \
        weight, subpixel-bin) into the shared coverage atlas, tinted per instance \
        — a theme switch never touches the atlas. Pack my box with five dozen \
        liquor jugs; the five boxing wizards jump quickly.";

    let mut children = Vec::with_capacity(paragraphs * 2);
    for i in 0..paragraphs {
        let title = h
            .app
            .world_mut()
            .spawn((
                Node,
                Style::default(),
                Text(format!("Section {i}: a representative heading")),
                FontSize(28.0),
            ))
            .id();
        let body = h
            .app
            .world_mut()
            .spawn((
                Node,
                Style::default(),
                Text(String::from(BODY)),
                FontSize(16.0),
            ))
            .id();
        children.push(title);
        children.push(body);
    }

    let root = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(720.0)
                .padding(24.0)
                .gap_px(12.0),
        ))
        .id();
    h.app.world_mut().entity_mut(root).add_children(&children);

    h
}

/// A flat, TEXT-FREE scene: `nodes` fixed-size plain `Node`s in a padded flex
/// column. No `Text`, so the per-frame cost is dominated by the LAYOUT pipeline
/// — the Taffy compute plus the ~12 post-Taffy full-tree passes (stacking,
/// transform composition, `ResolvedLayout` readback, writing-mode inherit, the
/// positioning sub-passes) that run UNCONDITIONALLY every frame with no run
/// condition (perf audit #3). Isolates #3 from the text/atlas cost (#5).
pub fn build_flat_scene(nodes: usize) -> PipelineHarness {
    let mut h = PipelineHarness::new();
    let mut children = Vec::with_capacity(nodes);
    for _ in 0..nodes {
        let id = h
            .app
            .world_mut()
            .spawn((Node, Style::default().width_px(40.0).height_px(20.0)))
            .id();
        children.push(id);
    }
    let root = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(800.0)
                .padding(8.0)
                .gap_px(2.0),
        ))
        .id();
    h.app.world_mut().entity_mut(root).add_children(&children);
    h
}

/// A flat scene of `nodes` painting `Background` nodes for the audit-#2
/// node-extract measurement. Each node carries a `Background`, so
/// `extract_buiy_nodes` builds a per-entity `ExtractedNode` record for ALL N on
/// any dirty frame. Returns the harness + one victim entity to mutate.
pub fn build_flat_bg_scene(nodes: usize) -> (PipelineHarness, Entity) {
    let mut h = PipelineHarness::new();
    let mut children = Vec::with_capacity(nodes);
    let mut victim = Entity::PLACEHOLDER;
    for i in 0..nodes {
        let id = h
            .app
            .world_mut()
            .spawn((
                Node,
                Style::default().width_px(40.0).height_px(20.0),
                Background {
                    // An opaque placeholder fill — the bench only needs a quad
                    // emitted, not a specific color (the old `"surface"` key
                    // missed the map and resolved to opaque magenta).
                    color: ColorToken::Custom(Color::WHITE),
                },
            ))
            .id();
        if i == 0 {
            victim = id;
        }
        children.push(id);
    }
    let root = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(800.0)
                .padding(8.0)
                .gap_px(2.0),
        ))
        .id();
    h.app.world_mut().entity_mut(root).add_children(&children);
    (h, victim)
}
