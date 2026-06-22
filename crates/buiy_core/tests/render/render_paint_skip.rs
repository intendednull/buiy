//! Render-prep subtree visibility-suppression pass (`write_paint_skip`) —
//! the computed `ComputedPaintSkip` marker.
//!
//! HEADLESS: the pass is a plain `Update` ECS system, no wgpu adapter — the
//! same app shape as tests/render_clip_rects.rs. The one `#[ignore]` test at
//! the bottom is the GPU smoke (extract → prepare consumption on a real
//! adapter).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/
//! paint-order-and-top-layer.md § 5.3 / § 5.4; design note (Option A):
//! 2026-06-06-render-subtree-visibility-suppression-design.md.

use bevy::prelude::*;
use buiy_core::render::components::{ComputedPaintSkip, OffscreenAuto, SkipReason};
use buiy_core::{
    CorePlugin, CssVisibility, Node,
    layout::{LayoutPlugin, Style},
    render::BuiyRenderPlugin,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyRenderPlugin);
    app
}

/// A 50x50 styled node — enough for layout to resolve a box (the pass itself
/// reads no geometry, but real trees always carry `Style`).
fn node_bundle() -> (Node, Style) {
    (Node, Style::default().width_px(50.0).height_px(50.0))
}

fn marker(app: &App, e: Entity) -> Option<ComputedPaintSkip> {
    app.world().get::<ComputedPaintSkip>(e).copied()
}

/// The § 5.4 gap this follow-up closes: a `Visible`/default child of a
/// `CssVisibility::Hidden` parent must be paint-suppressed too. Before the
/// pass, only the parent (the leaf skip) was dropped — the child kept its
/// `painters_z` entry and painted.
#[test]
fn hidden_parent_suppresses_visible_default_child() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let child = app.world_mut().spawn(node_bundle()).id(); // no CssVisibility
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    assert_eq!(
        marker(&app, child),
        Some(ComputedPaintSkip {
            reason: SkipReason::CssHidden
        }),
        "a default-visibility child inside a Hidden subtree must carry the \
         computed paint-skip marker (§ 5.4 subtree scope)"
    );
    assert_eq!(
        marker(&app, parent),
        Some(ComputedPaintSkip {
            reason: SkipReason::CssHidden
        }),
        "the Hidden root itself carries the marker (extract's single skip source)"
    );
}

/// Parity with the old per-entity leaf skip: a lone Hidden entity is marked.
#[test]
fn hidden_leaf_entity_is_marked() {
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    app.update();
    assert_eq!(
        marker(&app, e),
        Some(ComputedPaintSkip {
            reason: SkipReason::CssHidden
        })
    );
}

/// A fully-visible tree gets no markers — explicit `Visible` and absent
/// `CssVisibility` are both non-suppressing.
#[test]
fn visible_tree_carries_no_markers() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Visible))
        .id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();
    assert_eq!(marker(&app, parent), None);
    assert_eq!(marker(&app, child), None);
}

/// The removal path: flipping the root back to `Visible` must take the marker
/// OFF every entity in the subtree. `Changed` alone cannot express "no longer
/// suppressed" downstream — the pass must actively remove stale markers.
#[test]
fn hide_then_show_flip_removes_markers_from_subtree() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Visible))
        .id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    // Hide: both get the marker.
    *app.world_mut().get_mut::<CssVisibility>(parent).unwrap() = CssVisibility::Hidden;
    app.update();
    assert!(marker(&app, parent).is_some(), "parent marked after hide");
    assert!(marker(&app, child).is_some(), "child marked after hide");

    // Show: both lose it.
    *app.world_mut().get_mut::<CssVisibility>(parent).unwrap() = CssVisibility::Visible;
    app.update();
    assert_eq!(
        marker(&app, parent),
        None,
        "parent marker removed after show"
    );
    assert_eq!(
        marker(&app, child),
        None,
        "child marker removed after show — a stale marker here is the \
         permanently-vanished-subtree bug"
    );
}

/// REMOVING the `CssVisibility` component entirely (not flipping its value)
/// also un-hides the subtree — the `RemovedComponents<CssVisibility>` seed,
/// which `Changed` cannot see.
#[test]
fn removing_css_visibility_component_lifts_the_suppression() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();
    assert!(marker(&app, child).is_some());

    app.world_mut().entity_mut(parent).remove::<CssVisibility>();
    app.update();
    assert_eq!(marker(&app, parent), None);
    assert_eq!(marker(&app, child), None);
}

/// Nesting + isolation: a hidden grandparent suppresses the grandchild
/// through an unmarked middle node, while an unrelated sibling root tree is
/// untouched.
#[test]
fn hidden_grandparent_suppresses_grandchild_but_not_sibling_tree() {
    let mut app = app();
    let grandparent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let parent = app.world_mut().spawn(node_bundle()).id();
    let grandchild = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(grandparent).add_child(parent);
    app.world_mut().entity_mut(parent).add_child(grandchild);

    // An unrelated visible root tree.
    let other_root = app.world_mut().spawn(node_bundle()).id();
    let other_child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut()
        .entity_mut(other_root)
        .add_child(other_child);

    app.update();

    for e in [grandparent, parent, grandchild] {
        assert_eq!(
            marker(&app, e),
            Some(ComputedPaintSkip {
                reason: SkipReason::CssHidden
            }),
            "every entity in the hidden subtree is marked"
        );
    }
    assert_eq!(marker(&app, other_root), None, "sibling tree untouched");
    assert_eq!(marker(&app, other_child), None, "sibling tree untouched");
}

/// The `OffscreenAuto` variant of the basic case (§ 5.3). Layout does not
/// emit the marker yet, so the test plays the producer and inserts it
/// directly — exactly what the future layout emission will do.
#[test]
fn offscreen_auto_marks_the_subtree() {
    let mut app = app();
    let parent = app.world_mut().spawn((node_bundle(), OffscreenAuto)).id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    assert_eq!(
        marker(&app, parent),
        Some(ComputedPaintSkip {
            reason: SkipReason::OffscreenAuto
        })
    );
    assert_eq!(
        marker(&app, child),
        Some(ComputedPaintSkip {
            reason: SkipReason::OffscreenAuto
        }),
        "OffscreenAuto suppression is subtree-scoped, same as Hidden (§ 5.3)"
    );

    // Removing the layout-written marker lifts the suppression (the
    // RemovedComponents<OffscreenAuto> seed).
    app.world_mut().entity_mut(parent).remove::<OffscreenAuto>();
    app.update();
    assert_eq!(marker(&app, parent), None);
    assert_eq!(marker(&app, child), None);
}

/// An entity's OWN skip input takes precedence for its marker's reason and
/// survives an ancestor un-hide: a child that is itself suppressed keeps its
/// marker (with its own reason) when the hidden parent flips visible.
#[test]
fn own_suppression_survives_ancestor_unhide() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let child = app.world_mut().spawn((node_bundle(), OffscreenAuto)).id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    // While both suppress, the child's OWN reason wins over the inherited one.
    assert_eq!(
        marker(&app, child),
        Some(ComputedPaintSkip {
            reason: SkipReason::OffscreenAuto
        }),
        "own skip input takes precedence over the inherited reason"
    );

    *app.world_mut().get_mut::<CssVisibility>(parent).unwrap() = CssVisibility::Visible;
    app.update();
    assert_eq!(marker(&app, parent), None, "un-hidden parent unmarked");
    assert_eq!(
        marker(&app, child),
        Some(ComputedPaintSkip {
            reason: SkipReason::OffscreenAuto
        }),
        "the child's own suppression must survive the ancestor flip"
    );
}

/// A child spawned LATER under an already-hidden parent must be marked — the
/// `Changed<Children>` / `Added<ChildOf>` hierarchy seed.
#[test]
fn late_spawned_child_under_hidden_parent_is_marked() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    app.update(); // parent settles, seeds drain

    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();
    assert_eq!(
        marker(&app, child),
        Some(ComputedPaintSkip {
            reason: SkipReason::CssHidden
        }),
        "a child added to a settled hidden subtree must be marked (hierarchy seed)"
    );
}

/// Reparenting INTO a hidden subtree must ADD the marker — the symmetric
/// counterpart of the move-out test below (`Changed<ChildOf>` seed; the
/// full-walk visits the child under its new hidden ancestor).
#[test]
fn reparenting_into_hidden_subtree_marks_the_child() {
    let mut app = app();
    let hidden_parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let visible_parent = app.world_mut().spawn(node_bundle()).id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(visible_parent).add_child(child);
    app.update();
    assert_eq!(
        marker(&app, child),
        None,
        "visible subtree carries no marker"
    );

    app.world_mut().entity_mut(hidden_parent).add_child(child);
    app.update();
    assert_eq!(
        marker(&app, child),
        Some(ComputedPaintSkip {
            reason: SkipReason::CssHidden
        }),
        "moving under a hidden parent must inherit the suppression marker"
    );
}

/// Reparenting OUT of a hidden subtree into a visible one must remove the
/// marker — the `Changed<ChildOf>` seed plus the full-walk reconcile.
#[test]
fn reparenting_out_of_hidden_subtree_unmarks_the_child() {
    let mut app = app();
    let hidden_parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let visible_parent = app.world_mut().spawn(node_bundle()).id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(hidden_parent).add_child(child);
    app.update();
    assert!(marker(&app, child).is_some());

    app.world_mut().entity_mut(visible_parent).add_child(child);
    app.update();
    assert_eq!(
        marker(&app, child),
        None,
        "moving into a visible subtree must drop the stale marker"
    );
}

/// Detaching to become a ROOT (removing `ChildOf` outright) must also remove
/// the marker — the `RemovedComponents<ChildOf>` seed (no `Changed` fires on
/// the detached entity, and the old parent may have lost its `Children`
/// component entirely).
#[test]
fn detaching_to_root_unmarks_the_child() {
    let mut app = app();
    let hidden_parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(hidden_parent).add_child(child);
    app.update();
    assert!(marker(&app, child).is_some());

    app.world_mut().entity_mut(child).remove::<ChildOf>();
    app.update();
    assert_eq!(
        marker(&app, child),
        None,
        "a detached (now-root) node is outside every suppressed subtree"
    );
}

// ---------------------------------------------------------------------------
// Steady-state quietness — the damage-gate contract, asserted headlessly.
//
// Extract's damage gate hears the marker through exactly two signals:
// `Changed<ComputedPaintSkip>` (probe) and `RemovedComponents<ComputedPaintSkip>`
// (stream), both reading MAIN-world ticks/events. A main-world probe over the
// same signals is therefore a faithful headless proxy: if it stays at zero on
// unchanged frames, the render-world gate stays quiet and the retention path
// (render_prepare.rs damage tests) keeps the prior buffers. A pass that
// re-inserted markers every frame (no reconcile) or re-walked into spurious
// ops would fail this.

/// Per-frame (changed, removed) counts for `ComputedPaintSkip`, pushed by the
/// probe system below.
#[derive(Resource, Default)]
struct MarkerOps(Vec<(usize, usize)>);

fn probe_marker_ops(
    mut ops: ResMut<MarkerOps>,
    changed: Query<(), Changed<ComputedPaintSkip>>,
    mut removed: RemovedComponents<ComputedPaintSkip>,
) {
    ops.0.push((changed.iter().count(), removed.read().count()));
}

#[test]
fn steady_state_frames_issue_no_marker_ops() {
    let mut app = app();
    app.init_resource::<MarkerOps>();
    // After Picking: write_paint_skip's commands are applied by then (the
    // auto-inserted sync on its .before(Picking) edge), so the probe sees the
    // same frame's marker flips — the same timing extract has.
    app.add_systems(Update, probe_marker_ops.after(buiy_core::BuiySet::Picking));

    let parent = app
        .world_mut()
        .spawn((node_bundle(), CssVisibility::Hidden))
        .id();
    let child = app.world_mut().spawn(node_bundle()).id();
    app.world_mut().entity_mut(parent).add_child(child);

    app.update(); // frame 0: markers inserted
    for _ in 0..3 {
        app.update(); // steady-state frames
    }

    let ops = &app.world().resource::<MarkerOps>().0;
    assert_eq!(ops.len(), 4);
    assert!(
        ops[0].0 >= 2,
        "frame 0 inserts the markers (parent + child), got {:?}",
        ops[0]
    );
    for (frame, &(changed, removed)) in ops.iter().enumerate().skip(1) {
        assert_eq!(
            (changed, removed),
            (0, 0),
            "steady-state frame {frame} must issue NO marker ops (extract's \
             damage gate would re-fire otherwise), got ({changed}, {removed})"
        );
    }
}

// ---------------------------------------------------------------------------
// GPU (#[ignore]) — needs a wgpu adapter.
// Run with: `cargo test -p buiy_core --test render_paint_skip -- --ignored --test-threads=1`.

/// End-to-end smoke on the real adapter: a hidden parent's subtree packs ZERO
/// quads (extract consumed the marker for the child too — the § 5.4 gap), the
/// show flip makes BOTH reappear (the `RemovedComponents<ComputedPaintSkip>`
/// damage path — missing it leaves the subtree permanently vanished), and the
/// re-hide drops them again (the `Changed` damage path). Buffer counts
/// suffice: the extract→prepare→draw spine is already pixel-verified by the
/// readback harness.
#[test]
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); subtree paint-skip smoke"]
fn hidden_subtree_packs_no_quads_and_reappears_on_show() {
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;
    use buiy_core::render::prepare::BuiyInstanceBuffers;
    use std::borrow::Cow;

    let opaque = || Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
    };
    let quad_count = |app: &App| {
        crate::support::render_world_resource::<BuiyInstanceBuffers>(app)
            .map(|b| b.quad_count)
            .unwrap_or(u32::MAX)
    };

    let mut app = crate::support::gpu_test_app_with_layout();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(50.0).height_px(50.0),
            opaque(),
            CssVisibility::Hidden,
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(20.0).height_px(20.0),
            opaque(),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);

    crate::support::finish_and_run(&mut app, 3);
    assert_eq!(
        quad_count(&app),
        0,
        "a Hidden parent's WHOLE subtree must pack zero quads — a count of 1 \
         means the default-visible child still painted (the § 5.4 gap)"
    );

    // Show: both quads must reappear (the marker-REMOVAL damage path).
    *app.world_mut().get_mut::<CssVisibility>(parent).unwrap() = CssVisibility::Visible;
    app.update();
    app.update();
    assert_eq!(
        quad_count(&app),
        2,
        "the shown subtree must re-extract and pack both quads — 0 here means \
         the RemovedComponents<ComputedPaintSkip> stream is missing from the \
         extract damage gate (the permanently-vanished-subtree bug)"
    );

    // Re-hide: both drop again (the marker-ADD damage path).
    *app.world_mut().get_mut::<CssVisibility>(parent).unwrap() = CssVisibility::Hidden;
    app.update();
    app.update();
    assert_eq!(
        quad_count(&app),
        0,
        "the re-hidden subtree drops both quads"
    );
}
