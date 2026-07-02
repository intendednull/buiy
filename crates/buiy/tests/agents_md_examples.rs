//! Track C/D — CI gate for the `AGENTS.md` code snippets. Compiling this file
//! proves the agent front-door doc stays accurate as the API evolves (drift in a
//! doc an LLM reads poisons generation). It also proves the doc's central claim:
//! every snippet is authored from **`use buiy::prelude::*;` (+ `buiy::probe::*`)
//! ALONE** — no second `use bevy::prelude::*;`.
//!
//! Snippets are compiled (as functions / a headless build), not run — the point
//! is that the API shapes the doc shows are real and reachable from the prelude.

#![allow(dead_code)]

use buiy::prelude::*;

// ── "One import" — the quick-start setup system ───────────────────────────────
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Button::new("Save"));
    commands.spawn(Checkbox::new("Dark mode").checked(true));
}

// ── State: MVU ────────────────────────────────────────────────────────────────
#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter {
    value: i64,
}

#[derive(Clone, Debug, PartialEq, Reflect)]
enum CounterMsg {
    Increment,
    Reset,
}

impl Model for Counter {
    type Msg = CounterMsg;
}

fn update(m: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => m.value += 1,
        CounterMsg::Reset => m.value = 0,
    }
    Cmd::none()
}

fn send_a_message(mut commands: Commands, model: Query<Entity, With<Counter>>) {
    for e in &model {
        enqueue::<Counter>(&mut commands, e, CounterMsg::Increment);
    }
}

// ── Widgets: every constructor form the doc shows ─────────────────────────────
fn spawn_the_catalog(mut commands: Commands) {
    commands.spawn(Checkbox::new("Dark mode").checked(true));
    commands.spawn(Checkbox::new("Tri").indeterminate(true));
    commands.spawn(Switch::new("Wi-Fi").on(true));
    commands.spawn(Disclosure::new("Details").expanded(true));
    commands.spawn(Slider::new("Volume", 0.5, 0.0, 1.0, 0.1));
    commands.spawn(Button::new("Save"));
    commands.spawn(TextInput::single_line("Search…"));
}

// ── Reading state: the domain accessors (no `accesskit::Toggled`) ─────────────
fn read_state(
    boxes: Query<&A11yToggled, With<Checkbox>>,
    switches: Query<&A11yToggled, With<Switch>>,
    sliders: Query<&A11yValue, With<Slider>>,
    disclosures: Query<&A11yExpanded, With<Disclosure>>,
    inputs: Query<&A11yTextValue, With<TextInput>>,
) {
    let _checked = boxes.iter().filter(|t| Checkbox::checked(t)).count();
    let _on = switches.iter().filter(|t| Switch::on(t)).count();
    for v in &sliders {
        let _ = (
            Slider::value(v),
            Slider::min(v),
            Slider::max(v),
            Slider::fraction(v),
        );
    }
    for e in &disclosures {
        let _ = Disclosure::expanded(e);
    }
    for v in &inputs {
        let _ = TextInput::value(v);
    }
}

// ── Reacting: typed ValueChange + OnPress ─────────────────────────────────────
fn on_toggle(mut changes: MessageReader<ValueChange<bool>>) {
    for c in changes.read() {
        let _ = (c.source, c.value, c.is_final);
    }
}

fn on_slide(mut changes: MessageReader<ValueChange<f64>>) {
    for c in changes.read() {
        let _ = c.value;
    }
}

fn on_press(mut presses: MessageReader<OnPress>) {
    for OnPress(entity) in presses.read() {
        let _ = entity;
    }
}

// ── Theming: the closed-enum ColorToken ───────────────────────────────────────
fn theming() {
    let _a = Background {
        color: ColorToken::SurfacePrimary,
    };
    let _b = Background {
        color: ColorToken::Custom(Color::srgb(0.2, 0.5, 0.95)),
    };
    let _c = ColorToken::Accent;
}

// ── The feedback loop — `buiy::probe` (GPU-free) ──────────────────────────────
#[test]
fn probe_loop_from_the_prelude_alone() {
    use buiy::probe::*;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(BuiyProbePlugin);

    app.world_mut().spawn(Checkbox::new("Dark mode"));
    for _ in 0..8 {
        app.update();
    }

    let report = snapshot_report(app.world_mut());
    assert!(report.contains("Checkbox \"Dark mode\""));

    let cb = get_by_role(app.world_mut(), A11yRole::Checkbox, Some("Dark mode"), None).unwrap();
    click(app.world_mut(), cb).unwrap();
    app.update(); // the toggle commits through the funnel on the next step
    // The drive step flipped the state — exactly what the doc claims.
    let after = snapshot_report(app.world_mut());
    assert!(after.contains("[checked]"));
}

/// The MVU build snippet (compile-only; `.run()` would need a display).
#[test]
fn mvu_build_compiles() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .mvu_model(update)
        .app()
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (send_a_message, read_state, on_toggle, on_slide, on_press),
        );
    // Register the theming/catalog snippets so they are not dead-code-eliminated
    // before the type-check that gates the doc.
    let _ = (theming, spawn_the_catalog);
}
