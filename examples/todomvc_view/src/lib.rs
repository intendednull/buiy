//! TodoMVC, authored in the `buiy_view` surface.
//!
//! This is the WHOLE app-author surface: a `Model`, an `enum Msg`, a pure
//! `update`, and a `view(&Model) -> Element<Msg>`. There is **NO** hand-written
//! `route_*` press system, **NO** `route_add_submit` editor bridge, and **NO**
//! hand-rolled `bind_todo_list` keyed reconcile — the library's routers + the
//! keyed reconciler do all three. Compare to `examples/todomvc`, whose
//! `route_todo_press` (DX-3), `route_add_submit`, and ~60-line `bind_todo_list`
//! (DX-2) are exactly what this file deletes.
//!
//! The whole list lives in ONE `TodoApp` model; the rows are pure *derived view*
//! (`keyed_column`). Because the structural truth (`items`) is in the model, and
//! toggles/edits route to the model as funneled `Msg`s, the model is a pure
//! function of the message log — which is what makes whole-UI replay hold here
//! (the reconciler re-derives the rows from the replayed model).
//!
//! Shared by the windowed `todomvc_view` bin and the headless
//! `capture_todomvc_view` bin, so both drive the same authored code.

use bevy::prelude::*;
use buiy_core::mvu::{Cmd, Model};
use buiy_view::{
    BuiyViewAppExt, Element, Space, button, checkbox, column, keyed_column, row, text, text_input,
};

/// MODEL — the whole app state in one component (the single source of truth).
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct TodoApp {
    pub items: Vec<Todo>,
    /// The draft text-input's content, kept in the model (Elm-style controlled
    /// input): `on_input` syncs it per keystroke, `Add` reads + clears it.
    pub draft: String,
    pub next_id: u64,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct Todo {
    pub id: u64,
    pub title: String,
    pub done: bool,
}

impl Model for TodoApp {
    type Msg = TodoMsg;
}

/// The app's messages. All fold onto the ONE `TodoApp` model.
#[derive(Clone, Debug, PartialEq, Reflect)]
pub enum TodoMsg {
    /// Draft synced from the editor per keystroke (`text_input(..).on_input`).
    SetDraft(String),
    /// Enter / the Add button — appends the (trimmed) draft, then clears it.
    Add,
    /// Flip one todo's done flag (a keyed row's `checkbox(..).on_toggle`).
    Toggle(u64),
    /// Delete one todo (a keyed row's `X` button).
    Remove(u64),
    /// Drop every completed todo.
    ClearCompleted,
}

/// UPDATE — the pure reducer over the whole list.
pub fn update(s: &mut TodoApp, m: TodoMsg) -> Cmd<TodoMsg> {
    match m {
        TodoMsg::SetDraft(text) => s.draft = text,
        TodoMsg::Add => {
            let title = s.draft.trim().to_string();
            if !title.is_empty() {
                let id = s.next_id;
                s.next_id += 1;
                s.items.push(Todo {
                    id,
                    title,
                    done: false,
                });
                s.draft.clear();
            }
        }
        TodoMsg::Toggle(id) => {
            if let Some(t) = s.items.iter_mut().find(|t| t.id == id) {
                t.done = !t.done;
            }
        }
        TodoMsg::Remove(id) => s.items.retain(|t| t.id != id),
        TodoMsg::ClearCompleted => s.items.retain(|t| !t.done),
    }
    Cmd::none()
}

/// VIEW — one declarative description of the whole card. No markers, no bind, no
/// route systems: the derived "{n} items left" is a plain expression over the
/// model, and the row list is a `keyed_column` (reorder-safe by todo id).
pub fn view(s: &TodoApp) -> Element<TodoMsg> {
    let remaining = s.items.iter().filter(|t| !t.done).count();
    let any_done = s.items.iter().any(|t| t.done);

    // The keyed list — one row per todo, matched by `id` so add/remove/reorder
    // preserve each row's widget identity + the checkbox's internal state.
    let list = keyed_column(
        s.items.iter(),
        |t| t.id,
        |t| {
            let id = t.id;
            row![
                // `on_toggle` eagerly resolves its (capturing!) closure into a
                // concrete `Toggle(id)` value — no closure is stored anywhere.
                checkbox(t.done).on_toggle(move |_new| TodoMsg::Toggle(id)),
                text(t.title.clone()).size(20.0),
                button("X").on_press(TodoMsg::Remove(id)),
            ]
            .gap(Space::Sm)
            .align_center()
        },
    )
    .gap(Space::Sm);

    column![
        text!("todos").size(40.0),
        row![
            text_input(s.draft.clone())
                .placeholder("What needs to be done?")
                .on_input(TodoMsg::SetDraft)
                .on_submit(TodoMsg::Add),
            button("Add").on_press(TodoMsg::Add),
        ]
        .gap(Space::Sm)
        .align_center(),
        list,
        row![
            text!("{} items left", remaining).size(18.0),
            button("Clear completed").on_press_maybe(any_done.then_some(TodoMsg::ClearCompleted)),
        ]
        .gap(Space::Md)
        .align_center(),
    ]
    .gap(Space::Md)
    .padding(Space::Xl)
}

/// The seed scene the demo starts from (3 items, id 1 seeded done, so the
/// capture shows a checked row).
pub fn seed() -> TodoApp {
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
        draft: String::new(),
        next_id: 3,
    }
}

/// Install the TodoMVC onto an app already carrying the Buiy plugins.
pub fn install(app: &mut App) -> &mut App {
    app.register_type::<Todo>();
    app.ui(seed(), update, view)
}

/// Install with a caller-chosen initial model (the capture bin seeds a draft).
pub fn install_with(app: &mut App, init: TodoApp) -> &mut App {
    app.register_type::<Todo>();
    app.ui(init, update, view)
}
