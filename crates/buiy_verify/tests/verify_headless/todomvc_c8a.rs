//! C8-a — the **S1 TodoMVC inspection-driver acceptance** (widget-gallery-exemplar
//! §6 / co-drive grounding loops). The point of the slice: every TodoMVC
//! interaction branch is driven THROUGH the in-process a11y driver
//! (`buiy_core::a11y::inprocess`: `get_by_role`/`set_value`/`click`/`snapshot`/
//! `wait_for`) and asserted through the a11y tree — never by reading internal
//! state. That IS the inspection. Plus the activation grounding loop: the same
//! checkbox toggle happens via the AT driver, a synthetic keyboard Space, AND a
//! real synthetic `Pointer<Click>` — pointer + keyboard + AT converge on the one
//! `A11yToggled` advance.
//!
//! The screen + its app logic are `buiy_gallery::{spawn_todomvc_screen,
//! TodoMvcPlugin}` (pure composition over the landed P1d widgets). This test is the
//! live gate; the static layout snapshot lives in `examples/buiy_gallery/tests`.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use buiy_core::a11y::inprocess::{StateQuery, TreeView};
use buiy_core::a11y::{
    A11yPlugin, A11yRole, ActionError, SemanticTree, click, get_by_role, set_value, snapshot,
    wait_for,
};
use buiy_core::a11y::{A11yToggled, Toggled};
use buiy_core::focus::{FocusPlugin, FocusedEntity};
use buiy_core::text::BuiyTextPlugin;
use buiy_gallery::{
    DEMO_SEEDS, TodoMvcPlugin, append_row, items_left_utterance, spawn_todomvc_screen,
};
use buiy_widgets::WidgetsPlugin;

// ===========================================================================
// Harness — the headless acceptance app (the live a11y + editor + widget stack)
// ===========================================================================

/// A headless app with the a11y tree (`build_tree`), the editor + its keyboard
/// path, focus, the widget systems (the `OnPress`→toggle consumer), and the
/// gallery's `TodoMvcPlugin`. The same plugin shape the P1d driver-acceptance
/// tests use (`text_input.rs::driver_set_value_drives_the_text_input_and_a11y`),
/// plus `TodoMvcPlugin`. No window, no GPU, no winit adapter — the driver reads
/// the same canonical `A11yTreeBuilder` a real AT consumes.
fn todomvc_app(seed: bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    // `spawn_scene` resolves through the asset registry + `Assets<ScenePatch>`.
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    app.add_plugins(TodoMvcPlugin);
    // The editor's keyboard path reads `Messages<KeyboardInput>` +
    // `Res<ButtonInput<KeyCode>>` (both optional; the gate's typing/Enter legs
    // need them present). MinimalPlugins seeds neither.
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();

    // Build the screen frame; seed the demo rows imperatively if asked.
    spawn_todomvc_screen(app.world_mut());
    if seed {
        for &(label, completed) in DEMO_SEEDS {
            append_row(app.world_mut(), label, completed);
        }
    }
    // Settle so `build_tree` populates the a11y tree the driver reads.
    app.update();
    app
}

/// Count nodes of `role` in the live a11y tree (read through the driver).
fn count_role(app: &mut App, role: A11yRole) -> usize {
    snapshot(app.world_mut(), TreeView::default())
        .by_role(role)
        .count()
}

/// Whether a Checkbox node named `name` exists in the live a11y tree.
fn has_checkbox_named(tree: &SemanticTree, name: &str) -> bool {
    tree.by_role(A11yRole::Checkbox).any(|n| n.name == name)
}

/// The `ItemsLeft` Status region's announced text (read through the driver).
fn items_left(app: &mut App) -> Option<String> {
    snapshot(app.world_mut(), TreeView::default())
        .by_role(A11yRole::Status)
        .next()
        .map(|n| n.name.clone())
}

/// Send a synthetic key press (a `KeyboardInput` Pressed message) and update.
/// `apply_keyboard_edits` reads the focused editor's logical key.
fn send_key(app: &mut App, key: Key) {
    let window = app.world_mut().spawn(()).id();
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Enter, // physical code is unused by the editor's logical-key path here
        logical_key: key,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
}

// ===========================================================================
// 1. ADD — driver set_value into the field + synthetic Enter → a new row appears
// ===========================================================================

#[test]
fn driver_add_todo_via_set_value_then_enter_appends_a_row() {
    // Start with no seeded rows so the count is unambiguous.
    let mut app = todomvc_app(false);

    // Precondition through the driver: the add-field is the single TextInput,
    // and there are zero checkboxes (no rows) → "0 items left".
    let before = count_role(&mut app, A11yRole::Checkbox);
    assert_eq!(before, 0, "no rows before add (read through the a11y tree)");
    assert_eq!(
        items_left(&mut app).as_deref(),
        Some(items_left_utterance(0).as_str())
    );

    // get_by_role(TextInput) — strict single match (the add-field).
    let field = get_by_role(app.world_mut(), A11yRole::TextInput, None, None)
        .expect("exactly one TextInput (the add field)");

    // Focus the field (so the editor receives the keyboard), set its value via
    // the driver (SelectAll+Insert lowering), then commit with a synthetic Enter.
    let field_entity = buiy_core::a11y::translate::entity_for_node_id(field).unwrap();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(field_entity);
    set_value(app.world_mut(), field, "buy milk").expect("driver set_value honored");
    app.update();
    send_key(&mut app, Key::Enter); // → EditSubmitted → add_todo_on_submit appends

    // Observe the result THROUGH the driver: a new Checkbox row named "buy milk"
    // is in the tree, and the count is now "1 item left".
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert!(
        has_checkbox_named(&tree, "buy milk"),
        "the new todo row's checkbox appears in the a11y tree, named from the field text"
    );
    assert_eq!(
        items_left(&mut app).as_deref(),
        Some("1 item left"),
        "the Status live region recounts to 1 item left"
    );

    // get_by_role can now address the new row's checkbox by name (strict match).
    get_by_role(app.world_mut(), A11yRole::Checkbox, Some("buy milk"), None)
        .expect("the new row's checkbox is addressable by role+name");
}

// ===========================================================================
// 2. TOGGLE — driver click the row checkbox → A11yToggled flips + count drops
// ===========================================================================

#[test]
fn driver_click_checkbox_flips_toggled_and_decrements_count() {
    let mut app = todomvc_app(true);

    // Seeded: 3 rows, one already completed → "2 items left".
    assert_eq!(count_role(&mut app, A11yRole::Checkbox), 3);
    assert_eq!(items_left(&mut app).as_deref(), Some("2 items left"));

    // Address an active row's checkbox by name; assert it is unchecked.
    let target = get_by_role(
        app.world_mut(),
        A11yRole::Checkbox,
        Some("Compose the P1d widgets"),
        None,
    )
    .expect("the active row's checkbox");
    let pre = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        pre.node(target).unwrap().state.toggled,
        Some(Toggled::False),
        "the active row starts unchecked (read through the consumer)"
    );

    // Driver click → OnPress (the click's own snapshot is pre-advance); a driven
    // frame runs the WidgetsPlugin toggle consumer, then we re-snapshot.
    click(app.world_mut(), target).expect("AT click honored");
    app.update();
    let post = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        post.node(target).unwrap().state.toggled,
        Some(Toggled::True),
        "AT click → OnPress → toggle advanced False→True (observed in the a11y tree)"
    );

    // The count decremented (2 → 1), observed through the Status region.
    assert_eq!(
        items_left(&mut app).as_deref(),
        Some("1 item left"),
        "completing a row decrements the live count"
    );
}

// ===========================================================================
// 3. DESTROY — driver click the row's × button → the row leaves the tree
// ===========================================================================

#[test]
fn driver_click_destroy_removes_the_row_from_the_tree() {
    let mut app = todomvc_app(true);
    assert_eq!(count_role(&mut app, A11yRole::Checkbox), 3);

    // The destroy buttons are all named "×"; address by role+name+… ambiguous (3
    // rows). Instead, find the destroy button entity whose row holds the target
    // checkbox, via the driver tree's structure: a row's children are its
    // checkbox + its destroy Button. We address the destroy Button uniquely by
    // walking to the row of a uniquely-named checkbox, then its sibling Button.
    let cb = get_by_role(
        app.world_mut(),
        A11yRole::Checkbox,
        Some("Compose the P1d widgets"),
        None,
    )
    .unwrap();
    let cb_entity = buiy_core::a11y::translate::entity_for_node_id(cb).unwrap();
    let row = app
        .world()
        .get::<ChildOf>(cb_entity)
        .map(|c| c.parent())
        .expect("checkbox has a row parent");
    let destroy = app
        .world()
        .get::<Children>(row)
        .unwrap()
        .iter()
        .find(|&c| app.world().get::<buiy_widgets::Button>(c).is_some())
        .expect("the row's destroy Button");
    let destroy_node = buiy_core::a11y::translate::node_id_for(destroy);

    // Driver click the destroy button → OnPress → handle row despawns the row.
    click(app.world_mut(), destroy_node).expect("AT click on destroy honored");
    app.update();

    // Observe THROUGH the driver: the row's checkbox is gone, count drops to 2.
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert!(
        !has_checkbox_named(&tree, "Compose the P1d widgets"),
        "the destroyed row's checkbox is no longer in the a11y tree"
    );
    assert_eq!(
        count_role(&mut app, A11yRole::Checkbox),
        2,
        "two rows remain"
    );
}

// ===========================================================================
// 4. CLEAR-COMPLETED — driver click "Clear done" → done rows leave the tree
// ===========================================================================

#[test]
fn driver_clear_completed_removes_only_done_rows() {
    let mut app = todomvc_app(true);
    // Seeded: one completed ("Taste BSN authoring"), two active.
    assert_eq!(count_role(&mut app, A11yRole::Checkbox), 3);
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert!(has_checkbox_named(&tree, "Taste BSN authoring"));

    let clear = get_by_role(app.world_mut(), A11yRole::Button, Some("Clear done"), None)
        .expect("the clear-done button");
    click(app.world_mut(), clear).expect("AT click on clear honored");
    app.update();

    let tree = snapshot(app.world_mut(), TreeView::default());
    assert!(
        !has_checkbox_named(&tree, "Taste BSN authoring"),
        "the completed row was cleared"
    );
    assert_eq!(
        count_role(&mut app, A11yRole::Checkbox),
        2,
        "only the two active rows remain"
    );
}

// ===========================================================================
// 5. FILTER — driver click "Active"/"Done"/"All" → the tree prunes/restores
// ===========================================================================

#[test]
fn driver_filter_prunes_and_restores_rows_in_the_tree() {
    let mut app = todomvc_app(true);
    // Seeded: 1 completed + 2 active = 3 in the tree under the default All filter.
    assert_eq!(count_role(&mut app, A11yRole::Checkbox), 3);

    // Filter → Active: the completed row is A11yHidden-pruned from the tree.
    let active = get_by_role(app.world_mut(), A11yRole::Button, Some("Active"), None).unwrap();
    click(app.world_mut(), active).expect("filter Active honored");
    app.update();
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        tree.by_role(A11yRole::Checkbox).count(),
        2,
        "Active filter prunes the completed row from the a11y tree"
    );
    assert!(!has_checkbox_named(&tree, "Taste BSN authoring"));

    // Filter → Done: only the completed row is present (the design labels the
    // "completed" filter "Done").
    let completed = get_by_role(app.world_mut(), A11yRole::Button, Some("Done"), None).unwrap();
    click(app.world_mut(), completed).expect("filter Done honored");
    app.update();
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        tree.by_role(A11yRole::Checkbox).count(),
        1,
        "Completed filter shows only the done row"
    );
    assert!(has_checkbox_named(&tree, "Taste BSN authoring"));

    // Filter → All: every row is restored.
    let all = get_by_role(app.world_mut(), A11yRole::Button, Some("All"), None).unwrap();
    click(app.world_mut(), all).expect("filter All honored");
    app.update();
    assert_eq!(
        count_role(&mut app, A11yRole::Checkbox),
        3,
        "All filter restores every row to the tree"
    );
}

// ===========================================================================
// 6. wait_for — block on the semantic "row added" condition (no sleeps)
// ===========================================================================

#[test]
fn wait_for_observes_the_new_row_in_the_tree() {
    let mut app = todomvc_app(false);
    let field = get_by_role(app.world_mut(), A11yRole::TextInput, None, None).unwrap();
    let field_entity = buiy_core::a11y::translate::entity_for_node_id(field).unwrap();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(field_entity);
    set_value(app.world_mut(), field, "ship it").unwrap();
    app.update();
    // The Enter commit + the append happen across a frame; `wait_for` blocks a
    // real frame-loop on the semantic condition (a Checkbox named "ship it").
    send_key(&mut app, Key::Enter);
    let tree = wait_for(
        &mut app,
        |t| t.by_role(A11yRole::Checkbox).any(|n| n.name == "ship it"),
        8,
    )
    .expect("wait_for resolves once the row is in the tree");
    assert!(has_checkbox_named(&tree, "ship it"));
}

// ===========================================================================
// 7. CONVERGENCE — the SAME toggle flips via AT click, keyboard Space, and a
// REAL synthetic pointer click. Pointer + keyboard + AT converge (grounding loop).
// ===========================================================================

/// Common assertion: in `app` (a live a11y app), the checkbox addressed by
/// `name` flips False→True after `flip` drives it. Reads the result through the
/// driver snapshot, settling up to a few frames (the keyboard path's `OnPress`
/// is written by `keyboard_activation` and consumed by `advance_toggle_on_press`
/// — both in `BuiySet::Input`, unordered — so the toggle can land on the next
/// frame; `wait_for` over the a11y tree is the no-sleep way to observe it).
fn assert_flips_via(app: &mut App, name: &str, flip: impl FnOnce(&mut App, bevy::prelude::Entity)) {
    let cb = get_by_role(app.world_mut(), A11yRole::Checkbox, Some(name), None).unwrap();
    let cb_entity = buiy_core::a11y::translate::entity_for_node_id(cb).unwrap();
    assert_eq!(
        snapshot(app.world_mut(), TreeView::default())
            .node(cb)
            .unwrap()
            .state
            .toggled,
        Some(Toggled::False),
        "{name} starts unchecked"
    );
    flip(app, cb_entity);
    let tree = wait_for(
        app,
        |t| t.node(cb).and_then(|n| n.state.toggled) == Some(Toggled::True),
        4,
    )
    .unwrap_or_else(|_| panic!("{name} flipped to checked (observed through the a11y tree)"));
    assert_eq!(
        tree.node(cb).unwrap().state.toggled,
        Some(Toggled::True),
        "{name} flipped to checked (observed through the a11y tree)"
    );
}

#[test]
fn at_driver_click_flips_the_row_toggle() {
    let mut app = todomvc_app(true);
    assert_flips_via(&mut app, "Compose the P1d widgets", |app, entity| {
        let node = buiy_core::a11y::translate::node_id_for(entity);
        click(app.world_mut(), node).expect("AT click honored");
    });
}

#[test]
fn keyboard_space_flips_the_focused_row_toggle() {
    let mut app = todomvc_app(true);
    assert_flips_via(&mut app, "Compose the P1d widgets", |app, entity| {
        // Focus the checkbox, then send a Space key — the APG checkbox activation
        // (Space-only) lowers through the keyboard activation path → OnPress →
        // the toggle consumer. Convergence with the AT/pointer paths.
        app.world_mut().resource_mut::<FocusedEntity>().0 = Some(entity);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Space);
        let window = app.world_mut().spawn(()).id();
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::Space,
            logical_key: Key::Space,
            state: ButtonState::Pressed,
            text: Some(" ".into()),
            repeat: false,
            window,
        });
    });
}

/// The pointer leg uses the C7 `PointerHarness` recipe (a real synthetic
/// `PointerInput` press/release over the production picking path). The harness
/// has no `A11yPlugin`, so the toggle is asserted on the live `A11yToggled`
/// component — the same component the AT/keyboard paths flip in the a11y tree.
/// All three producers converge on the one `advance_toggle_on_press` advance.
#[test]
fn synthetic_pointer_click_flips_the_same_row_toggle() {
    use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
    use bevy::picking::pointer::{
        Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
    };
    use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};
    use buiy_core::ResolvedLayout;
    use buiy_core::picking::{BuiyPickingBackendPlugin, PickingPlugin};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(bevy::picking::PickingPlugin);
    app.add_plugins(buiy_core::CorePlugin);
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

    // Spawn a real gallery row, give its checkbox an absolute box (the basis
    // `emit_picks` reads), and aim a synthetic pointer at its center.
    spawn_todomvc_screen(app.world_mut());
    let row = append_row(app.world_mut(), "Compose the P1d widgets", false);
    let checkbox = app
        .world()
        .get::<Children>(row)
        .unwrap()
        .iter()
        .find(|&c| app.world().get::<A11yToggled>(c).is_some())
        .expect("the row's checkbox");
    app.world_mut().entity_mut(checkbox).insert((
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

    assert_eq!(
        app.world().get::<A11yToggled>(checkbox).map(|t| t.0),
        Some(Toggled::False),
        "the row starts unchecked"
    );
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
    assert_eq!(
        app.world().get::<A11yToggled>(checkbox).map(|t| t.0),
        Some(Toggled::True),
        "a REAL synthetic pointer click flips the same A11yToggled the AT/keyboard paths flip \
         (pointer + keyboard + AT converge on one toggle advance)"
    );
}

// ===========================================================================
// 8. EDIT-IN-PLACE — double-click a label → editable editor → Enter commits the
// new label, observed through the a11y tree (C8 §3.5, C3b MultiClick).
// ===========================================================================

#[test]
fn double_click_label_edits_in_place_and_commit_renames_the_row() {
    use bevy::picking::events::{Click, Pointer};
    let mut app = todomvc_app(true);

    // Find the row label entity (the checkbox's visible label child) for the
    // active row, then fire a synthetic double-click `MultiClick` at it. We emit
    // the production `MultiClick` event directly (the `PointerHarness` is the
    // pointer-path proof; here we drive the gesture the gallery observes).
    let cb = get_by_role(
        app.world_mut(),
        A11yRole::Checkbox,
        Some("Compose the P1d widgets"),
        None,
    )
    .unwrap();
    let cb_entity = buiy_core::a11y::translate::entity_for_node_id(cb).unwrap();
    let label = app
        .world()
        .get::<Children>(cb_entity)
        .unwrap()
        .iter()
        .find(|&c| {
            app.world().get::<buiy_core::text::Text>(c).is_some()
                && app
                    .world()
                    .get::<buiy_widgets::checkbox::CheckboxMark>(c)
                    .is_none()
        })
        .expect("the checkbox's label child");

    // Trigger the double-click gesture the gallery's observer listens for.
    app.world_mut().trigger(buiy_core::picking::MultiClick {
        entity: label,
        count: 2,
        button: bevy::picking::pointer::PointerButton::Primary,
    });
    app.update(); // begin_edit_in_place swaps in the editor + focuses it

    // An editor (a single-line TextInput) now exists for the edit; the row's old
    // checkbox name is still "Compose the P1d widgets" until commit. There are now
    // two TextInputs (the add-field + the in-place editor), so address the editor
    // as the focused one via a StateQuery.
    let editor = get_by_role(
        app.world_mut(),
        A11yRole::TextInput,
        None,
        Some(&StateQuery {
            focused: true,
            ..Default::default()
        }),
    )
    .expect("the focused in-place editor");

    // Drive the new label text through the driver, then commit with Enter.
    set_value(app.world_mut(), editor, "Compose ALL the widgets").expect("seed editor");
    app.update();
    send_key(&mut app, Key::Enter); // → EditSubmitted → commit_edit_in_place

    // Observe THROUGH the driver: the checkbox is renamed, the editor is gone.
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert!(
        has_checkbox_named(&tree, "Compose ALL the widgets"),
        "the row's checkbox is renamed to the edited text (read through the a11y tree)"
    );
    assert!(
        !has_checkbox_named(&tree, "Compose the P1d widgets"),
        "the old label is gone"
    );
    // Pacify the unused import in the no-edit path; the Pointer/Click types are
    // the production gesture's lineage referenced in the module doc.
    let _ = std::marker::PhantomData::<Pointer<Click>>;
}

/// Sanity: an unsupported verb on the screen is a typed `Err`, never a panic
/// (the driver's strict contract). `increment` on a Checkbox is unsupported.
#[test]
fn driver_increment_on_a_checkbox_is_a_typed_error() {
    let mut app = todomvc_app(true);
    let cb = get_by_role(
        app.world_mut(),
        A11yRole::Checkbox,
        Some("Compose the P1d widgets"),
        None,
    )
    .unwrap();
    let res = buiy_core::a11y::increment(app.world_mut(), cb);
    assert!(
        matches!(res, Err(ActionError::Unsupported { .. })),
        "increment on a Checkbox surfaces a typed Unsupported error, never a panic; got {res:?}"
    );
}
