//! Phase 9 — stacking + top layer integration tests.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 4, § 6.

use bevy::prelude::*;
use buiy_core::components::StackingContext;
use buiy_core::layout::{
    ContainFlags, Isolation, LayoutPlugin, Length, PositionKind, Style, TopLayer,
    TopLayerActivation, ZIndex,
};
use buiy_core::render::components::{Filter, FilterFn, MixBlendMode, Opacity};
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
fn opacity_forms_stacking_context_and_paints_atomically() {
    // Trigger 5 (render-side former) end-to-end: an `Opacity(0.5)` parent
    // forms a stacking context, so its subtree is ONE atomic entry in the
    // root's painters_z — its children paint inside the parent's OWN list.
    // This is the paint-order atomicity the effect-compositor's contiguity
    // invariant rests on (render/buckets.rs `pack_view_partitioned`): the
    // member carries its OWN z-index stacking context at the positive-z tier
    // (z=2), and the non-member sibling sits at z=1 — before the trigger
    // landed, the root's tier sort interleaved the sibling BETWEEN the parent
    // and its member, splitting the group's run.
    let mut app = app();
    let member = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .z_index(ZIndex::Layer(2)),
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((Node, Style::default(), Opacity(0.5)))
        .add_child(member)
        .id();
    let sibling = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .position(PositionKind::Relative)
                .z_index(ZIndex::Layer(1)),
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[parent, sibling])
        .id();
    app.update();

    let parent_sc = app
        .world()
        .get::<StackingContext>(parent)
        .expect("Opacity(0.5) forms a stacking context (trigger 5)");
    assert!(
        parent_sc.painters_z.contains(&member),
        "the member paints inside the parent's own context"
    );
    let root_sc = app.world().get::<StackingContext>(root).unwrap();
    assert!(
        root_sc.painters_z.contains(&parent),
        "the parent is an atomic painter in the root context"
    );
    assert!(
        !root_sc.painters_z.contains(&member),
        "the member must NOT surface in the root's painters_z — the sibling \
         could otherwise interleave between the parent and its member"
    );
    assert!(
        root_sc.painters_z.contains(&sibling),
        "the non-member sibling stays in the root context"
    );
}

#[test]
fn opaque_opacity_forms_no_stacking_context() {
    // Trigger-5 boundary, end-to-end: `Opacity(1.0)` is the CSS-initial no-op
    // — presence of the component alone must not form a context (guards a
    // query-wiring bug that fires on presence instead of value).
    let mut app = app();
    let child = app
        .world_mut()
        .spawn((Node, Style::default(), Opacity(1.0)))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(child)
        .id();
    app.update();
    assert!(
        app.world().get::<StackingContext>(child).is_none(),
        "Opacity(1.0) (the no-op initial) must not form a stacking context"
    );
}

#[test]
fn filter_forms_stacking_context_end_to_end() {
    // Trigger 5: a non-empty `Filter` forms a stacking context via the real
    // 6f query path.
    let mut app = app();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Filter(vec![FilterFn::Blur(Length::px(2.0))]),
        ))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(child)
        .id();
    app.update();
    assert!(
        app.world().get::<StackingContext>(child).is_some(),
        "a non-empty Filter forms a stacking context (trigger 5)"
    );
}

#[test]
fn mix_blend_mode_forms_stacking_context_end_to_end() {
    // Trigger 5: a non-Normal `MixBlendMode` forms a stacking context via the
    // real 6f query path.
    let mut app = app();
    let child = app
        .world_mut()
        .spawn((Node, Style::default(), MixBlendMode::Multiply))
        .id();
    let _root = app
        .world_mut()
        .spawn((Node, Style::default()))
        .add_child(child)
        .id();
    app.update();
    assert!(
        app.world().get::<StackingContext>(child).is_some(),
        "a non-Normal MixBlendMode forms a stacking context (trigger 5)"
    );
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

#[test]
fn paint_rank_matches_documented_order() {
    use buiy_core::layout::top_layer_paint_rank;

    // The single source of truth for top-layer dominance — Fullscreen paints
    // BOTTOM (rank 0), Modal paints TOP (rank 3), `None` is the in-flow
    // sentinel (`u8::MAX`). The *declared* enum order
    // (`None, Modal, Popover, Tooltip, Fullscreen`) is deliberately NOT this
    // order, so `#[derive(Ord)]` on `TopLayer` would give the WRONG dominance;
    // the rank fn is what callers compare on (spec stacking-and-top-layer.md
    // § 4 / verification invariants.md deviation #3).
    assert_eq!(top_layer_paint_rank(TopLayer::Fullscreen), 0);
    assert_eq!(top_layer_paint_rank(TopLayer::Tooltip), 1);
    assert_eq!(top_layer_paint_rank(TopLayer::Popover), 2);
    assert_eq!(top_layer_paint_rank(TopLayer::Modal), 3);
    assert_eq!(top_layer_paint_rank(TopLayer::None), u8::MAX);

    // The rank is strictly increasing along the documented dominance chain,
    // and every escaping variant outranks (paints below) the in-flow sentinel.
    let chain = [
        TopLayer::Fullscreen,
        TopLayer::Tooltip,
        TopLayer::Popover,
        TopLayer::Modal,
    ];
    for pair in chain.windows(2) {
        assert!(
            top_layer_paint_rank(pair[0]) < top_layer_paint_rank(pair[1]),
            "{:?} must paint below {:?}",
            pair[0],
            pair[1],
        );
        assert!(top_layer_paint_rank(pair[0]) < top_layer_paint_rank(TopLayer::None));
    }
}
