//! Phase 6 integration: anchor-resolution end-to-end.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3 + § 4.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    Anchor, AnchorErrorKind, AnchorName, AnchorNameRegistry, AnchorRef, Display, Inset,
    LayoutAnchorBroken, LayoutAnchorWarnedThisFrame, LayoutPlugin, Length, PositionTry, Style,
    SyncStylesIterCount, TryCondition,
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
    let plain = app
        .world_mut()
        .spawn((Node, Style::default().width_px(50.0).height_px(50.0)))
        .id();

    // Spawn a normal-anchor pair: anchor at (10,10) size 100x100, anchored 5px below.
    let anchor_e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(100.0).height_px(100.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("a".into())),
                ..default()
            },
        ))
        .id();
    let anchored_e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("a".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(5.0)),
                    conditions: vec![], // no conditions = always passes
                }],
                ..default()
            },
        ))
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

// ---------------------------------------------------------------------
// Phase 6 Task 11 — full integration coverage per spec § 4.
// ---------------------------------------------------------------------

#[test]
fn anchor_fallback_chain_second_wins_when_first_overflows_viewport() {
    use bevy::window::{PrimaryWindow, Window, WindowResolution};
    let mut app = app();

    // Synthesize a 200x200 PrimaryWindow so `FitsInViewport` has a
    // meaningful upper bound. Pattern matches
    // tests/layout_container_queries.rs:336.
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(200, 200),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    // Anchor: 50x50 at default flexbox position (0, 0) inside the
    // 200x200 root. The "above" inset would place the anchored entity
    // at y = -20 → fails FitsInViewport. The "below" inset places it
    // at y = 50 + 10 = 60 → passes.
    let root = app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(200.0)))
        .id();
    let anchor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("a".into())),
                ..default()
            },
        ))
        .id();
    app.world_mut().entity_mut(root).add_children(&[anchor]);

    // Anchored: try ABOVE first (fails: y = -20 < 0), then BELOW.
    let anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("a".into())),
                position_try: vec![
                    PositionTry {
                        inset: Inset::above(Length::Px(10.0)),
                        conditions: vec![TryCondition::FitsInViewport],
                    },
                    PositionTry {
                        inset: Inset::below(Length::Px(10.0)),
                        conditions: vec![TryCondition::FitsInViewport],
                    },
                ],
                ..default()
            },
        ))
        .id();

    app.update();
    app.update();

    let rl = app.world().get::<ResolvedLayout>(anchored).unwrap();
    // BELOW fallback wins: y = anchor.y + anchor.h + 10 = 0 + 50 + 10 = 60.
    assert_eq!(rl.position.y, 60.0);
    // Not broken — a fallback resolved successfully.
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_none());
}

#[test]
fn anchor_cycle_marks_both_endpoints_broken() {
    let mut app = app();
    // a -> b (a anchors to "b"); b -> a (b anchors to "a"). b is
    // spawned after a, so b's epoch is higher → b's outgoing edge is
    // dropped per Kahn. Per spec § 3.4 D8, BOTH endpoints get
    // LayoutAnchorBroken.
    let a = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("a".into())),
                position_anchor: Some(AnchorRef::Name("b".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(5.0)),
                    conditions: vec![],
                }],
            },
        ))
        .id();
    let b = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("b".into())),
                position_anchor: Some(AnchorRef::Name("a".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(5.0)),
                    conditions: vec![],
                }],
            },
        ))
        .id();
    app.update();
    app.update();

    // Spec § 3.4 line 229: "Both endpoints get LayoutAnchorBroken markers."
    assert!(
        app.world().get::<LayoutAnchorBroken>(b).is_some(),
        "spec § 3.4: cycle source (dropped edge) must be marked"
    );
    assert!(
        app.world().get::<LayoutAnchorBroken>(a).is_some(),
        "spec § 3.4: cycle target (other endpoint of dropped edge) must be marked"
    );

    // Exactly one InCycle warn per cycle per frame.
    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    let in_cycle_count = warned
        .set
        .iter()
        .filter(|(_, k)| *k == AnchorErrorKind::InCycle)
        .count();
    assert_eq!(in_cycle_count, 1);
}

#[test]
fn anchor_duplicate_name_warns_each_frame_dupe_persists() {
    let mut app = app();
    // First entity claims "dupe".
    let _e1 = app
        .world_mut()
        .spawn(Anchor {
            anchor_name: Some(AnchorName::Named("dupe".into())),
            ..default()
        })
        .id();
    // Second entity also claims "dupe" — e2 is the late inserter.
    let e2 = app
        .world_mut()
        .spawn(Anchor {
            anchor_name: Some(AnchorName::Named("dupe".into())),
            ..default()
        })
        .id();
    app.update();

    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    assert!(warned.set.contains(&(e2, AnchorErrorKind::DuplicateName)));

    // After a second update, the duplicate persists — warn should still
    // fire (re-detected each frame from the registry, not only at
    // observer-insert time).
    app.update();
    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    assert!(warned.set.contains(&(e2, AnchorErrorKind::DuplicateName)));
}

#[test]
fn anchor_missing_target_marks_broken_and_warns_once_per_frame() {
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("nonexistent".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(0.0)),
                    conditions: vec![],
                }],
                ..default()
            },
        ))
        .id();
    app.update();
    app.update();

    assert!(app.world().get::<LayoutAnchorBroken>(e).is_some());
    let rl = app.world().get::<ResolvedLayout>(e).unwrap();
    assert_eq!(rl.position, Vec2::ZERO);

    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    assert!(warned.set.contains(&(e, AnchorErrorKind::TargetMissing)));
}

#[test]
fn layout_anchor_broken_clears_when_resolution_succeeds() {
    let mut app = app();

    // Start with a missing target → broken.
    let anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("late".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(0.0)),
                    conditions: vec![],
                }],
                ..default()
            },
        ))
        .id();
    app.update();
    app.update();
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_some());

    // Now spawn the target.
    let _target = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("late".into())),
                ..default()
            },
        ))
        .id();
    app.update();
    app.update();

    // Broken marker should be removed (idempotent cleanup).
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_none());
}

#[test]
fn anchor_steady_state_no_extra_sync_styles_iter() {
    let mut app = app();
    let _anchor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("a".into())),
                ..default()
            },
        ))
        .id();
    let _anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("a".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(5.0)),
                    conditions: vec![],
                }],
                ..default()
            },
        ))
        .id();

    // Run several frames to reach steady state.
    for _ in 0..5 {
        app.update();
    }

    // Steady-state Phase 2 invariant: sync_styles iter count is 0 (no
    // Changed<> for any tracked component on this frame). Anchor pass
    // writes to AnchorOverrides (resource) but does NOT cascade
    // Changed<> back into sync_styles's trigger set.
    let count = app.world().resource::<SyncStylesIterCount>().0;
    assert_eq!(count, 0);
}

#[test]
fn anchor_observer_cleans_registry_on_despawn() {
    let mut app = app();
    let e = app
        .world_mut()
        .spawn(Anchor {
            anchor_name: Some(AnchorName::Named("ephemeral".into())),
            ..default()
        })
        .id();

    {
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("ephemeral"), Some(e));
    }

    app.world_mut().entity_mut(e).despawn();

    // Despawn fires On<Remove, Anchor> which calls reg.remove(e).
    let reg = app.world().resource::<AnchorNameRegistry>();
    assert_eq!(reg.find_entity_by_name("ephemeral"), None);
}

#[test]
fn anchor_target_with_display_none_is_treated_as_missing() {
    let mut app = app();
    let _hidden_anchor = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(50.0)
                .height_px(50.0)
                .display(Display::None),
            Anchor {
                anchor_name: Some(AnchorName::Named("hidden".into())),
                ..default()
            },
        ))
        .id();
    let anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("hidden".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(0.0)),
                    conditions: vec![],
                }],
                ..default()
            },
        ))
        .id();
    app.update();
    app.update();

    // Display::None target → anchored is broken (spec § 3.2 step 1,
    // D9 explicit query in anchor_resolution).
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_some());
}
