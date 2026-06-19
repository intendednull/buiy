//! font-display (font-assets § 7): Swap renders the next resolved family
//! immediately (FOUT — load completion reshapes via the generation bump);
//! Block keeps the IDENTICAL fallback layout but paints zero-alpha until
//! load or the 3 s timeout. Loading states are driven headless — via
//! `reserve_handle` where the asset machinery exists (no async IO), via a
//! never-loading `uuid_handle!` where it doesn't; the alpha side runs
//! through the adapterless extract harness.

use std::sync::Arc;
use std::time::Duration;

use crate::support::extract_harness::TextExtractHarness;
use bevy::asset::{AssetPlugin, Assets, uuid_handle};
use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyFont, BuiyTextPlugin, ComputedTextLayout, FamilyEntry, FontDisplay, FontFaceDescriptors,
    FontFamily, FontLoadState, FontRegistry, FontSize, FontStack, FontsGeneration, GenericFamily,
    PendingFontBlock, Text, TextBuffer,
};
use cosmic_text::Family;

fn fira_bytes() -> Arc<Vec<u8>> {
    Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .unwrap(),
    )
}

/// The declared family under test + the generic the resolver falls to while
/// it loads (the FOUT/Block fallback face — the serif pin).
fn pending_stack(family: &str) -> FontStack {
    FontStack(vec![
        FamilyEntry::Named(String::from(family)),
        FamilyEntry::Generic(GenericFamily::Serif),
    ])
}

fn block_descriptors() -> FontFaceDescriptors {
    FontFaceDescriptors {
        font_display: FontDisplay::Block,
        ..Default::default()
    }
}

/// The asset-backed sibling of [`crate::support::headless_text_app`] (the
/// text_registry.rs asset-path shape): `MinimalPlugins + ThemePlugin +
/// AssetPlugin + CorePlugin + LayoutPlugin + BuiyTextPlugin`. Loading is driven
/// via `reserve_handle`, completion via `Assets::insert`.
///
/// It does NOT call `crate::support::headless_text_app()` because of a hard plugin
/// ordering: `BuiyTextPlugin::build` calls `init_asset::<BuiyFont>()` only when
/// an `AssetServer` already exists (text/mod.rs § T5 — the headless text
/// fixtures carry no `AssetPlugin`), so `AssetPlugin` MUST be added BEFORE
/// `BuiyTextPlugin`. `headless_text_app()` adds `BuiyTextPlugin` with no
/// `AssetPlugin`, so layering `AssetPlugin` on top of it afterward would leave
/// `Assets<BuiyFont>` uninitialized and `reserve_handle()` would panic. The
/// plugin SET is otherwise the shared text stack (ThemePlugin included, matching
/// the shared builder); only the asset prelude differs.
fn asset_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::theme::ThemePlugin);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update(); // settle plugin init (generation is_added frame)
    app
}

// The shared condition-polled `crate::support::settle` (#35): converges the
// layout-and-text pipeline by polling geometry + text-shaping quiescence
// (`TextSyncAppliedCount`/`TextCommitReshapeCount`/`FontsGeneration`). This file
// is the genuine 3-update font-reshape case — a load completion bumps
// `FontsGeneration`, which re-lays shaped glyphs WITHOUT moving box geometry, so
// the widened signal (not a geometry-only poll) is what makes this converge on
// the right frame.
use crate::support::settle;

fn generation(app: &App) -> u64 {
    app.world().resource::<FontsGeneration>().0
}

fn spawn_text(app: &mut App, text: &str, stack: FontStack) -> Entity {
    app.world_mut()
        .spawn((
            buiy_core::Node,
            Style::default().width_px(300.0).height_px(60.0),
            Text(String::from(text)),
            FontFamily(stack),
        ))
        .id()
}

/// The entity's lowered base family. Single-span resolution writes it via
/// `set_text` — the line's `AttrsList` defaults — so this distinguishes the
/// Loading-time generic fallback (`Family::Serif`) from the post-load
/// `Family::Name` win without relying on face IDs (the embedded face shares
/// the "Fira Sans" name — the text_resolver.rs tie note).
fn lowered_family(app: &App, entity: Entity) -> Family<'_> {
    app.world()
        .get::<TextBuffer>(entity)
        .expect("text entity has a buffer")
        .buffer
        .lines[0]
        .attrs_list()
        .defaults()
        .family
}

fn glyph_count(app: &App, entity: Entity) -> usize {
    app.world()
        .get::<TextBuffer>(entity)
        .expect("text entity has a buffer")
        .buffer
        .layout_runs()
        .map(|run| run.glyphs.len())
        .sum()
}

// --- the harness side (instance alpha) -------------------------------------

/// A handle that can never load: the harness app carries no asset machinery
/// (no `Assets<BuiyFont>`), so a record registered against it stays
/// `Loading` forever — exactly the Block-window driver, no async IO.
const NEVER_LOADS: Handle<BuiyFont> = uuid_handle!("4f2f9c1e-6b3a-4d8e-9c5f-7a1b2c3d4e5f");

fn register_pending(harness: &mut TextExtractHarness, family: &str, display: FontDisplay) {
    harness
        .app
        .world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset(
            family,
            NEVER_LOADS.clone(),
            FontFaceDescriptors {
                font_display: display,
                ..Default::default()
            },
        );
}

/// The text_extract.rs fixture shape: "Hi!" under a sized column root —
/// 3 non-whitespace glyphs.
fn spawn_harness_text(harness: &mut TextExtractHarness, stack: FontStack) -> Entity {
    let text = harness
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            Style::default(),
            Text(String::from("Hi!")),
            FontSize(16.0),
            FontFamily(stack),
        ))
        .id();
    harness
        .app
        .world_mut()
        .spawn((
            buiy_core::Node,
            Style::default()
                .flex_column()
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text);
    text
}

// ---------------------------------------------------------------------------

#[test]
fn swap_renders_fallback_while_loading_and_reshapes_once_on_load() {
    let mut app = asset_app();
    let handle = app.world().resource::<Assets<BuiyFont>>().reserve_handle();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset("Fira Sans", handle.clone(), FontFaceDescriptors::default());
    let entity = spawn_text(&mut app, "hello", pending_stack("Fira Sans"));
    settle(&mut app);

    // Loading: the resolver skipped the Loading family — the entity shaped
    // IMMEDIATELY against the generic serif pin (FOUT's first paint), with
    // no Block marker (Swap is the default).
    assert_eq!(
        app.world()
            .resource::<FontRegistry>()
            .load_state("Fira Sans"),
        Some(FontLoadState::Loading)
    );
    assert!(
        app.world().get::<PendingFontBlock>(entity).is_none(),
        "Swap never blocks"
    );
    assert_eq!(lowered_family(&app, entity), Family::Serif);
    assert!(
        glyph_count(&app, entity) > 0,
        "fallback glyphs shaped while loading"
    );

    // The asset arrives: exactly ONE generation bump, and the sweep
    // reshapes the entity onto the now-Loaded registered family.
    let gen_before = generation(&app);
    let id = handle.id();
    app.world_mut()
        .resource_mut::<Assets<BuiyFont>>()
        .insert(id, BuiyFont { data: fira_bytes() })
        .unwrap();
    settle(&mut app);

    assert_eq!(
        app.world()
            .resource::<FontRegistry>()
            .load_state("Fira Sans"),
        Some(FontLoadState::Loaded)
    );
    assert_eq!(
        generation(&app),
        gen_before + 1,
        "exactly one bump per load"
    );
    assert_eq!(
        lowered_family(&app, entity),
        Family::Name("Fira Sans"),
        "load completion reshaped onto the registered family (FOUT's swap)"
    );
}

#[test]
fn block_layout_is_the_fallback_layout_and_paint_is_zero_alpha() {
    // The Swap reference: same stack, same content, Swap display.
    let mut swap = TextExtractHarness::new();
    register_pending(&mut swap, "Pending Sans", FontDisplay::Swap);
    let swap_entity = spawn_harness_text(&mut swap, pending_stack("Pending Sans"));
    swap.settle();
    let swap_layout = swap
        .app
        .world()
        .get::<ComputedTextLayout>(swap_entity)
        .unwrap()
        .clone();
    assert!(swap.glyph_count() > 0);
    assert!(
        swap.glyphs().glyphs.iter().all(|g| g.color[3] > 0.0),
        "Swap paints the fallback at full alpha"
    );

    // Block: IDENTICAL fallback layout, marker present, zero-alpha emission.
    let mut block = TextExtractHarness::new();
    register_pending(&mut block, "Pending Sans", FontDisplay::Block);
    let block_entity = spawn_harness_text(&mut block, pending_stack("Pending Sans"));
    block.settle();
    assert_eq!(
        block
            .app
            .world()
            .get::<ComputedTextLayout>(block_entity)
            .unwrap(),
        &swap_layout,
        "geometry never jumps twice: Block lays out exactly like Swap (§ 7)"
    );
    assert!(
        block
            .app
            .world()
            .get::<PendingFontBlock>(block_entity)
            .is_some(),
        "TextSync marked the entity while the Block family loads"
    );
    assert!(
        block.glyph_count() > 0,
        "instances ARE emitted (the zero-alpha skip is bypassed; the atlas \
         stays warm with the fallback's glyphs)"
    );
    assert!(
        block.glyphs().glyphs.iter().all(|g| g.color[3] == 0.0),
        "every blocked instance paints zero-alpha"
    );
}

#[test]
fn block_clears_on_load_with_one_bump() {
    let mut app = asset_app();
    let handle = app.world().resource::<Assets<BuiyFont>>().reserve_handle();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset("Fira Sans", handle.clone(), block_descriptors());
    let entity = spawn_text(&mut app, "hello", pending_stack("Fira Sans"));
    settle(&mut app);
    assert!(
        app.world().get::<PendingFontBlock>(entity).is_some(),
        "blocked while loading"
    );

    let gen_before = generation(&app);
    let id = handle.id();
    app.world_mut()
        .resource_mut::<Assets<BuiyFont>>()
        .insert(id, BuiyFont { data: fira_bytes() })
        .unwrap();
    settle(&mut app);

    // Alpha restoration on marker removal is the producer's removal-probe
    // arm — pinned on the harness in the timeout test below.
    assert!(
        app.world().get::<PendingFontBlock>(entity).is_none(),
        "the load lifted the block"
    );
    assert_eq!(
        generation(&app),
        gen_before + 1,
        "load + unblock composed under ONE bump"
    );
    assert_eq!(
        lowered_family(&app, entity),
        Family::Name("Fira Sans"),
        "glyphs now shape against the loaded family"
    );
}

#[test]
fn block_times_out_to_swap_after_three_seconds() {
    // Paint side (harness): expiry removes the marker and restores alpha.
    let mut harness = TextExtractHarness::new();
    register_pending(&mut harness, "Pending Sans", FontDisplay::Block);
    let entity = spawn_harness_text(&mut harness, pending_stack("Pending Sans"));
    harness.settle();
    assert!(
        harness
            .app
            .world()
            .get::<PendingFontBlock>(entity)
            .is_some()
    );
    assert!(harness.glyphs().glyphs.iter().all(|g| g.color[3] == 0.0));

    // The stepped-clock discipline — no sleeps: advance past
    // FONT_BLOCK_TIMEOUT_SECS and let expire_font_block run.
    harness
        .app
        .world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_secs_f32(3.5));
    harness.frame();

    assert!(
        harness
            .app
            .world()
            .get::<PendingFontBlock>(entity)
            .is_none(),
        "expire_font_block removed the expired marker"
    );
    assert!(
        harness.glyph_count() > 0 && harness.glyphs().glyphs.iter().all(|g| g.color[3] > 0.0),
        "fallback now PAINTS — the § 7 'then swap' arm (removal probe repaint)"
    );

    // State side (asset app): a LATER load still swaps the face via the
    // normal generation path.
    let mut app = asset_app();
    let handle = app.world().resource::<Assets<BuiyFont>>().reserve_handle();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset("Fira Sans", handle.clone(), block_descriptors());
    let entity = spawn_text(&mut app, "hello", pending_stack("Fira Sans"));
    settle(&mut app);
    assert!(app.world().get::<PendingFontBlock>(entity).is_some());

    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_secs_f32(3.5));
    app.update();
    assert!(
        app.world().get::<PendingFontBlock>(entity).is_none(),
        "timed out without a load"
    );
    assert_eq!(
        lowered_family(&app, entity),
        Family::Serif,
        "layout stays the fallback family's — geometry never moved"
    );

    let gen_before = generation(&app);
    let id = handle.id();
    app.world_mut()
        .resource_mut::<Assets<BuiyFont>>()
        .insert(id, BuiyFont { data: fira_bytes() })
        .unwrap();
    settle(&mut app);
    assert_eq!(generation(&app), gen_before + 1);
    assert_eq!(
        lowered_family(&app, entity),
        Family::Name("Fira Sans"),
        "the late load still swaps the face (the normal generation path)"
    );
    assert!(
        app.world().get::<PendingFontBlock>(entity).is_none(),
        "a Loaded family never re-blocks"
    );
}

#[test]
fn fallback_and_optional_degrade_to_swap_with_warn_once() {
    // FontDisplay::Fallback / ::Optional registrations behave exactly as
    // Swap (the C-tier reserve, warn-once): no PendingFontBlock, fallback
    // paints with full alpha.
    let mut harness = TextExtractHarness::new();
    register_pending(&mut harness, "Pending Fallback", FontDisplay::Fallback);
    register_pending(&mut harness, "Pending Optional", FontDisplay::Optional);
    let a = spawn_harness_text(&mut harness, pending_stack("Pending Fallback"));
    let b = spawn_harness_text(&mut harness, pending_stack("Pending Optional"));
    harness.settle();

    assert!(harness.app.world().get::<PendingFontBlock>(a).is_none());
    assert!(harness.app.world().get::<PendingFontBlock>(b).is_none());
    assert!(harness.glyph_count() > 0);
    assert!(
        harness.glyphs().glyphs.iter().all(|g| g.color[3] > 0.0),
        "the C-tier reserve behaves exactly as Swap: full-alpha fallback"
    );
}
