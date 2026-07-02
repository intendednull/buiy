//! Headless (no wgpu adapter) verification of the C6-b per-side BORDER + box
//! SHADOW render channels (styling-f-tier.md § 2.2 / § 2.3). Three tiers, all
//! pure CPU (the GPU residue is `render_border_shadow_gpu.rs`):
//!
//! 1. **Pure functions** — `resolve_border` (per-side color/width/radius, the
//!    band AT the box edge, inner-radius shrink) and `resolve_shadows` (the
//!    `sigma = blur/2` factor, outset-only with inset warn-skip, the
//!    forced-colors producer suppression).
//! 2. **Pack** — `pack_band_instances` (one band per node border) +
//!    `pack_shadow_instances` (one `(Shadow, layer)` instance per shadow term),
//!    and the byte-stable strides (the quad 68 B / band 192 B unchanged).
//! 3. **End-to-end extract** — drive the REAL `extract_buiy_nodes` over a
//!    bordered + shadowed widget, asserting `ExtractedNode.border` /
//!    `.shadows` populate, and that `forced_colors` empties the shadow list
//!    while the border survives.
//!
//! The audit gap (`2026-06-21-todomvc-prototype-audit.md`): "shadow.wgsl +
//! Shadow primitive exist but are unfed" + the per-side border band oracle is
//! present but wired to no shader. These tests are RED before C6-b (no
//! `border`/`shadows` field, no `resolve_border`/`resolve_shadows`, no shadow
//! pipeline) and GREEN after.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::Length;
use buiy_core::components::Node;
use buiy_core::layout::Edges;
use buiy_core::render::ColorToken;
use buiy_core::render::buckets::{pack_band_instances, pack_shadow_instances};
use buiy_core::render::components::{
    Border, BorderSide, BoxShadow, Corners, LineStyle, Radius, Shadow,
};
use buiy_core::render::extract::{
    ExtractedNodesView, extract_buiy_nodes, resolve_border, resolve_shadows,
};
use buiy_core::render::instance::{border_band_stride_agrees, packed_raw_stride_agrees};
use buiy_core::theme::{UserPreferences, default_light_theme};

// --- Tier 1: the pure border resolver ---------------------------------------

/// A `Border` with the four sides set to distinct, resolvable token colors and
/// the given uniform radius. Widths are layout-owned (`BoxModel.border`), passed
/// separately to `resolve_border`.
fn four_color_border(radius_px: f32) -> Border {
    let side = |color: ColorToken| BorderSide {
        color,
        style: LineStyle::Solid,
    };
    Border {
        // Four distinct semantic tokens, so each side resolves to a DISTINCT
        // real color.
        top: side(ColorToken::Accent),
        right: side(ColorToken::TextPrimary),
        bottom: side(ColorToken::TextSecondary),
        left: side(ColorToken::SurfaceSecondary),
        radius: Corners::all(Radius::circular(radius_px)),
    }
}

#[test]
fn resolve_border_emits_per_side_colors_and_widths_at_the_box_edge() {
    let theme = default_light_theme();
    let border = four_color_border(0.0);
    // Per-side widths from BoxModel.border (the layout-owned Taffy input).
    let widths = Edges {
        top: Length::px(2.0),
        right: Length::px(4.0),
        bottom: Length::px(6.0),
        left: Length::px(8.0),
    };
    let got = resolve_border(
        &border,
        widths,
        Vec2::new(10.0, 30.0),
        Vec2::new(40.0, 20.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    )
    .expect("a fully-styled border produces a band");

    // The band sits AT the box edge: the outer box IS the border box (NOT grown,
    // unlike the outline). The pos/size are the node's position/size.
    assert_eq!(got.outer_pos, Vec2::new(10.0, 30.0));
    assert_eq!(got.outer_size, Vec2::new(40.0, 20.0));
    // Per-side widths [top, right, bottom, left].
    assert_eq!(got.width, [2.0, 4.0, 6.0, 8.0]);

    // Each side resolves to a DISTINCT, non-magenta color (the per-side channel).
    let sentinel = [1.0, 0.0, 1.0, 1.0];
    for c in [
        got.color_top,
        got.color_right,
        got.color_bottom,
        got.color_left,
    ] {
        assert_ne!(c, sentinel, "every side resolves, not magenta-miss");
    }
    assert_ne!(got.color_top, got.color_right, "top != right (per-side)");
    assert_ne!(
        got.color_bottom, got.color_left,
        "bottom != left (per-side)"
    );
}

#[test]
fn resolve_border_shrinks_inner_radius_by_adjacent_width() {
    // The inner radius shrinks per corner by the adjacent border width (the
    // oracle's load-bearing shrink, render_border_sdf.rs). With a uniform 10px
    // outer radius and per-side widths, each corner's inner radius is
    // max(outer - adjacent_width, 0) on each axis.
    let theme = default_light_theme();
    let border = four_color_border(10.0);
    let widths = Edges {
        top: Length::px(3.0),
        right: Length::px(4.0),
        bottom: Length::px(5.0),
        left: Length::px(6.0),
    };
    let got = resolve_border(
        &border,
        widths,
        Vec2::ZERO,
        Vec2::new(100.0, 80.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    )
    .expect("band");

    // outer_radius is 10 on every corner (clamped to <= half the box = 40/50).
    assert_eq!(got.outer_radius, [10.0; 8]);
    // inner_radius: TL shrinks x by left(6), y by top(3) → (4, 7).
    //               TR shrinks x by right(4), y by top(3) → (6, 7).
    //               BR shrinks x by right(4), y by bottom(5) → (6, 5).
    //               BL shrinks x by left(6), y by bottom(5) → (4, 5).
    assert_eq!(
        got.inner_radius,
        [4.0, 7.0, 6.0, 7.0, 6.0, 5.0, 4.0, 5.0],
        "inner radius shrinks per corner by the adjacent side widths"
    );
}

#[test]
fn resolve_border_clamps_radius_to_half_the_box() {
    // CSS clamps each corner radius to half the box dimension on its axis: a
    // 100px radius on a 40x20 box clamps to (20, 10).
    let theme = default_light_theme();
    let border = four_color_border(100.0);
    let widths = Edges::all(2.0);
    let got = resolve_border(
        &border,
        widths,
        Vec2::ZERO,
        Vec2::new(40.0, 20.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    )
    .expect("band");
    // half_w = 20, half_h = 10 → every corner (rx, ry) = (20, 10).
    assert_eq!(
        got.outer_radius,
        [20.0, 10.0, 20.0, 10.0, 20.0, 10.0, 20.0, 10.0]
    );
}

#[test]
fn resolve_border_skips_when_no_side_paints() {
    let theme = default_light_theme();

    // All sides `None`-style → no band.
    let plain = Border::default();
    assert!(
        resolve_border(
            &plain,
            Edges::all(4.0),
            Vec2::ZERO,
            Vec2::splat(10.0),
            None,
            [[1.0, 0.0], [0.0, 1.0]],
            &theme,
        )
        .is_none(),
        "a default (None-style) border paints no band"
    );

    // Styled sides but ZERO width everywhere → no band.
    let styled = four_color_border(0.0);
    assert!(
        resolve_border(
            &styled,
            Edges::ZERO,
            Vec2::ZERO,
            Vec2::splat(10.0),
            None,
            [[1.0, 0.0], [0.0, 1.0]],
            &theme,
        )
        .is_none(),
        "zero width on every side paints no band"
    );
}

#[test]
fn resolve_border_paints_only_the_styled_sides() {
    // A border with only the TOP side styled paints a transparent color on the
    // other three sides (so the band's per-side selection draws nothing there),
    // while still emitting a band for the top edge.
    let theme = default_light_theme();
    let border = Border {
        top: BorderSide {
            color: ColorToken::Accent,
            style: LineStyle::Solid,
        },
        ..Default::default()
    };
    let got = resolve_border(
        &border,
        Edges::all(3.0),
        Vec2::ZERO,
        Vec2::splat(50.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    )
    .expect("the styled top side emits a band");
    assert_ne!(got.color_top, [0.0; 4], "the styled top side has a color");
    assert_eq!(
        got.color_right, [0.0; 4],
        "an unstyled side is transparent (paints nothing)"
    );
    assert_eq!(got.color_bottom, [0.0; 4]);
    assert_eq!(got.color_left, [0.0; 4]);
}

// --- Tier 1: the pure shadow resolver ---------------------------------------

#[test]
fn resolve_shadows_pins_sigma_to_half_the_blur_and_expands_the_box() {
    // sigma = blur / 2 (the CSS blur-radius → Gaussian-sigma factor, § 3.2). The
    // shadow box = border box ⊕ spread ⊕ offset.
    let theme = default_light_theme();
    let bs = BoxShadow(vec![Shadow {
        color: ColorToken::TextPrimary,
        offset_x: Length::px(4.0),
        offset_y: Length::px(8.0),
        blur: Length::px(10.0),
        spread: Length::px(2.0),
        inset: false,
    }]);
    let got = resolve_shadows(
        &bs,
        Vec2::new(20.0, 30.0),
        Vec2::new(40.0, 50.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        false,
        &theme,
    );
    assert_eq!(got.len(), 1, "one resolved term for one outset shadow");
    let s = got[0];
    // sigma = 10 / 2 = 5.
    assert_eq!(
        s.sigma, 5.0,
        "sigma is blur/2 (the de-facto-standard factor)"
    );
    // pos = border-box pos (20,30) + offset (4,8) - spread (2,2) = (22, 36).
    assert_eq!(s.rect_pos, Vec2::new(22.0, 36.0));
    // size = border-box (40,50) + 2*spread (4,4) = (44, 54).
    assert_eq!(s.rect_size, Vec2::new(44.0, 54.0));
    let sentinel = [1.0, 0.0, 1.0, 1.0];
    assert_ne!(s.color, sentinel, "the shadow color resolves, not magenta");
}

#[test]
fn resolve_shadows_preserves_css_list_order() {
    // Index 0 is frontmost; the list order is preserved verbatim.
    let theme = default_light_theme();
    let bs = BoxShadow(vec![
        Shadow {
            color: ColorToken::Accent,
            offset_x: Length::px(1.0),
            blur: Length::px(2.0),
            ..Default::default()
        },
        Shadow {
            color: ColorToken::Accent,
            offset_x: Length::px(9.0),
            blur: Length::px(2.0),
            ..Default::default()
        },
    ]);
    let got = resolve_shadows(
        &bs,
        Vec2::ZERO,
        Vec2::splat(10.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        false,
        &theme,
    );
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].rect_pos.x, 1.0, "index 0 first (frontmost)");
    assert_eq!(got[1].rect_pos.x, 9.0, "index 1 second");
}

#[test]
fn resolve_shadows_skips_inset_terms_in_v1() {
    // v1 ships outset only; an inset term is warn-and-skipped (§ 3.1). A list of
    // one inset + one outset yields ONE resolved term (the outset).
    let theme = default_light_theme();
    let bs = BoxShadow(vec![
        Shadow {
            color: ColorToken::Accent,
            blur: Length::px(4.0),
            inset: true,
            ..Default::default()
        },
        Shadow {
            color: ColorToken::Accent,
            blur: Length::px(4.0),
            inset: false,
            ..Default::default()
        },
    ]);
    let got = resolve_shadows(
        &bs,
        Vec2::ZERO,
        Vec2::splat(10.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        false,
        &theme,
    );
    assert_eq!(
        got.len(),
        1,
        "the inset term is skipped; only the outset remains"
    );
}

#[test]
fn resolve_shadows_suppresses_all_under_forced_colors() {
    // Forced-colors empties the shadow list at the producer (§ 2.5) — a
    // shadow-only affordance is then invisible, which the structural-cue
    // guarantee relies on. The SAME shadow that resolves to one term in the
    // default theme resolves to ZERO under forced-colors.
    let theme = default_light_theme();
    let bs = BoxShadow(vec![Shadow {
        color: ColorToken::TextPrimary,
        blur: Length::px(8.0),
        ..Default::default()
    }]);
    let normal = resolve_shadows(
        &bs,
        Vec2::ZERO,
        Vec2::splat(10.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        false,
        &theme,
    );
    assert_eq!(normal.len(), 1, "the shadow paints under the default theme");
    let forced = resolve_shadows(
        &bs,
        Vec2::ZERO,
        Vec2::splat(10.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        true, // forced_colors
        &theme,
    );
    assert!(
        forced.is_empty(),
        "forced-colors suppresses every shadow at the producer"
    );
}

// --- Tier 2: pack + byte-stability ------------------------------------------

#[test]
fn pack_routes_border_to_band_and_shadow_to_shadow_blob() {
    use buiy_core::render::extract::{ExtractedBorder, ExtractedNode, ExtractedShadow};

    let bordered_and_shadowed = ExtractedNode {
        entity: Entity::from_raw_u32(1).unwrap(),
        position: Vec2::ZERO,
        size: Vec2::splat(20.0),
        color: Color::WHITE,
        clip: None,
        group: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: Some(ExtractedBorder {
            outer_pos: Vec2::ZERO,
            outer_size: Vec2::splat(20.0),
            color_top: [0.2, 0.45, 0.95, 1.0],
            color_right: [0.2, 0.45, 0.95, 1.0],
            color_bottom: [0.2, 0.45, 0.95, 1.0],
            color_left: [0.2, 0.45, 0.95, 1.0],
            width: [2.0, 2.0, 2.0, 2.0],
            outer_radius: [0.0; 8],
            inner_radius: [0.0; 8],
            clip: None,
            affine: [[1.0, 0.0], [0.0, 1.0]],
        }),
        shadows: vec![
            ExtractedShadow {
                rect_pos: Vec2::new(2.0, 2.0),
                rect_size: Vec2::splat(20.0),
                color: [0.0, 0.0, 0.0, 0.5],
                sigma: 3.0,
                clip: None,
                affine: [[1.0, 0.0], [0.0, 1.0]],
            },
            ExtractedShadow {
                rect_pos: Vec2::new(4.0, 4.0),
                rect_size: Vec2::splat(20.0),
                color: [0.0, 0.0, 0.0, 0.25],
                sigma: 6.0,
                clip: None,
                affine: [[1.0, 0.0], [0.0, 1.0]],
            },
        ],
        gradients: Vec::new(),
    };
    let plain = ExtractedNode {
        entity: Entity::from_raw_u32(2).unwrap(),
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
        color: Color::WHITE,
        clip: None,
        group: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    };

    let nodes = [bordered_and_shadowed, plain];
    // One band per node border (the bordered node), zero for the plain node.
    let bands = pack_band_instances(&nodes);
    assert_eq!(bands.len(), 1, "one band instance for the bordered node");
    // The band carries the per-side widths verbatim.
    assert_eq!(bands[0].width, [2.0, 2.0, 2.0, 2.0]);

    // One shadow instance per shadow TERM (two on the shadowed node).
    let shadows = pack_shadow_instances(&nodes);
    assert_eq!(shadows.len(), 2, "two shadow instances (two shadow terms)");
    // The radius slot carries the blur sigma (§ 2.2 — shadow.wgsl reads it as blur).
    assert_eq!(shadows[0].radius, 3.0);
    assert_eq!(shadows[1].radius, 6.0);
}

#[test]
fn strides_are_byte_stable() {
    // The R1/R2-frozen quad 68 B stride is untouched (shadow reuses it — radius
    // slot → blur sigma, NOT a stride bump), and the band record stays 192 B.
    assert!(packed_raw_stride_agrees(), "quad stays 68 B (= [f32;17])");
    assert_eq!(std::mem::size_of::<[f32; 17]>(), 68);
    assert!(border_band_stride_agrees(), "band stays 192 B (= 48 f32)");
    assert_eq!(
        buiy_core::render::instance::BORDER_BAND_INSTANCE_STRIDE_BYTES,
        192
    );
}

// --- Tier 3: end-to-end through the REAL extract system ---------------------

/// Adapterless extract harness (mirrors `render_focus_ring.rs`): swap the live
/// main world into a bare render world's `MainWorld` slot, run an
/// `ExtractSchedule` carrying the production `extract_buiy_nodes`, swap back, and
/// read `ExtractedNodesView`. No wgpu adapter requested.
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

    fn node_for(&self, entity: Entity) -> Option<buiy_core::render::extract::ExtractedNode> {
        self.render
            .resource::<ExtractedNodesView>()
            .0
            .nodes
            .iter()
            .find(|n| n.entity == entity)
            .cloned()
    }

    fn band_count(&self) -> usize {
        pack_band_instances(&self.render.resource::<ExtractedNodesView>().0.nodes).len()
    }

    fn shadow_count(&self) -> usize {
        pack_shadow_instances(&self.render.resource::<ExtractedNodesView>().0.nodes).len()
    }
}

/// Spawn a bordered + shadowed laid-out widget. The border WIDTH lives in the
/// layout `Style` (`border_*`), which Taffy folds into `BoxModel.border`; the
/// border PAINT + shadow are render components.
fn spawn_bordered_shadowed(app: &mut App) -> Entity {
    use buiy_core::layout::{Inset, Sizing, Style};
    use buiy_core::render::components::Background;

    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(40.0)),
                    left: Sizing::Length(Length::px(40.0)),
                    ..default()
                })
                .width_px(80.0)
                .height_px(40.0)
                .border(3.0),
            Background {
                color: ColorToken::SurfacePrimary,
            },
            four_color_border(0.0),
            BoxShadow(vec![Shadow {
                color: ColorToken::TextPrimary,
                offset_x: Length::px(0.0),
                offset_y: Length::px(4.0),
                blur: Length::px(8.0),
                spread: Length::px(0.0),
                inset: false,
            }]),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[child]);
    child
}

fn settle(h: &mut NodeExtractHarness) {
    for _ in 0..4 {
        h.update();
    }
}

#[test]
fn bordered_shadowed_widget_extracts_border_band_and_shadow() {
    let mut h = NodeExtractHarness::new();
    let widget = spawn_bordered_shadowed(&mut h.app);
    settle(&mut h);
    h.extract();

    let node = h
        .node_for(widget)
        .expect("the widget reaches the display list");

    // The border band resolved: the layout-owned 3px border width threads into
    // the band, and the per-side colors resolved (not magenta).
    let border = node
        .border
        .expect("a bordered widget extracts a Border band (C6-b)");
    assert_eq!(
        border.width,
        [3.0, 3.0, 3.0, 3.0],
        "the layout-owned BoxModel.border width threads into the band"
    );
    let sentinel = [1.0, 0.0, 1.0, 1.0];
    assert_ne!(border.color_top, sentinel, "the top side color resolved");

    // The shadow resolved: one term, sigma = blur/2 = 4, offset-shifted box.
    assert_eq!(node.shadows.len(), 1, "one outset shadow term extracted");
    assert_eq!(node.shadows[0].sigma, 4.0, "sigma = blur(8)/2 = 4");
    // The box is the border box shifted down by offset_y = 4.
    assert!(
        node.shadows[0].rect_pos.y > node.position.y,
        "the +4 offset_y shifts the shadow box downward (got {:?} vs node {:?})",
        node.shadows[0].rect_pos,
        node.position,
    );

    // The packers route them: one band, one shadow instance.
    assert_eq!(h.band_count(), 1, "one border band instance");
    assert_eq!(h.shadow_count(), 1, "one box-shadow instance");
}

#[test]
fn forced_colors_empties_the_shadow_but_keeps_the_border() {
    // Under forced-colors the shadow producer emits nothing (§ 2.5), but the
    // border survives (it resolves through the forced-colors Theme variant). So
    // a structural cue stays visible while the shadow-only affordance vanishes.
    let mut h = NodeExtractHarness::new();
    let widget = spawn_bordered_shadowed(&mut h.app);
    settle(&mut h);

    // Flip forced-colors ON. (Set the preference flag; extract reads it directly.
    // The theme-swap edge that normally re-extracts is irrelevant here — we run
    // a fresh extract below, which always reads the current flag.)
    h.app
        .world_mut()
        .resource_mut::<UserPreferences>()
        .forced_colors = true;
    h.update();
    h.extract();

    let node = h.node_for(widget).expect("widget in display list");
    assert!(
        node.shadows.is_empty(),
        "forced-colors suppresses the box-shadow (the structural-cue guarantee)"
    );
    assert!(
        node.border.is_some(),
        "the border survives forced-colors (a non-shadow structural cue)"
    );
    assert_eq!(
        h.shadow_count(),
        0,
        "no shadow instance under forced-colors"
    );
    assert_eq!(h.band_count(), 1, "the border band still draws");
}
