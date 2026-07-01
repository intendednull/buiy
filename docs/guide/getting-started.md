# Building a UI with Buiy

**Date:** 2026-06-30
**Status:** `[active]`
**Audience:** users building a UI with Buiy (not internals contributors).

A task-ordered walkthrough: add the plugin, spawn widgets, lay them out, react to input,
theme, author declaratively, and hold state. It complements the [`README.md`](../../README.md)
quick start and the runnable [`examples/`](../../examples/); the *why* behind these APIs lives
in the design specs indexed by [`docs/README.md`](../README.md).

> Buiy is pre-0.1, pre-alpha, and largely unreviewed (see the note atop the README). APIs
> break in any commit.

## 0. Add Buiy

```toml
[dependencies]
bevy = "0.19"
buiy = { git = "https://github.com/intendednull/buiy" }
```

On Linux, Bevy needs the usual system libraries (plus `at-spi2-core` for the AccessKit bridge):

```sh
sudo apt-get install -y libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev at-spi2-core
```

Add `BuiyPlugin` **after** `DefaultPlugins`. It composes layout, render, text + editing, a11y,
focus, picking, the widget catalog, and the MVU state funnel.

```rust
use bevy::prelude::*;
use buiy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)
        .add_systems(Startup, setup)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d); // Buiy paints into a 2D camera's view
}
```

`use buiy::prelude::*;` brings the components, widgets, and the `bsn!` macros into scope in one
import. (`use buiy::*;` re-exports the same flat set — the `hello_*` examples use that form.)

## 1. Your first widget: a Button

`Button::new(label)` returns a ready bundle — marker + `Node` + `Style` + `Background`/`Border` +
`Focusable` + `A11yRole::Button` + `A11yLabel` — and emits `OnPress` (a Bevy `Message`) on
pointer, keyboard, or assistive-tech activation.

```rust
fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Button::new("Save"));
}

fn on_press(mut presses: MessageReader<OnPress>) {
    for OnPress(entity) in presses.read() {
        info!("pressed: {entity:?}");
    }
}
```

Register `on_press` with `.add_systems(Update, on_press)`. Runnable version:
`cargo run -p hello_button`.

## 2. Layout with the `Style` builder

`Style` is a fluent builder — the authoring front-end to Buiy's decomposed layout components. It
covers the CSS box: flex, grid, box model, positioning, writing modes, and more.

```rust
let card = commands
    .spawn((
        Node,
        Style::default()
            .flex_column()
            .width_px(320.0)
            .padding(24.0)
            .gap_px(12.0),
        Background { color: ColorToken::Token("color.surface.primary".into()) },
        Border { radius: Corners::all(Radius::circular(12.0)), ..default() },
    ))
    .add_children(&[/* header, body, buttons … */])
    .id();
```

Common setters: `.flex_row()` / `.flex_column()` / `.grid()`, `.width_px()` / `.height_px()`,
`.padding()` / `.gap_px()`, `.justify_content(JustifyContent::…)` / `.align_items(AlignItems::…)`,
`.absolute()` / `.inset(…)`. Discover the rest via rustdoc / IDE completion on `Style`.

## 3. Text

Spawn a `Text` node; it shapes through cosmic-text into the glyph atlas. `TextColor` defaults to
the theme foreground.

```rust
commands.spawn((
    Node,
    Style::default(),
    Text(String::from("Hello, Buiy!")),
    FontSize(24.0),
));
```

Runnable: `cargo run -p hello_text` (a title over a wrapped paragraph).

## 4. The widget catalog

Beyond `Button` and `TextInput`, `buiy_widgets` ships accessible, keyboard-driven Checkbox,
Switch, Slider, Disclosure, Dialog, Tooltip, Popover, Menu, and ScrollArea (plus composites:
progress meter, table rows, search input, kbd chip, status dot). Each is authorable **three ways**:

| Way | Looks like | Use when |
|---|---|---|
| Bundle constructor | `commands.spawn(Checkbox::new("Wireframe"))` | imperative spawning |
| `bsn!` scene-fn | `bsn! { checkbox("Wireframe") }` | declarative trees (§6) |
| Raw decomposed components | `(Node, Style…, Background…, A11yRole::Checkbox, …)` | full control / custom widgets |

> **Gotcha (the §4.1c trap):** widgets carry `#[require(...)]` component contracts. Style them
> via the parameterized scene-fns (`button("…")`, `text_input_single_line("…")`), **never** by
> patching a single field of a `#[require]`'d component — a single-field patch drops the widget's
> other defaults. See [`hello_bsn`](../../examples/hello_bsn) and the BSN spec.

The [`buiy_gallery`](../../examples/buiy_gallery) example (`cargo run -p buiy_gallery`) is a live
tour of the whole catalog across five screens.

## 5. Reacting to input

Widgets emit `OnPress` (Button, MenuItem, …) and observe the `bevy_picking` `Pointer<E>` event
family (`Pointer<Click>`, `Pointer<Over>`, …). Focus is ordered Tab / Shift-Tab with
`:focus-visible`; spawn a widget with `Focusable` (the widget bundles already include it) to make
it keyboard-reachable.

## 6. Declarative authoring with `bsn!`

`use buiy::prelude::*;` brings Bevy 0.19's `bsn!` / `bsn_list!` scene macros into scope. Author
the decomposed components directly, and use the widget **scene-fns** for widgets:

```rust
commands.spawn_scene(bsn! {
    Node
    Style::default() // (author the decomposed components; `Style` is a Bundle builder)
    [
        button("Save")
        checkbox("Wireframe")
    ]
});
```

Runnable: `cargo run -p hello_bsn`. (The `.bsn` asset-file loader + hot-reload are deferred; see
the BSN integration spec.)

## 7. Theming

Buiy resolves paint through **theme tokens** (`ColorToken::Token("color.surface.primary")`), so a
theme swap never touches the atlas. Start from a bundled theme and set it as a resource:

```rust
use buiy::prelude::*;
app.insert_resource(default_light_theme()); // or default_dark_theme()
```

- **Dark / light / accent:** `default_dark_theme()` ships a live accent ramp; drive a runtime
  accent change with the `SetAccent` message.
- **OS preferences:** `UserPreferences` (dark / reduced-motion / forced-colors / …) drives theme
  selection; the forced-colors path is statically verified to keep every default-catalog paint
  legible under high contrast.

## 8. State with MVU

Widget state flows through Buiy's **Model-View-Update** funnel: an actor declares a `Model` and
its `Msg`; a pure reducer folds messages; handlers `enqueue` (never mutate directly); one ordered
drain is the sole writer; a bind projects the folded model into the view. This is what makes state
recordable and replayable.

MVU currently lives in `buiy_core::mvu` and is **not yet re-exported through `buiy`** (a known
ergonomics gap — see the [coverage audit](../reports/2026-06-30-documentation-coverage-audit.md)),
so an app authoring its own model depends on `buiy_core` directly:

```toml
buiy_core = { path = "…/crates/buiy_core" } # or the git dep
```

```rust
use buiy_core::mvu::{Cmd, Model, MvuModelExt, MvuSet, enqueue};

#[derive(Component, Clone, PartialEq, Reflect, Default)]
#[reflect(Component)]
struct Counter { value: i32 }

#[derive(Clone, Debug, Reflect, PartialEq)]
enum CounterMsg { Increment, Add(i32) }

impl Model for Counter { type Msg = CounterMsg; }

fn update(counter: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => counter.value += 1,
        CounterMsg::Add(n) => counter.value += n,
    }
    Cmd::none()
}

// In main(), after BuiyPlugin (which already installs MvuCorePlugin via the widgets):
app.mvu_model(update);
// A handler enqueues in the enqueue window so it folds the same frame:
app.add_systems(Update, (my_handler.in_set(MvuSet::Enqueue), my_bind));
```

The full, runnable end-to-end version (a `+ / −` counter wired to buttons and a live label) is
[`examples/hello_mvu`](../../examples/hello_mvu) — `cargo run -p hello_mvu`. See the
[MVU design](../specs/2026-06-29-mvu-as-core-design.md) for tiers (leaf / machine / raw-ECS),
record/replay, and the `enqueue`-not-fold rule.

## 9. Testing your UI

Buiy treats verification as a deliverable. For visual / layout / render / a11y tests, follow the
verification how-to (the `using-buiy-verification` skill; mirrors
[the design spec](../specs/2026-06-15-buiy-verification-design/README.md)): pick the lowest tier
that observes the bug (layout snapshot → display-list → invariant → reftest → golden), plus the
a11y-tree and contrast headless gates and the live-interaction tier. The same AccessKit tree that
serves screen readers also backs an in-process test/agent driver.

## Where to go next

- **Examples:** [`examples/`](../../examples/) — `hello_button`, `hello_text`, `hello_bsn`,
  `hello_mvu`, and the `buiy_gallery` reference app (plus `buiy_web` / `gallery_web` for WebGPU).
- **API reference:** the crate rustdoc (start at the `buiy` crate root).
- **Design + architecture:** [`docs/README.md`](../README.md) — specs, plans, reports, prior-art.
