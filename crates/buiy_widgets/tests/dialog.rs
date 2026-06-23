//! Wave-3 slice-5 — Dialog widget: the P1d a11y SHAPE (the container is
//! `A11yRole::Dialog` + `A11yModal` with `A11yRelations.labelled_by = [title]` /
//! `described_by = [body]`, the title a `Heading`, the body a `Text`) plus the
//! invoker (a `Button` with `controls = [dialog]`) and pick-through
//! (`Pickable::IGNORE` on the decorative title/body children).
//!
//! The contract honoring (the invoker's Click → OnPress) and the modal/labelling
//! AccessKit translation are asserted at the `buiy_core` layer (`a11y_action` /
//! `a11y_inprocess` / `a11y_translate`); here the bundle SHAPE + the labelling
//! relations + the invoker controls relation + pick-through are exercised. There
//! is NO open/close/focus-trap (C5, Wave 4) — only the static a11y shape.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    CorePlugin, FocusReturn, FocusScope, FocusScopeMode,
    a11y::{A11yLabel, A11yModal, A11yRelations, A11yRole},
    components::Node,
    layout::{BoxModel, Stacking, TopLayer},
    render::components::{Background, Border, CssVisibility},
    text::Text,
};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::dialog::{Dialog, DialogBody, DialogTitle, dialog_invoker};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    app
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
fn bare_dialog_marker_materializes_the_full_required_contract() {
    let mut app = app();
    let d = app.world_mut().spawn(Dialog).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(d).is_some(), "Node");
    assert!(world.get::<BoxModel>(d).is_some(), "BoxModel");
    assert!(world.get::<Background>(d).is_some(), "Background");
    assert!(world.get::<Border>(d).is_some(), "Border");
    assert_eq!(
        world.get::<A11yRole>(d).copied(),
        Some(A11yRole::Dialog),
        "the dialog role is Dialog (plain Dialog; AlertDialog is deferred to C5)"
    );
    assert!(
        world.get::<A11yModal>(d).is_some(),
        "the dialog carries A11yModal (the modal flag)"
    );
}

#[test]
fn bare_dialog_marker_materializes_the_c5d_container_layer() {
    // C5-d: the Dialog #[require] now also carries the modal container/trap layer.
    let mut app = app();
    let d = app.world_mut().spawn(Dialog).id();
    app.update();

    let world = app.world();
    assert_eq!(
        world.get::<Stacking>(d).map(|s| s.top_layer),
        Some(TopLayer::Modal),
        "the dialog is a TopLayer::Modal (joins TopLayerActivation, paints above)"
    );
    assert_eq!(
        world.get::<FocusScope>(d).map(|s| s.mode),
        Some(FocusScopeMode::Trap),
        "the dialog carries a FocusScope::trap (the §C.1 modal trap)"
    );
    assert!(
        world.get::<FocusReturn>(d).is_some(),
        "the dialog carries FocusReturn (the §C.4 restoration target)"
    );
    assert_eq!(
        world.get::<CssVisibility>(d).copied(),
        Some(CssVisibility::Hidden),
        "the dialog starts CLOSED (CssVisibility::Hidden) — the invoker opens it"
    );
}

#[test]
fn dialog_new_spawns_title_body_and_wires_labelling() {
    let mut app = app();
    let d = app
        .world_mut()
        .spawn(Dialog::new("Delete?", "This cannot be undone."))
        .id();
    app.update();

    let children = app
        .world()
        .get::<Children>(d)
        .expect("dialog has children")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 2, "title + body children");

    let title = child_with::<DialogTitle>(&app, d);
    let body = child_with::<DialogBody>(&app, d);

    let world = app.world();
    // The title is a Heading carrying the title pixels + its label.
    assert_eq!(
        world.get::<A11yRole>(title).copied(),
        Some(A11yRole::Heading),
        "the title is an A11yRole::Heading"
    );
    assert_eq!(
        world.get::<Text>(title).map(|t| t.0.clone()),
        Some("Delete?".to_string()),
        "the title child carries the visible title pixels"
    );
    // The body is a Text carrying the body pixels.
    assert_eq!(
        world.get::<A11yRole>(body).copied(),
        Some(A11yRole::Text),
        "the body is an A11yRole::Text"
    );
    assert_eq!(
        world.get::<Text>(body).map(|t| t.0.clone()),
        Some("This cannot be undone.".to_string()),
        "the body child carries the visible body pixels"
    );

    // The decorative title/body are Pickable::IGNORE (pick-through).
    assert_eq!(
        world.get::<Pickable>(title).copied(),
        Some(Pickable::IGNORE),
        "the decorative title carries Pickable::IGNORE"
    );
    assert_eq!(
        world.get::<Pickable>(body).copied(),
        Some(Pickable::IGNORE),
        "the decorative body carries Pickable::IGNORE"
    );

    // The labelling relations were wired to the title/body.
    let relations = world
        .get::<A11yRelations>(d)
        .expect("the dialog carries A11yRelations after wiring");
    assert_eq!(
        relations.labelled_by,
        vec![title],
        "the dialog's A11yRelations.labelled_by references the title"
    );
    assert_eq!(
        relations.described_by,
        vec![body],
        "the dialog's A11yRelations.described_by references the body"
    );
}

// ---------------------------------------------------------------------------
// The invoker: a Button + controls = [dialog].
// ---------------------------------------------------------------------------

#[test]
fn dialog_invoker_is_a_button_that_controls_the_dialog() {
    let mut app = app();
    let dialog = app.world_mut().spawn(Dialog::new("Title", "Body")).id();
    let invoker = app.world_mut().spawn(dialog_invoker("Open", dialog)).id();
    app.update();

    let world = app.world();
    // The invoker rides the full Button contract (role Button + focus).
    assert_eq!(
        world.get::<A11yRole>(invoker).copied(),
        Some(A11yRole::Button),
        "the invoker is an A11yRole::Button"
    );
    assert!(
        world.get::<buiy_core::focus::Focusable>(invoker).is_some(),
        "the invoker is focusable (the Button contract)"
    );
    assert_eq!(
        world.get::<A11yLabel>(invoker).map(|l| l.0.clone()),
        Some("Open".to_string()),
        "the invoker carries its label"
    );
    // The invoker controls the dialog.
    assert_eq!(
        world
            .get::<A11yRelations>(invoker)
            .map(|r| r.controls.clone()),
        Some(vec![dialog]),
        "the invoker's A11yRelations.controls references the dialog"
    );
}
