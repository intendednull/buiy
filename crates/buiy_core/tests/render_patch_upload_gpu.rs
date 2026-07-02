//! GPU reftest (#[ignore]) for the #2 Stage D2 partial UPLOAD. A group-free,
//! pure-bg-quad value change (a hover re-tint) must (a) re-upload ONLY the changed
//! entity's quad slot — not the whole O(N) instance blob — and (b) produce a frame
//! pixel-identical to a cold full render of the same final state. A wrong slot, span,
//! or pack would diverge from the cold reference. Needs a real wgpu adapter; the
//! headless gate cannot exercise prepare (no device). Run with
//! `cargo test -p buiy_core --test render_patch_upload_gpu -- --ignored`.

mod support;

use bevy::prelude::*;
use bevy::render::RenderApp;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::prepare::BufferUploadStats;

const W: u32 = 64;
const H: u32 = 96;

/// A flex column of solid bg nodes, one per entry in `colors`. Returns
/// the GPU app, its readback target, and the child entities (index 0 is the victim).
fn scene(colors: &[Color]) -> (App, Handle<Image>, Vec<Entity>) {
    let mut app = support::gpu_render_app(W, H);
    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());

    let kids: Vec<Entity> = colors
        .iter()
        .map(|c| {
            app.world_mut()
                .spawn((
                    Node,
                    Style::default().width_px(40.0).height_px(12.0),
                    Background {
                        color: ColorToken::Custom(*c),
                    },
                ))
                .id()
        })
        .collect();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .padding(4.0)
                .gap_px(4.0),
        ))
        .add_children(&kids);
    (app, target, kids)
}

fn uploaded(app: &App) -> u64 {
    app.get_sub_app(RenderApp)
        .expect("RenderApp")
        .world()
        .resource::<BufferUploadStats>()
        .instances_uploaded
}

#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); run with --ignored"]
fn partial_upload_patches_one_slot_pixel_identical() {
    // App A: four solid bg nodes, then PATCH node 0's color (a group-free,
    // footprint-stable, pure-bg-quad change → C3b Patch → D2 partial upload).
    let red = Color::srgb(0.90, 0.12, 0.12);
    let blue = Color::srgb(0.12, 0.25, 0.90);
    let (mut a, ta, kids) = scene(&[red, red, red, red]);
    support::finish_and_run(&mut a, 3);
    let before = uploaded(&a); // the full upload packed all 4 instances
    a.world_mut().get_mut::<Background>(kids[0]).unwrap().color = ColorToken::Custom(blue);
    a.update(); // the Patch frame
    a.update(); // settle
    let after = uploaded(&a);
    let pixels_a = support::readback_rgba(&mut a, ta);

    // The Patch frame uploaded EXACTLY the one changed quad instance — proof the partial
    // path fired (a full-repack fallback would re-upload all four).
    assert_eq!(
        after - before,
        1,
        "#2 D2: a pure-bg-quad color Patch uploads exactly 1 quad instance, not all N \
         (delta {} — N is 4)",
        after - before
    );

    // App B: the SAME final colors rendered cold (one full pack + upload).
    let (mut b, tb, _) = scene(&[blue, red, red, red]);
    support::finish_and_run(&mut b, 3);
    let pixels_b = support::readback_rgba(&mut b, tb);

    assert_eq!(
        pixels_a, pixels_b,
        "#2 D2: the partial-upload Patch frame must be pixel-identical to a cold full \
         render of the same final state (a wrong slot/span/pack would diverge)"
    );
}
