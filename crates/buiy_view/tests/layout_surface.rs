//! Headless **layout-surface** verification (spec §2.2, no GPU) — one
//! layout-snapshot assertion per modifier: the reconciler lowers each view intent
//! to the right decomposed `buiy_core::layout` component (resolved geometry where
//! it is the clearest witness). Mirrors `styling.rs`; geometry is observable
//! pre-raster, so F2 needs no goldens (spec §4.1).

mod common;

use bevy::prelude::*;
use buiy_core::ResolvedLayout;
use buiy_core::layout::{
    AlignItems, BoxModel, FlexItem, FlexParams, FlexWrap, JustifyContent, Length, Overflow,
    OverflowMode, Position, PositionKind, Rotate, ScrollOffset, Sizing, Stacking, TopLayer,
};
use buiy_core::mvu::{Cmd, Model};
use buiy_core::text::TextAlign as CoreTextAlign;
use buiy_view::{
    BuiyViewAppExt, Element, Kind, TextAlign, column, entities_of_kind, find_kind, row, text,
};

/// A trivial model — the view is a pure function of `()`, so each test just picks
/// a different `view` fn.
#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Probe;
impl Model for Probe {
    type Msg = Noop;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
struct Noop;

fn update(_: &mut Probe, _: Noop) -> Cmd<Noop> {
    Cmd::none()
}

fn app_with(view: fn(&Probe) -> Element<Noop>) -> App {
    let mut app = common::logic_app();
    app.ui(Probe, update, view);
    common::settle(&mut app);
    app
}

fn px(v: f32) -> Sizing {
    Sizing::Length(Length::Px(v))
}

fn root(app: &mut App) -> Entity {
    find_kind(app.world_mut(), Kind::Column)
        .or_else(|| find_kind(app.world_mut(), Kind::Row))
        .expect("a root container exists")
}

/// Find a realized `Text` node by its content (robust: `entities_of_kind` order
/// is NOT spawn order across archetypes, e.g. when only some siblings carry a
/// `FlexItem`).
fn text_node(app: &mut App, content: &str) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &buiy_core::text::Text)>();
    q.iter(world)
        .find(|(_, t)| t.0 == content)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("a text node {content:?} exists"))
}

// ---------------------------------------------------------------------------
// Sizing.
// ---------------------------------------------------------------------------

fn v_sizing(_: &Probe) -> Element<Noop> {
    column![text("x")]
        .width(120.0)
        .height(80.0)
        .min_width(40.0)
        .min_height(30.0)
        .max_width(400.0)
        .max_height(300.0)
}

#[test]
fn sizing_lowers_to_box_model() {
    let mut app = app_with(v_sizing);
    let e = root(&mut app);
    let bm = app.world().get::<BoxModel>(e).unwrap();
    assert_eq!(bm.width, px(120.0), "width");
    assert_eq!(bm.height, px(80.0), "height");
    assert_eq!(bm.min_width, px(40.0), "min_width");
    assert_eq!(bm.min_height, px(30.0), "min_height");
    assert_eq!(bm.max_width, px(400.0), "max_width");
    assert_eq!(bm.max_height, px(300.0), "max_height");
}

fn v_fill(_: &Probe) -> Element<Noop> {
    column![text("x")].fill()
}

#[test]
fn fill_lowers_to_percent_and_spans_viewport() {
    let mut app = app_with(v_fill);
    let e = root(&mut app);
    let bm = app.world().get::<BoxModel>(e).unwrap();
    assert_eq!(
        bm.width,
        Sizing::Length(Length::Percent(100.0)),
        "fill width"
    );
    assert_eq!(
        bm.height,
        Sizing::Length(Length::Percent(100.0)),
        "fill height"
    );
    // The root's containing block is the viewport (800×600 under MinimalPlugins),
    // so `.fill()` spans the window — the observable geometry witness.
    let rl = app.world().get::<ResolvedLayout>(e).unwrap();
    assert_eq!(rl.size, Vec2::new(800.0, 600.0), "fill spans the viewport");
}

fn v_fill_axes(_: &Probe) -> Element<Noop> {
    // A row of two children so per-axis fill is distinguishable: the first fills
    // width only, the second height only.
    row![
        column![text("w")].fill_width(),
        column![text("h")].fill_height(),
    ]
}

#[test]
fn fill_width_and_fill_height_are_per_axis() {
    let mut app = app_with(v_fill_axes);
    let cols = entities_of_kind(app.world_mut(), Kind::Column);
    let (w, h) = (cols[0], cols[1]);
    let bw = app.world().get::<BoxModel>(w).unwrap();
    assert_eq!(
        bw.width,
        Sizing::Length(Length::Percent(100.0)),
        "fill_width w"
    );
    assert_eq!(bw.height, Sizing::Auto, "fill_width leaves height auto");
    let bh = app.world().get::<BoxModel>(h).unwrap();
    assert_eq!(bh.width, Sizing::Auto, "fill_height leaves width auto");
    assert_eq!(
        bh.height,
        Sizing::Length(Length::Percent(100.0)),
        "fill_height h"
    );
}

// ---------------------------------------------------------------------------
// Flex item (grow / shrink, inserted on demand).
// ---------------------------------------------------------------------------

fn v_flex_item(_: &Probe) -> Element<Noop> {
    row![
        text("g").grow_by(2.0),
        text("s").shrink(false),
        text("plain"),
    ]
}

#[test]
fn grow_and_shrink_insert_flex_item_on_demand() {
    let mut app = app_with(v_flex_item);
    let g = text_node(&mut app, "g");
    let s = text_node(&mut app, "s");
    let plain = text_node(&mut app, "plain");
    let fg = app
        .world()
        .get::<FlexItem>(g)
        .expect("grow inserts FlexItem");
    assert_eq!(fg.grow, 2.0, "grow_by(2.0)");
    assert_eq!(fg.shrink, 1.0, "grow leaves the shrink default");
    let fs = app
        .world()
        .get::<FlexItem>(s)
        .expect("shrink(false) inserts FlexItem");
    assert_eq!(fs.shrink, 0.0, "shrink(false) pins shrink to 0");
    assert!(
        app.world().get::<FlexItem>(plain).is_none(),
        "a plain child gets NO FlexItem (neutral grow/shrink, insert-on-demand)"
    );
}

// ---------------------------------------------------------------------------
// Flex container: justify (6) + align (4) + wrap.
// ---------------------------------------------------------------------------

fn v_justify(_: &Probe) -> Element<Noop> {
    column![
        row![text("a")].justify_start(),
        row![text("a")].justify_center(),
        row![text("a")].justify_end(),
        row![text("a")].justify_between(),
        row![text("a")].justify_around(),
        row![text("a")].justify_evenly(),
    ]
}

#[test]
fn justify_facade_lowers_all_six() {
    let mut app = app_with(v_justify);
    let rows = entities_of_kind(app.world_mut(), Kind::Row);
    let want = [
        JustifyContent::FlexStart,
        JustifyContent::Center,
        JustifyContent::FlexEnd,
        JustifyContent::SpaceBetween,
        JustifyContent::SpaceAround,
        JustifyContent::SpaceEvenly,
    ];
    for (i, w) in want.iter().enumerate() {
        let fp = app.world().get::<FlexParams>(rows[i]).unwrap();
        assert_eq!(fp.justify_content, *w, "justify row {i}");
    }
}

fn v_align(_: &Probe) -> Element<Noop> {
    column![
        row![text("a")].align_start(),
        row![text("a")].align_center(),
        row![text("a")].align_end(),
        row![text("a")].align_stretch(),
    ]
}

#[test]
fn align_facade_lowers_all_four() {
    let mut app = app_with(v_align);
    let rows = entities_of_kind(app.world_mut(), Kind::Row);
    let want = [
        AlignItems::FlexStart,
        AlignItems::Center,
        AlignItems::FlexEnd,
        AlignItems::Stretch,
    ];
    for (i, w) in want.iter().enumerate() {
        let fp = app.world().get::<FlexParams>(rows[i]).unwrap();
        assert_eq!(fp.align_items, *w, "align row {i}");
    }
}

fn v_wrap(_: &Probe) -> Element<Noop> {
    row![text("a"), text("b")].wrap()
}

#[test]
fn wrap_lowers_to_flex_wrap() {
    let mut app = app_with(v_wrap);
    let e = root(&mut app);
    let fp = app.world().get::<FlexParams>(e).unwrap();
    assert_eq!(fp.wrap, FlexWrap::Wrap, "wrap()");
}

// ---------------------------------------------------------------------------
// Spacing (per-side padding + xy + gap).
// ---------------------------------------------------------------------------

fn v_padding_sides(_: &Probe) -> Element<Noop> {
    use buiy_view::Space;
    column![text("x")]
        .padding_top(Space::Md)
        .padding_left(Space::Sm)
}

#[test]
fn per_side_padding_lowers_unset_sides_to_zero() {
    let mut app = app_with(v_padding_sides);
    let e = root(&mut app);
    let bm = app.world().get::<BoxModel>(e).unwrap();
    assert_eq!(bm.padding.top, Length::Px(16.0), "padding_top Md");
    assert_eq!(bm.padding.left, Length::Px(8.0), "padding_left Sm");
    assert_eq!(bm.padding.right, Length::Px(0.0), "unset right → 0");
    assert_eq!(bm.padding.bottom, Length::Px(0.0), "unset bottom → 0");
}

fn v_padding_xy(_: &Probe) -> Element<Noop> {
    use buiy_view::Space;
    column![text("x")].padding_xy(Space::Lg, Space::Sm)
}

#[test]
fn padding_xy_splits_horizontal_and_vertical() {
    let mut app = app_with(v_padding_xy);
    let e = root(&mut app);
    let bm = app.world().get::<BoxModel>(e).unwrap();
    assert_eq!(bm.padding.left, Length::Px(24.0), "horizontal Lg");
    assert_eq!(bm.padding.right, Length::Px(24.0), "horizontal Lg");
    assert_eq!(bm.padding.top, Length::Px(8.0), "vertical Sm");
    assert_eq!(bm.padding.bottom, Length::Px(8.0), "vertical Sm");
}

fn v_gap(_: &Probe) -> Element<Noop> {
    use buiy_view::Space;
    column![text("a"), text("b")].gap(Space::Md)
}

#[test]
fn gap_lowers_to_flex_gap() {
    let mut app = app_with(v_gap);
    let e = root(&mut app);
    let fp = app.world().get::<FlexParams>(e).unwrap();
    assert_eq!(fp.gap.row, Length::Px(16.0), "gap Md");
}

// ---------------------------------------------------------------------------
// Text alignment.
// ---------------------------------------------------------------------------

fn v_text_align(_: &Probe) -> Element<Noop> {
    column![text("centered").text_align(TextAlign::Center)]
}

#[test]
fn text_align_lowers_onto_the_text_node() {
    let mut app = app_with(v_text_align);
    let t = find_kind(app.world_mut(), Kind::Text).unwrap();
    assert_eq!(
        app.world().get::<CoreTextAlign>(t).copied(),
        Some(CoreTextAlign::Center),
        "text_align(Center) → the Text node's TextAlign"
    );
}

// ---------------------------------------------------------------------------
// Positioning: top_layer / fixed (viewport anchor) / absolute+inset / relative /
// center_self.
// ---------------------------------------------------------------------------

fn v_top_layer(_: &Probe) -> Element<Noop> {
    column![text("x")].top_layer()
}

#[test]
fn top_layer_lowers_to_popover() {
    let mut app = app_with(v_top_layer);
    let e = root(&mut app);
    assert_eq!(
        app.world().get::<Stacking>(e).unwrap().top_layer,
        TopLayer::Popover
    );
}

fn v_fixed_anchor(_: &Probe) -> Element<Noop> {
    // A padded, viewport-filling root with a bare `.fixed()` child: the child must
    // resolve to the viewport origin (0,0) DESPITE the root's padding — the
    // `.fixed()` viewport-anchor requirement (spec §2.2 finding #4).
    use buiy_view::Space;
    column![column![text("scrim")].fixed().width(50.0).height(50.0)]
        .fill()
        .padding(Space::Xl)
}

#[test]
fn fixed_resolves_to_viewport_origin_regardless_of_root_padding() {
    let mut app = app_with(v_fixed_anchor);
    // The fixed child is the inner column (the root is the outer, padded one).
    let cols = entities_of_kind(app.world_mut(), Kind::Column);
    let fixed_child = cols
        .iter()
        .copied()
        .find(|&e| app.world().get::<Position>(e).map(|p| p.kind) == Some(PositionKind::Fixed))
        .expect("the fixed child exists");
    let rl = app.world().get::<ResolvedLayout>(fixed_child).unwrap();
    assert_eq!(
        rl.position,
        Vec2::ZERO,
        "`.fixed()` anchors to the viewport origin (0,0), NOT the root's 32px \
         padded content origin"
    );
    assert_eq!(
        rl.size,
        Vec2::new(50.0, 50.0),
        "the fixed scrim keeps its size"
    );
}

fn v_absolute_inset(_: &Probe) -> Element<Noop> {
    // A relative card with an absolute corner badge (top-right).
    column![
        column![text("badge")]
            .absolute()
            .inset_top(6.0)
            .inset_right(8.0)
    ]
    .relative()
    .width(200.0)
    .height(120.0)
}

#[test]
fn absolute_inset_and_relative_lower_correctly() {
    let mut app = app_with(v_absolute_inset);
    let cols = entities_of_kind(app.world_mut(), Kind::Column);
    // The outer (relative) card and the inner (absolute) badge.
    let card = cols
        .iter()
        .copied()
        .find(|&e| app.world().get::<Position>(e).map(|p| p.kind) == Some(PositionKind::Relative))
        .expect("relative card");
    let badge = cols
        .iter()
        .copied()
        .find(|&e| app.world().get::<Position>(e).map(|p| p.kind) == Some(PositionKind::Absolute))
        .expect("absolute badge");
    let card_pos = app.world().get::<Position>(card).unwrap();
    assert_eq!(card_pos.kind, PositionKind::Relative, "relative()");
    let badge_pos = app.world().get::<Position>(badge).unwrap();
    assert_eq!(badge_pos.kind, PositionKind::Absolute, "absolute()");
    assert_eq!(badge_pos.inset.top, px(6.0), "inset_top");
    assert_eq!(badge_pos.inset.right, px(8.0), "inset_right");
    // The unset LEFT side stays auto (a right-anchored badge), not defaulted to 0.
    assert_eq!(
        badge_pos.inset.left,
        Sizing::Auto,
        "unset left stays auto (right pin)"
    );
}

fn v_center_self(_: &Probe) -> Element<Noop> {
    column![text("modal")]
        .width(200.0)
        .height(100.0)
        .center_self()
}

#[test]
fn center_self_lowers_to_inset_50_plus_half_size_margin() {
    let mut app = app_with(v_center_self);
    let e = root(&mut app);
    let pos = app.world().get::<Position>(e).unwrap();
    assert_eq!(
        pos.kind,
        PositionKind::Absolute,
        "center_self implies absolute"
    );
    assert_eq!(
        pos.inset.top,
        Sizing::Length(Length::Percent(50.0)),
        "inset top 50%"
    );
    assert_eq!(
        pos.inset.left,
        Sizing::Length(Length::Percent(50.0)),
        "inset left 50%"
    );
    let bm = app.world().get::<BoxModel>(e).unwrap();
    assert_eq!(bm.margin.top, Length::Px(-50.0), "margin top -height/2");
    assert_eq!(bm.margin.left, Length::Px(-100.0), "margin left -width/2");
}

// ---------------------------------------------------------------------------
// Scroll.
// ---------------------------------------------------------------------------

fn v_scroll(_: &Probe) -> Element<Noop> {
    row![column![text("y")].scroll_y(), column![text("x")].scroll_x(),]
}

#[test]
fn scroll_lowers_overflow_and_inserts_the_runtime_bundle() {
    let mut app = app_with(v_scroll);
    let cols = entities_of_kind(app.world_mut(), Kind::Column);
    let (y, x) = (cols[0], cols[1]);
    let oy = app.world().get::<Overflow>(y).unwrap();
    assert_eq!(oy.y, OverflowMode::Scroll, "scroll_y → overflow-y");
    assert_eq!(oy.x, OverflowMode::Visible, "scroll_y leaves x visible");
    let ox = app.world().get::<Overflow>(x).unwrap();
    assert_eq!(ox.x, OverflowMode::Scroll, "scroll_x → overflow-x");
    // The opt-in runtime scroll bundle is inserted on a scroll container.
    assert!(
        app.world().get::<ScrollOffset>(y).is_some(),
        "scroll container gets a ScrollOffset (the runtime bundle)"
    );
}

// ---------------------------------------------------------------------------
// Byte-stability: a container with NO layout modifier lowers to the layout
// defaults (no drift) — the spec §2.2 design principle that keeps existing
// snapshots byte-identical.
// ---------------------------------------------------------------------------

fn v_plain(_: &Probe) -> Element<Noop> {
    column![text("x")]
}

#[test]
fn a_plain_container_lowers_to_defaults_no_drift() {
    let mut app = app_with(v_plain);
    let e = root(&mut app);
    // Position / Overflow are `#[require]`'d; with no modifier they must equal
    // their defaults (a non-default write would move an existing snapshot).
    assert_eq!(
        *app.world().get::<Position>(e).unwrap(),
        Position::default()
    );
    assert_eq!(
        *app.world().get::<Overflow>(e).unwrap(),
        Overflow::default()
    );
    assert_eq!(
        app.world().get::<Stacking>(e).unwrap().top_layer,
        TopLayer::None
    );
    // No opt-in components appear for a plain container.
    assert!(app.world().get::<FlexItem>(e).is_none(), "no FlexItem");
    assert!(
        app.world().get::<ScrollOffset>(e).is_none(),
        "no ScrollOffset"
    );
    // BoxModel sizing stays Auto; padding zero.
    let bm = app.world().get::<BoxModel>(e).unwrap();
    assert_eq!(bm.width, Sizing::Auto);
    assert_eq!(bm.height, Sizing::Auto);
    assert_eq!(bm.padding, buiy_core::layout::Edges::ZERO);
}

// ---------------------------------------------------------------------------
// Composite: wrap + per-side padding + scroll in ONE realistic tree (the RUN
// proxy for this headless host — the plan's "example exercising wrap + per-side
// padding + scroll"). Proves the modifiers compose without interference.
// ---------------------------------------------------------------------------

fn v_composite(_: &Probe) -> Element<Noop> {
    use buiy_view::{Space, scroll_column};
    column![
        // A top bar with only-top padding + edge-justified content.
        row![text("Dooduel"), text("60s")]
            .padding_top(Space::Lg)
            .justify_between(),
        // A swatch toolbar that WRAPS: 5×60px swatches in a 200px row → 2 rows.
        row![
            column![].width(60.0).height(30.0),
            column![].width(60.0).height(30.0),
            column![].width(60.0).height(30.0),
            column![].width(60.0).height(30.0),
            column![].width(60.0).height(30.0),
        ]
        .wrap()
        .width(200.0),
        // A bounded scrolling chat.
        scroll_column(vec![text("a"), text("b"), text("c")]).height(50.0),
    ]
    .fill()
}

#[test]
fn wrap_padding_and_scroll_compose_in_one_tree() {
    let mut app = app_with(v_composite);
    let rows = entities_of_kind(app.world_mut(), Kind::Row);
    // The top bar: only-top padding, space-between.
    let bar = rows
        .iter()
        .copied()
        .find(|&e| app.world().get::<BoxModel>(e).unwrap().padding.top == Length::Px(24.0))
        .expect("the padded top bar");
    let bm = app.world().get::<BoxModel>(bar).unwrap();
    assert_eq!(bm.padding.top, Length::Px(24.0), "padding_top Lg");
    assert_eq!(bm.padding.bottom, Length::Px(0.0), "other sides stay 0");
    assert_eq!(
        app.world().get::<FlexParams>(bar).unwrap().justify_content,
        JustifyContent::SpaceBetween
    );
    // The wrap toolbar: FlexWrap::Wrap AND its resolved height spans 2 rows
    // (5×60px swatches in a 200px row → 2 lines of 30px = 60px tall).
    let toolbar = rows
        .iter()
        .copied()
        .find(|&e| app.world().get::<FlexParams>(e).unwrap().wrap == FlexWrap::Wrap)
        .expect("the wrap toolbar");
    let tb_h = app.world().get::<ResolvedLayout>(toolbar).unwrap().size.y;
    assert!(
        tb_h >= 60.0,
        "the toolbar wrapped to (at least) 2 rows of 30px swatches (height {tb_h} >= 60)"
    );
    // The scroll chat: the column with overflow-y scroll + the runtime bundle.
    let chat = entities_of_kind(app.world_mut(), Kind::Column)
        .into_iter()
        .find(|&e| app.world().get::<Overflow>(e).map(|o| o.y) == Some(OverflowMode::Scroll))
        .expect("the scroll chat");
    assert_eq!(
        app.world().get::<Overflow>(chat).unwrap().y,
        OverflowMode::Scroll,
        "the chat scrolls vertically"
    );
    assert!(
        app.world().get::<ScrollOffset>(chat).is_some(),
        "the scroll chat carries the runtime scroll bundle"
    );
}

// ---------------------------------------------------------------------------
// Transform: .rotate() (F4b-7).
// ---------------------------------------------------------------------------

fn v_rotate(_: &Probe) -> Element<Noop> {
    column![text("x")].rotate(90.0)
}

fn v_unrotated(_: &Probe) -> Element<Noop> {
    column![text("x")]
}

#[test]
fn rotate_lowers_to_a_z_rotation_in_radians() {
    // `.rotate(deg)` inserts a `Rotate(Quat::from_rotation_z(deg.to_radians()))`
    // on demand — the decoration transform the confetti/ribbon ride (F4b-7).
    let mut app = app_with(v_rotate);
    let root = root(&mut app);
    let r = app
        .world()
        .get::<Rotate>(root)
        .expect("a rotated node gains a Rotate transform component");
    let want = Quat::from_rotation_z(90f32.to_radians());
    // Compare components directly: `angle_between`'s acos-of-near-1 is float-noisy
    // even for equal quats, so a per-component tolerance is the robust witness.
    assert!(
        (r.0.x - want.x).abs() < 1e-4
            && (r.0.y - want.y).abs() < 1e-4
            && (r.0.z - want.z).abs() < 1e-4
            && (r.0.w - want.w).abs() < 1e-4,
        "90° lowers to a z-rotation of π/2 radians (got {:?})",
        r.0
    );
}

#[test]
fn no_rotate_modifier_inserts_no_transform() {
    // The neutral path stays byte-identical: an unrotated node carries no
    // `Rotate` component (inserted on demand only).
    let mut app = app_with(v_unrotated);
    let root = root(&mut app);
    assert!(
        app.world().get::<Rotate>(root).is_none(),
        "an unrotated node inserts no Rotate transform"
    );
}
