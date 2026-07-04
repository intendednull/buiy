//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the F4b
//! shape/decoration render fixes actually PAINT correctly. Each test renders to an
//! offscreen target, reads back pixels, and asserts PROGRAMMATICALLY — adapter-
//! tolerant property probes (no stored pixel golden), so they pass on both a real
//! host GPU and CI's lavapipe, and they catch the ACTUAL bug (a dash with no gap,
//! a square shadow corner, an un-faded alpha, a lens instead of a pill) rather
//! than a byte diff only comparable on the pinned rasterizer.
//!
//! The render-side complement to the headless `render_border_shadow.rs` (extract /
//! pack / routing) and the standing Tier-4 SDF cross-check (rounded-fill corners).
//!
//! Run: `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`.

use bevy::prelude::*;
use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::{
    Background, Border, BorderSide, BoxShadow, Corners, LineStyle, QuadAlpha, Radius, Shadow,
};
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};

use crate::support::px;

/// F4b-3: a dashed border paints DASHES (ink + gaps), not a solid ring. Samples
/// the straight middle of the top band (where the per-side quadrant selects the
/// top's dashed flag) across x, and asserts BOTH an inked (dash) and a clear (gap)
/// sample exist — a solid border would ink every sample.
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn dashed_border_paints_alternating_dashes_and_gaps() {
    const W: u32 = 80;
    const H: u32 = 44;
    let mut app = capture_app(W, H);
    let dashed = BorderSide {
        color: ColorToken::Custom(Color::srgb(0.05, 0.90, 0.05)), // bright green ink
        style: LineStyle::Dashed,
    };
    // A 60×24 box at (10,10), 2px dashed border. Top band = y in [10,12); the
    // straight top middle is x in [~30,50] (where ay >= ax picks the TOP side).
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(10.0)),
                    left: Sizing::Length(Length::px(10.0)),
                    ..default()
                })
                .width_px(60.0)
                .height_px(24.0)
                .border(2.0),
            Background {
                color: ColorToken::Custom(Color::srgb(0.1, 0.1, 0.12)),
            },
            Border {
                top: dashed.clone(),
                right: dashed.clone(),
                bottom: dashed.clone(),
                left: dashed,
                radius: Corners::all(Radius::circular(4.0)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    let pixels = img.into_raw();
    let is_ink = |p: [u8; 4]| p[1] > 100 && p[1] > p[0] && p[1] > p[2];
    let is_gap = |p: [u8; 4]| p[0] < 60 && p[1] < 60 && p[2] < 60;

    let mut saw_ink = false;
    let mut saw_gap = false;
    // Sample across the straight top-band middle (x 30..50) at y=11 (mid-band).
    for x in 30..50 {
        let p = px(&pixels, W, x, 11);
        saw_ink |= is_ink(p);
        saw_gap |= is_gap(p);
    }
    assert!(
        saw_ink,
        "the dashed top border painted at least one DASH (green ink)"
    );
    assert!(
        saw_gap,
        "the dashed top border left at least one GAP (clear) — not a solid ring"
    );
}

/// F4b-6: a CRISP (zero-blur) shadow on a ROUNDED caster paints a ROUNDED shadow
/// (the 3D-press "sticker" edge), not a rectangular blur that pokes past the box.
/// Probes the shadow body (inked) and a shadow-box corner OUTSIDE the rounded
/// silhouette (clear — a square shadow would ink it).
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn crisp_rounded_shadow_rounds_its_corners() {
    const W: u32 = 80;
    const H: u32 = 110;
    let mut app = capture_app(W, H);
    // A 40×40 CIRCLE (radius 20, borderless-rounded) white caster at (20,20), with
    // a crisp blue shadow offset +40 down (so the shadow — box (20,60)..(60,100),
    // center (40,80), radius 20 — sits fully BELOW the caster, not occluded by it).
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
                .width_px(40.0)
                .height_px(40.0),
            Background {
                color: ColorToken::Custom(Color::WHITE),
            },
            // A borderless radius rounds the fill AND (F4b-6) routes the shadow to
            // the rounded pipeline (its caster_radius reads Border.radius).
            Border {
                top: BorderSide {
                    color: ColorToken::Custom(Color::NONE),
                    style: LineStyle::None,
                },
                right: BorderSide {
                    color: ColorToken::Custom(Color::NONE),
                    style: LineStyle::None,
                },
                bottom: BorderSide {
                    color: ColorToken::Custom(Color::NONE),
                    style: LineStyle::None,
                },
                left: BorderSide {
                    color: ColorToken::Custom(Color::NONE),
                    style: LineStyle::None,
                },
                radius: Corners::all(Radius::circular(20.0)),
            },
            BoxShadow(vec![Shadow {
                color: ColorToken::Custom(Color::srgba(0.10, 0.20, 0.95, 0.95)),
                offset_x: Length::px(0.0),
                offset_y: Length::px(40.0),
                blur: Length::px(0.0), // CRISP
                spread: Length::px(0.0),
                inset: false,
            }]),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    let pixels = img.into_raw();

    // Non-vacuous + shadow body: the shadow center (40,80) is deep inside the
    // circle → blue-dominant ink.
    let center = px(&pixels, W, 40, 80);
    assert!(
        center[2] > 100 && center[2] > center[0] && center[2] > center[1],
        "the crisp rounded shadow paints its body (blue), got {center:?}"
    );
    // Rounded silhouette: the shadow BOX's top-left corner (23,63) is OUTSIDE the
    // radius-20 circle (dist from (40,80) ≈ 24 > 20) → CLEAR. A square shadow (the
    // pre-F4b behavior) would ink this corner.
    let corner = px(&pixels, W, 23, 63);
    assert!(
        corner[0] < 45 && corner[1] < 45 && corner[2] < 60,
        "the shadow corner is rounded away (clear), not a square blur, got {corner:?}"
    );
}

/// F4b-5: `QuadAlpha` fades a fill quad's alpha (the composite-free particle
/// fade). A half-alpha blue quad over an opaque red backdrop blends to a purple —
/// both channels present. A pure-opaque (un-faded) quad would read pure blue
/// (red ≈ 0); this proves the alpha multiply reached the fragment.
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn quad_alpha_blends_the_fill_over_the_backdrop() {
    const W: u32 = 40;
    const H: u32 = 40;
    let mut app = capture_app(W, H);
    // Backdrop: a full-view opaque RED quad.
    let backdrop = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(W as f32).height_px(H as f32),
            Background {
                color: ColorToken::Custom(Color::srgb(0.90, 0.05, 0.05)),
            },
        ))
        .id();
    // Faded: a full-view opaque BLUE quad at QuadAlpha(0.5), painted OVER the
    // backdrop (document order → on top).
    let faded = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(0.0)),
                    left: Sizing::Length(Length::px(0.0)),
                    ..default()
                })
                .width_px(W as f32)
                .height_px(H as f32),
            Background {
                color: ColorToken::Custom(Color::srgb(0.05, 0.05, 0.95)),
            },
            QuadAlpha(0.5),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[backdrop, faded]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    let pixels = img.into_raw();
    let center = px(&pixels, W, 20, 20);
    // A blend: BOTH the red backdrop and the blue fill show through (the fade let
    // the backdrop bleed). A fully-opaque blue would read red ≈ 0.
    assert!(
        center[0] > 60,
        "the red backdrop bleeds through the faded blue quad (alpha < 1), got {center:?}"
    );
    assert!(
        center[2] > 60,
        "the blue fill still paints (a blend, not fully transparent), got {center:?}"
    );
}

/// F4b-2: a WIDE bordered pill with a full radius PILLS (circular ends), it does
/// NOT draw the pointed radius LENS the pre-F4b band did (TL.x used for every
/// corner). Probes the straight top band well inside the ends (inked — the lens
/// would have carved it away) plus a rounded far corner (clear).
#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn wide_bordered_pill_pills_not_lens() {
    const W: u32 = 100;
    const H: u32 = 40;
    let mut app = capture_app(W, H);
    let ink = BorderSide {
        color: ColorToken::Custom(Color::srgb(0.05, 0.90, 0.05)), // green
        style: LineStyle::Solid,
    };
    // An 80×20 pill at (10,10), 3px border, radius Full (9999 → clamps to rx=40,
    // ry=10; min(rx,ry)=10 → a semicircle-ended pill). The straight top runs for
    // local.x in ±(40−10) = ±30 → x in [20,80]. The lens (radius 40) would round
    // from x<40, carving away the top band there.
    let widget = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(10.0)),
                    left: Sizing::Length(Length::px(10.0)),
                    ..default()
                })
                .width_px(80.0)
                .height_px(20.0)
                .border(3.0),
            Background {
                color: ColorToken::Custom(Color::srgb(0.1, 0.1, 0.12)),
            },
            Border {
                top: ink.clone(),
                right: ink.clone(),
                bottom: ink.clone(),
                left: ink,
                radius: Corners::all(Radius::circular(9999.0)),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[widget]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    let pixels = img.into_raw();
    let is_ink = |p: [u8; 4]| p[1] > 100 && p[1] > p[0] && p[1] > p[2];

    // The straight top band reaches x=25 (well inside the ±30 straight region) at
    // y=11 (mid the 3px top band [10,13)). The lens would have curved it away here.
    let straight_top = px(&pixels, W, 25, 11);
    assert!(
        is_ink(straight_top),
        "the pill's straight top band reaches x=25 (a lens would carve it), got {straight_top:?}"
    );
    // The extreme box corner (11,11) is OUTSIDE the radius-10 rounded end → clear.
    let corner = px(&pixels, W, 11, 11);
    assert!(
        corner[0] < 60 && corner[1] < 60 && corner[2] < 60,
        "the pill end is rounded (corner clear), got {corner:?}"
    );
}
