//! GPU regression for the effect-group contiguity invariant
//! (`render/buckets.rs` `pack_view_partitioned`): an `Opacity(0.5)` group
//! whose member carries its OWN z-index stacking context (a different paint
//! tier) plus a non-member positioned sibling between the two tiers.
//!
//! Before the layout trigger-5 SC formers landed (stacking-and-top-layer.md
//! § 2 trigger 5), the parent formed no stacking context, so the root's tier
//! sort interleaved the sibling between the parent and its z-indexed member:
//! the group's instance range was non-contiguous, tripping the contiguity
//! `debug_assert_eq!` (and double-painting the spanned sibling in release).
//! With the trigger, the parent's subtree is one atomic `painters_z` entry
//! and the single-range partition holds by construction.
//!
//! Needs a wgpu adapter (real GPU or lavapipe) — `#[ignore]`, run with:
//!   cargo test -p buiy_core --test render_group_contiguity_gpu -- --ignored --test-threads=1

use bevy::prelude::*;

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn z_indexed_group_member_with_interleaving_sibling_composites_once() {
    use buiy_core::Node;
    use buiy_core::layout::{Inset, Length, Sizing, Style, ZIndex};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::{Background, Opacity};
    use buiy_core::render::compositor::composite_src_over;
    use std::borrow::Cow;

    const W: u32 = 64;
    const H: u32 = 64;

    let blue = Color::srgb(0.05, 0.05, 0.9);
    let red = Color::srgb(0.9, 0.05, 0.05);
    let green = Color::srgb(0.05, 0.9, 0.05);

    let mut app = crate::support::gpu_render_app(W, H);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert("test.blue".into(), blue);
        theme.colors.insert("test.red".into(), red);
        theme.colors.insert("test.green".into(), green);
    }

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());

    let fill = |token: &'static str| Background {
        color: ColorToken::Token(Cow::Borrowed(token)),
    };
    let abs_box = |left: f32, top: f32, w: f32, h: f32| {
        Style::default()
            .absolute()
            .inset(Inset {
                top: Sizing::Length(Length::px(top)),
                left: Sizing::Length(Length::px(left)),
                ..default()
            })
            .width_px(w)
            .height_px(h)
    };

    // The group: a BLUE `Opacity(0.5)` parent at (4,4)..(44,44) — itself a
    // painted instance — with a RED member at (8,8)..(24,24) carrying its
    // OWN z-index (Layer(2), the positive-z paint tier).
    let member = app
        .world_mut()
        .spawn((
            Node,
            abs_box(4.0, 4.0, 16.0, 16.0).z_index(ZIndex::Layer(2)),
            fill("test.red"),
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            abs_box(4.0, 4.0, 40.0, 40.0),
            Opacity(0.5),
            fill("test.blue"),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[member]);
    // The non-member sibling: GREEN at (48,8)..(60,20), z-index Layer(1) — a
    // paint tier BETWEEN the parent (positioned, auto z → tier 2) and the
    // member (z=2 → tier 3). Without the parent's stacking context the root's
    // tier sort emits [parent, sibling, member], splitting the group's
    // instance run around the flat sibling.
    let _sibling = app
        .world_mut()
        .spawn((
            Node,
            abs_box(48.0, 8.0, 12.0, 12.0).z_index(ZIndex::Layer(1)),
            fill("test.green"),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent, _sibling]);

    // Drive frames: finish + layout→extract→prepare upload + the graph paint
    // settle; `readback_rgba` polls further (the pipeline async-compiles).
    crate::support::finish_and_run(&mut app, 4);

    let pixels = crate::support::readback_rgba(&mut app, target);
    assert_eq!(pixels.len(), (W * H * 4) as usize);
    let px = |x: u32, y: u32| -> [u8; 4] {
        let i = ((y * W + x) * 4) as usize;
        [pixels[i], pixels[i + 1], pixels[i + 2], pixels[i + 3]]
    };

    // Expectations via the CPU port (`composite_src_over`), encoded linear →
    // sRGB8 (what the Rgba8UnormSrgb target stores):
    // - inside the group, the member paints OVER the parent (both opaque, so
    //   the group sample is plain red / plain blue), then the whole group
    //   composites ONCE at 0.5 over the black clear;
    // - the flat sibling draws at FULL alpha (it must NOT ride the group's
    //   off-screen target — pre-trigger it was spanned by the group range and
    //   double-painted).
    let black_lin = LinearRgba::new(0.0, 0.0, 0.0, 1.0);
    let expect = |c: Color, opacity: f32| -> [u8; 4] {
        let lin = composite_src_over(LinearRgba::from(c), black_lin, opacity);
        let srgb = Srgba::from(lin);
        [
            (srgb.red * 255.0).round() as u8,
            (srgb.green * 255.0).round() as u8,
            (srgb.blue * 255.0).round() as u8,
            255u8,
        ]
    };
    let expected_parent = expect(blue, 0.5);
    let expected_member = expect(red, 0.5);
    let expected_sibling = expect(green, 1.0);

    let clear = px(1, 62);
    let parent_only = px(34, 34);
    let member_px = px(20, 20); // inside member (8,8)..(24,24) ONLY (distinct color from parent either way)
    let sibling_px = px(54, 14);
    println!("clear       (1,62)  = {clear:?}");
    println!("parent-only (34,34) = {parent_only:?}  (expected {expected_parent:?})");
    println!("member      (14,14) = {member_px:?}  (expected {expected_member:?})");
    println!("sibling     (54,14) = {sibling_px:?}  (expected {expected_sibling:?})");

    assert_eq!(clear, [0, 0, 0, 255], "untouched corner reads the clear");

    const TOL: i32 = 4;
    let assert_px = |name: &str, got: [u8; 4], want: [u8; 4]| {
        for ch in 0..3 {
            let g = got[ch] as i32;
            let w = want[ch] as i32;
            assert!(
                (g - w).abs() <= TOL,
                "{name} channel {ch}: got {g}, expected {w} (±{TOL}); full \
                 got={got:?} expected={want:?}"
            );
        }
    };
    // (1) The group composites ONCE at 0.5 — parent-only and member pixels
    // read 50%-blue / 50%-red over black.
    assert_px("parent-only", parent_only, expected_parent);
    assert_px("member", member_px, expected_member);
    // (2) The non-member sibling paints FLAT at full alpha — neither dimmed
    // by the group's 0.5 composite nor double-painted (flat + group target).
    assert_px("sibling", sibling_px, expected_sibling);
}
