//! GPU lane: `render::golden::capture_app` builds a painting-capable headless
//! App identical to the test-support `gpu_render_app` stack, so the reftest /
//! golden tiers in buiy_verify build their app from `src` (reftests.md § build
//! seam). #[ignore] — needs a real adapter.

use bevy::prelude::*;
use buiy_core::components::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::golden::{GoldenConfig, capture_app, capture_to_image};
use std::borrow::Cow;

#[test]
#[ignore = "GPU: run under `cargo test -- --ignored` (real adapter / lavapipe)"]
fn capture_app_paints_a_non_blank_frame() {
    let mut app = capture_app(64, 64);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme
            .colors
            .insert("test.fill.a".into(), Color::srgb(0.90, 0.10, 0.10));
    }
    let e = app
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
                .width_px(40.0)
                .height_px(40.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed("test.fill.a")),
            },
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[e]);

    let img = capture_to_image(&mut app, &GoldenConfig::deterministic());
    assert_eq!(img.dimensions(), (64, 64));
    let painted = img.pixels().any(|p| p.0 != [0, 0, 0, 255]);
    assert!(painted, "capture_app must paint the box, not a blank frame");
}
