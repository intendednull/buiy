**Date:** 2026-06-26
**Status:** active
**Subject:** Druid — Linebender's data-oriented Rust UI toolkit (2018–2023, discontinued), analyzed through the state-management / MVU lens for Buiy prototype-3 (MVU-as-core)

# Druid

[`druid`](https://github.com/linebender/druid) was Linebender's **data-oriented, data-first** Rust-native UI toolkit — the direct architectural ancestor of Xilem/Masonry (see [`../xilem-masonry/history.md`](../xilem-masonry/history.md) and [`../xilem-masonry/linebender-stack.md`](../xilem-masonry/linebender-stack.md) for the full Druid → Xilem lineage). Raph Levien started it as a side project at Google ~2018; it reached `0.8.3` (2023-02-28) and was then **discontinued** in favour of Xilem. The README now reads, verbatim: *"The Druid project has been discontinued."*

For Buiy's prototype-3 — the bet that an MVU **message substrate belongs in `buiy_core`** so the log is *complete* (covers widget-internal state) — Druid is the single most instructive prior art in the whole corpus, because **Druid is the data-first design that did NOT reify state changes as messages**. It had a single source of truth (the `Data` value) and a retained widget tree that mutated it *imperatively in place*, with `Command`/`Selector` as a *separate, partial* side-effect channel. The result: **Druid shipped real apps for ~5 years with a single-source-of-truth model and never had time-travel or replay** — and the public record of *why* it was superseded names exactly the seams Buiy's funnel is trying to close. Druid is the natural-experiment that tells you which parts of the MVU-as-core thesis are load-bearing and which parts are where Druid bled.

This folder analyzes Druid strictly through the **state-management / MVU** lens (not its renderer, `piet`, or `druid-shell`, all superseded — see [`../xilem-masonry/linebender-stack.md`](../xilem-masonry/linebender-stack.md)). The consult-when-designing synthesis is [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md).

## Honest assessment (maturity + why it was superseded)

- **Discontinued, not merely stale.** Last release `0.8.3` on 2023-02-28; the README explicitly declares the project discontinued and points downstream to Xilem ("heavily inherits from Druid" but with "fundamental changes to allow for a wider variety of applications with better performance"). This is a *completed* design with a *documented* post-mortem — the most valuable kind of prior art.
- **It shipped real, non-trivial apps.** Druid's development was "largely driven by its use in **Runebender**, a new font editor" (Raph's own app). The project showcase ([issue #1360](https://github.com/linebender/druid/issues/1360)) lists **Psst** (a native Spotify client — a genuinely complex, stateful app), Scribl, kondo, kiro-synth, Zeitig, and others. So "data-first single-source-of-truth core for retained-mode Rust GUI" is *proven viable at app scale* — it is not vapourware. That matters: the MVU-as-core posture is not unprecedented.
- **Why it was superseded (Raph's own words).** The public design record — ["Towards principled reactive UI"](https://raphlinus.github.io/rust/druid/2020/09/25/principled-reactive-ui.html) (2020-09-25), ["Rust 2021: GUI"](https://raphlinus.github.io/rust/gui/2020/09/28/rust-2021.html), and the [Xilem paper](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html) (2022-05-07) — names four drivers, none of which is "single source of truth was wrong":
  1. **The `same()`/diffing model has scaling friction.** *"We have found that the heavy reliance on diffing creates its own problems, depending on how closely the app state fits into the paradigm of immutable (and therefore easily diffed) tree data structures."*
  2. **`Lens` confuses newcomers and doubles the work.** Integrating a component is *"each a half-lens, requiring the writing out of two pieces of logic"* (the read half and the write half); *"a lot of people coming to Druid find them confusing."*
  3. **Async was a poor fit.** *"Integration with Rust's async ecosystem is a major feature for a UI toolkit, and something the existing Druid architecture struggles with."* (`ExtEventSink::submit_command` was the bolt-on.)
  4. **The five-method OOP `Widget<Data>` trait is a lot of boilerplate per widget.**
  Xilem kept Druid's **single-source-of-truth** idea and replaced the *mechanisms*: view-as-pure-function-of-state, **id-path** message routing (replacing `same()` whole-tree targeting), **`Adapt`** (replacing `Lens`), `PartialEq` memoization, and a baked-in `tokio` runtime. **The SSoT survived; the diffing model, Lens, the bolt-on async, and the OOP widget trait did not.** For Buiy this is the headline: the parts Druid got *wrong* are mechanism choices Buiy can avoid; the part it got *right* (model is a value) is what Buiy-MVU doubles down on.
- **One critical thing Druid never did: a complete log.** Despite being "data-first", **widget-internal/ephemeral state lived in the retained `Widget` object, not in `Data`** (evidence below). Druid had *no* mechanism to record or replay state changes — there was no message between "user clicked" and "model mutated", just `&mut data` inside an event handler. This is the exact gap the proto-3 charter calls the *un-reflected `TextEditState` crux*, and Druid is direct evidence that it is real and that single-source-of-truth alone does **not** close it.

## Key facts (verified 2026-06-26)

| Fact | Value |
|---|---|
| Crate | `druid` |
| Latest / final version | **0.8.3**, published **2023-02-28** (project discontinued thereafter) |
| Prior releases | 0.8.0–0.8.2 (Jan 2023), 0.7.0 (2021-01-02) |
| License | **Apache-2.0** (per crates.io metadata; Buiy targets `MIT OR Apache-2.0`) |
| MSRV | not declared |
| Lifetime downloads | ~289,865 |
| 90-day downloads | ~15,183 (residual; still pulled by legacy deps) |
| Repository | https://github.com/linebender/druid |
| Author / lead | Raph Levien (started at Google ~2018, moved under Linebender) |
| Paradigm | **Data-oriented retained-mode**: single `Data` model + retained `Widget<T>` tree + `Lens` projection. **NOT MVU/Elm** — no `Msg` type, no reducer, no log. |
| State mutation | **Imperative, in place**: widgets get `&mut T` in `event()` and mutate the model directly |
| Change detection | `Data::same(&self, &Self) -> bool` (a cheap/false-negative-allowed alternative to `PartialEq`) drives a whole-tree `update` pass |
| Side-effect / message channel | `Command` = `Selector<T>` + payload + `Target`; delivered as `Event::Command`; **separate from and partial relative to** the primary `&mut data` path |
| App-level hook | `AppDelegate` (`event`/`command`/`window_added`/`window_removed`), all with `&mut T` |
| Local widget state | `Widget` private fields (ephemeral) + opt-in `Scope`/`ScopePolicy` (nested local model) — **both outside `Data`, both unrecordable** |
| Renderer / text / shell | `piet` + `druid-shell` (all superseded by Vello/Parley/winit; out of scope here) |
| Time-travel / replay | **None.** No action log, no record, no replay. |
| Successor | Xilem + Masonry (see [`../xilem-masonry/`](../xilem-masonry/)) |

## The state model (the part that matters for MVU-as-core)

Druid's overview names **three pillars**: the `Data` trait (your model), the `Widget` trait (your UI), and the `Lens` trait (the glue that "associates parts of your model with corresponding UI components"). It is "inspired by … Flutter, Jetpack Compose, and SwiftUI, while … conceptually simple and largely non-magical." Read MVU-style, the mapping is: **`Data` is the Model; there is no `Msg` and no `update` — `event()` mutating `&mut data` is *both* at once**; `Widget::layout`+`paint` are the *View*; `Lens` is sub-state addressing; `Command` is a *partial* side-effect bus.

### 1. `Data`: the single source of truth, compared by `same()` not `PartialEq`

```rust
pub trait Data: Clone + 'static {
    fn same(&self, other: &Self) -> bool;
}
```

- **`same()` over `PartialEq`** is deliberate: *"If it returns `true`, the two values **must** be equal, but two equal values need not be considered the same here."* False negatives are allowed (cheaper checks; e.g. `Arc`/`Rc` have blanket `Data` impls that do **pointer** comparison — `same()` is "do these share storage", not "are these structurally equal").
- **`Data` must be cheap to clone *and* cheap to compare.** The framework keeps the previous frame's model to diff against, so the whole model is cloned/compared on the hot path. Expensive types must be wrapped in `Arc`/`Rc`. **`Vec` and `HashMap` deliberately do NOT implement `Data`** — they would be O(n) to clone and compare — which is a loud, on-the-record admission that the whole-model-diff design does not scale to large unwrapped collections.
- `#[derive(Data)]` is **recursive**: every field must itself be `Data` (C-style enums also need `PartialEq`).
- The single `AppState` is the canonical root: `#[derive(Clone, Data, Lens)] struct AppState { … }`.

### 2. `Widget<T>`: a retained, *imperatively-mutating* tree (five methods)

The `Widget` trait is generic over the data slice `T` it operates on. The lifecycle is a fixed five-method cycle:

| Method | Role | Data access |
|---|---|---|
| `event(&mut self, ctx, event, data: &mut T, env)` | OS/user events | **`&mut T` — "the only place where your model can change"** |
| `lifecycle(&mut self, ctx, event, data: &T, env)` | framework-state changes (focus, mount, etc.) | `&T` |
| `update(&mut self, ctx, old_data: &T, data: &T, env)` | react to a model change | `&T` (old **and** new) |
| `layout(&mut self, ctx, bc, data, env) -> Size` | Flutter-style constraint layout | `&T` |
| `paint(&mut self, ctx, data, env)` | draw | `&T` |

The cycle: **`event` may mutate `data` → framework checks whether the model changed → if so it runs the `update` pass (handing each widget `old_data` and `data`) → widgets that observe a relevant change request `layout`/`paint`.** Crucially, **the state transition is an arbitrary Rust closure inside `event()`** — there is no reified, serializable "what happened". This is the structural reason Druid cannot record or replay: there is nothing *to* record between the event and the mutation.

### 3. `Lens<T, U>`: compile-time sub-state addressing (the ancestor of Xilem `Adapt`)

```rust
pub trait Lens<T, U> {
    fn with<V, F: FnOnce(&U) -> V>(&self, data: &T, f: F) -> V;
    fn with_mut<V, F: FnOnce(&mut U) -> V>(&self, data: &mut T, f: F) -> V;
}
```

- A `Lens` is a **two-way projection** from a container `T` to a field `U`: read via `with`, write via `with_mut`. It uses **closure-passing** (not "return `&U`") so the compiler can inline both the lens and the closure to zero cost, and so the parent retains ownership of `T`.
- `#[derive(Lens)]` generates one associated-const lens per field (`AppState::which`, `AppState::value`); `.then(...)` composes (`Outer::inner.then(Inner::text)`); `LensWrap`/`Widget::lens` adapt a `Widget<U>` to run inside a `Widget<T>` context.
- This is **how "every widget reads/writes its slice of the model" is expressed in Druid** without a reconciliation tree (contrast iced's position-keyed `Tree`; see [`../iced/architecture.md`](../iced/architecture.md)). It is also the feature Raph singles out as the newcomer-confuser and the source of "write two halves per component."

### 4. `Command` + `Selector`: the *partial* effect / cross-widget message bus

This is the **only message-shaped part of Druid**, and it is *not* on the primary state path:

```rust
let sel: Selector<Vec<usize>> = Selector::new("process_rows");      // typed, string-id'd
let cmd: Command = sel.with(rows);                                   // payload attached
ctx.submit_command(cmd.to(Target::Global));                         // submit from EventCtx / ExtEventSink / DelegateCtx
// … received elsewhere in event():
if let Some(rows) = cmd.get(sel) { /* handle */ }
```

- `Selector<T>` is a **typed-but-stringly-named** identifier (`Selector::new("...")`); `Command` = selector + payload + `Target` (`Widget` / `Window` / `Global` / `Auto`). Widgets receive it as `Event::Command` in `event()`.
- `ExtEventSink::submit_command` is the **async re-entry point**: a background thread completes work and submits a `Command` back onto the UI thread. This is the *one* place Druid is message-mediated — and notably it is the bolt-on Raph says the architecture "struggles with."
- The split is the key observation for Buiy: **primary state mutation is direct `&mut data` (unrecorded, not message-shaped); cross-cutting effects are `Command` (message-shaped, but never logged).** Druid has *two* state-change paths and *neither* is recorded. A funnel that is optional or covers only part of the surface is exactly this — which is why the charter's "complete recordable stream" requires the funnel to be the *only* path, in core.

### 5. `AppDelegate`: a real central choke-point — but only over `Command`s

```rust
fn event(&mut self, ctx, window_id, event: Event, data: &mut T, env) -> Option<Event>;     // intercept all non-command events first
fn command(&mut self, ctx, target: Target, cmd: &Command, data: &mut T, env) -> Handled;    // called for EVERY command before tree delivery
```

`AppDelegate::command` is called with **every** `(Target, Command)` pair *before* it reaches the tree, with `&mut T` global-model access, and returns `Handled::Yes/No` to swallow or forward. This is genuinely the shape of a **single ordered drain with a record tap** — a central point that sees every command and can mutate the model — and it *validates* that such a choke-point is the natural place to record/intercept (it is exactly where you'd hang a recorder). Its limitation is precisely the charter's thesis in miniature: **`AppDelegate` is optional and sees only `Command`s, never the direct `&mut data` widget mutations**, so even with a delegate the log would be incomplete.

### 6. `Env`: read-only ambient context (theming), threaded everywhere

`Env` is the immutable environment passed to every widget method: typed `Key<T>` lookups for theme colors/dimensions/fonts, with `EnvScope` overriding values for a subtree. It is a clean **read-only ambient context** pattern — a loose parallel to Buiy's `PureEnv` (the read-only env a reducer is allowed to read), with the same discipline ("this is read-only context, not mutable state"). The parallel is structural only: `Env` carries theme/config, not the determinism guarantee.

### 7. Widget-internal & local state: the hole that maps 1:1 onto the charter's crux

This is the most load-bearing finding for proto-3. Druid is explicitly "data-first", yet:

- **Ephemeral widget state lives in the `Widget` object, not in `Data`.** `TextBox` keeps `cursor_on`, `cursor_timer`, `scroll_to_selection_after_layout`, and `was_focused_from_click` as **private fields on the widget struct** (`druid/src/widget/textbox.rs`); scroll position is delegated to an internal `Scroll` widget. The documented rule is: *important* state is "lifted" to app-level `Data`; *unimportant implementation details* stay internal to the widget.
- **`Scope`/`ScopePolicy` is Druid's opt-in "give a subtree its own local model" mechanism** — for cases where *"a (potentially reusable) widget is composed of a tree of multiple cooperating child widgets … [but it is] undesirable to complicate the surrounding application state with the internal details of the widget"* (tabs' selected index, a table's sort/filter/scroll). `Scope::from_lens` / `from_function` set up a two-way transfer between the local scope state and the outer state. **But `Scope` state is still outside the recorded `Data` root.**

Both mechanisms put real, interaction-relevant state (cursor, selection, IME, scroll, sort) **outside the single source of truth**. So Druid's "single source of truth" was never actually *single* — it was "app-important state" with a large, deliberately-excluded periphery. **That periphery is exactly the `TextEditState` the proto-3 charter says an optional/app-boundary log cannot capture.** Druid is the existence proof that a data-first design naturally pushes ephemeral state out of the model — and therefore out of any replay — *unless the architecture forces it through the funnel*. That forcing is precisely proto-3's core bet.

### Time-travel / replay: does Druid support it? No — and the reason is the lesson

**Druid has no time-travel, no record, and no replay**, and there is no community pattern for it (confirmed by search: no Druid + time-travel material exists). The single-source-of-truth `Data` value is *necessary* for replay but nowhere near *sufficient*, because:

1. **State transitions are not reified.** The transition is an opaque `&mut data` closure inside `event()`. There is no value you could log and re-fold. (Contrast Elm/Redux/Buiy-MVU, where the `Msg` *is* the loggable value.)
2. **The model is not complete** (§7): cursor/selection/scroll/IME live off-model, so even a snapshot of `Data` would not capture the editing surface.
3. **The one message-shaped channel (`Command`) is partial and unlogged**, and async results enter through it non-deterministically.

The takeaway is sharp and directly supports the proto-3 framing: **what unlocks replay is not "have a single store" but "reify every state change as a logged message and route *all* state through it (including widget-internal state)."** Druid had the store and skipped the reification — and got no replay. Buiy-MVU's `Messages` inbox + ordered drain + `Reflect` record tap + `LogicalId` (proto-2 KEEP set) is the reification Druid lacked; making it *core* is what closes Druid's off-model hole.

## How to read this folder

1. **Designing the proto-3 message substrate / "complete log" claim** → read this README §4–§7 and [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) "AVOID: a partial log" + "KEEP: reify changes as messages". Druid is the cautionary case for any optional/partial funnel.
2. **Deciding widget granularity ("every widget a Model" vs "leaf widgets only route")** → read §7 (`Widget` private fields vs `Scope` vs `Data`) and the lessons file's "Every widget a Model?" section. Druid's tiering (app-model / opt-in `Scope` / ephemeral fields) is concrete prior art for a *tiered* answer.
3. **Performance / 60 Hz floor** → read §1 (`same()`, cheap-clone constraint, `Vec`/`HashMap` excluded) and the lessons "AVOID: whole-tree structural work on every change". Druid's diffing tax is the analog of Buiy's Reflect-record cost.
4. **Sub-state addressing (`LogicalId` vs lensing)** → read §3 and the lessons' Lens row. Druid + Xilem `Adapt` are the lensing lineage; Buiy's entity identity is the alternative.
5. **Central drain / record tap design** → read §5 (`AppDelegate::command`). It is the shape of Buiy's drain, with its incompleteness as the teaching moment.
6. **Lineage / why pre-1.0 substrate churn is principled** → [`../xilem-masonry/history.md`](../xilem-masonry/history.md), [`../xilem-masonry/xilem-architecture.md`](../xilem-masonry/xilem-architecture.md) (`Adapt` = `Lens` successor; id-path routing = `same()`-targeting successor).

## Framing disclosure

These docs are written from a **"Buiy is building MVU-as-core: a recordable `Msg` substrate in `buiy_core` whose log is *complete* (covers widget-internal state) to unlock deterministic tests, time-travel/replay, agent-driving, and hot-reload"** stance (the proto-3 charter). Druid is read **purely through the state-management/MVU lens**; its renderer (`piet`), shell (`druid-shell`), and layout are out of scope and superseded (see [`../xilem-masonry/linebender-stack.md`](../xilem-masonry/linebender-stack.md)).

The lens makes Druid look like a *near-miss*: a data-first single-source-of-truth core that proved retained-mode data-driven Rust GUI ships real apps, but that (a) never reified state changes as messages, (b) deliberately excluded widget-internal state from the model, and (c) had no log/replay. A reader who is *skeptical* of MVU-as-core should weigh the counter-signal honestly: **Druid never made "every widget a Model", kept its widgets imperative, and still shipped Psst and Runebender** — so "leaf widgets stay imperative and only route" is a *proven* point on the design line, while "every widget is a full reducer/actor" is *un*precedented and is where Druid's `same()` whole-tree cost (and the charter's SCALE/PERFORMANCE risk) lives. Druid does not validate the maximal version of the proto-3 bet; it validates the *core* (single store + the value of a central choke-point) and warns hard about the *cost surface* (whole-program structural work per change, off-model ephemeral state, OOP-widget boilerplate, bolt-on async). The corpus may under-weight Druid's renderer/layout strengths because they are out of scope; that is intentional. Druid is **not** an integration target — it is discontinued — only a design reference.

## Sources

- Druid repository (README, discontinued notice, project showcase #1360) — https://github.com/linebender/druid
- Druid book — Overview: https://linebender.org/druid/01_overview.html · Data: https://linebender.org/druid/03_data.html · Widget: https://linebender.org/druid/04_widget.html · Lens: https://linebender.org/druid/05_lens.html
- `druid::Data` / `Lens` / `Command` / `Selector` / `AppDelegate` / `Env` / `widget::Scope` — https://docs.rs/druid/latest/druid/
- `druid/src/widget/textbox.rs` (widget-internal cursor/scroll state) — https://github.com/linebender/druid/blob/master/druid/src/widget/textbox.rs
- crates.io metadata (version, downloads, license; fetched 2026-06-26) — https://crates.io/crates/druid · https://crates.io/api/v1/crates/druid
- Raph Levien, "Towards principled reactive UI" (2020-09-25) — https://raphlinus.github.io/rust/druid/2020/09/25/principled-reactive-ui.html
- Raph Levien, "Rust 2021: GUI" (2020-09-28) — https://raphlinus.github.io/rust/druid/2020/09/28/rust-2021.html
- Raph Levien, "Xilem: an architecture for UI in Rust" (2022-05-07) — https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html
- Cross-links: [`../xilem-masonry/`](../xilem-masonry/) (the successor), [`../iced/`](../iced/) (Elm-architecture peer, structural reference for this folder), [`../floem/`](../floem/) and [`../gpui/`](../gpui/) (sibling Rust state-model references)
- Buiy proto-3 charter — `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md`
- Buiy proto-2 retrospective (KEEP set: `Messages` inbox + drain + record tap, `EntityEvent` routing, `Callback`, `Reflect` log + `LogicalId`, sealed `PureEnv`) — `docs/prototypes/2026-06-26-elm-bevyified-state-PROTO2-RETROSPECTIVE.md`
