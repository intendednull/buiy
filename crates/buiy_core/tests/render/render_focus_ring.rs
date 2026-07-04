//! Headless (no wgpu adapter) verification of the C6-a focus-ring + Outline
//! render channel (styling-f-tier.md § 2.4 / § 2.6). Two tiers, both pure CPU:
//!
//! 1. **Pure functions** — `resolve_outline` + `effective_outline_clip`: the
//!    geometry (the ring sits `width + offset` outside the border box) and the
//!    clip selection (the OUTLINE clip is the `AncestorClip`, NOT the own box,
//!    so a ring survives an `overflow:hidden` ancestor).
//! 2. **End-to-end extract** — drive the REAL `extract_buiy_nodes` system via a
//!    `MainWorld` swap (the adapterless extract idiom, `support::extract_harness`)
//!    over a focused widget, asserting the `ExtractedNode.outline` (and so the
//!    packed band) appears iff `entity == FocusedEntity.0 && FocusVisible.0`,
//!    and NOT for a pointer-focused (`FocusVisible(false)`) or unfocused entity.
//!
//! The audit's CRITICAL bug (`2026-06-21-todomvc-prototype-audit.md` finding #1,
//! WCAG 2.4.7) is "keyboard focus is structurally invisible": `Outline` never
//! extracted/painted + `FocusVisible` read by no paint system. These tests are
//! RED before C6-a (no `outline` field, no band, no `lower_focus_ring`) and
//! GREEN after.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::components::Node;
use buiy_core::render::ColorToken;
use buiy_core::render::buckets::pack_band_instances;
use buiy_core::render::components::{AncestorClip, ClipRect, LineStyle, Outline};
use buiy_core::render::extract::{
    ExtractedNodesView, effective_outline_clip, extract_buiy_nodes, resolve_outline,
};
use buiy_core::theme::default_light_theme;

// --- Tier 1: the pure outline geometry + clip-selection functions -----------

#[test]
fn resolve_outline_grows_box_by_width_plus_offset() {
    // The ring outer box is the border box grown by `width + offset` on every
    // side (styling-f-tier.md § 2.4). A 40x20 box at (10,30), 2px ring, 2px
    // offset → grown by 4 each side: pos (6,26), size (48,28).
    let theme = default_light_theme();
    let outline = Outline {
        color: ColorToken::FocusRing,
        style: LineStyle::Solid,
        width: buiy_core::Length::px(2.0),
        offset: buiy_core::Length::px(2.0),
    };
    let got = resolve_outline(
        &outline,
        Vec2::new(10.0, 30.0),
        Vec2::new(40.0, 20.0),
        None,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    )
    .expect("a solid, positive-width, resolvable outline produces a band");

    assert_eq!(got.outer_pos, Vec2::new(6.0, 26.0));
    assert_eq!(got.outer_size, Vec2::new(48.0, 28.0));
    assert_eq!(got.width, 2.0);
    // The `FocusRing` token resolves to the default theme's high-contrast ring
    // color — an opaque, painted band.
    assert_ne!(
        got.color, [0.0; 4],
        "the focus-ring token must resolve to a painted color"
    );
}

#[test]
fn focus_ring_token_resolves_under_both_default_and_forced_colors() {
    // The ring is forced-colors-safe (styling-f-tier.md § 2.6): its `FocusRing`
    // token resolves to a real high-contrast color under BOTH the default theme
    // AND the forced-colors swap (where it maps to the system `Highlight` value).
    use buiy_core::render::color::{ColorToken, SystemColorKeyword, ThemeContract, resolve_token};
    use buiy_core::theme::forced_colors_theme;

    let token = ColorToken::FocusRing;

    // Under the default theme the ring resolves to a real, painted (non-
    // transparent) high-contrast color.
    let default_ring = resolve_token(&token, &default_light_theme());
    assert_ne!(
        default_ring,
        Color::NONE,
        "ring resolves to a painted color under the default theme"
    );

    let forced = forced_colors_theme();
    let forced_ring = resolve_token(&token, &forced);
    // Under forced-colors the ring takes the high-contrast `Highlight` value (the
    // swap maps `FocusRing` → `Highlight`), so a shadow-only / low-contrast
    // affordance can never make keyboard focus invisible there.
    assert_eq!(
        forced_ring,
        forced.resolve(ColorToken::SystemColor(SystemColorKeyword::Highlight)),
        "forced-colors focus ring uses the system Highlight value"
    );
}

#[test]
fn resolve_outline_skips_none_style_zero_width_and_transparent() {
    let theme = default_light_theme();
    let base = Outline {
        color: ColorToken::FocusRing,
        style: LineStyle::Solid,
        width: buiy_core::Length::px(2.0),
        offset: buiy_core::Length::px(2.0),
    };
    let pos = Vec2::ZERO;
    let size = Vec2::splat(10.0);
    let id = [[1.0, 0.0], [0.0, 1.0]];

    // style: None → no band.
    let none_style = Outline {
        style: LineStyle::None,
        ..base.clone()
    };
    assert!(resolve_outline(&none_style, pos, size, None, id, &theme).is_none());

    // zero width → no band.
    let zero_w = Outline {
        width: buiy_core::Length::px(0.0),
        ..base.clone()
    };
    assert!(resolve_outline(&zero_w, pos, size, None, id, &theme).is_none());

    // transparent color → no band.
    let transparent = Outline {
        color: ColorToken::Transparent,
        ..base
    };
    assert!(resolve_outline(&transparent, pos, size, None, id, &theme).is_none());
}

#[test]
fn outline_clip_is_ancestor_clip_not_own_box() {
    // The load-bearing survives-overflow:hidden property (styling-f-tier.md
    // § 2.4 / WCAG 2.4.7): `effective_outline_clip` takes the entity's
    // `AncestorClip` — the ancestor intersection WITHOUT the own-box step — so a
    // ring outside an `overflow:hidden` element is cropped by ancestors but NOT
    // erased by the element's own clip box.
    let ancestor = AncestorClip {
        min: Vec2::ZERO,
        max: Vec2::splat(500.0),
    };
    let got = effective_outline_clip(None, Some(&ancestor));
    assert_eq!(
        got,
        Some(ClipRect {
            min: ancestor.min,
            max: ancestor.max
        }),
        "the outline clips to the ANCESTOR clip, never the own box"
    );

    // The resolved outline carries that ancestor clip verbatim.
    let theme = default_light_theme();
    let outline = Outline {
        color: ColorToken::FocusRing,
        style: LineStyle::Solid,
        width: buiy_core::Length::px(2.0),
        offset: buiy_core::Length::px(0.0),
    };
    let resolved = resolve_outline(
        &outline,
        Vec2::splat(100.0),
        Vec2::splat(50.0),
        got,
        [[1.0, 0.0], [0.0, 1.0]],
        &theme,
    )
    .expect("outline band");
    assert_eq!(
        resolved.clip, got,
        "the band carries the ancestor (outline) clip"
    );
}

#[test]
fn pack_band_instances_emits_one_band_per_outlined_node() {
    // The packer produces one `BorderBandInstance` per node that carries an
    // outline; outline-free, border-free nodes contribute nothing (the
    // byte-stable path).
    use buiy_core::render::extract::{ExtractedNode, ExtractedOutline};

    let outlined = ExtractedNode {
        entity: Entity::from_raw_u32(1).unwrap(),
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
        radius: 0.0,
        color: Color::WHITE,
        clip: None,
        group: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: Some(ExtractedOutline {
            outer_pos: Vec2::splat(-2.0),
            outer_size: Vec2::splat(14.0),
            color: [0.2, 0.45, 0.95, 1.0],
            width: 2.0,
            outer_radius: [0.0; 8],
            inner_radius: [0.0; 8],
            clip: None,
            affine: [[1.0, 0.0], [0.0, 1.0]],
        }),
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    };
    let plain = ExtractedNode {
        entity: Entity::from_raw_u32(2).unwrap(),
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
        radius: 0.0,
        color: Color::WHITE,
        clip: None,
        group: None,
        affine: [[1.0, 0.0], [0.0, 1.0]],
        outline: None,
        border: None,
        shadows: Vec::new(),
        gradients: Vec::new(),
    };
    assert_eq!(pack_band_instances(std::slice::from_ref(&plain)).len(), 0);
    assert_eq!(pack_band_instances(&[outlined, plain.clone()]).len(), 1);
}

// --- Tier 2: end-to-end through the REAL extract system ---------------------

/// Adapterless extract: swap the live main world into a bare render world's
/// `MainWorld` slot, run an `ExtractSchedule` carrying the production
/// `extract_buiy_nodes`, swap back, and read `ExtractedNodesView`. The same
/// dance bevy_render's own `extract()` does, minus the renderer — so NO wgpu
/// adapter is requested (mirrors `support::extract_harness`).
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
            // The focus model: `handle_tab` (keyboard focus → FocusVisible(true))
            // and the C6-a `lower_focus_ring` lowering both live here.
            .add_plugins(buiy_core::focus::FocusPlugin)
            // BuiyRenderPlugin's MAIN-world half (write_clip_rects, paint-skip,
            // effect groups, forced colors) registers headless — its render half
            // is guarded on a RenderApp that never exists here, so no adapter.
            .add_plugins(buiy_core::render::BuiyRenderPlugin);
        // `handle_tab` reads `Res<ButtonInput<KeyCode>>`; MinimalPlugins has no
        // InputPlugin, so init the resource directly (the crosscut/focus.rs idiom).
        app.init_resource::<ButtonInput<KeyCode>>();
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

    /// The extracted node for `entity`, if it reached the display list.
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
}

/// Spawn a single focusable, laid-out widget at a fixed box. Returns its entity.
fn spawn_focusable(app: &mut App) -> Entity {
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::components::Background;

    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(20.0)),
                    left: Sizing::Length(Length::px(20.0)),
                    ..default()
                })
                .width_px(80.0)
                .height_px(30.0),
            Background {
                color: ColorToken::SurfacePrimary,
            },
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    // A root so layout/stacking has a context to walk.
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[child]);
    child
}

/// Settle layout + transform across a few frames (the bounded spawn-settle).
fn settle(h: &mut NodeExtractHarness) {
    for _ in 0..4 {
        h.update();
    }
}

#[test]
fn keyboard_focused_entity_gets_an_outline_band_in_extract() {
    let mut h = NodeExtractHarness::new();
    let widget = spawn_focusable(&mut h.app);
    settle(&mut h);

    // Press Tab → keyboard focus: handle_tab sets FocusedEntity = widget AND
    // FocusVisible(true); lower_focus_ring (.after Input) inserts the framework
    // ring `Outline` the same frame; write_clip_rects settles AncestorClip.
    h.app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Tab);
    h.update();
    // Release so the next update doesn't re-advance focus.
    h.app
        .world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::Tab);
    h.update();

    // Sanity: the focus signal is the keyboard-visible pair on the widget.
    assert_eq!(
        h.app.world().resource::<buiy_core::FocusedEntity>().0,
        Some(widget)
    );
    assert!(h.app.world().resource::<buiy_core::FocusVisible>().0);
    // The framework ring `Outline` is present on the widget.
    assert!(
        h.app.world().get::<Outline>(widget).is_some(),
        "lower_focus_ring inserts the framework Outline on the keyboard-focused widget"
    );

    h.extract();
    let node = h
        .node_for(widget)
        .expect("the widget reaches the display list");
    let outline = node
        .outline
        .expect("a keyboard-focused widget extracts an Outline band (the focus ring)");
    assert!(outline.width >= 2.0, "WCAG 2.4.11: the ring is >= 2px");
    assert_eq!(h.band_count(), 1, "exactly one band instance (the ring)");
}

#[test]
fn pointer_focused_entity_gets_no_outline_ring() {
    // Pointer focus sets FocusVisible(false) (C3d), so `lower_focus_ring` shows
    // NO ring — the correct `:focus-visible` behavior (a click does not draw a
    // keyboard ring). Driven by writing the resource pair directly (the pointer
    // path's net effect), since picking is not wired in this headless harness.
    let mut h = NodeExtractHarness::new();
    let widget = spawn_focusable(&mut h.app);
    settle(&mut h);

    h.app
        .world_mut()
        .resource_mut::<buiy_core::FocusedEntity>()
        .0 = Some(widget);
    h.app
        .world_mut()
        .resource_mut::<buiy_core::FocusVisible>()
        .0 = false;
    h.update();

    assert!(
        h.app.world().get::<Outline>(widget).is_none(),
        "pointer focus (FocusVisible=false) must NOT show a focus ring"
    );
    h.extract();
    let node = h.node_for(widget).expect("widget in display list");
    assert!(
        node.outline.is_none(),
        "no Outline band for a pointer-focused (not focus-visible) entity"
    );
    assert_eq!(h.band_count(), 0);
}

#[test]
fn unfocused_entity_gets_no_outline_ring() {
    let mut h = NodeExtractHarness::new();
    let widget = spawn_focusable(&mut h.app);
    settle(&mut h);
    h.extract();

    let node = h.node_for(widget).expect("widget in display list");
    assert!(node.outline.is_none(), "nothing focused → no ring");
    assert_eq!(h.band_count(), 0);
}

#[test]
fn focus_ring_outline_clip_is_ancestor_not_own_overflow_hidden_box() {
    // The survives-overflow:hidden property end-to-end: a focusable nested in an
    // `overflow: hidden` parent gets a focus ring whose extract clip is the
    // ANCESTOR clip (the parent's clip box), so the ring is NOT erased by the
    // parent's own-box clip the way the FILL is (styling-f-tier.md § 2.4).
    use buiy_core::layout::{Inset, Length, Sizing, Style};
    use buiy_core::render::components::Background;

    let mut h = NodeExtractHarness::new();
    // A clipping parent (overflow: hidden) with a small box.
    let child = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(10.0)),
                    left: Sizing::Length(Length::px(10.0)),
                    ..default()
                })
                .width_px(60.0)
                .height_px(24.0),
            Background {
                color: ColorToken::SurfacePrimary,
            },
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    let clipper = h
        .app
        .world_mut()
        .spawn((
            Node,
            // `Style` is a Bundle that already carries `Overflow`; set it through
            // the builder (`overflow_hidden`) rather than spawning a second
            // `Overflow` component (which would duplicate it in the bundle).
            Style::default()
                .absolute()
                .width_px(100.0)
                .height_px(60.0)
                .overflow_hidden(),
        ))
        .id();
    h.app.world_mut().entity_mut(clipper).add_children(&[child]);
    settle(&mut h);

    // Keyboard-focus the child.
    h.app
        .world_mut()
        .resource_mut::<buiy_core::FocusedEntity>()
        .0 = Some(child);
    h.app
        .world_mut()
        .resource_mut::<buiy_core::FocusVisible>()
        .0 = true;
    h.update();
    h.update();

    h.extract();
    let node = h.node_for(child).expect("child in display list");
    let outline = node.outline.expect("the keyboard-focused child has a ring");
    // The ring extends 4px (width 2 + offset 2) OUTSIDE the child box, so part
    // of it sits beyond the child's own box. The outline clip is the ANCESTOR
    // clip (the clipper's box), NOT the child's own box — so the ring survives.
    // The child's own FILL clip, by contrast, is its own box ∩ the clipper.
    let oclip = outline
        .clip
        .expect("an outline under a clipping ancestor carries that ancestor clip");
    // The outline clip is the clipper's box (100x60 at its origin), which is
    // LARGER than the child's own 60x24 box — proving it is the ancestor clip,
    // not the own-box clip that would crop the ring.
    let own_box_max = node.position + node.size;
    assert!(
        oclip.max.x > own_box_max.x || oclip.max.y > own_box_max.y,
        "the outline clip (ancestor) must exceed the child's own box, so the \
         ring outside the box is not erased — got oclip={oclip:?}, own_max={own_box_max:?}"
    );
}
