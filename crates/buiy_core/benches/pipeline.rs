//! Shape→layout→extract performance bench (audit finding #40, decision gate
//! DG-3). This is the workspace's FIRST and only wall-clock performance signal.
//!
//! ## What it measures, and what it is NOT
//!
//! The suite's `text_input_latency.rs` / `text_typing_latency.rs` files measure
//! FRAME-COUNT convergence (how many `app.update()` frames until an edit's glyph
//! publishes), *not* wall-clock time — see their reworded module docs. This bench
//! is the wall-clock complement: it times the per-frame hot path
//!
//!   TextSync → Taffy measure (cosmic-text SHAPING) → TextCommit → write layout
//!     → `extract_buiy_glyphs`
//!
//! over a LARGE scene, so an O(n²) regression or a per-frame allocation blow-up
//! in shaping / layout / extract shows up as a number that moves. It is driven
//! HEADLESS — no wgpu adapter, no `RenderApp` — on the same adapterless extract
//! harness the latency tests use (the render side is a bare `World` carrying only
//! the CPU resources the glyph producer touches; prepare/queue/draw never exist,
//! so nothing requests a GPU).
//!
//! ## Posture: INFORMATIONAL, never a CI gate (DG-3)
//!
//! Run manually with `cargo bench -p buiy_core --bench pipeline`. There is NO CI
//! step that fails the build on a slower number — wall-clock is host- and
//! load-dependent, so a hard threshold would flake. It is a signal a maintainer
//! reads (or diffs across commits with `cargo bench -- --baseline <name>`), not a
//! pass/fail check. See the campaign report's resolution note for the rationale.
//!
//! ## Why the harness is replicated here
//!
//! The canonical `TextExtractHarness` lives in
//! `crates/buiy_core/tests/support/extract_harness.rs`, which a `benches/` target
//! cannot `mod` (it is compiled only for the integration-test crate). The minimal
//! main-world-swap extract step is replicated below from the SAME public surface
//! the harness uses (`buiy_core::text::extract_buiy_glyphs`, the atlas resources,
//! `bevy::render::{ExtractSchedule, MainWorld}`) so both stay honest about driving
//! the real production systems, not a reimplementation.

use std::hint::black_box;

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};
use criterion::{Criterion, criterion_group, criterion_main};

use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::atlas::{AtlasConfig, BuiyAtlas, maintain_atlas};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::extract::{
    ExtractedEffectGroups, ExtractedNodesView, ExtractedTextQuads, extract_buiy_nodes,
};
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::text::{
    BuiySwashCache, BuiyTextPlugin, FontKeyInterner, FontSize, GlyphMetaCache, ResidentTextKeys,
    SharedFontSystem, Text, extract_buiy_glyphs,
};
use buiy_core::{CorePlugin, Node};

/// The adapterless shape→layout→extract harness — a bench-local twin of
/// `tests/support/extract_harness.rs::TextExtractHarness`. The main `App`
/// carries the full headless text + layout pipeline (no `RenderPlugin`); the
/// render `World` carries only the CPU resources `extract_buiy_glyphs` reads.
struct PipelineHarness {
    app: App,
    render: World,
    extract: Schedule,
}

impl PipelineHarness {
    fn new() -> Self {
        let mut app = App::new();
        // BuiyRenderPlugin's MAIN-world half registers headless; its render half
        // is guarded on a RenderApp that never exists here (no adapter).
        app.add_plugins(MinimalPlugins)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(CorePlugin)
            .add_plugins(LayoutPlugin)
            .add_plugins(BuiyTextPlugin::default())
            .add_plugins(BuiyRenderPlugin);
        // Synthetic primary window — the glyph producer reads scale/presence
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
        render.init_resource::<FontKeyInterner>();
        render.init_resource::<ResidentTextKeys>();
        render.init_resource::<GlyphMetaCache>();
        render.init_resource::<BuiySwashCache>();
        render.insert_resource(fonts);
        render.init_resource::<MainWorld>();

        let mut extract = Schedule::new(ExtractSchedule);
        extract.add_systems((maintain_atlas, extract_buiy_glyphs, extract_buiy_nodes).chain());

        Self {
            app,
            render,
            extract,
        }
    }

    /// One full pipeline pass: main-world Update (TextSync → measure/shape →
    /// TextCommit → layout) then the extract step against the live main world.
    fn frame(&mut self) {
        self.app.update();
        self.extract_only();
    }

    /// The extract step alone — swap the live main world into the render world's
    /// `MainWorld` slot, run `ExtractSchedule`, swap it back (bevy_render's own
    /// extract dance, minus the renderer).
    fn extract_only(&mut self) {
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

    fn glyph_count(&self) -> usize {
        self.render.resource::<ExtractedGlyphs>().glyphs.len()
    }
}

/// A representative large UI: `paragraphs` wrapped text blocks (a heading + a
/// long body each) in a padded flex column — hundreds of nodes and a multi-
/// thousand-glyph shaping workload, the shape that stresses the per-frame hot
/// path. Returns the populated, NOT-yet-updated harness.
fn build_large_scene(paragraphs: usize) -> PipelineHarness {
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

/// Bench the COLD first-pass: spawn the large scene then drive frames to
/// quiescence from scratch — the full shape→layout→extract cost of bringing a
/// fresh scene up (every paragraph shaped, laid out, and extracted for the first
/// time). Re-built per iteration so each sample is a genuine cold pass.
fn bench_cold_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("shape_layout_extract");
    // A bigger scene amortizes per-frame fixed costs into the shaping/layout/
    // extract work we care about; one sample is several `app.update()`s, so keep
    // the sample count modest (criterion still collects a distribution).
    group.sample_size(20);

    for &paragraphs in &[64usize, 256] {
        group.bench_function(format!("cold/{paragraphs}_paragraphs"), |b| {
            b.iter(|| {
                let mut h = build_large_scene(paragraphs);
                // Drive a small fixed number of frames: the spawn-frame shape +
                // layout + extract, plus the steady tail that confirms the cache
                // holds (the cosmic-text reshape echo settles within a few
                // frames — the latency-test convergence shape).
                for _ in 0..4 {
                    h.frame();
                }
                black_box(h.glyph_count())
            });
        });
    }
    group.finish();
}

/// Bench the per-frame STEADY hot path: settle a large scene ONCE, then time a
/// single full pipeline frame on the already-shaped scene. This isolates the
/// recurring per-frame cost (the path that runs every frame of a live app) from
/// the one-time cold build — an O(n²) regression in the steady extract / layout
/// pass shows up cleanly here.
fn bench_steady_pipeline(c: &mut Criterion) {
    let mut group = c.benchmark_group("shape_layout_extract");

    for &paragraphs in &[64usize, 256] {
        group.bench_function(format!("steady/{paragraphs}_paragraphs"), |b| {
            let mut h = build_large_scene(paragraphs);
            // Settle the scene so shaping/layout caches are warm before timing.
            for _ in 0..6 {
                h.frame();
            }
            b.iter(|| {
                h.frame();
                black_box(h.glyph_count())
            });
        });
    }
    group.finish();
}

/// A flat, TEXT-FREE scene: `nodes` fixed-size plain `Node`s in a padded flex
/// column. No `Text`, so the per-frame cost is dominated by the LAYOUT pipeline
/// — the Taffy compute plus the ~12 post-Taffy full-tree passes (stacking,
/// transform composition, `ResolvedLayout` readback, writing-mode inherit, the
/// positioning sub-passes) that run UNCONDITIONALLY every frame with no run
/// condition (perf audit #3). Isolates #3 from the text/atlas cost (#5).
fn build_flat_scene(nodes: usize) -> PipelineHarness {
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

/// Bench the per-frame STEADY layout cost on a large flat scene: settle once,
/// then time a single `app.update()` (the full BuiySet chain) on the
/// already-laid-out tree. An ungated full-tree pass (#3) shows up here as a
/// per-frame cost that grows with node count even though nothing changed — the
/// "static frame is not free" thesis, isolated from text.
fn bench_flat_steady(c: &mut Criterion) {
    let mut group = c.benchmark_group("layout_flat");
    for &nodes in &[1000usize, 5000] {
        group.bench_function(format!("steady/{nodes}_nodes"), |b| {
            let mut h = build_flat_scene(nodes);
            for _ in 0..6 {
                h.app.update();
            }
            b.iter(|| {
                h.app.update();
                black_box(h.app.world().entities().len())
            });
        });
    }
    group.finish();
}

/// A flat scene of `nodes` painting `Background` nodes for the audit-#2
/// node-extract measurement. Each node carries a `Background`, so
/// `extract_buiy_nodes` builds a per-entity `ExtractedNode` record for ALL N on
/// any dirty frame. Returns the harness + one victim entity to mutate.
fn build_flat_bg_scene(nodes: usize) -> (PipelineHarness, Entity) {
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
                    color: ColorToken::Token("surface".into()),
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

/// Bench audit #2 (all-or-nothing extract). `extract_buiy_nodes` is gated: a
/// CLEAN frame early-returns (the O(0) damage gate); ANY changed entity rebuilds
/// the FULL `by_entity` record set + re-clones every ~480 B `ExtractedNode`. So
/// `steady_gated` (no change → gate skips) vs `one_change` (mutate ONE node →
/// full N-node rebuild) sizes the CPU cost a single hover/caret-blink/scroll
/// pays per interactive frame. Both run `app.update()` (the layout pipeline), so
/// the DELTA isolates the extract rebuild. Adapterless: no RenderApp/GPU, so the
/// prepare re-upload (audit #2's other half) is NOT measured here.
fn bench_node_extract(c: &mut Criterion) {
    let mut group = c.benchmark_group("node_extract");
    for &nodes in &[1000usize, 5000] {
        group.bench_function(format!("steady_gated/{nodes}"), |b| {
            let (mut h, _victim) = build_flat_bg_scene(nodes);
            for _ in 0..6 {
                h.frame();
            }
            b.iter(|| {
                h.app.update();
                h.extract_only();
                black_box(h.app.world().entities().len())
            });
        });
        group.bench_function(format!("one_change/{nodes}"), |b| {
            let (mut h, victim) = build_flat_bg_scene(nodes);
            for _ in 0..6 {
                h.frame();
            }
            b.iter(|| {
                h.app.update();
                // One interactive change: mark a single node's paint dirty so the
                // damage gate trips and re-extracts the whole scene.
                if let Some(mut bg) = h.app.world_mut().get_mut::<Background>(victim) {
                    bg.set_changed();
                }
                h.extract_only();
                black_box(h.app.world().entities().len())
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_cold_pipeline,
    bench_steady_pipeline,
    bench_flat_steady,
    bench_node_extract
);
criterion_main!(benches);
