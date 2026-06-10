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
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::text::{
    BuiySwashCache, BuiyTextPlugin, FontKeyInterner, GlyphMetaCache, ResidentTextKeys,
    SharedFontSystem, extract_buiy_glyphs,
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

        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut render = World::new();
        render.insert_resource(BuiyAtlas::new(config));
        render.init_resource::<ExtractedGlyphs>();
        render.init_resource::<FontKeyInterner>();
        render.init_resource::<ResidentTextKeys>();
        render.init_resource::<GlyphMetaCache>();
        render.init_resource::<BuiySwashCache>();
        render.insert_resource(fonts);
        render.init_resource::<GlyphChangeLog>();
        // The slot the live main world is swapped into per extract step.
        render.init_resource::<MainWorld>();

        // Mirror the real chain: maintenance advances the frame clock, then
        // the producer (.after(maintain_atlas)), then the change probe.
        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems((maintain_atlas, extract_buiy_glyphs, log_glyph_changes).chain());

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

    pub fn resident_keys(&self) -> Vec<AtlasKey> {
        self.render.resource::<ResidentTextKeys>().keys.clone()
    }

    pub fn atlas(&self) -> &BuiyAtlas {
        self.render.resource::<BuiyAtlas>()
    }
}
