//! Wave-3 slice-3 — Disclosure widget: the P1d a11y bundle (the trigger is
//! `A11yRole::Button` + the state-keyed `A11yExpanded` + focus + a11y, with
//! `A11yRelations.controls = [panel]` and the panel a real `A11yRole::Region`)
//! plus the C4 visual (the caret rotated via the `Rotate` longhand + the panel
//! shown/hidden via `CssVisibility`, both driven by `Changed<A11yExpanded>`) and
//! pick-through (`Pickable::IGNORE` on the decorative caret/label children).
//!
//! The contract honoring (Expand/Collapse set `A11yExpanded`; Click toggles it via
//! `OnPress`) and the APG keyboard (Enter/Space activate the Button) are asserted
//! at the `buiy_core` layer (`a11y_action` / `a11y_inprocess`); here the bundle
//! shape + the `controls` relation + the C4 caret-rotates / panel-shows visual +
//! pick-through + the `OnPress`-toggles-expanded convergence are exercised.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yExpanded, A11yLabel, A11yRelations, A11yRole},
    components::Node,
    focus::Focusable,
    interaction::OnPress,
    layout::{BoxModel, Rotate},
    render::components::{Background, Border, CssVisibility},
    text::Text,
};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::disclosure::{
    Disclosure, DisclosureCaret, DisclosurePanel, caret_rotation_collapsed, caret_rotation_expanded,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    app
}

fn press(app: &mut App, entity: Entity) {
    app.world_mut().write_message(OnPress(entity));
    app.update();
}

fn child_with<C: Component>(app: &App, root: Entity) -> Entity {
    let world = app.world();
    world
        .get::<Children>(root)
        .unwrap()
        .iter()
        .find(|&c| world.get::<C>(c).is_some())
        .expect("expected child carrying the marker")
}

// ---------------------------------------------------------------------------
// The P1d bundle contract.
// ---------------------------------------------------------------------------

#[test]
fn bare_disclosure_marker_materializes_the_full_required_contract() {
    let mut app = app();
    let d = app.world_mut().spawn(Disclosure).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(d).is_some(), "Node");
    assert!(world.get::<BoxModel>(d).is_some(), "BoxModel");
    assert!(world.get::<Background>(d).is_some(), "Background");
    assert!(world.get::<Border>(d).is_some(), "Border");
    assert!(world.get::<Focusable>(d).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(d).copied(),
        Some(A11yRole::Button),
        "the trigger role is Button (expandability is state-keyed, not a new role)"
    );
    assert_eq!(
        world.get::<A11yExpanded>(d).map(|e| e.0),
        Some(false),
        "A11yExpanded present, defaulting to false (collapsed)"
    );
    assert!(world.get::<A11yLabel>(d).is_some(), "A11yLabel");
}

#[test]
fn disclosure_new_spawns_caret_label_panel_and_wires_controls() {
    let mut app = app();
    let d = app.world_mut().spawn(Disclosure::new("Details")).id();
    app.update();

    // The AT name stays on the trigger root.
    assert_eq!(
        app.world().get::<A11yLabel>(d).map(|l| l.0.clone()),
        Some("Details".to_string()),
        "the accessible name stays on the trigger root"
    );

    let children = app
        .world()
        .get::<Children>(d)
        .expect("disclosure has children")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 3, "caret + label + panel children");

    let world = app.world();
    let caret = child_with::<DisclosureCaret>(&app, d);
    let panel = child_with::<DisclosurePanel>(&app, d);

    // The decorative caret + label are Pickable::IGNORE (pick-through); the panel
    // is a real Region node, NOT pick-through (it can hold interactive content).
    assert_eq!(
        world.get::<Pickable>(caret).copied(),
        Some(Pickable::IGNORE),
        "the decorative caret carries Pickable::IGNORE"
    );
    let label = children
        .iter()
        .copied()
        .find(|&c| {
            world.get::<DisclosureCaret>(c).is_none() && world.get::<DisclosurePanel>(c).is_none()
        })
        .unwrap();
    assert_eq!(
        world.get::<Pickable>(label).copied(),
        Some(Pickable::IGNORE),
        "the decorative label carries Pickable::IGNORE"
    );
    assert_eq!(
        world.get::<Text>(label).map(|t| t.0.clone()),
        Some("Details".to_string()),
        "the label child carries the visible pixels"
    );

    // The panel is a real Region semantic node.
    assert_eq!(
        world.get::<A11yRole>(panel).copied(),
        Some(A11yRole::Region),
        "the controlled panel is an A11yRole::Region"
    );

    // The trigger's `controls` relation was wired to the panel.
    let relations = world
        .get::<A11yRelations>(d)
        .expect("the trigger carries A11yRelations after wiring");
    assert_eq!(
        relations.controls,
        vec![panel],
        "the trigger's A11yRelations.controls references the panel"
    );
}

// ---------------------------------------------------------------------------
// The C4 visual: A11yExpanded drives the caret rotation + panel visibility via
// Changed-detection.
// ---------------------------------------------------------------------------

#[test]
fn expanding_rotates_caret_and_shows_panel() {
    let mut app = app();
    let d = app.world_mut().spawn(Disclosure::new("Details")).id();
    app.update();
    let caret = child_with::<DisclosureCaret>(&app, d);
    let panel = child_with::<DisclosurePanel>(&app, d);

    // Collapsed (default): caret points right (identity), panel hidden.
    assert_eq!(
        app.world().get::<Rotate>(caret).cloned(),
        Some(caret_rotation_collapsed()),
        "collapsed ⇒ caret at the collapsed rotation (points right)"
    );
    assert_eq!(
        app.world().get::<CssVisibility>(panel).copied(),
        Some(CssVisibility::Hidden),
        "collapsed ⇒ panel hidden"
    );

    // Expand: caret rotates down, panel shows.
    app.world_mut().get_mut::<A11yExpanded>(d).unwrap().0 = true;
    app.update();
    assert_eq!(
        app.world().get::<Rotate>(caret).cloned(),
        Some(caret_rotation_expanded()),
        "expanded ⇒ caret rotated (points down)"
    );
    assert_eq!(
        app.world().get::<CssVisibility>(panel).copied(),
        Some(CssVisibility::Visible),
        "expanded ⇒ panel visible"
    );

    // Collapse again: back to right + hidden.
    app.world_mut().get_mut::<A11yExpanded>(d).unwrap().0 = false;
    app.update();
    assert_eq!(
        app.world().get::<Rotate>(caret).cloned(),
        Some(caret_rotation_collapsed()),
        "collapsed again ⇒ caret back to the collapsed rotation"
    );
    assert_eq!(
        app.world().get::<CssVisibility>(panel).copied(),
        Some(CssVisibility::Hidden),
        "collapsed again ⇒ panel hidden"
    );
}

// ---------------------------------------------------------------------------
// The OnPress toggle convergence: pointer/keyboard/AT-Click all write OnPress,
// the `advance_expanded_on_press` consumer flips A11yExpanded, and the C4 visual
// reacts — exercised end-to-end at the widget tier.
// ---------------------------------------------------------------------------

#[test]
fn on_press_toggles_expanded_and_drives_the_visual() {
    let mut app = app();
    let d = app.world_mut().spawn(Disclosure::new("Details")).id();
    app.update();
    let caret = child_with::<DisclosureCaret>(&app, d);
    let panel = child_with::<DisclosurePanel>(&app, d);

    // Press 1: collapsed → expanded.
    press(&mut app, d);
    assert_eq!(
        app.world().get::<A11yExpanded>(d).map(|e| e.0),
        Some(true),
        "first OnPress flips collapsed→expanded"
    );
    assert_eq!(
        app.world().get::<Rotate>(caret).cloned(),
        Some(caret_rotation_expanded()),
        "the caret rotated to the expanded orientation"
    );
    assert_eq!(
        app.world().get::<CssVisibility>(panel).copied(),
        Some(CssVisibility::Visible),
        "the panel shows on expand"
    );

    // Press 2: expanded → collapsed.
    press(&mut app, d);
    assert_eq!(
        app.world().get::<A11yExpanded>(d).map(|e| e.0),
        Some(false),
        "second OnPress flips expanded→collapsed"
    );
    assert_eq!(
        app.world().get::<CssVisibility>(panel).copied(),
        Some(CssVisibility::Hidden),
        "the panel hides on collapse"
    );
}

// ── Track C / C4b: DisclosureBuilder + .expanded ──────────────────────────────

/// `.expanded(true)` seeds the real `A11yExpanded`.
#[test]
fn disclosure_new_expanded_seeds_true() {
    use buiy_widgets::Disclosure;

    let mut app = app();
    let d = app
        .world_mut()
        .spawn(Disclosure::new("Details").expanded(true))
        .id();
    app.update();

    assert!(
        Disclosure::expanded(app.world().get::<A11yExpanded>(d).unwrap()),
        "`.expanded(true)` seeds A11yExpanded(true)",
    );
}
