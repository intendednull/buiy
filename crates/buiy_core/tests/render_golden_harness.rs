//! Golden-image harness (gate #2). The triad config + perceptual diff are
//! device-free and gating; the actual capture needs a wgpu adapter and is
//! #[ignore]. Spec: verification.md § 4.

mod support;

use buiy_core::render::golden::{GoldenConfig, perceptual_diff};

#[test]
fn golden_config_pins_the_flake_triad() {
    // All three nondeterminism sources must be pinned together (verification § 4.3).
    let cfg = GoldenConfig::deterministic();
    assert!(cfg.fixed_clock, "fixed clock");
    assert!(cfg.wait_for_fonts, "font-load sync");
    assert!(cfg.warm_atlas, "atlas warmup");
}

#[test]
fn identical_images_diff_to_zero() {
    let a = vec![10u8, 20, 30, 255, 40, 50, 60, 255];
    assert_eq!(perceptual_diff(&a, &a), 0.0);
}

#[test]
fn differing_images_diff_above_zero() {
    let a = vec![0u8, 0, 0, 255];
    let b = vec![255u8, 255, 255, 255];
    assert!(perceptual_diff(&a, &b) > 0.0);
}

#[test]
fn accept_flag_routes_through_config() {
    // The --accept workflow is human-curated: never an automatic overwrite
    // (verification § 4.4). The flag is off by default.
    let cfg = GoldenConfig::deterministic();
    assert!(
        !cfg.accept,
        "golden updates require explicit, human-curated --accept"
    );
}

// Needs a wgpu adapter (real GPU or lavapipe): paints two overlapping
// semitransparent quads into an offscreen render-target image, reads the pixels
// back on the GPU, and asserts the SrcOver composite. Headless CI without a GPU
// panics at adapter init, so this runs only with --ignored.
//
// Run locally with:
//   cargo test -p buiy_core --test render_golden_harness -- --ignored --nocapture
//
// SCOPE (this fixture proves the capture→readback infra, NOT the stored-PNG
// golden machinery): it asserts INLINE against a computed expected pixel, not
// against a stored PNG. The `--accept` stored-golden workflow (the `image`
// crate, `tests/goldens/`, `GoldenConfig.accept`, and the per-fixture tolerance
// budget owned by buiy-verification-design) is DEFERRED. `GoldenConfig::
// deterministic()` is touched so its triad gates this fixture's contract.
//
// HONEST CLAIM: the v1 `BuiyNode` issues a single flat `SrcOver` draw of all
// instances, so this proves *flat-composite* capture (two independent fills
// blended by the GPU blend state in linear space). It does NOT prove pillar-6
// group-opacity isolation (overlapping children under `Opacity < 1` compositing
// once as a unit) — that is the effect-compositor area and rides its own
// fixture (`render_compositor_gpu::group_opacity_overlap_is_single_layer_at_half`).
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn overlapping_semitransparent_fills_match_golden() {
    use bevy::prelude::*;
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use std::borrow::Cow;

    // Pin the flake triad for the fixture's contract (device-free gating above).
    let _cfg = GoldenConfig::deterministic();

    const W: u32 = 64;
    const H: u32 = 64;

    // Two known semitransparent sRGB fills. The extract pre-linearizes color
    // (`LinearRgba::from`), the shader outputs straight-alpha linear color, and
    // the pipeline blends `ALPHA_BLENDING` (SrcOver) into the `Rgba8UnormSrgb`
    // target — so the GPU blends in LINEAR space and re-encodes to sRGB8 on
    // write. We mirror that exact chain on the CPU below to form the expectation.
    let fill_a = Color::srgba(0.90, 0.10, 0.10, 0.50); // semitransparent red
    let fill_b = Color::srgba(0.10, 0.10, 0.90, 0.50); // semitransparent blue

    let mut app = support::gpu_render_app(W, H);

    // Known semitransparent theme tokens (set BEFORE finish so the first extract
    // resolves them). `Theme.colors` is a public map.
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.fill.a".into(), fill_a);
        theme.colors.insert("test.fill.b".into(), fill_b);
    }

    // Offscreen target + capture camera (opaque-black clear).
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // Two absolutely-positioned 40x40 square (radius 0) fills that overlap.
    // A: x∈[8,48), y∈[8,48).  B: x∈[20,60), y∈[20,60).
    // Overlap: x∈[20,48), y∈[20,48); sampled deep-interior at (34,34) — ≥6px
    // inside every edge so the SDF anti-aliased rim never reaches it.
    let abs_at = |left: f32, top: f32, token: &'static str| {
        (
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(top)),
                    left: Sizing::Length(Length::px(left)),
                    ..default()
                })
                .width_px(40.0)
                .height_px(40.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed(token)),
            },
        )
    };
    // A spawned first → earlier in painters_z; B second → painted ON TOP (the
    // SrcOver source). Both under one root so a single StackingContext forms.
    let a = app.world_mut().spawn(abs_at(8.0, 8.0, "test.fill.a")).id();
    let b = app
        .world_mut()
        .spawn(abs_at(20.0, 20.0, "test.fill.b"))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[a, b]);

    // Drive frames: finish materializes the device + pipeline; ≥2-3 frames let
    // layout → extract → prepare upload and the graph paint settle before the
    // readback poll. `finish_and_run` finishes; `readback_rgba` polls further.
    support::finish_and_run(&mut app, 3);

    let pixels = support::readback_rgba(&mut app, target);
    assert_eq!(
        pixels.len(),
        (W * H * 4) as usize,
        "readback returns un-padded RGBA8 for the whole {W}x{H} image"
    );

    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * W + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    // The clear color is opaque black; a corner pixel (outside both fills) must
    // read it. (a) Capture works: at least one painted pixel differs from clear.
    let clear = px(1, 1);
    assert_eq!(
        clear,
        [0, 0, 0, 255],
        "untouched corner reads the opaque-black clear"
    );
    let any_painted = (0..W * H).any(|n| {
        let i = (n * 4) as usize;
        pixels[i..i + 4] != [0, 0, 0, 255]
    });
    assert!(
        any_painted,
        "capture produced non-clear pixels (the frame is not uniformly the clear color)"
    );

    // Evidence (visible under --nocapture).
    let a_only = px(12, 12); // inside A, outside B
    let b_only = px(54, 54); // inside B, outside A
    let overlap = px(34, 34); // deep interior of the overlap
    println!("clear   (1,1)   = {clear:?}");
    println!("A-only  (12,12) = {a_only:?}");
    println!("B-only  (54,54) = {b_only:?}");
    println!("overlap (34,34) = {overlap:?}");

    // (b) The overlap pixel equals the expected SrcOver composite. Compute it on
    // the CPU through the SAME linear-blend chain the GPU runs:
    //   clear  = (0,0,0) opaque black, linear.
    //   A over clear : c1 = a_a * A_lin                 (dst == 0)
    //   B over c1    : c  = b_a * B_lin + (1 - b_a)*c1  (straight-alpha SrcOver)
    // then encode c linear → sRGB8 (what the Rgba8UnormSrgb target stores).
    let a_lin = LinearRgba::from(fill_a);
    let b_lin = LinearRgba::from(fill_b);
    let (aa, ba) = (a_lin.alpha, b_lin.alpha);
    let blend = |a: f32, b: f32| ba * b + (1.0 - ba) * (aa * a);
    let expected_lin = LinearRgba::new(
        blend(a_lin.red, b_lin.red),
        blend(a_lin.green, b_lin.green),
        blend(a_lin.blue, b_lin.blue),
        1.0,
    );
    let expected_srgb = Srgba::from(expected_lin);
    let expected = [
        (expected_srgb.red * 255.0).round() as u8,
        (expected_srgb.green * 255.0).round() as u8,
        (expected_srgb.blue * 255.0).round() as u8,
        255u8,
    ];
    println!("expected SrcOver composite = {expected:?}");

    // Direct per-channel abs-diff tolerance (a few LSB absorbs sRGB-encode
    // rounding + sub-LSB SDF/AA float jitter — the same jitter `perceptual_diff`
    // exists to tolerate). The RGB channels carry the composite; alpha is opaque.
    const TOL: i32 = 4;
    for ch in 0..3 {
        let got = overlap[ch] as i32;
        let want = expected[ch] as i32;
        assert!(
            (got - want).abs() <= TOL,
            "overlap channel {ch}: got {got}, expected {want} (±{TOL}) — \
             the GPU SrcOver composite of the two semitransparent fills must \
             match the linear-space blend. full pixel got={overlap:?} expected={expected:?}"
        );
    }

    // Cross-check via the device-free `perceptual_diff` metric on the single
    // overlap pixel vs. its expectation: it must be near zero (well under any
    // plausible tolerance budget). This exercises the gate-#2 metric against
    // real captured pixels, not synthetic buffers.
    let diff = perceptual_diff(&overlap, &expected);
    assert!(
        diff < 0.02,
        "perceptual_diff(overlap, expected) = {diff} must be ~0 (got pixel \
         {overlap:?}, expected {expected:?})"
    );
}
