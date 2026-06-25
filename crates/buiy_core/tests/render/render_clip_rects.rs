//! Render-prep clip pass (`write_clip_rects`) — clip AABB geometry.
//!
//! HEADLESS: a plain `Update` ECS system, no wgpu adapter. Build the app
//! with MinimalPlugins + CorePlugin + LayoutPlugin + BuiyRenderPlugin (its
//! `build` is a no-op without a RenderApp — see render_smoke.rs).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.

use bevy::prelude::*;
use buiy_core::render::components::AncestorClip;
use buiy_core::{
    ClipRect, ContainFlags, Containment, ContentVisibility, CorePlugin, Display, Node,
    OverflowMode, ResolvedLayout, ScrollOffset,
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

#[test]
fn unclipped_node_emits_no_clip_rect() {
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0)))
        .id();
    app.update();
    assert!(
        app.world().get::<ClipRect>(e).is_none(),
        "a node with no clipping ancestor must have NO ClipRect"
    );
    assert!(
        app.world().get::<AncestorClip>(e).is_none(),
        "a node with no clipping ancestor must have NO AncestorClip"
    );
}

#[test]
fn child_of_overflow_hidden_is_clipped_to_parent_padding_box() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .border(10.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(200.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    // Buiy uses content-box sizing (CSS default): width_px(100) is the CONTENT
    // width, so the border box is 100 + 2*10 = 120, and the padding box (border
    // box inset by border) is (10,10)-(110,110).
    let clip = *app.world().get::<ClipRect>(child).expect("child clipped");
    assert_eq!(
        clip.max,
        Vec2::new(110.0, 110.0),
        "clamped to parent padding max"
    );
    assert!(
        clip.min.x >= 10.0 && clip.min.y >= 10.0,
        "clamped to parent padding min"
    );

    let anc = *app
        .world()
        .get::<AncestorClip>(child)
        .expect("child has ancestor clip");
    assert_eq!(anc.min, Vec2::new(10.0, 10.0), "ancestor clip padding min");
    assert_eq!(
        anc.max,
        Vec2::new(110.0, 110.0),
        "ancestor clip padding max"
    );

    assert!(
        app.world().get::<ClipRect>(parent).is_none(),
        "parent unclipped"
    );
}

#[test]
fn contain_paint_ancestor_clips_to_border_box() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .border(10.0)
                .contain(ContainFlags::PAINT),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    // contain:paint clips to the BORDER box (own box, no inset). With
    // content-box sizing the border box is 100 + 2*10 = 120 — and crucially
    // NOT the 110 padding box, distinguishing it from overflow:hidden.
    let anc = *app
        .world()
        .get::<AncestorClip>(child)
        .expect("paint-contained");
    assert_eq!(
        anc.min,
        Vec2::ZERO,
        "paint clip = border box min (NOT padding)"
    );
    assert_eq!(
        anc.max,
        Vec2::new(120.0, 120.0),
        "paint clip = border box max"
    );
}

#[test]
fn nested_overflow_hidden_intersects_to_tighter_box() {
    let mut app = app();
    let outer = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(200.0)
                .height_px(200.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let inner = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(50.0)
                .height_px(50.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let grandchild = app
        .world_mut()
        .spawn((Node, Style::default().width_px(500.0).height_px(500.0)))
        .id();
    app.world_mut().entity_mut(outer).add_child(inner);
    app.world_mut().entity_mut(inner).add_child(grandchild);
    app.update();

    let anc = *app
        .world()
        .get::<AncestorClip>(grandchild)
        .expect("clipped");
    assert_eq!(
        anc.max,
        Vec2::new(50.0, 50.0),
        "intersection = tighter inner box"
    );
}

#[test]
fn per_axis_overflow_leaves_visible_axis_unbounded() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Visible),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    let anc = *app.world().get::<AncestorClip>(child).expect("x-clipped");
    assert_eq!(anc.max.x, 100.0, "x axis clamped to parent padding");
    assert_eq!(anc.max.y, f32::INFINITY, "y axis (visible) unbounded");
    assert_eq!(anc.min.y, f32::NEG_INFINITY, "y axis (visible) unbounded");
}

#[test]
fn scroll_container_clips_both_axes_even_with_visible_sibling() {
    let mut app = app();
    // overflow-x: scroll, overflow-y: visible — the node is still a scroll
    // container (either-axis rule), so CSS computes the visible axis to `auto`:
    // the child-facing clip is the full 2D padding-box viewport, NOT unbounded
    // on y. Distinguishes a scroll container from a plain per-axis clipper.
    let sc = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Scroll, OverflowMode::Visible),
            ScrollOffset::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(sc).add_child(child);
    app.update();

    let anc = *app.world().get::<AncestorClip>(child).expect("clipped");
    assert_eq!(
        anc.max,
        Vec2::new(100.0, 100.0),
        "both axes clipped to the scroll viewport"
    );
    assert_ne!(
        anc.max.y,
        f32::INFINITY,
        "y is NOT unbounded despite overflow-y:visible (scroll container → auto)"
    );
}

#[test]
fn clip_rect_is_ancestor_clip_intersected_with_own_box() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(200.0)
                .height_px(200.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(40.0).height_px(40.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.update();

    let clip = *app.world().get::<ClipRect>(child).expect("ClipRect");
    let anc = *app
        .world()
        .get::<AncestorClip>(child)
        .expect("AncestorClip");

    assert_eq!(anc.min, Vec2::ZERO);
    assert_eq!(anc.max, Vec2::new(200.0, 200.0));
    assert_eq!(
        clip.max,
        Vec2::new(40.0, 40.0),
        "ClipRect bounded by own box"
    );
    assert_ne!(
        clip.max, anc.max,
        "ClipRect != AncestorClip when own box tighter"
    );
    assert_eq!(clip.min, anc.min.max(Vec2::ZERO));
    assert_eq!(clip.max, anc.max.min(Vec2::new(40.0, 40.0)));
}

#[test]
fn content_visibility_hidden_subtree_is_not_clipped() {
    let mut app = app();
    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let hidden = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(50.0)
                .height_px(50.0)
                .containment(Containment {
                    content_visibility: ContentVisibility::Hidden,
                    ..Default::default()
                }),
        ))
        .id();
    let descendant = app
        .world_mut()
        .spawn((Node, Style::default().width_px(20.0).height_px(20.0)))
        .id();
    app.world_mut().entity_mut(root).add_child(hidden);
    app.world_mut().entity_mut(hidden).add_child(descendant);
    app.update();

    assert!(
        app.world().get::<ClipRect>(descendant).is_none(),
        "descendant of content-visibility:hidden is pruned"
    );
    assert!(
        app.world().get::<AncestorClip>(descendant).is_none(),
        "descendant of content-visibility:hidden is pruned"
    );
}

#[test]
fn scroll_container_clips_to_viewport_independent_of_offset() {
    let mut app = app();
    let sc = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Scroll, OverflowMode::Scroll),
            ScrollOffset::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(sc).add_child(child);
    app.update();

    let clip_before = *app
        .world()
        .get::<ClipRect>(child)
        .expect("child clipped by scroll viewport");
    assert_eq!(
        clip_before.max,
        Vec2::new(100.0, 100.0),
        "clamped to viewport"
    );

    {
        let mut off = app.world_mut().get_mut::<ScrollOffset>(sc).unwrap();
        off.y = 80.0;
    }
    app.update();
    let clip_after = *app.world().get::<ClipRect>(child).expect("still clipped");
    assert_eq!(
        clip_before, clip_after,
        "scroll-container clip box is offset-independent (A.4)"
    );
}

#[test]
fn scroll_only_frame_does_not_relayout() {
    let mut app = app();
    let sc = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Scroll, OverflowMode::Scroll),
            ScrollOffset::default(),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(sc).add_child(child);

    app.update();
    app.update();

    let rl_before = app.world().get::<ResolvedLayout>(child).unwrap().position;
    let size_before = app.world().get::<ResolvedLayout>(child).unwrap().size;

    {
        let mut off = app.world_mut().get_mut::<ScrollOffset>(sc).unwrap();
        off.y = 120.0;
    }
    app.update();

    let rl_after = app.world().get::<ResolvedLayout>(child).unwrap().position;
    let size_after = app.world().get::<ResolvedLayout>(child).unwrap().size;

    // The decisive proof for R2: scroll moves content, not geometry. The
    // child's resolved layout is byte-stable AND — stronger than value
    // equality — its change tick did not advance, so layout did not even
    // rewrite ResolvedLayout (a relayout recomputing the same value would
    // fail this). (The sync_styles-trigger proof of "scroll never re-runs
    // Taffy" is owned by tests/layout_scroll_offset_no_invalidate.rs.)
    assert_eq!(
        rl_before, rl_after,
        "ResolvedLayout.position stable across scroll"
    );
    assert_eq!(
        size_before, size_after,
        "ResolvedLayout.size stable across scroll"
    );
    let mut q = app.world_mut().query::<Ref<ResolvedLayout>>();
    let r = q.get(app.world(), child).expect("child has ResolvedLayout");
    assert!(
        !r.is_changed(),
        "scroll-only frame must not rewrite ResolvedLayout (no relayout)"
    );
}

#[test]
fn node_parented_under_non_node_is_a_clip_root() {
    let mut app = app();
    // A plain (non-Node) Bevy entity as parent — a supported topology that
    // layout treats as a clip root. The clip walk must reach this subtree via
    // the two-disjunct root (no ChildOf OR parent-not-a-Node), not only via
    // detached Nodes.
    let non_node = app.world_mut().spawn(()).id();
    let clipper = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(non_node).add_child(clipper);
    app.world_mut().entity_mut(clipper).add_child(child);
    app.update();

    let clip = app.world().get::<ClipRect>(child);
    assert!(
        clip.is_some(),
        "a Node rooted under a non-Node parent must still be clip-walked"
    );
    assert_eq!(
        clip.unwrap().max,
        Vec2::new(100.0, 100.0),
        "child clamped to the clipper viewport"
    );
}

#[test]
fn steady_state_frame_does_not_rewrite_clip_rect() {
    // A re-insert is detected by a `Changed<ClipRect>` system run inside the
    // schedule (a query created *after* the updates cannot tell a gated insert
    // from a re-insert-every-frame impl). With the change-gate, frame 2 must
    // see no `ClipRect` change; without it this test fails (re-insert marks
    // the component Changed every frame).
    #[derive(Resource, Default)]
    struct SawClipChange(bool);
    fn detect(q: Query<(), Changed<ClipRect>>, mut saw: ResMut<SawClipChange>) {
        if !q.is_empty() {
            saw.0 = true;
        }
    }

    let mut app = app();
    app.init_resource::<SawClipChange>();
    app.add_systems(Update, detect.after(buiy_core::render::write_clip_rects));

    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(40.0).height_px(40.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);

    app.update(); // frame 1: ClipRect inserted; `detect` sees the change.
    assert!(
        app.world().get::<ClipRect>(child).is_some(),
        "frame 1 wrote the clip"
    );
    app.world_mut().resource_mut::<SawClipChange>().0 = false;
    app.update(); // frame 2: steady — `detect` must NOT see a change.
    assert!(
        !app.world().resource::<SawClipChange>().0,
        "steady-state frame must not re-insert ClipRect (change-gate)"
    );
}

#[test]
fn pruned_node_drops_its_stale_clip() {
    let mut app = app();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(40.0).height_px(40.0)))
        .id();
    let grandchild = app
        .world_mut()
        .spawn((Node, Style::default().width_px(20.0).height_px(20.0)))
        .id();
    app.world_mut().entity_mut(parent).add_child(child);
    app.world_mut().entity_mut(child).add_child(grandchild);

    app.update();
    assert!(
        app.world().get::<ClipRect>(child).is_some(),
        "child clipped on frame 1"
    );
    assert!(
        app.world().get::<ClipRect>(grandchild).is_some(),
        "grandchild clipped on frame 1"
    );

    // Flip the child to Display::None — it AND its grandchild must drop their
    // stale clips (the prune path clears the whole subtree, spec § A.3).
    app.world_mut().entity_mut(child).insert(Display::None);
    app.update();

    assert!(
        app.world().get::<ClipRect>(child).is_none(),
        "pruned child drops ClipRect"
    );
    assert!(
        app.world().get::<AncestorClip>(child).is_none(),
        "pruned child drops AncestorClip"
    );
    assert!(
        app.world().get::<ClipRect>(grandchild).is_none(),
        "descendant of pruned drops ClipRect"
    );
    assert!(
        app.world().get::<AncestorClip>(grandchild).is_none(),
        "descendant of pruned drops AncestorClip"
    );
}

#[test]
fn offset_clipper_clips_in_absolute_space() {
    // The C1-specific gate (audit §1 MISSED #1): no existing clip test offsets
    // a clipper. Outer is translated (70,90); the clipper is its child at
    // parent-local (0,0), so its ABSOLUTE box is (70,90)..(170,190). A child
    // overflowing it must be clipped to that ABSOLUTE box. Pre-C1 (own box from
    // ResolvedLayout.position) the clip is wrongly anchored at the origin
    // (0,0)..(100,100); post-C1 it is the absolute (70,90)..(170,190).
    let mut app = app();
    let outer = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(400.0)
                .height_px(400.0)
                .translate_px(70.0, 90.0),
        ))
        .id();
    let clipper = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(outer).add_child(clipper);
    app.world_mut().entity_mut(clipper).add_child(child);
    app.update();
    app.update();

    let anc = *app
        .world()
        .get::<AncestorClip>(child)
        .expect("child clipped by the offset clipper");
    assert_eq!(
        anc.min,
        Vec2::new(70.0, 90.0),
        "ancestor clip min = clipper ABSOLUTE top-left, not the origin"
    );
    assert_eq!(
        anc.max,
        Vec2::new(170.0, 190.0),
        "ancestor clip max = clipper ABSOLUTE bottom-right"
    );
    let clip = *app
        .world()
        .get::<ClipRect>(child)
        .expect("child has clip rect");
    assert_eq!(clip.min, Vec2::new(70.0, 90.0), "ClipRect min = absolute");
    assert_eq!(clip.max, Vec2::new(170.0, 190.0), "ClipRect max = absolute");
}

#[test]
fn clip_reflects_post_bridge_transform_same_frame() {
    // A scroll container scrolls its content; a CLIPPER nested inside the
    // scrolled content moves in absolute space by the scroll delta. Clip must
    // read the POST-bridge GlobalTransform, so the deeper child's clip tracks
    // the clipper's new absolute position within the same settled frame —
    // proving write_clip_rects runs .after(sync_simple_transforms) (D3).
    let mut app = app();
    let scroller = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(400.0)
                .height_px(400.0)
                .overflow(OverflowMode::Scroll, OverflowMode::Scroll),
            ScrollOffset::default(),
        ))
        .id();
    let clipper = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(scroller).add_child(clipper);
    app.world_mut().entity_mut(clipper).add_child(child);
    app.update();
    app.update();

    // Before scroll: clipper absolute box = (0,0)..(100,100); the child's
    // AncestorClip is that box ∩ the scroller's own viewport (0,0)..(400,400) =
    // (0,0)..(100,100).
    let before = *app.world().get::<AncestorClip>(child).expect("clipped");
    assert_eq!(before.min, Vec2::ZERO);
    assert_eq!(before.max, Vec2::new(100.0, 100.0));

    // Scroll the OUTER container by (0,+50): content (incl. clipper) shifts up
    // by 50px in absolute space => clipper absolute box = (0,-50)..(100,50). The
    // child's AncestorClip is that box ∩ the scroller viewport (0,0)..(400,400):
    // the scroller's own clip clamps min.y back to 0, but max.y drops to 50 — and
    // THAT drop is the freshness witness. A clip that read a STALE (pre-bridge)
    // transform would keep max.y = 100; reading the POST-bridge transform yields
    // max.y = 50, proving write_clip_rects runs .after(sync_simple_transforms).
    {
        let mut off = app.world_mut().get_mut::<ScrollOffset>(scroller).unwrap();
        off.y = 50.0; // positive scroll offset moves content up (acc add, bridge.rs:184)
    }
    app.update();

    let after = *app
        .world()
        .get::<AncestorClip>(child)
        .expect("still clipped");
    assert_eq!(
        after.max,
        Vec2::new(100.0, 50.0),
        "clip max tracks the clipper's POST-bridge absolute position (not stale): \
         the scrolled clipper's bottom edge moved from y=100 to y=50"
    );
    // min.x/min.y are clamped to the scroller's own viewport origin (0,0); the
    // freshness discriminator is max.y above.
    assert_eq!(
        after.min,
        Vec2::ZERO,
        "clamped to the scroller viewport origin"
    );
}

#[test]
fn clipper_without_global_transform_contributes_no_clip() {
    // A clipper Node with a resolved box but NO GlobalTransform contributes no
    // clip (the `_ =>` arm clears, does not fall back to rl.position — D2). Its
    // child therefore has NO AncestorClip.
    //
    // To produce a genuinely un-bridged clipper we settle the tree (so layout +
    // bridge run), then strip BOTH Transform and GlobalTransform and run one
    // more *no-change* frame: with nothing dirty the bridge walk descends no
    // subtree and re-adds neither component (verified: the bridge only writes
    // seeded subtrees), so the clip pass sees `GlobalTransform: None` and must
    // drop the clip rather than silently using ResolvedLayout.position.
    let mut app = app();
    let clipper = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .width_px(100.0)
                .height_px(100.0)
                .overflow(OverflowMode::Hidden, OverflowMode::Hidden),
        ))
        .id();
    let child = app
        .world_mut()
        .spawn((Node, Style::default().width_px(300.0).height_px(300.0)))
        .id();
    app.world_mut().entity_mut(clipper).add_child(child);
    // Settle: layout + bridge run; the clipper IS bridged and clips its child.
    for _ in 0..4 {
        app.update();
    }
    assert!(
        app.world().get::<AncestorClip>(child).is_some(),
        "the bridged clipper clips its child while it has a GlobalTransform"
    );

    // Strip both transform components so the clipper is genuinely un-bridged,
    // then run one no-change frame: nothing is dirty, so the bridge walk does
    // not re-add them.
    app.world_mut()
        .entity_mut(clipper)
        .remove::<GlobalTransform>();
    app.world_mut().entity_mut(clipper).remove::<Transform>();
    assert!(
        app.world().get::<GlobalTransform>(clipper).is_none(),
        "the clipper has no GlobalTransform before the clip pass"
    );
    app.update();
    assert!(
        app.world().get::<GlobalTransform>(clipper).is_none(),
        "the no-change frame did not re-bridge the clipper"
    );
    assert!(
        app.world().get::<AncestorClip>(child).is_none(),
        "an un-bridged clipper contributes no clip (no fallback to ResolvedLayout.position)"
    );
}
