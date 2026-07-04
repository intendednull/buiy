//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the F4a general
//! per-raster-anchor paint-order interleave. A `RasterImage` node now paints at
//! its OWN node's stacking position (the same anchor mechanism gradients use), so:
//!
//! - **raster-over-AND-under an overlay** — a canvas paints OVER an earlier in-flow
//!   sibling and UNDER a later one (a non-top-layer overlay positioned over the
//!   canvas now shows, which the pre-F4a "raster draws after ALL quads" fill tier
//!   hid); and
//! - **raster-in-a-top-layer OPAQUE modal** — a raster CHILD of an opaque
//!   top-layer modal panel shows OVER the panel's own bg (the design's
//!   avatar-editor modal, not a full screen).
//!
//! These are PAINT-ORDER proofs by OPAQUE overlap: every layer is fully opaque, so
//! the winning color at a pixel is exactly the topmost layer covering it — an
//! adapter-independent readback (the F1 raster-readback precedent), NOT a
//! perceptual golden, so it needs no pinned-lavapipe blessing. Interior pixels are
//! sampled (away from quad edges) so the quad SDF coverage is 1.0 (no AA), and
//! channel DOMINANCE (not exact bytes) is asserted so the sRGB round-trip of the
//! `ColorToken::Custom` quad fills stays adapter-robust.
//!
//! The boundary this does NOT cross (finding #5): a raster nested in an EFFECT
//! GROUP (an `Opacity < 1.0` / backdrop-filter member) rides the off-screen group
//! pass, which composites quads + glyphs only — a raster there is dropped. So the
//! opaque modal PANEL must carry no effect (a translucent scrim SIBLING is fine).
//! That case is a documented follow-up and is deliberately NOT exercised here.
//!
//! Run: `cargo test -p buiy_core --test render raster_interleave -- --ignored --test-threads=1`

use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style, TopLayer};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::raster::RasterImage;

use crate::support::{
    finish_and_run, gpu_render_app, px, readback_rgba, render_to_image, spawn_capture_camera,
};

/// The canvas texel color (opaque red) — authored directly as the destination
/// `Rgba8UnormSrgb` byte, so it reads back byte-exact (the F1 raster round-trip).
const CANVAS_RED: [u8; 4] = [220, 40, 40, 255];

/// Author an `n×n` solid-`rgba` `Rgba8UnormSrgb` canvas with `RenderAssetUsages::all()`
/// (the `GpuImage` is created AND the main-world `data` survives the render-world
/// clone — the `data.take()` trap the raster module documents).
fn author_solid_canvas(app: &mut App, n: u32, rgba: [u8; 4]) -> Handle<Image> {
    let image = Image::new_fill(
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
    app.world_mut().resource_mut::<Assets<Image>>().add(image)
}

/// `Background(Custom(srgb u8))` — an opaque fill quad the paint-order proof reads.
fn fill(r: u8, g: u8, b: u8) -> Background {
    Background {
        color: ColorToken::Custom(Color::srgb_u8(r, g, b)),
    }
}

/// Absolute box at `(x, y)` sized `w×h` (paint order follows document/spawn order).
fn abs(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style::default()
        .absolute()
        .inset(Inset {
            top: Sizing::Length(Length::px(y)),
            left: Sizing::Length(Length::px(x)),
            ..Default::default()
        })
        .width_px(w)
        .height_px(h)
}

/// `true` iff channel `dom` strictly dominates the other two by `margin` — the
/// adapter-robust "this pixel is layer X" test (X's fill is X-channel-dominant).
fn dominates(px: [u8; 4], dom: usize, margin: i32) -> bool {
    let v = px[dom] as i32;
    (0..3).all(|c| c == dom || v - px[c] as i32 > margin)
}

/// F4a case 1 — raster OVER an earlier sibling AND UNDER a later one. Three
/// absolute siblings fully/partly overlapping, in paint order:
///   under (blue, 50×50) → raster (red canvas, 40×40) → over (green, 20×20 @16,16).
/// Post-F4a the raster splices between them: red where it covers blue, green where
/// the later overlay covers it. PRE-F4a (raster drawn after ALL quads) the green
/// overlay pixel would be RED — the assertion at (26,26) is the RED→GREEN attack.
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn raster_interleave_over_and_under_an_overlay() {
    const W: u32 = 60;
    let mut app = gpu_render_app(W, W);
    let canvas = author_solid_canvas(&mut app, 40, CANVAS_RED);

    let under = app
        .world_mut()
        .spawn((
            Node,
            Name::new("under_blue"),
            abs(0.0, 0.0, 50.0, 50.0),
            fill(40, 60, 220),
        ))
        .id();
    let raster = app
        .world_mut()
        .spawn((
            Node,
            Name::new("canvas"),
            abs(0.0, 0.0, 40.0, 40.0),
            RasterImage(canvas),
        ))
        .id();
    let over = app
        .world_mut()
        .spawn((
            Node,
            Name::new("over_green"),
            abs(16.0, 16.0, 20.0, 20.0),
            fill(40, 200, 60),
        ))
        .id();
    // Spawn order = paint order: under < raster < over.
    app.world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().width_px(W as f32).height_px(W as f32),
        ))
        .add_children(&[under, raster, over]);

    let target = render_to_image(&mut app, W, W);
    spawn_capture_camera(&mut app, target.clone());
    finish_and_run(&mut app, 5); // image upload + raster pipeline compile + draw
    let pixels = readback_rgba(&mut app, target);

    // Non-vacuous: the canvas actually painted its red texel somewhere.
    assert!(
        pixels.chunks_exact(4).any(|p| p == CANVAS_RED),
        "the raster canvas painted (its red texel is present)"
    );

    // THE FIX: the later in-flow overlay (green) paints OVER the raster. Pre-F4a
    // the raster drew after ALL quads, so this pixel was RED — this is the attack.
    let over_px = px(&pixels, W, 26, 26);
    assert!(
        dominates(over_px, 1, 60),
        "a later in-flow overlay must paint OVER the canvas (F4a): (26,26) is \
         green-dominant; pre-F4a (raster drawn last) it was red. got {over_px:?}"
    );
    // The raster paints OVER the earlier sibling (blue): a raster-only pixel is red.
    let raster_px = px(&pixels, W, 8, 8);
    assert!(
        dominates(raster_px, 0, 60),
        "the canvas must paint OVER the earlier blue sibling: (8,8) is \
         red-dominant. got {raster_px:?}"
    );
    // The earlier sibling (blue) still painted where the raster does not cover it.
    let under_px = px(&pixels, W, 45, 20);
    assert!(
        dominates(under_px, 2, 60),
        "the earlier blue sibling paints where the canvas does not reach: \
         (45,20) is blue-dominant. got {under_px:?}"
    );
}

/// F4a case 2 — a raster CHILD of an OPAQUE top-layer modal panel shows OVER the
/// panel. The panel (gray, top_layer(Modal), opaque) escapes to the painters_z
/// tail; its raster child anchors AFTER the panel's own quad, so the canvas paints
/// over the panel bg (the design's avatar-editor modal, not a full screen). The
/// panel is opaque (no effect group), so the raster stays in the flat pass — the
/// boundary the fix respects.
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn raster_interleave_shows_over_an_opaque_top_layer_modal_panel() {
    const W: u32 = 80;
    let mut app = gpu_render_app(W, W);
    let canvas = author_solid_canvas(&mut app, 20, CANVAS_RED);

    // The avatar canvas: a 20×20 raster absolutely placed INSIDE the panel (inset
    // 10 from the panel origin → panel-relative (10,10), so (20,20)..(40,40)).
    let avatar = app
        .world_mut()
        .spawn((
            Node,
            Name::new("avatar"),
            abs(10.0, 10.0, 20.0, 20.0),
            RasterImage(canvas),
        ))
        .id();
    // The OPAQUE modal panel: top-layer, absolute (10,10), a clean 40×40 border box
    // (no padding → content-box width == border box), gray. Its absolute child
    // positions relative to it (the panel is a positioned ancestor).
    let panel = app
        .world_mut()
        .spawn((
            Node,
            Name::new("panel"),
            abs(10.0, 10.0, 40.0, 40.0).top_layer(TopLayer::Modal),
            fill(95, 95, 100),
        ))
        .add_children(&[avatar])
        .id();
    // The screen root (dark blue), which the modal panel escapes to sit over.
    app.world_mut()
        .spawn((
            Node,
            Name::new("screen"),
            Style::default().width_px(W as f32).height_px(W as f32),
            fill(30, 30, 90),
        ))
        .add_children(&[panel]);

    let target = render_to_image(&mut app, W, W);
    spawn_capture_camera(&mut app, target.clone());
    finish_and_run(&mut app, 5);
    let pixels = readback_rgba(&mut app, target);

    assert!(
        pixels.chunks_exact(4).any(|p| p == CANVAS_RED),
        "the avatar canvas painted (its red texel is present)"
    );
    // The raster shows OVER the opaque panel: the avatar box (20..40) is red.
    let avatar_px = px(&pixels, W, 30, 30);
    assert!(
        dominates(avatar_px, 0, 60),
        "the raster child must paint OVER the opaque modal panel (F4a): (30,30) \
         is red-dominant, not the gray panel bg. got {avatar_px:?}"
    );
    // The panel is opaque: its own gray bg shows where the avatar does not cover it
    // (panel 10..50, avatar 20..40 — sample (44,44)) — neutral gray, neither red
    // (the canvas) nor the screen blue.
    let panel_px = px(&pixels, W, 44, 44);
    assert!(
        (60..150).contains(&(panel_px[0] as i32))
            && (panel_px[0] as i32 - panel_px[2] as i32).abs() < 40
            && (panel_px[0] as i32 - panel_px[1] as i32).abs() < 40,
        "the opaque panel's own gray bg paints where the avatar does not reach: \
         (44,44) is neutral gray (not red, not screen blue). got {panel_px:?}"
    );
    // The modal is a PARTIAL top-layer overlay: outside it (panel 10..50), the
    // screen blue shows.
    let screen_px = px(&pixels, W, 60, 60);
    assert!(
        dominates(screen_px, 2, 40),
        "the screen behind the modal shows outside the panel: (60,60) is \
         blue-dominant. got {screen_px:?}"
    );
}
