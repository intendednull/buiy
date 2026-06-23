//! Wave-3 slice-1 — Checkbox widget: the P1d a11y bundle (role + tri-state
//! `A11yToggled` + focus + a11y) plus the C4 visual (the check/dash mark driven
//! by `Changed<A11yToggled>`) and pick-through (`Pickable::IGNORE` on the
//! decorative children).
//!
//! The keyboard asymmetry (Checkbox = Space-only; Enter inert) is asserted at the
//! `buiy_core` keyboard layer (`a11y_action.rs`); here the end-to-end activation
//! convergence is exercised: an `OnPress` (whatever the producer) advances the
//! checkbox's `A11yToggled`, and the visual repaints off the `Changed` gate.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yLabel, A11yRole, A11yToggled, Toggled},
    components::Node,
    focus::Focusable,
    interaction::OnPress,
    layout::BoxModel,
    render::components::{Background, Border, CssVisibility},
    text::Text,
};
use buiy_widgets::checkbox::{CHECK_GLYPH, Checkbox, CheckboxMark, DASH_GLYPH};
use buiy_widgets::{OnPress as ReexportedOnPress, WidgetsPlugin};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    app
}

/// Spawn an `OnPress(entity)` directly (modelling any producer — pointer,
/// keyboard, AT) and run one frame so `advance_toggle_on_press` + the visual run.
fn press(app: &mut App, entity: Entity) {
    app.world_mut().write_message(OnPress(entity));
    app.update();
}

// ---------------------------------------------------------------------------
// The P1d bundle contract.
// ---------------------------------------------------------------------------

#[test]
fn bare_checkbox_marker_materializes_the_full_required_contract() {
    let mut app = app();
    let cb = app.world_mut().spawn(Checkbox).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(cb).is_some(), "Node");
    assert!(world.get::<BoxModel>(cb).is_some(), "BoxModel");
    assert!(world.get::<Background>(cb).is_some(), "Background");
    assert!(world.get::<Border>(cb).is_some(), "Border");
    assert!(world.get::<Focusable>(cb).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(cb).copied(),
        Some(A11yRole::Checkbox),
        "role defaults to Checkbox"
    );
    // The tri-state toggle exists and defaults to unchecked (False).
    assert_eq!(
        world.get::<A11yToggled>(cb).map(|t| t.0),
        Some(Toggled::False),
        "A11yToggled present, defaulting to False (unchecked)"
    );
    assert!(world.get::<A11yLabel>(cb).is_some(), "A11yLabel");
}

#[test]
fn checkbox_new_spawns_label_and_mark_children_pick_through() {
    // The visible label is a CHILD Text (the AT name stays `A11yLabel` on the
    // root — the C4 decoupling); the mark + label are `Pickable::IGNORE`.
    let mut app = app();
    let cb = app.world_mut().spawn(Checkbox::new("Done")).id();
    app.update();

    // The AT name is on the root.
    assert_eq!(
        app.world().get::<A11yLabel>(cb).map(|l| l.0.clone()),
        Some("Done".to_string()),
        "the accessible name stays on the widget root"
    );

    let children = app
        .world()
        .get::<Children>(cb)
        .expect("checkbox has children")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 2, "mark + label children");

    // Every decorative child is Pickable::IGNORE (pick-through, co-drive SC-3) so
    // a hit resolves to the widget root the router addresses.
    for &child in &children {
        assert_eq!(
            app.world().get::<Pickable>(child).copied(),
            Some(Pickable::IGNORE),
            "decorative child carries Pickable::IGNORE"
        );
    }

    // Exactly one child is the CheckboxMark; the other carries the label pixels.
    let world = app.world();
    let mark = children
        .iter()
        .copied()
        .find(|&c| world.get::<CheckboxMark>(c).is_some())
        .expect("a CheckboxMark child");
    let label = children
        .iter()
        .copied()
        .find(|&c| world.get::<CheckboxMark>(c).is_none())
        .expect("a label child");
    assert_eq!(
        world.get::<Text>(label).map(|t| t.0.clone()),
        Some("Done".to_string()),
        "the label child carries the visible pixels"
    );
    // The mark starts hidden (default toggle is False).
    assert_eq!(
        world.get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Hidden),
        "the mark starts hidden (unchecked)"
    );
}

// ---------------------------------------------------------------------------
// The C4 visual: A11yToggled drives the mark via Changed-detection.
// ---------------------------------------------------------------------------

#[test]
fn toggling_a11y_toggled_drives_the_mark_visual() {
    let mut app = app();
    let cb = app.world_mut().spawn(Checkbox::new("Done")).id();
    app.update();

    let mark = {
        let world = app.world();
        world
            .get::<Children>(cb)
            .unwrap()
            .iter()
            .find(|&c| world.get::<CheckboxMark>(c).is_some())
            .unwrap()
    };

    // False ⇒ hidden.
    assert_eq!(
        app.world().get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Hidden),
        "False ⇒ mark hidden"
    );

    // Flip to True ⇒ the check glyph is shown.
    app.world_mut().get_mut::<A11yToggled>(cb).unwrap().0 = Toggled::True;
    app.update();
    assert_eq!(
        app.world().get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Visible),
        "True ⇒ mark visible"
    );
    assert_eq!(
        app.world().get::<Text>(mark).map(|t| t.0.clone()),
        Some(CHECK_GLYPH.to_string()),
        "True ⇒ the check glyph"
    );

    // Flip to Mixed ⇒ the dash glyph, still visible.
    app.world_mut().get_mut::<A11yToggled>(cb).unwrap().0 = Toggled::Mixed;
    app.update();
    assert_eq!(
        app.world().get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Visible),
        "Mixed ⇒ mark visible"
    );
    assert_eq!(
        app.world().get::<Text>(mark).map(|t| t.0.clone()),
        Some(DASH_GLYPH.to_string()),
        "Mixed ⇒ the dash glyph"
    );

    // Back to False ⇒ hidden again.
    app.world_mut().get_mut::<A11yToggled>(cb).unwrap().0 = Toggled::False;
    app.update();
    assert_eq!(
        app.world().get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Hidden),
        "False ⇒ mark hidden again"
    );
}

// ---------------------------------------------------------------------------
// The activation convergence: OnPress (any producer) advances the tri-state
// toggle through the single consumer, and the visual repaints.
// ---------------------------------------------------------------------------

#[test]
fn on_press_advances_checkbox_tristate_and_repaints() {
    let mut app = app();
    let cb = app.world_mut().spawn(Checkbox::new("Done")).id();
    app.update();

    let mark = {
        let world = app.world();
        world
            .get::<Children>(cb)
            .unwrap()
            .iter()
            .find(|&c| world.get::<CheckboxMark>(c).is_some())
            .unwrap()
    };

    // Press 1: False → True (the check shows).
    press(&mut app, cb);
    assert_eq!(
        app.world().get::<A11yToggled>(cb).map(|t| t.0),
        Some(Toggled::True),
        "first OnPress advances False→True"
    );
    assert_eq!(
        app.world().get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Visible),
        "the mark repainted to visible off the Changed gate"
    );

    // Press 2: True → False.
    press(&mut app, cb);
    assert_eq!(
        app.world().get::<A11yToggled>(cb).map(|t| t.0),
        Some(Toggled::False),
        "second OnPress advances True→False"
    );
    assert_eq!(
        app.world().get::<CssVisibility>(mark).copied(),
        Some(CssVisibility::Hidden),
        "the mark repainted back to hidden"
    );
}

#[test]
fn on_press_resolves_mixed_to_false() {
    // APG: activating a Mixed (indeterminate) checkbox sets it unchecked (False).
    let mut app = app();
    let cb = app.world_mut().spawn(Checkbox::new("Tri")).id();
    app.world_mut().get_mut::<A11yToggled>(cb).unwrap().0 = Toggled::Mixed;
    app.update();

    press(&mut app, cb);
    assert_eq!(
        app.world().get::<A11yToggled>(cb).map(|t| t.0),
        Some(Toggled::False),
        "OnPress on a Mixed checkbox resolves to False (APG)"
    );
}

#[test]
fn reexport_on_press_is_the_core_sink() {
    // The `buiy_widgets::OnPress` re-export is the same type as the core sink
    // (co-drive SC-1 source-compat).
    fn _same(a: ReexportedOnPress) -> OnPress {
        a
    }
}

/// The pointer convergence (co-drive SC-1, grounding loop 2): a REAL synthetic
/// `Pointer<Click>` over the checkbox root lowers through the production pointer
/// producer (`pointer_click_emits_on_press`, whose role gate now includes
/// Checkbox) into the shared `OnPress` sink, and the same `advance_toggle_on_press`
/// consumer advances the tri-state toggle — exactly as the keyboard/AT paths do.
/// Pointer + AT + keyboard all converge on the one toggle advance.
#[test]
fn synthetic_pointer_click_flips_checkbox_toggled() {
    use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
    use bevy::picking::pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
    };
    use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};
    use buiy_core::ResolvedLayout;
    use buiy_core::picking::{BuiyPickingBackendPlugin, PickingPlugin};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(CorePlugin);
    app.add_plugins(PickingPlugin);
    app.add_plugins(BuiyPickingBackendPlugin);
    app.add_plugins(WidgetsPlugin);

    let window = app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(800, 600),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    app.world_mut()
        .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));

    // A checkbox at an absolute box (0,0)..(18,18). Hand-give it the absolute
    // basis `emit_picks` reads so the synthetic pointer hits it.
    let cb = app.world_mut().spawn(Checkbox::new("Done")).id();
    app.world_mut().entity_mut(cb).insert((
        ResolvedLayout {
            position: Vec2::ZERO,
            size: Vec2::new(18.0, 18.0),
        },
        GlobalTransform::IDENTITY,
    ));

    let target = WindowRef::Entity(window).normalize(Some(window)).unwrap();
    let location = Location {
        target: NormalizedRenderTarget::Window(target),
        position: Vec2::new(9.0, 9.0),
    };
    app.world_mut()
        .spawn((PointerId::Mouse, PointerLocation::new(location.clone())));
    app.update();

    for action in [
        PointerAction::Press(PointerButton::Primary),
        PointerAction::Release(PointerButton::Primary),
    ] {
        app.world_mut().write_message(PointerInput {
            pointer_id: PointerId::Mouse,
            location: location.clone(),
            action,
        });
        app.update();
    }

    // The synthetic Pointer<Click> → OnPress → consumer advanced False→True.
    assert_eq!(
        app.world().get::<A11yToggled>(cb).map(|t| t.0),
        Some(Toggled::True),
        "a synthetic pointer click flips the checkbox (pointer converges with AT/keyboard)"
    );
}
