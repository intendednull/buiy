//! Perf audit #3 — gating the post-Taffy override chain.
//!
//! The override chain (`clear_post_taffy_overrides` → sticky → table → multicol
//! → anchor → transform_composition → stacking_context) is the
//! `BuiyLayoutStep::PostTaffyOverrides` set, now gated by `seed_layout_dirty`'s
//! per-frame `LayoutDirtyThisFrame` flag so an idle frame skips it entirely.
//!
//! ## The completeness proof (the differential property test)
//!
//! Gating OFF a frame on which no override input changed is output-identical
//! ONLY IF the seed fires on EVERY input the chain reads. If the seed
//! under-gates an input, that input's edit never re-runs the chain (or, for
//! geometry, self-heals a frame late) and the gated pipeline DIVERGES from a
//! fresh full build that ran everything via `Added`.
//!
//! Each `diff_*` test mutates ONE input kind on a settled scene `A`, settles,
//! then builds an independent scene `B` FRESH in the SAME end state (a fresh
//! build always runs the whole chain) and asserts A's `ResolvedLayout` +
//! `ResolvedTransform` + `StackingContext` + `PostTaffyPositionOverrides` equal
//! B's for every tracked entity. A divergence is a missing seed term.
//!
//! ## The gate tests
//!
//! `LayoutPostTaffyRunCount` reads 0 on a gated-off idle frame and >= 1 when the
//! chain runs — the deterministic instrument the idle/mutated/cold assertions use.

use std::collections::HashMap;

use bevy::prelude::*;
use buiy_core::components::StackingContext;
use buiy_core::layout::{
    BoxModel, Inset, LayoutPostTaffyRunCount, Length, OverflowMode, Position, PositionKind,
    ScrollOffset, Sizing, Stacking, Style, TransformMatrix, UiTransform, ZIndex,
};
use buiy_core::{Node, PostTaffyPositionOverrides, ResolvedLayout, ResolvedTransform};

use crate::support::{bare_layout_app, settle};

// ---------------------------------------------------------------------------
// Scene building.
// ---------------------------------------------------------------------------

/// A child kind for the flat differential base scene. Each exercises a distinct
/// override-chain output: `Plain` only `ResolvedLayout`; `Translated` also
/// `ResolvedTransform` (transform_composition); `Stacked` also `StackingContext`
/// (stacking_context, via a positioned + z-indexed node).
#[derive(Clone, Copy)]
enum Child {
    Plain { w: f32 },
    Translated { tx: f32, ty: f32 },
    Stacked { z: i32 },
}

/// Build a flat flex-column scene of the given children into `app`. Returns the
/// tracked entities in canonical order `[root, child0, child1, …]` (the order
/// `B` is rebuilt in, so two scenes compare positionally, not by raw id).
fn build(app: &mut App, children: &[Child]) -> Vec<Entity> {
    let kids: Vec<Entity> = children
        .iter()
        .map(|spec| {
            let style = match *spec {
                Child::Plain { w } => Style::default().width_px(w).height_px(20.0),
                Child::Translated { tx, ty } => Style::default()
                    .width_px(40.0)
                    .height_px(20.0)
                    .translate_px(tx, ty),
                Child::Stacked { z } => Style::default()
                    .width_px(40.0)
                    .height_px(20.0)
                    .position(PositionKind::Relative)
                    .stacking(Stacking {
                        z_index: ZIndex::Layer(z),
                        ..default()
                    }),
            };
            app.world_mut().spawn((Node, style)).id()
        })
        .collect();
    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(800.0)
                .padding(8.0)
                .gap_px(4.0),
        ))
        .id();
    app.world_mut().entity_mut(root).add_children(&kids);
    let mut ents = vec![root];
    ents.extend(kids);
    ents
}

/// The standard base children used by the mutate-in-place cases: a plain, a
/// translated, a stacked, and a trailing plain node.
fn base_children() -> [Child; 4] {
    [
        Child::Plain { w: 40.0 },
        Child::Translated { tx: 5.0, ty: 0.0 },
        Child::Stacked { z: 1 },
        Child::Plain { w: 40.0 },
    ]
}

// ---------------------------------------------------------------------------
// Output comparison.
// ---------------------------------------------------------------------------

/// Assert the gated pipeline's per-entity override outputs (`ResolvedLayout`,
/// `ResolvedTransform`, `StackingContext`, `PostTaffyPositionOverrides`) for
/// `a_ents` equal the fresh reference's for `b_ents`, position-by-position.
///
/// `StackingContext.painters_z` and the override map are keyed by `Entity` (raw
/// ids differ across the two apps), so they are remapped to the canonical
/// tracked index before comparison.
fn assert_outputs_equiv(
    a: &mut App,
    a_ents: &[Entity],
    b: &mut App,
    b_ents: &[Entity],
    case: &str,
) {
    assert_eq!(
        a_ents.len(),
        b_ents.len(),
        "{case}: tracked entity count differs (A={}, B={})",
        a_ents.len(),
        b_ents.len()
    );
    let a_idx: HashMap<Entity, usize> = a_ents.iter().enumerate().map(|(i, &e)| (e, i)).collect();
    let b_idx: HashMap<Entity, usize> = b_ents.iter().enumerate().map(|(i, &e)| (e, i)).collect();

    for i in 0..a_ents.len() {
        let (ea, eb) = (a_ents[i], b_ents[i]);

        let rl_a = a
            .world()
            .get::<ResolvedLayout>(ea)
            .map(|r| (r.position, r.size));
        let rl_b = b
            .world()
            .get::<ResolvedLayout>(eb)
            .map(|r| (r.position, r.size));
        assert_eq!(rl_a, rl_b, "{case}: ResolvedLayout differs at tracked #{i}");

        let rt_a = a.world().get::<ResolvedTransform>(ea).map(|t| t.matrix);
        let rt_b = b.world().get::<ResolvedTransform>(eb).map(|t| t.matrix);
        assert_eq!(
            rt_a, rt_b,
            "{case}: ResolvedTransform differs at tracked #{i}"
        );

        let sc_a = a.world().get::<StackingContext>(ea).map(|sc| {
            (
                sc.painters_z
                    .iter()
                    .map(|e| a_idx.get(e).copied())
                    .collect::<Vec<_>>(),
                sc.cross_root_rank,
            )
        });
        let sc_b = b.world().get::<StackingContext>(eb).map(|sc| {
            (
                sc.painters_z
                    .iter()
                    .map(|e| b_idx.get(e).copied())
                    .collect::<Vec<_>>(),
                sc.cross_root_rank,
            )
        });
        assert_eq!(
            sc_a, sc_b,
            "{case}: StackingContext differs at tracked #{i}"
        );

        let ov_a = a
            .world()
            .resource::<PostTaffyPositionOverrides>()
            .by_entity
            .get(&ea)
            .copied();
        let ov_b = b
            .world()
            .resource::<PostTaffyPositionOverrides>()
            .by_entity
            .get(&eb)
            .copied();
        assert_eq!(
            ov_a, ov_b,
            "{case}: PostTaffyPositionOverrides differs at tracked #{i}"
        );
    }
}

// ---------------------------------------------------------------------------
// Differential property cases — one per mutation kind.
// ---------------------------------------------------------------------------

#[test]
fn diff_geometry_change_matches_fresh() {
    let children = base_children();
    let mut a = bare_layout_app();
    let ea = build(&mut a, &children);
    settle(&mut a);
    // (a) geometry: child0 width 40 -> 80 (mutates BoxModel; cascades the
    // flex-column siblings' positions).
    a.world_mut().get_mut::<BoxModel>(ea[1]).unwrap().width = Sizing::Length(Length::px(80.0));
    settle(&mut a);

    let mut fresh = children;
    fresh[0] = Child::Plain { w: 80.0 };
    let mut b = bare_layout_app();
    let eb = build(&mut b, &fresh);
    settle(&mut b);

    assert_outputs_equiv(&mut a, &ea, &mut b, &eb, "geometry(width)");
}

#[test]
fn diff_transform_change_matches_fresh() {
    let children = base_children();
    let mut a = bare_layout_app();
    let ea = build(&mut a, &children);
    settle(&mut a);
    // (b) transform: retarget child1's UiTransform. A transform does NOT change
    // ResolvedLayout, so this can ONLY self-heal via the explicit
    // Changed<UiTransform> seed term — the case that would break under-gating.
    a.world_mut().get_mut::<UiTransform>(ea[2]).unwrap().matrix =
        TransformMatrix::Translate(Length::px(17.0), Length::px(3.0), Length::ZERO);
    settle(&mut a);

    let mut fresh = children;
    fresh[1] = Child::Translated { tx: 17.0, ty: 3.0 };
    let mut b = bare_layout_app();
    let eb = build(&mut b, &fresh);
    settle(&mut b);

    assert_outputs_equiv(&mut a, &ea, &mut b, &eb, "transform");
}

#[test]
fn diff_stacking_change_matches_fresh() {
    let children = base_children();
    let mut a = bare_layout_app();
    let ea = build(&mut a, &children);
    settle(&mut a);
    // (c) stacking: bump child2's z-index. Also a non-geometry change — only the
    // explicit Changed<Stacking> seed term re-runs stacking_context.
    a.world_mut().get_mut::<Stacking>(ea[3]).unwrap().z_index = ZIndex::Layer(7);
    settle(&mut a);

    let mut fresh = children;
    fresh[2] = Child::Stacked { z: 7 };
    let mut b = bare_layout_app();
    let eb = build(&mut b, &fresh);
    settle(&mut b);

    assert_outputs_equiv(&mut a, &ea, &mut b, &eb, "stacking(z-index)");
}

#[test]
fn diff_add_child_matches_fresh() {
    let children = base_children();
    let mut a = bare_layout_app();
    let mut ea = build(&mut a, &children);
    settle(&mut a);
    // (d) hierarchy: append a child to root.
    let c4 = a
        .world_mut()
        .spawn((Node, Style::default().width_px(40.0).height_px(20.0)))
        .id();
    a.world_mut().entity_mut(ea[0]).add_child(c4);
    ea.push(c4);
    settle(&mut a);

    let mut fresh = children.to_vec();
    fresh.push(Child::Plain { w: 40.0 });
    let mut b = bare_layout_app();
    let eb = build(&mut b, &fresh);
    settle(&mut b);

    assert_outputs_equiv(&mut a, &ea, &mut b, &eb, "add_child");
}

#[test]
fn diff_despawn_matches_fresh() {
    let children = base_children();
    let mut a = bare_layout_app();
    let mut ea = build(&mut a, &children);
    settle(&mut a);
    // (e) despawn the trailing child.
    let victim = ea[4];
    a.world_mut().entity_mut(victim).despawn();
    ea.remove(4);
    settle(&mut a);

    // Fresh reference with only the first three children.
    let fresh = &children[..3];
    let mut b = bare_layout_app();
    let eb = build(&mut b, fresh);
    settle(&mut b);

    assert_outputs_equiv(&mut a, &ea, &mut b, &eb, "despawn");
}

#[test]
fn diff_reparent_matches_fresh() {
    let children = base_children();
    let mut a = bare_layout_app();
    let ea = build(&mut a, &children);
    settle(&mut a);
    // (f) reparent: move child3 (ea[4]) from root under child0 (ea[1]).
    let (root, c0, c3) = (ea[0], ea[1], ea[4]);
    a.world_mut().entity_mut(c0).add_child(c3);
    // canonical order [root, c0, c1, c2, c3] is unchanged (same entities).
    settle(&mut a);

    // Fresh reference: build flat, then nest c3 under c0 so the END structure
    // matches. Tracked order must mirror A's [root, c0, c1, c2, c3].
    let mut b = bare_layout_app();
    let eb_flat = build(&mut b, &children);
    let (b_c0, b_c3) = (eb_flat[1], eb_flat[4]);
    b.world_mut().entity_mut(b_c0).add_child(b_c3);
    settle(&mut b);

    assert_outputs_equiv(&mut a, &ea, &mut b, &eb_flat, "reparent");
    let _ = root; // (documents the reparent source for the reader)
}

#[test]
fn diff_scroll_offset_matches_fresh() {
    // (g) scroll: a sticky child inside a scroll container. Changing
    // ScrollOffset must re-run sticky_offset (writing PostTaffyPositionOverrides)
    // even though ScrollOffset is excluded from sync_styles' filter — the seed
    // adds it back explicitly for the override gate.
    fn build_sticky(app: &mut App, offset_y: f32) -> Vec<Entity> {
        let spacer = app
            .world_mut()
            .spawn((Node, Style::default().width_px(300.0).height_px(50.0)))
            .id();
        let sticky = app
            .world_mut()
            .spawn((Node, {
                let mut s = Style::default().width_px(50.0).height_px(50.0);
                s.position = Position {
                    kind: PositionKind::Sticky,
                    inset: Inset {
                        top: Sizing::Length(Length::Px(0.0)),
                        ..default()
                    },
                };
                s
            }))
            .id();
        let content = app
            .world_mut()
            .spawn((Node, Style::default().width_px(300.0).height_px(1000.0)))
            .id();
        app.world_mut()
            .entity_mut(content)
            .add_children(&[spacer, sticky]);
        let scroll = app
            .world_mut()
            .spawn((
                Node,
                Style::default()
                    .width_px(300.0)
                    .height_px(500.0)
                    .overflow_y(OverflowMode::Scroll),
                ScrollOffset {
                    x: 0.0,
                    y: offset_y,
                },
            ))
            .id();
        app.world_mut().entity_mut(scroll).add_child(content);
        // canonical: [scroll, content, spacer, sticky]
        vec![scroll, content, spacer, sticky]
    }

    let mut a = bare_layout_app();
    let ea = build_sticky(&mut a, 0.0);
    settle(&mut a);
    a.world_mut().get_mut::<ScrollOffset>(ea[0]).unwrap().y = 120.0;
    settle(&mut a);

    let mut b = bare_layout_app();
    let eb = build_sticky(&mut b, 120.0);
    settle(&mut b);

    // Sanity: the sticky entity actually has a displacement override (so this
    // case exercises PostTaffyPositionOverrides, not a trivially-empty map).
    assert!(
        a.world()
            .resource::<PostTaffyPositionOverrides>()
            .by_entity
            .contains_key(&ea[3]),
        "scroll case did not produce a sticky override — fixture is not exercising the map",
    );

    assert_outputs_equiv(&mut a, &ea, &mut b, &eb, "scroll_offset");
}

// ---------------------------------------------------------------------------
// Gate tests — the LayoutPostTaffyRunCount instrument.
// ---------------------------------------------------------------------------

#[test]
fn post_taffy_chain_skipped_on_idle_frame() {
    let children = base_children();
    let mut app = bare_layout_app();
    let ents = build(&mut app, &children);
    settle(&mut app);

    // A clean idle frame: nothing changed since settle converged.
    app.update();
    assert_eq!(
        app.world().resource::<LayoutPostTaffyRunCount>().0,
        0,
        "the post-Taffy override chain must be GATED OFF on an idle frame (#3)",
    );

    // Mutating one node makes the frame dirty -> the chain runs.
    app.world_mut().get_mut::<BoxModel>(ents[1]).unwrap().width = Sizing::Length(Length::px(80.0));
    app.update();
    assert!(
        app.world().resource::<LayoutPostTaffyRunCount>().0 >= 1,
        "the override chain must run on a frame that mutated a layout input",
    );

    // The frame AFTER the mutation settles is idle again -> gated off.
    settle(&mut app);
    app.update();
    assert_eq!(
        app.world().resource::<LayoutPostTaffyRunCount>().0,
        0,
        "the chain must gate off again once the mutation has settled",
    );
}

#[test]
fn cold_first_frame_runs_chain() {
    // The first frame has everything Added; Changed includes Added, so the seed
    // is dirty and the whole cold build runs the override chain.
    let mut app = bare_layout_app();
    let _ = build(&mut app, &[Child::Plain { w: 40.0 }]);
    app.update();
    assert!(
        app.world().resource::<LayoutPostTaffyRunCount>().0 >= 1,
        "the cold first frame (all components Added) must run the override chain",
    );
}

#[test]
fn transform_only_change_dirties_gate() {
    // A transform-only edit produces NO ResolvedLayout change, so the gate can
    // only fire via the explicit Changed<UiTransform> term. Guards that the
    // non-self-healing inputs are wired into the seed.
    let children = base_children();
    let mut app = bare_layout_app();
    let ents = build(&mut app, &children);
    settle(&mut app);
    app.update();
    assert_eq!(app.world().resource::<LayoutPostTaffyRunCount>().0, 0);

    app.world_mut()
        .get_mut::<UiTransform>(ents[2])
        .unwrap()
        .matrix = TransformMatrix::Translate(Length::px(9.0), Length::px(0.0), Length::ZERO);
    app.update();
    assert!(
        app.world().resource::<LayoutPostTaffyRunCount>().0 >= 1,
        "a transform-only change must dirty the gate via Changed<UiTransform>",
    );
}
