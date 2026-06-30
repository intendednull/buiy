**Date:** 2026-06-26
**Status:** active
**Subject:** Relm4 — idiomatic Elm-style GUI library on GTK4/Rust; the closest existing analog to Buiy's "actor per widget" bet. Researched for the MVU-as-core (proto-3) state-management re-decision.

# Relm4

[`relm4`](https://github.com/Relm4/Relm4) is "an idiomatic GUI library inspired by Elm and based on gtk4-rs." It is the most production-credible **component + message-passing** architecture in Rust GUI: every meaningful stateful unit is a **Component** with its own `Model`, an inbound **`Input`** message type, an outbound **`Output`** message type, an async **`CommandOutput`** type, a pure-ish `update()`, and a retained `view`. Components compose by message passing over channels — a child emits `Output`, the parent maps it to its own `Input` at the connection edge. This is the design space Buiy's proto-3 is entering, built by people who have shipped it for five years.

For Buiy's MVU-as-core question, Relm4 is the **single most relevant prior art**: it answers, from production, the charter's hardest open question — *"does every widget become a `Model` + reducer, or do leaf widgets stay imperative and only route?"* Relm4's answer is decisive and load-bearing for Buiy's performance/scale risk (see § "The granularity verdict" below and [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md)).

Relm4 is **not** an integration target — Buiy is on Bevy ECS + winit, Relm4 is on GTK4 + glib's main loop. It is a **paradigm peer**: the same Elm-MVU idea, a different runtime substrate. The value is in how it shapes per-component state, composes children, and handles effects — and, critically, in what its architecture *cannot* do that Buiy's thesis demands (recordable log / time-travel / replay), and *why*.

## Honest assessment

- **Relm4 is the strongest existence proof that Elm-style per-component MVU works at production scale in Rust — at COARSE granularity.** Components are heavyweight by design: each has its own async runtime task and its own set of channels (`input`, `output`, `command`); a `Worker` can even own an OS thread. You do **not** make every `gtk::Button` a component. Idiomatic Relm4 reserves the component boundary for *stateful units* (a screen, a dialog, a list-item, a header bar); **leaf widgets are plain GTK widgets inside the `view!` macro**, not components. This is the central finding for Buiy: the actor-per-widget poster child does **not** do actor-per-leaf-widget.

- **Relm4 has message passing but NO recordable log, NO time-travel, NO replay** — despite being Elm-inspired. Its messages are ephemeral channel sends; its `update_view`/`view!` path mutates **live retained GTK widget handles** stored *inside* the component; its commands run as **unrecorded side effects** on a background runtime. The Elm *insight* (state is a value, mutations are described) is present in the `update` signature, but the Elm *superpower* Buiy is chasing (a complete, serializable message stream → deterministic tests + time-travel + agent-driving) is **architecturally foreclosed** in Relm4. This is the sharpest line in the whole comparison: Relm4 validates the ergonomics and cautions on granularity/perf, but it leaves Buiy's record/replay headline as genuinely novel territory.

- **The `Input`/`Output`/`forward()` composition is excellent — but it does not eliminate `Msg.map`, it relocates it.** Relm4's headline ergonomic over raw Elm/Iced is that the child→parent type mapping lives at **one site per parent-child edge** (`.forward(parent.input_sender(), |child_out| ParentInput::…)`), not threaded through every node of a `view` tree (Iced's `Element::map`). That is a real, large improvement — O(edges) mappings instead of O(view-nodes). But it is still per-edge boilerplate, and deep nesting chains forwards (child→mid→parent each maps). Buiy's `EntityEvent` auto-bubbling (route an event up to the nearest ancestor that owns the `Msg`, no per-edge map) is arguably *better* than Relm4 for the common "bubble an event to whoever handles it" case.

- **Effect results are a SEPARATE message type (`CommandOutput`), handled in a separate method (`update_cmd`), not folded back as regular `Input`.** Relm4 deliberately splits user-intent messages (`Input` → `update`) from effect results (`CommandOutput` → `update_cmd`). Buiy's draft folds async results *back* through the same `update` as normal `Msg`s (one log). Both are defensible; for the *record/replay* thesis Buiy's unified-log approach is better (results are just later log entries), and Relm4's split is the road-not-taken worth naming.

- **`DynamicIndex` over `usize` is a battle-tested validation of Buiy's "keyed reconcile by domain id, not position."** Relm4's factories index children by a stable `DynamicIndex` precisely because a deferred message carrying a positional `usize` points to the *wrong* element after a reorder. Buiy reached the same conclusion independently (the entity *is* the identity; reconcile keys on domain id). Relm4 is the documented bug-report that confirms it.

- **Async-in-`update` (`AsyncComponent`) is a documented footgun; `Commands` is the blessed path.** Relm4's `AsyncComponent` makes `update` an `async fn`, but awaiting a slow future there **blocks that component's entire message queue** (messages process strictly one-at-a-time). Relm4's own guidance: use `Commands` (separate runtime, results via `CommandOutput`) for anything slow. This directly validates Buiy's plan (pure reducer returns `Cmd::task`; a poll system folds the result back) over any "async reducer," which would also break purity/determinism/recording.

- **Healthier project vitality than Iced.** Dual-licensed `Apache-2.0 OR MIT` (matches Buiy's posture, unlike Iced's MIT-only), two named maintainers (Aaron Erhardt, Andy Russell) plus 68 contributors (better bus factor than Iced's single architect), edition 2024 / MSRV 1.93, actively released (0.11.0, 2026-04-08). The constraint is the **GTK4 binding**: it inherits GTK's main-loop threading model, GObject lifecycle, and C-library footprint — none of which Buiy shares.

## Key facts (verified 2026-06-26)

| Fact | Value |
|---|---|
| Crate | `relm4` (+ `relm4-macros`, `relm4-components`, `relm4-icons`) |
| Latest version | **0.11.0** (published 2026-04-08) |
| License | **Apache-2.0 OR MIT** (dual — matches Buiy; unlike Iced's single-MIT) |
| MSRV / edition | rust-version 1.93, edition 2024 |
| Downloads | ~16,243 / month |
| Repository | https://github.com/Relm4/Relm4 |
| Maintainers | Aaron Erhardt + Andy Russell; 68 contributors |
| Paradigm | Elm-style **Component** (Model + Input/Output Msg + `init` + `update` + retained view) |
| Toolkit | **GTK4** via `gtk4-rs` + **libadwaita**; glib main loop (NOT winit, NOT ECS) |
| Renderer / layout / text | All GTK's (GالسSK / GTK's box+grid layout / Pango) — none shared with Buiy |
| Effects | **Commands** (`oneshot_command` / `command` / `spawn_oneshot_command`) → `CommandOutput` |
| Background actors | **Workers** (`Worker` trait, `detach_worker` → `WorkerController`, own thread) |
| Collections | **Factories** (`FactoryVecDeque`, `FactoryHashMap`; `FactoryComponent`; `DynamicIndex`) |
| Change detection | **No virtual DOM**; `#[watch]` (unconditional) + `#[tracker::track]` (dirty-bit) + factory diff |
| Global dispatch | `MessageBroker` (cross-component, escapes strict tree routing) |
| Predecessor | `relm` (GTK3); Relm4 is a from-scratch GTK4 rewrite |
| Record / replay / time-travel | **ABSENT** — no message log; view holds live widget handles; commands unrecorded |

## The component model in depth (state-management lens)

### Four component tiers, by weight

Relm4 layers the trait hierarchy so you pay only for what you use. Lightest to heaviest capability:

1. **`Worker`** — a `SimpleComponent` with **no widgets**. `detach_worker()` runs its `update` loop on its **own OS thread**, returning a `WorkerController`. For background, message-driven services (a sync engine, a file watcher). Processes one message at a time, sequentially.
2. **`SimpleComponent`** — the "Elm-style variant." Has widgets and `Input`/`Output`, but **no `Commands`** (no async background work). `update()` mutates the model; a separate `update_view()` (usually macro-generated) syncs widgets. This is the cleanest Model+Msg+update+view tier — the closest to textbook Elm.
3. **`Component`** — `SimpleComponent` + **`Commands`**: adds the `CommandOutput` associated type and `update_cmd()` for background async/CPU work whose results return as a *separate* message type.
4. **`AsyncComponent` / `SimpleAsyncComponent`** — `init` and `update` become `async fn` (you can `.await` inside them). Tradeoff below.

Plus **`FactoryComponent`** — "similar to `Component` but adjusted to fit the life cycle of factories" — for elements stored in a dynamic collection (§ "Factories").

### The `Component` trait surface

From [`relm4::component::Component`](https://relm4.org/docs/next/relm4/component/trait.Component.html):

```rust
// Associated types — the message + widget contract
type CommandOutput: Debug + Send + 'static; // results from background commands
type Input:         Debug + 'static;         // messages this component accepts
type Output:        Debug + 'static;         // messages this component emits to its parent
type Init;                                    // construction parameter
type Root:          Debug + Clone;            // top-level widget
type Widgets:       'static;                  // generated storage for created widgets

// Required
fn init_root() -> Self::Root;
fn init(init: Self::Init, root: Self::Root, sender: ComponentSender<Self>)
        -> ComponentParts<Self>;   // build initial Model + Widgets

// Provided (override as needed)
fn update(&mut self, message: Self::Input,        sender: ComponentSender<Self>, root: &Self::Root);
fn update_cmd(&mut self, message: Self::CommandOutput, sender: ComponentSender<Self>, root: &Self::Root);
fn update_view(&self, widgets: &mut Self::Widgets, sender: ComponentSender<Self>);
fn shutdown(&mut self, widgets: &mut Self::Widgets, output: Sender<Self::Output>);
```

Four observations that matter for Buiy:

- **State and view are different things, joined by `update_view`.** The `Model` (`Self`) is plain data; `Self::Widgets` is the retained GTK widget tree. `update()` mutates the model; `update_view()` (or the macro) pushes model→widgets. This *looks* like Buiy's "components hold state, systems render" — but the widgets are stored *inside* the component and mutated by direct side-effecting GTK calls, not derived from data by an external system. (Consequence for replay: see § "Why Relm4 cannot replay.")
- **Three distinct message channels, three distinct handlers.** `Input`→`update`, `CommandOutput`→`update_cmd`, and the view is a third concern (`update_view`). User intent, effect results, and rendering are type-separated. Buiy's draft unifies the first two (effect results fold back as `Msg`s).
- **`init` is the only place that builds widgets**, and it receives the `sender` — so a widget's signal handlers are wired at construction to push `Input` back into *this* component's mailbox.
- **`shutdown` gives a last `Output` sender** — a component can emit a final message as it tears down (lifecycle event).

### `SimpleComponent` — the Elm core

`SimpleComponent` ([docs](https://relm4.org/docs/next/relm4/component/trait.SimpleComponent.html)) drops `CommandOutput`/`update_cmd` and keeps `Input`/`Output`/`Init`/`Root`/`Widgets` + `init_root`/`init`/`update`/`update_view`/`shutdown`. It is "an Elm-style variant" that "separates view updates from input updates" — `update()` handles messages, the dedicated `update_view()` reflects state into widgets. This is the minimal Model + Message + Update + View unit.

### `ComponentSender` — the message API

`ComponentSender<C>` ([docs](https://relm4.org/docs/next/relm4/struct.ComponentSender.html)) wraps three senders and is the entire message-passing surface a component sees:

```rust
fn input(&self, message: C::Input);                       // send a message to *self*
fn output(&self, message: C::Output) -> Result<(), C::Output>; // emit upward (Err if no receiver)
fn command<Cmd, Fut>(&self, cmd: Cmd)                      // spawn background work, multiple results
    where Cmd: FnOnce(Sender<C::CommandOutput>, ShutdownReceiver) -> Fut + Send + 'static,
          Fut: Future<Output = ()> + Send + 'static;
fn oneshot_command<Fut>(&self, future: Fut)               // spawn background work, one result
    where Fut: Future<Output = C::CommandOutput> + Send + 'static;
// + input_sender() / output_sender() / command_sender() for the raw Sender<T>s
```

`output()` returning `Err` when all receivers were dropped is a built-in **dead-letter** signal at the channel level — relevant to Buiy's "unhandled = loud typed dead-letter" decision.

### A concrete `SimpleComponent` (counter)

The macro (`#[relm4::component]`) generates `Widgets` + `update_view` from a `view!` block. The canonical shape:

```rust
struct AppModel { counter: u8 }

#[derive(Debug)]
enum AppMsg { Increment, Decrement }

#[relm4::component]
impl SimpleComponent for AppModel {
    type Init = u8;
    type Input = AppMsg;
    type Output = ();

    view! {
        gtk::Window {
            gtk::Box {
                gtk::Button {
                    set_label: "+",
                    connect_clicked => AppMsg::Increment,   // signal → Input message
                },
                gtk::Label {
                    #[watch]                                 // re-run setter whenever model changes
                    set_label: &format!("Counter: {}", model.counter),
                },
            }
        }
    }

    fn init(counter: u8, root: Self::Root, sender: ComponentSender<Self>)
            -> ComponentParts<Self> {
        let model = AppModel { counter };
        let widgets = view_output!();      // macro builds the view! tree
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Increment => self.counter = self.counter.wrapping_add(1),
            AppMsg::Decrement => self.counter = self.counter.wrapping_sub(1),
        }
    }
}
```

`connect_clicked => AppMsg::Increment` is the macro's shorthand for "wire this GTK signal to push this `Input` into the component's mailbox." `#[watch]` re-runs `set_label` on every model change. The `update` is a plain `&mut self, msg` match — no `World`, no `Commands`, **no return value** (effects are issued via `sender`, not returned as values — a divergence from Iced's `update -> Task` and from Buiy's `update -> Cmd`).

### Parent ↔ child composition (the heart of it)

A parent embeds a child by **launching** it through a builder and **forwarding** its `Output` into the parent's `Input`:

```rust
struct AppModel { header: Controller<HeaderModel>, dialog: Controller<DialogModel> }

// in init():
let header: Controller<HeaderModel> = HeaderModel::builder()
    .launch(())                                  // build child with its Init
    .forward(sender.input_sender(), |msg| match msg {   // child Output → parent Input
        HeaderOutput::View   => AppMsg::SetMode(Mode::View),
        HeaderOutput::Edit   => AppMsg::SetMode(Mode::Edit),
        HeaderOutput::Export => AppMsg::SetMode(Mode::Export),
    });
// store `header` in the model; mount its widget in the view:
//   set_titlebar: Some(model.header.widget()),
```

The directions:

- **Parent → child:** `model.dialog.sender().send(DialogInput::Show)` (or `.emit(...)`). The parent holds the child's `Controller`, which exposes the child's `input_sender()`.
- **Child → parent:** the child calls `sender.output(HeaderOutput::Edit)`; the `forward()` closure registered at launch maps it to a parent `Input` and pushes it into the parent's mailbox.
- **No forwarding needed:** `.detach()` instead of `.forward(...)` launches an independent child.
- **The view mounts the child's root widget** via `model.header.widget()`.

This is the **`forward()`-at-the-edge** pattern: child→parent type mapping is declared **once, at the connection site**, not threaded through the view tree. Compare Iced, where `Element::map` must wrap the child's element everywhere it appears in the parent's `view`. Relm4's improvement is real (one map per edge), but it is still per-edge code, and a 3-deep nesting maps three times (child→mid, mid→parent).

> **Buiy mapping.** This is exactly the shape of Buiy's proto-2 `Callback<T>` (`callback::<T, M>(parent, |t| ParentMsg::…)` — `examples/mvu_native/src/callback.rs`): a cloneable sink that resolves a child payload to a *funnel dispatch* of a parent `Msg`. Relm4's `Output` enum is the **coarse** version (one enum per child, one match arm per variant); Buiy's `Callback` is the **fine** version (one typed sink per event). Both localize the mapping to the connection edge. See [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) § Borrow #1.

### Factories — dynamic lists of child components ("N child actors")

This is the section most directly relevant to Buiy's "N child actors / keyed reconcile" decision. A `FactoryVecDeque<C>` holds a dynamic, ordered collection where **each element `C` is itself a full `FactoryComponent`** (its own `Model`/`Input`/`Output`/`update`).

```rust
#[relm4::factory]
impl FactoryComponent for Counter {
    type Init        = u8;
    type Input       = CounterMsg;
    type Output      = CounterOutput;     // emitted up to the owning component
    type CommandOutput = ();
    type ParentWidget  = gtk::Box;        // container the factory fills
    type Index         = DynamicIndex;    // STABLE id, not usize
    // init_model / init_root / init_widgets / update …
}
```

Wiring the list into a parent:

```rust
let counters = FactoryVecDeque::builder()
    .launch(gtk::Box::default())          // the ParentWidget container
    .forward(sender.input_sender(), |output| match output {
        CounterOutput::SendFront(index) => AppMsg::SendFront(index),
        CounterOutput::MoveUp(index)    => AppMsg::MoveUp(index),
        CounterOutput::MoveDown(index)  => AppMsg::MoveDown(index),
    });
```

Mutating the collection goes through an **RAII `guard()`** — "similar to a `MutexGuard` you get from locking a mutex." The guard batches structural edits; **dropping the guard triggers minimal widget synchronization** (only the changed elements update):

```rust
self.counters.guard().push_back(init);                 // add
self.counters.guard().pop_back();                      // remove
self.counters.guard().move_to(idx, new_idx);           // reorder
```

The parent's `update` handles the up-routed structural requests, keyed by the **stable index**:

```rust
AppMsg::MoveUp(index) => {
    let i = index.current_index();
    if i != 0 { self.counters.guard().move_to(i, i - 1); }
}
```

Three findings for Buiy:

1. **`DynamicIndex`, not `usize` — stable identity under reorder.** "If you used `usize` as index … the index points to another element by the time it is processed." Elements move; a message carrying a positional index is stale on arrival. `DynamicIndex` "maintains stability by tracking the actual element identity, not position." **This is Buiy's "keyed reconcile by DOMAIN id, not Entity/position," confirmed from production.**
2. **The `guard()` batch-then-sync** is Relm4's "structural mutations are deferred and applied at a sync point." Buiy gets this from ECS `Commands` (deferred) + a reconcile system. Same discipline: never mutate the collection mid-`update`; batch and apply at a boundary.
3. **The factory→parent route is the *same* `builder().launch().forward()` as a single child.** Notably, Relm4 **removed** the old special-cased `ParentInput` + `forward_to_parent()` methods from `FactoryComponent`; "factories now support basically the same builder pattern as components." They tried a bespoke collection-routing API and **deleted it for uniformity.** (Lesson in [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) § Avoid #3.)

### Commands — async effects

Background/async work is issued via the `sender`, and results return as the separate `CommandOutput` type into `update_cmd`:

```rust
// I/O-bound: one async future → one CommandOutput
sender.oneshot_command(async {
    CmdMsg::Fetched(fetch_data().await)
});

// CPU-bound: runs on the blocking pool
sender.spawn_oneshot_command(|| CmdMsg::Computed(expensive()));

// flexible: multiple results, lifetime-bound to the component
sender.command(|out, shutdown| async move {
    // `shutdown: ShutdownReceiver` cancels the future when the component dies
    while let Some(tick) = stream.next().await { let _ = out.send(CmdMsg::Tick(tick)); }
});

fn update_cmd(&mut self, msg: Self::CommandOutput, _s: ComponentSender<Self>, _r: &Self::Root) {
    match msg { CmdMsg::Fetched(d) => self.data = d, /* … */ }
}
```

Key properties for Buiy:

- **Effect results are typed apart from user intent** (`CommandOutput` ≠ `Input`; `update_cmd` ≠ `update`). Deliberate separation of "a user did X" from "an effect produced Y."
- **`ShutdownReceiver` binds the future to the component's lifetime** — when the component shuts down, the command future is cancelled (dropped). This is Buiy's "cancellation is free on despawn (drop = cancel)," in Relm4's vocabulary.
- **`oneshot_command` does not enforce single-in-flight** — call it twice and two futures race. Buiy's *takeLatest* (one `InFlight` per model, new supersedes old) is a refinement Relm4 leaves to the author.
- Commands run on a configurable runtime (`RELM_THREADS`, `RELM_BLOCKING_THREADS`).

### Workers — headless background actors

A `Worker` is `SimpleComponent` minus widgets, whose update loop can run **on its own thread** (`detach_worker()` → `WorkerController<W>`). The parent holds the `WorkerController`, clones its sender to send `Input`, and `forward`s the worker's `Output` back. Workers process messages **strictly sequentially** (one at a time); for non-blocking concurrency you reach for `Commands` instead. Workers are Relm4's answer to "an actor that is pure logic + state, no view" — the closest analog to a Buiy model that has no rendered widget.

### AsyncComponent — and why it is a footgun

`AsyncComponent` makes `init`/`update` `async fn`:

```rust
async fn update(&mut self, msg: Self::Input, _sender: AsyncComponentSender<Self>, _root: &Self::Root) {
    let data = slow_fetch().await;   // ← legal, but…
    self.data = data;
}
```

The documented tradeoff: *"Awaiting slow futures will block the processing of further messages. … the update function can only process one message after the other."* Inline `await` in `update` serializes the component's whole mailbox behind the slow future. Relm4's own guidance is to prefer **Commands** (separate runtime, non-blocking, results via `CommandOutput`) for slow work. For Buiy this is a clean **negative result**: an "async reducer" is the wrong shape — it blocks the drain and (worse for Buiy) introduces non-deterministic interleaving that breaks recording/replay. Buiy's `Cmd::task` + poll-fold-back *is* the Commands model, and that is the right one.

### Efficient view updates without a virtual DOM

Relm4 explicitly rejects vdom diffing. The motivating example from its own docs: *"an app with 1000 counters … increment the first counter … the view function gets the updated model with 1000 counters but has no idea what has changed, so instead of one UI update you need to do 1000."* Its fixes:

- **`#[watch]`** — re-run a setter every update cycle (cheap, unconditional; fine for a handful of bindings).
- **`#[tracker::track]`** — the `tracker` crate generates a **dirty bit per field**; `set_field()` marks the bit only if the value differs; the view guards a setter with `#[track = "model.changed(Model::field())"]` so it runs **only when that field changed**; `reset()` clears the bits each cycle. Field-level change detection, no vdom.
- **Factories** — collection-level diffing (the `guard()` sync only touches changed elements).

For Buiy this is a strong validation that **change-detection-gated binds beat full-view recompute at scale** — and Buiy gets `#[tracker::track]`'s machinery *for free* from ECS `Changed<T>` + `set_if_neq` (proto-2's `bind`). Relm4 had to build a whole crate to get what Bevy's change detection gives natively. (See [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) § Borrow #5 — a Buiy advantage worth naming.)

### Global dispatch — `MessageBroker`

For messages that must cross the component tree (not just parent↔child), Relm4 ships a `MessageBroker` — a global, typed broadcast point a component can subscribe to. This is the escape hatch from strict tree routing, analogous to Buiy's "explicit-address dispatch is the cross-tree escape hatch."

## Why Relm4 cannot replay (the decisive divergence)

Relm4 has every *surface* feature of Buiy's MVU — Model, typed messages, `update`, child/parent composition, effects — yet it has **no time-travel and no replay**. The architecture forbids it, for reasons Buiy must not reproduce:

1. **Messages are ephemeral channel sends.** There is no single ordered log of `(component, message)`; each `sender.input(...)` is a fire-and-forget enqueue onto a per-component channel. Nothing taps a global stream.
2. **The view holds live widget handles.** `Self::Widgets` is a tree of retained GTK objects mutated by side-effecting calls (`set_label`, `set_class_active`). State is *not* purely data — re-running `update` from the start would not reconstruct the GTK tree, and the GTK tree has its own hidden state.
3. **`update`/`update_cmd` are not pure and return nothing.** They issue effects through `sender` *as side effects mid-update* (sending, spawning commands, mutating widgets). There is no "effects as values" boundary to intercept and record.
4. **Commands are unrecorded background side effects.** A command's I/O happens off-runtime; its result re-enters as a `CommandOutput`, but the *intervening effect* (the network call, the file write) is neither logged nor reproducible.

Buiy's thesis is precisely the inversion of all four: **one ordered drain owns the global log; state is `Reflect` data not widget handles; the reducer is pure and returns `Cmd` as values; effect *results* fold back as recorded `Msg`s.** Relm4 is the proof that Elm-MVU ergonomics alone do **not** buy you replay — you have to architect for it, which is the entire point of MVU-as-*core*. (Detailed in [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) § "The headline.")

## The granularity verdict (the charter's hardest question)

> *"Does every widget become a `Model` + reducer, or do leaf widgets stay imperative and only route?"* — proto-3 charter.

**Relm4's production answer is unambiguous: components are COARSE; leaves are plain widgets.** A `gtk::Button`/`gtk::Label`/`gtk::Entry` inside a `view!` block is **not** a component — it has no `Model`, no mailbox, no `update`. The component boundary (with its async runtime task + three channels + `Controller`) is reserved for units that genuinely have their own state, logic, and lifecycle: a screen, a dialog, a sidebar, a list-*item* (via `FactoryComponent`). This is not an accident of GTK — it is the load-bearing reason Relm4 scales: a typical app has tens-to-hundreds of components, not thousands.

This is the most important single input to Buiy's performance/scale risk. The actor-per-widget poster child **does not** put an actor on every leaf, because a per-component mailbox + runtime is too heavy to multiply by thousands. Buiy's draft already leans this way ("leaf widgets stay imperative and only route") — Relm4 is the strongest external confirmation that this is the *correct* default, and that "every widget is a `Model`" would be a scale mistake. Full treatment, with the perf evidence and the recommended Buiy granularity, in [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md).

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — Relm4's component/message/effect model in depth, the state-management lens, the granularity verdict, and why Relm4 cannot replay. |
| [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) | **The consult-this-when-designing decision file.** KEEP / AVOID / Borrow for Buiy's proto-3: per-component state, composition, effects; what Relm4's ergonomics teach Buiy's reducer/macro design and the "every widget a Model vs route-only" decision. |

## How to use this prior-art doc

1. **Deciding widget granularity (Model+reducer vs route-only):** read § "The granularity verdict" here and [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) § "The granularity decision." Relm4 is the decisive evidence.
2. **Designing child→parent composition / the `Callback` surface:** read § "Parent ↔ child composition" + "Factories" here, then [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) § Borrow #1–#3. The `forward()`-at-the-edge pattern and `DynamicIndex` are the takeaways.
3. **Designing the `Cmd`/effect algebra:** read § "Commands" + "AsyncComponent" here, then [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) § Avoid #4 (no async reducer) and § Borrow #4 (`ShutdownReceiver` = drop-cancel).
4. **Pressure-testing the record/replay thesis:** read § "Why Relm4 cannot replay." It is the cautionary architecture — Elm ergonomics without replay — that names exactly what Buiy must do differently.
5. **Designing the reducer macro / `bind`:** read § "Efficient view updates" — `#[tracker::track]` is what Bevy's `Changed<T>` gives Buiy for free.

## Framing disclosure

These notes are written from a **"Buiy is building MVU-as-core on Bevy 0.19 ECS, chasing a complete recordable message log → deterministic tests + time-travel + agent-driving"** stance (the proto-3 charter). Relm4 is read primarily as a **paradigm peer that has shipped the ergonomic half of this idea in production but explicitly forgoes the record/replay half.** That lens shapes the emphasis: Relm4's `Input`/`Output`/`forward` and `DynamicIndex` validate Buiy's composition decisions (Borrow); its coarse component granularity validates Buiy's "leaves route only" default (KEEP); its per-component runtime/channel weight and its impossibility of replay are framed as cautions Buiy's architecture must respect or invert (Avoid).

This is **not** a neutral survey of Relm4 as a GTK app framework — its GTK4/libadwaita strengths (native theming, GObject ecosystem, accessibility via GTK's ATK/AT-SPI, mature widget set) are largely out of scope because Buiy shares none of that substrate. A reader evaluating "should we build a GTK4 Rust app?" should read Relm4's own book, not this folder. A reader evaluating "what does production Elm-style per-component MVU in Rust teach a *new* MVU core?" — that is this folder's question. Where Relm4's experience suggests something Buiy hasn't considered (e.g. the single-`Output`-enum ergonomics, the `MessageBroker` global escape hatch, the `Worker`-on-own-thread tier), it is flagged as a Borrow rather than dismissed.

Secondary disclosure: Relm4 is a *peer*, not a dependency. Buiy does not and will not depend on Relm4. The corpus may underweight Relm4's GTK-native polish; pressure-test where its five years of production message-passing experience contradicts a Buiy assumption (the granularity verdict is the prime example — it could overturn an "every widget is an actor" spec).

## Sources

- Relm4 repository — https://github.com/Relm4/Relm4
- Relm4 website / landing — https://relm4.org/
- Relm4 book — https://relm4.org/book/stable/
- `Component` trait — https://relm4.org/docs/next/relm4/component/trait.Component.html
- `SimpleComponent` trait — https://relm4.org/docs/next/relm4/component/trait.SimpleComponent.html
- `FactoryComponent` trait — https://relm4.org/docs/next/relm4/factory/trait.FactoryComponent.html
- `ComponentSender` — https://relm4.org/docs/next/relm4/struct.ComponentSender.html
- Book — Components (parent/child, Controller, forward) — https://relm4.org/book/stable/components.html
- Book — Factories (`FactoryVecDeque`, `DynamicIndex`, `guard`, `forward`) — https://relm4.org/book/stable/efficient_ui/factory.html
- Book — Commands (`oneshot_command`, `command`, `CommandOutput`) — https://relm4.org/book/stable/threads_and_async/commands.html
- Book — Async components — https://relm4.org/book/stable/threads_and_async/async.html
- Book — Efficient UI updates (no vdom, tracker, factory) — https://relm4.org/book/stable/efficient_ui/index.html
- Book — Tracker pattern — https://relm4.org/book/stable/efficient_ui/tracker.html
- Book — Component macro reference (`view!`, `#[watch]`, `connect_* => Msg`) — https://relm4.org/book/stable/component_macro/reference.html
- Book — migrations 0.6→0.7 (removal of `ParentInput`/`forward_to_parent`) — https://relm4.org/book/stable/migrations/0_6_to_0_7.html
- relm4 on crates.io — https://crates.io/crates/relm4
- relm4 on lib.rs — https://lib.rs/crates/relm4
- Buiy proto-3 charter — `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md`
- Buiy proto-2 retrospective + code — `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`, `examples/mvu_native/src/`
- Buiy draft state-management spec — `docs/specs/2026-06-26-buiy-state-management-design.md`
- iced prior-art folder (structural reference, Elm-architecture peer) — [`../iced/`](../iced/)
