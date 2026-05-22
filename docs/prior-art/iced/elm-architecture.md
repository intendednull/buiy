**Date:** 2026-05-22
**Status:** active
**Subject:** Iced — the Elm-architecture pattern (Model + Message + Update + View) in Rust

# The Elm architecture in Iced

Iced is the canonical implementation of **The Elm Architecture (TEA)** for Rust desktop GUI. The pattern: a single application state (`Model` / `State`) mutated only via an `update(state, message)` function that produces a new state plus side-effect descriptions (`Task<Message>`); a pure `view(state) → Element` function that renders the current state to a fresh widget tree.

Sibling files: [`architecture.md`](architecture.md), [`widgets-and-styling.md`](widgets-and-styling.md), [`text-and-cosmic.md`](text-and-cosmic.md).

## The four pieces

Following Elm's vocabulary directly:

| Elm name | Iced name | Type / shape | Role |
|---|---|---|---|
| **Model** | `State` (user-named) | any `Default + 'static` value | Single source of truth. All app data here. |
| **Message** | `Message` (user-named enum) | `enum Message { ... }` | Discrete events: every user interaction, every async result. |
| **Update** | `update` (function or method) | `fn(&mut State, Message) -> Task<Message>` | Pure-ish state mutation. Returns side-effect description. |
| **View** | `view` (function or method) | `fn(&State) -> Element<'_, Message>` | Builds widget tree from state. Re-runs every event. |

A trivial counter looks like:

```rust
#[derive(Default)]
struct Counter { value: i64 }

#[derive(Debug, Clone)]
enum Message { Inc, Dec }

fn update(state: &mut Counter, message: Message) {
    match message {
        Message::Inc => state.value += 1,
        Message::Dec => state.value -= 1,
    }
}

fn view(state: &Counter) -> Element<'_, Message> {
    row![
        button("-").on_press(Message::Dec),
        text(state.value),
        button("+").on_press(Message::Inc),
    ].into()
}

fn main() -> iced::Result {
    iced::run("Counter", update, view)
}
```

Notable Rust-isms: `&mut State` instead of Elm's returned-new-state (Rust uses mutation for ergonomics; semantically equivalent if `update` is total). `Element<'_, Message>` is generic over `Message` so widget event handlers stay type-safe.

## Why Elm in Rust

The pattern fits Rust well because:

- **Single ownership.** `State` is owned by the runtime; `update` takes `&mut State`; `view` takes `&State`. Borrow checker enforces "update and view never alias the state simultaneously" without effort.
- **Type-safe messages.** `Message` is a sum type; `match`-exhaustiveness checks every interaction is handled. Compare to web "events as strings" or GTK signals.
- **No globals.** All app state lives in one struct; no `static mut`, no `lazy_static`, no thread-locals required. Library code threads `&mut State` through.
- **Async via descriptors.** `Task<Message>` is *a description* of work — not the work itself. The runtime executes it on whatever executor the app picked (`tokio` / `smol` / etc.). `update` stays pure-ish (mutates `State` and returns a value).

## `Task<Message>` for side effects

`Task<Message>` (renamed from `Command<Message>` in 0.13, [PR #2463](https://github.com/iced-rs/iced/pull/2463), 2024-09-18) is Iced's effect type. An `update` function can return:

```rust
Task::none()                        // no side effect
Task::done(Message::Loaded)         // immediately fold a Message back
Task::perform(future, Message::Got) // run a Future, wrap its output in Message::Got
Task::run(stream, Message::Tick)    // consume a Stream<Item=T>
Task::batch([t1, t2])               // fork multiple
t1.chain(t2)                        // sequence
```

The runtime executes the task, collects any `Message` it produces, and feeds the message back into `update` on the next tick. This is how Iced apps do HTTP, file I/O, timers, subprocess spawning — without `update` ever blocking and without `view` knowing async exists.

Compare to:

- **Bevy / Buiy:** async work goes through `AsyncComputeTaskPool`, `IoTaskPool`, or third-party `bevy_tasks`. Results land on entities via systems polling task handles or via observers. No single effect type.
- **React:** `useEffect` co-locates effects with components; React's reconciler runs them after commit.
- **Dioxus / Leptos:** signal-based; effects subscribe to signals.
- **GTK / Qt:** synchronous APIs + manual thread dispatch; the runtime doesn't model async.

`Task` is the Elm contribution: effects are **values** the framework executes, not callbacks the framework calls.

## State management at scale

Single global `State` works at small scale and stops being ergonomic above ~5,000 lines. Iced's recommended pattern for larger apps:

1. **Decompose `State` into substructs**, one per screen / pane / dialog.
2. **Decompose `Message` into a nested enum**, mirroring substructs: `Message::Editor(editor::Message)`, `Message::Sidebar(sidebar::Message)`.
3. **Decompose `update`** — each substruct has its own `fn update(&mut self, msg: SubMessage) -> Task<SubMessage>`; the top-level update dispatches and `.map`s the resulting Task to wrap the inner Message.
4. **Decompose `view`** the same way — each substruct has `fn view(&self) -> Element<'_, SubMessage>`; the top-level view composes them with `Element::map(SubMessage → Message)`.

This is "manual component composition" — every component owns its state, messages, update, view. No framework-magic component model. Lots of boilerplate per component (the `.map(Message::Editor)` calls), but every coupling is visible. Iced apps look quite different from React apps where component-local state is hidden.

COSMIC desktop apps (cosmic-files, cosmic-edit, cosmic-settings) all follow this pattern — they ship at tens-of-thousands of lines using nested substruct decomposition and have not needed an abstraction layer on top.

## Comparison to React/Dioxus (component-local state)

| Aspect | Iced (Elm) | React / Dioxus |
|---|---|---|
| Where state lives | One global `State` struct | Components own state via hooks (`useState`, `use_state`) |
| Reactivity | None — full re-view every event | Fine-grained re-render via dep arrays / signals |
| Effects | Returned `Task<Message>` values | `useEffect` callbacks |
| Side-effect testability | Direct — `Task` is a value | Indirect — effects fire in render |
| State sharing across components | Pass refs / split `State` | Context / providers |
| Mental model | "What does my app's state look like, and what events change it?" | "What does this component need to know, and when?" |

The trade: Elm gives you **one place to look** for the whole app; React/Dioxus give you **locality** at the cost of state being scattered. For a 10-screen productivity app, both work. For a 200-widget editor, Elm's single-state pattern becomes a discipline burden.

## Comparison to ECS (Bevy / Buiy)

ECS turns the picture sideways:

| Aspect | Iced (Elm) | Bevy / Buiy (ECS) |
|---|---|---|
| Where state lives | One `State` struct | Per-entity components; resources for global state |
| Updates | One `update(&mut state, msg)` | Many systems, each touching specific components |
| Events | Strongly-typed `Message` enum dispatched centrally | `EventReader<E>` per event type; observers for fine-grained reaction |
| Tree model | Built every `view()` from `State` | Persistent entity hierarchy with `ChildOf` |
| Reactivity | Re-view every frame; `Lazy` for caching | Change detection (`Changed<T>`) + observers |
| State locality | Centralized | Decomposed per entity |
| Cross-component data | Via shared `&State` fields | Via shared `Resource` or via querying multiple components |

**ECS has decomposed state per entity; Elm has unified state.** Both work; they optimize for different things. ECS is better when you have *many similar things* (1,000 enemies, 100 UI nodes) and want them processed uniformly; Elm is better when you have *a few unique things* (the editor, the settings dialog, the file tree) each with distinct logic.

Buiy is committed to ECS-native authoring ([architecture.md § 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md)). The Buiy "BSN" hierarchical authoring story sits on top of ECS, not on top of Elm-style central state. Cross-link: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Validates → Decomposed visual components.

## The "stateless widgets" trade-off

`view()` re-runs every event. Widgets are built fresh each call — a `button("Save")` is a small `Button` value, allocated, used to compute layout / handle the event / draw, and dropped before the next event.

Consequences:

- **`view()` must be cheap.** Heavy work in `view()` is a perf bug, not a usability optimization. Iced's `widget::Lazy` exists precisely to memoize subtrees against dep hashes.
- **Widget state needs reconciliation.** Button-pressed-states, text-input cursor positions, scroll offsets cannot live on the `Button` value — they live in a parallel `Tree` keyed by tree-position + type-id. Reordering children without keys *desyncs* the state tree against the widget tree; `widget::keyed::column` exists to give children stable identifiers for reconciliation.
- **No long-running references.** A widget can't hold `&'static` to a long-lived object across frames; the value is gone. Apps that need to share data with widgets pass it via `view()` arguments or through `Lazy` keys.

Buiy's ECS model sidesteps this trade-off — UI entities persist across frames; widget state lives on the entity directly; reconciliation is unnecessary because the entity *is* the identity. The cost is more complex authoring (entities + components + systems, not just `view()` + `update()`).

## Implications for Buiy

Iced demonstrates Elm-architecture is viable for Rust desktop GUI at production scale (COSMIC, Halloy, Icebreaker, Modrinth-launcher). But:

1. **Buiy is not Elm-architecture.** Buiy is ECS-native; the `Model + Message + Update + View` pattern is *not* an organizing principle. Buiy widgets are entities; widget state lives on entities; observers replace `update`. Iced is reference for "how does a retained-mode Rust GUI feel" — not for the state-model itself.
2. **`Task<Message>` is the cleanest async-effect type in Rust GUI.** Buiy can study it as a model for async-effect descriptors if Buiy ever ships an effect-layer above raw `AsyncComputeTaskPool`. Open question — currently not planned, but the pattern is good.
3. **Per-frame `view()` rebuild is the wrong shape for Buiy.** ECS retains; Iced rebuilds. The rebuild model forces a reconciliation layer (`Tree`, `keyed`, `Lazy`) that Buiy doesn't need. Don't borrow this.
4. **Centralized `Message` enum scales linearly with widget count.** Each new interactable adds a variant; nested decomposition costs `.map` calls. Buiy's per-entity observer pattern decouples this — observers can fire per-component without a central message type. Validates Buiy's observer-based reactivity ([architecture.md § 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md)).

## Sources

- Iced book — https://book.iced.rs/the-runtime.html
- Iced 0.14.0 docs — https://docs.rs/iced/0.14.0/iced/
- PR #2463 (Task API + Command rename) — https://github.com/iced-rs/iced/pull/2463
- Iced 0.13.0 release — https://github.com/iced-rs/iced/releases/tag/0.13.0
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Elm Architecture (canonical) — https://guide.elm-lang.org/architecture/
- COSMIC desktop apps (production Iced consumers) — https://github.com/pop-os/libcosmic
- Halloy IRC client (production Iced consumer) — https://github.com/squidowl/halloy
