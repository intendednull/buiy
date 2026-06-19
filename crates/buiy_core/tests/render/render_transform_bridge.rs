//! Phase R3 — the Transform / GlobalTransform bridge (clip-and-transform.md § B).
//! All tests are HEADLESS: MinimalPlugins + TransformPlugin + CorePlugin +
//! LayoutPlugin, no wgpu adapter, no RenderApp.

use bevy::prelude::*;
use buiy_core::{
    Node, ResolvedLayout,
    layout::{Length, Sizing, Style},
    render::bridge::ScrollDirty,
};

// HEADLESS harness for the bridge: the shared 3-plugin transform-bridge stack
// ([`crate::support::headless_layout_app`]) — TransformPlugin populates the three
// propagation systems CorePlugin chains in Update (§ B.2.1), so reading
// GlobalTransform after `update()` is meaningful.
use crate::support::headless_layout_app as app;

#[test]
fn scroll_dirty_is_empty_in_steady_state() {
    let mut app = app();
    app.world_mut().spawn((
        Node,
        Style {
            box_model: buiy_core::BoxModel {
                width: Sizing::Length(Length::Px(50.0)),
                height: Sizing::Length(Length::Px(50.0)),
                ..Default::default()
            },
            ..Default::default()
        },
    ));
    // Frame 1: spawn frame — everything Changed, so ScrollDirty is non-empty.
    app.update();
    // Frame 2: nothing mutated — ScrollDirty must be empty (the seed observed
    // no Changed inputs).
    app.update();
    let dirty = app.world().resource::<ScrollDirty>();
    assert!(
        dirty.0.is_empty(),
        "steady-state frame must leave ScrollDirty empty, got {:?}",
        dirty.0
    );
}

use buiy_core::ResolvedTransform;

#[test]
fn plain_layout_translation_equals_position_y_down_no_flip() {
    // A single 50×50 box at the root resolves to position (0,0); its
    // Transform.translation is (0,0,0) — y-down, NO flip (the flip is in
    // the GPU view uniform, § B.4).
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: buiy_core::BoxModel {
                    width: Sizing::Length(Length::Px(50.0)),
                    height: Sizing::Length(Length::Px(50.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();
    app.update();
    let pos = app.world().get::<ResolvedLayout>(e).unwrap().position;
    let t = app
        .world()
        .get::<Transform>(e)
        .expect("bridge wrote Transform");
    assert_eq!(t.translation, pos.extend(0.0));
    assert_eq!(t.translation.z, 0.0);
}

#[test]
fn transform_folds_resolved_transform_matrix_into_translation_path() {
    // A translate(15,25) box: ResolvedTransform.matrix is a pure
    // translation, so the composed Transform equals
    // from_translation(position) * matrix. With position (0,0) the
    // resulting translation is (15,25,0).
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(15.0, 25.0)))
        .id();
    app.update();
    let pos = app.world().get::<ResolvedLayout>(e).unwrap().position;
    let rt = app.world().get::<ResolvedTransform>(e).unwrap().matrix;
    let t = app.world().get::<Transform>(e).unwrap();
    // Drive the EXTRACTED production seam (no re-implementation): at a root the
    // accumulated ancestor scroll is zero, so the expected Transform is exactly
    // `compose_buiy_transform(position, ZERO, Some(matrix))`.
    let expected = buiy_core::render::bridge::compose_buiy_transform(pos, Vec2::ZERO, Some(rt));
    assert_eq!(t.translation, expected.translation);
    // translation component equals position + (15,25)
    assert!((t.translation.x - (pos.x + 15.0)).abs() < 1e-4);
    assert!((t.translation.y - (pos.y + 25.0)).abs() < 1e-4);
}

#[test]
fn transform_animating_back_to_identity_clears_stale_translation() {
    // Bug: when a transform returns to identity, sub-pass 6e REMOVES
    // ResolvedTransform. A removal does not match Changed<ResolvedTransform>,
    // and a transform change never rewrites ResolvedLayout, so without
    // re-seeding on RemovedComponents<ResolvedTransform> the walk would never
    // re-run and Transform.translation would stay stale at the old
    // transformed value (15,25) instead of returning to position-only.
    use buiy_core::{TransformMatrix, UiTransform};

    let mut app = app();
    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(15.0, 25.0)))
        .id();
    app.update();

    let pos = app.world().get::<ResolvedLayout>(e).unwrap().position;
    let t = app.world().get::<Transform>(e).unwrap();
    assert!(
        (t.translation.x - (pos.x + 15.0)).abs() < 1e-4,
        "precondition: translate is applied"
    );

    // Animate the transform back to identity: clear UiTransform.matrix. 6e
    // removes ResolvedTransform next frame; the seed must catch the removal.
    app.world_mut().get_mut::<UiTransform>(e).unwrap().matrix = TransformMatrix::None;
    app.update();

    assert!(
        app.world().get::<ResolvedTransform>(e).is_none(),
        "precondition: 6e removed ResolvedTransform on return to identity"
    );
    let t = app.world().get::<Transform>(e).unwrap();
    assert!(
        (t.translation.x - pos.x).abs() < 1e-4 && (t.translation.y - pos.y).abs() < 1e-4,
        "translation must return to position-only, got {:?} (expected {:?})",
        t.translation,
        pos
    );
}

#[test]
fn node_under_non_node_bevy_parent_is_a_walk_root() {
    // Bug: the bridge selected roots with `Without<ChildOf>` alone, so a Buiy
    // Node subtree parented under a plain (non-Node) Bevy entity — which has
    // a ChildOf — was never visited and got no Transform. The walk root
    // predicate must match write_clip_rects' two-disjunct definition: a Node
    // is a root iff it has no ChildOf OR its ChildOf parent is not a Node.
    let mut app = app();
    let non_node_parent = app.world_mut().spawn_empty().id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: buiy_core::BoxModel {
                    width: Sizing::Length(Length::Px(50.0)),
                    height: Sizing::Length(Length::Px(50.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();
    app.world_mut()
        .entity_mut(child)
        .insert(ChildOf(non_node_parent));
    app.update();

    assert!(
        app.world().get::<Transform>(child).is_some(),
        "a Node parented under a non-Node Bevy entity must be walked and get a Transform"
    );
}

#[test]
fn global_transform_is_final_after_update_no_postupdate_needed() {
    // After ONE app.update(), GlobalTransform must already equal the
    // composed Transform — proving the Update propagation chain ran before
    // PostUpdate (the picking/extract window reads it in Update).
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(10.0, 20.0)))
        .id();
    app.update();
    let t = *app.world().get::<Transform>(e).unwrap();
    let gt = app
        .world()
        .get::<GlobalTransform>(e)
        .expect("GlobalTransform present");
    // Root entity: GlobalTransform == GlobalTransform::from(Transform).
    assert_eq!(gt.translation(), t.translation);
}

#[test]
fn nested_transforms_compose_through_global_transform() {
    // Parent translate(100,0), child translate(0,50): the child's
    // GlobalTransform translation is the composed (100,50,0) once Bevy's
    // propagation runs in Update.
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(100.0, 0.0)))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(0.0, 50.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();
    let parent_pos = app.world().get::<ResolvedLayout>(parent).unwrap().position;
    let child_local = app.world().get::<ResolvedLayout>(child).unwrap().position;
    let gt = app.world().get::<GlobalTransform>(child).unwrap();
    // Child global = parent local (pos + 100,0) composed with child local
    // (pos + 0,50). Buiy translations are pure (no rotation/scale here), so
    // the global translation is the sum of the two local translations.
    let expected_x = (parent_pos.x + 100.0) + (child_local.x + 0.0);
    let expected_y = (parent_pos.y + 0.0) + (child_local.y + 50.0);
    assert!(
        (gt.translation().x - expected_x).abs() < 1e-3,
        "x: {} vs {}",
        gt.translation().x,
        expected_x
    );
    assert!(
        (gt.translation().y - expected_y).abs() < 1e-3,
        "y: {} vs {}",
        gt.translation().y,
        expected_y
    );
}

#[test]
fn propagation_runs_in_update_without_transform_plugin_postupdate() {
    // The load-bearing § B.2.1 claim: GlobalTransform is final in the UPDATE
    // window (before Picking/extract), NOT only after PostUpdate. The `app()`
    // harness includes TransformPlugin, whose PostUpdate pass would finalize
    // GlobalTransform regardless — masking a missing Update chain. So build a
    // harness WITHOUT TransformPlugin: now the ONLY propagation is CorePlugin's
    // Update copy of mark_dirty_trees → propagate_parent_transforms →
    // sync_simple_transforms. Without that chain, sync_simple_transforms never
    // runs and a root entity's GlobalTransform stays at its identity default,
    // so this test fails until Task 3 schedules the chain.
    //
    // `crate::support::bare_layout_app()` IS exactly this no-TransformPlugin stack
    // (MinimalPlugins + CorePlugin + LayoutPlugin), so the builder is
    // self-documenting here — its whole reason to exist is the deliberate
    // omission this test relies on.
    let mut app = crate::support::bare_layout_app();

    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(10.0, 20.0)))
        .id();
    app.update();

    let t = *app
        .world()
        .get::<Transform>(e)
        .expect("bridge wrote Transform");
    let gt = app
        .world()
        .get::<GlobalTransform>(e)
        .expect("Transform's required GlobalTransform companion present");
    assert_eq!(
        gt.translation(),
        t.translation,
        "GlobalTransform must be finalized by the Update propagation chain, \
         with no TransformPlugin PostUpdate pass to fall back on"
    );
}

#[test]
fn scroll_offset_folds_into_descendant_translation() {
    use buiy_core::{BoxModel, Overflow, OverflowMode, ScrollOffset};
    let mut app = app();
    // Scroll container (overflow-y: scroll) with one child.
    let container = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(100.0)),
                    height: Sizing::Length(Length::Px(100.0)),
                    ..Default::default()
                },
                overflow: Overflow {
                    y: OverflowMode::Scroll,
                    ..Default::default()
                },
                ..Default::default()
            },
            ScrollOffset::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style {
                box_model: BoxModel {
                    width: Sizing::Length(Length::Px(50.0)),
                    height: Sizing::Length(Length::Px(50.0)),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();
    app.world_mut().entity_mut(container).add_child(child);
    app.update();

    let gt_before = app
        .world()
        .get::<GlobalTransform>(child)
        .unwrap()
        .translation();
    let layout_before = app.world().get::<ResolvedLayout>(child).unwrap().position;

    // Scroll down by 30px: child content moves UP by 30 (translation.y −30).
    app.world_mut()
        .get_mut::<ScrollOffset>(container)
        .unwrap()
        .y = 30.0;
    app.update();

    let gt_after = app
        .world()
        .get::<GlobalTransform>(child)
        .unwrap()
        .translation();
    let layout_after = app.world().get::<ResolvedLayout>(child).unwrap().position;

    assert!(
        (gt_after.y - (gt_before.y - 30.0)).abs() < 1e-3,
        "child translation must fold in −scroll: before {} after {}",
        gt_before.y,
        gt_after.y
    );
    // ResolvedLayout is byte-stable across scroll (§ A.4 / layout invariant).
    assert_eq!(
        layout_before, layout_after,
        "scroll must not move ResolvedLayout"
    );
}

#[test]
fn from_matrix_drops_projective_perspective_row_keeps_affine() {
    // clip-and-transform.md § B.2 / § B.5: Transform::from_matrix decomposes
    // to TRS and drops any projective row, so a perspective term in the
    // ResolvedTransform Mat4 cannot survive the bridge — perspective must
    // ride a separate render-side channel (C-tier, deferred). This test
    // pins WHY the bridge is Flat-only.
    let mut m = Mat4::from_translation(Vec3::new(7.0, 0.0, 0.0));
    // Inject a projective term. glam `Mat4` is column-major, so the
    // conceptual bottom (projective) row of the math matrix is the set of
    // `.w` components of the four column-axes; `z_axis.w` is a real
    // perspective-on-z term, while `w_axis.xyz` holds the translation.
    m.w_axis.w = 1.0;
    m.z_axis.w = -0.01; // perspective on z
    let t = Transform::from_matrix(m);
    // The affine translation survives.
    assert!((t.translation.x - 7.0).abs() < 1e-4);
    // Reconstructing the Mat4 from the decomposed TRS has a pure-affine
    // projective row — the perspective term is gone — and the translation
    // column is preserved.
    let round_trip = t.to_matrix();
    assert_eq!(round_trip.z_axis.w, 0.0, "projective z-perspective dropped");
    // Projective row (column-axis `.w` components) is the affine (0,0,0,1):
    // no perspective on any axis survives the TRS decomposition.
    assert_eq!(
        Vec4::new(
            round_trip.x_axis.w,
            round_trip.y_axis.w,
            round_trip.z_axis.w,
            round_trip.w_axis.w,
        ),
        Vec4::new(0.0, 0.0, 0.0, 1.0),
        "projective row dropped to pure-affine (0,0,0,1)"
    );
    // The translation column (w_axis.xyz) is the surviving affine part.
    assert_eq!(round_trip.w_axis, Vec4::new(7.0, 0.0, 0.0, 1.0));
}

#[test]
fn render_spatial_source_is_global_transform_not_resolved_layout() {
    // Pillar 5: render reads GlobalTransform, not ResolvedLayout, for
    // position. With a transform present, GlobalTransform.translation
    // differs from ResolvedLayout.position by exactly the transform — so a
    // consumer reading ResolvedLayout would paint in the wrong place.
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(40.0, 0.0)))
        .id();
    app.update();
    let layout = app.world().get::<ResolvedLayout>(e).unwrap().position;
    let gt = app
        .world()
        .get::<GlobalTransform>(e)
        .unwrap()
        .translation()
        .truncate();
    assert!((gt.x - (layout.x + 40.0)).abs() < 1e-3);
    assert_ne!(
        gt, layout,
        "render must read GlobalTransform, not ResolvedLayout"
    );
}
