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
