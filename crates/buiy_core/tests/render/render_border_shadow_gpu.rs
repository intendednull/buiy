//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the C6-b
//! per-side BORDER + box SHADOW render channels actually PAINT. Spawns a filled
//! box carrying a `Border` (per-side colors) and a `BoxShadow`, renders to an
//! offscreen target, reads back pixels, and asserts PROGRAMMATICALLY (no pixel
//! golden — adapter-tolerant, so it passes on both this RX 6700 XT host and CI's
//! lavapipe):
//!
//! * the border band paints per-side colors AT the box edge (the top edge reads
//!   the top color, the left edge the left color — distinct),
//! * the box-shadow darkens the region OFFSET behind the box (where, pre-C6-b,
//!   the Shadow primitive + shadow.wgsl were unfed — the audit gap).
//!
//! The render-side complement to the headless `render_border_shadow.rs` (which
//! verifies the extract/pack tier with no adapter). It exercises the shadow
//! pipeline + the band pipeline (per-side selection) end-to-end on a real device.
//!
//! Run:   cargo test -p buiy_core --test render -- --ignored --test-threads=1
//!        (or the documented `cargo test -p buiy_core -j 2 -- --ignored
//!         --test-threads=1` GPU lane)

use bevy::prelude::*;
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::{
    Background, Border, BorderSide, BoxShadow, Corners, LineStyle, Shadow,
};
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};

use crate::support::px;

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn border_band_paints_distinct_per_side_colors_at_the_box_edge() {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut app = capture_app(W, H);
    // Distinct, saturated, channel-separable per-side colors so readback can
    // tell the top edge (green) from the left edge (blue) unambiguously, and
    // both from the red fill. Carried inline as `Custom` (Track B: the test-
    // injection theme HashMap is gone; the color travels on the token itself).
    let fill_color = Color::srgb(0.90, 0.10, 0.10); // red
    let top_color = Color::srgb(0.05, 0.90, 0.05); // green
    let left_color = Color::srgb(0.05, 0.05, 0.90); // blue
    let right_color = Color::srgb(0.90, 0.90, 0.05); // yellow
    let bottom_color = Color::srgb(0.90, 0.05, 0.90); // magenta-ish

    // A 40x40 fill box at (12,12), 6px border on every side. The border band
    // occupies the OUTER 6px ring INSIDE the box: x in [12,18) at the left edge.
    let side = |color: Color| BorderSide {
        color: ColorToken::Custom(color),
        style: LineStyle::Solid,
    };
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(12.0)),
                    left: Sizing::Length(Length::px(12.0)),
                    ..default()
                })
                .width_px(40.0)
                .height_px(40.0)
                .border(6.0),
            Background {
                color: ColorToken::Custom(fill_color),
            },
            Border {
                top: side(top_color),
                right: side(right_color),
                bottom: side(bottom_color),
                left: side(left_color),
                radius: Corners::ZERO,
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));
    let pixels = img.into_raw();

    // The fill box interior (center, (32,32)) is the red fill (inside the border
    // hole). The border box is [12,52); the 6px band is the outer ring, so the
    // content hole is [18,46) — (32,32) is dead-center in the hole.
    let fill = px(&pixels, W, 32, 32);
    assert!(
        fill[0] > 120 && fill[1] < 90 && fill[2] < 90,
        "box interior is the red fill, got {fill:?}"
    );

    // Top edge band: sample the top-middle of the band ring, y=14 (2px into the
    // 6px top band, x=32 mid-width). Must be GREEN-dominant (the top color), not
    // red (fill) and not blue (left).
    let top = px(&pixels, W, 32, 14);
    assert!(
        top[1] > top[0] && top[1] > top[2],
        "top border edge must be the green top color, got {top:?}"
    );

    // Left edge band: sample the left-middle, x=14, y=32. Must be BLUE-dominant
    // (the left color) — proving PER-SIDE selection (a single-color band would
    // paint the same color here as on the top edge).
    let left = px(&pixels, W, 14, 32);
    assert!(
        left[2] > left[0] && left[2] > left[1],
        "left border edge must be the blue left color (per-side), got {left:?}"
    );

    // The two edges are DISTINCT colors — the load-bearing per-side proof.
    assert!(
        top[1] > 120 && left[2] > 120 && top != left,
        "top (green) and left (blue) border edges are distinct per-side colors: \
         top={top:?} left={left:?}"
    );

    // Far outside the box stays the clear backdrop (the band is inside the box).
    let outside = px(&pixels, W, 4, 4);
    assert_eq!(
        outside,
        [0, 0, 0, 255],
        "far outside stays clear, got {outside:?}"
    );
}

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn box_shadow_darkens_the_region_offset_behind_the_box() {
    const W: u32 = 96;
    const H: u32 = 96;
    let mut app = capture_app(W, H);
    // White fill so the box is unambiguous, and a COLORED (blue) shadow so
    // the offset region is unambiguously distinguishable from the (black)
    // clear backdrop in readback — a pure-black shadow over a black clear is
    // indistinguishable in RGB, so the channel-separable blue is what makes
    // "the shadow painted here" an observable, adapter-tolerant assertion
    // (not a brittle near-zero RGB threshold). Carried inline as `Custom`
    // (Track B: the test-injection theme HashMap is gone).
    let fill_color = Color::WHITE;
    let shadow_color = Color::srgba(0.10, 0.20, 0.95, 0.95);

    // A 30x30 white box at (24,24). The shadow is offset (+16,+16) with blur 6,
    // no spread → the shadow box sits at (40,40), behind+below+right of the fill.
    // The region around (60,60) — below-right of the fill box, inside the shadow
    // spread — must darken; the far top-left corner (4,4) stays clear.
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(24.0)),
                    left: Sizing::Length(Length::px(24.0)),
                    ..default()
                })
                .width_px(30.0)
                .height_px(30.0),
            Background {
                color: ColorToken::Custom(fill_color),
            },
            BoxShadow(vec![Shadow {
                color: ColorToken::Custom(shadow_color),
                offset_x: Length::px(16.0),
                offset_y: Length::px(16.0),
                blur: Length::px(6.0),
                spread: Length::px(0.0),
                inset: false,
            }]),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));
    let pixels = img.into_raw();

    // The fill box interior (center, (39,39)) is white. The shadow draws BEHIND
    // it (drawn first), so the box still reads white where it covers the shadow.
    let fill = px(&pixels, W, 39, 39);
    assert!(
        fill[0] > 200 && fill[1] > 200 && fill[2] > 200,
        "fill box interior is white (the box paints over its own shadow), got {fill:?}"
    );

    // The shadow region: offset (+16,+16) puts the shadow box at [40,70). A
    // point at (60,60) is below-right of the fill box (which ends at 54) yet
    // inside the shadow box — it must be the BLUE shadow, NOT the clear backdrop.
    let shadow = px(&pixels, W, 60, 60);
    let is_clear = shadow == [0, 0, 0, 255];
    assert!(
        !is_clear,
        "the box-shadow must paint the offset region behind the box (pre-C6-b \
         the Shadow primitive was unfed — this was the empty clear); got {shadow:?}"
    );
    // The shadow color is blue-dominant: the blue channel exceeds red/green,
    // proving it is the shadow (not white fill leakage, not the clear). This is
    // adapter-tolerant — it asserts a channel ORDER, not exact RGB, so it holds
    // on both this RX 6700 XT host and CI's lavapipe (NOT a brittle pixel golden).
    assert!(
        shadow[2] > shadow[0] && shadow[2] > shadow[1] && shadow[2] > 60,
        "the shadow region is the blue shadow color, not the white fill or clear, got {shadow:?}"
    );

    // Far top-left (opposite the shadow offset) stays the clear backdrop.
    let clear = px(&pixels, W, 4, 4);
    assert_eq!(
        clear,
        [0, 0, 0, 255],
        "far top-left stays clear, got {clear:?}"
    );
}
