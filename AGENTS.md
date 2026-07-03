# AGENTS.md — authoring Buiy UIs with an LLM

Buiy is an accessible, web-quality UI library for the [Bevy](https://bevyengine.org)
game engine — a **parallel UI stack to `bevy_ui`** (its own CSS-subset layout,
complex text, a wgpu render pipeline, an AccessKit semantic tree, and an Elm-style
MVU state funnel). This file is the front door for a coding agent: the one-import
surface, the widget/state/theme APIs, the **headless feedback loop that lets you
*see* what you built**, and the gotchas that are otherwise silent-wrong.

> Status: pre-0.1, pre-alpha — APIs may break in any commit. This file tracks the
> current surface; when in doubt, trust `cargo` and the doctests, not memory.

## One import

`use buiy::prelude::*;` is **self-sufficient** — it brings the Buiy surface AND a
curated set of Bevy ECS essentials (`Component`/`Commands`/`Query`/`Res`/
`MessageReader`/`With`/`Camera2d`/`App`/`Startup`/`Update`/`Reflect`/… + the MVU
funnel). You can define components *and wire systems* from this one import; you do
**not** need a second `use bevy::prelude::*;` (and shouldn't — that reintroduces a
`Text`/`Node` name collision the prelude is designed to avoid).

```rust
use buiy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)          // installs the whole Buiy stack
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Button::new("Save"));  // focusable, accessible; emits `OnPress`
    commands.spawn(Checkbox::new("Dark mode").checked(true));
}
```

**Plugins** (compose one): `BuiyPlugin` (full, windowed) · `BuiyHeadlessPlugin`
(no window, keeps render for offscreen capture) · `BuiyProbePlugin` (GPU-free, no
window/adapter — for the feedback loop below). **Widgets only render fully under a
Buiy plugin** — their visible children are attached by `WidgetsPlugin`'s observers,
not at spawn (see Gotchas).

## State: MVU (the primary interface)

Widget/app state flows through a Model-View-Update funnel. A model is a
`Component` implementing `Model`; the reducer is a pure `fn(&mut Model, Msg) ->
Cmd<Msg>`; register with `.mvu_model(reducer)`.

```rust
use buiy::prelude::*;

#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter { value: i64 }

#[derive(Clone, Debug, PartialEq, Reflect)]
enum CounterMsg { Increment, Reset }

impl Model for Counter { type Msg = CounterMsg; }

fn update(m: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => m.value += 1,
        CounterMsg::Reset => m.value = 0,
    }
    Cmd::none()
}

fn build(app: &mut App) {
    app.add_plugins(BuiyPlugin)
        .mvu_model(update)  // register_type + add_model + add_reducer, one call
        .app();             // ModelWiring handle → &mut App (remember to escape it)
}
```

Send messages with `enqueue::<Counter>(&mut commands, model_entity, CounterMsg::Increment)`.
See `examples/counter_view` and `examples/todomvc`. There is also a declarative
`view(&Model) -> Element<Msg>` surface in `buiy::view` (`examples/counter_view`,
`examples/todomvc_view`).

For a **time-driven** model (countdowns, animations, game loops), add
`ClockPlugin::<M>::new(Msg::Tick)` — a poll clock that folds `Msg::Tick(now)` every frame.
Derive from `now` and store only the *derived* value (never `now` itself), so an idempotent
steady frame is absorbed by `set_if_neq`. Drive it deterministically in a headless test with
`advance_clock(&mut app, delta)` (no real sleeps).

## Widgets

Catalog: `Button` · `Checkbox` · `Switch` · `Slider` · `Disclosure` · `Dialog` ·
`TooltipTrigger` · `Popover` · `Menu`/`MenuButton`/`MenuItem` · `ScrollArea` ·
`TextInput`.

**Constructors** are `Widget::new(...)`, spawned with `commands.spawn(...)`. The
**setter-bearing** widgets return a named builder with chainable setters that
store the real state:

```rust
Checkbox::new("Dark mode").checked(true)       // or .indeterminate(true)
Switch::new("Wi-Fi").on(true)
Disclosure::new("Details").expanded(true)
Slider::new("Volume", /*now*/ 0.5, /*min*/ 0.0, /*max*/ 1.0, /*step*/ 0.1)
Button::new("Save")
TextInput::single_line("Search…")              // also ::multi_line
```

Widgets without scalar state (`Dialog::new(title, body)`, `MenuItem::new(label)`,
etc.) return `impl Bundle` — no chained setters; just spawn them.

**Reading state** — use the domain accessors; you never touch the foreign
`accesskit::Toggled` enum. Query the state component (all preluded) alongside the
widget marker:

```rust
fn read(boxes: Query<&A11yToggled, With<Checkbox>>) {
    let checked = boxes.iter().filter(|t| Checkbox::checked(t)).count();
}
// also: Switch::on(&A11yToggled), Slider::value/min/max/fraction(&A11yValue),
//       Disclosure::expanded(&A11yExpanded), TextInput::value(&A11yTextValue)
```

**Reacting to changes** — read the typed `ValueChange<T>` message (not the untyped
`OnPress` sink):

```rust
fn on_toggle(mut changes: MessageReader<ValueChange<bool>>) {   // <f64> for Slider
    for c in changes.read() { println!("{:?} -> {}", c.source, c.value); }
}
```

`OnPress(entity)` is the shared activation message every button/widget emits; read
it with `MessageReader<OnPress>` for click/activate handling.

## Theming: typed color tokens

Colors are a **closed enum** `ColorToken`, resolved through a `ThemeContract` — a
bad color is a compile error, not a silent fallback. Use the semantic variants
(never strings):

```rust
Background { color: ColorToken::SurfacePrimary }   // or SurfaceCard, Accent, TextPrimary, FocusRing, …
ColorToken::Custom(Color::srgb(0.2, 0.5, 0.95))    // an explicit color
ColorToken::SystemColor(/*forced-colors keyword*/) // Canvas, ButtonText, …
```

`Theme` carries a palette + a `PaletteMode` (`Normal` / `ForcedColors`). Widgets
carry theme-safe defaults; recolor the theme via `SetAccent` / the `Theme` resource.

## The feedback loop — SEE what you built (no GPU)

This is the highest-leverage tool for an agent: run a scene **headless + GPU-free**
and read a **stable, diffable semantic tree** (roles / names / state / layout
rects / text) — the compiler-plus-eyes loop. Everything is in `buiy::probe`.

```rust
use buiy::prelude::*;
use buiy::probe::*;

let mut app = App::new();
app.add_plugins(MinimalPlugins)
    .add_plugins(bevy::asset::AssetPlugin::default())
    .add_plugins(bevy::input::InputPlugin)
    .add_plugins(BuiyProbePlugin);          // GPU-free: no window, no wgpu adapter

app.world_mut().spawn(Checkbox::new("Dark mode"));
for _ in 0..8 { app.update(); }             // let layout + a11y + text settle

println!("{}", snapshot_report(app.world_mut()));  // the Playwright-style tree
// Drive it:
let cb = get_by_role(app.world_mut(), A11yRole::Checkbox, Some("Dark mode"), None).unwrap();
click(app.world_mut(), cb).unwrap();
app.update();                               // the toggle commits on the next step
// snapshot_report(app.world_mut()) now shows [checked]
```

`snapshot_report` prints each node as `Role "name" [state] @x,y wxh` plus a `---
text & layout ---` section that surfaces plain (role-less) text and flags
**zero-size / invisible** content. Reference loop: **`cargo run -p buiy_probe`**
(edit its `scene()`, run, read the tree). Drivers: `snapshot` / `snapshot_report`
/ `get_by_role` / `click` / `focus` / `set_value` / `wait_for` / `perform`.

## Gotchas (these are silent-wrong otherwise)

- **Widgets need a Buiy plugin.** A widget's visible children (label, checkbox
  mark, switch thumb) attach via `WidgetsPlugin` observers, NOT at spawn. Spawn
  widgets only under `BuiyPlugin` / `BuiyHeadlessPlugin` / `BuiyProbePlugin` — a
  plugin-less app gets label-less (invisible) widgets.
- **Don't single-field-patch a `#[require]`'d component on a bare marker.**
  `bsn! { Button BoxModel { width: … } }` *suppresses* the widget's `BoxModel`
  initializer and silently drops its other defaults (the "§4.1c trap"). Style via
  the builders (`commands.spawn`) or the `bsn!` **scene-fns** (`button("Save")`,
  `checkbox("…")`, `slider(…)`), which merge-patch correctly.
- **`bsn!` uses the scene-fns, not the builders.** A builder (`Checkbox::new(..)`)
  is a spawn-time `impl Bundle` for `commands.spawn`; it can't be field-merged in
  a `bsn!` block. `bsn! { button("Save") BoxModel { width: … } }` uses the scene-fn.
- **Don't co-spawn a component the builder already carries.** `spawn((Checkbox::new(""),
  A11yToggled(..)))` panics (duplicate component) — the builder carries the state;
  use `.checked(..)` to seed it.
- **`use buiy::prelude::*;` is enough** — adding `use bevy::prelude::*;` alongside
  reintroduces the `Text`/`Node` collision.
- **`ColorToken` is a closed enum** — pick a variant; there are no string tokens.

## Verify before claiming done

- Read it: `cargo run -p buiy_probe` (or a `snapshot_report` in a test) — confirm
  the roles/names/state/text are what you intended, and nothing is `[ZERO-SIZE]`.
- Build/lint/test: `cargo fmt --all -- --check && cargo clippy --workspace
  --all-targets --locked -- -D warnings && cargo test --workspace --locked`
  (headless; render GPU tests are a separate `--ignored` lane, see `CLAUDE.md`).

## Worked examples (all compile in CI)

- `examples/buiy_probe` — the agent feedback loop (author → run → inspect → drive).
- `examples/hello_button` — MVU counter (`Model` + reducer + press route + bind).
- `examples/counter_view` / `examples/todomvc_view` — the declarative `buiy::view`
  surface (`view(&Model) -> Element<Msg>`).
- `examples/todomvc` — a fuller MVU app.
- `examples/buiy_gallery` — the widget catalog (every widget, real interaction).
- `examples/hello_text`, `examples/hello_bsn` — text stack, `bsn!` authoring.

## Deeper docs

Start at `docs/README.md` (the master index of specs / plans / reports / prior-art).
The LLM-development-support design is
`docs/specs/2026-07-01-first-class-llm-dev-support-design.md`. Project conventions
for human + agent contributors are in `CLAUDE.md`.
