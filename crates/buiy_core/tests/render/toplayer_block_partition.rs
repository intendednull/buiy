//! Headless RED→GREEN unit tests for the top-layer extract signal — Wave 0 of the
//! top-layer stacking composite (`docs/specs/2026-07-10-toplayer-stacking-composite-design.md`
//! § 3.1). Later waves partition each tier's instance blob at this signal; W0 is
//! only the signal itself.
//!
//! `ExtractedNode.top_layer` is INHERITED: a node is top-layer iff itself OR any
//! ancestor formed a top-layer stacking context (`Stacking.top_layer !=
//! TopLayer::None`), computed by a `ChildOf` ancestor CLIMB after
//! `assemble_context_tree` (mirroring the landed `nearest_group_entity`
//! effect-group climb). A plain CHILD of an overlay carries no
//! `Stacking.top_layer` of its own, so a per-node read would MISCLASSIFY it as
//! base and split the contiguous top-layer tail — the inheritance assertion here
//! is the guard against that (the spike hit it as a hard tripwire panic the
//! instant it tested a raster INSIDE an overlay).
//!
//! Adapterless (no wgpu adapter / `RenderApp`): the `MainWorld`-swap idiom the
//! sibling `render_extract_composite` node harness uses.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style, TopLayer};
use buiy_core::render::buckets::pack_view_partitioned;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;
use buiy_core::render::extract::{
    ExtractedEffectGroups, ExtractedNode, ExtractedNodesView, extract_buiy_nodes,
};

/// Adapterless extract harness: swap the live main world into a bare render
/// world's `MainWorld` slot, run an `ExtractSchedule` carrying the production
/// `extract_buiy_nodes`, swap back, and read the carrier. Mirrors the
/// `render_extract_composite` / focus-ring / border-shadow node harnesses.
struct NodeExtractHarness {
    app: App,
    render: World,
    schedule: Schedule,
}

impl NodeExtractHarness {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(buiy_core::CorePlugin)
            .add_plugins(buiy_core::layout::LayoutPlugin)
            .add_plugins(bevy::transform::TransformPlugin)
            // BuiyRenderPlugin's MAIN-world half (write_clip_rects, paint-skip,
            // effect groups, forced colors) registers headless — its render half
            // is guarded on a RenderApp that never exists here, so no adapter.
            .add_plugins(buiy_core::render::BuiyRenderPlugin);
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            PrimaryWindow,
        ));

        let mut render = World::new();
        render.init_resource::<ExtractedNodesView>();
        render.init_resource::<ExtractedEffectGroups>();
        render.init_resource::<MainWorld>();

        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems(extract_buiy_nodes);

        Self {
            app,
            render,
            schedule,
        }
    }

    fn update(&mut self) {
        self.app.update();
    }

    fn extract(&mut self) {
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
        self.schedule.run(&mut self.render);
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
    }

    fn node_for(&self, entity: Entity) -> Option<ExtractedNode> {
        self.render
            .resource::<ExtractedNodesView>()
            .0
            .nodes
            .iter()
            .find(|n| n.entity == entity)
            .cloned()
    }
}

/// Settle layout + transform across a few frames (the bounded spawn-settle the
/// sibling node harnesses use).
fn settle(h: &mut NodeExtractHarness) {
    for _ in 0..4 {
        h.update();
    }
}

fn surface() -> Background {
    Background {
        color: ColorToken::SurfacePrimary,
    }
}

/// An absolutely-positioned box `w×h` at `(x, y)` — a distinct laid-out leaf.
fn abs(x: f32, y: f32, w: f32, h: f32) -> Style {
    Style::default()
        .absolute()
        .inset(Inset {
            top: Sizing::Length(Length::px(y)),
            left: Sizing::Length(Length::px(x)),
            ..default()
        })
        .width_px(w)
        .height_px(h)
}

/// A `.top_layer()` parent with a PLAIN child (no own `Stacking.top_layer`) and a
/// disjoint base node, all under one root. Asserts the ancestor-climb
/// classification: the child INHERITS the parent's top-layer tag; the base does
/// not. The child assertion is FIRST because it is the RED witness — a per-node
/// `Stacking` read (or the un-implemented default) leaves it `false`.
#[test]
fn toplayer_child_inherits() {
    let mut h = NodeExtractHarness::new();

    // A plain in-flow child: it carries `Stacking::default()` (top_layer = None),
    // so ONLY the ancestor climb — not its own component — can tag it top-layer.
    let child = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(30.0).height_px(30.0),
            surface(),
        ))
        .id();
    // The overlay ROOT: it itself called `.top_layer(...)`.
    let parent = h
        .app
        .world_mut()
        .spawn((
            Node,
            abs(50.0, 50.0, 60.0, 60.0).top_layer(TopLayer::Popover),
            surface(),
        ))
        .id();
    h.app.world_mut().entity_mut(parent).add_children(&[child]);
    // A disjoint base node (no top-layer ancestor).
    let base = h
        .app
        .world_mut()
        .spawn((Node, abs(10.0, 10.0, 40.0, 40.0), surface()))
        .id();
    let root = h
        .app
        .world_mut()
        .spawn((Node, Style::default().width_px(200.0).height_px(150.0)))
        .id();
    h.app
        .world_mut()
        .entity_mut(root)
        .add_children(&[base, parent]);

    settle(&mut h);
    h.extract();

    // The inheritance witness FIRST: a plain child of a top-layer root must be
    // tagged via the ancestor climb, NOT its own (None) `Stacking.top_layer`.
    assert!(
        h.node_for(child).expect("child is extracted").top_layer,
        "a plain CHILD of a top-layer root inherits top_layer (ancestor climb, not per-node)"
    );
    // The top-layer root itself is tagged, and a base node is not — the brackets
    // that pin the classifier.
    assert!(
        h.node_for(parent).expect("parent is extracted").top_layer,
        "the top-layer root is tagged"
    );
    assert!(
        !h.node_for(base).expect("base is extracted").top_layer,
        "a base node is not top-layer"
    );
}

// === Wave 1: per-tier packer boundaries ======================================
//
// The extract signal above rides `ExtractedNode.top_layer`. Each tier packer
// walks its producer in paint order and records the instance index where the
// first top-layer-tagged instance begins = the tier's `top_layer_boundary`, plus
// a tail-contiguity `debug_assert` tripwire (spec § 3.4): once a top-layer node
// is seen no base node may follow, because top-layer content is a contiguous
// suffix of the paint order. These headless unit tests drive the packers
// directly off literal `ExtractedNode`s (the flag rides the record — no extract
// needed), which is far less brittle than round-tripping the extract harness.

/// A minimal opaque fill node with an explicit `top_layer` flag. Reused by the
/// quad/shadow/band boundary tests: each packer reads `ExtractedNode.top_layer`
/// directly, so a literal fixture is enough to exercise the boundary + tripwire.
fn fill(entity: u32, top_layer: bool) -> ExtractedNode {
    ExtractedNode {
        entity: Entity::from_raw_u32(entity).unwrap(),
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
        radius: 0.0,
        color: Color::WHITE,
        clip: None,
        group: None,
        top_layer,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    }
}

// --- Task 1.1: quad packer `PackedPartition.top_layer_boundary` ---------------

#[test]
fn quad_boundary_at_first_top_layer_instance() {
    // nodes = [base, base, TOP, TOP] — the boundary is the instance index of the
    // first top-layer node's quad (2), so [0..2) is the base block and [2..4) the
    // top-layer block.
    let nodes = [fill(1, false), fill(2, false), fill(3, true), fill(4, true)];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.instances.len(), 4);
    assert_eq!(
        p.top_layer_boundary, 2,
        "boundary at the first top-layer instance"
    );
}

#[test]
fn quad_boundary_is_count_when_no_top_layer() {
    // No top-layer node ⇒ boundary == the instance count (the whole blob is the
    // base block; the empty top-layer block is [count..count)). Byte-stable path.
    let nodes = [fill(1, false), fill(2, false), fill(3, false)];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.top_layer_boundary, 3);
}

#[test]
fn quad_boundary_all_top_layer_is_zero() {
    // Every node top-layer ⇒ boundary 0 (the base block is empty).
    let nodes = [fill(1, true), fill(2, true)];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.top_layer_boundary, 0);
}

#[test]
#[should_panic(expected = "contiguous tail")]
fn quad_base_after_top_layer_trips_the_tripwire() {
    // [base, TOP, base] — a base node AFTER a top-layer node violates
    // tail-contiguity; the packer's `debug_assert` fires (the § 3.1-class bug the
    // spike caught as a hard panic, not a silent wrong pixel).
    let nodes = [fill(1, false), fill(2, true), fill(3, false)];
    let _ = pack_view_partitioned(&nodes, 0, &[]);
}

// --- Task 1.2: shadow + rounded-shadow packer boundaries ---------------------

use buiy_core::render::buckets::{pack_rounded_shadow_instances, pack_shadow_instances};
use buiy_core::render::extract::ExtractedShadow;

/// One shadow term; `rounded` routes it to the ROUNDED pipeline (radius > 0),
/// else the SQUARE pipeline (radius 0) — the two packers partition a node's terms
/// by radius, so a fixture drives exactly one of them.
fn shadow_term(rounded: bool) -> ExtractedShadow {
    ExtractedShadow {
        rect_pos: Vec2::ZERO,
        rect_size: Vec2::splat(10.0),
        color: [0.0, 0.0, 0.0, 0.5],
        sigma: 2.0,
        clip: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        radius: if rounded { 4.0 } else { 0.0 },
    }
}

/// A fill node carrying one shadow term (square or rounded).
fn fill_with_shadow(entity: u32, top_layer: bool, rounded: bool) -> ExtractedNode {
    let mut n = fill(entity, top_layer);
    n.shadows = vec![shadow_term(rounded)];
    n
}

#[test]
fn shadow_boundary_at_first_top_layer_caster() {
    // [base square shadow, TOP square shadow] — the square-shadow boundary is the
    // instance index of the top-layer caster's first shadow term (1).
    let nodes = [
        fill_with_shadow(1, false, false),
        fill_with_shadow(2, true, false),
    ];
    let (shadows, boundary) = pack_shadow_instances(&nodes);
    assert_eq!(shadows.len(), 2, "one square shadow per caster");
    assert_eq!(boundary, 1, "boundary at the top-layer caster's shadow");
}

#[test]
fn rounded_shadow_boundary_at_first_top_layer_caster() {
    // [base rounded shadow, TOP rounded shadow] — the rounded caster's shadow
    // lands in the top-layer range; the boundary is 1. (Square terms of these
    // casters are empty — the two blobs partition by radius.)
    let nodes = [
        fill_with_shadow(1, false, true),
        fill_with_shadow(2, true, true),
    ];
    let (square, sq_boundary) = pack_shadow_instances(&nodes);
    let (rounded, rn_boundary) = pack_rounded_shadow_instances(&nodes);
    assert!(square.is_empty(), "rounded casters emit no square shadow");
    assert_eq!(rounded.len(), 2, "one rounded shadow per caster");
    // The square blob is empty ⇒ its boundary is the count (0). The rounded
    // boundary is the top-layer caster's rounded-shadow index (1).
    assert_eq!(sq_boundary, 0);
    assert_eq!(rn_boundary, 1, "boundary at the top-layer rounded caster");
}

#[test]
fn shadow_boundary_is_count_when_no_top_layer() {
    // No top-layer caster ⇒ the boundary is the shadow count (empty top block).
    let nodes = [
        fill_with_shadow(1, false, false),
        fill_with_shadow(2, false, false),
    ];
    let (shadows, boundary) = pack_shadow_instances(&nodes);
    assert_eq!(boundary, shadows.len() as u32);
    assert_eq!(boundary, 2);
}
