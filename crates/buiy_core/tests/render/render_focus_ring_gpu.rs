//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the C6-a
//! OUTLINE render channel actually PAINTS. Spawns a filled box carrying an
//! `Outline` (the focus-ring shape — `Highlight`, Solid, 2px, 2px offset),
//! renders it to an offscreen target, reads back the pixels, and asserts a
//! high-contrast ring band paints in the gap OUTSIDE the fill box (where,
//! pre-C6, nothing was drawn — the audit's structurally-invisible-focus bug).
//!
//! This is the render-side complement to the headless `render_focus_ring.rs`
//! (which verifies the extract/pack tier + the `lower_focus_ring` lowering with
//! no adapter). It exercises the band pipeline + `band.wgsl` end-to-end on a
//! real device.
//!
//! Run:   cargo test -p buiy_core --test render -- --ignored --test-threads=1
//!        (or the documented `cargo test -p buiy_core -j 2 -- --ignored
//!         --test-threads=1` GPU lane)

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::{Background, LineStyle, Outline};
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};

use crate::support::px;

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn outline_band_paints_a_ring_outside_the_fill_box() {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut app = capture_app(W, H);
    // A distinct fill (red) so the fill box and the ring (the default theme's
    // blue focus ring) are unambiguously different colors in readback. Carried
    // inline as `Custom` (Track B: the test-injection theme HashMap is gone).
    let fill_color = Color::srgb(0.90, 0.10, 0.10);

    // A 24x24 fill box at (20,20). The Outline is 2px wide, 2px offset → the ring
    // occupies the 4px gap from the border box outward: x in [16,20) is the ring
    // (offset gap + stroke) on the left, the box interior is [20,44).
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(20.0)),
                    left: Sizing::Length(Length::px(20.0)),
                    ..default()
                })
                .width_px(24.0)
                .height_px(24.0),
            Background {
                color: ColorToken::Custom(fill_color),
            },
            // The framework focus-ring SHAPE (`FocusRing`, Solid, 2px, 2px
            // offset). Painted directly here to isolate the render channel; the
            // `lower_focus_ring` lowering that inserts this exact `Outline` is
            // covered headless.
            Outline {
                color: ColorToken::FocusRing,
                style: LineStyle::Solid,
                width: Length::px(2.0),
                offset: Length::px(2.0),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));
    let pixels = img.into_raw();

    // The fill box interior (center, (32,32)) is the red fill.
    let fill = px(&pixels, W, 32, 32);
    assert!(
        fill[0] > 120 && fill[1] < 80 && fill[2] < 80,
        "fill box interior must be the red fill, got {fill:?}"
    );

    // The ring band sits OUTSIDE the fill box: the stroke is at distance
    // (offset..offset+width) = (2..4) px outside the border box. At y=32 (box
    // mid-height), x=17 is ~3px left of the box's left edge (x=20) — inside the
    // ring band. Pre-C6 this was the opaque-black clear (nothing painted there);
    // now it is the high-contrast ring (the default Highlight = blue).
    let ring = px(&pixels, W, 17, 32);
    let is_clear = ring == [0, 0, 0, 255];
    assert!(
        !is_clear,
        "the focus ring must paint OUTSIDE the fill box (pre-C6 this was the \
         empty clear — the structurally-invisible-focus bug); got {ring:?}"
    );
    // And it must be the ring color (blue-dominant), NOT the red fill bleeding
    // out — proving it is the outline band, not the box.
    assert!(
        ring[2] > ring[0],
        "the ring band must be the Highlight ring color (blue-dominant), not \
         the red fill; got {ring:?}"
    );

    // The region BEYOND the ring (well outside, e.g. (8,32)) stays clear — the
    // ring is a thin band, not a flood fill.
    let outside = px(&pixels, W, 8, 32);
    assert_eq!(
        outside,
        [0, 0, 0, 255],
        "far outside the ring stays the clear backdrop, got {outside:?}"
    );
}
