**Date:** 2026-07-01
**Status:** design-options (prototype seed)

> Design-selection artifact for the `safer-v-authoring` prototype: 5 authoring paradigms as comparable code (same Counter+Todo), each grounded in prior-art + the MVU substrate. Feeds the prototype decision (paradigm ② = the Elm/iced view-function — see the [LLM panel](2026-07-01-view-declaration-llm-panel.md)).

# What could writing a Buiy app look like? — 5 authoring paradigms, side by side

## The setup

Every snippet below sits on the same fixed foundation: Buiy's **MVU-as-core** substrate (`buiy_core::mvu`), where a `Model` is a Component (+Reflect+Clone+PartialEq) with an associated `Msg`, a free-fn reducer `fn update(&mut Model, Msg) -> Cmd<Msg>` folds messages through a single `enqueue` ingress and one ordered `set_if_neq` drain, and a11y is a derived output of state. The five paradigms differ only in the **app-author-facing authoring surface** they put on top of that substrate — none of them redesign the funnel. To make them honestly comparable, each one implements the identical two apps: **Counter** (an `i32` with −/+/Reset, Reset disabled at 0) and **Todo** (a themed card with a bound draft input, a keyed dynamic list of checkbox rows, and a derived "N items left" footer).

## Example A — Counter, in each paradigm

### bsn!-native MVU — one tree where buttons say `on_press(Msg::Inc)`

```rust
use bevy::prelude::*;
use buiy::prelude::*; // brings `view!`, `bind!`, `col`/`row`/`button`/`text`, `Cmd`, `add_mvu_ui`, `tokens`

// ── Model ────────────────────────────────────────────────────────────────
// `#[derive(Model)]` emits `impl buiy_core::mvu::Model for Counter { type Msg = CounterMsg; }`.
// The other derives are the substrate's real bounds (Component + Reflect + Clone + PartialEq).
#[derive(Model, Component, Reflect, Clone, PartialEq, Default)]
#[reflect(Component)]
#[model(msg = CounterMsg)]
struct Counter { count: i32 }

// `#[derive(Msg)]` = the Reflect/FromReflect/TypePath/GetTypeRegistration bundle the
// `Model::Msg` bound requires, so the message logs + replays.
#[derive(Msg, Clone, Debug, Reflect)]
enum CounterMsg { Inc, Dec, Reset }

// ── Update (the real free-fn reducer; env-free ⇒ purity is structural) ─────
fn update(m: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Inc   => m.count += 1,
        CounterMsg::Dec   => m.count -= 1,
        CounterMsg::Reset => m.count = 0,
    }
    Cmd::none()
}

// ── View (ONE tree; `Counter` at the root roots the model + reducer) ───────
fn view() -> impl Scene {
    view! {
        Counter                              // seeds Counter::default(); owns the subtree's reducer
        col().gap(8.0).padding(tokens::space::md)
        Children [
            text(bind!("Count: {}", m.count)),          // derived label
            row().gap(8.0) Children [
                button("−")     on_press(CounterMsg::Dec),
                button("+")     on_press(CounterMsg::Inc),
                button("Reset") on_press(CounterMsg::Reset)
                                disabled(bind!(m.count == 0)),  // derived bool
            ],
        ]
    }
}

fn main() {
    App::new()
        .add_plugins(BuiyPlugins)
        // Infers M = Counter from `update`; registers mvu_model(update) +
        // route_on_press::<Counter> + apply_derived::<Counter>, and spawns `view()`.
        .add_mvu_ui(update, view)
        .run();
}
```

The state lives on the entity named at the tree root; typed messages ride on the widgets as `on_press(...)` attributes, and `add_mvu_ui(update, view)` is the entire wiring — the macro emits the routing and derive systems.

### Elm / iced rebuilding view-fn — `view(&Model) -> Element<Msg>`

```rust
use buiy::prelude::*;   // Model, Cmd, Element, app, tokens, and the column!/row!/text! macros
use buiy::widget::*;    // button, text, column, row, container, checkbox, text_input …

#[derive(Model, Default)]
#[model(msg = Msg)]
struct Counter {
    count: i32,
}

#[derive(Clone)]
enum Msg {
    Inc,
    Dec,
    Reset,
}

fn update(state: &mut Counter, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::Inc   => state.count += 1,
        Msg::Dec   => state.count -= 1,
        Msg::Reset => state.count = 0,
    }
    Cmd::none()
}

fn view(state: &Counter) -> Element<Msg> {
    column![
        text!("Count: {}", state.count),
        row![
            button("−").on_press(Msg::Dec),
            button("+").on_press(Msg::Inc),
            // on_press_maybe(None) renders a disabled button — Reset is dead at 0.
            button("Reset").on_press_maybe((state.count != 0).then_some(Msg::Reset)),
        ]
        .gap(SpaceToken::Sm),
    ]
    .gap(SpaceToken::Md)
    .into()
}

fn main() {
    buiy::app(Counter::default(), update, view).run();
}
```

The view is a pure function returning a description tree that a keyed reconciler diffs onto the retained ECS; disabled state is expressed as the absence of a message via `on_press_maybe`.

### Jetpack-Compose controlled widgets — `view(&Model, &mut Ui)` with a Modifier

```rust
use buiy::compose::prelude::*;

#[derive(Component, Clone, PartialEq, Reflect, Default)]
struct Counter { count: i32 }

#[derive(Clone)]
enum Msg { Inc, Dec, Reset }

impl Model for Counter {
    type Msg = Msg;
    // Lowers to the spec's free-fn reducer `fn update(&mut Counter, Msg) -> Cmd<Msg>`.
    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Inc   => self.count += 1,
            Msg::Dec   => self.count -= 1,
            Msg::Reset => self.count = 0,
        }
        Cmd::none()
    }
}

// value flows DOWN (read from the Model); typed Msgs flow UP (returned by on_press).
fn view(m: &Counter, ui: &mut Ui) {
    let t = ui.theme();
    ui.column(Modifier::new().gap(t.space_3), |ui| {
        ui.text(format!("Count: {}", m.count));
        ui.row(Modifier::new().gap(t.space_2), |ui| {
            ui.button("−", || Msg::Dec);
            ui.button("+", || Msg::Inc);
            ui.button("Reset", || Msg::Reset).enabled(m.count != 0);
        });
    });
}

fn main() {
    App::new().add_plugins(BuiyUi).run_mvu(Counter::default, view);
}
```

Widgets are emitted by calling builder methods on `&mut Ui`; each takes an `on_change`/`on_press` closure returning a typed `Msg`, and styling rides as a `Modifier` value passed first.

### SwiftUI value-views — `fn view(&Model) -> impl View<Msg>` + modifier chains

```rust
use buiy::prelude::*;

// ---- STATE -----------------------------------------------------------------
#[derive(Model, Default, Clone, PartialEq, Reflect)]
struct Counter { count: i32 }

#[derive(Msg, Clone, Debug, Reflect)]
enum Msg { Dec, Inc, Reset }

impl Update for Counter {
    type Msg = Msg;
    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::Dec   => self.count -= 1,
            Msg::Inc   => self.count += 1,
            Msg::Reset => self.count  = 0,
        }
        Cmd::none()
    }
}

// ---- VIEW ------------------------------------------------------------------
fn view(m: &Counter) -> impl View<Msg> {
    VStack((
        Text(format!("Count: {}", m.count)).font(Font::Title),
        HStack((
            Button("−").action(Msg::Dec),
            Button("+").action(Msg::Inc),
            Button("Reset").action(Msg::Reset).disabled(m.count == 0),
        )).gap(Space::S2),
    ))
    .gap(Space::S3)
    .padding(Space::S4)
}

// ---- APP -------------------------------------------------------------------
fn main() {
    App::new()
        .add_plugins(BuiyPlugins)
        .ui(Counter::default(), view)   // spawns the model root + reducer + reconciler
        .run();
}
```

Views are inert values composed by nesting tuples; typed events attach as `.action(Msg)` and styling as an order-independent modifier chain, both on the same value.

### Fine-grained signals — the road not taken; `signal(0)` cells + effects

```rust
use buiy::prelude::*;
use buiy::signal::{signal, view, For};   // the reactive island: signal(), memo(), view!, For

// The setup fn runs ONCE. It wires the signal graph; it is NOT a per-frame render.
fn counter_app() -> impl View {
    let (count, set_count) = signal(0i32);   // (ReadSignal, WriteSignal) — a read-cap + a write-cap

    view! {
        Column.gap(tokens::space::MD) [
            // reactive text: this closure is an effect; it re-runs ONLY when `count` changes
            Text(move || format!("Count: {}", count.get())),
            Row.gap(tokens::space::SM) [
                Button("−").on_press(move || set_count.update(|c| *c -= 1)),   // typed handler, no OnPress(Entity)
                Button("+").on_press(move || set_count.update(|c| *c += 1)),
                Button("Reset")
                    .disabled(move || count.get() == 0)                       // reactive prop = another effect
                    .on_press(move || set_count.set(0)),
            ],
        ]
    }
}

fn main() {
    App::new()
        .add_plugins((BuiyPlugins, SignalRuntimePlugin))  // SignalRuntimePlugin = the ONE exclusive-&mut-World system
        .mount(counter_app)                               // runs the setup fn once, installs the graph
        .run();
}
```

State is a signal cell captured by closures; the setup fn runs once, and each `move || sig.get()` becomes an effect that re-runs only when the read signal changes — no Msg enum, no reducer.

## Example B — Todo, in each paradigm

### bsn!-native MVU — `for_each(bind!(m.todos), key:, row:)` inside the tree

```rust
use bevy::prelude::*;
use buiy::prelude::*;

// ── Domain + Model ─────────────────────────────────────────────────────────
#[derive(Clone, PartialEq, Reflect)]
struct Todo { id: u64, text: String, done: bool }

#[derive(Model, Component, Reflect, Clone, PartialEq, Default)]
#[reflect(Component)]
#[model(msg = TodoMsg)]
struct TodoApp { todos: Vec<Todo>, draft: String, next_id: u64 }

#[derive(Msg, Clone, Debug, Reflect)]
enum TodoMsg {
    DraftChanged(String),   // fn(String) -> TodoMsg — used directly as the on_input adapter
    Add,
    Toggle(u64),            // per-row key baked into the payload
    ClearCompleted,
}

// ── Update ─────────────────────────────────────────────────────────────────
fn update(m: &mut TodoApp, msg: TodoMsg) -> Cmd<TodoMsg> {
    match msg {
        TodoMsg::DraftChanged(s) => m.draft = s,
        TodoMsg::Add => {
            let text = m.draft.trim().to_string();
            if !text.is_empty() {
                m.todos.push(Todo { id: m.next_id, text, done: false });
                m.next_id += 1;
                m.draft.clear();               // reducer clears draft ⇒ bind re-seeds the input
            }
        }
        TodoMsg::Toggle(id) => if let Some(t) = m.todos.iter_mut().find(|t| t.id == id) {
            t.done = !t.done;
        },
        TodoMsg::ClearCompleted => m.todos.retain(|t| !t.done),
    }
    Cmd::none()
}

// ── View ───────────────────────────────────────────────────────────────────
fn view() -> impl Scene {
    view! {
        TodoApp
        card()                                             // themed surface (F6 typed tokens)
            .bg(tokens::color::surface::card)
            .padding(tokens::space::lg)
            .radius(tokens::radius::md)
        Children [
            text("Todos").font_size(20.0),
            row().gap(8.0) Children [
                text_input(bind!(m.draft))                 // two-way: reads draft, writes via on_input
                    placeholder("What needs doing?")
                    on_input(TodoMsg::DraftChanged)
                    on_submit(TodoMsg::Add),               // Enter
                button("Add") on_press(TodoMsg::Add),
            ],
            col().gap(4.0) Children [
                // Keyed reconcile — the crux of F7. No hand-rolled reconciler, no marker sprawl.
                for_each(bind!(m.todos), key: |t| t.id, row: |t| view! {
                    row().gap(8.0) Children [
                        checkbox(bind!(t.done)) on_toggle(TodoMsg::Toggle(t.id)),  // controlled
                        text(bind!("{}", t.text)),
                    ]
                }),
            ],
            row().gap(8.0).justify_between() Children [
                // Derived footer — recomputed from the model at the Bind stage.
                text(bind!("{} items left", m.todos.iter().filter(|t| !t.done).count())),
                button("Clear completed") on_press(TodoMsg::ClearCompleted),
            ],
        ]
    }
}

fn main() {
    App::new()
        .add_plugins(BuiyPlugins)
        .add_mvu_ui(update, view)   // registers model+reducer + route/derive/reconcile systems for TodoApp
        .run();
}
```

The dynamic list is `for_each(bind!(m.todos), key: |t| t.id, row: |t| ...)` — a keyed reconciler emitted by the macro — and the derived count is a `bind!` expression over the whole model, recomputed at the Bind stage.

### Elm / iced rebuilding view-fn — `.iter().map()` into `column(iter)`

```rust
use buiy::prelude::*;
use buiy::widget::*;

#[derive(Reflect, Clone, PartialEq)]
struct Todo {
    id: u64,
    text: String,
    done: bool,
}

#[derive(Model, Default)]
#[model(msg = Msg)]
struct Todos {
    items: Vec<Todo>,
    draft: String,
    next_id: u64,
}

#[derive(Clone)]
enum Msg {
    Draft(String),      // per-keystroke edit of the input
    Add,                // Enter or the Add button
    Toggle(u64),        // a row's checkbox, by stable id
    ClearCompleted,
}

fn update(state: &mut Todos, msg: Msg) -> Cmd<Msg> {
    match msg {
        Msg::Draft(s) => state.draft = s,
        Msg::Add => {
            let text = state.draft.trim().to_string();
            if !text.is_empty() {
                state.items.push(Todo { id: state.next_id, text, done: false });
                state.next_id += 1;
                state.draft.clear();
            }
        }
        Msg::Toggle(id) => {
            if let Some(t) = state.items.iter_mut().find(|t| t.id == id) {
                t.done = !t.done;
            }
        }
        Msg::ClearCompleted => state.items.retain(|t| !t.done),
    }
    Cmd::none()
}

fn view(state: &Todos) -> Element<Msg> {
    let left = state.items.iter().filter(|t| !t.done).count();   // derived, pure, per-rebuild

    let rows = state.items.iter().map(|todo| {
        let id = todo.id;                                        // capture id, not &todo
        checkbox(&todo.text, todo.done)                          // label + checked driven by Model
            .on_toggle(move |_checked| Msg::Toggle(id))
            .key(id)                                             // stable identity for the reconciler
    });

    container(
        column![
            text("Todos").size(24.0),
            row![
                text_input("What needs doing?", &state.draft)    // value bound to the Model
                    .on_input(Msg::Draft)
                    .on_submit(Msg::Add),                        // Enter
                button("Add").on_press(Msg::Add),                // …or the button
            ]
            .gap(SpaceToken::Sm),
            column(rows).gap(SpaceToken::Xs),                    // the dynamic list
            row![
                text!("{left} items left"),
                button("Clear completed").on_press(Msg::ClearCompleted),
            ]
            .gap(SpaceToken::Md),
        ]
        .gap(SpaceToken::Md),
    )
    .padding(SpaceToken::Lg)          // typed token  (F6)
    .background(ColorToken::Surface)  // typed token  (F6)
    .rounded(RadiusToken::Md)         // typed token  (F6)
    .into()
}

fn main() {
    buiy::app(Todos::default(), update, view).run();
}
```

The list is ordinary `state.items.iter().map(...)` fed to `column(rows)` with `.key(id)`, and the derived count is a plain local `let left = ...count()` — no markers, no app-side systems, just Rust in the view fn.

### Jetpack-Compose controlled widgets — `for`-emit with `.key(id)`

```rust
use buiy::compose::prelude::*;

#[derive(Clone, PartialEq, Reflect)]
struct Todo { id: u64, text: String, done: bool }

#[derive(Component, Clone, PartialEq, Reflect, Default)]
struct Todos { items: Vec<Todo>, draft: String, next_id: u64 }

#[derive(Clone)]
enum Msg {
    DraftChanged(String),
    Add,
    Toggle(u64, bool),   // keyed by stable todo id, NOT list index
    ClearCompleted,
}

impl Model for Todos {
    type Msg = Msg;
    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::DraftChanged(s) => self.draft = s,
            Msg::Add => {
                let text = self.draft.trim();
                if !text.is_empty() {
                    self.items.push(Todo { id: self.next_id, text: text.into(), done: false });
                    self.next_id += 1;
                    self.draft.clear();
                }
            }
            Msg::Toggle(id, done) =>
                if let Some(t) = self.items.iter_mut().find(|t| t.id == id) { t.done = done },
            Msg::ClearCompleted => self.items.retain(|t| !t.done),
        }
        Cmd::none()
    }
}

fn view(m: &Todos, ui: &mut Ui) {
    let t = ui.theme();
    ui.card(Modifier::new()
        .background(t.surface)          // typed token — F6
        .padding(t.space_4)
        .rounded(t.radius_lg), |ui|
    {
        ui.text("Todos").style(Modifier::new().font(t.title));

        ui.row(Modifier::new().gap(t.space_2), |ui| {
            ui.text_input(&m.draft, Msg::DraftChanged)   // value in, Msg out (variant as Fn(String)->Msg)
              .placeholder("What needs doing?")
              .on_submit(|| Msg::Add);                    // Enter appends
            ui.button("Add", || Msg::Add);
        });

        // Dynamic list: plain `for`-emit, no markers, no tree-walk. (F7)
        for todo in &m.items {
            let id = todo.id;                              // owned copy — on_change must be 'static
            ui.row(Modifier::new().gap(t.space_2), |ui| {
                ui.checkbox(todo.done, move |done| Msg::Toggle(id, done));
                ui.text(&todo.text);
            }).key(id);                                    // stable key → keyed reconcile
        }

        // Derived "items left": a pure expression over the Model, recomputed each run.
        let left = m.items.iter().filter(|t| !t.done).count();
        ui.row(Modifier::new().gap(t.space_3), |ui| {
            ui.text(format!("{left} items left"));
            ui.button("Clear completed", || Msg::ClearCompleted);
        });
    });
}

fn main() {
    App::new().add_plugins(BuiyUi).run_mvu(Todos::default, view);
}
```

The list is a plain `for todo in &m.items { ui.row(...).key(id) }` emit, and the derived count is a pure expression before the footer row; `on_change` closures must capture owned `id` because they outlive the emit call.

### SwiftUI value-views — `ForEach(&items, |t| t.id, ...)` + `bind!`

```rust
use buiy::prelude::*;

// ---- STATE -----------------------------------------------------------------
#[derive(Model, Default, Clone, PartialEq, Reflect)]
struct Todos { items: Vec<Todo>, draft: String, next_id: u64 }

#[derive(Clone, PartialEq, Reflect)]
struct Todo { id: u64, text: String, done: bool }

#[derive(Msg, Clone, Debug, Reflect)]
enum Msg {
    DraftChanged(String),   // from the text field's two-way binding
    Add,                    // Enter (on_submit) or the Add button
    Toggle(u64),            // a row checkbox, addressed by stable id
    ClearCompleted,
}

impl Update for Todos {
    type Msg = Msg;
    fn update(&mut self, msg: Msg) -> Cmd<Msg> {
        match msg {
            Msg::DraftChanged(s) => self.draft = s,
            Msg::Add => {
                let text = std::mem::take(&mut self.draft);
                if !text.trim().is_empty() {
                    self.next_id += 1;
                    self.items.push(Todo { id: self.next_id, text, done: false });
                }
            }
            Msg::Toggle(id) => {
                if let Some(t) = self.items.iter_mut().find(|t| t.id == id) { t.done = !t.done; }
            }
            Msg::ClearCompleted => self.items.retain(|t| !t.done),
        }
        Cmd::none()
    }
}

// ---- VIEW ------------------------------------------------------------------
fn view(m: &Todos) -> impl View<Msg> {
    let left = m.items.iter().filter(|t| !t.done).count();   // derived — plain Rust
    VStack((
        Text("Todos").font(Font::Title),
        HStack((
            TextField("What needs doing?", bind!(m.draft => Msg::DraftChanged))
                .on_submit(Msg::Add),
            Button("Add").action(Msg::Add),
        )).gap(Space::S2),
        ForEach(&m.items, |t| t.id, |t| HStack((        // keyed by id
            Toggle(t.done).on_toggle(Msg::Toggle(t.id)),
            Text(&t.text),
        )).gap(Space::S2)),
        HStack((
            Text(format!("{left} items left")).foreground(Color::Muted),
            Spacer,
            Button("Clear completed").action(Msg::ClearCompleted),
        )),
    ))
    .gap(Space::S3)
    .padding(Space::S4)             // F6 typed token — padding
    .background(Surface::Card)      // F6 typed token — themed surface (bg+fg pair)
    .corner_radius(Radius::Lg)      // F6 typed token — rounded corners
}

// ---- APP -------------------------------------------------------------------
fn main() {
    App::new().add_plugins(BuiyPlugins).ui(Todos::default(), view).run();
}
```

`ForEach` takes the slice, a stable-key fn, and a per-item view builder; the derived count is a plain `let left` before the tree, and the input uses a `bind!(m.draft => Msg::DraftChanged)` two-way binding that lowers to `enqueue`.

### Fine-grained signals — keyed `For` + `memo`, per-row `RwSignal`

```rust
use buiy::prelude::*;
use buiy::signal::{RwSignal, memo, view, For};

// Per-row signal: `done` is its OWN RwSignal, so toggling a row does NOT touch the `todos` Vec.
// (This is the Leptos answer to the keyed-For value-change trap: wrap the mutable field in a signal.)
#[derive(Clone)]
struct Todo { id: usize, text: String, done: RwSignal<bool> }

fn todo_app() -> impl View {
    let todos   = RwSignal::new(Vec::<Todo>::new());
    let draft   = RwSignal::new(String::new());
    let next_id = RwSignal::new(0usize);

    let add = move || {
        let text = draft.get();
        if text.trim().is_empty() { return; }
        let id = next_id.get(); next_id.set(id + 1);
        todos.update(|v| v.push(Todo { id, text, done: RwSignal::new(false) }));
        draft.set(String::new());                          // clear the draft
    };

    // DERIVED (F7): a memo. It reads `todos` AND every row's `done` inside the filter, so it
    // dynamically subscribes to each row's signal; toggling any checkbox re-runs it ONCE, glitch-free.
    let items_left = memo(move || todos.with(|v| v.iter().filter(|t| !t.done.get()).count()));

    view! {
        Card                                               // F6: typed, compile-checked tokens
            .background(tokens::surface::CARD)             //   (no stringly HashMap, no magenta-on-typo)
            .padding(tokens::space::LG)
            .radius(tokens::radius::MD)
        [
            Text("Todos"),
            Row.gap(tokens::space::SM) [
                TextInput
                    .value(draft)                          // controlled: Signal in  (F2)
                    .on_input(move |s| draft.set(s))       // typed onChange out (String)
                    .on_submit(move || add()),             // Enter
                Button("Add").on_press(move || add()),
            ],
            // DYNAMIC LIST (F7): keyed by `id`. Rows spawn/despawn/move by key — no wholesale re-render,
            // no marker components, no hand-rolled reconciler in author code.
            For(todos, |t| t.id, move |t: Todo| view! {
                Row.gap(tokens::space::SM) [
                    Checkbox.checked(t.done).on_toggle(move |v| t.done.set(v)),  // per-row toggle (RwSignal is Copy)
                    Text(t.text.clone()),
                ]
            }),
            Row.gap(tokens::space::SM) [
                Text(move || format!("{} items left", items_left.get())),        // derived footer
                Button("Clear completed")
                    .on_press(move || todos.update(|v| v.retain(|t| !t.done.get()))),  // structural write → For re-diffs
            ],
        ]
    }
}

fn main() {
    App::new()
        .add_plugins((BuiyPlugins, SignalRuntimePlugin))
        .mount(todo_app)
        .run();
}
```

The list is a keyed `For(todos, |t| t.id, ...)` and each row's `done` is its own `RwSignal` so a toggle disturbs zero siblings; the derived count is a `memo` that auto-subscribes to every row's signal and re-runs exactly once per change.

## At a glance — the comparison matrix

| Paradigm | Declaration style | Event model | Styling attach | Tokens | Dynamic lists | MVU fit | Verbosity | Feels familiar to |
|---|---|---|---|---|---|---|---|---|
| **bsn!-native MVU** | One `view!` tree (bsn! superset), model at root | `on_press(Msg)` → generic route to nearest ancestor model | Fluent scene-builder chain splatted inside the tree | Typed `tokens::…` path, build-time | `for_each(key:, row:)` macro reconciler | Natural (routing/bind are roadmap slots) | Low (~30/~45) | Bevy/ECS, Elm/Redux, SwiftUI/Compose |
| **Elm / iced view-fn** | `view(&Model) -> Element<Msg>`, rebuilt on change | `.on_press(Msg)`, one central Msg enum | Method chain on the Element | Typed enums (`ColorToken` etc.) | `.iter().map()` into `column(iter)`, `.key(id)` | Thin-adapter (native update, reconciler added) | Low (~24/~55) | iced, Elm, Redux, React useReducer |
| **Jetpack-Compose** | `view(&Model, &mut Ui)` emit calls | `on_change: Fn(T) -> Msg` closures | `Modifier` value passed as first param | Typed `Theme` resource read positionally | `for`-emit with `.key(id)` | Thin-adapter/fights (reconciler + full hoist) | Low (~30/~60) | Compose, React, Flutter, SwiftUI |
| **SwiftUI value-views** | `fn view(&Model) -> impl View<Msg>`, nested tuples | `.action(Msg)` on entity, generic route | Order-independent modifier chain | Typed enums, total match, bonded (bg,fg) | `ForEach(slice, key, builder)` | Thin-adapter (needs §7.4 reconciler) | Low (~25/~55) | SwiftUI, Compose, Flutter, React |
| **Fine-grained signals** | `view!` setup fn runs once, wires graph | Typed setter closures, bypass funnel | Builder methods; reactive = closure | Typed const module | keyed `For` + per-row `RwSignal` + `memo` | Fights-it (serial island, bypasses log) | Lowest (~20/~48) | Leptos, Solid, React, Vue, Svelte |

## Friction scorecard

| Paradigm | F2 events | F4 styling | F5 one spelling | F6 tokens | F7 lists/derived | F8 verbosity |
|---|---|---|---|---|---|---|
| **bsn!-native MVU** | great — typed, routed to ancestor | great — chain inside `view!` | great — mandates scene-fns | great — typed path, build-time | ok — reconciler, replay unproven | great — one `add_mvu_ui` |
| **Elm / iced view-fn** | ok — central Msg enum, `.map` tax | great — plain-Rust chain | great — one builder each | great — enums, no hatch | great — `.map()` + local `.count()` | great — no schedule/markers |
| **Jetpack-Compose** | great — `Fn(T)->Msg` per widget | great — Modifier param | great — one fn per widget | great — typed `Theme` resource | ok — great DX on unproven §7.4 | great — reads like Compose |
| **SwiftUI value-views** | great — typed on entity, routed | great — chain, order-independent | great — one app spelling (additive) | great — total match, bonded pairs | great — `ForEach` + pure count | great — pure fn + reducer |
| **Fine-grained signals** | great — typed closures (bypass funnel) | great — chain + closure styles | ok — 5th spelling unless sole path | great — typed const module | great — memo + keyed For | great — no Msg/reducer/match |

## The honest read on each

**bsn!-native MVU** is best at being maximally Buiy-idiomatic: it extends the one authoring macro the project already committed to and sits directly on the MVU roadmap slots (typed press-routing IS `with_routing`, `bind!` IS the `MvuSet::Bind` stage), so the author reads one tree that *is* the app. Its sharpest cost is a heavy, high-surface proc-macro that must track upstream `bsn!` and moves real logic (bind systems, projections, reconcilers, the `m`/item binder) into generated spans the author never sees — type and borrow errors surface in machinery you didn't write. It sits **naturally** on MVU for routing and derived binds, thin-adapter for `for_each` (keyed-list replay is unproven), and mildly fights the command-sourced editor at the two-way input.

**Elm / iced view-fn** is best at being the most literal MVU spelling — one Model, one Msg enum, one `update`, and F4/F5/F6/F7 essentially fall out for free because the view is plain Rust with no bsn!/Style split. Its sharpest cost is the central Msg enum that scales linearly with interactables (sub-apps need `.map(Msg::Sub)`), plus controlled widgets that bypass the substrate's free tiered reducers, forcing you to re-implement toggle/value logic. The update-half is **natural** (zero-adapter); the `view()`-half is a **thin-adapter** that reintroduces the reconciliation layer MVU-as-core was specifically designed to avoid — a coherence cost as a second paradigm beside bsn!.

**Jetpack-Compose** is best at the smallest conceptual on-ramp for anyone from Compose/React/SwiftUI/Flutter — value-down/event-up, Modifier-as-parameter, and slot closures transfer 1:1, and derived state is a plain expression. Its sharpest cost is that its headline dynamic-list ergonomics ride Buiy's least-mature substrate piece (the §7.4 keyed reconciler, "targeted, not yet proven"), and re-running `view` plus rebuilding boxed closures every changed frame is churn the retained substrate is designed to skip. Natural at the state contract, **thin-adapter-to-fights-it** at the view engine and the full-hoisting that collides with MVU's deliberate widget tiering (the editor becomes bound-not-controlled).

**SwiftUI value-views** is best at approachability for the enormous declarative-state→view+modifiers population, with styling co-located with structure and — because it lowers every binding/action to `enqueue` — whole-UI record/replay and agent-drive for free. Its sharpest cost is the biggest framework lift: it needs a value-tree `View` layer and the exact §7.4 keyed reconciler that isn't proven yet, so it can't ship until that lands; plus Rust-specific edges (tuple-ViewBuilder arity ceiling, `impl View` monomorphization blowups) and a `bind!` that is honestly one-directional. It leans on MVU cleanly — **thin-adapter** — and asks the roadmap to land the reconciler the roadmap already wants; it fights nothing.

**Fine-grained signals** is best at derived/list ergonomics and lowest ceremony, and is the single most transferable mental model for web-frontend devs (state is a value, derived is a memo, list is a For). Its sharpest cost is architectural: the graph is one exclusive-`&mut World` serial system that abandons Bevy's parallel scheduler, and its writes bypass `enqueue` — forfeiting the record/replay, agent-drive, and hot-reload-via-replay that MVU-as-core was made core to guarantee. It **fights** MVU head-on as a runtime; as a thin adapter it collapses into what MVU already is, which is exactly why it's shown as the road not taken — steal the authoring shapes, leave the serial global-graph runtime on the shelf.

## What to decide next

- **Tree-with-attributes vs rebuild-view-fn?** — the bsn!-native `view!` tree spawns once and mutates in place, while iced/Compose/SwiftUI re-run a `view(&Model)` and diff; do you want one authoring macro extended, or a value-tree + reconciler added alongside bsn!?
- **Central Msg enum vs per-widget event closures?** — Elm/SwiftUI route one `enum Msg` (one place to look, `.map` tax at scale); Compose/signals attach a typed closure per widget (local, but closures/handler-tables under the hood). Which locality do you want?
- **Typed Msg routing vs two-way `bind!` for inputs?** — every paradigm must bridge the command-sourced editor; do you spell it as an explicit `on_input(Msg)` + reducer, or hide it behind `bind!(m.draft => Msg)` sugar that still lowers to `enqueue`?
- **Modifier chains vs component patches for styling?** — a fluent `.padding().background()` chain (Compose/SwiftUI/iced) vs the bsn!-native re-lowering of the builder into decomposed components; and if chains, order-independent (SwiftUI-here) or order-significant?
- **Ship the §7.4 keyed reconciler now, and how hard?** — Compose, SwiftUI, and the bsn! `for_each` all depend on the "targeted, not yet proven" keyed reconciler; is proving it (byte-identical replay of reconcile-spawned rows) a prerequisite, or do you scope the first surface to avoid derived structure?
- **One canonical spelling by addition or by deletion?** — do you resolve F5 by making a new app-surface the sole path (deleting the other three spellings for app authors), or by adding a fifth surface on top and demoting the rest to a documented widget-author hatch?
