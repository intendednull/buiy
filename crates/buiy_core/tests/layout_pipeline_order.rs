//! 9-step pipeline order asserted at the integration level.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 3.
//!
//! Phase 7 Task 11 — the fixture below also exercises the 4-sub-pass
//! `PostTaffyOverrides` chain (6a sticky → 6b table → 6c multicol → 6d
//! anchor) with realistic data so the order assertion doubles as a
//! smoke-and-side-effect check: every sub-pass must produce its
//! declared observable (override entry for sticky/anchored; warn-once
//! set entries for table + multicol). The pivotal *ordering* proof is
//! that the anchored entity tracks the sticky target's DISPLACED
//! position — only possible if 6a runs before 6d (Task 9's D1 fix).
//!
//! Plan: docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md
//! Task 11 (BLOCKER B3 in plan v2).

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout, ResolvedTransform,
    components::StackingContext,
    layout::{
        Anchor, AnchorName, AnchorRef, BuiyLayoutStep, ContainerQuery, Display, Inset,
        LayoutPlugin, LayoutWarnOnceKey, LayoutWarnedOnceSession, Length, MultiColumn,
        OverflowMode, Position, PositionKind, PositionTry, PostTaffyPositionOverrides,
        QueryCondition, ScrollOffset, Sizing, Stacking, Style, TransformMatrix, UiTransform,
        ZIndex,
    },
};

fn stacking_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

#[test]
fn layout_steps_are_chained_in_declared_order() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    // Force an Update build so set ordering is materialized.
    app.update();

    // The Schedule API in 0.18 doesn't expose a stable enumeration of
    // SystemSet ordering directly. We use the existence-and-ordering
    // contract: every BuiyLayoutStep set is configured, and configuring
    // a contradictory order fails schedule build. The smoke check here
    // is that adding a tracker system to each set runs them in the
    // declared order.
    let order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));

    fn make_tracker(
        order: std::sync::Arc<std::sync::Mutex<Vec<&'static str>>>,
        label: &'static str,
    ) -> impl Fn() + Send + Sync + 'static {
        move || {
            order.lock().unwrap().push(label);
        }
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let o = order.clone();
    app.add_systems(
        Update,
        make_tracker(o.clone(), "gc").in_set(BuiyLayoutStep::RemovedNodesGc),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "wmi").in_set(BuiyLayoutStep::WritingModeInherit),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "sync").in_set(BuiyLayoutStep::SyncStyles),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "cq_activate").in_set(BuiyLayoutStep::CqActivate),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "taffy").in_set(BuiyLayoutStep::TaffyCompute),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "cq_flip").in_set(BuiyLayoutStep::CqFlipCheck),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "cq_rerun").in_set(BuiyLayoutStep::CqFlipReRun),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "post_taffy").in_set(BuiyLayoutStep::PostTaffyOverrides),
    );
    app.add_systems(
        Update,
        make_tracker(o.clone(), "write").in_set(BuiyLayoutStep::WriteResolvedLayout),
    );

    // Phase 5 Task 10: spawn one Container + one ContainerQuery + one
    // descendant with Cqw so cq_activate / cq_flip_check / cq_flip_rerun
    // (and `translate_one_entity`'s `Cq*` resolution) all have reachable
    // work. The order assertion below stays unchanged; this addition
    // makes the order test also a smoke test that the cq systems
    // compile and run with realistic data.
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(800.0)
                .height_px(400.0)
                .container_size(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::Length(Length::Cqw(50.0))),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[child]);

    // Phase 7 Task 11: extend the fixture so each of the 4 sub-passes
    // in `PostTaffyOverrides` has reachable work AND so the per-pass
    // side-effects are observable from the assertions below.
    //
    // Structure (Option 2 from the plan — reuses the Phase 6 anchor
    // target as the sticky entity to make the ordering invariant
    // testable):
    //
    //   scroll_container  (overflow_y: scroll, ScrollOffset.y = 100)
    //     └─ content_block          (height 1000)
    //          ├─ spacer            (height 50, pushes sticky to y=50)
    //          └─ sticky_target     (Position::Sticky, top inset 0,
    //                                 Anchor name "test-anchor")
    //   anchored                    (root) — references "test-anchor"
    //   table_entity                (root, Display::Table)
    //     └─ table_row              (Display::TableRow)
    //          └─ table_cell        (Display::TableCell, 40x20)
    //   multicol_entity             (root, MultiColumn)
    //
    // After update(), sub-pass 6a displaces `sticky_target` from
    // y_in_block = 50 to y_in_block = 100 (visible_top = 100, threshold
    // = 100, max(50, 100) = 100). Sub-pass 6d reads the *displaced*
    // target position from PostTaffyPositionOverrides (Task 9 D1 fix)
    // and places the anchored entity 5 px below the displaced target,
    // not below the natural Taffy target. This is the explicit ordering
    // proof (BLOCKER B3): a wrong ordering (6d before 6a) would yield
    // anchored.y = natural_target_y + h + gap = 50 + 50 + 5 = 105,
    // not displaced_target_y + h + gap = 100 + 50 + 5 = 155.
    //
    // The sticky_target is 50x50; the anchored entity is 30x20; the
    // anchored entity sits at the root, so its parent-relative override
    // is in the root frame (same as the scroll-container's parent
    // frame in this single-child-of-root layout) — mirrors the
    // `anchor_target_is_sticky_anchored_tracks_displaced_position` test
    // in `tests/layout_sticky.rs`.
    let scroll = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(300.0)
                .height_px(500.0)
                .overflow_y(OverflowMode::Scroll),
            ScrollOffset { x: 0.0, y: 100.0 },
        ))
        .id();
    let content = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(1000.0)))
        .id();
    app.world_mut().entity_mut(scroll).add_children(&[content]);
    let spacer = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(50.0)))
        .id();
    let sticky_target = app
        .world_mut()
        .spawn((
            Node,
            {
                let mut s = Style::default().width_px(50.0).height_px(50.0);
                s.position = Position {
                    kind: PositionKind::Sticky,
                    inset: Inset {
                        top: Sizing::Length(Length::Px(0.0)),
                        ..Default::default()
                    },
                };
                s
            },
            Anchor {
                anchor_name: Some(AnchorName::Named("test-anchor".into())),
                ..default()
            },
        ))
        .id();
    app.world_mut()
        .entity_mut(content)
        .add_children(&[spacer, sticky_target]);

    let anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(30.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("test-anchor".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(5.0)),
                    conditions: vec![],
                }],
                ..default()
            },
        ))
        .id();

    // Sub-pass 6b — a minimal table (Table > Row > Cell). The real 6b
    // algorithm places the cell into a column grid and writes a
    // corrected position to PostTaffyPositionOverrides (Phase 12).
    let table_cell = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .display(Display::TableCell)
                .width_px(40.0)
                .height_px(20.0),
        ))
        .id();
    let table_row = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::TableRow)))
        .add_child(table_cell)
        .id();
    let _table_entity = app
        .world_mut()
        .spawn((Node, Style::default().display(Display::Table)))
        .add_child(table_row)
        .id();

    // Sub-pass 6c — MultiColumn entity (warns once per session total).
    let _multicol_entity = app.world_mut().spawn((Node, MultiColumn::default())).id();

    // Single update is sufficient: sticky_offset (6a) and
    // anchor_resolution (6d) are chained in `PostTaffyOverrides` via
    // `.chain()` (see layout/mod.rs ~line 180), so the anchor pass
    // reads the displacement the sticky pass just wrote, on the same
    // frame.
    app.update();

    // Order assertion — the 9-step chain ran in declared order.
    let observed_full = order.lock().unwrap().clone();
    let n = observed_full.len();
    assert_eq!(
        n, 9,
        "expected exactly one full pipeline cycle ({} entries); got {} entries: {:?}",
        9, n, observed_full,
    );
    let observed = &observed_full[..];
    assert_eq!(
        observed,
        &[
            "gc",
            "wmi",
            "sync",
            "cq_activate",
            "taffy",
            "cq_flip",
            "cq_rerun",
            "post_taffy",
            "write",
        ],
        "BuiyLayoutStep sets did not run in declared order; full trace: {:?}",
        observed_full,
    );

    // Sub-pass 6a (sticky_offset) side-effect: the sticky_target gets
    // an override entry whose y equals the displaced position. With
    // ScrollOffset.y = 100, top inset = 0, natural y_in_block = 50,
    // threshold = 100 → displaced y_in_block = 100.
    let overrides = app.world().resource::<PostTaffyPositionOverrides>();
    let sticky_pos = overrides
        .by_entity
        .get(&sticky_target)
        .copied()
        .unwrap_or_else(|| panic!("expected sticky_target in override map after sub-pass 6a"));
    assert_eq!(
        sticky_pos.y, 100.0,
        "sub-pass 6a (sticky_offset) should displace sticky_target to y=100; got {:?}",
        sticky_pos,
    );

    // Sub-pass 6d (anchor_resolution) side-effect + the explicit
    // ordering proof: the anchored entity tracks the DISPLACED sticky
    // target. If 6d ran before 6a (wrong order), anchored.y would be
    // 50 (natural target y) + 50 (target h) + 5 (gap) = 105.
    // Correct order (6a before 6d): anchored.y = 100 (displaced
    // target y in scroll-container frame, which equals root frame
    // here) + 50 (target h) + 5 (gap) = 155.
    let anchored_pos = overrides
        .by_entity
        .get(&anchored)
        .copied()
        .unwrap_or_else(|| panic!("expected anchored entity in override map after sub-pass 6d"));
    assert_eq!(
        anchored_pos.y, 155.0,
        "anchored entity must track the DISPLACED sticky target — if y=105 the anchor pass \
         read the natural target position, meaning sub-pass 6a did not run before 6d; got {:?}",
        anchored_pos,
    );

    // ResolvedLayout reflects the override on both entities.
    let sticky_rl = app
        .world()
        .get::<ResolvedLayout>(sticky_target)
        .expect("sticky_target has ResolvedLayout");
    assert_eq!(
        sticky_rl.position.y, 100.0,
        "ResolvedLayout for sticky_target reflects sub-pass 6a override",
    );

    // Sub-pass 6b (table_layout) side-effect: the real algorithm
    // writes a corrected position for the table cell into
    // PostTaffyPositionOverrides (Phase 12 — placing it into the
    // column grid).
    let table_overrides = app.world().resource::<PostTaffyPositionOverrides>();
    assert!(
        table_overrides.by_entity.contains_key(&table_cell),
        "sub-pass 6b (table_layout) should write a position override for the table cell; \
         override keys: {:?}",
        table_overrides.by_entity.keys().collect::<Vec<_>>(),
    );

    // Sub-pass 6c (multicol_pack) side-effect: the per-session
    // MulticolUnsupported sentinel is recorded (no entity payload —
    // first multicol entity triggers, all later are silent).
    let warned = app.world().resource::<LayoutWarnedOnceSession>();
    assert!(
        warned.set.contains(&LayoutWarnOnceKey::MulticolUnsupported),
        "sub-pass 6c (multicol_pack) should record MulticolUnsupported; warn set: {:?}",
        warned.set,
    );
}

#[test]
fn transform_composition_runs_and_writes_resolved_transform() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let e = app
        .world_mut()
        .spawn((
            Node,
            Style::default().ui_transform(UiTransform {
                matrix: TransformMatrix::Translate(Length::px(10.0), Length::px(0.0), Length::ZERO),
                ..Default::default()
            }),
        ))
        .id();

    app.update();

    let rt = app
        .world()
        .get::<ResolvedTransform>(e)
        .expect("6e should write ResolvedTransform for a non-identity UiTransform");
    assert_eq!(rt.matrix, Mat4::from_translation(Vec3::new(10.0, 0.0, 0.0)));
}

#[test]
fn identity_transform_gets_no_resolved_transform() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);

    let e = app
        .world_mut()
        .spawn((Node, Style::default().ui_transform(UiTransform::default())))
        .id();

    app.update();

    assert!(
        app.world().get::<ResolvedTransform>(e).is_none(),
        "identity transform must not produce a ResolvedTransform (spec § 7)"
    );
}

#[test]
fn stacking_context_runs_and_marks_positioned_z_index() {
    let mut app = stacking_app();
    // A root with one positioned + z-index child. `Stacking` is a `Style`
    // bundle field (T10), so set it via the `.stacking()` setter; position
    // is set via the existing `Style::position` builder.
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .stacking(Stacking {
                    z_index: ZIndex::Layer(1),
                    ..Default::default()
                }),
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(child)
        .id();
    app.update();
    // Root always forms a context; the child forms one (positioned+z).
    assert!(
        app.world().get::<StackingContext>(root).is_some(),
        "root forms a context"
    );
    assert!(
        app.world().get::<StackingContext>(child).is_some(),
        "positioned+z child forms a context"
    );
    // The root's painters_z contains the child (it is a descendant painter).
    let root_sc = app.world().get::<StackingContext>(root).unwrap();
    assert!(root_sc.painters_z.contains(&child));
}

#[test]
fn plain_child_gets_no_stacking_context() {
    let mut app = stacking_app();
    let child = app.world_mut().spawn((Node, Style::default())).id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(child)
        .id();
    app.update();
    assert!(
        app.world().get::<StackingContext>(child).is_none(),
        "a plain in-flow child forms no context"
    );
}

#[test]
fn phase9_types_are_registered() {
    let mut app = stacking_app();
    app.update();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    for name in [
        "buiy_core::layout::types::ZIndex",
        "buiy_core::layout::types::Isolation",
        "buiy_core::layout::types::TopLayer",
        "buiy_core::layout::components::Stacking",
        "buiy_core::components::StackingContext",
    ] {
        assert!(
            registry.get_with_type_path(name).is_some(),
            "type not registered: {name}",
        );
    }
}
