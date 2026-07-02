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
    use std::borrow::Cow;

    const W: u32 = 64;
    const H: u32 = 64;
    let red = Color::srgb(0.9, 0.05, 0.05); // OPAQUE red

    let mut app = support::gpu_render_app(W, H);
    // Degrade everything: budget far below one group's target bytes.
    force_tiny_rt_budget(&mut app, 64);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.red".into(), red);
    }
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
                color: ColorToken::Token(Cow::Borrowed("test.red")),
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
    use std::borrow::Cow;

    const W: u32 = 64;
    const H: u32 = 64;
    let red = Color::srgb(0.9, 0.05, 0.05);

    let mut app = support::gpu_render_app(W, H);
    force_tiny_rt_budget(&mut app, 64);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.red".into(), red);
    }
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
                color: ColorToken::Token(Cow::Borrowed("test.red")),
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
    use std::borrow::Cow;

    const W: u32 = 96;
    const H: u32 = 64;
    let blue = Color::srgb(0.05, 0.05, 0.9);

    let mut app = support::gpu_render_app(W, H);
    force_tiny_rt_budget(&mut app, 64);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.blue".into(), blue);
        theme.colors.insert("test.white".into(), Color::WHITE);
    }
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
                color: ColorToken::Token(Cow::Borrowed("test.blue")),
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
            TextColor(ColorToken::Token(Cow::Borrowed("test.white"))),
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
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("test.blue".into(), Color::srgb(0.05, 0.05, 0.7));
    }
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
fn nested_degraded_child_forward_composites_into_parent() {
    // CASE A (effect-compositor.md § 2.3; nested-degraded-forward-composite
    // design): a NESTED effect group that degrades under RT-pool pressure while its
    // immediate parent KEEPS its target must forward-composite its (folded) members
    // DIRECTLY into the parent's Rgba16Float target at node step-2a — present at the
    // composed opacity, NOT vanished, and NOT mis-placed at window level.
    //
    // The fixture reaches case A by construction. Each group's bounds are SEEDED
    // with the group's own box at its (0,0) origin, then grown by OWN DIRECT
    // members only (extract.rs) — so INNER's bounds = (0,0)..(inner_fill.max). With
    // inner_fill at (8,8) 16×16, inner bounds = 24×24 → 32² bucket → 8192 B; OUTER
    // (a 60×60 green bg → 64² bucket → 32768 B) is the LARGER target. A budget in
    // [32768, 40960) keeps outer, degrades only inner — that
    // allocation is pinned deterministically WITHOUT a GPU by the headless
    // `plan_allocation_pins_case_a_budget_outer_kept_inner_degraded` test; this GPU
    // test proves the PIXELS. The old `cfg!(debug_assertions)` skip is gone: nested
    // groups no longer panic prepare (`fold_degraded_groups` dropped the
    // debug_assert). RED before the node injection: inner is nested-degraded → not
    // drawn → interior fails the red-dominance assertion.
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};
    use buiy_core::render::compositor::composite_src_over;
    use std::borrow::Cow;

    const W: u32 = 96;
    const H: u32 = 96;
    let red = Color::srgb(0.9, 0.05, 0.05); // inner fill
    let green = Color::srgb(0.05, 0.6, 0.05); // outer's own bg
    const INNER_OP: f32 = 0.5;
    const OUTER_OP: f32 = 0.8;

    let mut app = support::gpu_render_app(W, H);
    // Case-A budget: keeps outer (32768 B), degrades only inner (8192 B).
    force_tiny_rt_budget(&mut app, 33_000);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.red".into(), red);
        theme.colors.insert("test.green".into(), green);
    }
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    // INNER group: Opacity(0.5) wrapping a 16×16 red fill at (8,8). Its bounds =
    // (0,0)..(24,24) (seed origin ∪ fill) → 32² bucket → the smaller, degraded target.
    let inner_fill = app
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
                .width_px(16.0)
                .height_px(16.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.red")),
            },
        ))
        .id();
    let inner = app
        .world_mut()
        .spawn((Node, Style::default().absolute(), Opacity(INNER_OP)))
        .id();
    app.world_mut()
        .entity_mut(inner)
        .add_children(&[inner_fill]);
    // OUTER group: Opacity(0.8) with its OWN 60×60 green bg at (0,0) — spatially
    // CONTAINS the inner fill (so the injection isn't clipped) and is the LARGER
    // target (64² bucket), so it keeps its target while inner degrades.
    let outer = app
        .world_mut()
        .spawn((
            Node,
            Style::default().absolute().width_px(60.0).height_px(60.0),
            Opacity(OUTER_OP),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.green")),
            },
        ))
        .id();
    app.world_mut().entity_mut(outer).add_children(&[inner]);
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[outer]);

    support::finish_and_run(&mut app, 4);
    let pixels = support::readback_rgba(&mut app, target);
    let px = |x: u32, y: u32| support::px(&pixels, W, x, y);

    // Expected composed values, via the two compositing stages the pipeline runs.
    // Stage 1 (step-2a inject): inner red folded to alpha INNER_OP, straight-alpha
    // SrcOver over the outer bg in the outer target. Stage 2 (step-2b root
    // composite): the outer target composited into the black window at OUTER_OP.
    let red_lin = LinearRgba::from(red);
    let green_lin = LinearRgba::from(green);
    let black = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
    let enc = |lin: LinearRgba| {
        let s = Srgba::from(lin);
        [
            (s.red * 255.0).round() as u8,
            (s.green * 255.0).round() as u8,
            (s.blue * 255.0).round() as u8,
            255u8,
        ]
    };
    let folded_inner = LinearRgba::new(red_lin.red, red_lin.green, red_lin.blue, INNER_OP);
    let inner_in_outer = composite_src_over(folded_inner, green_lin, 1.0); // inject over outer bg
    let expected_inner = enc(composite_src_over(inner_in_outer, black, OUTER_OP));
    let expected_outer = enc(composite_src_over(green_lin, black, OUTER_OP)); // outer bg only

    let interior = px(16, 16); // inner-fill center (8,8)+8 → the injected child
    let outer_only = px(50, 50); // inside outer bg (0..60), outside inner (8..24)
    let corner = px(88, 88); // outside outer entirely → clear
    println!("interior (16,16)={interior:?} expected≈{expected_inner:?}");
    println!("outer_only (50,50)={outer_only:?} expected≈{expected_outer:?}");
    println!("corner (88,88)={corner:?}");

    const TOL: i32 = 8;
    // (1) The injected inner is PRESENT (not vanished) at the composed level.
    for ch in 0..3 {
        assert!(
            (interior[ch] as i32 - expected_inner[ch] as i32).abs() <= TOL,
            "case A: injected inner channel {ch}: got {}, expected {} (±{TOL})",
            interior[ch],
            expected_inner[ch]
        );
    }
    // (2) Red dominates at the inner position AND clearly exceeds the outer-only
    // red — the child is present ON TOP of the parent, not vanished.
    assert!(
        interior[0] as i32 > interior[1] as i32 + 20
            && interior[0] as i32 > outer_only[0] as i32 + 20,
        "inner must be red-dominant and brighter-red than the outer-only region: \
         interior {interior:?} vs outer_only {outer_only:?}"
    );
    // (3) The parent KEPT its target and painted: the outer-only region is its
    // green bg at OUTER_OP (proves this is case A, not both-degraded).
    for ch in 0..3 {
        assert!(
            (outer_only[ch] as i32 - expected_outer[ch] as i32).abs() <= TOL,
            "outer-only channel {ch}: got {}, expected {} (±{TOL})",
            outer_only[ch],
            expected_outer[ch]
        );
    }
    assert!(
        outer_only[1] as i32 > outer_only[0] as i32 + 20,
        "outer-only region must be green-dominant (parent kept + painted): {outer_only:?}"
    );
    // (4) No mis-placement at window level: a point outside the parent is clear.
    assert_eq!(
        corner,
        [0, 0, 0, 255],
        "nested inject must not mis-place the child at window level (corner clean)"
    );
}
