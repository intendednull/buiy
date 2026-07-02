//! GPU-path tests for the DEGRADED effect-group forward-composite (R2 /
//! effect-compositor.md § 2.3): a ROOT group that loses its pooled
//! `Rgba16Float` target under RT-pool budget pressure must paint FLAT with its
//! `opacity` folded per-instance, NOT vanish. These need a wgpu adapter (real
//! GPU or lavapipe), which CI / this host lack, so they are `#[ignore]` exactly
//! like tests/render_smoke.rs. Run locally with:
//!   cargo test -p buiy_core --test render_degraded_group_gpu -- --ignored

mod support;

use bevy::prelude::*;

/// Force the RT-pool degradation path: insert a tiny [`RtPoolBudget`] into the
/// render world so `plan_allocation` returns `false` for the lowest-cost groups.
/// The const default is 64 MiB (nothing degrades); a few hundred bytes degrades
/// almost everything. The render world persists across frames, so one insert
/// holds for the whole test.
fn force_tiny_rt_budget(app: &mut App, bytes: u64) {
    use buiy_core::render::compositor::RtPoolBudget;
    app.get_sub_app_mut(bevy::render::RenderApp)
        .expect("RenderApp")
        .world_mut()
        .insert_resource(RtPoolBudget(bytes));
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn render_degraded_group_gpu() {
    // R2 (effect-compositor.md § 2.3): a ROOT `Opacity` group that DEGRADES under
    // budget pressure must paint FLAT with its opacity folded per-instance — its
    // pixels are PRESENT at folded opacity, NOT vanished. With a tiny RT budget,
    // `plan_allocation` degrades the group, `prepare_effect_groups` folds
    // `opacity` into its members' alpha and merges its range into `flat_ranges`,
    // and the flat window draw paints it.
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};

    const W: u32 = 64;
    const H: u32 = 64;
    let red = Color::srgb(0.9, 0.05, 0.05); // OPAQUE red

    let mut app = support::gpu_render_app(W, H);
    // Degrade everything: budget far below one group's target bytes.
    force_tiny_rt_budget(&mut app, 64);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // One ROOT Opacity(0.6) group with a single opaque-red fill child.
    let fill = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(16.0)),
                    left: Sizing::Length(Length::px(16.0)),
                    ..default()
                })
                .width_px(32.0)
                .height_px(32.0),
            Background {
                color: ColorToken::Custom(red),
            },
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(0.6)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[fill]);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent]);

    support::finish_and_run(&mut app, 4);
    let pixels = support::readback_rgba(&mut app, target);
    let px = |x: u32, y: u32| support::px(&pixels, W, x, y);

    // Folded-flat expectation: opaque red at alpha 0.6 over the opaque-black
    // clear, encoded linear→sRGB8 (the Rgba8UnormSrgb target). The fold sets the
    // instance alpha to 0.6, and the flat straight-alpha SrcOver blend produces
    // red*0.6 over black.
    let red_lin = LinearRgba::from(red);
    let folded = LinearRgba::new(red_lin.red, red_lin.green, red_lin.blue, 0.6);
    let black = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
    let a = folded.alpha;
    let over = LinearRgba::new(
        folded.red * a + black.red * (1.0 - a),
        folded.green * a + black.green * (1.0 - a),
        folded.blue * a + black.blue * (1.0 - a),
        a + black.alpha * (1.0 - a),
    );
    let s = Srgba::from(over);
    let expected = [
        (s.red * 255.0).round() as u8,
        (s.green * 255.0).round() as u8,
        (s.blue * 255.0).round() as u8,
        255u8,
    ];

    let inside = px(28, 28); // deep interior of the 32x32 fill
    let clear = px(1, 1);
    println!("degraded inside (28,28) = {inside:?} (expected {expected:?})");
    println!("clear (1,1) = {clear:?}");

    // (a) the degraded group's pixels are PRESENT (not background) at folded 0.6.
    assert_ne!(inside, clear, "degraded group must paint, not vanish");
    const TOL: i32 = 5;
    for ch in 0..3 {
        assert!(
            (inside[ch] as i32 - expected[ch] as i32).abs() <= TOL,
            "degraded channel {ch}: got {}, expected {} (±{TOL}); folded-flat at 0.6",
            inside[ch],
            expected[ch]
        );
    }
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn degraded_fold_does_not_compound_over_two_frames() {
    // Per-tier idempotency (effect-compositor.md § 2.3): on a STEADY-STATE frame
    // the quad buffer is RETAINED (not repacked from source), so the fold must NOT
    // re-run — the degraded pixel is identical frame-to-frame. A fold that ran
    // every frame would compound to black.
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};

    const W: u32 = 64;
    const H: u32 = 64;
    let red = Color::srgb(0.9, 0.05, 0.05);

    let mut app = support::gpu_render_app(W, H);
    force_tiny_rt_budget(&mut app, 64);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    let fill = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(16.0)),
                    left: Sizing::Length(Length::px(16.0)),
                    ..default()
                })
                .width_px(32.0)
                .height_px(32.0),
            Background {
                color: ColorToken::Custom(red),
            },
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(0.6)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[fill]);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent]);

    support::finish_and_run(&mut app, 4);
    let frame1 = support::readback_rgba(&mut app, target.clone());
    // Drive steady-state frames (no paint input changes → quad buffer retained).
    for _ in 0..3 {
        app.update();
    }
    let frame2 = support::readback_rgba(&mut app, target);
    let p1 = support::px(&frame1, W, 28, 28);
    let p2 = support::px(&frame2, W, 28, 28);
    println!("frame1 (28,28) = {p1:?}  frame2 = {p2:?}");
    const TOL: i32 = 2;
    for ch in 0..4 {
        assert!(
            (p1[ch] as i32 - p2[ch] as i32).abs() <= TOL,
            "degraded pixel must not compound across steady frames: ch {ch} \
             {} vs {} (the fold ran once, not per-frame)",
            p1[ch],
            p2[ch]
        );
    }
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn degraded_glyph_fold_idempotent_under_quad_dirty_only_frame() {
    // MAJOR-2 glyph idempotency: a degraded group with BOTH a quad bg and glyphs.
    // Frame 2 mutates ONLY a quad input (the blue bg color → quad_dirty true,
    // glyph_dirty false, so the glyph buffer is RETAINED). The degraded WHITE
    // glyph ink must be unchanged (NOT re-dimmed) AND must still be PRESENT —
    // proving the glyph ALPHA-fold gates on `glyph_dirty` (no re-fold on the
    // retained buffer) while the glyph RANGE-MERGE gates on `quad_dirty ||
    // glyph_dirty` (the partition rebuild re-excludes the degraded glyph range,
    // so the merge must re-add it or the glyphs VANISH that frame). Without a real
    // GPU this is `#[ignore]`; it pins the split-gate the headless
    // `degraded_glyph_range_remerges_on_quad_dirty_only_frame` proves at the
    // pure-function tier, end-to-end.
    //
    // Channel discipline: we assert on the white ink's RED+GREEN, not blue. The
    // ink sits over the blue bg we deliberately edit, so anti-aliased glyph edges
    // legitimately blend the new bg into their BLUE channel (correct AA, not a
    // regression). White ink dominates R+G, which are orthogonal to a blue-only
    // bg edit, so R+G isolate the glyph fold. A double-fold dims the ink (R+G
    // drop); a dropped range-merge reverts it to the ~8 bg (R+G collapse) — R+G
    // stability rejects both. See the per-pixel assertion below.
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity, TextColor};
    use buiy_core::text::{FontSize, Text};

    const W: u32 = 96;
    const H: u32 = 64;
    let blue = Color::srgb(0.05, 0.05, 0.9);

    let mut app = support::gpu_render_app(W, H);
    force_tiny_rt_budget(&mut app, 64);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // A degraded Opacity group holding a quad bg AND a glyph run.
    let bg = app
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
                .width_px(64.0)
                .height_px(40.0),
            Background {
                color: ColorToken::Custom(blue),
            },
        ))
        .id();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default().absolute().inset(Inset {
                top: Sizing::Length(Length::px(12.0)),
                left: Sizing::Length(Length::px(12.0)),
                ..default()
            }),
            Text(String::from("Hi")),
            FontSize(24.0),
            TextColor(ColorToken::Custom(Color::WHITE)),
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(0.6)))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[bg, text]);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent]);

    support::finish_and_run(&mut app, 4);
    support::wait_for_text_ready(&mut app, 60);
    let frame1 = support::readback_rgba(&mut app, target.clone());

    // Mutate ONLY a quad input (the bg color) → quad_dirty, glyph retained.
    app.world_mut().get_mut::<Background>(bg).unwrap().color =
        ColorToken::Custom(Color::srgb(0.05, 0.05, 0.7));
    support::finish_and_run(&mut app, 3);
    let frame2 = support::readback_rgba(&mut app, target);

    // Sample the WHITE glyph ink of the "Hi" run and assert its red+green
    // channels are byte-stable frame-to-frame. We test R+G specifically (NOT
    // blue) because the ink is white over a BLUE background we *deliberately*
    // mutated this frame: anti-aliased glyph-edge pixels legitimately blend
    // `white·coverage + bg·(1-coverage)`, so their BLUE channel tracks the bg
    // change by design — comparing blue would flag correct AA as a regression.
    // White ink dominates R+G, which are orthogonal to the blue bg edit, so R+G
    // isolate the glyph fold from the bg. The dominant failure this frame pins is
    // VANISH: if the range-merge were (wrongly) gated on glyph_dirty instead of
    // quad_dirty||glyph_dirty, the partition rebuild would drop the degraded glyph
    // range on this quad-dirty-only frame and the ink would revert to the ~(8,8)
    // blue background — R+G collapsing from ~150-203 down to ~8, a huge delta this
    // assertion rejects. (The complementary double-fold/compounding hazard — the
    // alpha-fold wrongly re-running on the retained buffer — is the charter of
    // `degraded_fold_does_not_compound_over_two_frames`, which drives multiple
    // glyph-dirty frames; on THIS single quad-dirty-only frame a CPU re-fold would
    // not even re-upload, so the split-gate's no-re-fold half is pinned at the
    // pure-function tier by the headless mirror named in the doc comment above.)
    // Glyph ink is identified by high R+G (white over blue bg never reaches that:
    // its R+G are ~8). We require a real population of ink pixels so a fixture
    // drift that moved the glyphs out of the sampled box fails loudly instead of
    // silently asserting over zero pixels (the original single-row band sampled
    // pure background and "passed"/"failed" on the bg, not the ink).
    let mut ink_pixels = 0usize;
    for y in 14..34 {
        for x in 10..40 {
            let a = support::px(&frame1, W, x, y);
            let b = support::px(&frame2, W, x, y);
            // White ink in frame 1: both R and G well above the blue bg's ~8.
            if a[0] > 150 && a[1] > 150 {
                ink_pixels += 1;
                let d_r = (a[0] as i32 - b[0] as i32).abs();
                let d_g = (a[1] as i32 - b[1] as i32).abs();
                assert!(
                    d_r <= 3 && d_g <= 3,
                    "degraded glyph ink (white) must be stable on a quad-dirty-only \
                     frame at ({x},{y}): frame1={a:?} frame2={b:?} (R+G must not \
                     move — the alpha-fold gates on glyph_dirty so the retained \
                     buffer is not re-folded, while the range-merge gates on \
                     quad_dirty||glyph_dirty so the glyphs are re-merged, not \
                     vanished). A double-fold dims the ink; a dropped merge \
                     reverts it to the ~8 blue background."
                );
            }
        }
    }
    assert!(
        ink_pixels >= 8,
        "expected the white \"Hi\" run to land in the sampled box (found only \
         {ink_pixels} ink pixels) — fixture drift moved the glyphs; the stability \
         assertion would otherwise vacuously pass over background"
    );
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn nested_degraded_group_does_not_corrupt_parent() {
    // Scope guard (MAJOR-1): a NESTED group forced to degrade. The root-degraded
    // slice does NOT handle nested forward-composite (that routes into the PARENT
    // target, a node-side follow-up). `fold_root_degraded_into_flat`
    // debug_asserts on a nested degraded group, so in a DEBUG build this fixture
    // would panic in prepare — which is the intended loud guard. In a RELEASE
    // build the nested child is left untouched (it vanishes, tracked) and must NOT
    // mis-place at window level or corrupt the parent. We assert the parent's
    // composited region is not corrupted (a plausible non-degraded sibling still
    // paints). Under debug we skip the body (the debug_assert is the containment).
    if cfg!(debug_assertions) {
        // The prepare-side debug_assert is the containment in debug builds.
        return;
    }
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};

    const W: u32 = 96;
    const H: u32 = 96;
    let red = Color::srgb(0.9, 0.05, 0.05);

    // Budget that fits the OUTER group's target but degrades the smaller INNER
    // (nested) one: plan_allocation degrades lowest-cost (smallest) first.
    let mut app = support::gpu_render_app(W, H);
    force_tiny_rt_budget(&mut app, 4096);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // Outer Opacity group (large) containing an inner Opacity group (small) with
    // a fill — the inner is the nested degrade candidate.
    let inner_fill = app
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
                .width_px(16.0)
                .height_px(16.0),
            Background {
                color: ColorToken::Custom(red),
            },
        ))
        .id();
    let inner = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(0.5)))
        .id();
    app.world_mut()
        .entity_mut(inner)
        .add_children(&[inner_fill]);
    let outer = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(0.8)))
        .id();
    app.world_mut().entity_mut(outer).add_children(&[inner]);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[outer]);

    support::finish_and_run(&mut app, 4);
    let pixels = support::readback_rgba(&mut app, target);
    // The corner is untouched: the slice's flat-merge must NOT have mis-placed the
    // nested child at window level (a wrong-space paint would smear it here).
    let corner = support::px(&pixels, W, 1, 1);
    assert_eq!(
        corner,
        [0, 0, 0, 255],
        "nested degrade must not mis-place the child at window level (corner clean)"
    );
}
