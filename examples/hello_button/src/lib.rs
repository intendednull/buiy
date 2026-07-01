//! Buiy MVU demo (prototype): a **counter** — the "hello world" of Model-View-Update.
//!
//! Migrated from the original stateless `hello_button` smoke test (which only
//! `info!`-logged presses) to exercise the new `buiy_core::mvu` paradigm end to
//! end as an *app author* would. The whole feature is packaged as a
//! [`CounterPlugin`] so `main.rs`, the headless logic test, and the GPU capture
//! bin all drive the **same** wiring.
//!
//! Commented with the DX friction it hit (`DX-N`) — evidence for the journal
//! (`docs/reports/2026-06-30-demos-mvu-migration-journal.md`).

use bevy::prelude::*;
use buiy::*;
// DX-1: none of these are in `buiy::prelude` — they come from a second, direct
// `buiy_core` dependency, and `enqueue` is only reachable under `::mvu::`.
use buiy_core::mvu::{Cmd, Model, MvuModelExt, MvuSet, enqueue};

// ---------------------------------------------------------------------------
// MODEL — the single source of truth. Must be a `Component` (DX-4: there is no
// resource-backed model, so app-global state lives on a chosen entity).
// ---------------------------------------------------------------------------

#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct Counter {
    pub value: i64,
}

impl Model for Counter {
    type Msg = CounterMsg;
}

#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum CounterMsg {
    Increment,
    Decrement,
    Reset,
}

/// UPDATE — the pure reducer. Tiny and clean; this is the part MVU makes nice.
pub fn counter_update(c: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => c.value += 1,
        CounterMsg::Decrement => c.value -= 1,
        CounterMsg::Reset => c.value = 0,
    }
    Cmd::none()
}

// ---------------------------------------------------------------------------
// View markers — DX-3: routing is not built, so a press handler disambiguates
// buttons by a marker component placed on each, and finds the model's entity by
// querying for it.
// ---------------------------------------------------------------------------

#[derive(Component)]
pub struct IncButton;
#[derive(Component)]
pub struct DecButton;
#[derive(Component)]
pub struct ResetButton;
#[derive(Component)]
pub struct CountLabel;

/// The whole counter feature as one plugin: model+reducer wiring, the scene, the
/// press→Msg route, and the model→label bind.
pub struct CounterPlugin;

impl Plugin for CounterPlugin {
    fn build(&self, app: &mut App) {
        app
            // One-call model wiring: register_type + add_model + add_reducer (late drain).
            .mvu_model(counter_update)
            .app() // ModelWiring → &mut App (DX: must remember to escape the handle)
            .add_systems(Startup, setup)
            // DX-3: the press→Msg edge is a hand-written system, in the MVU enqueue
            // window so the pinned ApplyDeferred folds it the same frame.
            .add_systems(Update, route_counter_press.in_set(MvuSet::Enqueue))
            // DX-2: the "View" is a hand-written bind that projects the model onto
            // the label's `Text`. There is no `view(model) -> Element`.
            .add_systems(Update, bind_counter_label.in_set(MvuSet::Bind));
    }
}

pub fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);

    // The model lives on its own entity (DX-4). The label is a *separate* entity,
    // so the bind must bridge model-entity → label-entity by hand.
    commands.spawn((Counter::default(), Name::new("counter-model")));

    let label = commands
        .spawn((
            Node,
            Style::default(),
            Text(String::from("0")),
            FontSize(48.0),
            CountLabel,
        ))
        .id();

    let dec = commands.spawn((Button::new("-"), DecButton)).id();
    let inc = commands.spawn((Button::new("+"), IncButton)).id();
    let reset = commands.spawn((Button::new("Reset"), ResetButton)).id();

    let row = commands
        .spawn((Node, Style::default().flex_row().gap_px(12.0)))
        .add_children(&[dec, inc, reset])
        .id();

    commands
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .padding(32.0)
                .gap_px(24.0)
                .align_items(AlignItems::Center),
        ))
        .add_children(&[label, row]);
}

/// DX-3: hand-rolled routing. Read the shared `OnPress` sink, decide which Msg by
/// the pressed entity's marker, and `enqueue` it to the (singleton) counter entity.
pub fn route_counter_press(
    mut presses: MessageReader<OnPress>,
    inc: Query<(), With<IncButton>>,
    dec: Query<(), With<DecButton>>,
    reset: Query<(), With<ResetButton>>,
    counter: Query<Entity, With<Counter>>,
    mut commands: Commands,
) {
    let Ok(target) = counter.single() else {
        return; // DX-4: "find the model's entity" is the app author's problem.
    };
    for OnPress(e) in presses.read() {
        let msg = if inc.contains(*e) {
            CounterMsg::Increment
        } else if dec.contains(*e) {
            CounterMsg::Decrement
        } else if reset.contains(*e) {
            CounterMsg::Reset
        } else {
            continue;
        };
        enqueue::<Counter>(&mut commands, target, msg);
    }
}

/// DX-2: the hand-written "View". Projects `Changed<Counter>` onto the label text.
pub fn bind_counter_label(
    changed: Query<&Counter, Changed<Counter>>,
    mut label: Query<&mut Text, With<CountLabel>>,
) {
    let Ok(counter) = changed.single() else {
        return; // no change this frame (or no counter yet)
    };
    if let Ok(mut text) = label.single_mut() {
        let next = counter.value.to_string();
        if text.0 != next {
            text.0 = next; // set_if_neq-style guard, by hand
        }
    }
}
