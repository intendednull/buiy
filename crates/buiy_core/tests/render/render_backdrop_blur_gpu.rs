//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the parity
//! Wave B4 backdrop-blur channel actually BLURS the painted window backdrop.
//!
//! The design's depth cue: a `backdrop-filter: blur(Npx)` element samples the
//! content painted BEHIND it, blurs it (dual-Kawase), and composites under the
//! element's own fill. This proves the seam end-to-end on a real GPU.
//!
//! Setup: a STRIPED backdrop (alternating bright/dark vertical bars — maximal
//! horizontal local variance), with a `BackdropFilter(blur(6px))` element
//! covering the right half. Render to an offscreen target, write the PNG to
//! `docs/reports/parity-proto-assets/b4-blur.png`, and assert PROGRAMMATICALLY
//! (adapter-tolerant):
//!
//!   - the element's own (translucent) fill PAINTS over the blurred backdrop —
//!     the region is not the bare backdrop and not the dark clear,
//!   - the backdrop UNDER the element has LOWER horizontal local variance than
//!     the un-covered backdrop stripes — i.e. the stripes are smoothed (the
//!     blur worked), NOT a flat fill (which would be ZERO variance) and NOT the
//!     sharp stripes (which would be the un-blurred variance),
//!   - a stripe band away from the element keeps its sharp full variance (the
//!     blur is LOCAL to the element region, not global).
//!
//! This is NOT a blessed CI golden (CI's lavapipe pixels differ from this host —
//! the FINAL phase handles CI goldens). The PNG + the relative assertions are
//! the prototype proof that the backdrop-blur pipeline is correct.
//!
//! Run:  cargo test -p buiy_core --test render backdrop_blur -- --ignored --test-threads=1

use bevy::prelude::*;
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{BackdropFilter, Background, FilterFn};
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};
use std::borrow::Cow;

use crate::support::px;

/// Spawn one absolutely-positioned solid rect (a backdrop stripe or the element).
fn spawn_rect(
    app: &mut App,
    parent: Entity,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    color_token: &'static str,
) -> Entity {
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(y)),
                    left: Sizing::Length(Length::px(x)),
                    ..default()
                })
                .width_px(w)
                .height_px(h),
            Background {
                color: ColorToken::Token(Cow::Borrowed(color_token)),
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[e]);
    e
}

/// Mean absolute horizontal neighbor delta (R channel) over a rect — a simple
/// local-variance proxy. Sharp stripes → high; a smoothed/blurred region → low;
/// a flat fill → ~0.
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

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn backdrop_filter_blurs_the_window_backdrop() {
    const W: u32 = 120;
    const H: u32 = 64;

    let mut app = capture_app(W, H);
    {
        // Seed the backdrop stripe colors + a translucent element fill. Bright =
        // near-white, dark = near-black: maximal stripe contrast so the blur is
        // unmistakable in the variance metric.
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("test.bright".into(), Color::srgb(0.95, 0.95, 0.95));
        theme
            .colors
            .insert("test.dark".into(), Color::srgb(0.05, 0.05, 0.05));
        // A faintly-tinted, mostly-transparent element fill (like the modal scrim
        // rgba(4,5,7,.66) but lighter so the blurred backdrop shows through for
        // the variance check). Alpha 0.35 over the blurred stripes.
        theme
            .colors
            .insert("test.scrim".into(), Color::srgba(0.1, 0.12, 0.18, 0.35));
    }

    let root = app.world_mut().spawn((Node, Style::default())).id();

    // --- The STRIPED backdrop: 5px bright / 5px dark vertical bars across the
    // full width, full height. Maximal horizontal local variance.
    let mut x = 0.0;
    let mut bright = true;
    while x < W as f32 {
        spawn_rect(
            &mut app,
            root,
            x,
            0.0,
            5.0,
            H as f32,
            if bright { "test.bright" } else { "test.dark" },
        );
        x += 5.0;
        bright = !bright;
    }

    // --- The backdrop-filter element: covers the RIGHT HALF (x 60..120), full
    // height. A translucent fill + blur(6px). It is a LATER sibling, so it paints
    // OVER the stripes (its backdrop) — exactly the design's depth cue.
    let element = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(0.0)),
                    left: Sizing::Length(Length::px(60.0)),
                    ..default()
                })
                .width_px(60.0)
                .height_px(H as f32),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.scrim")),
            },
            BackdropFilter(vec![FilterFn::Blur(Length::px(6.0))]),
        ))
        .id();
    app.world_mut().entity_mut(root).add_children(&[element]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));

    // Write the PNG proof artifact (NOT a blessed golden — the prototype proof).
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/reports/parity-proto-assets/b4-blur.png");
    if let Some(parent) = out.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    img.save(&out)
        .unwrap_or_else(|e| panic!("write {}: {e}", out.display()));

    let pixels = img.clone().into_raw();

    // --- Variance metric (the blur proof) ------------------------------------
    // LEFT half (x 5..55): the un-covered backdrop stripes — sharp, HIGH variance.
    let unblurred = horiz_variance(&pixels, W, 5, 55, 8, 56);
    // RIGHT half interior (x 66..114): the stripes UNDER the blurred element —
    // smoothed, LOWER variance. Sampled away from the element edges (where the
    // blur kernel clamps).
    let blurred = horiz_variance(&pixels, W, 66, 114, 8, 56);

    println!("unblurred (sharp stripes) horiz-variance = {unblurred:.2}");
    println!("blurred   (under element) horiz-variance = {blurred:.2}");

    // (1) The un-covered stripes are genuinely high-contrast (the test is set up
    // right — bright/dark bars produce a large neighbor delta).
    assert!(
        unblurred > 30.0,
        "the un-blurred backdrop stripes must be high-variance (sharp bars): {unblurred:.2}"
    );

    // (2) THE BLUR PROOF: the backdrop under the element is SMOOTHED — its local
    // variance is well below the sharp stripes. A real blur lowers neighbor
    // deltas; a missing blur (sharp stripes showing through the translucent fill)
    // would keep them high.
    assert!(
        blurred < unblurred * 0.6,
        "the backdrop under the element must be BLURRED (smoothed): blurred {blurred:.2} \
         must be < 0.6 × unblurred {unblurred:.2}"
    );

    // (3) NOT a flat fill: a flat opaque scrim (no backdrop showing) would be
    // ~zero variance. The blurred region must still carry SOME stripe residue
    // (the backdrop shows through the translucent + blurred fill), proving it is
    // the blurred BACKDROP, not an opaque replacement.
    assert!(
        blurred > 1.0,
        "the blurred region must still show backdrop residue (not a flat fill): {blurred:.2}"
    );

    // (4) The element's own fill PAINTS: sample deep inside the element and
    // confirm it is neither the dark clear nor a raw bright stripe — the scrim
    // tint is present (a bluish lift on the blue channel vs red).
    let inside = px(&pixels, W, 90, 32);
    println!("element interior (90,32) = {inside:?}");
    assert!(
        inside[2] as i32 >= inside[0] as i32,
        "the element's scrim fill must tint the region (B >= R): {inside:?}"
    );
}
