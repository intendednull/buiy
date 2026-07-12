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

// --- Task 1.4: band (border/outline) packer boundary -------------------------

use buiy_core::render::buckets::pack_band_instances;
use buiy_core::render::extract::ExtractedBorder;

/// A solid yellow border term (one band instance).
fn border_term() -> ExtractedBorder {
    ExtractedBorder {
        outer_pos: Vec2::ZERO,
        outer_size: Vec2::splat(10.0),
        color_top: [1.0, 1.0, 0.0, 1.0],
        color_right: [1.0, 1.0, 0.0, 1.0],
        color_bottom: [1.0, 1.0, 0.0, 1.0],
        color_left: [1.0, 1.0, 0.0, 1.0],
        width: [2.0, 2.0, 2.0, 2.0],
        style: [0.0, 0.0, 0.0, 0.0],
        outer_radius: [0.0; 8],
        inner_radius: [0.0; 8],
        clip: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
    }
}

/// A fill node carrying one border band.
fn fill_with_border(entity: u32, top_layer: bool) -> ExtractedNode {
    let mut n = fill(entity, top_layer);
    n.border = Some(border_term());
    n
}

#[test]
fn band_boundary_at_first_top_layer_border() {
    // [base border, TOP border] — the band boundary is the instance index of the
    // top-layer node's border band (1).
    let nodes = [fill_with_border(1, false), fill_with_border(2, true)];
    let (bands, boundary) = pack_band_instances(&nodes);
    assert_eq!(bands.len(), 2, "one band per bordered node");
    assert_eq!(boundary, 1, "boundary at the top-layer node's band");
}

#[test]
fn band_boundary_is_count_when_no_top_layer() {
    // No top-layer band ⇒ the boundary is the band count (empty top block).
    let nodes = [fill_with_border(1, false), fill_with_border(2, false)];
    let (bands, boundary) = pack_band_instances(&nodes);
    assert_eq!(boundary, bands.len() as u32);
    assert_eq!(boundary, 2);
}

// --- Task 1.5: glyph/icon partition (entity-keyed) + cut_ranges straddle ------

use buiy_core::render::buckets::{cut_ranges, partition_glyph_ranges};
use std::ops::Range;

/// One producer entity-run `(entity, instance range)` — the carrier-agnostic
/// shape of `ExtractedGlyphs`/`ExtractedIcons::entity_runs`.
fn grun(entity: u32, range: Range<u32>) -> (Entity, Range<u32>) {
    (Entity::from_raw_u32(entity).unwrap(), range)
}

/// True for the given entity id — a per-entity `top_layer_of` closure for the
/// glyph/icon partition (parallel to the group `group_of` closure).
fn is_entity(id: u32) -> impl Fn(Entity) -> bool {
    move |e: Entity| e == Entity::from_raw_u32(id).unwrap()
}

#[test]
fn glyph_boundary_at_first_top_layer_entity_run() {
    // runs: entity 1 [0..3] base, entity 2 [3..5] TOP — the glyph boundary is the
    // first top-layer entity's run start (3). Glyph + icon share this FUNCTION +
    // closure (called twice, distinct boundary each — separate instance spaces).
    let (_groups, _flat, boundary) =
        partition_glyph_ranges([grun(1, 0..3), grun(2, 3..5)], 5, 0, |_| None, is_entity(2));
    assert_eq!(boundary, 3, "boundary at the first top-layer entity's run");
}

#[test]
fn glyph_boundary_is_total_when_no_top_layer() {
    let (_groups, _flat, boundary) =
        partition_glyph_ranges([grun(1, 0..3), grun(2, 3..5)], 5, 0, |_| None, |_| false);
    assert_eq!(boundary, 5, "no top-layer entity ⇒ boundary == total");
}

#[test]
fn glyph_flat_run_coalesces_across_the_top_layer_boundary() {
    // The `RangePartitioner` splits flat runs on GROUP only, NOT on top_layer, so
    // a base + top-layer NON-group run COALESCES into ONE flat run that STRADDLES
    // the boundary — the motivation for `cut_ranges`. entity 1 [0..3] base +
    // entity 2 [3..8] TOP, both group=None ⇒ one flat run [0..8], boundary 3.
    let (_groups, flat, boundary) =
        partition_glyph_ranges([grun(1, 0..3), grun(2, 3..8)], 8, 0, |_| None, is_entity(2));
    assert_eq!(flat, vec![0..8], "flat run coalesces across the boundary");
    assert_eq!(boundary, 3);
    // The per-block draw slices this straddling run with `cut_ranges`.
    assert_eq!(cut_ranges(&flat, 0, boundary), vec![0..3], "base block");
    assert_eq!(
        cut_ranges(&flat, boundary, 8),
        vec![3..8],
        "top-layer block"
    );
}

#[test]
#[should_panic(expected = "contiguous tail")]
fn glyph_base_run_after_top_layer_run_trips_the_tripwire() {
    // A base entity's run AFTER a top-layer entity's run violates tail-contiguity.
    let _ = partition_glyph_ranges(
        [grun(1, 0..2), grun(2, 2..4), grun(3, 4..6)],
        6,
        0,
        |_| None,
        is_entity(2), // only entity 2 is top-layer; entity 3 (base) follows it
    );
}

// --- cut_ranges (the straddle-cut helper, pure) ------------------------------

#[test]
fn cut_ranges_cuts_a_straddling_run_at_the_boundary() {
    // The plan's witness: `2..8` cut at boundary 5 yields base `[2..5]` + top
    // `[5..8]` — the straddling run is CUT, not dropped. (`slice::from_ref` of the
    // single run, not a `[2..8]` array literal — the latter reads ambiguously as
    // `[2; 8]` to clippy.)
    let straddling = 2u32..8;
    let ranges = std::slice::from_ref(&straddling);
    assert_eq!(cut_ranges(ranges, 0, 5), vec![2..5], "base half [0,5)");
    assert_eq!(cut_ranges(ranges, 5, 8), vec![5..8], "top half [5,8)");
}

#[test]
fn cut_ranges_drops_runs_fully_outside_the_window() {
    // A run entirely outside `[lo,hi)` is dropped; partial overlap is clipped;
    // multiple runs are each intersected.
    let ranges = [0..2, 4..6, 8..10];
    assert_eq!(
        cut_ranges(&ranges, 3, 7),
        vec![4..6],
        "only the middle run overlaps"
    );
    assert_eq!(
        cut_ranges(&ranges, 1, 9),
        vec![1..2, 4..6, 8..9],
        "clip both ends"
    );
    assert!(
        cut_ranges(&ranges, 20, 30).is_empty(),
        "window past every run"
    );
}

// === Task 2.1: block_interleave (base/top-layer split of the flat draw) =======
//
// `block_interleave` splits the interleaved flat-draw schedule at the quad
// boundary into a BASE block + a TOP-LAYER block, both with ABSOLUTE indices. The
// base block draws every flat instance BEFORE the boundary; the top-layer block
// draws every one AT/after it (its `Gradients`/`Raster` indices re-offset back to
// absolute after `interleave_flat_draw` re-bases the sliced sub-array to 0). The
// quad `flat_ranges` are `cut_ranges`-sliced so a straddling run is cut, not
// dropped or double-drawn. This is the pure/headless heart of the W2 per-block
// draw restructure.

use buiy_core::render::buckets::{FlatDrawStep, block_interleave, interleave_flat_draw};

/// Build flat quad runs from `(start, end)` pairs (the `render_buckets.rs` idiom —
/// a helper sidesteps clippy's `single_range_in_vec_init`).
fn runs(pairs: &[(u32, u32)]) -> Vec<Range<u32>> {
    pairs.iter().map(|&(s, e)| s..e).collect()
}

#[test]
fn block_interleave_splits_quads_and_rasters_at_the_boundary() {
    // flat run [0..4], quad_boundary 2; a raster at anchor 1 (base) + a raster at
    // anchor 3 (top-layer). The base block draws only the [0..2) quads + the base
    // raster (index 0); the top block draws only the [2..4) quads + the top raster
    // (re-offset to absolute index 1). NO base step references a top-layer
    // instance (a quad >= 2 or the raster at anchor 3).
    let (base, top) = block_interleave(&runs(&[(0, 4)]), &[], &[1, 3], 2);
    assert_eq!(
        base,
        vec![
            FlatDrawStep::Quads(0..1),
            FlatDrawStep::Raster(0),
            FlatDrawStep::Quads(1..2),
        ],
        "base block: quads [0..2) split by the base raster (anchor 1)"
    );
    assert_eq!(
        top,
        vec![
            FlatDrawStep::Quads(2..3),
            FlatDrawStep::Raster(1),
            FlatDrawStep::Quads(3..4),
        ],
        "top block: quads [2..4) split by the top raster (re-offset to absolute index 1)"
    );
    // The load-bearing guarantee: no base step references a top-layer instance.
    for step in &base {
        match step {
            FlatDrawStep::Quads(r) => {
                assert!(r.end <= 2, "base quad run {r:?} stays below the boundary")
            }
            FlatDrawStep::Raster(k) => assert_eq!(*k, 0, "base references only the base raster"),
            FlatDrawStep::Gradients(_) => {}
        }
    }
}

#[test]
fn block_interleave_re_offsets_top_gradient_indices_to_absolute() {
    // flat run [0..6], quad_boundary 4; gradients at anchors [1 (base), 5 (top)].
    // The base gradient keeps absolute index 0; the top gradient is re-offset from
    // the sliced sub-array's 0 back to absolute index 1, and its quad ranges stay
    // absolute (>= 4).
    let (base, top) = block_interleave(&runs(&[(0, 6)]), &[1, 5], &[], 4);
    assert_eq!(
        base,
        vec![
            FlatDrawStep::Quads(0..1),
            FlatDrawStep::Gradients(0..1),
            FlatDrawStep::Quads(1..4),
        ],
        "base block: [0..4) quads, base gradient at absolute index 0"
    );
    assert_eq!(
        top,
        vec![
            FlatDrawStep::Quads(4..5),
            FlatDrawStep::Gradients(1..2),
            FlatDrawStep::Quads(5..6),
        ],
        "top block: [4..6) quads, top gradient re-offset to absolute index 1"
    );
}

#[test]
fn block_interleave_cuts_a_straddling_flat_run() {
    // A single flat run [0..8] with NO group gap STRADDLES the boundary 3 (the
    // RangePartitioner splits on group only). `cut_ranges` cuts it: base draws
    // [0..3), top draws [3..8) — the run is not dropped or double-drawn.
    let (base, top) = block_interleave(&runs(&[(0, 8)]), &[], &[], 3);
    assert_eq!(
        base,
        vec![FlatDrawStep::Quads(0..3)],
        "base half of the straddling run"
    );
    assert_eq!(
        top,
        vec![FlatDrawStep::Quads(3..8)],
        "top half of the straddling run"
    );
}

#[test]
fn block_interleave_empty_top_block_is_byte_identical_to_the_single_interleave() {
    // Byte-stability (F9): with `quad_boundary` at/above the quad count, EVERY
    // anchor is base — the base block equals a single `interleave_flat_draw` call
    // and the top block is empty. `node.rs` passes `u32::MAX` on a no-top-layer
    // view, so it issues the identical draws.
    let flat = runs(&[(0, 6)]);
    let grad = [2u32];
    let raster = [4u32];
    // boundary == the quad count (6): all anchors < 6 stay base.
    let (base, top) = block_interleave(&flat, &grad, &raster, 6);
    assert_eq!(
        base,
        interleave_flat_draw(&flat, &grad, &raster),
        "base == today's single interleave"
    );
    assert!(top.is_empty(), "empty top-layer block");
    // `u32::MAX` (node.rs's no-top-layer sentinel) is equivalent.
    let (base_max, top_max) = block_interleave(&flat, &grad, &raster, u32::MAX);
    assert_eq!(base_max, interleave_flat_draw(&flat, &grad, &raster));
    assert!(top_max.is_empty());
}

#[test]
fn block_interleave_all_top_layer_leaves_the_base_empty() {
    // quad_boundary 0 ⇒ every instance is top-layer; the base block is empty and
    // the top block is byte-identical to a single interleave (indices unchanged —
    // grad_split/raster_split are 0, so no re-offset).
    let flat = runs(&[(0, 5)]);
    let grad = [1u32, 3];
    let raster = [2u32];
    let (base, top) = block_interleave(&flat, &grad, &raster, 0);
    assert!(base.is_empty(), "no base instances");
    assert_eq!(
        top,
        interleave_flat_draw(&flat, &grad, &raster),
        "the whole schedule is the top block, indices unchanged"
    );
}

#[test]
fn block_interleave_routes_an_at_boundary_gradient_to_the_top_block() {
    // A bare gradient-only top-layer node's gradient anchors AT the quad boundary
    // (its Color::NONE node pushes no quad, so its anchor == the quad count == the
    // boundary tl.quad). `a < boundary` is FALSE at the boundary, so the gradient
    // routes to the TOP block — where Signal B's has_top_layer gate opens the top
    // pass so it occludes. This is exactly what the bare-gradient-only occlusion
    // gap depended on (drift-#1 fix).
    let (base, top) = block_interleave(&runs(&[(0, 3)]), &[3], &[], 3);
    assert_eq!(
        base,
        vec![FlatDrawStep::Quads(0..3)],
        "the base block draws its quads, NOT the at-boundary gradient"
    );
    assert_eq!(
        top,
        vec![FlatDrawStep::Gradients(0..1)],
        "the at-boundary gradient routes to the TOP block"
    );
}

// === Signal B: PackedPartition.any_top_layer authoritative bit (drift-#1 fix) ==
//
// The per-tier boundaries CANNOT detect a bare gradient/raster-only top-layer
// node: it pushes NO quad/shadow/band/glyph/icon instance, so every per-tier
// boundary == its count. The pre-fix `has_top_layer` gate (any per-tier boundary
// < count) reads such a scene as "no top-layer content" and SKIPS the top block,
// so the overlay's gradient/raster never occludes base text/icons/borders (the
// bare-overlay gap). `PackedPartition.any_top_layer` is the authoritative fix: it
// rides the SAME TopLayerBoundaryTracker (which observes EVERY node, quad or not),
// so a Color::NONE top-layer node still flips it true — tier- AND anchor-
// independent, and byte-IDENTICAL for no-top scenes (u32::MAX boundary → base).

/// A Color::NONE top-layer node — a bare gradient/raster-only overlay member. It
/// pushes NO quad instance, so it moves no per-tier boundary; only the node-level
/// `any_top_layer` bit (the tracker observes it regardless) can see it.
fn bare_top(entity: u32) -> ExtractedNode {
    ExtractedNode {
        color: Color::NONE,
        ..fill(entity, true)
    }
}

#[test]
fn any_top_layer_true_for_a_bare_gradient_or_raster_only_top_node() {
    // [base fill, bare TOP (Color::NONE)]: the top node pushes no quad, so the QUAD
    // boundary is the full count — the RED witness that the pre-fix per-tier gate
    // (boundary < count) MISSES it. any_top_layer is TRUE regardless.
    let nodes = [fill(1, false), bare_top(2)];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert_eq!(p.instances.len(), 1, "only the base fill pushes a quad");
    assert_eq!(
        p.top_layer_boundary,
        p.instances.len() as u32,
        "the QUAD boundary is the count — no per-tier boundary can see the bare top node"
    );
    assert!(
        p.any_top_layer,
        "any_top_layer is TRUE — the authoritative bit sees the Color::NONE top-layer node"
    );
}

#[test]
fn any_top_layer_false_for_a_no_top_layer_scene() {
    // No top-layer node ⇒ any_top_layer false (the byte-stable base-only path;
    // node.rs then routes quad_boundary to u32::MAX — identical draws).
    let nodes = [fill(1, false), fill(2, false)];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert!(!p.any_top_layer, "no top-layer node ⇒ any_top_layer false");
}

#[test]
fn any_top_layer_true_when_a_top_node_has_a_quad() {
    // The common case (a scrim fill): any_top_layer true AND the quad boundary <
    // count — both signals agree, so Signal B strictly SUBSUMES the per-tier gate.
    let nodes = [fill(1, false), fill(2, true)];
    let p = pack_view_partitioned(&nodes, 0, &[]);
    assert!(p.any_top_layer);
    assert_eq!(p.top_layer_boundary, 1);
}
