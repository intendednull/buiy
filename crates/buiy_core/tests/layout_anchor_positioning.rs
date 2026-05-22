//! Phase 6 integration: anchor-resolution end-to-end.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3 + § 4.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    Anchor, AnchorName, AnchorRef, Inset, LayoutPlugin, Length, PositionTry, Style, TryCondition,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

#[test]
fn anchor_basic_positions_below_anchor() {
    let mut app = app();
    // Anchor: 100x50 at (50, 50)
    let anchor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(100.0).height_px(50.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("btn".into())),
                ..default()
            },
        ))
        .id();
    let _ = anchor; // anchor referenced by name; the registry resolves it

    // Anchored: 80x20, placed 10px below the anchor.
    let anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(80.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("btn".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(10.0)),
                    conditions: vec![TryCondition::FitsInViewport],
                }],
                ..default()
            },
        ))
        .id();

    // Run a couple of frames to let Taffy resolve sizes + anchor pass apply.
    app.update();
    app.update();

    let anchored_rl = app.world().get::<ResolvedLayout>(anchored).unwrap();
    // anchored's y should be at anchor.y + anchor.size.y + 10
    // (anchor at default 0,0 in Taffy with size 100,50 → anchored at y=60)
    assert_eq!(anchored_rl.position.y, 60.0);
}
