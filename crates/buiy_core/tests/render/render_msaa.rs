//! GPU (`#[ignore]`) regression tests for multisampled views. A bare `Camera2d`
//! defaults to `Msaa::Sample4`, so the view's color attachments are 4-sample;
//! every Buiy pipeline that draws into the VIEW pass (quad, glyph, the root-group
//! composite) must be specialized to the view's sample count or wgpu rejects the
//! first `set_pipeline` with "Render pipeline targets are incompatible with
//! render pass" — the `cargo run -p hello_button` startup crash. The per-view
//! specialization lives in `pipeline::prepare_buiy_view_pipelines` (quad/glyph)
//! and `compositor::prepare_effect_groups` (the window composite); the
//! off-screen group targets stay single-sampled (`group_target_descriptor`,
//! `sample_count: 1`), so the group/nested pipelines keep `samples: 1`.
//!
//! Run locally with:
//!   cargo test -p buiy_core --test render_msaa -- --ignored --test-threads=1

use bevy::prelude::*;
use bevy::render::view::Msaa;

// The hello_button startup-crash regression: a default-MSAA (Sample4) view +
// one painted node must complete a frame and actually paint — not panic with
// "Render pipeline targets are incompatible with render pass" at the first
// `set_pipeline` (the count=1 pipeline vs the 4x attachment mismatch).
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn msaa4_view_paints_one_node_without_pipeline_mismatch() {
    use buiy_core::Node;
    use buiy_core::layout::Style;
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use std::borrow::Cow;

    const W: u32 = 32;
    const H: u32 = 32;
    let mut app = crate::support::gpu_render_app(W, H);
    let target = crate::support::render_to_image(&mut app, W, H);
    // Sample4 — the bare-`Camera2d` default; do NOT force Msaa::Off.
    crate::support::spawn_capture_camera_with_msaa(&mut app, target.clone(), Msaa::Sample4);
    app.world_mut().spawn((
        Node,
        // 40×40 opaque fill over the 32×32 view → every pixel painted.
        Style::default().width_px(40.0).height_px(40.0),
        Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
        },
    ));
    crate::support::finish_and_run(&mut app, 3);

    // The frame completed (no targets-incompatible validation panic) AND the
    // node painted: the readback is not uniformly the opaque-black clear.
    //
    // READBACK PATH NOTE: under `Msaa::Sample4` the `RenderTarget::Image` handle
    // is Bevy's single-sampled RESOLVE target — the Core2d graph resolves the 4x
    // samples into it before this readback copies it out. So `readback_rgba`
    // reads post-resolve pixels; if that resolve path ever stopped firing for
    // offscreen targets, this assertion would fail loudly (all-clear), never
    // false-green.
    let pixels = crate::support::readback_rgba(&mut app, target);
    assert_eq!(pixels.len(), (W * H * 4) as usize, "full RGBA8 readback");
    let clear = [0u8, 0, 0, 255];
    assert!(
        pixels.chunks_exact(4).any(|px| px != clear),
        "BuiyNode::run painted non-clear pixels through the 4x view pass"
    );
}

// The compositor-path MSAA regression: the root-group composite (step 2b) draws
// into the WINDOW pass attachment, which is 4-sample under Msaa::Sample4 — the
// composite pipeline must be view-sample-specialized too (the group's own
// off-screen passes stay 1x). Same fixture as render_compositor_gpu.rs's
// `group_opacity_overlap_is_single_layer_at_half`, under Sample4: two
// overlapping opaque-red children inside an `Opacity(0.5)` parent; the overlap
// pixel still equals the single-layer `composite_src_over` expectation.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn msaa4_group_opacity_composite_matches_src_over() {
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};
    use buiy_core::render::compositor::composite_src_over;
    use std::borrow::Cow;

    const W: u32 = 64;
    const H: u32 = 64;

    let red = Color::srgb(0.9, 0.05, 0.05); // an OPAQUE red (alpha 1)

    let mut app = crate::support::gpu_render_app(W, H);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.red".into(), red);
    }

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera_with_msaa(&mut app, target.clone(), Msaa::Sample4);

    // Two absolutely-positioned 32x32 opaque-red children that OVERLAP, both
    // children of one `Opacity(0.5)` parent (so they share its group).
    // A: x∈[8,40), y∈[8,40).  B: x∈[20,52), y∈[20,52).
    // Overlap: x∈[20,40), y∈[20,40); sampled deep-interior at (30,30) — far
    // from every edge, so the 4x resolve cannot blend a boundary into it.
    let child = |left: f32, top: f32| {
        (
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(top)),
                    left: Sizing::Length(Length::px(left)),
                    ..default()
                })
                .width_px(32.0)
                .height_px(32.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.red")),
            },
        )
    };
    let a = app.world_mut().spawn(child(8.0, 8.0)).id();
    let b = app.world_mut().spawn(child(20.0, 20.0)).id();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(0.5)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[a, b]);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent]);

    crate::support::finish_and_run(&mut app, 4);

    let pixels = crate::support::readback_rgba(&mut app, target);
    assert_eq!(pixels.len(), (W * H * 4) as usize);
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * W + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    // Expectation via the CPU port: an opaque-red group sample at 0.5 over the
    // opaque-black clear, encoded linear→sRGB8. Interior pixels are identical
    // under MSAA (multisampling only affects primitive edges).
    let red_lin = LinearRgba::from(red);
    let black_lin = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
    let expected_lin = composite_src_over(red_lin, black_lin, 0.5);
    let expected_srgb = Srgba::from(expected_lin);
    let expected = [
        (expected_srgb.red * 255.0).round() as u8,
        (expected_srgb.green * 255.0).round() as u8,
        (expected_srgb.blue * 255.0).round() as u8,
        255u8,
    ];

    let clear = px(1, 1);
    let a_only = px(12, 12);
    let overlap = px(30, 30);
    println!("clear   (1,1)   = {clear:?}");
    println!("A-only  (12,12) = {a_only:?}  (expected {expected:?})");
    println!("overlap (30,30) = {overlap:?}  (expected {expected:?})");

    assert_eq!(clear, [0, 0, 0, 255], "untouched corner reads the clear");

    const TOL: i32 = 4;
    // The overlap is 50%-red-over-black — composited ONCE through the 4x window
    // pass, not doubled, not dropped.
    for ch in 0..3 {
        let got = overlap[ch] as i32;
        let want = expected[ch] as i32;
        assert!(
            (got - want).abs() <= TOL,
            "overlap channel {ch}: got {got}, expected {want} (±{TOL}); the root \
             composite into the 4x window pass must apply the group once at 0.5. \
             full overlap={overlap:?} expected={expected:?}"
        );
    }
    // A non-overlap red pixel equals the same 0.5 red (no double-darken).
    for ch in 0..3 {
        assert!(
            (a_only[ch] as i32 - overlap[ch] as i32).abs() <= TOL,
            "non-overlap red ({a_only:?}) must equal the overlap ({overlap:?}) — \
             both are the group composited once at 0.5"
        );
    }
}
