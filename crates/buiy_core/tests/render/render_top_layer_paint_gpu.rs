//! GPU lane (`#[ignore]` — needs a real wgpu adapter / lavapipe): the M1/M6
//! parity-prototype render fix — a **top-layer subtree's descendants actually
//! rasterize at their correct on-screen position** (their `Background` fills AND
//! their text glyphs), matching how icons/borders already did.
//!
//! The bug: a top-layer node (`Stacking { top_layer: Modal/Popover, .. }`)
//! escapes its parent stacking context and is appended to the root context's
//! `painters_z` tail as one atomic entry. The render node + glyph walk
//! (`context_tree_paint_order`) descends into a painter's subtree ONLY when that
//! painter owns a `StackingContext`. Before the trigger-7 fix a plain top-layer
//! node (no transform / isolation / z-index) formed NO stacking context, so the
//! walk treated it as a childless LEAF — its descendants' fills + glyphs were
//! dropped (only icons survived, since `icon_producer` iterates entities
//! directly). This renders a top-layer subtree to an offscreen target and asserts
//! the descendant's accent FILL and a descendant text GLYPH are present in the
//! readback, then writes the PNG proof to
//! `docs/reports/parity-proto-assets/fix-m1m6.png`.
//!
//! Run:   cargo test -p buiy_core --test render fix_m1m6 -- --ignored --test-threads=1

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style, TopLayer};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, TextColor};
use buiy_core::text::{FontSize, Text};
use std::borrow::Cow;

use crate::support::{
    finish_and_run, gpu_render_app, px, readback_rgba, render_to_image, spawn_capture_camera,
    wait_for_text_ready,
};

const W: u32 = 200;
const H: u32 = 160;

/// `color.accent` and the glyph tint, inserted as distinct, saturated colors so
/// the readback can tell the descendant FILL (red) and the descendant GLYPH
/// (green) apart from the dialog card bg (a dim gray) and the black clear.
const FILL_TOKEN: &str = "test.m6.fill"; // the descendant Background (red)
const TEXT_TOKEN: &str = "test.m1.text"; // the descendant glyph (green)
const CARD_TOKEN: &str = "test.card"; // the top-layer node's own bg (gray)

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn fix_m1m6_top_layer_descendants_fill_and_text_paint() {
    let mut app = gpu_render_app(W, H);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert(FILL_TOKEN.into(), Color::srgb(0.90, 0.10, 0.10)); // red
        theme
            .colors
            .insert(TEXT_TOKEN.into(), Color::srgb(0.10, 0.90, 0.20)); // green
        theme
            .colors
            .insert(CARD_TOKEN.into(), Color::srgb(0.20, 0.20, 0.22)); // gray
    }

    // The descendant FILL (M6): a 70×40 red box inside the top-layer card. With
    // the card at (40,40) and 10px padding, this paints at ~(50,50)..(120,90).
    let fill_child = app
        .world_mut()
        .spawn((
            Node,
            Name::new("fill_child"),
            Style::default().width_px(70.0).height_px(40.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed(FILL_TOKEN)),
            },
        ))
        .id();
    // The descendant TEXT (M1): a green glyph leaf below the fill child. Large so
    // a glyph stem covers several pixels in the readback.
    let text_child = app
        .world_mut()
        .spawn((
            Node,
            Name::new("text_child"),
            Style::default(),
            Text(String::from("M")),
            FontSize(40.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
        ))
        .id();

    // The TOP-LAYER card: absolute, inset (40,40), with its OWN gray bg. Plain
    // `top_layer(Modal)` — NO transform / isolation / z-index, so it forms an SC
    // ONLY via trigger 7. Its descendants (fill_child + text_child) ride the
    // walk's descent into this node's context.
    let card = app
        .world_mut()
        .spawn((
            Node,
            Name::new("card"),
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(40.0)),
                    left: Sizing::Length(Length::px(40.0)),
                    ..Default::default()
                })
                .flex_column()
                .padding(10.0)
                .top_layer(TopLayer::Modal),
            Background {
                color: ColorToken::Token(Cow::Borrowed(CARD_TOKEN)),
            },
        ))
        .add_children(&[fill_child, text_child])
        .id();
    // The screen root (the card escapes ITS parent context to the root's tail).
    app.world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().width_px(W as f32).height_px(H as f32),
        ))
        .add_children(&[card]);

    let target = render_to_image(&mut app, W, H);
    spawn_capture_camera(&mut app, target.clone());
    finish_and_run(&mut app, 1);
    wait_for_text_ready(&mut app, 60);
    let pixels = readback_rgba(&mut app, target.clone());

    // Write the PNG proof artifact (NOT a blessed CI golden — the prototype proof;
    // lavapipe pixels differ host-to-host). The two channel assertions below are
    // the programmatic proof.
    {
        let img = image::RgbaImage::from_raw(W, H, pixels.clone())
            .expect("readback dimensions match the target");
        let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/reports/parity-proto-assets/fix-m1m6.png");
        if let Some(parent) = out.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        img.save(&out)
            .unwrap_or_else(|e| panic!("write {}: {e}", out.display()));
    }

    // (M6) The descendant FILL paints: a pixel inside the red box (card at 40,40 +
    // 10 padding → fill at ~50,50; sample the interior at (80,65)) is red-dominant,
    // NOT the gray card bg and NOT the black clear.
    let fill = px(&pixels, W, 80, 65);
    assert!(
        fill[0] > 120 && fill[1] < 90 && fill[2] < 90,
        "M6: the top-layer card's descendant Background FILL must paint red \
         (pre-fix it never extracted — the bg-quad walk dropped it); got {fill:?}"
    );

    // (M1) The descendant TEXT paints: the brightest GREEN-dominant pixel anywhere
    // in the readback is a glyph texel of the green text leaf — proving a
    // top-layer descendant's GLYPHS extract through the same `context_tree`
    // descent (pre-fix the glyph walk dropped them too).
    let greenest = pixels
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .filter(|p| p[1] as i32 - p[0] as i32 > 40 && p[1] as i32 - p[2] as i32 > 40)
        .max_by_key(|p| p[1] as u32)
        .unwrap_or([0, 0, 0, 0]);
    assert!(
        greenest[1] > 100,
        "M1: a top-layer card's descendant TEXT GLYPH must paint green \
         (pre-fix the glyph walk dropped top-layer descendants); brightest \
         green-dominant texel = {greenest:?}"
    );

    // Sanity: the top-layer node's OWN gray bg also paints (it always did — it is
    // the leaf entry itself), so the fix did not regress the node's own fill. The
    // card spans (40,40)..(40+pad+content). Sample the top padding strip at
    // (45,45): inside the card, above the fill child → the gray card bg.
    let card_bg = px(&pixels, W, 45, 45);
    assert!(
        card_bg[0] > 30 && card_bg[0] < 110 && (card_bg[0] as i32 - card_bg[2] as i32).abs() < 40,
        "the top-layer card's OWN bg must still paint gray (no regression to the \
         node's own fill); got {card_bg:?}"
    );
}
