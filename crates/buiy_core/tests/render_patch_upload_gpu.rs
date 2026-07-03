//! GPU reftests (#[ignore]) for the PARTIAL instance uploads.
//!
//! Quad tier (#2 Stage D2): a group-free, pure-bg-quad value change (a hover
//! re-tint) must (a) re-upload ONLY the changed entity's quad slot — not the
//! whole O(N) instance blob — and (b) produce a frame pixel-identical to a
//! cold full render of the same final state.
//!
//! Glyph tier (partial-reextract Stage D / D6): a value change on ONE
//! mid-scene resident text entity must (a) upload ONLY the suffix
//! `[first_dirty_slot..len)` via one `write_buffer_range` — the retained GPU
//! prefix substitutes for the rest — and (b) stay pixel-identical to a cold
//! full render; growth past the GPU buffer's capacity must fall back to the
//! full `write_buffer` repack (still pixel-identical).
//!
//! A wrong slot, span, or pack would diverge from the cold reference. Needs a
//! real wgpu adapter; the headless gate cannot exercise prepare (no device).
//! Run with `cargo test -p buiy_core --test render_patch_upload_gpu -- --ignored`.

mod support;

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, TextColor};
use buiy_core::render::prepare::{BufferUploadStats, ExtractedGlyphs};
use buiy_core::text::{FontSize, GlyphDamage, Text};

const W: u32 = 64;
const H: u32 = 96;

/// A flex column of solid bg nodes, one per entry in `colors`. Returns
/// the GPU app, its readback target, and the child entities (index 0 is the victim).
fn scene(colors: &[Color]) -> (App, Handle<Image>, Vec<Entity>) {
    let mut app = support::gpu_render_app(W, H);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    let kids: Vec<Entity> = colors
        .iter()
        .map(|c| {
            app.world_mut()
                .spawn((
                    Node,
                    Style::default().width_px(40.0).height_px(12.0),
                    Background {
                        color: ColorToken::Custom(*c),
                    },
                ))
                .id()
        })
        .collect();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .padding(4.0)
                .gap_px(4.0),
        ))
        .add_children(&kids);
    (app, target, kids)
}

fn uploaded(app: &App) -> u64 {
    app.get_sub_app(RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<BufferUploadStats>()
        .instances_uploaded
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn partial_upload_patches_one_slot_pixel_identical() {
    // App A: four solid bg nodes, then PATCH node 0's color (a group-free,
    // footprint-stable, pure-bg-quad change → C3b Patch → D2 partial upload).
    let red = Color::srgb(0.90, 0.12, 0.12);
    let blue = Color::srgb(0.12, 0.25, 0.90);
    let (mut a, ta, kids) = scene(&[red, red, red, red]);
    support::finish_and_run(&mut a, 3);
    let before = uploaded(&a); // the full upload packed all 4 instances
    a.world_mut().get_mut::<Background>(kids[0]).unwrap().color = ColorToken::Custom(blue);
    a.update(); // the Patch frame
    a.update(); // settle
    let after = uploaded(&a);
    let pixels_a = support::readback_rgba(&mut a, ta);

    // The Patch frame uploaded EXACTLY the one changed quad instance — proof the partial
    // path fired (a full-repack fallback would re-upload all four).
    assert_eq!(
        after - before,
        1,
        "#2 D2: a pure-bg-quad color Patch uploads exactly 1 quad instance, not all N \
         (delta {} — N is 4)",
        after - before
    );

    // App B: the SAME final colors rendered cold (one full pack + upload).
    let (mut b, tb, _) = scene(&[blue, red, red, red]);
    support::finish_and_run(&mut b, 3);
    let pixels_b = support::readback_rgba(&mut b, tb);

    assert_eq!(
        pixels_a, pixels_b,
        "#2 D2: the partial-upload Patch frame must be pixel-identical to a cold full \
         render of the same final state (a wrong slot/span/pack would diverge)"
    );
}

// --- Glyph tier: the partial-reextract Stage D suffix ranged upload ---------

const GW: u32 = 256;
const GH: u32 = 96;

/// Three absolutely-positioned text rows (no layout coupling between them, so
/// a one-entity edit never reflows a sibling into the changed set). Paint
/// order = spawn order, so index 1 ("the victim") is MID-scene: a retained
/// prefix (row 0) exists below it and a shifted/retained suffix (row 2) above.
fn glyph_scene(labels: [&str; 3], colors: [Color; 3]) -> (App, Handle<Image>, [Entity; 3]) {
    let mut app = support::gpu_render_app(GW, GH);
    let target = support::render_to_image(&mut app, GW, GH);
    support::spawn_capture_camera(&mut app, target.clone());

    let mut kids = [Entity::PLACEHOLDER; 3];
    for (i, (label, color)) in labels.iter().zip(colors).enumerate() {
        kids[i] = app
            .world_mut()
            .spawn((
                Node,
                Style::default().absolute().inset(Inset {
                    top: Sizing::Length(Length::px(8.0 + 28.0 * i as f32)),
                    left: Sizing::Length(Length::px(8.0)),
                    ..default()
                }),
                Text(String::from(*label)),
                FontSize(20.0),
                TextColor(ColorToken::Custom(color)),
            ))
            .id();
    }
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&kids);
    (app, target, kids)
}

fn glyph_stats(app: &App) -> BufferUploadStats {
    *support::render_world_resource::<BufferUploadStats>(app).expect("BufferUploadStats")
}

/// Settle a glyph scene to steady state: text atlas-resident, then drain
/// until an update uploads nothing (pipeline warm-up + the readback poller
/// can dirty early frames — the caret-blink pin's idiom).
fn settle_glyph_scene(app: &mut App) -> BufferUploadStats {
    support::finish_and_run(app, 3);
    support::wait_for_text_ready(app, 60);
    let mut base = glyph_stats(app);
    for _ in 0..10 {
        app.update();
        let now = glyph_stats(app);
        if now == base {
            break;
        }
        base = now;
    }
    app.update();
    assert_eq!(glyph_stats(app), base, "a steady frame uploads NOTHING");
    base
}

/// The victim's run in the published glyph carrier, by value.
fn victim_run(app: &App, victim: Entity) -> std::ops::Range<u32> {
    support::render_world_resource::<ExtractedGlyphs>(app)
        .expect("ExtractedGlyphs")
        .entity_runs
        .iter()
        .find(|r| r.entity == victim)
        .expect("the victim emitted a run")
        .instances
        .clone()
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn glyph_patch_uploads_only_the_suffix_pixel_identical() {
    // App A: three text rows, then re-tint the MID row (a value-tier,
    // length-preserving change on one of three residents → an executed
    // GlyphDamage::Patch → the Stage D suffix ranged upload).
    let white = Color::srgb(0.95, 0.95, 0.95);
    let orange = Color::srgb(0.95, 0.55, 0.10);
    let (mut a, ta, [_, victim, _]) = glyph_scene(["Alpha", "Beta", "Gamma"], [white; 3]);
    let base = settle_glyph_scene(&mut a);

    a.world_mut()
        .entity_mut(victim)
        .insert(TextColor(ColorToken::Custom(orange)));
    a.update(); // the Patch frame
    let after = glyph_stats(&a);

    // The published verdict carries the D6 first dirty slot == the victim's
    // run start (a re-tint is length-preserving), and it is genuinely
    // MID-buffer: a retained prefix exists and the suffix is not the whole
    // carrier.
    let damage = support::render_world_resource::<GlyphDamage>(&a)
        .expect("GlyphDamage")
        .clone();
    let GlyphDamage::Patch {
        first_dirty_slot: Some(first_dirty),
        ..
    } = damage
    else {
        panic!(
            "the re-tint frame must publish an executed Patch with a first dirty slot, got {damage:?}"
        );
    };
    let total = support::render_world_resource::<ExtractedGlyphs>(&a)
        .expect("ExtractedGlyphs")
        .glyphs
        .len() as u64;
    assert_eq!(
        first_dirty,
        victim_run(&a, victim).start,
        "the first dirty slot is the victim's run start"
    );
    assert!(
        first_dirty > 0 && u64::from(first_dirty) < total,
        "mid-scene fixture: 0 < first_dirty ({first_dirty}) < total ({total})"
    );

    // Stage D: exactly ONE suffix ranged upload of `total - first_dirty`
    // instances — not the full buffer (a fallback would count `total`), not
    // zero (a missed upload would keep the old tint).
    assert_eq!(
        after.glyph_uploads,
        base.glyph_uploads + 1,
        "the Patch frame uploads the glyph buffer exactly once"
    );
    assert_eq!(
        after.glyph_partial_uploads,
        base.glyph_partial_uploads + 1,
        "…via the suffix ranged path, not the full-repack fallback"
    );
    assert_eq!(
        after.glyph_instances_uploaded - base.glyph_instances_uploaded,
        total - u64::from(first_dirty),
        "the upload delta is EXACTLY the suffix length (computed from the \
         published first_dirty_slot)"
    );
    assert_eq!(
        after.quad_uploads, base.quad_uploads,
        "a glyph re-tint does not touch the quad buffer (independent gating)"
    );

    a.update(); // settle
    assert_eq!(glyph_stats(&a), after, "post-patch frame is steady");
    let pixels_a = support::readback_rgba(&mut a, ta);

    // App B: the SAME final state rendered cold (one full pack + upload).
    let (mut b, tb, _) = glyph_scene(["Alpha", "Beta", "Gamma"], [white, orange, white]);
    settle_glyph_scene(&mut b);
    let pixels_b = support::readback_rgba(&mut b, tb);

    assert_eq!(
        pixels_a, pixels_b,
        "D6: the suffix-upload Patch frame must be pixel-identical to a cold \
         full render of the same final state (a wrong range start/span would \
         freeze stale instances in the retained prefix)"
    );
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn glyph_patch_growth_falls_back_to_full_upload_pixel_identical() {
    // The #84 review's fallback ask, for glyphs: a Patch whose splice GROWS
    // the carrier past the GPU buffer's capacity (steady-state capacity ==
    // the old length) cannot ride `write_buffer_range` — prepare must fall
    // back to the full `write_buffer` repack (which reserves/recreates), and
    // the frame must still be pixel-identical to a cold render.
    let white = Color::srgb(0.95, 0.95, 0.95);
    let (mut a, ta, [_, victim, _]) = glyph_scene(["Alpha", "Beta", "Gamma"], [white; 3]);
    let base = settle_glyph_scene(&mut a);

    // A length-GROWING single-line edit ("Beta" → "Betamax": +3 instances,
    // no wrap, so no sibling reflow joins the changed set).
    a.world_mut().get_mut::<Text>(victim).unwrap().0 = String::from("Betamax");
    a.update(); // the Patch frame (extract splices; prepare falls back)
    let after = glyph_stats(&a);

    // Extract still executed a Patch (the splice win is seam-independent)…
    let damage = support::render_world_resource::<GlyphDamage>(&a)
        .expect("GlyphDamage")
        .clone();
    assert!(
        matches!(
            damage,
            GlyphDamage::Patch {
                first_dirty_slot: Some(_),
                ..
            }
        ),
        "the growth edit still executes as an extract-side Patch, got {damage:?}"
    );
    let total = support::render_world_resource::<ExtractedGlyphs>(&a)
        .expect("ExtractedGlyphs")
        .glyphs
        .len() as u64;

    // …but prepare's capacity guard rejected the ranged path: one FULL
    // upload of the whole grown carrier, zero partial uploads.
    assert_eq!(
        after.glyph_uploads,
        base.glyph_uploads + 1,
        "the growth frame uploads the glyph buffer exactly once"
    );
    assert_eq!(
        after.glyph_partial_uploads, base.glyph_partial_uploads,
        "growth past GPU capacity must NOT ride the ranged path"
    );
    assert_eq!(
        after.glyph_instances_uploaded - base.glyph_instances_uploaded,
        total,
        "the fallback wrote the WHOLE grown carrier"
    );

    a.update(); // settle
    assert_eq!(glyph_stats(&a), after, "post-growth frame is steady");
    let pixels_a = support::readback_rgba(&mut a, ta);

    // App B: the grown final state rendered cold.
    let (mut b, tb, _) = glyph_scene(["Alpha", "Betamax", "Gamma"], [white; 3]);
    settle_glyph_scene(&mut b);
    let pixels_b = support::readback_rgba(&mut b, tb);

    assert_eq!(
        pixels_a, pixels_b,
        "the growth-fallback frame must be pixel-identical to a cold full \
         render of the same final state"
    );
}
