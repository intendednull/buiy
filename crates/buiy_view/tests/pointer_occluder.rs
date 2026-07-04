//! F6 (spec §2.7) — the **native-pointer live-interaction tier** + the
//! transparent-top-layer **occluder guard**.
//!
//! The invisible-occluder bug class (an invisible-but-pickable box that sits
//! topmost and swallows every click) shipped THREE times because a11y-probe
//! clicks and screenshots bypass pick occlusion. These tests drive REAL synthetic
//! pointer clicks through the production `bevy_picking` + Buiy backend against a
//! running `ui()` view app (`common::ViewPointer`), so they observe the exact
//! occlusion the class lives in — and assert the reconciler's auto-`Pickable::IGNORE`
//! rule makes it unwritable. The [`opaque_top_layer_overlay_occludes_the_click`]
//! test is the teeth: it proves the harness genuinely SEES occlusion, so the
//! transparent case's penetration is meaningful.
//!
//! A separate structural leg reconciles a catalog of fixtures and runs the
//! reusable Tier-3 invariant `buiy_verify::invariant::no_transparent_top_layer_occluder`
//! over each, proving no reconciled fixture re-introduces the class.

mod common;

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Model};
use buiy_verify::invariant::no_transparent_top_layer_occluder;
use buiy_view::{BuiyViewAppExt, Color, Element, Kind, button, column, text};

use common::ViewPointer;

/// A one-field model: how many times the button folded its press.
#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct M {
    hits: i32,
}
impl Model for M {
    type Msg = Msg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
enum Msg {
    Hit,
}

fn update(s: &mut M, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Hit => s.hits += 1,
    }
    Cmd::none()
}

// --- Fixture views -----------------------------------------------------------

/// The prototype bug shape: a full-viewport TRANSPARENT (no background)
/// `.fixed().fill().top_layer()` container painted OVER a button. Without the
/// auto-ignore it sits topmost and swallows the click.
fn view_transparent_overlay(_: &M) -> Element<Msg> {
    column![
        button("hit").on_press(Msg::Hit).width(200.0).height(80.0),
        column![].fixed().fill().top_layer(),
    ]
}

/// The same overlay but PAINTING a fill (opaque) — a real scrim/panel that
/// legitimately occludes.
fn view_opaque_overlay(_: &M) -> Element<Msg> {
    column![
        button("hit").on_press(Msg::Hit).width(200.0).height(80.0),
        column![]
            .fixed()
            .fill()
            .top_layer()
            .background(Color::Surface),
    ]
}

/// A PAINTED overlay explicitly opted out of picking with `.ignore_picking()`
/// (the non-transparent escape the transparent auto-rule does not cover).
fn view_ignore_painted_overlay(_: &M) -> Element<Msg> {
    column![
        button("hit").on_press(Msg::Hit).width(200.0).height(80.0),
        column![]
            .fixed()
            .fill()
            .top_layer()
            .background(Color::Surface)
            .ignore_picking(),
    ]
}

/// The theme-toggle float: a transparent positioning container that fills + tops
/// the viewport, holding an interactive child. The container is click-through
/// (auto-ignored); its child stays pickable.
fn view_toggle_float(_: &M) -> Element<Msg> {
    column![
        button("under").on_press(Msg::Hit),
        column![button("toggle").on_press(Msg::Hit)]
            .fixed()
            .fill()
            .top_layer(),
    ]
}

/// Nested transparent top-layer containers (both auto-ignored).
fn view_nested_transparent(_: &M) -> Element<Msg> {
    column![column![column![text("x")].top_layer()].top_layer()]
}

/// A control fixture with NO top layer (out of the invariant's scope).
fn view_no_top_layer(_: &M) -> Element<Msg> {
    column![column![button("b").on_press(Msg::Hit)]]
}

// --- Helpers -----------------------------------------------------------------

/// The realized button entity (the reconciler stamps `Kind::Button`).
fn button_entity(h: &ViewPointer) -> Entity {
    for e in h.world().iter_entities() {
        if e.get::<Kind>() == Some(&Kind::Button) {
            return e.id();
        }
    }
    panic!("no realized button entity");
}

/// The model's press count.
fn hits(h: &ViewPointer) -> i32 {
    for e in h.world().iter_entities() {
        if let Some(m) = e.get::<M>() {
            return m.hits;
        }
    }
    panic!("no model entity");
}

/// The single top-layer CONTAINER entity in a reconciled world.
fn top_layer_container(world: &World) -> Entity {
    use buiy_core::layout::{Stacking, TopLayer};
    for e in world.iter_entities() {
        let is_container = matches!(e.get::<Kind>(), Some(Kind::Column) | Some(Kind::Row));
        let is_top = e
            .get::<Stacking>()
            .is_some_and(|s| s.top_layer != TopLayer::None);
        if is_container && is_top {
            return e.id();
        }
    }
    panic!("no top-layer container");
}

// --- The native-pointer self-tests -------------------------------------------

/// The headline: a transparent top-layer overlay does NOT swallow the click — the
/// reconciler auto-`Pickable::IGNORE`d it, so a real pointer click penetrates to
/// the button beneath and folds its `Msg`.
#[test]
fn transparent_top_layer_overlay_does_not_swallow_the_click() {
    let mut app = common::logic_app();
    app.ui(M::default(), update, view_transparent_overlay);
    let mut h = ViewPointer::new(app);

    let button = button_entity(&h);
    assert_eq!(hits(&h), 0, "no press before the click");

    h.click(button);
    assert_eq!(
        hits(&h),
        1,
        "the click penetrated the transparent top-layer overlay to the button (auto-IGNORE)",
    );
}

/// The teeth: an OPAQUE top-layer overlay paints a fill, is NOT auto-ignored, and
/// correctly occludes — the click is absorbed and the button never folds. This
/// proves the harness genuinely observes pick occlusion (an a11y/probe click would
/// activate the button regardless, missing the whole class).
#[test]
fn opaque_top_layer_overlay_occludes_the_click() {
    let mut app = common::logic_app();
    app.ui(M::default(), update, view_opaque_overlay);
    let mut h = ViewPointer::new(app);

    let button = button_entity(&h);
    h.click(button);
    assert_eq!(
        hits(&h),
        0,
        "an opaque top-layer overlay occludes the click — the harness sees occlusion",
    );
}

/// The explicit escape: `.ignore_picking()` on a PAINTED overlay makes it
/// click-through, so the click reaches the button (the case the transparent
/// auto-rule cannot cover, since the overlay paints a fill).
#[test]
fn ignore_picking_passes_the_click_through_a_painted_overlay() {
    let mut app = common::logic_app();
    app.ui(M::default(), update, view_ignore_painted_overlay);
    let mut h = ViewPointer::new(app);

    let button = button_entity(&h);
    h.click(button);
    assert_eq!(
        hits(&h),
        1,
        ".ignore_picking() makes a painted top-layer overlay click-through to the button",
    );
}

// --- The structural (auto-ignore) leg ----------------------------------------

/// Pin the reconciler mechanics directly (no picking needed): a transparent
/// top-layer container is auto-`Pickable::IGNORE`d; an opaque one is not.
#[test]
fn transparent_top_layer_container_is_auto_ignored_opaque_is_not() {
    use bevy::picking::Pickable;

    let mut app = common::logic_app();
    app.ui(M::default(), update, view_transparent_overlay);
    common::settle(&mut app);
    let overlay = top_layer_container(app.world());
    assert_eq!(
        app.world().get::<Pickable>(overlay).copied(),
        Some(Pickable::IGNORE),
        "a transparent top-layer container is auto-Pickable::IGNORE",
    );

    let mut app = common::logic_app();
    app.ui(M::default(), update, view_opaque_overlay);
    common::settle(&mut app);
    let overlay = top_layer_container(app.world());
    assert_ne!(
        app.world().get::<Pickable>(overlay).copied(),
        Some(Pickable::IGNORE),
        "an opaque top-layer container is NOT auto-ignored (it paints a fill)",
    );
}

/// The Tier-3 invariant over the view fixture catalog: after reconcile, NO
/// fixture leaves a transparent top-layer occluder. Fails the moment a fixture
/// (re)introduces the class — independent of anyone remembering to click it.
#[test]
fn no_reconciled_fixture_is_a_transparent_top_layer_occluder() {
    let catalog: [fn(&M) -> Element<Msg>; 6] = [
        view_transparent_overlay,
        view_opaque_overlay,
        view_ignore_painted_overlay,
        view_toggle_float,
        view_nested_transparent,
        view_no_top_layer,
    ];
    for view in catalog {
        let mut app = common::logic_app();
        app.ui(M::default(), update, view);
        common::settle(&mut app);
        no_transparent_top_layer_occluder(app.world()).unwrap_or_else(|v| {
            panic!("a reconciled fixture re-introduced the invisible-occluder class: {v}")
        });
    }
}
