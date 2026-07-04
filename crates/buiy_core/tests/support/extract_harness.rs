//! The adapterless extract harness (text verification.md § 1.2): drives
//! `extract_buiy_glyphs` with NO wgpu adapter. The main `App` is built
//! WITHOUT `RenderPlugin`; the render side is a bare `World` carrying only
//! the CPU-side resources the producer touches (`BuiyAtlas` is device-free
//! by design). Each step swaps the main world in as `MainWorld`, runs a
//! manually built `ExtractSchedule`, and swaps it back — bevy_render's own
//! `extract()` dance (lib.rs:453–463), minus the renderer. Prepare/queue/
//! draw never exist, so nothing requests an adapter.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::CorePlugin;
use buiy_core::layout::LayoutPlugin;
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::atlas::{AtlasConfig, AtlasKey, BuiyAtlas, maintain_atlas};
use buiy_core::render::extract::ExtractedTextQuads;
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::text::{
    BuiySwashCache, BuiyTextPlugin, FontKeyInterner, FontsGeneration, GlyphDamage, GlyphMetaCache,
    ResidentTextKeys, SharedFontSystem, extract_buiy_glyphs,
};

/// Mirrors `prepare_buiy_instances`' glyph gate: counts frames on which
/// `ExtractedGlyphs` was rebuilt (`is_changed()` relative to this system's
/// last run — exactly the prepare semantics).
#[derive(Resource, Default)]
pub struct GlyphChangeLog {
    pub frames: usize,
    pub changed_frames: usize,
}

fn log_glyph_changes(glyphs: Res<ExtractedGlyphs>, mut log: ResMut<GlyphChangeLog>) {
    log.frames += 1;
    if glyphs.is_changed() {
        log.changed_frames += 1;
    }
}

/// The quad-carrier mirror of [`GlyphChangeLog`]: counts frames on which
/// `ExtractedTextQuads` was rebuilt — the third quad-gate term in
/// `prepare_buiy_instances` (T6, decoration-and-paint § 4.6).
#[derive(Resource, Default)]
pub struct TextQuadChangeLog {
    pub changed_frames: usize,
}

fn log_text_quad_changes(quads: Res<ExtractedTextQuads>, mut log: ResMut<TextQuadChangeLog>) {
    if quads.is_changed() {
        log.changed_frames += 1;
    }
}

pub struct TextExtractHarness {
    pub app: App,
    pub render: World,
    schedule: Schedule,
}

impl TextExtractHarness {
    pub fn new() -> Self {
        Self::with_atlas_config(AtlasConfig::default())
    }

    pub fn with_atlas_config(config: AtlasConfig) -> Self {
        let mut app = App::new();
        // BuiyRenderPlugin's MAIN-world half (clip rects, paint-skip,
        // effect groups, forced colors) registers headless — its render
        // half is guarded on a RenderApp that never exists here.
        app.add_plugins(MinimalPlugins)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(CorePlugin)
            .add_plugins(LayoutPlugin)
            .add_plugins(BuiyTextPlugin::default())
            .add_plugins(BuiyRenderPlugin);
        // Synthetic primary window (the layout_container_queries idiom):
        // component-only — the producer reads scale/presence via a Query.
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            PrimaryWindow,
        ));

        // Determinism (MT-safety): pause the virtual clock so test elapsed is
        // driven ONLY by explicit `advance_by`, never by absorbed wall-clock
        // deltas. An UNpaused `Time<Virtual>` also advances by each frame's real
        // delta (clamped to 250ms); under heavy load — e.g. the multi_threaded CI
        // lane running the whole workspace concurrently — `app.update()` stalls
        // inflate that delta, so cumulative elapsed silently crosses blink/anim
        // thresholds mid-test and flakes work-count assertions (observed:
        // text_caret_selection blink-edge glyph-rebuild counts, 2 vs 1). Pausing
        // matches the suite's existing paused-clock blink tests. See
        // docs/specs/2026-06-30-mt-safety-design.md and the 2026-06-30 MT audit.
        app.world_mut().resource_mut::<Time<Virtual>>().pause();

        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut render = World::new();
        render.insert_resource(BuiyAtlas::new(config));
        render.init_resource::<ExtractedGlyphs>();
        render.init_resource::<ExtractedTextQuads>();
        render.init_resource::<FontKeyInterner>();
        render.init_resource::<ResidentTextKeys>();
        render.init_resource::<GlyphMetaCache>();
        render.init_resource::<BuiySwashCache>();
        render.insert_resource(fonts);
        render.init_resource::<GlyphChangeLog>();
        render.init_resource::<TextQuadChangeLog>();
        // Stage B (glyph partial-reextract D1): the producer's Full|Patch
        // verdict, so the § 12 damage tests can assert the classifier.
        render.init_resource::<GlyphDamage>();
        // The slot the live main world is swapped into per extract step.
        render.init_resource::<MainWorld>();

        // Mirror the real chain: maintenance advances the frame clock, then
        // the producer (.after(maintain_atlas)), then the change probes.
        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems(
            (
                maintain_atlas,
                extract_buiy_glyphs,
                log_glyph_changes,
                log_text_quad_changes,
            )
                .chain(),
        );

        Self {
            app,
            render,
            schedule,
        }
    }

    /// One full frame: main-world Update (TextSync → measure → TextCommit),
    /// then the extract step against the live main world.
    pub fn frame(&mut self) {
        self.app.update();
        self.extract_only();
    }

    /// The extract step alone (no main-world Update).
    pub fn extract_only(&mut self) {
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
        self.schedule.run(&mut self.render);
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
    }

    /// Three frames: spawn-settle (TextSync insert + first commit + first
    /// extract rebuild all land within these).
    pub fn settle(&mut self) {
        for _ in 0..3 {
            self.frame();
        }
    }

    pub fn glyphs(&self) -> &ExtractedGlyphs {
        self.render.resource::<ExtractedGlyphs>()
    }

    pub fn glyph_count(&self) -> usize {
        self.glyphs().glyphs.len()
    }

    pub fn changed_frames(&self) -> usize {
        self.render.resource::<GlyphChangeLog>().changed_frames
    }

    pub fn text_quads(&self) -> &ExtractedTextQuads {
        self.render.resource::<ExtractedTextQuads>()
    }

    pub fn quad_changed_frames(&self) -> usize {
        self.render.resource::<TextQuadChangeLog>().changed_frames
    }

    pub fn resident_keys(&self) -> Vec<AtlasKey> {
        self.render.resource::<ResidentTextKeys>().keys.clone()
    }

    /// The Stage B classifier verdict the producer published on the last
    /// dirty frame (partial-reextract D1; `Full` until the first rebuild).
    pub fn glyph_damage(&self) -> GlyphDamage {
        self.render.resource::<GlyphDamage>().clone()
    }

    pub fn atlas(&self) -> &BuiyAtlas {
        self.render.resource::<BuiyAtlas>()
    }

    /// Inject the `FontsGeneration` bump deterministically: increment the
    /// `FontsGeneration` resource the way `apply_font_registry`
    /// (registry.rs:543) does on a runtime add_font, then run one frame. This
    /// is the trigger for Bugs 2/3 (the all-buffers `TextSync` sweep fires on
    /// EVERY runtime add_font, not just startup — audit Bug 3). `TextSync` keys
    /// its sweep on `fonts_generation.is_changed() && !is_added()`
    /// (sync.rs:251); writing through `resource_mut` sets the `Changed` tick, so
    /// the sweep fires next frame — the same path the real `apply_font_registry`
    /// bump triggers. C2 must confirm this reproduces the clobber identically to
    /// the async loader path (C7 §3.3 hand-off).
    pub fn bump_fonts_generation(&mut self) -> &mut Self {
        {
            let mut generation = self.app.world_mut().resource_mut::<FontsGeneration>();
            generation.0 += 1;
        }
        self.frame();
        self
    }
}
