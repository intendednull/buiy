//! Headless **F3 styling-surface** verification (no GPU): each new modifier
//! (`.color`/`.font`/`.weight`/`.border(w,c,style)`/`.radius_corners`/`.shadow`,
//! the `icon()` element, and the styleable-button gating) lowers to the right
//! decomposed render/text component. Component-level is the LOWEST tier that
//! observes the lowering (the borderless-rounded *fill* rasterization is the GPU
//! SDF cross-check's job); every write is drift-only, so this is also the byte-
//! stability guard for the shared-crate widgets (an unstyled button is untouched).

mod common;

use bevy::prelude::*;
use buiy_core::layout::{BoxModel, Length};
use buiy_core::mvu::{Cmd, Model};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, Border, BoxShadow, Icon, LineStyle, TextColor};
use buiy_core::text::{FontFamily, FontWeight};
use buiy_view::{
    BuiyViewAppExt, Color, Element, Kind, LineStyle as ViewLineStyle, Radius, Weight, button,
    column, icon, text,
};

// The view re-exports the render `LineStyle` verbatim; alias-assert they are the
// same type so `.border(.., ViewLineStyle::Dashed)` lowers to the render enum.
const _: fn(ViewLineStyle) -> LineStyle = |s| s;

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct M;
impl Model for M {
    type Msg = Noop;
}
#[derive(Clone, Debug, Reflect, PartialEq)]
struct Noop;
fn update(_: &mut M, _: Noop) -> Cmd<Noop> {
    Cmd::none()
}

fn app_for(view: fn(&M) -> Element<Noop>) -> App {
    let mut app = common::logic_app();
    app.ui(M, update, view);
    common::settle(&mut app);
    app
}

/// The single entity of the given kind.
fn entity_of(app: &mut App, kind: Kind) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &Kind)>();
    q.iter(world)
        .find(|(_, k)| **k == kind)
        .map(|(e, _)| e)
        .unwrap_or_else(|| panic!("a {kind:?} entity exists"))
}

// --- Custom / rgb escape + facade roles ------------------------------------

#[test]
fn custom_and_rgb_lower_to_exact_srgb() {
    assert_eq!(
        Color::rgb(10, 20, 30).to_token(),
        ColorToken::Custom(bevy::color::Color::srgba_u8(10, 20, 30, 255)),
        "rgb() is Custom with full alpha"
    );
    assert_eq!(
        Color::Custom(1, 2, 3, 4).to_token(),
        ColorToken::Custom(bevy::color::Color::srgba_u8(1, 2, 3, 4)),
    );
    // A grown protokit-role facade names a theme token (re-themes), not a literal.
    assert_eq!(Color::OnAccent.to_token(), ColorToken::TextOnAccent);
    assert_eq!(Color::Positive.to_token(), ColorToken::StatusOk);
    assert_eq!(Color::Canvas.to_token(), ColorToken::SurfaceApp);
}

// --- Text: color / font / weight -------------------------------------------

fn text_styled(_: &M) -> Element<Noop> {
    column![
        text("hi")
            .color(Color::Accent)
            .font("Caveat")
            .weight(Weight::Bold)
    ]
}

#[test]
fn text_color_font_weight_lower_to_components() {
    let mut app = app_for(text_styled);
    let t = entity_of(&mut app, Kind::Text);
    assert_eq!(
        app.world().get::<TextColor>(t).expect("TextColor").0,
        ColorToken::Accent,
    );
    let fam = app.world().get::<FontFamily>(t).expect("FontFamily");
    assert!(
        format!("{:?}", fam.0).contains("Caveat"),
        "the named family is threaded: {:?}",
        fam.0
    );
    assert_eq!(
        app.world().get::<FontWeight>(t).expect("FontWeight").0,
        700,
        "Bold → wght 700"
    );
}

// --- Container: border(style) + radius_corners + shadow --------------------

fn card(_: &M) -> Element<Noop> {
    column![text("card")]
        .background(Color::Surface)
        .border(2.0, Color::rgb(10, 12, 16), ViewLineStyle::Dashed)
        .radius_corners(4.0, 8.0, 12.0, 16.0)
        .shadow(0.0, 3.0, 8.0, 0.0, Color::rgb(0, 0, 0))
}

#[test]
fn border_style_radius_corners_and_shadow_lower() {
    let mut app = app_for(card);
    let e = entity_of(&mut app, Kind::Column);

    // Border: 4 painting sides carry the requested style (dashed is REQUESTABLE —
    // its rasterization is F4b) + the per-corner wobble radius.
    let b = app.world().get::<Border>(e).expect("Border");
    assert_eq!(
        b.top.style,
        LineStyle::Dashed,
        "dashed is requestable in F3"
    );
    assert_eq!(b.left.style, LineStyle::Dashed);
    // Per-corner radius (tl, tr, br, bl) authored directly.
    assert_eq!(b.radius.top_left.x, Length::px(4.0));
    assert_eq!(b.radius.top_right.x, Length::px(8.0));
    assert_eq!(b.radius.bottom_right.x, Length::px(12.0));
    assert_eq!(b.radius.bottom_left.x, Length::px(16.0));

    // The layout-owned border WIDTH rides BoxModel.border.
    let bm = app.world().get::<BoxModel>(e).expect("BoxModel");
    assert_eq!(bm.border.top, Length::px(2.0));

    // One outset box-shadow term.
    let sh = app.world().get::<BoxShadow>(e).expect("BoxShadow");
    assert_eq!(sh.0.len(), 1);
    assert!(!sh.0[0].inset);
    assert_eq!(sh.0[0].blur, Length::px(8.0));
}

// --- icon() element with an author viewBox ---------------------------------

fn doodle(_: &M) -> Element<Noop> {
    icon("M4 4 L20 20", 22, 1.8, 40.0)
        .color(Color::Accent)
        .background(Color::Surface)
        .radius(Radius::Full)
}

#[test]
fn icon_element_lowers_with_author_viewbox_and_badge() {
    let mut app = app_for(doodle);
    let e = entity_of(&mut app, Kind::Icon);
    let ic = app.world().get::<Icon>(e).expect("Icon");
    assert_eq!(ic.path_d, "M4 4 L20 20");
    assert_eq!(ic.size_px, 22);
    assert_eq!(ic.stroke_width, 1.8);
    assert_eq!(
        ic.viewbox, 40.0,
        "the author 40x40 viewBox rides Icon.viewbox"
    );
    assert_eq!(ic.color, ColorToken::Accent, "the stroke tint");
    // The SAME node carries the tinted badge fill + rounding (one node = badge +
    // doodle): the borderless-rounded fill (F3 render) rounds it at paint.
    assert!(app.world().get::<Background>(e).is_some(), "the badge fill");
    assert!(
        app.world().get::<Border>(e).is_some(),
        "the badge rounding (borderless — no painting side)"
    );
}

// --- styleable-button gating (the §4.1c suppression safety) -----------------

fn unstyled_button(_: &M) -> Element<Noop> {
    column![button("Save")]
}

fn styled_button(_: &M) -> Element<Noop> {
    column![
        button("Save")
            .background(Color::Accent)
            .color(Color::OnAccent)
    ]
}

/// The button's recorded label child (its `Text`).
fn button_label_child(app: &mut App, btn: Entity) -> Option<Entity> {
    let world = app.world_mut();
    let kids = world.get::<Children>(btn)?;
    let kids: Vec<Entity> = kids.iter().collect();
    kids.into_iter()
        .find(|&c| world.get::<buiy_core::text::Text>(c).is_some())
}

#[test]
fn unstyled_button_keeps_every_default() {
    let mut app = app_for(unstyled_button);
    let btn = entity_of(&mut app, Kind::Button);
    // The widget attaches its label with `TextColor::default()`. The styling
    // surface must add NOTHING to an unstyled button, so the label keeps exactly
    // that default — never a view-authored override (the §4.1c shared-crate
    // byte-stability guard that keeps the counter / gallery goldens identical).
    let label = button_label_child(&mut app, btn).expect("label child");
    assert_eq!(
        app.world().get::<TextColor>(label).expect("label color").0,
        TextColor::default().0,
        "an unstyled button's label keeps the widget default (no view override)"
    );
}

#[test]
fn styled_button_applies_fill_and_label_color() {
    let mut app = app_for(styled_button);
    let btn = entity_of(&mut app, Kind::Button);
    assert_eq!(
        app.world().get::<Background>(btn).expect("fill").color,
        ColorToken::Accent,
        "the explicit fill applies to the button entity"
    );
    let label = button_label_child(&mut app, btn).expect("label child");
    assert_eq!(
        app.world().get::<TextColor>(label).expect("label color").0,
        ColorToken::TextOnAccent,
        "the label color lowers onto the slot child"
    );
}
