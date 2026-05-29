//! Phase 9 — stacking + top layer integration tests.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 4, § 6.

use bevy::prelude::*;
use buiy_core::components::StackingContext;
use buiy_core::layout::{
    ContainFlags, Isolation, LayoutPlugin, PositionKind, Style, TopLayer, TopLayerActivation,
    ZIndex,
};
use buiy_core::{CorePlugin, Node};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

#[test]
fn top_layer_modal_escapes_to_root() {
    let mut app = app();
    let modal = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Modal)))
        .id();
    // The parent forms its OWN stacking context (isolate) so the
    // "modal absent from parent" assertion below actually executes — with a
    // plain `Style::default()` parent (no SC) the branch was vacuous. This
    // is the spec § 6 escape: the modal escapes a real ancestor context.
    let parent = app
        .world_mut()
        .spawn((Node, Style::default().isolation(Isolation::Isolate)))
        .add_child(modal)
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(parent)
        .id();
    app.update();
    // Membership = root, not parent (spec § 4.1 / § 6 escape test).
    let root_sc = app.world().get::<StackingContext>(root).unwrap();
    assert!(
        root_sc.painters_z.contains(&modal),
        "modal escapes to root context"
    );
    // The parent DOES form a context (isolate); the modal must not appear in
    // it — it escaped. This assertion now genuinely runs.
    let parent_sc = app
        .world()
        .get::<StackingContext>(parent)
        .expect("parent forms a stacking context via Isolation::Isolate");
    assert!(
        !parent_sc.painters_z.contains(&modal),
        "modal must not be counted in its parent's context (it escaped)"
    );
}

#[test]
fn transform_forms_stacking_context_end_to_end() {
    // Trigger 3 via the REAL 6e→6f handoff: a non-identity transform makes
    // `transform_composition` (6e) write `ResolvedTransform`, which 6f reads
    // (`transformed.get(e).is_ok()`) to form a stacking context. Exercises
    // the cross-system wiring, not a literal `has_transform: true`.
    let mut app = app();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(10.0, 0.0)))
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(child)
        .id();
    app.update();
    assert!(
        app.world().get::<StackingContext>(child).is_some(),
        "non-identity transform forms a stacking context (trigger 3, via 6e ResolvedTransform)"
    );
    let root_sc = app.world().get::<StackingContext>(root).unwrap();
    assert!(
        root_sc.painters_z.contains(&child),
        "transformed child is an atomic painter in the root context"
    );
}

#[test]
fn paint_containment_forms_stacking_context_end_to_end() {
    // Trigger 4 via the real `containment_q.get(e)` path in 6f.
    let mut app = app();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().contain(ContainFlags::PAINT)))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(child)
        .id();
    app.update();
    assert!(
        app.world().get::<StackingContext>(child).is_some(),
        "PAINT containment forms a stacking context (trigger 4)"
    );
}

#[test]
fn parentless_top_layer_does_not_self_reference() {
    // Regression (review finding B1): a top-layer entity that is itself a
    // root has no parent context to escape from, so it must NOT be appended
    // to its own `painters_z` — a self-edge would make a paint-order walk
    // recurse infinitely.
    let mut app = app();
    let modal = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Modal)))
        .id();
    app.update();
    if let Some(sc) = app.world().get::<StackingContext>(modal) {
        assert!(
            !sc.painters_z.contains(&modal),
            "a parentless top-layer root must not list itself in painters_z"
        );
    }
}

#[test]
fn top_layer_activation_tracks_open_order() {
    let mut app = app();
    let a = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Popover)))
        .id();
    let b = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Popover)))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[a, b])
        .id();
    app.update();
    let act = app.world().resource::<TopLayerActivation>();
    let order: Vec<Entity> = act.order.iter().copied().collect();
    assert_eq!(
        order,
        vec![a, b],
        "activation order follows tree/open order; most recent last"
    );
}

#[test]
fn z_index_ordering_neg_zero_pos() {
    // spec § 6: three positioned siblings z=[2,-1,0] → painters_z [-1,0,2].
    let mut app = app();
    let z2 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .z_index(ZIndex::Layer(2)),
        ))
        .id();
    let zneg = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .z_index(ZIndex::Layer(-1)),
        ))
        .id();
    let z0 = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .z_index(ZIndex::Layer(0)),
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[z2, zneg, z0])
        .id();
    app.update();
    let sc = app.world().get::<StackingContext>(root).unwrap();
    // The three positioned children are themselves SC roots (positioned+z),
    // so they appear as atomic entries in root.painters_z, ordered by z.
    let order: Vec<Entity> = sc
        .painters_z
        .iter()
        .copied()
        .filter(|e| [z2, zneg, z0].contains(e))
        .collect();
    assert_eq!(
        order,
        vec![zneg, z0, z2],
        "painters ordered by z-index [-1,0,2]"
    );
}

#[test]
fn static_z_index_paints_in_document_order() {
    // spec § 6: static element + z-index 5 paints in document order, not lifted.
    let mut app = app();
    let a = app.world_mut().spawn((Node, Style::default())).id(); // first, static
    let b = app
        .world_mut()
        .spawn((Node, Style::default().z_index(ZIndex::Layer(5))))
        .id(); // static z=5
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[a, b])
        .id();
    app.update();
    let sc = app.world().get::<StackingContext>(root).unwrap();
    let order: Vec<Entity> = sc
        .painters_z
        .iter()
        .copied()
        .filter(|e| [a, b].contains(e))
        .collect();
    assert_eq!(
        order,
        vec![a, b],
        "static z-index ignored; document order preserved"
    );
    assert!(
        app.world().get::<StackingContext>(b).is_none(),
        "static+z forms no context"
    );
}

#[test]
fn isolation_forms_stacking_context() {
    // spec § 6: Isolation::Isolate → a StackingContext appears.
    let mut app = app();
    let iso = app
        .world_mut()
        .spawn((Node, Style::default().isolation(Isolation::Isolate)))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(iso)
        .id();
    app.update();
    assert!(app.world().get::<StackingContext>(iso).is_some());
}

#[test]
fn mixed_top_layer_tiers_order_tooltip_below_modal() {
    // spec § 6: Modal + Tooltip open → Tooltip below Modal regardless of activation order.
    let mut app = app();
    // activate modal first, tooltip second — tier must still win.
    let modal = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Modal)))
        .id();
    let tooltip = app
        .world_mut()
        .spawn((Node, Style::default().top_layer(TopLayer::Tooltip)))
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[modal, tooltip])
        .id();
    app.update();
    let sc = app.world().get::<StackingContext>(root).unwrap();
    let mi = sc.painters_z.iter().position(|e| *e == modal).unwrap();
    let ti = sc.painters_z.iter().position(|e| *e == tooltip).unwrap();
    assert!(
        ti < mi,
        "tooltip paints below modal (earlier in painters_z) regardless of activation"
    );
}
