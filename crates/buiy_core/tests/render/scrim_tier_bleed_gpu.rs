//! STAGE-1 ROOT-CAUSE controlled A/B (GPU lane, `#[ignore]`): the decisive
//! per-tier experiment for the "translucent modal scrim looks transparent" bug.
//!
//! The CPU pipeline is proven correct (the scrim quad is drawn LAST in the quad
//! tier, correct alpha, group=None — see `apps/dooduel/tests/scrim_group_probe.rs`),
//! and the scrim DOES blend correctly over base QUADS. The real cause is Buiy's
//! **GLOBAL TIER rendering** (`crates/buiy_core/src/render/node.rs`): every quad
//! (in paint order, incl. the escaped top-layer scrim) draws FIRST, then ONE
//! global glyph pass (`0..glyph_count`), then ONE global icon pass, then ONE
//! global band pass. So a top-layer scrim QUAD cannot occlude base TEXT / ICONS /
//! BORDERS — they draw in later global tiers and BLEED THROUGH undimmed. This is
//! the documented limitation at `apps/dooduel/src/view/mod.rs:7-11`.
//!
//! This renders ONE scene twice — identical except the scrim's alpha (156 vs 0)
//! — at the real capture's `Msaa::Sample4`, and reads back one pixel of EACH
//! tier, reporting the WITH-vs-WITHOUT dim table:
//!   * a base QUAD fill      → EXPECT: dims (scrim is a quad, drawn over it)
//!   * a bordered-box BAND   → EXPECT: bleeds (band tier draws after the quad)
//!   * a base GLYPH (text)   → EXPECT: bleeds (glyph tier draws after the quad)
//!   * the RASTER canvas     → EXPECT: dims (raster interleaves in the quad tier)
//!
//! Plus a DARK-base quad variant → EXPECT: near-zero delta (iso-luminance).
//!
//! Run: `env RUST_MIN_STACK=33554432 cargo test -p buiy_core --test render \
//!   scrim_tier_bleed -- --ignored --test-threads=1 --nocapture`

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::view::Msaa;
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, Border, BorderSide, Corners, LineStyle, TextColor};
use buiy_core::render::raster::RasterImage;
use buiy_core::text::Text;

use crate::support::{
    finish_and_run, gpu_render_app, px, readback_rgba, render_to_image,
    spawn_capture_camera_with_msaa, wait_for_text_ready,
};

const W: u32 = 200;
const H: u32 = 120;
/// The real Dooduel SCRIM (`apps/dooduel/src/theme.rs:164`): alpha 156/255.
const SCRIM: (u8, u8, u8, u8) = (0x14, 0x16, 0x1b, 0x9c);
/// The dooduel DARK canvas token (`0x1b1e25`) — the dark-theme iso-luminance case.
const DARK_CANVAS: (u8, u8, u8) = (0x1b, 0x1e, 0x25);
const CANVAS_RED: [u8; 4] = [220, 40, 40, 255];

fn solid_canvas(app: &mut App, rgba: [u8; 4]) -> Handle<Image> {
    let img = Image::new_fill(
        Extent3d {
            width: 8,
            height: 8,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        &rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::all(),
    );
    app.world_mut().resource_mut::<Assets<Image>>().add(img)
}

fn opaque(r: u8, g: u8, b: u8) -> Background {
    Background {
        color: ColorToken::Custom(Color::srgb_u8(r, g, b)),
    }
}

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

/// Render the tiered scene once. `scrim_alpha` = 156 (present) or 0 (suppressed);
/// `base` is the base-quad color. Returns the readback pixels.
fn render(scrim_alpha: u8, base: (u8, u8, u8)) -> Vec<u8> {
    let mut app = gpu_render_app(W, H);
    let canvas = solid_canvas(&mut app, CANVAS_RED);

    // Base: opaque full-viewport quad (the "game" background) — QUAD tier.
    let base_e = app
        .world_mut()
        .spawn((Node, Name::new("base"), abs(0.0, 0.0, W as f32, H as f32), opaque(base.0, base.1, base.2)))
        .id();
    // A bordered box: blue fill (QUAD tier) + a thick yellow border (BAND tier),
    // at (10,10) 44×44, 8px border. Fill hole center ~ (32,32); left band ~ (13,32).
    let side = |c: Color| BorderSide {
        color: ColorToken::Custom(c),
        style: LineStyle::Solid,
    };
    let box_e = app
        .world_mut()
        .spawn((
            Node,
            Name::new("bordered_box"),
            abs(10.0, 10.0, 44.0, 44.0).border(8.0),
            opaque(20, 40, 210), // blue fill
            Border {
                top: side(Color::srgb_u8(240, 220, 20)),
                right: side(Color::srgb_u8(240, 220, 20)),
                bottom: side(Color::srgb_u8(240, 220, 20)),
                left: side(Color::srgb_u8(240, 220, 20)), // yellow band
                radius: Corners::ZERO,
            },
        ))
        .id();
    // A base GLYPH: white text in the lower-left — GLYPH tier.
    let text_e = app
        .world_mut()
        .spawn((
            Node,
            Name::new("glyph"),
            abs(8.0, 74.0, 120.0, 40.0),
            Text(String::from("MMMMMM")),
            TextColor(ColorToken::Custom(Color::srgb_u8(245, 245, 245))),
            buiy_core::text::FontSize(34.0),
        ))
        .id();
    // A RASTER (drawing canvas): red, at (120,10) 60×44 — quad-tier interleave.
    let raster_e = app
        .world_mut()
        .spawn((Node, Name::new("raster"), abs(120.0, 10.0, 60.0, 44.0), RasterImage(canvas)))
        .id();

    let mut kids = vec![base_e, box_e, text_e, raster_e];
    // The translucent full-viewport TOP-LAYER scrim (alpha 156 or 0), painted last.
    let scrim_e = app
        .world_mut()
        .spawn((
            Node,
            Name::new("scrim"),
            abs(0.0, 0.0, W as f32, H as f32).top_layer(buiy_core::layout::TopLayer::Popover),
            Background {
                color: ColorToken::Custom(Color::srgba_u8(SCRIM.0, SCRIM.1, SCRIM.2, scrim_alpha)),
            },
        ))
        .id();
    kids.push(scrim_e);
    app.world_mut()
        .spawn((Node, Name::new("root"), Style::default().width_px(W as f32).height_px(H as f32)))
        .add_children(&kids);

    let target = render_to_image(&mut app, W, H);
    spawn_capture_camera_with_msaa(&mut app, target.clone(), Msaa::Sample4);
    // `finish()`/`cleanup()` + one frame FIRST (initializes the RenderApp +
    // its text resources), then drive to font-readiness (also settles the
    // quad/raster/band tiers) — the text_gpu.rs idiom.
    finish_and_run(&mut app, 1);
    wait_for_text_ready(&mut app, 60);
    readback_rgba(&mut app, target)
}

/// The brightest (max-channel-sum) pixel in a WxH-relative box — used to locate a
/// glyph's ink (white text) or a band's color robustly without pixel-perfect coords.
fn brightest_in(pixels: &[u8], x0: u32, y0: u32, x1: u32, y1: u32) -> [u8; 4] {
    let mut best = [0u8; 4];
    let mut best_sum = -1i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = px(pixels, W, x, y);
            let s = p[0] as i32 + p[1] as i32 + p[2] as i32;
            if s > best_sum {
                best_sum = s;
                best = p;
            }
        }
    }
    best
}

/// Most-yellow (R+G high, B low) pixel in a box — locates the yellow band ink.
fn most_yellow_in(pixels: &[u8], x0: u32, y0: u32, x1: u32, y1: u32) -> [u8; 4] {
    let mut best = [0u8; 4];
    let mut best_score = -1i32;
    for y in y0..y1 {
        for x in x0..x1 {
            let p = px(pixels, W, x, y);
            let score = p[0] as i32 + p[1] as i32 - 2 * p[2] as i32;
            if score > best_score {
                best_score = score;
                best = p;
            }
        }
    }
    best
}

fn sum(p: [u8; 4]) -> i32 {
    p[0] as i32 + p[1] as i32 + p[2] as i32
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn per_tier_scrim_bleed_table() {
    // A = scrim present (alpha 156); B = scrim suppressed (alpha 0). Everything
    // else identical, so any per-pixel delta is exactly the scrim's effect.
    let a = render(SCRIM.3, (0, 150, 0)); // green base
    let b = render(0, (0, 150, 0));

    // Tier sample points (chosen from the deterministic layout):
    //  * base QUAD fill: (170, 100) — green base, away from box/text/raster.
    //  * box QUAD fill:  (32, 32)  — inside the border hole (blue).
    //  * band (BAND):    yellow ink in the box's left edge ring x∈[10,20] y∈[20,44].
    //  * glyph (GLYPH):  brightest ink in the text box x∈[8,128] y∈[74,112].
    //  * raster (quad):  (150, 32) — red canvas center.
    let base_a = px(&a, W, 170, 100);
    let base_b = px(&b, W, 170, 100);
    let boxfill_a = px(&a, W, 32, 32);
    let boxfill_b = px(&b, W, 32, 32);
    let band_a = most_yellow_in(&a, 10, 20, 20, 44);
    let band_b = most_yellow_in(&b, 10, 20, 20, 44);
    let glyph_a = brightest_in(&a, 8, 74, 128, 112);
    let glyph_b = brightest_in(&b, 8, 74, 128, 112);
    let raster_a = px(&a, W, 150, 32);
    let raster_b = px(&b, W, 150, 32);

    let row = |name: &str, wa: [u8; 4], wb: [u8; 4]| {
        eprintln!(
            "  {name:<16} WITH={wa:?} (sum {})   WITHOUT={wb:?} (sum {})   Δsum={}",
            sum(wa),
            sum(wb),
            sum(wb) - sum(wa)
        );
    };
    eprintln!("\n===== PER-TIER SCRIM DIM TABLE (Msaa::Sample4, light green base) =====");
    eprintln!("  (WITH = scrim a156 present ; WITHOUT = scrim a0 suppressed)");
    row("QUAD base", base_a, base_b);
    row("QUAD box-fill", boxfill_a, boxfill_b);
    row("BAND border", band_a, band_b);
    row("GLYPH text", glyph_a, glyph_b);
    row("RASTER canvas", raster_a, raster_b);

    // --- DARK-theme iso-luminance: the scrim over the dooduel dark canvas ---
    let da = render(SCRIM.3, DARK_CANVAS);
    let db = render(0, DARK_CANVAS);
    let dbase_a = px(&da, W, 170, 100);
    let dbase_b = px(&db, W, 170, 100);
    eprintln!("\n===== DARK-THEME base QUAD (scrim over dooduel dark canvas 0x1b1e25) =====");
    row("QUAD dark-base", dbase_a, dbase_b);
    eprintln!(
        "  dark base per-channel Δ = [{}, {}, {}]",
        dbase_b[0] as i32 - dbase_a[0] as i32,
        dbase_b[1] as i32 - dbase_a[1] as i32,
        dbase_b[2] as i32 - dbase_a[2] as i32
    );

    // --- The witnesses: quads/raster dim; band/glyph BLEED (don't dim). ---
    // Dimming metric = the DOMINANT channel drop (the scrim tints toward its own
    // dark blue-gray, so a channel-SUM understates a dim on a saturated base).
    const DIM: i32 = 30;
    let drop = |dom: usize, wa: [u8; 4], wb: [u8; 4]| wb[dom] as i32 - wa[dom] as i32;
    assert!(
        drop(1, base_a, base_b) >= DIM,
        "QUAD base must DIM under the scrim: green {} -> {} (WITH={base_a:?} WITHOUT={base_b:?})",
        base_b[1],
        base_a[1]
    );
    assert!(
        drop(2, boxfill_a, boxfill_b) >= DIM,
        "QUAD box-fill must DIM: blue {} -> {} (WITH={boxfill_a:?} WITHOUT={boxfill_b:?})",
        boxfill_b[2],
        boxfill_a[2]
    );
    assert!(
        drop(0, raster_a, raster_b) >= DIM,
        "RASTER must DIM under the scrim (it interleaves in the quad tier): red {} -> {} \
         (WITH={raster_a:?} WITHOUT={raster_b:?})",
        raster_b[0],
        raster_a[0]
    );
    // The ROOT-CAUSE witnesses: the band + glyph tiers BLEED — their ink is
    // essentially UNCHANGED by the scrim (drawn in a later global tier over it).
    const BLEED_TOL: i32 = 18; // near-identical WITH vs WITHOUT
    assert!(
        (sum(band_a) - sum(band_b)).abs() < BLEED_TOL,
        "BAND (border) BLEEDS THROUGH the top-layer scrim (undimmed) — the tiered-render \
         root cause. WITH={band_a:?} WITHOUT={band_b:?}"
    );
    assert!(
        (sum(glyph_a) - sum(glyph_b)).abs() < BLEED_TOL,
        "GLYPH (text) BLEEDS THROUGH the top-layer scrim (undimmed) — the tiered-render \
         root cause. WITH={glyph_a:?} WITHOUT={glyph_b:?}"
    );
}
