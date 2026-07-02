//! GPU-lane end-to-end fixture for the `capture` example's render-to-texture +
//! GPU readback pipeline (audit finding #42). This is the ONLY automated
//! end-to-end exercise of the offscreen screenshot path the README assets are
//! generated through — `examples/capture` itself is a binary (`cargo run -p
//! capture`) with no test, so a startup/readback panic in it would otherwise
//! ship green.
//!
//! `#[ignore]` — needs a real wgpu adapter; runs in the GPU lane
//! (`cargo test --test capture_e2e -- --ignored --test-threads=1`). Vulkan
//! render-to-texture needs no X server, so this is headless on any host with an
//! adapter (RX 6700 XT / RADV here; lavapipe in CI).
//!
//! ## What it exercises
//!
//! The `capture` example's `render_scene` documents itself as mirroring the
//! canonical headless capture stack (`gpu_render_app_with_resolution` /
//! `buiy_core::render::golden::capture_app`), and its `readback_rgba` mirrors
//! `buiy_core::render::golden::readback_rgba_into`. This fixture drives that SAME
//! promoted seam — `capture_app` (build the offscreen painting App) +
//! `capture_to_image` (finish → settle-to-quiescence → render-to-texture → GPU
//! readback) — over a scene built from real, shipping Buiy components (a themed
//! background box + a text node, the example's own primitive vocabulary). So a
//! regression that breaks the example's render/readback path (device init, the
//! offscreen target wiring, the `Readback`/`ReadbackComplete` dance, row-padding
//! strip) reddens here.
//!
//! ## Assertions are rasterizer-AGNOSTIC (the T2.9 trap)
//!
//! A real GPU and lavapipe disagree on exact anti-aliased edge pixels, so this
//! asserts STRUCTURE and NON-VACUITY only — the image came back at the requested
//! dimensions, it is not the blank clear color, and it carries more than one
//! distinct color (the box painted over the clear). It NEVER pins an exact pixel
//! value (no lavapipe-specific encode); exact-pixel residue is the `buiy_verify`
//! Tier-5 golden lane's job, adapter-gated.

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};
use buiy_core::text::{FontSize, Text};

/// The opaque-black clear the capture camera (`capture_to_image`) clears to —
/// the backdrop a "something painted" probe is measured against.
const CLEAR: [u8; 4] = [0, 0, 0, 255];

#[test]
#[ignore = "GPU: run under `cargo test --test capture_e2e -- --ignored` (real adapter / lavapipe)"]
fn capture_example_pipeline_reads_back_a_non_vacuous_image() {
    let (w, h) = (200u32, 120u32);
    let mut app = capture_app(w, h);

    // The example's primitive vocabulary: an absolutely-positioned themed
    // background box (the load-bearing, asserted element below), plus a text node
    // so the glyph atlas → coverage-draw → readback half also flows through the
    // capture path without panicking (its own output is asserted by
    // hello_text_e2e + the GPU text lane, not pinned here).
    let box_e = app
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
                .width_px(140.0)
                .height_px(70.0),
            Background {
                // Typed escape hatch: a guaranteed-non-clear color for the
                // rasterizer-agnostic non-vacuity probe (was a theme-injected token
                // via the removed stringly HashMap).
                color: ColorToken::Custom(Color::srgb(0.20, 0.55, 0.90)),
            },
        ))
        .id();
    let text_e = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Buiy")),
            FontSize(24.0),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[box_e, text_e]);

    // Drive the full render-to-texture + GPU readback (finish → quiescence →
    // paint → readback). No panic across this == the example's capture path runs.
    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());

    // Structure: the readback came back at the requested physical pixel grid.
    assert_eq!(
        img.dimensions(),
        (w, h),
        "capture readback dimensions must match the requested target size"
    );
    let raw = img.as_raw();
    assert_eq!(
        raw.len(),
        (w * h * 4) as usize,
        "readback buffer must be width*height*4 RGBA8 (row padding stripped)"
    );

    // Non-vacuity 1: SOME pixel is not the clear color — the box painted, the
    // frame is not blank. Rasterizer-agnostic (any non-black pixel suffices).
    let painted = raw
        .chunks_exact(4)
        .any(|p| [p[0], p[1], p[2], p[3]] != CLEAR);
    assert!(
        painted,
        "capture must paint the scene, not read back a blank clear frame"
    );

    // Non-vacuity 2: the BOX specifically painted — some pixel is chromatically
    // BLUE-DOMINANT (the fill is srgb(0.20,0.55,0.90), so blue >> red). This
    // gates the box-paint path (Background token resolve → quad draw): a
    // regression that dropped the box while text still rendered would leave only
    // the text's anti-aliased GRAYS (r==g==b, so blue−red == 0) and fail here — a
    // bare "more than one color" check is satisfied by that gray fringe alone
    // (verified by mutation), so it would NOT gate the box. Rasterizer-agnostic:
    // pins no exact value, only the blue>red structure of the fill interior,
    // which the RX and lavapipe agree on.
    let box_painted = raw
        .chunks_exact(4)
        .any(|p| i16::from(p[2]) - i16::from(p[0]) > 40);
    assert!(
        box_painted,
        "readback must contain the blue box fill (a blue-dominant pixel); text \
         anti-aliasing alone is achromatic gray and cannot satisfy this, so a \
         dropped box-paint path reddens here"
    );
}
