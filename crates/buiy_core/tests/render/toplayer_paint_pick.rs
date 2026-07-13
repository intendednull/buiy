//! Headless paint==pick acceptance for the top-layer stacking composite (Wave 4
//! Task 4.5 of `docs/plans/2026-07-10-toplayer-stacking-composite.md`; spec §4).
//! This closes the pick≠paint seam that motivated the whole refactor
//! (`docs/plans/follow-ups.md:2311`): a `.top_layer()` overlay that WINS the pick
//! at a point must also be the entity that PAINTS last there — across a tier that
//! previously bled (the border BAND tier).
//!
//! Two INDEPENDENT derivations are asserted to agree:
//!   * PICK — the free `hit_test` (== `emit_picks`) ranks by `global_paint_order`,
//!     the `painters_z`-tail flatten. It already put the overlay topmost BEFORE
//!     this refactor.
//!   * PAINT — the render draw order now follows the `ExtractedNode.top_layer`
//!     block partition (W0–W2): a top-layer subtree's whole tier-stack draws in
//!     the TOP block, AFTER every base tier (incl. the base band). This is the
//!     NEW mechanism; before it, a base band drew in a later GLOBAL band tier over
//!     the overlay quad (the bleed).
//!
//! The witness: the entity `hit_test` returns as topmost (the overlay) is exactly
//! the entity tagged `top_layer` (drawn last), while the base band it occludes is
//! tagged base — so pick-winner == paint-winner. `top_layer` derives from
//! `Stacking.top_layer` (an ancestor climb) and `global_paint_order` from
//! `painters_z`; asserting BOTH proves the two mechanisms cannot disagree.
//!
//! Adapterless (no wgpu adapter / `RenderApp`): the same `MainWorld`-swap extract
//! idiom `toplayer_block_partition.rs` uses. `hit_test` is a free function that
//! runs NO systems, so it needs only its query components registered — not buiy's
//! `PickingPlugin` (whose `PointerInput` event pipeline would panic under
//! `MinimalPlugins`).

use bevy::ecs::query::QueryState;
use bevy::picking::Pickable;
use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::Node;
use buiy_core::components::StackingContext;
use buiy_core::layout::{Inset, Length, Sizing, Style, TopLayer};
use buiy_core::picking::{global_paint_order, hit_test};
use buiy_core::render::RenderWorkCounters;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, Border, BorderSide, ClipRect, Corners, LineStyle};
use buiy_core::render::extract::{
    ExtractedEffectGroups, ExtractedNode, ExtractedNodesView, NodeDamage, RetainedNodeIndex,
    extract_buiy_nodes,
};

/// Adapterless extract + picking harness: the `toplayer_block_partition.rs`
/// `MainWorld`-swap harness, plus explicit registration of the components the
/// free `hit_test` / `global_paint_order` `QueryState::try_new` over
/// (`StackingContext`/`ClipRect`/`Pickable`) so they resolve on this bare world.
struct PaintPickHarness {
    app: App,
    render: World,
    schedule: Schedule,
}

impl PaintPickHarness {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(buiy_core::CorePlugin)
            .add_plugins(buiy_core::layout::LayoutPlugin)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(buiy_core::render::BuiyRenderPlugin);
        // Register the components the free `hit_test` / `global_paint_order`
        // `QueryState::try_new` over (`StackingContext`/`ClipRect`/`Pickable`), so
        // it resolves on this bare world. `hit_test` runs NO systems, so buiy's
        // `PickingPlugin` (which adds the `PointerInput`-reading event pipeline) is
        // NOT needed — and its systems would panic under MinimalPlugins (no
        // `Messages<PointerInput>` owner). `register_component` is idempotent.
        let world = app.world_mut();
        world.register_component::<StackingContext>();
        world.register_component::<ClipRect>();
        world.register_component::<Pickable>();
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
        render.init_resource::<RetainedNodeIndex>();
        render.init_resource::<NodeDamage>();
        render.init_resource::<RenderWorkCounters>();

        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems(extract_buiy_nodes);

        Self {
            app,
            render,
            schedule,
        }
    }

    fn settle(&mut self) {
        for _ in 0..4 {
            self.app.update();
        }
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

    /// The topmost pick at `point` (the free `hit_test`, == the backend's ranking).
    fn pick(&self, point: Vec2) -> Option<Entity> {
        hit_test(self.app.world(), point)
    }

    /// The global front-to-back paint index of `entity` (0 = bottom-most). `None`
    /// if the entity is absent from `global_paint_order` (does not paint).
    fn paint_index(&self, entity: Entity) -> Option<usize> {
        let world = self.app.world();
        let mut contexts =
            QueryState::<(Entity, &StackingContext)>::try_new(world).expect("contexts registered");
        let order = global_paint_order(&contexts.query(world));
        order.iter().position(|e| *e == entity)
    }
}

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

fn yellow_border() -> Border {
    let side = || BorderSide {
        color: ColorToken::Custom(Color::srgb_u8(240, 220, 20)),
        style: LineStyle::Solid,
    };
    Border {
        top: side(),
        right: side(),
        bottom: side(),
        left: side(),
        radius: Corners::ZERO,
    }
}

/// A base bordered box (BAND tier — the tier that previously bled) under a
/// full-viewport `.top_layer()` scrim. The pick at a point over the base band
/// returns the OVERLAY, and the overlay is the top-block (last-painted) entity —
/// so pick-winner == paint-winner.
#[test]
fn top_layer_overlay_is_both_pick_winner_and_paint_winner_over_a_base_band() {
    let mut h = PaintPickHarness::new();

    // The base bordered box: an 8px yellow border BAND at (10,10) 40×40 → left
    // band x[10,18]. No fill; the border is the base ink the overlay must occlude.
    let base_box = h
        .app
        .world_mut()
        .spawn((
            Node,
            Name::new("base_bordered_box"),
            abs(10.0, 10.0, 40.0, 40.0).border(8.0),
            yellow_border(),
        ))
        .id();
    // The top-layer scrim: full viewport, a translucent dark fill, a LATER sibling.
    let overlay = h
        .app
        .world_mut()
        .spawn((
            Node,
            Name::new("scrim_overlay"),
            abs(0.0, 0.0, 200.0, 150.0).top_layer(TopLayer::Popover),
            Background {
                color: ColorToken::Custom(Color::srgba_u8(0x14, 0x16, 0x1b, 0x9c)),
            },
        ))
        .id();
    let root = h
        .app
        .world_mut()
        .spawn((
            Node,
            Name::new("root"),
            Style::default().width_px(200.0).height_px(150.0),
        ))
        .id();
    h.app
        .world_mut()
        .entity_mut(root)
        .add_children(&[base_box, overlay]);

    h.settle();
    h.extract();

    // A point over the base box's LEFT border band (x[10,18]) — a BAND-tier pixel
    // that, pre-refactor, bled OVER the overlay quad.
    let band_point = Vec2::new(13.0, 30.0);

    // PICK: the topmost hit at the band point is the OVERLAY (it covers the point
    // and sits at the painters_z tail — the pick the user's click resolves to).
    assert_eq!(
        h.pick(band_point),
        Some(overlay),
        "PICK: the top-layer overlay wins the hit at a point over the base band"
    );

    // PAINT (block): the overlay is tagged `top_layer` (its whole tier-stack draws
    // in the TOP block, AFTER every base tier incl. the base band); the base box is
    // base. This is the NEW mechanism (W0–W2) — the paint order now honors it.
    assert!(
        h.node_for(overlay).expect("overlay extracted").top_layer,
        "PAINT: the pick winner (overlay) is a top-block entity — it paints last"
    );
    assert!(
        !h.node_for(base_box).expect("base box extracted").top_layer,
        "PAINT: the occluded base band is a base-block entity — it paints first"
    );

    // PAINT (order): the independent `painters_z` derivation `hit_test` uses AGREES
    // — the overlay's global paint index is strictly greater than the base band's,
    // so the overlay paints AFTER (over) it. Two mechanisms, one verdict.
    let overlay_z = h.paint_index(overlay).expect("overlay paints");
    let base_z = h.paint_index(base_box).expect("base box paints");
    assert!(
        overlay_z > base_z,
        "PAINT: the pick winner also paints last — overlay paint-index {overlay_z} > base band {base_z}"
    );
}
