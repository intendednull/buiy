//! GPU acceptance fixtures for the top-layer stacking composite (Wave 4 of
//! `docs/plans/2026-07-10-toplayer-stacking-composite.md` §4; spec
//! `docs/specs/2026-07-10-toplayer-stacking-composite-design.md` §4). These
//! GRADUATE the spike fixtures into permanent `#[ignore]` GPU reftests that prove
//! a `.top_layer()` subtree occludes the base across EVERY paint tier + effect
//! path — not just the fill tier `scrim_tier_bleed_gpu.rs` already witnesses.
//!
//! They are acceptance GATES, not RED-first: the W2 per-block draw restructure
//! already landed, so each PASSES on current behavior. The value is durable
//! coverage of the harder paths (effect groups, backdrop-filter both directions,
//! rasters/gradients inside overlays, the bare-overlay `any_top_layer` fix, and
//! the accepted single-boundary-v1 limitation) plus proving byte-stability holds.
//!
//! Assertions follow the crate convention: DOMINANT-/per-channel deltas (NOT
//! color-sum, which understates a dim on a saturated base), and adapter-robust
//! channel DOMINANCE for opaque-overlap paint-order proofs (interior pixels only,
//! so quad SDF coverage is 1.0 — no AA to fuzz the readback).
//!
//! Run: `env RUST_MIN_STACK=33554432 cargo test -p buiy_core --test render \
//!   toplayer_occludes -- --ignored --test-threads=1 --nocapture`

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style, TopLayer};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{
    BackdropFilter, Background, BackgroundLayer, BackgroundLayers, Border, BorderSide, ColorStop,
    Corners, FilterFn, LineStyle, LinearGradient, Opacity, TextColor,
};
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};
use buiy_core::render::raster::RasterImage;
use buiy_core::text::{FontSize, Text};

use crate::support::{
    finish_and_run, gpu_render_app, px, readback_rgba, render_to_image, spawn_capture_camera,
    wait_for_text_ready,
};

/// The real Dooduel SCRIM (`apps/dooduel/src/theme.rs`): a dark blue-gray at
/// alpha 156/255 — the same translucent overlay `scrim_tier_bleed_gpu.rs` uses.
const SCRIM: (u8, u8, u8, u8) = (0x14, 0x16, 0x1b, 0x9c);
/// The canvas texel color (opaque red), authored as the destination
/// `Rgba8UnormSrgb` byte so it reads back byte-exact (the F1 raster round-trip).
const CANVAS_RED: [u8; 4] = [220, 40, 40, 255];
/// The per-channel drop (WITHOUT − WITH) that counts as a real DIM under the
/// scrim — the same threshold `scrim_tier_bleed_gpu.rs` uses.
const DIM: i32 = 30;

// --- shared authoring helpers (mirrors scrim_tier_bleed_gpu.rs) --------------

fn opaque(r: u8, g: u8, b: u8) -> Background {
    Background {
        color: ColorToken::Custom(Color::srgb_u8(r, g, b)),
    }
}

/// Absolute box at `(x, y)` sized `w×h`; paint order follows document/spawn order.
fn abs(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style::default()
        .absolute()
        .inset(Inset {
            top: Sizing::Length(Length::px(y)),
            left: Sizing::Length(Length::px(x)),
            ..default()
        })
        .width_px(w)
        .height_px(h)
}

/// Author an `n×n` solid-`rgba` `Rgba8UnormSrgb` canvas with `RenderAssetUsages::all()`
/// (the `GpuImage` is created AND the main-world `data` survives the render-world
/// clone — the `data.take()` trap the raster module documents).
fn solid_canvas(app: &mut App, n: u32, rgba: [u8; 4]) -> Handle<Image> {
    let img = Image::new_fill(
        Extent3d {
            width: n,
            height: n,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    app.world_mut().resource_mut::<Assets<Image>>().add(img)
}

/// `true` iff channel `dom` strictly dominates the other two by `margin` — the
/// adapter-robust "this pixel is layer X" test (X's fill is X-channel-dominant).
fn dominates(px: [u8; 4], dom: usize, margin: i32) -> bool {
    let v = px[dom] as i32;
    (0..3).all(|c| c == dom || v - px[c] as i32 > margin)
}

/// The brightest (max channel-sum) pixel in a box — locates sparse ink (a white
/// glyph stroke, a saturated overlay) without pixel-perfect coordinates.
fn brightest_in(pixels: &[u8], w: u32, x0: u32, y0: u32, x1: u32, y1: u32) -> [u8; 4] {
    let mut best = [0u8; 4];
    let mut best_sum = -1i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = px(pixels, w, x, y);
            let s = p[0] as i32 + p[1] as i32 + p[2] as i32;
            if s > best_sum {
                best_sum = s;
                best = p;
            }
        }
    }
    best
}

/// Mean absolute horizontal neighbor delta (R channel) over a rect — a simple
/// local-variance proxy. Sharp stripes → high; a smoothed/blurred region → low;
/// a flat fill → ~0. (Mirrors `render_backdrop_blur_gpu.rs::horiz_variance`.)
fn horiz_variance(pixels: &[u8], w: u32, x0: u32, x1: u32, y0: u32, y1: u32) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for y in y0..y1 {
        for x in x0..(x1 - 1) {
            let a = px(pixels, w, x, y)[0] as i32;
            let b = px(pixels, w, x + 1, y)[0] as i32;
            sum += (a - b).abs() as f32;
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { sum / n as f32 }
}

/// Mean channel value over a rect — a brightness proxy for a DIM (A/B) check on a
/// textured region (a blurred/striped backdrop under a translucent scrim).
fn mean_channel(pixels: &[u8], w: u32, dom: usize, x0: u32, x1: u32, y0: u32, y1: u32) -> f32 {
    let mut sum = 0.0f32;
    let mut n = 0u32;
    for y in y0..y1 {
        for x in x0..x1 {
            sum += px(pixels, w, x, y)[dom] as f32;
            n += 1;
        }
    }
    if n == 0 { 0.0 } else { sum / n as f32 }
}

// =============================================================================
// Task 4.1 — a base EFFECT GROUP under a top-layer scrim DIMS.
// =============================================================================

/// Render an `Opacity(0.5)` base group (an opaque-red child) under a full-viewport
/// top-layer scrim at `scrim_alpha`. Returns the readback.
fn render_group_under_scrim(scrim_alpha: u8) -> (Vec<u8>, u32) {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut app = gpu_render_app(W, H);
    let target = render_to_image(&mut app, W, H);
    spawn_capture_camera(&mut app, target.clone());

    // A base OPAQUE-red child that fully paints the group's off-screen target.
    let child = app
        .world_mut()
        .spawn((
            Node,
            Name::new("group_child"),
            abs(16.0, 16.0, 32.0, 32.0),
            opaque(230, 13, 13), // ≈ srgb(0.9, 0.05, 0.05)
        ))
        .id();
    // The Opacity(0.5) parent — an EffectGroup former; its child composites ONCE
    // in the off-screen `Rgba16Float` target, then that target composites at 0.5
    // over the window (the base block's step-2b composite).
    let group = app
        .world_mut()
        .spawn((
            Node,
            Name::new("opacity_group"),
            Style::default().absolute(),
            Opacity(0.5),
        ))
        .id();
    app.world_mut().entity_mut(group).add_children(&[child]);

    // The translucent full-viewport TOP-LAYER scrim (alpha present or 0), a LATER
    // sibling so it paints last — the top block draws it OVER the composited group.
    let scrim = app
        .world_mut()
        .spawn((
            Node,
            Name::new("scrim"),
            abs(0.0, 0.0, W as f32, H as f32).top_layer(TopLayer::Popover),
            Background {
                color: ColorToken::Custom(Color::srgba_u8(SCRIM.0, SCRIM.1, SCRIM.2, scrim_alpha)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().width_px(W as f32).height_px(H as f32),
        ))
        .add_children(&[group, scrim]);

    finish_and_run(&mut app, 4); // compositor: off-screen group pass + settle
    (readback_rgba(&mut app, target), W)
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn base_effect_group_under_top_layer_scrim_dims() {
    // A = scrim present (alpha 156); B = scrim suppressed (alpha 0). Everything
    // else identical, so any per-pixel delta is exactly the scrim's effect on the
    // COMPOSITED base group.
    let (a, w) = render_group_under_scrim(SCRIM.3);
    let (b, _) = render_group_under_scrim(0);

    // Deep-interior sample of the group's red child (16..48).
    let group_a = px(&a, w, 30, 30);
    let group_b = px(&b, w, 30, 30);
    eprintln!("group under scrim: WITH={group_a:?}  WITHOUT={group_b:?}");

    // Non-vacuous: the group actually composited a visible mid-red at 0.5 in B
    // (0.5·red over black), so there IS something for the scrim to dim.
    assert!(
        group_b[0] as i32 >= 90 && dominates(group_b, 0, 40),
        "the base Opacity(0.5) group must composite a visible red (0.5 over black): {group_b:?}"
    );
    // THE WITNESS: the top-layer scrim DIMS the composited group — the group is a
    // BASE-block composite (step-2b), the scrim draws over it in the top block.
    let drop = group_b[0] as i32 - group_a[0] as i32;
    assert!(
        drop >= DIM,
        "the top-layer scrim must DIM the composited base effect group (per-block \
         composite ordering): red {} -> {} (Δ{drop}, WITH={group_a:?} WITHOUT={group_b:?})",
        group_b[0],
        group_a[0]
    );
}

// =============================================================================
// Task 4.2 — backdrop-filter BOTH directions.
//   (a) a TOP-LAYER backdrop-filter overlay blurs the BASE beneath it.
//   (b) a BASE backdrop-filter element under a top-layer scrim blurs the base
//       AND is dimmed by the scrim above (the scrim does not blur; it occludes).
// =============================================================================

/// Spawn one absolutely-positioned solid rect under `parent` (a backdrop stripe).
fn spawn_stripe(app: &mut App, parent: Entity, x: f32, w: f32, h: f32, color: Color) {
    let e = app
        .world_mut()
        .spawn((
            Node,
            abs(x, 0.0, w, h),
            Background {
                color: ColorToken::Custom(color),
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[e]);
}

/// Lay a bright/dark 5px striped backdrop across `root` (maximal horizontal
/// local variance — the blur is unmistakable in the variance metric).
fn stripe_backdrop(app: &mut App, root: Entity, w: u32, h: u32) {
    let bright = Color::srgb(0.95, 0.95, 0.95);
    let dark = Color::srgb(0.05, 0.05, 0.05);
    let mut x = 0.0;
    let mut on = true;
    while x < w as f32 {
        spawn_stripe(app, root, x, 5.0, h as f32, if on { bright } else { dark });
        x += 5.0;
        on = !on;
    }
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn top_layer_backdrop_filter_blurs_base_beneath() {
    // Direction (a): a `.top_layer()` overlay carrying `backdrop-filter: blur`
    // samples the BASE content painted beneath it (the base block drew first), so
    // the base stripes under the overlay are smoothed — the top block reaching
    // DOWN to the base window state.
    const W: u32 = 120;
    const H: u32 = 64;
    let mut app = capture_app(W, H);
    let root = app.world_mut().spawn((Node, Style::default())).id();
    stripe_backdrop(&mut app, root, W, H);

    // The blur overlay covers the RIGHT HALF, `.top_layer()`, translucent fill.
    let overlay = app
        .world_mut()
        .spawn((
            Node,
            abs(60.0, 0.0, 60.0, H as f32).top_layer(TopLayer::Popover),
            Background {
                color: ColorToken::Custom(Color::srgba(0.1, 0.12, 0.18, 0.35)),
            },
            BackdropFilter(vec![FilterFn::Blur(Length::px(6.0))]),
        ))
        .id();
    app.world_mut().entity_mut(root).add_children(&[overlay]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    let pixels = img.into_raw();

    let unblurred = horiz_variance(&pixels, W, 5, 55, 8, 56); // uncovered stripes
    let blurred = horiz_variance(&pixels, W, 66, 114, 8, 56); // under the overlay
    eprintln!("top-layer backdrop: unblurred={unblurred:.2}  blurred={blurred:.2}");

    assert!(
        unblurred > 30.0,
        "the uncovered base stripes must be high-variance (sharp bars): {unblurred:.2}"
    );
    assert!(
        blurred < unblurred * 0.6,
        "a TOP-LAYER backdrop-filter must BLUR the base beneath it (it samples the \
         base block painted first): blurred {blurred:.2} < 0.6 × unblurred {unblurred:.2}"
    );
    assert!(
        blurred > 1.0,
        "the blurred region must still show base-stripe residue (not a flat fill): {blurred:.2}"
    );
}

/// Render a BASE backdrop-filter element (left half) over base stripes, under a
/// full-width top-layer scrim at `scrim_alpha`. Returns the raw pixels.
fn render_base_backdrop_under_scrim(scrim_alpha: u8, w: u32, h: u32) -> Vec<u8> {
    let mut app = capture_app(w, h);
    let root = app.world_mut().spawn((Node, Style::default())).id();
    stripe_backdrop(&mut app, root, w, h);

    // A BASE backdrop-filter element over the LEFT half — it blurs the base
    // stripes behind it (base block), no `.top_layer()`.
    let base_blur = app
        .world_mut()
        .spawn((
            Node,
            abs(0.0, 0.0, 60.0, h as f32),
            Background {
                color: ColorToken::Custom(Color::srgba(0.1, 0.12, 0.18, 0.35)),
            },
            BackdropFilter(vec![FilterFn::Blur(Length::px(6.0))]),
        ))
        .id();
    // A full-width TOP-LAYER scrim over everything (present or suppressed).
    let scrim = app
        .world_mut()
        .spawn((
            Node,
            abs(0.0, 0.0, w as f32, h as f32).top_layer(TopLayer::Modal),
            Background {
                color: ColorToken::Custom(Color::srgba_u8(SCRIM.0, SCRIM.1, SCRIM.2, scrim_alpha)),
            },
        ))
        .id();
    app.world_mut()
        .entity_mut(root)
        .add_children(&[base_blur, scrim]);

    capture_to_image(&mut app, &GoldenConfig::deterministic()).into_raw()
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn base_backdrop_filter_under_top_layer_scrim_blurs_and_dims() {
    // Direction (b): a BASE backdrop element blurs the base stripes behind it
    // (base block); a top-layer scrim ABOVE it does NOT blur it — the scrim
    // OCCLUDES (dims) the already-blurred region. "blur + dim" in one scene.
    const W: u32 = 120;
    const H: u32 = 64;
    let a = render_base_backdrop_under_scrim(SCRIM.3, W, H); // scrim present
    let b = render_base_backdrop_under_scrim(0, W, H); // scrim suppressed

    // (1) BLUR: measured in B (no scrim), the base-backdrop region is smoothed
    // well below the sharp uncovered stripes on the right.
    let unblurred = horiz_variance(&b, W, 66, 114, 8, 56);
    let blurred = horiz_variance(&b, W, 6, 54, 8, 56);
    eprintln!("base backdrop: unblurred={unblurred:.2}  blurred={blurred:.2}");
    assert!(
        unblurred > 30.0,
        "the uncovered base stripes stay sharp (right half, no backdrop element): {unblurred:.2}"
    );
    assert!(
        blurred < unblurred * 0.6,
        "the BASE backdrop element must blur the base stripes behind it: blurred \
         {blurred:.2} < 0.6 × unblurred {unblurred:.2}"
    );

    // (2) DIM: the top-layer scrim dims the blurred base-backdrop region (A vs B).
    // The scrim occludes it; it does NOT blur it (a base backdrop cannot reach
    // forward to a later top-layer element, and the scrim carries no filter).
    let dim_a = mean_channel(&a, W, 0, 6, 54, 8, 56);
    let dim_b = mean_channel(&b, W, 0, 6, 54, 8, 56);
    eprintln!("base backdrop under scrim: WITH mean-R={dim_a:.1}  WITHOUT mean-R={dim_b:.1}");
    assert!(
        dim_b - dim_a >= DIM as f32,
        "the top-layer scrim must DIM the base backdrop region beneath it: mean-R \
         {dim_b:.1} -> {dim_a:.1} (Δ{:.1})",
        dim_b - dim_a
    );
}

// =============================================================================
// Task 4.3 — a RASTER inside a top-layer overlay draws in the top block, over
// base; PLUS the drift-#1 witness: a BARE raster/gradient-only top-layer overlay
// (no panel/background quad) OCCLUDES base glyph via `any_top_layer` (aacddfc).
// =============================================================================

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn raster_inside_a_top_layer_overlay_occludes_base_band() {
    // A `RasterImage` CHILD of a `.top_layer()` panel paints in the TOP block over
    // a base BORDERED box — the overlay occludes the base BAND tier (the delta
    // over `render_raster_interleave_gpu.rs`, which only proved raster-over-fill).
    const W: u32 = 100;
    let mut app = gpu_render_app(W, W);
    let canvas = solid_canvas(&mut app, 30, CANVAS_RED);

    // A base bordered box: no fill, a thick YELLOW border (BAND tier), at
    // (8,8) 60×60, 8px border → left border x[8,16], bottom border y[60,68].
    let side = |c: Color| BorderSide {
        color: ColorToken::Custom(c),
        style: LineStyle::Solid,
    };
    let base_box = app
        .world_mut()
        .spawn((
            Node,
            Name::new("base_bordered_box"),
            abs(8.0, 8.0, 60.0, 60.0).border(8.0),
            Border {
                top: side(Color::srgb_u8(240, 220, 20)),
                right: side(Color::srgb_u8(240, 220, 20)),
                bottom: side(Color::srgb_u8(240, 220, 20)),
                left: side(Color::srgb_u8(240, 220, 20)),
                radius: Corners::ZERO,
            },
        ))
        .id();
    // A raster CHILD (red canvas) inside a `.top_layer()` OPAQUE gray panel. Panel
    // at (20,20) 50×50; raster child absolute inset (10,10) → (30,30) 30×30.
    let raster = app
        .world_mut()
        .spawn((
            Node,
            Name::new("avatar"),
            abs(10.0, 10.0, 30.0, 30.0),
            RasterImage(canvas),
        ))
        .id();
    let panel = app
        .world_mut()
        .spawn((
            Node,
            Name::new("panel"),
            abs(20.0, 20.0, 50.0, 50.0).top_layer(TopLayer::Modal),
            opaque(95, 95, 100),
        ))
        .add_children(&[raster])
        .id();
    app.world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().width_px(W as f32).height_px(W as f32),
        ))
        .add_children(&[base_box, panel]);

    let target = render_to_image(&mut app, W, W);
    spawn_capture_camera(&mut app, target.clone());
    finish_and_run(&mut app, 5); // image upload + raster pipeline compile + draw
    let pixels = readback_rgba(&mut app, target);

    // (i) the raster painted its red texel (inside the top-layer overlay).
    assert!(
        pixels.chunks_exact(4).any(|p| p == CANVAS_RED),
        "the raster canvas painted inside the top-layer overlay (its red texel is present)"
    );
    // (ii) the raster shows OVER the panel bg: (44,44) is red-dominant.
    let raster_px = px(&pixels, W, 44, 44);
    assert!(
        dominates(raster_px, 0, 60),
        "the raster child paints in the top block over its panel: (44,44) red-dominant. got {raster_px:?}"
    );
    // (iii) the overlay OCCLUDES the base band: the base box's bottom border
    // (y[60,68]) at (40,64) is under the panel (20..70) → gray panel, NOT yellow.
    let occluded = px(&pixels, W, 40, 64);
    let is_yellow = occluded[0] as i32 + occluded[1] as i32 - 2 * occluded[2] as i32 > 120;
    assert!(
        !is_yellow,
        "the top-layer overlay must OCCLUDE the base border BAND beneath it: (40,64) \
         must NOT read yellow (it reads the panel bg); got {occluded:?}"
    );
    // (iv) the base band still paints where the overlay does NOT cover it: the
    // base box's LEFT border (x[8,16]) at (12,40) is outside the panel → yellow.
    let visible = px(&pixels, W, 12, 40);
    let visible_yellow = visible[0] as i32 + visible[1] as i32 - 2 * visible[2] as i32 > 120;
    assert!(
        visible_yellow,
        "the base border BAND still paints where the overlay does not cover it: (12,40) \
         is yellow; got {visible:?}"
    );
}

/// Spawn a base white-glyph text leaf + a bare (no-Background) top-layer overlay
/// built by `overlay`, and read back. Shared by the raster-only + gradient-only
/// drift-#1 witnesses. The overlay covers the LEFT half of the text; the right
/// half stays uncovered (the control).
fn render_bare_overlay_over_glyph(overlay: impl FnOnce(&mut App) -> Entity) -> (Vec<u8>, u32, u32) {
    const W: u32 = 200;
    const H: u32 = 80;
    let mut app = gpu_render_app(W, H);

    // A wide base GLYPH row (white text), large so a stem covers several pixels.
    let text = app
        .world_mut()
        .spawn((
            Node,
            Name::new("base_glyph"),
            abs(4.0, 30.0, 190.0, 44.0),
            Text(String::from("MMMMMMMMMM")),
            TextColor(ColorToken::Custom(Color::srgb_u8(245, 245, 245))),
            FontSize(40.0),
        ))
        .id();
    let ov = overlay(&mut app);
    app.world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().width_px(W as f32).height_px(H as f32),
        ))
        .add_children(&[text, ov]);

    let target = render_to_image(&mut app, W, H);
    spawn_capture_camera(&mut app, target.clone());
    finish_and_run(&mut app, 1);
    wait_for_text_ready(&mut app, 60);
    (readback_rgba(&mut app, target), W, H)
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn bare_raster_only_top_layer_overlay_occludes_base_glyph() {
    // The drift-#1 GPU witness (aacddfc / `any_top_layer`): a BARE raster-only
    // (no panel / no Background quad) `.top_layer()` overlay must OCCLUDE the base
    // glyph beneath it. Pre-fix a bare raster-only top-layer node drew in the BASE
    // block, so the base glyph (later global glyph tier) BLED over it — the
    // brightest pixel in the covered region would be WHITE. Post-fix the bare
    // raster draws in the TOP block, so the covered region is RED (glyph occluded).
    let (pixels, w, _h) = render_bare_overlay_over_glyph(|app| {
        let canvas = solid_canvas(app, 30, CANVAS_RED);
        // A bare RasterImage — NO Background quad — covering the LEFT half of the
        // text (x[4,96]). Its top-layer tag flips `any_top_layer` even though it
        // pushes no quad/shadow/band, so the top block draws + occludes.
        app.world_mut()
            .spawn((
                Node,
                Name::new("bare_raster_overlay"),
                abs(4.0, 26.0, 92.0, 48.0).top_layer(TopLayer::Popover),
                RasterImage(canvas),
            ))
            .id()
    });

    // Non-vacuous: the raster painted (its red texel is present somewhere).
    assert!(
        pixels.chunks_exact(4).any(|p| p == CANVAS_RED),
        "the bare raster overlay painted (its red texel is present)"
    );
    // THE WITNESS: in the COVERED left region the brightest pixel is RED (the
    // opaque raster occludes the glyph) — NOT white glyph ink bleeding through.
    let covered = brightest_in(&pixels, w, 10, 30, 90, 74);
    assert!(
        dominates(covered, 0, 60),
        "a BARE raster-only top-layer overlay must OCCLUDE the base glyph (any_top_layer \
         fix): the brightest covered pixel is RED, not bled-through white glyph ink. got {covered:?}"
    );
    // Control: the RIGHT (uncovered) half still shows white glyph ink — proving
    // the glyph really is there and it is the overlay, not a missing glyph, that
    // makes the left red.
    let uncovered = brightest_in(&pixels, w, 110, 30, 190, 74);
    assert!(
        uncovered[0] > 150 && uncovered[1] > 150 && uncovered[2] > 150,
        "the base glyph still paints white where the overlay does not cover it: {uncovered:?}"
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn bare_gradient_only_top_layer_overlay_occludes_base_glyph() {
    // The drift-#1 companion for the GRADIENT carrier: a BARE gradient-only (no
    // solid Background) `.top_layer()` overlay pushes only a gradient anchor AT the
    // quad boundary, which `block_interleave` routes to the TOP block (the headless
    // `block_interleave_routes_an_at_boundary_gradient_to_the_top_block` proves the
    // routing; this proves it OCCLUDES on a real GPU). Opaque blue→cyan stops so
    // the covered glyph reads blue-dominant, not white.
    let (pixels, w, _h) = render_bare_overlay_over_glyph(|app| {
        app.world_mut()
            .spawn((
                Node,
                Name::new("bare_gradient_overlay"),
                abs(4.0, 26.0, 92.0, 48.0).top_layer(TopLayer::Popover),
                // Gradient ONLY — no `Background`, so the node pushes no quad.
                BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                    angle_deg: 90.0,
                    stops: vec![
                        ColorStop {
                            color: ColorToken::Custom(Color::srgb_u8(20, 60, 230)),
                            position: 0.0,
                        },
                        ColorStop {
                            color: ColorToken::Custom(Color::srgb_u8(40, 210, 230)),
                            position: 1.0,
                        },
                    ],
                })]),
            ))
            .id()
    });

    // In the COVERED left region, the brightest pixel is BLUE-dominant (the opaque
    // gradient occludes the glyph) — NOT bled-through white glyph ink (all-high).
    let covered = brightest_in(&pixels, w, 10, 30, 90, 74);
    assert!(
        covered[2] as i32 - covered[0] as i32 > 40 && covered[2] > covered[1],
        "a BARE gradient-only top-layer overlay must OCCLUDE the base glyph (any_top_layer \
         + at-boundary gradient routing): the covered region is blue-dominant gradient, not \
         white glyph ink. got {covered:?}"
    );
    // Control: the uncovered right half still shows white glyph ink.
    let uncovered = brightest_in(&pixels, w, 110, 30, 190, 74);
    assert!(
        uncovered[0] > 150 && uncovered[1] > 150 && uncovered[2] > 150,
        "the base glyph still paints white where the overlay does not cover it: {uncovered:?}"
    );
}

// =============================================================================
// Task 4.4 — single-boundary-v1 limitation (a deferred-follow-up characterization
// gate). Two OVERLAPPING top-layer overlays draw as ONE block, so within it the
// tiers are still global: a Modal-tier scrim QUAD draws before a Tooltip-tier
// BORDER band, so the scrim does NOT dim a fellow top-layer band — while it DOES
// dim a base band. This asserts the CURRENT (accepted) v1 behavior, not the
// desired per-context behavior.
//
// FIXME(per-context-v1): when the per-context follow-up ships (spec §4 "per-context
// ordering" — `docs/plans/follow-ups.md`), a Modal scrim OVER a Tooltip should DIM
// the Tooltip's border. Flip the tooltip-border assertion below from Δ≈0 to Δ≥DIM
// at that point. Until then this gate PINS the single-boundary-v1 limitation.
// =============================================================================

/// Render a base bordered box + a Tooltip-tier bordered overlay, both under a
/// full-viewport Modal-tier scrim at `scrim_alpha`. Returns the readback.
fn render_overlapping_top_layers(scrim_alpha: u8) -> (Vec<u8>, u32) {
    const W: u32 = 160;
    const H: u32 = 60;
    let mut app = gpu_render_app(W, H);
    let target = render_to_image(&mut app, W, H);
    spawn_capture_camera(&mut app, target.clone());

    let side = |c: Color| BorderSide {
        color: ColorToken::Custom(c),
        style: LineStyle::Solid,
    };
    let bordered = |c: Color| Border {
        top: side(c),
        right: side(c),
        bottom: side(c),
        left: side(c),
        radius: Corners::ZERO,
    };

    // A BASE bordered box (RED border, BAND tier), left border x[10,18].
    let base_box = app
        .world_mut()
        .spawn((
            Node,
            Name::new("base_box"),
            abs(10.0, 10.0, 40.0, 40.0).border(8.0),
            bordered(Color::srgb_u8(230, 30, 30)),
        ))
        .id();
    // A TOOLTIP-tier bordered overlay (GREEN border, BAND tier), left border x[108,116].
    let tooltip = app
        .world_mut()
        .spawn((
            Node,
            Name::new("tooltip_overlay"),
            abs(105.0, 10.0, 40.0, 40.0)
                .border(8.0)
                .top_layer(TopLayer::Tooltip),
            bordered(Color::srgb_u8(30, 220, 60)),
        ))
        .id();
    // A MODAL-tier full-viewport scrim over BOTH (present or suppressed).
    let scrim = app
        .world_mut()
        .spawn((
            Node,
            Name::new("modal_scrim"),
            abs(0.0, 0.0, W as f32, H as f32).top_layer(TopLayer::Modal),
            Background {
                color: ColorToken::Custom(Color::srgba_u8(SCRIM.0, SCRIM.1, SCRIM.2, scrim_alpha)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().width_px(W as f32).height_px(H as f32),
        ))
        .add_children(&[base_box, tooltip, scrim]);

    finish_and_run(&mut app, 3);
    (readback_rgba(&mut app, target), W)
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn single_boundary_v1_scrim_dims_base_band_not_a_fellow_top_layer_band() {
    // A = Modal scrim present; B = suppressed. The base RED border must DIM (the
    // W2 all-tiers occlusion: a top-block scrim over a base-block band), but the
    // Tooltip GREEN border must NOT dim — both overlays are ONE top block, so the
    // Tooltip's band draws AFTER the Modal scrim's quad WITHIN it (global tiers).
    let (a, w) = render_overlapping_top_layers(SCRIM.3);
    let (b, _) = render_overlapping_top_layers(0);

    // Base RED border interior (left band x[10,18], sample x=13).
    let base_a = px(&a, w, 13, 30);
    let base_b = px(&b, w, 13, 30);
    // Tooltip GREEN border interior (left band x[105,113], sample x=108).
    let tip_a = px(&a, w, 108, 30);
    let tip_b = px(&b, w, 108, 30);
    eprintln!("base band:    WITH={base_a:?} WITHOUT={base_b:?}");
    eprintln!("tooltip band: WITH={tip_a:?} WITHOUT={tip_b:?}");

    // The base band DIMS (proving the scrim is live and does occlude base content).
    let base_drop = base_b[0] as i32 - base_a[0] as i32;
    assert!(
        base_drop >= DIM,
        "the Modal scrim DIMS the BASE band (W2 all-tiers occlusion): red {} -> {} (Δ{base_drop})",
        base_b[0],
        base_a[0]
    );
    // The Tooltip band does NOT dim — the accepted single-boundary-v1 limitation:
    // both overlays are one top block, so the Tooltip band draws after the scrim
    // quad WITHIN it. This is Δ≈0 (spike-confirmed), NOT the desired per-context DIM.
    let tip_drop = tip_b[1] as i32 - tip_a[1] as i32;
    assert!(
        tip_drop.abs() < 12,
        "single-boundary-v1: the Modal scrim must NOT dim a fellow top-layer (Tooltip) \
         band — both draw in ONE top block (global tiers within it). green {} vs {} (Δ{tip_drop}). \
         FIXME(per-context-v1): flip to Δ≥DIM when per-context ships",
        tip_b[1],
        tip_a[1]
    );
}
