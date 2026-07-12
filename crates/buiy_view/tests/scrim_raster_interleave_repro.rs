//! STAGE-1 (CPU tier — NO fix): the raster-interleave regression for the modal
//! scrim — a RASTER (drawing-canvas) node in the "game" body, painting in paint
//! order BEFORE the scrim.
//!
//! The real in-game screen (`apps/dooduel/src/view/in_game.rs`) places the
//! drawing canvas as a `buiy_view::raster(handle, w, h)` node inside the opaque
//! game body, then paints the `.fixed().top_layer()` scrim
//! (`apps/dooduel/src/view/widgets.rs:189`) OVER everything. The scrim's
//! translucent full-viewport quad must draw AFTER the raster (so it paints over
//! the canvas), which this test proves at the CPU tier.
//!
//! It runs the REAL downstream packing + interleave (`pack_view_partitioned` +
//! `interleave_flat_draw`,
//! `crates/buiy_core/src/render/buckets.rs`) — the CPU stage `node.rs` executes
//! against the open render pass — to prove (or disprove) that the scrim's quad
//! is drawn AFTER the raster splice, i.e. the scrim paints over the canvas.
//!
//! The raster's paint-order splice anchor is the SAME value `node.rs` uses:
//! `PackedPartition::node_quad_anchor_of[raster_entity]`
//! (`crates/buiy_core/src/render/raster.rs:508` joins the extracted raster to
//! this map; `build_raster_draws` feeds it to `interleave_flat_draw`).

use bevy::image::Image;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::layout::TopLayer;
use buiy_core::mvu::{Cmd, Model};
use buiy_core::render::RasterImage;
use buiy_core::render::buckets::{FlatDrawStep, interleave_flat_draw, pack_view_partitioned};
use buiy_core::render::extract::{ExtractedNode, ExtractedNodesView, extract_buiy_nodes};
use buiy_core::{Background, Stacking};
use buiy_view::{BuiyViewAppExt, Color, Element, raster};

/// The model carries the canvas `Handle<Image>` so the plain-fn `view` (no
/// capture) can build the raster node from it — the same shape dooduel uses
/// (the canvas handle lives on the app state, read by `in_game`).
#[derive(Component, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct RasterProbe {
    canvas: Handle<Image>,
}

impl Model for RasterProbe {
    type Msg = Noop;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
struct Noop;

fn update(_: &mut RasterProbe, _: Noop) -> Cmd<Noop> {
    Cmd::none()
}

/// Opaque dark-green "game" base — fills the viewport, sits BELOW the raster + scrim.
const GAME_BG: Color = Color::Custom(0x00, 0x64, 0x00, 255);
/// Opaque white modal card — the scrim's child, sits ABOVE the scrim.
const CARD_BG: Color = Color::Custom(0xff, 0xff, 0xff, 255);
/// The REAL Dooduel `SCRIM` token (`apps/dooduel/src/theme.rs:164`).
const SCRIM_TRANSLUCENT: Color = Color::Custom(0x14, 0x16, 0x1b, 0x9c);
/// The canvas display size (a stand-in for the 600x375 in-game canvas).
const CANVAS_W: f32 = 400.0;
const CANVAS_H: f32 = 300.0;

/// The faithful raster scene: root column
///   [ base-game (fill, opaque) containing a raster (drawing canvas),
///     scrim (fill+fixed+top_layer, translucent) wrapping an opaque card ].
/// Paint order (document order, with the scrim escaped to the root tail):
/// root, base, raster, scrim, card.
fn raster_scrim_view(s: &RasterProbe) -> Element<Noop> {
    let canvas = raster(s.canvas.clone(), CANVAS_W, CANVAS_H);
    let base = Element::column(vec![canvas])
        .fill()
        .justify_center()
        .align_center()
        .background(GAME_BG);
    let card = Element::column(vec![])
        .width(200.0)
        .height(100.0)
        .background(CARD_BG);
    let scrim = Element::column(vec![card])
        .fill()
        .fixed()
        .top_layer()
        .justify_center()
        .align_center()
        .background(SCRIM_TRANSLUCENT);
    Element::column(vec![base, scrim]).fill()
}

/// Adapterless extract harness (the `ShowcaseExtractHarness` MainWorld-swap
/// idiom), plus the `Image` asset registration the raster path needs. No
/// `RenderApp`, so no wgpu adapter is requested.
struct RasterHarness {
    app: App,
    render: World,
    schedule: Schedule,
}

impl RasterHarness {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            // Register the `Image` asset (MinimalPlugins does not) so the raster
            // node's `Handle<Image>` resolves — matches the golden harness idiom
            // (`crates/buiy_core/src/render/golden.rs:645`).
            .add_plugins(bevy::image::ImagePlugin::default())
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(buiy_core::CorePlugin)
            .add_plugins(buiy_core::a11y::A11yPlugin)
            .add_plugins(buiy_core::layout::LayoutPlugin)
            .add_plugins(buiy_core::text::BuiyTextPlugin::default())
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(buiy_core::focus::FocusPlugin)
            .add_plugins(buiy_widgets::WidgetsPlugin)
            .add_plugins(buiy_core::render::BuiyRenderPlugin);
        app.add_message::<bevy::input::keyboard::KeyboardInput>();
        app.init_resource::<ButtonInput<KeyCode>>();

        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..Default::default()
            },
            PrimaryWindow,
        ));

        // A tiny solid-color 4x4 RGBA image (the drawing canvas stand-in).
        let canvas = {
            let mut images = app.world_mut().resource_mut::<Assets<Image>>();
            let img = Image::new_fill(
                Extent3d {
                    width: 4,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                TextureDimension::D2,
                &[0xff, 0x00, 0x00, 0xff], // opaque red
                TextureFormat::Rgba8UnormSrgb,
                bevy::asset::RenderAssetUsages::default(),
            );
            images.add(img)
        };

        app.ui(RasterProbe { canvas }, update, raster_scrim_view);

        let mut render = World::new();
        render.init_resource::<ExtractedNodesView>();
        render.init_resource::<buiy_core::render::extract::ExtractedEffectGroups>();
        render.init_resource::<MainWorld>();

        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems(extract_buiy_nodes);

        Self {
            app,
            render,
            schedule,
        }
    }

    fn settle(&mut self) {
        for _ in 0..6 {
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

    fn nodes(&self) -> Vec<ExtractedNode> {
        self.render.resource::<ExtractedNodesView>().0.nodes.clone()
    }
}

fn find_scrim(world: &mut World) -> Entity {
    let mut q = world.query::<(Entity, &Stacking)>();
    q.iter(world)
        .find(|(_, s)| s.top_layer != TopLayer::None)
        .map(|(e, _)| e)
        .expect("exactly one top_layer entity (the scrim) exists")
}

fn find_raster(world: &mut World) -> Entity {
    let mut q = world.query::<(Entity, &RasterImage)>();
    let matches: Vec<Entity> = q.iter(world).map(|(e, _)| e).collect();
    assert_eq!(matches.len(), 1, "exactly one raster node exists");
    matches[0]
}

fn find_by_bg(world: &mut World, color: Color) -> Entity {
    let want = color.to_token();
    let mut q = world.query::<(Entity, &Background)>();
    let matches: Vec<Entity> = q
        .iter(world)
        .filter(|(_, bg)| bg.color == want)
        .map(|(e, _)| e)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "expected one entity with background {color:?}, found {matches:?}"
    );
    matches[0]
}

fn dump_nodes(nodes: &[ExtractedNode]) {
    eprintln!("=== extracted nodes ({} total) ===", nodes.len());
    for (i, n) in nodes.iter().enumerate() {
        let lin = LinearRgba::from(n.color);
        eprintln!(
            "[{i}] entity={:?} pos=({:.1},{:.1}) size=({:.1},{:.1}) color=[{:.3},{:.3},{:.3},{:.3}]",
            n.entity,
            n.position.x,
            n.position.y,
            n.size.x,
            n.size.y,
            lin.red,
            lin.green,
            lin.blue,
            lin.alpha
        );
    }
}

#[test]
fn scrim_quad_paints_after_raster_in_the_real_interleave() {
    let mut h = RasterHarness::new();
    h.settle();

    let scrim = find_scrim(h.app.world_mut());
    let raster_e = find_raster(h.app.world_mut());
    let base = find_by_bg(h.app.world_mut(), GAME_BG);
    let card = find_by_bg(h.app.world_mut(), CARD_BG);

    h.extract();
    let nodes = h.nodes();
    dump_nodes(&nodes);

    // Run the REAL downstream pack (group-free, no text quads) — the exact
    // partition `prepare.rs` builds and `node.rs` draws from.
    let partition = pack_view_partitioned(&nodes, 0, &[]);

    eprintln!("flat_ranges          = {:?}", partition.flat_ranges);
    eprintln!("node_quad_anchors    = {:?}", partition.node_quad_anchors);
    eprintln!("quad blob length     = {}", partition.instances.len());

    // The scrim's quad INSTANCE index in the packed blob (D1 map).
    let scrim_slot = partition
        .quad_slot_of
        .get(&scrim)
        .copied()
        .expect("the scrim paints a quad (has a translucent Background)");
    // The raster's paint-order splice anchor — the SAME value `node.rs` uses
    // via `build_raster_draws` (raster.rs:508 joins entity -> this map).
    let raster_anchor = partition
        .node_quad_anchor_of
        .get(&raster_e)
        .copied()
        .expect("the raster node has a node_quad_anchor");
    let base_slot = partition.quad_slot_of.get(&base).copied();
    let card_slot = partition.quad_slot_of.get(&card).copied();
    eprintln!(
        "scrim_slot={scrim_slot} raster_anchor={raster_anchor} base_slot={base_slot:?} card_slot={card_slot:?}"
    );

    // The interleave `node.rs` executes: no gradients, one raster at its anchor.
    let raster_anchors = vec![raster_anchor];
    let steps = interleave_flat_draw(&partition.flat_ranges, &[], &raster_anchors);
    eprintln!("=== FlatDrawStep sequence ===");
    for (i, s) in steps.iter().enumerate() {
        eprintln!("  step[{i}] = {s:?}");
    }

    // Locate the Raster step and the Quads step covering the scrim's index.
    let raster_step_idx = steps
        .iter()
        .position(|s| matches!(s, FlatDrawStep::Raster(_)))
        .expect("the raster is drawn (a Raster step is present)");
    let scrim_quads_step_idx = steps.iter().position(|s| match s {
        FlatDrawStep::Quads(r) => r.contains(&scrim_slot),
        _ => false,
    });
    eprintln!("raster_step_idx={raster_step_idx} scrim_quads_step_idx={scrim_quads_step_idx:?}");

    // --- CORRECT-behavior assertions (this is the witness — RED if the CPU
    // interleave is where the bug lives) ---
    let scrim_quads_step_idx = scrim_quads_step_idx.expect(
        "the scrim's quad instance index is covered by SOME Quads draw step (else the \
         scrim quad is never drawn — the transparent-scrim bug at the CPU tier)",
    );
    assert!(
        scrim_quads_step_idx > raster_step_idx,
        "the scrim's Quads step (idx {scrim_quads_step_idx}) must come AFTER the Raster \
         step (idx {raster_step_idx}) so the translucent scrim paints OVER the canvas; \
         if it does not, the raster overpaints the scrim (the transparent-scrim bug)"
    );
}
