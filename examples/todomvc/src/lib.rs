//! TodoMVC on the Buiy MVU paradigm (prototype) — the canonical *real* MVU app.
//!
//! The whole todo list lives in ONE model (`TodoApp { items: Vec<Todo>, .. }`);
//! the view is a hand-written **keyed-reconcile bind** that spawns/despawns row
//! entities to match the model. This deliberately drives the spec's flagged
//! walls: derived/structural view (§7.4 "targeted, not yet proven" for replay)
//! and the absence of any `view(model) -> Element`. DX notes are inline (`DX-N`)
//! as evidence for `docs/reports/2026-06-30-demos-mvu-migration-journal.md`.
//!
//! Wave 2a: "Add" is a button that appends a templated item (isolates the
//! structural-reconcile learning). Wave 2b adds the real text-input bridge.

use std::collections::{HashMap, HashSet};

use bevy::prelude::*;
use buiy::*;
use buiy_core::mvu::{Cmd, Model, MvuModelExt, MvuSet, enqueue};
// DX-1 (again): the editor bridge types are NOT in the `buiy` prelude either.
use buiy_core::text::edit::{EditSubmitted, TextEditState};

// ---------------------------------------------------------------------------
// MODEL — the whole app state in one component (DX-4: on a chosen entity).
// ---------------------------------------------------------------------------

#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TodoApp {
    pub items: Vec<Todo>,
    pub filter: FilterMode,
    pub next_id: u64,
}

#[derive(Clone, PartialEq, Reflect, Debug)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    pub done: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Reflect, Debug, Default)]
pub enum FilterMode {
    #[default]
    All,
    Active,
    Completed,
}

impl FilterMode {
    fn matches(self, done: bool) -> bool {
        match self {
            FilterMode::All => true,
            FilterMode::Active => !done,
            FilterMode::Completed => done,
        }
    }
}

impl Model for TodoApp {
    type Msg = TodoMsg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum TodoMsg {
    Add(String),
    Toggle(u64),
    Remove(u64),
    SetFilter(FilterMode),
    ClearCompleted,
}

/// UPDATE — pure reducer over the whole list. Still clean (this is MVU's win).
pub fn todo_update(m: &mut TodoApp, msg: TodoMsg) -> Cmd<TodoMsg> {
    match msg {
        TodoMsg::Add(title) => {
            let id = m.next_id;
            m.next_id += 1;
            m.items.push(Todo {
                id,
                title,
                done: false,
            });
        }
        TodoMsg::Toggle(id) => {
            if let Some(t) = m.items.iter_mut().find(|t| t.id == id) {
                t.done = !t.done;
            }
        }
        TodoMsg::Remove(id) => m.items.retain(|t| t.id != id),
        TodoMsg::SetFilter(f) => m.filter = f,
        TodoMsg::ClearCompleted => m.items.retain(|t| !t.done),
    }
    Cmd::none()
}

// ---------------------------------------------------------------------------
// View markers (DX-3: routing + reconcile both key off these by hand).
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct TodoList; // the row container the bind reconciles into
#[derive(Component)]
pub struct RowId(pub u64); // identity stamp on a row (the reconcile key)
#[derive(Component)]
pub struct RowRef(pub u64); // back-ref on a row's buttons/label → the todo id
#[derive(Component)]
pub struct RowLabel;
#[derive(Component)]
pub struct ToggleButton;
#[derive(Component)]
pub struct DestroyButton;
#[derive(Component)]
pub struct AddButton;
#[derive(Component)]
pub struct AddField; // the real text input (Wave 2b, the editor→MVU bridge)
#[derive(Component)]
pub struct FilterButton(pub FilterMode);
#[derive(Component)]
pub struct ClearButton;

pub struct TodoPlugin;

impl Plugin for TodoPlugin {
    fn build(&self, app: &mut App) {
        // DX: nested Reflect types used in the model/Msg must be registered too.
        app.register_type::<Todo>();
        app.register_type::<FilterMode>();
        app.mvu_model(todo_update)
            .app()
            .add_systems(Startup, setup)
            .add_systems(Update, route_todo_press.in_set(MvuSet::Enqueue))
            // Wave 2b: the editor→MVU bridge — Enter in the field → enqueue Add.
            .add_systems(Update, route_add_submit.in_set(MvuSet::Enqueue))
            // DX-2: the entire "View" is this one hand-written reconcile system.
            .add_systems(Update, bind_todo_list.in_set(MvuSet::Bind));
    }
}

pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // Seed three items directly on the model (seed-scene initial state). The bind
    // materializes the rows on frame 1 (the model is `Changed` when spawned).
    commands.spawn((
        TodoApp {
            items: vec![
                Todo {
                    id: 0,
                    title: "Buy milk".into(),
                    done: false,
                },
                Todo {
                    id: 1,
                    title: "Walk the dog".into(),
                    done: true,
                },
                Todo {
                    id: 2,
                    title: "Write the journal".into(),
                    done: false,
                },
            ],
            filter: FilterMode::All,
            next_id: 3,
        },
        Name::new("todo-model"),
    ));

    let title = commands
        .spawn((Node, Style::default(), Text("todos".into()), FontSize(40.0)))
        .id();
    // Wave 2b: a real single-line text input. Pressing Enter submits it
    // (`EditSubmitted`); `route_add_submit` reads its value and enqueues `Add`.
    let field = commands
        .spawn_scene(text_input_single_line("What needs to be done? (Enter)"))
        .insert((AddField, Name::new("#add-field")))
        .id();
    // The templated-add button (kept alongside the field for quick capture demos).
    let add = commands.spawn((Button::new("Add todo"), AddButton)).id();
    let list = commands
        .spawn((Node, Style::default().flex_column().gap_px(8.0), TodoList))
        .id();

    let all = commands
        .spawn((Button::new("All"), FilterButton(FilterMode::All)))
        .id();
    let active = commands
        .spawn((Button::new("Active"), FilterButton(FilterMode::Active)))
        .id();
    let completed = commands
        .spawn((
            Button::new("Completed"),
            FilterButton(FilterMode::Completed),
        ))
        .id();
    let filter_bar = commands
        .spawn((Node, Style::default().flex_row().gap_px(8.0)))
        .add_children(&[all, active, completed])
        .id();

    let clear = commands
        .spawn((Button::new("Clear completed"), ClearButton))
        .id();

    commands
        .spawn((
            Node,
            Style::default().flex_column().padding(32.0).gap_px(16.0),
        ))
        .add_children(&[title, field, add, list, filter_bar, clear]);
}

/// Wave 2b — the editor→MVU BRIDGE. The text editor is command-sourced *separately*
/// from MVU (spec §6), so its value is not an MVU model. On Enter (`EditSubmitted`)
/// this reads the field's `TextEditState::value()` and enqueues it as `Add`.
///
/// DX-6: note what's missing — there is no auto-clear here. Clearing the field
/// back to empty rides the a11y DRIVER channel (`set_value`, an exclusive
/// `&mut World` op that lowers to `SelectAll`+`Insert`), which `enqueue` handlers
/// cannot reach. So "add from input, then clear" is inherently a two-phase,
/// two-tier dance (the gallery stages it through an `*Intents` resource + an
/// exclusive applier). The editor simply does not compose into the MVU funnel.
pub fn route_add_submit(
    mut submits: MessageReader<EditSubmitted>,
    fields: Query<&TextEditState, With<AddField>>,
    app: Query<Entity, With<TodoApp>>,
    mut commands: Commands,
) {
    let Ok(target) = app.single() else {
        return;
    };
    for EditSubmitted(e) in submits.read() {
        if let Ok(state) = fields.get(*e) {
            let text = state.value().trim().to_string();
            if !text.is_empty() {
                enqueue::<TodoApp>(&mut commands, target, TodoMsg::Add(text));
            }
        }
    }
}

/// DX-3: ONE hand-rolled route system, fanning `OnPress` to the right `TodoMsg`
/// by marker. Toggle/Destroy recover the todo id from the button's `RowRef`.
#[allow(clippy::too_many_arguments)]
pub fn route_todo_press(
    mut presses: MessageReader<OnPress>,
    toggles: Query<&RowRef, With<ToggleButton>>,
    destroys: Query<&RowRef, With<DestroyButton>>,
    filters: Query<&FilterButton>,
    adds: Query<(), With<AddButton>>,
    clears: Query<(), With<ClearButton>>,
    app: Query<Entity, With<TodoApp>>,
    mut add_seq: Local<u32>,
    mut commands: Commands,
) {
    let Ok(target) = app.single() else {
        return;
    };
    for OnPress(e) in presses.read() {
        let e = *e;
        let msg = if let Ok(r) = toggles.get(e) {
            TodoMsg::Toggle(r.0)
        } else if let Ok(r) = destroys.get(e) {
            TodoMsg::Remove(r.0)
        } else if let Ok(f) = filters.get(e) {
            TodoMsg::SetFilter(f.0)
        } else if adds.contains(e) {
            *add_seq += 1;
            TodoMsg::Add(format!("Todo {}", *add_seq))
        } else if clears.contains(e) {
            TodoMsg::ClearCompleted
        } else {
            continue;
        };
        enqueue::<TodoApp>(&mut commands, target, msg);
    }
}

/// DX-2 + the structural WALL: the hand-written keyed-reconcile "View". An
/// exclusive `&mut World` system (structural spawn/despawn need it) that diffs
/// the model's visible items against the live row entities by `RowId`, spawns
/// missing rows, despawns gone rows, fixes child order, and refreshes labels.
///
/// This is ~60 lines of fiddly reconciliation that an Elm/Iced `view(model)`
/// would express in ~5 — and it is exactly the derived/structural path the spec
/// flags as not-yet-proven for replay.
pub fn bind_todo_list(world: &mut World) {
    // Did the model change this frame?
    let model = {
        let mut q = world.query_filtered::<&TodoApp, Changed<TodoApp>>();
        match q.iter(world).next() {
            Some(m) => m.clone(),
            None => return,
        }
    };

    // The visible items, in document order (the filter is part of the view).
    let visible: Vec<Todo> = model
        .items
        .iter()
        .filter(|t| model.filter.matches(t.done))
        .cloned()
        .collect();
    let visible_ids: HashSet<u64> = visible.iter().map(|t| t.id).collect();

    let list = {
        let mut q = world.query_filtered::<Entity, With<TodoList>>();
        match q.iter(world).next() {
            Some(e) => e,
            None => return,
        }
    };

    // Existing rows: id → entity.
    let existing: HashMap<u64, Entity> = {
        let mut q = world.query::<(Entity, &RowId)>();
        q.iter(world).map(|(e, r)| (r.0, e)).collect()
    };

    // Despawn rows whose todo is no longer visible (removed or filtered out).
    for (&id, &row) in existing.iter() {
        if !visible_ids.contains(&id) {
            world.entity_mut(row).despawn();
        }
    }

    // Find-or-spawn each visible row, collecting them in document order.
    let mut ordered = Vec::with_capacity(visible.len());
    for todo in &visible {
        let row = match existing.get(&todo.id) {
            Some(&e) if visible_ids.contains(&todo.id) => e,
            _ => spawn_row(world, todo.id),
        };
        ordered.push(row);
    }

    // Pin the container's children to the exact visible order.
    world.entity_mut(list).replace_children(&ordered);

    // Refresh each visible row's label text (done prefix + title).
    let want: HashMap<u64, String> = visible
        .iter()
        .map(|t| {
            (
                t.id,
                format!("{} {}", if t.done { "[x]" } else { "[ ]" }, t.title),
            )
        })
        .collect();
    let updates: Vec<(Entity, String)> = {
        let mut q = world.query_filtered::<(Entity, &RowRef), With<RowLabel>>();
        q.iter(world)
            .filter_map(|(e, r)| want.get(&r.0).map(|s| (e, s.clone())))
            .collect()
    };
    for (e, text) in updates {
        if let Some(mut t) = world.get_mut::<Text>(e)
            && t.0 != text
        {
            t.0 = text;
        }
    }
}

/// Build one row (toggle | label | destroy), stamped with its `RowId`/`RowRef`.
/// Uses immediate `world.spawn` so the new entities are queryable THIS frame.
fn spawn_row(world: &mut World, id: u64) -> Entity {
    let toggle = world
        .spawn((Button::new("toggle"), RowRef(id), ToggleButton))
        .id();
    let label = world
        .spawn((
            Node,
            Style::default(),
            Text(String::new()),
            FontSize(18.0),
            RowRef(id),
            RowLabel,
        ))
        .id();
    let destroy = world
        .spawn((Button::new("X"), RowRef(id), DestroyButton))
        .id();
    let row = world
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .gap_px(12.0)
                .align_items(AlignItems::Center),
            RowId(id),
        ))
        .id();
    world
        .entity_mut(row)
        .add_children(&[toggle, label, destroy]);
    row
}
