//! End-to-end verification fixture for the `hello_text` example scene (audit
//! finding #42). Exercises the full Buiy TEXT stack HEADLESS — no wgpu adapter,
//! no window — against the example's scene:
//!   - the scene builds + drives `app.update()` without panicking
//!   - the text pipeline runs: TextSync → Taffy measure (cosmic-text shaping)
//!     → TextCommit lays the glyphs out (`ComputedTextLayout` populated)
//!   - the GLYPH PRODUCER emits: `extract_buiy_glyphs` publishes a non-empty
//!     `ResidentTextKeys` / `ExtractedGlyphs` — the headless end-to-end of the
//!     "shape → layout → extract" path the visible example walks.
//!
//! `examples/hello_text` itself runs `DefaultPlugins` (a real window + renderer),
//! which cannot run headlessly in CI. So this fixture replicates the example's
//! SCENE construction (a 32px title above a 16px wrapped body in a 560px padded
//! column — kept in sync with `examples/hello_text/src/main.rs`) on the
//! adapterless extract harness, the same substrate the `buiy_core`
//! `text_*_latency` tests use. The example's `src/main.rs` is the human-eyes
//! smoke (`cargo run -p hello_text`); this is its automated, panic-free twin.
//!
//! Coverage NOT in this file:
//! - GPU rasterization of those glyphs (atlas upload, coverage draw): the GPU
//!   `#[ignore]` lane in `crates/buiy_core/tests/*_gpu.rs` + the `capture`
//!   example e2e (`tests/capture_e2e.rs`). This fixture stops at the producer
//!   (CPU-observable glyph publish), by design — it is the headless half.
//! - Visual/golden correctness of the laid-out text: `buiy_verify` Tier-5
//!   goldens (the rasterization residue), not a panic-free smoke.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::atlas::{AtlasConfig, BuiyAtlas, maintain_atlas};
use buiy_core::render::extract::ExtractedTextQuads;
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::text::{
    BuiySwashCache, BuiyTextPlugin, ComputedTextLayout, FontKeyInterner, FontSize, GlyphMetaCache,
    ResidentTextKeys, SharedFontSystem, Text, extract_buiy_glyphs,
};
use buiy_core::{CorePlugin, Node};

/// The adapterless shape→layout→extract harness — a fixture-local twin of
/// `crates/buiy_core/tests/support/extract_harness.rs::TextExtractHarness` (that
/// module is compiled only for the `buiy_core` integration-test crate, so the
/// workspace `tests/` crate cannot `mod` it). Built from the SAME public
/// `buiy_core` surface so it drives the real production systems, not a
/// reimplementation. The main `App` carries the full headless text + layout
/// pipeline (NO `RenderPlugin`); the render `World` carries only the CPU
/// resources `extract_buiy_glyphs` reads — prepare/queue/draw never exist, so
/// nothing requests a GPU.
struct TextStackHarness {
    app: App,
    render: World,
    extract: Schedule,
}

impl TextStackHarness {
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
        // through a Query, mirroring `examples/hello_text`'s real window.
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(620, 300),
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
        // Swap the live main world into the render world's `MainWorld` slot, run
        // `ExtractSchedule`, swap it back — bevy_render's own extract dance.
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

    fn resident_key_count(&self) -> usize {
        self.render.resource::<ResidentTextKeys>().keys.len()
    }
}

/// Build the `examples/hello_text` scene: a 32px title above a 16px wrapped body
/// in a 560px padded flex column. Kept in sync with
/// `examples/hello_text/src/main.rs`'s `setup` (the strings + sizes match so this
/// fixture exercises the same shaping workload the visible example does).
fn build_hello_text_scene(h: &mut TextStackHarness) -> Entity {
    let title = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hello, Buiy text!")),
            FontSize(32.0),
        ))
        .id();
    let body = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "The quick brown fox jumps over the lazy dog. Shaped by \
                 cosmic-text at the committed wrap width, rasterized once per \
                 (font, size, weight, subpixel-bin) into the shared coverage \
                 atlas, tinted per instance — a theme switch never touches \
                 the atlas.",
            )),
            FontSize(16.0),
        ))
        .id();
    h.app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(560.0)
                .padding(24.0)
                .gap_px(12.0),
        ))
        .add_children(&[title, body])
        .id()
}

#[test]
fn e2e_hello_text_scene_publishes_glyphs() {
    let mut h = TextStackHarness::new();
    let title = build_hello_text_scene(&mut h);
    assert!(title != Entity::PLACEHOLDER, "scene root spawned");

    // Drive the pipeline to quiescence: the spawn-frame shape + layout + first
    // extract, plus the cosmic-text reshape echo (the latency-test convergence
    // shape settles within a few frames). No panic across these = the headless
    // text stack runs end-to-end.
    for _ in 0..6 {
        h.frame();
    }

    // The producer EMITTED: shaping + layout + extract published real glyphs.
    // Both signals the audit names ("ResidentTextKeys non-empty / the text
    // producer emitted") are asserted — the headless end-to-end of the stack.
    assert!(
        h.glyph_count() > 0,
        "the hello_text scene must publish glyphs through shape → layout → \
         extract (ExtractedGlyphs is empty — the text producer emitted nothing)"
    );
    assert!(
        h.resident_key_count() > 0,
        "the producer must register resident atlas keys for the published \
         glyphs (ResidentTextKeys is empty)"
    );
}

#[test]
fn e2e_hello_text_lays_out_both_text_nodes() {
    let mut h = TextStackHarness::new();
    build_hello_text_scene(&mut h);
    for _ in 0..6 {
        h.frame();
    }

    // Both the title and the body shaped into laid-out lines — the TextCommit
    // half of the stack (distinct from the extract producer above). The body
    // wraps at the 560px column, so it must produce more than one line.
    let mut laid_out: Vec<ComputedTextLayout> = h
        .app
        .world_mut()
        .query::<&ComputedTextLayout>()
        .iter(h.app.world())
        .cloned()
        .collect();
    laid_out.sort_by_key(|l| l.lines.len());

    assert_eq!(
        laid_out.len(),
        2,
        "both text nodes (title + body) must have a ComputedTextLayout"
    );
    assert!(
        laid_out.iter().all(|l| !l.lines.is_empty()),
        "every text node must shape into at least one laid-out line"
    );
    assert!(
        laid_out.last().unwrap().lines.len() > 1,
        "the long body must wrap into multiple lines at the 560px column width"
    );
}
