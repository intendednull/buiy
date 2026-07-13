//! FW2 headless logic tests — the HARD parts the surface exists to stress:
//! **keyed reconcile** identity, the **editor→MVU bridge**, and the **whole-UI
//! replay** wall (spec §5). No GPU: they drive the real schedule (routers +
//! reconciler) with synthesized `OnPress` + real `KeyboardInput`, and assert
//! the model folds AND the reconciled entity tree.
//!
//! The app fixture below (`TodoApp` + `TodoMsg` + `update` + `view`) is the
//! WHOLE app-author surface — no hand-written `route_*`, no `bind_*` — exactly
//! what `buiy_view` lets an author write. The tests drive it through the real
//! path to prove the library's routers + keyed reconciler do the rest.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::prelude::*;
use bevy::window::Ime;

use buiy_core::focus::FocusedEntity;
use buiy_core::mvu::{Cmd, LogicalId, Model, MsgLog, RecordSession};
use buiy_core::replay::{replay_into, unified_stream};
use buiy_core::text::edit::{Clipboard, EditLog, EditSubmitted, MemClipboard};
use buiy_view::{
    BuiyViewAppExt, Element, Kind, MODEL_LID, Space, button, checkbox, checkbox_checked, column,
    editor_value, find_kind, find_press_target, keyed_column, keyed_rows, row, text, text_input,
};

// ---------------------------------------------------------------------------
// The app-author fixture — Model + Msg + update + view ONLY.
// ---------------------------------------------------------------------------

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct TodoApp {
    items: Vec<Todo>,
    /// The draft text-input's content, kept in the model (Elm-style controlled
    /// input): `on_input` syncs it per keystroke, `Add` reads + clears it.
    draft: String,
    next_id: u64,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
struct Todo {
    id: u64,
    title: String,
    done: bool,
}

impl Model for TodoApp {
    type Msg = TodoMsg;
}

#[derive(Clone, Debug, PartialEq, Reflect)]
enum TodoMsg {
    SetDraft(String),
    Add,
    Toggle(u64),
    Remove(u64),
    ClearCompleted,
}

fn update(s: &mut TodoApp, m: TodoMsg) -> Cmd<TodoMsg> {
    match m {
        TodoMsg::SetDraft(t) => s.draft = t,
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

fn view(s: &TodoApp) -> Element<TodoMsg> {
    let remaining = s.items.iter().filter(|t| !t.done).count();
    let any_done = s.items.iter().any(|t| t.done);

    let list = keyed_column(
        s.items.iter(),
        |t| t.id,
        |t| {
            let id = t.id;
            row![
                // `on_toggle` eagerly resolves the (capturing!) closure into a
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

/// The seed scene both the record app and a replay app start from (3 items, id 1
/// seeded done, so a capture shows a checked row).
fn seed() -> TodoApp {
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

// ---------------------------------------------------------------------------
// Harness — the headless plugin subset the widgets + editor need.
// ---------------------------------------------------------------------------

fn app_with(init: TodoApp) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins((
            buiy_core::CorePlugin,
            buiy_core::theme::ThemePlugin,
            buiy_core::a11y::A11yPlugin,
            buiy_core::focus::FocusPlugin,
            buiy_core::layout::LayoutPlugin,
            buiy_core::text::BuiyTextPlugin::default(),
            buiy_widgets::WidgetsPlugin,
        ));
    // Deterministic, headless clipboard + the (winit-absent) IME message channel.
    app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
    app.add_message::<Ime>();
    app.register_type::<Todo>();
    app.ui(init, update, view);
    app
}

/// The default seeded app (3 items, id 1 done).
fn seeded_app() -> App {
    app_with(seed())
}

fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

fn model(app: &mut App) -> TodoApp {
    app.world_mut()
        .query::<&TodoApp>()
        .iter(app.world())
        .next()
        .expect("todo model exists")
        .clone()
}

/// Synthesize a real `OnPress` on `e` and settle the route→drain→reconcile chain.
fn press(app: &mut App, e: Entity) {
    app.world_mut()
        .resource_mut::<Messages<buiy_core::interaction::OnPress>>()
        .write(buiy_core::interaction::OnPress(e));
    for _ in 0..4 {
        app.update();
    }
}

fn press_msg(app: &mut App, want: &TodoMsg) {
    let e = find_press_target::<TodoApp>(app.world_mut(), want)
        .unwrap_or_else(|| panic!("no press target routes {want:?}"));
    press(app, e);
}

/// The realized checkbox state for todo `id` (found via its `Toggle(id)` route).
fn checkbox_of(app: &mut App, id: u64) -> bool {
    let e = find_press_target::<TodoApp>(app.world_mut(), &TodoMsg::Toggle(id))
        .expect("checkbox routes its toggle");
    checkbox_checked(app.world_mut(), e)
}

/// Focus the draft field + type `s` a char at a time through the REAL editor path
/// (KeyboardInput → editor edit → TextChanged → route_text_input → SetDraft fold).
fn type_into_field(app: &mut App, s: &str) {
    let field = find_kind(app.world_mut(), Kind::TextInput).expect("draft field realized");
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(field);
    app.update();
    for ch in s.chars() {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA,
            logical_key: Key::Character(ch.to_string().into()),
            state: ButtonState::Pressed,
            text: Some(ch.to_string().into()),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
}

fn submit_field(app: &mut App) {
    let field = find_kind(app.world_mut(), Kind::TextInput).expect("draft field realized");
    app.world_mut()
        .resource_mut::<Messages<EditSubmitted>>()
        .write(EditSubmitted(field));
    for _ in 0..4 {
        app.update();
    }
}

// ---------------------------------------------------------------------------
// (a) KEYED RECONCILE — add/remove/reorder preserve row identity + state.
// ---------------------------------------------------------------------------

#[test]
fn keyed_reconcile_preserves_row_identity_and_state() {
    let mut app = seeded_app();
    settle(&mut app);

    // Seed → 3 keyed rows (ids 0,1,2). Snapshot their entity ids by key.
    let rows0: std::collections::HashMap<u64, Entity> =
        keyed_rows(app.world_mut()).into_iter().collect();
    assert_eq!(rows0.len(), 3, "three seeded rows");
    let (e0, e1, e2) = (rows0[&0], rows0[&1], rows0[&2]);

    // The seeded done flag drove the REAL checkbox leaf (id 1 is checked).
    assert!(
        checkbox_of(&mut app, 1),
        "id 1 seeded done → its real checkbox is checked"
    );
    assert!(!checkbox_of(&mut app, 0), "id 0 unchecked");

    // Toggle the MIDDLE row (id 1) OFF via its real checkbox press.
    press_msg(&mut app, &TodoMsg::Toggle(1));
    settle(&mut app);

    // Add a row (draft "New" + submit).
    type_into_field(&mut app, "New");
    submit_field(&mut app);
    settle(&mut app);

    // Remove the FIRST row (id 0) via its X button.
    press_msg(&mut app, &TodoMsg::Remove(0));
    settle(&mut app);

    // Final model: ids {1,2,3}, id 1 now undone.
    let m = model(&mut app);
    let ids: Vec<u64> = m.items.iter().map(|t| t.id).collect();
    assert_eq!(ids, vec![1, 2, 3], "id0 removed, id3 (New) appended");
    assert!(!m.items[0].done, "id 1 was toggled off");

    // THE KEYED GUARANTEE: surviving rows kept their EXACT entity ids (no churn),
    // and the despawn/spawn touched only the removed/added keys.
    let rows1: std::collections::HashMap<u64, Entity> =
        keyed_rows(app.world_mut()).into_iter().collect();
    assert_eq!(rows1.len(), 3, "3 rows after +1 -1");
    assert_eq!(
        rows1[&1], e1,
        "row id 1 kept its entity id across toggle+add+remove"
    );
    assert_eq!(rows1[&2], e2, "row id 2 kept its entity id (never rebuilt)");
    assert!(!rows1.contains_key(&0), "id 0's row despawned");
    assert!(rows1.contains_key(&3), "id 3's row spawned fresh");
    assert!(
        rows1[&3] != e0 && rows1[&3] != e1 && rows1[&3] != e2,
        "new row is a new entity"
    );

    // The surviving checkbox reflects the toggled-off model state (state
    // preserved via the model, not a rebuild).
    assert!(
        !checkbox_of(&mut app, 1),
        "id 1's checkbox is now unchecked after the toggle"
    );
}

// ---------------------------------------------------------------------------
// (b) EDITOR BRIDGE — on_input syncs the draft; Add appends + clears.
// ---------------------------------------------------------------------------

#[test]
fn editor_bridge_on_input_and_submit() {
    let mut app = app_with(TodoApp {
        items: vec![],
        draft: String::new(),
        next_id: 0,
    });
    settle(&mut app);

    // Type into the real editor → on_input syncs each keystroke into the draft.
    type_into_field(&mut app, "Ship it");
    settle(&mut app);
    assert_eq!(
        model(&mut app).draft,
        "Ship it",
        "on_input synced draft per keystroke"
    );

    // Enter → on_submit(Add) → append + clear draft.
    submit_field(&mut app);
    settle(&mut app);
    let m = model(&mut app);
    assert_eq!(m.items.len(), 1, "submit appended a todo");
    assert_eq!(m.items[0].title, "Ship it");
    assert_eq!(m.draft, "", "Add cleared the model draft");

    // The controlled reconciler cleared the REAL editor buffer (clear =
    // SelectAll+Delete) to match the empty draft.
    let field = find_kind(app.world_mut(), Kind::TextInput).unwrap();
    assert_eq!(
        editor_value(app.world_mut(), field),
        "",
        "editor cleared to match draft (SelectAll+Delete, not Insert(\"\"))"
    );
}

// ---------------------------------------------------------------------------
// (c) THE REPLAY WALL (spec §5, the headline property) — record a dynamic
//     session, replay the unified log into a FRESH same-seed app, and assert the
//     single model + reconciled keyed tree reproduce; report the dead-letters.
// ---------------------------------------------------------------------------

#[test]
fn replay_of_add_toggle_remove_session_reproduces_the_model_and_tree() {
    // --- Record a dynamic add/toggle/remove session. -------------------------
    let mut rec = seeded_app();
    settle(&mut rec);
    rec.world_mut().resource_mut::<RecordSession>().start(); // seq=0, unified switch ON

    // Add "Task A", toggle id 0 done, remove id 2 — structural churn galore.
    type_into_field(&mut rec, "Task A");
    submit_field(&mut rec);
    press_msg(&mut rec, &TodoMsg::Toggle(0));
    press_msg(&mut rec, &TodoMsg::Remove(2));
    settle(&mut rec);

    let recorded = model(&mut rec);
    let rec_ids: Vec<u64> = recorded.items.iter().map(|t| t.id).collect();
    assert_eq!(rec_ids, vec![0, 1, 3], "recorded end-state ids");
    assert!(recorded.items[0].done, "id 0 toggled done in the session");
    let rec_rows = keyed_rows(rec.world_mut()).len();
    assert_eq!(rec_rows, 3, "recorded tree has 3 rows");

    // Track A (1a-ii/1a-iii) must NOT false-positive on the normal buiy_view path: the one
    // root model carries `MODEL_LID`, and every controlled leaf (the checkboxes) is exempt via
    // `ControlledLeaf` (their id-less recorded folds are intentionally model-reconstructed on
    // replay). So a full record session raises ZERO MVU id-diagnostics.
    #[cfg(debug_assertions)]
    {
        let diags = &rec
            .world()
            .resource::<buiy_core::mvu::MvuDiagnostics>()
            .violations;
        assert!(
            diags.is_empty(),
            "buiy_view raises ZERO MVU id-diagnostics on a normal record session \
             (root=MODEL_LID, leaves=ControlledLeaf-exempt); got: {diags:?}"
        );
    }

    // How much did the session log, and of what kinds? (Reported, not exact.)
    let (widget_entries, edit_entries, off_model) = {
        let world = rec.world();
        let stream = unified_stream(world.resource::<MsgLog>(), world.resource::<EditLog>());
        let w = stream.iter().filter(|e| e.is_widget()).count();
        let ed = stream.len() - w;
        let off = stream
            .iter()
            .filter(|e| e.lid() != LogicalId(MODEL_LID))
            .count();
        (w, ed, off)
    };
    println!(
        "REPLAY LOG: {widget_entries} widget folds, {edit_entries} editor commands; \
         {off_model} entries target a non-model LID (off-log children)"
    );

    // --- Replay the unified log into a FRESH app from the same seed. ----------
    let mut replay = seeded_app();
    settle(&mut replay);
    let dead = {
        let world = rec.world();
        replay_into(
            &mut replay,
            world.resource::<MsgLog>(),
            world.resource::<EditLog>(),
        )
    };
    settle(&mut replay);

    // --- THE VERDICT: the MODEL replays state-identically (pure fn of the log). --
    let replayed = model(&mut replay);
    assert_eq!(
        replayed, recorded,
        "MODEL REPLAY: the whole TodoApp (items + done flags + draft + next_id) is \
         reproduced state-identically by replaying the message log into a fresh app"
    );

    // ...and the reconciled TREE (derived from the replayed model) matches too.
    let replay_rows: std::collections::HashSet<u64> = keyed_rows(replay.world_mut())
        .into_iter()
        .map(|(k, _)| k)
        .collect();
    let expect_rows: std::collections::HashSet<u64> = recorded.items.iter().map(|t| t.id).collect();
    assert_eq!(
        replay_rows, expect_rows,
        "TREE REPLAY: the keyed rows re-derive to match"
    );

    // ...and the checkbox VISUAL is re-driven from the replayed model (id 0 checked).
    let cb0 = find_press_target::<TodoApp>(replay.world_mut(), &TodoMsg::Toggle(0)).unwrap();
    assert!(
        checkbox_checked(replay.world_mut(), cb0),
        "id 0's checkbox reads checked after replay (re-derived from the model)"
    );

    // --- THE HONEST §7.4 REPORT: dead letters. --------------------------------
    // Off-log children (the reconciler-spawned editor field + checkboxes) carry
    // NO stable LogicalId, so their leaf/editor log entries dead-letter on
    // replay — HARMLESS here, because the model path already reconstructs
    // everything (the value is model-reconstructed, not replayed per-entity).
    println!(
        "REPLAY DEAD-LETTERS: {} (off-log children — editor field + checkbox leaves — \
         whose entries could not resolve; harmless: the model re-derives their state)",
        dead.len()
    );
    // The model-targeted folds must NOT dead-letter (that would break replay).
    let model_deadletters = dead
        .iter()
        .filter(|d| d.lid == LogicalId(MODEL_LID))
        .count();
    assert_eq!(
        model_deadletters, 0,
        "the ONE model's folds all resolved on replay (its stable LogicalId is the \
         load-bearing identity)"
    );
}
