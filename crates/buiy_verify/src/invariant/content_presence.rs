//! `content_is_present` (C7 §2.4): the production extract path MUST emit more
//! than zero glyph instances for a text-bearing scene. Text-bearing is inferred
//! from `Text` / `TextEditState` presence (§3.4); the placeholder edge is
//! handled by reading `PlaceholderActive` (an active placeholder is positive,
//! an inactive empty editor is legal at 0 glyphs).
//!
//! Runs the production `extract_buiy_glyphs` adapterless via the same
//! main-world↔MainWorld swap `TextExtractHarness` uses (extract_harness.rs).
//! The CALLER owns the `App` and must build it on a text-capable stack
//! (`BuiyTextPlugin` seeds `SharedFontSystem`) and `update()` it once before
//! calling, so TextSync → measure → commit have shaped the buffers.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};

use buiy_core::render::atlas::{AtlasConfig, BuiyAtlas, maintain_atlas};
use buiy_core::render::extract::ExtractedTextQuads;
use buiy_core::render::prepare::ExtractedGlyphs;
use buiy_core::text::edit::{PlaceholderActive, TextEditState};
use buiy_core::text::{
    BuiySwashCache, FontKeyInterner, GlyphMetaCache, ResidentTextKeys, SharedFontSystem, Text,
    extract_buiy_glyphs,
};

use super::predicates::Violation;

/// Is `app`'s spawned scene text-bearing for the content-presence guard?
/// A `Text` component, or an editor with an ACTIVE placeholder, or a
/// non-empty editor value, counts; an empty editor with no active placeholder
/// does not (the legal 0-glyph case, §3.4).
fn scene_is_text_bearing(world: &mut World) -> bool {
    if let Some(mut q) = world.try_query::<&Text>()
        && q.iter(world).next().is_some()
    {
        return true;
    }
    if let Some(mut q) = world.try_query::<(&TextEditState, Option<&PlaceholderActive>)>() {
        let mut bearing = false;
        for (state, active) in q.iter(world) {
            if active.is_some() || !state.value().is_empty() {
                bearing = true;
                break;
            }
        }
        return bearing;
    }
    false
}

/// Run the production glyph producer over `app`'s live (already-updated)
/// scene, adapterless, and return `(text_bearing, glyph_count)`. The honest,
/// non-`unsafe` form: the caller owns the `App`, so we borrow its world via
/// `world_mut()` and swap it through `MainWorld`. The bless-guard
/// (`golden::bless_guard_check`) reuses this exact pair, so the invariant and
/// the bless refusal never diverge.
pub fn glyph_census(app: &mut App) -> (bool, usize) {
    // The shared font engine the producer shapes against (seeded by
    // `BuiyTextPlugin` in the caller's stack). Clone the Arc handle.
    let fonts = app.world().resource::<SharedFontSystem>().clone();

    // The bare extract world the producer touches — exactly the resources
    // `TextExtractHarness::with_atlas_config` seeds (extract_harness.rs:88-100).
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

    let mut schedule = Schedule::new(ExtractSchedule);
    schedule.add_systems((maintain_atlas, extract_buiy_glyphs).chain());

    let text_bearing = scene_is_text_bearing(app.world_mut());

    // The bevy_render extract dance (extract_harness.rs:130-140): swap the
    // live main world into MainWorld, run the schedule, swap it back.
    {
        let mut main = render.resource_mut::<MainWorld>();
        core::mem::swap(&mut **main, app.world_mut());
    }
    schedule.run(&mut render);
    {
        let mut main = render.resource_mut::<MainWorld>();
        core::mem::swap(&mut **main, app.world_mut());
    }

    (
        text_bearing,
        render.resource::<ExtractedGlyphs>().glyphs.len(),
    )
}

/// Assert > 0 glyph instances for a text-bearing scene (the silent-no-paint
/// guard). A non-text scene, or a text scene that shaped some glyphs, is ok.
pub fn content_is_present(app: &mut App) -> Result<(), Violation> {
    let (text_bearing, glyphs) = glyph_census(app);
    if text_bearing && glyphs == 0 {
        return Err(Violation::new(
            "content_is_present",
            "a text-bearing scene emitted 0 glyph instances (silent-no-paint)",
        ));
    }
    Ok(())
}
