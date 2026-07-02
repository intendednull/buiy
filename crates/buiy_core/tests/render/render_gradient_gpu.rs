//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the parity
//! Wave B1 background-gradient render channel actually PAINTS. Spawns a box
//! carrying `BackgroundLayers(Linear(150deg, --ac, --ac2))` — the design's
//! logo / accent-button / slider-preview gradient — renders it to an offscreen
//! target and asserts PROGRAMMATICALLY (adapter-tolerant — passes on this RX 6700 XT
//! host AND CI's lavapipe) that the gradient runs in the right DIRECTION with the
//! right COLORS:
//!
//!   - the top-left corner is near `--ac` (#5b86f5, the START stop),
//!   - the bottom-right corner is near `--ac2` (#7fa1f7, the END stop),
//!   - so progressing TL→BR, the red + green channels RISE (91→127, 134→161)
//!     while blue stays ~constant — the 2-stop interpolation along the 150deg
//!     axis (which in y-down fragment space points right-and-down).
//!
//! This is NOT a blessed CI golden (CI's lavapipe pixels differ from this host —
//! the FINAL phase handles CI goldens). The relative corner-channel assertions
//! are the prototype proof that the gradient pipeline is correct.
//!
//! The render-side complement to the headless `render_gradient.rs` (extract tier,
//! no adapter). It exercises the gradient pipeline + `gradient.wgsl` end-to-end.
//!
//! Run:   cargo test -p buiy_core --test render gradient -- --ignored --test-threads=1

use bevy::prelude::*;
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{
    Background, BackgroundLayer, BackgroundLayers, ColorStop, LinearGradient, RadialGradient,
};
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};
use std::borrow::Cow;

use crate::support::px;

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn linear_gradient_paints_ac_to_ac2_top_left_to_bottom_right() {
    const W: u32 = 64;
    const H: u32 = 64;
    let mut app = capture_app(W, H);
    {
        // The design's blue accent ramp: --ac = #5b86f5, --ac2 = #7fa1f7
        // (values.md § 1.2). Both opaque (gradient stops are opaque per § 8).
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("color.accent".into(), Color::srgb_u8(0x5b, 0x86, 0xf5));
        theme.colors.insert(
            "color.accent.lighter".into(),
            Color::srgb_u8(0x7f, 0xa1, 0xf7),
        );
    }

    // A 48x48 gradient box at (8,8) → box spans [8,56). The design's
    // `linear-gradient(150deg, --ac, --ac2)` (logo / accent button / slider
    // preview). No solid `Background` — the gradient IS the fill.
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(8.0)),
                    left: Sizing::Length(Length::px(8.0)),
                    ..default()
                })
                .width_px(48.0)
                .height_px(48.0),
            BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                angle_deg: 150.0,
                stops: vec![
                    ColorStop {
                        color: ColorToken::Token(Cow::Borrowed("color.accent")),
                        position: 0.0,
                    },
                    ColorStop {
                        color: ColorToken::Token(Cow::Borrowed("color.accent.lighter")),
                        position: 1.0,
                    },
                ],
            })]),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));

    let pixels = img.clone().into_raw();

    // Sample the two corners, a few px inside the box edge to avoid the SDF-AA
    // rim (box [8,56); sample at 12 and 52).
    let tl = px(&pixels, W, 12, 12); // near the START stop (--ac)
    let br = px(&pixels, W, 52, 52); // near the END stop (--ac2)

    // Both corners are painted (not the black clear): a non-trivial blue.
    assert!(
        tl[2] > 120 && br[2] > 120,
        "both corners must be painted blue (gradient fill present), TL {tl:?} BR {br:?}"
    );

    // The 150deg gradient axis is (0.5, 0.866) in y-down space (right-and-down),
    // so TL is the START (--ac #5b86f5) and BR is the END (--ac2 #7fa1f7). --ac2
    // is LIGHTER: its red (0x7f=127 vs 0x5b=91) and green (0xa1=161 vs 0x86=134)
    // are higher, blue ~equal (0xf7 vs 0xf5). The DIRECTION proof is the rising
    // R+G channels TL→BR — a relative, adapter-tolerant assertion.
    assert!(
        br[0] > tl[0] + 10,
        "BR red must exceed TL red (gradient runs --ac→--ac2), TL {tl:?} BR {br:?}"
    );
    assert!(
        br[1] > tl[1] + 5,
        "BR green must exceed TL green (gradient runs --ac→--ac2), TL {tl:?} BR {br:?}"
    );

    // The CORNERS match the resolved stop colors (adapter-tolerant ±18/255 over
    // the sRGB encode + SDF-AA). TL ~ #5b86f5, BR ~ #7fa1f7.
    let near = |got: [u8; 4], want: [u8; 3], tol: i32, label: &str| {
        for ch in 0..3 {
            let d = (got[ch] as i32 - want[ch] as i32).abs();
            assert!(
                d <= tol,
                "{label} channel {ch}: got {} want {} (|Δ|={d} > {tol}); px {got:?}",
                got[ch],
                want[ch]
            );
        }
    };
    near(tl, [0x5b, 0x86, 0xf5], 18, "top-left ~ --ac (#5b86f5)");
    near(br, [0x7f, 0xa1, 0xf7], 18, "bottom-right ~ --ac2 (#7fa1f7)");

    // The box CENTER is the midpoint blend (~#6d94f6), between the two corners on
    // each rising channel — a third sample proving the smooth interpolation.
    let mid = px(&pixels, W, 32, 32);
    assert!(
        mid[0] >= tl[0] && mid[0] <= br[0] + 4,
        "center red is between the corners (smooth interp), TL {tl:?} MID {mid:?} BR {br:?}"
    );
}

/// GPU lane (`#[ignore]`): the parity Wave B2 **dotted radial-grid** viewport
/// background actually PAINTS the repeating pattern. Spawns a 66×66 box (3×3 of
/// the 22px tiles) with a solid `Background(#0b0c0e)` (the app bg) UNDER a
/// `BackgroundLayers(Radial(dot_grid(#16181c, 1px, 22px)))` — the design's
/// `radial-gradient(#16181c 1px, transparent 1px); background-size: 22px 22px`
/// (values.md § 7.3). It renders to an offscreen target and asserts PROGRAMMATICALLY
/// that the PATTERN + SPACING are right:
///
///   - a DOT-CENTER pixel (the cell center, 11px into a tile) is the dot color
///     ~#16181c (`color.misc.dot-bg`), clearly brighter than the app bg, and
///   - a BETWEEN-DOTS pixel (a tile corner, ~15px from every dot center) is the
///     bare app bg #0b0c0e (transparent gap → the solid fill shows through).
///
/// Proving a lit dot AND a dark gap one tile apart is the spacing proof. This is
/// NOT a blessed CI golden (lavapipe pixels differ — the FINAL phase handles CI
/// goldens); the PNG + the two sampled pixels are the prototype proof.
///
/// Run:   cargo test -p buiy_core --test render dotted -- --ignored --test-threads=1
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn dotted_grid_paints_lit_dot_and_dark_gap_one_tile_apart() {
    const W: u32 = 80;
    const H: u32 = 80;
    const APP_BG: [u8; 3] = [0x0b, 0x0c, 0x0e]; // color.surface.app
    const DOT: [u8; 3] = [0x16, 0x18, 0x1c]; // color.misc.dot-bg
    const TILE: f32 = 22.0;

    let mut app = capture_app(W, H);
    {
        // A2's dark tokens carry `color.surface.app` (#0b0c0e) + `color.misc.dot-bg`
        // (#16181c); seed them so the tokens resolve (the bare capture theme does
        // not load the full dark ramp).
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("color.surface.app".into(), Color::srgb_u8(0x0b, 0x0c, 0x0e));
        theme
            .colors
            .insert("color.misc.dot-bg".into(), Color::srgb_u8(0x16, 0x18, 0x1c));
    }

    // A 66×66 box (3×3 tiles) at (8,8) → box spans [8,74). Solid app-bg fill UNDER
    // the dotted layer (the gradient pipeline paints layers ON TOP of
    // `Background.color`, so the transparent gaps reveal the app bg).
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(8.0)),
                    left: Sizing::Length(Length::px(8.0)),
                    ..default()
                })
                .width_px(66.0)
                .height_px(66.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("color.surface.app")),
            },
            BackgroundLayers(vec![BackgroundLayer::Radial(RadialGradient::dot_grid(
                ColorToken::Token(Cow::Borrowed("color.misc.dot-bg")),
                1.0,
                TILE,
            ))]),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (W, H));

    let pixels = img.clone().into_raw();

    // Box local (x,y) = window (x+8, y+8). Tiles align to the box top-left, so
    // cell centers sit at local (11+22k, 11+22k). Sample the SECOND cell's center
    // (local 33,33 → window 41,41), comfortably inside the box (away from the
    // edge AA), and a tile CORNER between dots (local 22,22 → window 30,30), which
    // is ~15.5px from every dot center → transparent gap → bare app bg.
    let dot_center = px(&pixels, W, 41, 41);
    let between = px(&pixels, W, 30, 30);

    let near = |got: [u8; 4], want: [u8; 3], tol: i32, label: &str| {
        for ch in 0..3 {
            let d = (got[ch] as i32 - want[ch] as i32).abs();
            assert!(
                d <= tol,
                "{label} channel {ch}: got {} want {} (|Δ|={d} > {tol}); px {got:?}",
                got[ch],
                want[ch]
            );
        }
    };

    // The between-dots gap is the bare app bg (#0b0c0e) — the transparent gap
    // reveals the solid fill (tight tolerance; this is a flat fill, no gradient).
    near(between, APP_BG, 6, "between-dots ~ app bg (#0b0c0e)");

    // The dot center is the dot color (#16181c). #16181c (rgb 22,24,28) is
    // BRIGHTER than the app bg (#0b0c0e, rgb 11,12,14) on every channel, so the
    // lit dot is unambiguously distinct from the gap. (A 1px-radius dot's center
    // pixel is sampled ~0.7px off the exact center, so it reaches near — not
    // exactly — full dot color; ±6 over the analytic 1px ramp + sRGB encode.)
    near(dot_center, DOT, 6, "dot-center ~ dot color (#16181c)");
    assert!(
        dot_center[0] > between[0] && dot_center[1] > between[1] && dot_center[2] > between[2],
        "the lit dot must be brighter than the gap on every channel \
         (pattern present + spaced), dot {dot_center:?} gap {between:?}"
    );
}
