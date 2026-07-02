//! GPU reftest (#[ignore]) for R1 transform paint: a UiTransform's 2D affine
//! (rotation / scale) is applied in the quad vertex stage so a transformed fill
//! paints OFF the axis-aligned box. Needs a real wgpu adapter; the headless gate
//! proves the byte layout + the WGSL naga-parse shape, the human runs the GPU
//! lane (`cargo test -p buiy_core -- --ignored --test-threads=1`).
//!
//! SCOPE: pure rotation / pure scale only — within the bridge's faithful TRS
//! range. Skew / general TransformMatrix::Matrix are bounded by the bridge's
//! TRS-only Transform::from_matrix decompose (a separate residual,
//! clip-and-transform.md § B.5), so this fixture deliberately avoids them.

mod support;

/// A pure 2x scale about the box-local top-left grows a 10×10 fill to 20×20, so
/// a pixel ~15px from the top-left — INSIDE the scaled fill but OUTSIDE the
/// unscaled 10×10 box — must be painted. If the affine were dropped (the R1
/// bug), render would paint the original 10×10 axis-aligned box and that pixel
/// would read the clear color. The scaled-grown corner is direction-independent,
/// so this is the unambiguous transform-paint assertion.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn scaled_fill_paints_beyond_unscaled_box() {
    use bevy::prelude::*;
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;

    const W: u32 = 64;
    const H: u32 = 64;

    let mut app = support::gpu_render_app(W, H);

    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // A 10×10 fill at top-left (16,16), scaled 2x about its box-local top-left →
    // occupies x∈[16,36), y∈[16,36). The unscaled box is x∈[16,26), y∈[16,26).
    let child = (
        Node,
        Style::default()
            .absolute()
            .inset(Inset {
                top: Sizing::Length(Length::px(16.0)),
                left: Sizing::Length(Length::px(16.0)),
                ..default()
            })
            .width_px(10.0)
            .height_px(10.0)
            .scale(2.0),
        Background {
            color: ColorToken::Custom(Color::WHITE),
        },
    );
    let c = app.world_mut().spawn(child).id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[c]);

    support::finish_and_run(&mut app, 3);
    let pixels = support::readback_rgba(&mut app, target);
    assert_eq!(pixels.len(), (W * H * 4) as usize);
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * W + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    let clear = px(1, 1);
    assert_eq!(
        clear,
        [0, 0, 0, 255],
        "untouched corner reads the clear color"
    );

    // Deep interior of the scaled-only region (x∈[26,36), y∈[26,36)) — OUTSIDE
    // the unscaled 10×10 box, well clear of the SDF rim. The R1 bug (axis-aligned
    // paint, scale dropped) leaves this at the clear color.
    let scaled_only = px(30, 30);
    assert_ne!(
        scaled_only,
        [0, 0, 0, 255],
        "the 2x scale must paint at (30,30), beyond the unscaled 10×10 box \
         (a dropped affine would leave this at the clear color)"
    );
}

/// A pure 90° rotation about the box-local top-left sweeps a tall thin rect into
/// a horizontal extent the unrotated rect never reaches. The exact swept
/// quadrant depends on the rotation sign, so this asserts the rotated fill
/// paints SOME pixel off the unrotated rect's vertical column (a column the
/// axis-aligned rect would leave at the clear color), which holds for either
/// sign of a 90° turn about the top-left.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn rotated_fill_paints_off_axis() {
    use bevy::prelude::*;
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use std::f32::consts::FRAC_PI_2;

    const W: u32 = 64;
    const H: u32 = 64;

    let mut app = support::gpu_render_app(W, H);

    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // A 4px-wide × 30px-tall rect with its top-left at (32,32) (image center).
    // Unrotated it occupies x∈[32,36), y∈[32,62). Rotated 90° about its top-left
    // it sweeps a ~30px HORIZONTAL extent the unrotated thin column never covers.
    let child = (
        Node,
        Style::default()
            .absolute()
            .inset(Inset {
                top: Sizing::Length(Length::px(32.0)),
                left: Sizing::Length(Length::px(32.0)),
                ..default()
            })
            .width_px(4.0)
            .height_px(30.0)
            .rotate_z(FRAC_PI_2),
        Background {
            color: ColorToken::Custom(Color::WHITE),
        },
    );
    let c = app.world_mut().spawn(child).id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[c]);

    support::finish_and_run(&mut app, 3);
    let pixels = support::readback_rgba(&mut app, target);
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * W + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    // The unrotated thin column is x∈[32,36): any painted pixel with x far from
    // that column (≥10px away horizontally) proves the rotation moved fill onto
    // a horizontal extent the axis-aligned rect never reaches. Scan the rotated
    // sweep band near the pivot row.
    let off_axis_painted = (0..W).any(|x| {
        (x + 10 < 32 || x > 36 + 10) && {
            // sample a few rows around the pivot (y=32) where a 90° turn lands fill
            (28..=36).any(|y| px(x, y) != [0, 0, 0, 255])
        }
    });
    assert!(
        off_axis_painted,
        "the 90° rotation must paint fill off the unrotated thin column \
         (a dropped affine would paint only the axis-aligned x∈[32,36) column)"
    );
}
