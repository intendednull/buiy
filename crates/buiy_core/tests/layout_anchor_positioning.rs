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

#[test]
fn write_resolved_layout_prefers_anchor_override_over_taffy_position() {
    use buiy_core::layout::AnchorOverrides;
    let mut app = app();

    // Spawn an entity with NO Anchor — just a normal layout node.
    let plain = app.world_mut().spawn((Node, Style::default().width_px(50.0).height_px(50.0))).id();

    // Spawn a normal-anchor pair: anchor at (10,10) size 100x100, anchored 5px below.
    let anchor_e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0),
                Anchor { anchor_name: Some(AnchorName::Named("a".into())), ..default() }))
        .id();
    let anchored_e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(20.0).height_px(20.0),
                Anchor { position_anchor: Some(AnchorRef::Name("a".into())),
                          position_try: vec![PositionTry {
                              inset: Inset::below(Length::Px(5.0)),
                              conditions: vec![],  // no conditions = always passes
                          }], ..default() }))
        .id();

    app.update();
    app.update();

    // Plain entity: position comes from Taffy.
    let plain_rl = app.world().get::<ResolvedLayout>(plain).unwrap();
    assert!(plain_rl.position.x == 0.0); // first child in root: Taffy places at 0,0

    // Anchored entity: position comes from override (anchor.y + anchor.size.y + 5 = 0 + 100 + 5 = 105)
    let anchored_rl = app.world().get::<ResolvedLayout>(anchored_e).unwrap();
    assert_eq!(anchored_rl.position.y, 105.0);

    // Confirm via AnchorOverrides resource directly.
    let overrides = app.world().resource::<AnchorOverrides>();
    assert!(overrides.by_entity.contains_key(&anchored_e));
    assert!(!overrides.by_entity.contains_key(&plain));
    assert!(!overrides.by_entity.contains_key(&anchor_e)); // anchor target, not anchored
}

#[test]
fn sync_styles_reruns_when_anchor_changes() {
    use buiy_core::layout::SyncStylesIterCount;
    let mut app = app();

    // Spawn one entity with default Style (no Anchor).
    let e = app.world_mut().spawn((Node, Style::default())).id();
    app.update();
    let count_before = app.world().resource::<SyncStylesIterCount>().0;

    // Insert Anchor — Changed<Anchor> fires.
    app.world_mut().entity_mut(e).insert(Anchor {
        anchor_name: Some(AnchorName::Named("x".into())),
        ..default()
    });
    app.update();
    let count_after = app.world().resource::<SyncStylesIterCount>().0;

    // The entity should have been re-translated. After steady-state, the
    // count drops back to zero. SyncStylesIterCount measures THIS frame's
    // matched count, so count_after >= 1 immediately after the Anchor insert.
    assert!(count_after >= 1);

    // After a second update, the entity is no longer Changed; count drops.
    app.update();
    let count_steady = app.world().resource::<SyncStylesIterCount>().0;
    assert_eq!(count_steady, 0);

    // Sanity: silence unused warning on count_before — it's kept to make
    // the steady→change→steady arc explicit in the test body.
    let _ = count_before;
}
