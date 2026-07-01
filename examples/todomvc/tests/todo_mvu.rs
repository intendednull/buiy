//! Headless logic verification for the MVU TodoMVC (no GPU).
//!
//! Drives the real schedule with synthesized `OnPress` and asserts BOTH the model
//! fold AND the structural reconcile (how many `RowId` entities the bind keeps
//! live) — the keyed-list view the prototype is stress-testing.

use bevy::prelude::*;
use buiy::{CorePlugin, OnPress, WidgetsPlugin};
use buiy_core::text::edit::EditSubmitted;
use todomvc::{
    AddButton, ClearButton, FilterButton, FilterMode, RowId, RowRef, TodoApp, TodoPlugin,
    ToggleButton,
};

fn logic_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        // DX: adding a text input (a Scene) makes the app require ScenePlugin —
        // the hand-composed logic subset must track that or spawn_scene panics.
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            WidgetsPlugin,
        ))
        .add_plugins(TodoPlugin);
    app
}

fn settle(app: &mut App) {
    for _ in 0..4 {
        app.update();
    }
}

fn items(app: &mut App) -> usize {
    app.world_mut()
        .query::<&TodoApp>()
        .iter(app.world())
        .next()
        .unwrap()
        .items
        .len()
}

/// How many row entities the reconcile currently keeps live (the *visible* set).
fn rows(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, With<RowId>>()
        .iter(app.world())
        .count()
}

fn press(app: &mut App, e: Entity) {
    app.world_mut()
        .resource_mut::<Messages<OnPress>>()
        .write(OnPress(e));
    app.update();
}

fn find<M: Component>(app: &mut App) -> Entity {
    app.world_mut()
        .query_filtered::<Entity, With<M>>()
        .iter(app.world())
        .next()
        .expect("marker exists")
}

fn filter_btn(app: &mut App, mode: FilterMode) -> Entity {
    let world = app.world_mut();
    let mut q = world.query::<(Entity, &FilterButton)>();
    q.iter(world).find(|(_, f)| f.0 == mode).unwrap().0
}

fn row_button<M: Component>(app: &mut App, id: u64) -> Entity {
    let world = app.world_mut();
    let mut q = world.query_filtered::<(Entity, &RowRef), With<M>>();
    q.iter(world).find(|(_, r)| r.0 == id).unwrap().0
}

#[test]
fn seed_reconciles_to_three_rows() {
    let mut app = logic_app();
    settle(&mut app);
    assert_eq!(items(&mut app), 3, "three seeded items");
    assert_eq!(rows(&mut app), 3, "the bind spawned three rows for them");
}

#[test]
fn add_spawns_remove_despawns() {
    let mut app = logic_app();
    settle(&mut app);
    let add = find::<AddButton>(&mut app);
    press(&mut app, add);
    assert_eq!(items(&mut app), 4, "add folded");
    assert_eq!(rows(&mut app), 4, "reconcile spawned the new row");

    // Remove the first seeded item (id 0).
    let destroy0 = row_button::<todomvc::DestroyButton>(&mut app, 0);
    press(&mut app, destroy0);
    assert_eq!(items(&mut app), 3, "remove folded");
    assert_eq!(rows(&mut app), 3, "reconcile despawned the gone row");
}

#[test]
fn filter_changes_the_visible_row_set_structurally() {
    let mut app = logic_app();
    settle(&mut app);
    // seeds: id0 undone, id1 DONE, id2 undone → 2 active, 1 completed.
    let active = filter_btn(&mut app, FilterMode::Active);
    press(&mut app, active);
    assert_eq!(rows(&mut app), 2, "Active shows the 2 undone");
    assert_eq!(items(&mut app), 3, "the model still holds all 3");

    let completed = filter_btn(&mut app, FilterMode::Completed);
    press(&mut app, completed);
    assert_eq!(rows(&mut app), 1, "Completed shows the 1 done");

    let all = filter_btn(&mut app, FilterMode::All);
    press(&mut app, all);
    assert_eq!(rows(&mut app), 3, "All shows everything again");
}

#[test]
fn add_from_text_input_bridges_editor_to_mvu() {
    use buiy_core::a11y::inprocess::set_value;
    use buiy_core::a11y::translate::node_id_for;
    use todomvc::AddField;

    let mut app = logic_app();
    settle(&mut app);
    let field = find::<AddField>(&mut app);

    // Seed the field's text via the a11y driver set-value channel (the editor is
    // command-sourced separately from MVU), then let it apply.
    let nid = node_id_for(field);
    let _ = set_value(app.world_mut(), nid, "Ship the prototype");
    settle(&mut app);

    // Enter → EditSubmitted → route_add_submit reads value → enqueue Add.
    app.world_mut()
        .resource_mut::<Messages<EditSubmitted>>()
        .write(EditSubmitted(field));
    app.update();

    assert_eq!(items(&mut app), 4, "the typed text folded into a new todo");
    assert_eq!(rows(&mut app), 4, "the reconcile spawned the new row");
}

#[test]
fn toggle_then_clear_completed() {
    let mut app = logic_app();
    settle(&mut app);
    // Toggle id0 done → 2 completed (id0, id1).
    let toggle0 = row_button::<ToggleButton>(&mut app, 0);
    press(&mut app, toggle0);
    let clear = find::<ClearButton>(&mut app);
    press(&mut app, clear);
    assert_eq!(items(&mut app), 1, "cleared the 2 completed, 1 remains");
    assert_eq!(rows(&mut app), 1, "reconcile despawned the cleared rows");
}
