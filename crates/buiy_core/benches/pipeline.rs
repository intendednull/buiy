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
use buiy_core::render::extract::ExtractedTextQuads;
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
        render.init_resource::<FontKeyInterner>();
        render.init_resource::<ResidentTextKeys>();
        render.init_resource::<GlyphMetaCache>();
        render.init_resource::<BuiySwashCache>();
        render.insert_resource(fonts);
        render.init_resource::<MainWorld>();

        let mut extract = Schedule::new(ExtractSchedule);
        extract.add_systems((maintain_atlas, extract_buiy_glyphs).chain());

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

criterion_group!(benches, bench_cold_pipeline, bench_steady_pipeline);
criterion_main!(benches);
