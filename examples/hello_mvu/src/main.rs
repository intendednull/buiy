//! Buiy MVU hello-world: a `+ / −` counter driven entirely through the MVU funnel.
//!
//! MVU (Model-View-Update) is Buiy's primary state interface (`buiy_core::mvu`).
//! The loop this example wires:
//!
//! 1. **Model** — a `Clone + PartialEq + Reflect` component the drain owns.
//! 2. **Message** — a `Reflect` enum describing an intent (so a recorded session
//!    replays byte-identically).
//! 3. **Reducer** — a pure `fn(&mut Model, Msg) -> Cmd<Msg>` that folds a message.
//! 4. **Enqueue** — the single sanctioned mutation: a handler *enqueues* a message
//!    (it never touches the model). The one ordered drain is the sole writer.
//! 5. **Bind** — a system projects the folded model into the view (the Text node);
//!    `Changed<Model>` fires only when the drain actually mutated it, so an
//!    idempotent fold does no view work.
//!
//! Run: `cargo run -p hello_mvu` — click `+` / `−` to fold the counter.

use bevy::prelude::*;
use buiy::*;
use buiy_core::mvu::{Cmd, Model, MvuModelExt, MvuSet, enqueue};

// 1. The MODEL: a single-source-of-truth component the MVU drain owns.
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter {
    value: i32,
}

// 2. Its MESSAGES: `Reflect` so the record/replay log round-trips.
#[derive(Clone, Debug, Reflect, PartialEq)]
enum CounterMsg {
    Increment,
    Add(i32),
}

impl Model for Counter {
    type Msg = CounterMsg;
}

// 3. The REDUCER: pure — fold a message into the model, return an effect.
//    No `Commands`/`World` in scope, so it *cannot* mutate anything but the model.
fn update(counter: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => counter.value += 1,
        CounterMsg::Add(n) => counter.value += n,
    }
    Cmd::none()
}

// Which message a button enqueues, and to which `Counter` actor.
#[derive(Component)]
struct CounterButton {
    target: Entity,
    msg: CounterMsg,
}

// Where the view (the Text node) lives, so the bind can update it.
#[derive(Resource)]
struct CounterUi {
    label: Entity,
}

fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins).add_plugins(BuiyPlugin);
    // Register the model + its reducer. `BuiyPlugin` already installs `MvuCorePlugin`
    // (via the widgets), so we only declare our own model here.
    app.mvu_model(update);
    app.add_systems(Startup, setup);
    // A press ENQUEUES (in the enqueue window, so it folds the same frame); the bind
    // projects the folded model into the view.
    app.add_systems(
        Update,
        (enqueue_presses.in_set(MvuSet::Enqueue), bind_counter),
    );
    app.run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // The MVU actor: one `Counter` model entity.
    let counter = commands.spawn(Counter::default()).id();

    // The view: a Text node the bind system keeps in sync with the model.
    let label = commands
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Count: 0")),
            FontSize(28.0),
        ))
        .id();
    commands.insert_resource(CounterUi { label });

    // Two buttons; a press enqueues a message to the counter actor.
    let minus = commands
        .spawn((
            Button::new("-"),
            CounterButton {
                target: counter,
                msg: CounterMsg::Add(-1),
            },
        ))
        .id();
    let plus = commands
        .spawn((
            Button::new("+"),
            CounterButton {
                target: counter,
                msg: CounterMsg::Increment,
            },
        ))
        .id();

    // Lay the row out: [-]  Count: N  [+]
    commands
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .gap_px(16.0)
                .padding(40.0)
                .align_items(AlignItems::Center),
        ))
        .add_children(&[minus, label, plus]);
}

// A HANDLER: it only ENQUEUES (the single sanctioned mutation). It never writes the
// `Counter` directly — the drain is the sole writer, which is what makes the state
// recordable and replayable.
fn enqueue_presses(
    mut presses: MessageReader<OnPress>,
    buttons: Query<&CounterButton>,
    mut commands: Commands,
) {
    for OnPress(entity) in presses.read() {
        if let Ok(button) = buttons.get(*entity) {
            enqueue::<Counter>(&mut commands, button.target, button.msg.clone());
        }
    }
}

// The BIND (the "V" in MVU): project the folded model into the view. `Changed<Counter>`
// fires only on a real mutation (`set_if_neq`), so an idempotent fold does no view work.
fn bind_counter(
    counter: Query<&Counter, Changed<Counter>>,
    ui: Res<CounterUi>,
    mut texts: Query<&mut Text>,
) {
    if let Ok(counter) = counter.single()
        && let Ok(mut text) = texts.get_mut(ui.label)
    {
        text.0 = format!("Count: {}", counter.value);
    }
}
