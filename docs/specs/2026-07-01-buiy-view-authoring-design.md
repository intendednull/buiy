# Buiy view-authoring surface ("safer V") — FINAL design

**Date:** 2026-07-01
**Status:** Spec / target-state design — FINAL (production, merge-targeted), the Phase-B output of a `prototype-first-development` effort. `[active]`
**Base:** branch from latest `origin/main` (@ `93f370e` at time of writing). The throwaway prototype was cut off `origin/main` @ `6c4ff22`; the shared-base overlap makes the KEEP work an **audited port**, not a rebuild.
**Supersedes:** nothing. Net-new authoring surface layered on the MVU substrate. It does not touch `bsn!` (that surface stays as-is) and does not re-open the paradigm choice.
**Seeds:** the [safer-V prototype retrospective](../prototypes/2026-07-01-safer-v-authoring-RETROSPECTIVE.md) + [journal](../prototypes/2026-07-01-safer-v-authoring-journal.md) (built + RUN across Counter/TodoMVC/scaling, 9/9 green, GPU-verified); the [authoring-paradigm comparison](../prototypes/2026-07-01-authoring-paradigm-comparison.md) + unanimous 6–0 [view-declaration LLM panel](../prototypes/2026-07-01-view-declaration-llm-panel.md); the [demos→MVU migration report](../reports/2026-06-30-demos-mvu-migration-journal.md) (DX-2/DX-3); the [MVU-as-core design](2026-06-29-mvu-as-core-design.md) (the substrate this lowers onto).

> **Prototype lineage.** The prototype built the Iced-style `view(&Model) -> Element<Msg>` surface + a reconciler + typed routing on MVU-as-core, ran it across three apps of rising difficulty, and journalled where it held and broke. It **proved the bet** (patch-in-place, keyed reconcile, and — the headline — that a single-model `view(model)` shape makes **whole-UI record/replay a property of the model alone**, sidestepping the §7.4 keyed-list-replay wall) and **surfaced the pressure points** (`Cmd::task` is a real MVU hole; `Style`-is-a-`Bundle` blocks style patching; widget internals leak into the reconciler; the checkbox double-folds). This spec re-decides every choice with the full picture: **port the validated, redesign the pressure points, and scope the first PR honestly.**

---

## §0. Decision log

"KEEP" = ported with a re-derived rationale. "REFINE/REDESIGN" = the FINAL does it differently. "NEW" = net-new. The **PR** column is the scoping call (§4): **P1** = first mergeable PR; **P2/P3** = explicit follow-ups.

| # | Decision | Disposition | PR | Where |
|---|----------|-------------|----|-------|
| 1 | `view(&Model) -> Element<Msg>` + reconciler as the app-author surface | KEEP | P1 | `buiy_view` |
| 2 | New crate **`buiy_view`**; reconciler + routers live here, **not** core | NEW | P1 | §1 |
| 3 | Typed routing via marker components carrying a `Msg` **value** or bare `fn` (no stored closures) | KEEP | P1 | §2 |
| 4 | `keyed_column(iter, key_fn, view_fn)` — **required** key | KEEP | P1 | §2 |
| 5 | `when(cond, el)` + `Kind::Empty` stable-index conditional | KEEP | P1 | §2 |
| 6 | `Element::map(f: fn(Msg)->Parent)` message-lifting + parent-owned sub-state | KEEP | P1 | §2 |
| 7 | `ui(init, update, view)` one-call install, `M` **inferred from all three** | REFINE | P1 | §2/§3 |
| 8 | Typed tokens `Space`/`Color`/`Radius` (F6), resolved at build/patch against `Theme` | REFINE (was `Space` only) | P1 | §2 |
| 9 | **Patchable styling** — surface emits **decomposed components** (`FlexGap`/`BoxModel`/`FlexParams`), reconciler patches in place | REFINE | P1 | §3 |
| 10 | **Reconcile-before-layout** — reconciler runs `.before(BuiySet::Layout)` | REFINE | P1 | §3 |
| 11 | **Enqueue/apply-controlled-writes-only-on-drift** — the load-bearing anti-log-flood invariant | KEEP (formalized) | P1 | §3 |
| 12 | **Internal `ViewSlot` contract** — buiy_view stamps where a widget's patchable content lives (no child re-walk) | REFINE | P1 | §3 |
| 13 | `on_input` as a bare `fn(String)->Msg` (covers enum-ctor drafts) | KEEP | P1 | §2 |
| 14 | Reconciler + view **work-counters** + the **W4 steady-frame go/no-go gate** | NEW | P1 | §5 |
| 15 | **`Cmd::task`** (async as a value from `update`) + `Cmd::map` + `Origin::Command` replay | REDESIGN | **P2** | §3 |
| 16 | **Controlled-leaf mode** — suppress the widget's built-in `advance_toggle_on_press` | REFINE | **P3** | §3 |
| 17 | **Boxed/`Arc` `on_input`** for *capturing* per-row input handlers | REFINE | **P3** | §3 |
| 18 | **Upstream `ViewWidget` slot trait** — third-party widgets declare their own slot | NEW | **P3** | §3 |

The bet is triple-corroborated (maintainer lean + 6–0 panel + the demos report's DX-2/DX-3). §6 records the rejected paradigms.

---

## §1. Crate + module boundary

### The `buiy_view` crate

The surface lives in a **new `buiy_view` crate**, not a module in `buiy` and not in `buiy_core`.

```
buiy_view
├── depends on buiy_core     (mvu::{Model, Cmd, enqueue, LogicalId, MvuSet, work-counters};
│                             layout::{FlexGap, BoxModel, FlexParams, Style}; theme::Theme;
│                             interaction::OnPress;
│                             text::edit::{TextEditState, EditCommand, TextChanged, EditSubmitted})
└── depends on buiy_widgets  (Button, Checkbox, TextInput constructors)
```

`OnPress` lives in `buiy_core::interaction` (`buiy_widgets` merely re-exports it); the reconciler's press router reads `MessageReader<OnPress>` from **core**, and needs `buiy_widgets` only for the `Button`/`Checkbox`/`TextInput` constructors.

`buiy` already depends on both `buiy_core` and `buiy_widgets`, and adds a dependency on `buiy_view`; `buiy_view` depends on neither `buiy` nor `buiy_bsn`. **No dependency cycle.**

**Why a new crate, not core.** `buiy_core::mvu` is the state substrate — the recordable message funnel that widgets, `bsn!`, and *any* authoring surface route through. The view-function + reconciler is **one** authoring surface layered on top of it; widgets and `bsn!` do not need it, and coupling it into core would make core carry a `buiy_widgets` dependency (the reconciler realizes real `Button`/`Checkbox`/`TextInput` entities) — an inversion of the layering. Keeping the reconciler out of core also keeps the paradigm-agnostic promise: MVU is the substrate; safer-V is a *client*.

**Why not a module in `buiy`.** `buiy` is the umbrella/prelude crate; it deliberately holds re-exports and the plugin composition, not subsystem logic. The surface is ~1.1 KLoC of real logic (Element + reconciler + routers) with its own test suite — it earns a crate, and a crate lets it be versioned and depended on independently.

**Reconciler + router placement:** `buiy_view` (both). They are generic over `M: Model` and lower onto the real `enqueue` funnel; nothing about them belongs in core.

**The one core-touching exception (§3, PR2):** `Cmd::task` is an *enum variant* on `buiy_core::mvu::Cmd`, which can only be added in core. That is the sole item this effort lands in `buiy_core::mvu`, and it is a scoped follow-up PR — see §4.

### Public API — two import surfaces (`buiy::prelude` + `buiy::view`)

**The name-collision constraint (must be encoded, or the surface does not build).** `buiy::prelude` already re-exports the `buiy_widgets` **scene-fns** `button` / `checkbox` / `text_input_*` — the **`Scene`-returning** `bsn!` styling path (CLAUDE.md §4.1c) — at the crate root, and `prelude = pub use crate::*`. The view-authoring builders `button` / `checkbox` / `text_input` are **`Element`-returning** and would collide name-for-name; two named `pub use` of `button` in one module is a hard `E0252` at the crate root, *before* any glob ambiguity. They are a **distinct authoring surface**, so they get a **distinct import path** — a new `buiy::view` sub-prelude module — rather than being flattened into the everyday prelude (which would break the documented `bsn!` scene-fn path or fail to compile):

```rust
// buiy::view — the view-authoring sub-prelude (NEW module in `buiy`)
pub use buiy_view::{
    Element, Space, Color, Radius,          // description value + typed tokens
    text, button, checkbox, text_input,     // Element-returning builders
    column, row,                            // container macros (see note)
    keyed_column, when,                     // list + conditional
    BuiyViewAppExt,                         // .ui(init, update, view)
};
```

An MVU view-function app imports the view surface plus the MVU/Bevy types it needs:

```rust
use bevy::prelude::*;                        // App, Component, Reflect, …
use buiy::prelude::{BuiyPlugin, Cmd, Model}; // the MVU/plugin types — BY NAME
use buiy::view::*;                           // Element, the Element-returning builders, ui(), tokens
use buiy::view::column;                      // disambiguate `column!` from the std built-in
```

**As-built correction.** The two `button`s must not land in one glob — but `use buiy::prelude::*;` **does** pull the `Scene`-returning scene-fn `button` (the prelude is `pub use crate::*`), so glob-importing *both* `buiy::prelude::*` **and** `buiy::view::*` makes `button` (and `checkbox`) ambiguous on use (`E0659`). The working pattern is therefore: glob **only** `buiy::view::*` for the view surface, and pull the MVU/Bevy types a view app also needs (`Model` / `Cmd` / `App` / `BuiyPlugin`) **by name** from `buiy::prelude` (an explicit import does not glob-collide). A `bsn!` scene author, conversely, keeps the untouched `buiy::prelude` scene-fns and never globs `buiy::view`. `column!` / `row!` / `text!` are `#[macro_export]` macros that surface at `buiy_view`'s crate root (as `bsn!` does today) and are re-exported **by name** through `buiy::view` — a single `pub use buiy_view::text` re-exports **both** the builder fn `text` and the macro `text!` (distinct namespaces, one path). One further papercut: the **`column!`** macro collides with the **`std::column!`** built-in under a glob, so it is imported **by name** (`use buiy::view::column;`, or the whole surface by name as the shipped examples do); `row!` / `text!` have no std collision.

---

## §2. The public API (the star)

The whole app-author surface is **`Model` + `enum Msg` + `fn update` + `fn view`** — nothing else. No `route_*`, no `bind_*`, no `Changed<Model>` system. This is what deletes DX-2 (no declarative view) and DX-3 (no `OnPress→Model` routing).

### The finalized Counter (final API)

```rust
use bevy::prelude::*;
use buiy::prelude::{BuiyPlugin, Cmd, Model}; // MVU/plugin types — by name (§1)
use buiy::view::*;                           // Element, the Element-returning builders, ui(), tokens
use buiy::view::column;                      // disambiguate `column!` from the std built-in

#[derive(Component, Default, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter { count: i32 }

impl Model for Counter { type Msg = Msg; }

#[derive(Clone, Debug, PartialEq, Reflect)]
enum Msg { Inc, Dec, Reset }

fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
    match m {
        Msg::Inc => s.count += 1,
        Msg::Dec => s.count -= 1,
        Msg::Reset => s.count = 0,
    }
    Cmd::none()
}

fn view(s: &Counter) -> Element<Msg> {
    column![
        text!("Count: {}", s.count).size(48.0),
        row![
            button("-").on_press(Msg::Dec),
            button("+").on_press(Msg::Inc),
            button("Reset").on_press_maybe((s.count != 0).then_some(Msg::Reset)),
        ].gap(Space::Sm),
    ]
    .gap(Space::Md)
    .padding(Space::Xl)
    .align_center()
}

fn main() {
    App::new()
        .add_plugins((DefaultPlugins, BuiyPlugin))
        .ui(Counter::default(), update, view)   // ← the whole install
        .run();
}
```

### The finalized Todo row (final API)

```rust
keyed_column(
    s.items.iter(),
    |t| t.id,                                   // REQUIRED key — reorder/insert-safe identity
    |t| {
        let id = t.id;
        row![
            // `on_toggle` resolves f(!checked) eagerly → a plain Msg value; the closure's
            // captured `id` never outlives the `view` call, so nothing is stored on an entity.
            checkbox(t.done).on_toggle(move |_| TodoMsg::Toggle(id)),
            text(&t.title).size(20.0),
            button("X").on_press(TodoMsg::Remove(id)),
        ]
        .gap(Space::Sm)
        .align_center()
    },
)
.gap(Space::Sm)
```

### `Element<Msg>` — the inert description value

`Element<Msg>` is a plain value (not an entity): a widget `Kind`, typed props, modifier state, a typed handler, and children. Built by the builders, consumed by the reconciler. Ported verbatim from the prototype (validated); the prop set gains the decomposed-style fields (§3, #9).

### Builders + uniform dot-method modifiers

Re-derived KEEP — the 6–0 panel's decisive ergonomic point was a **uniform dot-method API** (no method-vs-bare-attribute split), which yields clean compiler errors and matches near-Iced training priors:

- `text(s)` / `text!("fmt", ..)` — a text node.
- `button(label)` — a labelled button (real `buiy_widgets::Button`).
- `checkbox(checked)` — a controlled `buiy_widgets::Checkbox` (model is the source of truth).
- `text_input(value)` — a controlled single-line `buiy_widgets::TextInput` (command-sourced editor).
- `column![..]` / `row![..]` — containers.
- Modifiers (return `Self`): `.gap(Space)`, `.padding(Space)`, `.align_center()`, `.size(f32)`, `.disabled(bool)`, `.background(Color)`, `.radius(Radius)`, `.placeholder(s)` (text-input).

### Event handlers — the replay-safety rule

**The rule: a handler stores a `Msg` *value* or a bare `fn` pointer — never a captured closure.** A value or a capture-free `fn` cannot close over a `Res` snapshot that would diverge on a fresh-process replay; the recorded thing is always the *resulting `Msg`* folded through the funnel, not the handler. This is the same discipline the AT seam's bare-`fn` reducer uses (MVU §5.5).

| Handler | Signature | Storage | Why replay-safe |
|---|---|---|---|
| `on_press(Msg)` | value | `PressAction<M>{msg,model}` component | a value |
| `on_press_maybe(Option<Msg>)` | value/none | present ⇒ enabled+routes; none ⇒ disabled+dimmed | a value |
| `on_toggle(impl FnOnce(bool)->Msg)` | **eager** | resolves `f(!checked)` at build time → a `PressAction` value | eager eval dissolves the capture; nothing stored |
| `on_input(fn(String)->Msg)` | **bare fn** | `InputAction<M>{f,model}` | a bare fn cannot capture; result Msg is recorded |
| `on_submit(Msg)` | value | `SubmitAction<M>{msg,model}` | a value |

The asymmetry, made explicit: **toggle can resolve eagerly** (its target is deterministic given `checked`), so it accepts a *capturing* `FnOnce` and dissolves it; **`on_input` cannot** (the value is only known at keystroke time), so it is constrained to a bare `fn`. `Msg::SetDraft` — an enum tuple-variant ctor — *is* exactly `fn(String)->Msg`, so the common case is clean. A *capturing* per-row `on_input` (inline title edit, `move |s| Edit(id, s)`) needs a boxed/`Arc` handler — deferred to **P3** (#17), documented as a known gap, guarded by the existing `debug_assert` in `map`.

### Routers (library-generic, DX-3)

Three routers, all generic over `M: Model`, installed by `ui()`; the app writes none:

- `route_presses<M>` — `OnPress(e)` → `PressAction<M>` → `enqueue`. Carries buttons **and** checkbox toggles.
- `route_text_input<M>` — `TextChanged(e)` → read editor value → `(InputAction.f)(value)` → `enqueue`.
- `route_text_submit<M>` — `EditSubmitted(e)` → `SubmitAction.msg` → `enqueue`.

All run in `MvuSet::Enqueue`; the pinned `ApplyDeferred` flushes their writes into the same-frame drain.

### `keyed_column` — required key (KEEP, #4)

`keyed_column(iter, key_fn, view_fn)` takes the key as a **mandatory argument** — the deliberate designed-out fix for the panel's silent-`.key()` landmine. The reconciler matches rows by `RowKey`: spawn-new / despawn-gone / **reorder-in-place without rebuild**, preserving each row's widget-entity identity and internal state. Positional `column!`/`row!` diff by index (a front-insert churns siblings — the reason `keyed_column` exists).

### `when` / conditional (KEEP, #5)

`when(cond, el)` renders `el` when `cond`, else a `Kind::Empty` **zero-paint placeholder that still occupies the slot**. Show/hide is a `content↔Empty` kind-swap at a **stable index**, so siblings keep identity. A bare `Option<Element>` at a non-terminal slot changes the child count and churns following siblings — `when` is the blessed spelling; the two-branch `if cond { a } else { b }` (two different *kinds* at one slot) reconciles cleanly too (kind-mismatch ⇒ despawn+respawn in place). **REFINE:** the positional reconciler auto-wraps a `None` child as `Empty`, so even a stray `Option` is churn-free (defense in depth). This auto-wrap is a real conversion, not a hope: the `column!`/`row!` child slots accept `impl Into<Element<M>>`, and `buiy_view` provides `impl From<Option<Element<M>>> for Element<M>` mapping `None ⇒ Kind::Empty` (`Some(e) ⇒ e`), so a stray `Option<Element>` in a positional container lowers to a stable-index `Empty` slot.

### `Element::map` — message-lifting + parent-owned sub-state (KEEP, #6)

`child_view.map(f: fn(ChildMsg)->ParentMsg)` lifts a reusable component's `Element<ChildMsg>` into the parent's `Element<ParentMsg>` (the Elm `Html.map`). The child is held as **parent sub-state** (a field of the one model); its `view`/`update` are reused verbatim; the parent reducer delegates one line (`child::update(&mut s.left, cm)`). `f` is a bare `fn` (an enum tuple-variant ctor like `Msg::Left` qualifies) — `Copy`, determinism-clean.

**This is the composition default, and it is load-bearing for replay:** parent-owned sub-state keeps *all* structural truth in the ONE on-log model, which is exactly what preserves the whole-UI-replay property (§5) through composition. The **machine tier** (child-entity models — what `buiy_widgets::menu.rs` does) is reserved for widgets with genuinely independent lifecycles (menus/dialogs/popovers with their own open/active/dismiss + AT seams); it fragments state across entities and breaks single-model replay, so it is *not* the composition default.

Known limits of `map` (see #15/#17): it drops `on_input` (a bare fn can't be re-tagged into a new bare fn) and can't re-tag a child `Cmd` (`Cmd::map` is P2).

### Typed tokens (F6, #8)

Styling is **typed enums resolved at build/patch time**, never stringly keys:

- `Space::{Xs,Sm,Md,Lg,Xl}` → logical px (gap/padding).
- `Color` → **semantic theme tokens** (`Color::Accent`, `Color::Surface`, `Color::Text`, …). The `Theme` today is a Phase-0 **string-keyed** `HashMap<String, Color>` resolved via `Theme::color(&str)`; the view `Color` enum is a **typed facade** over those keys, each variant pinned to one fixed key — `Accent → "color.accent"`, `Surface → "color.surface.primary"`, `Text → "color.text.primary"`. **As-built refinement (#8):** the reconciler lowers `Color` into a `ColorToken::Token(key)` written onto `Background`/`TextColor` — **not** a concrete color; that token resolves against the live `Theme` at **extract** (`render::color`), so a theme swap re-derives the color with **no reconcile**, and a **missing key** surfaces the loud **magenta sentinel** (`MISSING_TOKEN_FALLBACK`), not a silent compiled default. (Storing the token is also the only shape the component model accepts — `ColorToken` has no concrete-`bevy::Color`-literal variant.)
- `Radius` → corner radius token. **As-built refinement (#8):** render `Radius` is a *value* struct, not a `Component`, so `.radius(..)` patches the entity's `Border.radius: Corners` (a rounded `Border`), not a standalone `buiy_core::render::components::Radius` component.

Tokens are the whole styling vocabulary the surface exposes in P1. Raw literal escape hatches are a follow-up only if a real app needs one.

### `ui(init, update, view)` — one-call install (REFINE, #7)

```rust
trait BuiyViewAppExt {
    fn ui<M, Marker>(&mut self, init: M, reducer: impl IntoViewReducer<M, Marker>,
                     view: fn(&M) -> Element<M::Msg>) -> &mut Self
    where M: Model;
}
```

`ui()` does, in one call: `register_type::<M>()` + `add_model::<M>()` + `add_reducer::<M>()` (the real MVU wiring); inserts `ViewFn<M>` + `UiRoot<M>`; spawns the model entity with a stable `LogicalId(MODEL_LID)` + a 2D camera at `Startup`; installs the three routers (`MvuSet::Enqueue`) and the reconciler (`.before(BuiySet::Layout)`, #10).

**The inference fix:** the prototype had to spell `register_type + add_model + add_reducer` because `mvu_model` infers `M` from the *reducer only*, leaving `view`/`init` unconstrained. `ui()` pins `M` from **all three** arguments (`init: M`, `reducer: FnMut(&mut M, ..)`, `view: fn(&M)->..`) via an `IntoViewReducer<M, Marker>` marker (the `IntoModelReducer` trick, generalized), so a single monomorphic `M` is forced with no turbofish.

**P1 single-root constraint.** `ui()` stamps one **fixed** `LogicalId(MODEL_LID)`, one `UiRoot<M>`, and one 2D camera, so **P1 supports exactly one `ui()` / one root / one model per app**. Two `ui()` calls would collide on the fixed model LID — and a model without a *unique* LID **dead-letters all** its folds (the prototype's frame-1 lesson) — and would spawn a second camera. Multi-root is a deliberate P1 non-goal; the forward fix is to derive the model LID **per model `TypeId`** (distinct models → distinct LIDs) and drop the implicit-single-camera assumption — deferred.

---

## §3. The REFINE / REDESIGN items — the real design work

### #15 — `Cmd::task` (async as a value from `update`) — **REDESIGN, PR2**

**Problem.** `Cmd` is `None | Emit | Batch` only (MVU §8 defers `task`). A pure `update` cannot launch an effect, so the prototype hand-drove async via two out-of-band systems (`launch_load` watches a flag, `poll_load` `enqueue`s the result) — the sharpest DX gap the prototype found.

**Production approach.** Add **one** variant to `buiy_core::mvu::Cmd` (today `None | Emit | Batch`):

```rust
Cmd::Task(BoxFuture<'static, Msg>),            // constructed via Cmd::task(fut, |r| Msg::Loaded(r))
```

with the ergonomic constructor `Cmd::task(future, |result| Msg::Loaded(result))`.

**`Cmd::map` is NOT a variant — it is an eager combinator.** A stored `Cmd<ParentMsg>::Map` variant would have to hold a `Cmd<ChildMsg>` plus `fn(ChildMsg)->ParentMsg` for an arbitrary foreign `ChildMsg`, forcing a second type parameter on `Cmd<Msg>` (or type-erased boxing) — unworkable. Instead `Cmd::map(child_cmd, f)` **rewrites in place**, keeping `Cmd` single-type-param: `None→None`, `Emit(cm)→Emit(f(cm))`, `Batch(v)→Batch(v.into_iter().map(map))`, `Task(fut)→Task(Box::pin(async move { f(fut.await) }))`. This re-tags a child reducer's `Cmd<ChildMsg>` into the parent's `Cmd<ParentMsg>`, completing message-lifting for effect-emitting children — no new variant.

Lowering:

1. The drain, when a fold returns `Cmd::Task`, spawns the future on `AsyncComputeTaskPool` and tracks it (a small `PendingTasks<M>` resource keyed by target `Entity`). It does **not** block.
2. On completion, a core-owned poll system enqueues the mapped `Msg` onto the model — stamped **`Origin::Command`** in the log (this is *why* `Origin::Command` was reserved in the v1 log format; no format break). **Transport surgery this requires — call it out, don't under-scope PR2:** the inbox `Envelope<M>` today carries only `{target, msg}` (no origin), and *both* drains hardcode `Origin::User` on every inbox item. A plain `enqueue()` from the poll system would therefore record as `User`, not `Command`. So PR2 must make the transport **origin-aware** — `Envelope` (or a parallel command-inbox path) carries an `Origin`, and the drain's hardcoded `Origin::User` stamp becomes origin-carrying — so a `Cmd::task` result folds as `Origin::Command`. This is inside the "touches the drain" scope, named here so PR2 is not mis-scoped as "just add a variant".
3. **Replay** re-plays the *recorded result*, not the effect: the replay driver, on an `Origin::Command` entry, re-folds the recorded `Msg` directly and the drain **suppresses** re-launching that fold's `Cmd::Task`. The suppression keys on an explicit **`replaying: bool`** signal (a new field on `RecordSession` — or a separate resource — set **only for the duration of `replay_into`**), **orthogonal to `RecordMode`**. It must **NOT** key on "recording off": production runs `RecordMode::Off` by default (the hot path) and `replay_into` itself stops the session to `Off`, so gating on record-mode would suppress **all** task launches in normal operation — async would be dead in the common case. So a non-deterministic effect (network, clock, RNG) replays deterministically from what actually happened, while a live `RecordMode::Off` run launches tasks normally.

**Rejected alternative:** keep async out-of-band (the prototype's `launch_load`/`poll_load` pattern) as the blessed path. Rejected — it re-introduces exactly the hand-written-system friction (DX-3-shaped) the surface exists to delete, and the effect launch escapes the funnel so replay can't re-derive it. Async-as-a-value is the whole point of `Cmd`.

**Scope call:** **PR2**, not P1. `Cmd::task` touches the core enum + drain + replay driver — a distinct, reviewable change with its own tests. P1 (Counter, TodoMVC) needs no async and is fully coherent without it. An app that needs async before P2 uses the documented out-of-band pattern in the interim.

### #9 — Patchable styling (decomposed components) — **REFINE, P1**

**Problem.** `Style` is a `#[derive(Bundle)]` builder, so the prototype could only apply container style **at spawn, never patch it** — the biggest structural limitation of W1.

**Production approach.** The surface **emits the decomposed components directly** and the reconciler **patches them in place**. `container_style` becomes `apply_container_props(world, entity, el)` which computes and `set_if_neq`-patches the individual layout components the `Style` builder would have produced:

- `.gap(Space)` → `FlexParams.gap` *(as-built: `FlexGap` is a **field** of `FlexParams`, not a standalone component, so gap patches `FlexParams`)*
- `.padding(Space)` → `BoxModel` (padding edges)
- `row!`/`column!` + `.align_center()` → `FlexParams` (direction + `AlignItems`)
- `.background(Color)` → `Background` *(as-built: holds a `ColorToken`, not a resolved color)*; `.radius(Radius)` → `Border.radius` *(as-built: render `Radius` is a value struct, not a `Component`, so a rounded box is a `Border` with `Corners`)*

At **spawn** the reconciler still assembles the full `Style` bundle for defaults (one source of truth for initializer values); at **patch** it writes only the decomposed components that changed. Because these are real components, a runtime style change (e.g. a token that depends on model state) now patches without a rebuild.

**Rejected alternative:** make `Style` itself a `Component` (`Mutable`). Rejected — `Style` is a *Bundle builder* used pervasively across `buiy_widgets`, `bsn!`, and every `#[require]` contract; converting it is a large cross-cutting refactor with blast radius far beyond this surface, for a benefit (uniform patch) the decomposed-patch path already delivers locally.

### #12 — Widget-slot contract (internal) — **REFINE, P1**

**Problem.** A `Button`'s label is a *child* `Text` + a root `A11yLabel`; the prototype's reconciler walked into the child to patch the label, coupling it to each widget's internal layout.

**Production approach (P1, no `buiy_widgets` change).** `buiy_view` stamps a `ViewSlot { label: Option<Entity> }` component on each realized widget root **at spawn**, recording where its patchable content lives (it knows it just called `Button::new`, so it finds the label child once and records it). The reconciler reads `ViewSlot` on patch — no re-walk, no per-widget special-casing. This keeps the contract entirely inside `buiy_view`; production widget crates are untouched.

**Suppression-gotcha guard (CLAUDE.md §4.1c).** On a realized *widget* the reconciler only ever patches its **label** (via `ViewSlot`), its **handlers**, and its **opacity/enabled** state — it **never single-field-patches a widget's `#[require]`'d contract component** (which would drop that component's other defaults). Decomposed **style** patching (#9) targets **containers only** — the `column!`/`row!` roots that own their own layout components — never a widget's required components.

**Rejected/deferred alternative:** a public `ViewWidget` trait that widgets *implement* to declare their own slot(s) — the right answer for **third-party** widgets, but it touches `buiy_widgets` and defines public API, so it is **P3** (#18). P1's internal `ViewSlot` covers the three built-in widgets the demos use and removes the coupling the prototype flagged.

### #16 — Controlled-leaf mode — **REFINE, P3**

**Problem.** The core's `advance_toggle_on_press` leaf always fires on a checkbox press (unsuppressible per-widget), so a controlled checkbox **double-folds**: the leaf flips `A11yToggled` (immediate visual) *and* the router enqueues the model `Msg`. They **converge** (the reconciler re-asserts `A11yToggled` from the model via a single-writer `ToggleMsg::Set`-on-drift), so there is no flicker — but the surface can't cleanly *own* the widget.

**Production approach.** Add an opt-out marker `ControlledLeaf` in `buiy_core`; `advance_toggle_on_press` gains a `Without<ControlledLeaf>` filter. `buiy_view` stamps `ControlledLeaf` on checkboxes it owns, making the model route the **sole** writer.

**Scope call:** **P3**. The prototype proved the double-fold is *correct* (converges via drift-reassert) — P1 ships without suppression, documenting the redundant fold. `ControlledLeaf` is a small, surgical core change (one `Without` filter + one marker) that is a clean, independently-reviewable follow-up. **Rejected alternative:** have the reconciler despawn/replace the widget's built-in toggle systems per-entity — impossible/ugly in ECS; the marker-filter is the idiomatic opt-out.

### #10 — Reconcile-before-layout — **REFINE, P1**

**Problem.** The prototype reconciled in `MvuSet::Bind` (after `BuiySet::Layout`, since the MVU chain sits `.after(A11yUpdate)`), so structurally-new nodes were spawned *after* layout ran and **flashed unlaid-out for one frame** before layout caught them the next frame.

**Production approach.** The reconciler runs in a `ViewSet::Reconcile` set ordered **`.before(BuiySet::Layout)`**. Because `BuiySet` order is `Layout → Style → Input → … → A11yUpdate → Render` and the MVU drain folds late (between `A11yUpdate` and `Render`), a front-of-frame reconciler reads the **previous** frame's late-drain `Changed<M>`. The honest tradeoff (corrected from the prototype's "no one-frame lag" framing):

- **Win:** newly-spawned nodes are laid out **in the same frame they are created** — the unlaid-out-node **flash is eliminated**.
- **Cost:** because it reads the prior frame's `Changed<M>`, **value** patches land one drain later than the prototype's Bind-time reconcile (prototype: drain(N)→patch(N)→visible N; here: drain(N)→patch before Layout(N+1)→visible N+1). **Structural** latency is unchanged (visible at N+1 either way).

The one-frame value latency is **by design** and matches retained-mode / Elm / Iced — a builder who sees value patches one frame behind the prototype should know it is intentional, not a regression. Frame-1 seeding still works (the model spawns `Changed` at `Startup`; the first reconcile before layout builds the initial tree and lays it out that frame). This is a pure scheduling change — cheap, P1.

**Scheduling caveat (post-MT-safety).** The reconciler is an exclusive `&mut World` system at the front of the frame, so it is a hard sync barrier every frame and **cannot** use a normal `Changed<M>` run-condition. Its idle-frame cheapness depends on an **internal `Changed<M>` emptiness early-out** (an internal `QueryState` it checks first, returning immediately when empty). Confirmed acceptable under the `multi_threaded` lane; a non-exclusive `Commands`+queries reconciler is the fallback if the front-of-frame sync cost ever bites.

### #7 — `ui()` M inference — **REFINE, P1**

Covered in §2: `M` pinned from `init` + `reducer` + `view` via `IntoViewReducer<M, Marker>`.

### #11 — Enqueue/apply-controlled-writes-only-on-drift — **KEEP (formalized), P1**

The drain records **every** folded `Msg`, including idempotent `set_if_neq`-noops (`messages_recorded` bumps before the change test). So a reconciler that enqueued a controlled `Set` (checkbox re-assert) or re-applied the editor value **every frame** would flood the replay log. **Invariant (tested):** the reconciler enqueues a controlled write / applies an editor `apply()` **only on real drift** (`set_checkbox_checked` writes `ToggleMsg::Set` only when `A11yToggled` differs; `set_editor_value` calls `apply()` only when the buffer differs). Controlled writes use the **low-level `apply()` seam** (not the recorded keyboard system), so they emit no `TextChanged`/`EditLog` — no feedback loop, no log pollution. (`clear ≠ Insert("")`: clearing is `SelectAll`+`Delete`, since an empty insert deletes nothing.)

**Deferred non-goal — byte-identical editor internals.** Whole-UI replay reconstructs each editor's **value** from the model, but mid-edit **caret/selection byte-identity** is **not** replayed; that would need reconciler-assigned stable field `LogicalId`s per editor, deferred (consistent with MVU-as-core §7.3's scoped replay). The off-log editor-internal entries that dead-letter on replay are harmless precisely because the value is model-reconstructed.

---

## §4. SCOPING — the first mergeable PR

The bias from the retrospective holds: **the core view surface is a strong, coherent first PR**; the heavy core-touching items are honest follow-ups. Concretely:

### PR1 — `buiy_view` foundation (the first mergeable, reviewable PR)

**Ships:** the `buiy_view` crate; `Element<Msg>`/`Kind`; builders (`text`/`button`/`checkbox`/`text_input`) + `column!`/`row!`/`text!`; uniform dot-methods; typed tokens `Space`/`Color`/`Radius` wired end-to-end (#8); handlers `on_press`/`on_press_maybe`/`on_toggle`/`on_input`(bare fn)/`on_submit` with the replay-safety rule; `keyed_column`; `when`/`Empty` (+ `Option`→`Empty` auto-wrap); `Element::map` (value + bare-fn handlers); `ui(init, update, view)` with `M`-inference; the reconciler (positional + keyed) with **decomposed-style patching** (#9), **reconcile-before-layout** (#10), the internal **`ViewSlot`** contract (#12), and **drift-only controlled writes** (#11); the three routers; view work-counters + the **W4 go/no-go gate** (#14); the `buiy::view` sub-prelude module re-exporting the surface through `buiy` (§1); **Counter + TodoMVC re-authored as `examples/`**; the full test suite (§5).

**What PR1 can do:** author Counter and TodoMVC as `Model+Msg+update+view` and nothing else; patch-in-place; keyed add/remove/reorder; controlled editor draft; conditional show/hide; parent-owned child composition via `map`; **whole-UI record/replay of the single model**; typed styling.

**What PR1 cannot do (and why it's still coherent):**
- **No async from `update`** (#15) — Counter/TodoMVC need none. An app needing async uses the interim out-of-band pattern; P2 lands `Cmd::task`.
- **No capturing `on_input`** (#17) — the enum-ctor draft case (`Msg::SetDraft`) is covered by bare fn; inline per-row text edit waits for the boxed handler. Guarded by `debug_assert`.
- **Checkbox double-folds** (#16) — correct (converges via drift-reassert), just does one redundant leaf fold; `ControlledLeaf` suppression is P3.
- **Only the 3 built-in widgets are reconciler-wired** — third-party widget slots wait for the `ViewWidget` trait (P3, #18).

None of these block authoring a real app; each is documented at its site.

**Note on `map`/`when` demo coverage.** Their combined end-to-end showcase — the scaling demo (embed Counter twice + a `when`-gated panel) — is a **PR2** example, so in PR1 `map` and `when` are validated by the child-lift and conditional **logic tests** (§5), not by a demo app. This is acceptable (both paths are exercised); if a demo-level composition showcase is wanted in PR1, add a minimal two-Counter `map`+`when` example rather than waiting for PR2.

### PR2 — `Cmd::task` in core + async composition (follow-up) — **DELIVERED**

`Cmd::Task` + `Cmd::map` + `Origin::Command` emission + the replay guard in `buiy_core::mvu` (#15); the **scaling demo** (embeds Counter twice via `map`, `when`-gated panel, and a real async load) re-authored as an example; async + `Cmd::map` tests. Depends on PR1. Distinct because it touches the core enum, drain, and replay driver — a self-contained, separately-reviewable change.

**As-built (PR2):**
- **`Cmd::Task(BoxedFuture<'static, Msg>)`** + the ergonomic `Cmd::task(future, map)` constructor. The field type is bevy's own `BoxedFuture` (`Send` on native, `?Send` on wasm via `ConditionalSend`), so the surface is **wasm-clean** by construction (verified: the whole stack compiles for `wasm32-unknown-unknown` under `-D warnings`).
- **`Cmd::map`** is the eager combinator the spec called for (`None→None`, `Emit(m)→Emit(f(m))`, `Batch` recurses, `Task(fut)→Task(async { f(fut.await) })`), `Arc`-sharing the mapper so `Batch` recursion + the `Task` async-move both work.
- **Origin-aware transport (as-built):** `Envelope<M>` gained an `origin: Origin` field (with an `Envelope::user` constructor for the User shorthand); `enqueue` stamps `User`, the new `enqueue_with_origin` stamps `Command`, and both drains read `env.origin` instead of hardcoding `User`. A completed task's result folds — and records — as `Origin::Command`.
- **Lowering:** the drain spawns a returned `Cmd::Task` on `AsyncComputeTaskPool` into a per-model `PendingTasks<M>` bag (a bag, not one-per-entity — the substrate does not silently cancel concurrent tasks); `poll_pending_tasks<M>` (registered per model in `add_model`, running in `MvuSet::Enqueue`) polls and enqueues the result stamped `Command` so it folds the same frame.
- **Replay guard:** a `RecordSession.replaying` flag (set only for the duration of `replay_into`, orthogonal to `RecordMode`) makes the drain **suppress re-launching** a `Cmd::Task`; the recorded `Origin::Command` result is re-folded from the log instead, so a non-deterministic effect replays from what actually happened. Proven by a determinism test (the effect increments a shared counter live; replay reproduces the model with the counter untouched).
- **The buiy_view surface needed NO change** — `ui()` already wires the (now `Task`-capable) env-free drain, so a `view`-authored `update` returning `Cmd::task` gets async end-to-end (the reconciler re-renders on the async-driven model change); proven by `crates/buiy_view/tests/async_task.rs`. This is the clean outcome §1's "the sole core-touching item" anticipated.
- **Root-caused an exposed latent bug:** adding the per-model poll system perturbed system-execution order and revealed that `reset_mvu_counters` was ordered before the *early* leaf drain only by insertion luck (the `BuiySet` backbone is unchained in minimal `MvuCorePlugin`-only apps). Fixed by anchoring the reset explicitly before all three bump sites (`BuiySet::Input`, `BuiySet::Picking`, `MvuSet::Drain`).
- **Ships:** `examples/scaling_view` (windowed `scaling_view` bin + headless `capture_scaling_view` GPU bin, RUN-verified on a real adapter — left=2/right=1 via `map`, the `when` panel, and "42 rows" via `Cmd::task`).

### PR3 — composition completeness (follow-up)

Boxed/`Arc` `on_input` for capturing handlers (#17); the public `ViewWidget` slot trait for third-party widgets (#18); `ControlledLeaf` opt-out in core (#16). Each is small and independent; they can land as one PR or three. None are needed for the demos to work.

**As-shipped (PR3):** #16 + #17 delivered; **#18 deferred (deliberately — no consumer yet).**

- **#16 `ControlledLeaf`: DELIVERED.** A marker in `buiy_core::mvu` + a `Without<ControlledLeaf>` filter on `buiy_widgets::advance_toggle_on_press`; `buiy_view` stamps it on the checkbox it spawns. So a view-owned checkbox opts out of the built-in press-to-toggle leaf, and the model route (→ reconciler re-assert → the same `ToggleLeafSet::Drain`) is the **sole source** of its `A11yToggled` fold — the double-fold is gone (the drain stays the sole *writer* either way). Test: `buiy_widgets/tests/checkbox.rs::controlled_leaf_checkbox_opts_out_of_press_to_toggle` (a `ControlledLeaf` checkbox does NOT auto-fold on press; a plain one still does).
- **#17 capturing `on_input`: DELIVERED.** `Element::on_input` now stores an `InputHandler<Msg>` enum — `Bare(fn)` (the replay-safe-by-type default) or `Boxed(Arc<dyn Fn>)` via the new `Element::on_input_with`, for a *capturing* per-row handler (`move |v| Msg::Edit(id, v)` — the inline-edit case). Purity is the author's contract (capture only values; not statically enforceable, mirroring the reducer rule). **Bonus:** this also lets `Element::map` **lift `on_input`** (by boxing `move |s| f(bare(s))`), closing the P1 "map drops on_input" limitation. Test: `buiy_view/tests/input_capturing.rs` (a captured row id + typed value route end-to-end through the real editor path).
- **#18 public `ViewWidget` trait: DEFERRED — speculative until a consumer exists.** The three built-in widgets are covered by PR1's internal `ViewSlot`; a public trait for *third-party* widgets requires making the closed `Kind` enum **extensible** (a real redesign of the `Element` model + reconciler dispatch + a registration path) — a public-API surface with **no current consumer**. Per scope discipline (build for need, not speculation), it should be designed *with* the first third-party-widget use case in hand, so its API fits a real requirement rather than a guess. Left as a documented design item; the internal `ViewSlot` continues to cover the built-ins with zero coupling to `buiy_widgets`.

**The call:** PR1 is a shippable, reviewable, genuinely-useful surface (two real apps authored in it, replay working) that stands alone; PR2/PR3 extend it without reshaping it. This avoids a mega-PR while keeping each follow-up honest about what the base can't yet do.

---

## §5. Verification plan

Every claim is verified by RUNNING (headless logic tests + a GPU capture per demo), mirroring the prototype's discipline. Tests live at the **lowest tier that observes the behavior**.

### Logic tests (headless, no GPU)

- **reconcile / DX-2:** author writes only `view`; assert the reconciler builds the right nodes, **patches the label in place** (identical entity ids across a 0→3→Reset fold), and attaches/detaches the `Reset` handler as the model crosses 0.
- **router / DX-3:** a synthesized `OnPress` routes through `route_presses` → real `enqueue` → drain → model; **no app route system** exists.
- **keyed:** seed ids{0,1,2}; toggle middle, add id3, remove id0 → surviving rows keep **exact** entity ids; only id0 despawns, id3 spawns; the surviving checkbox's real `A11yToggled` reflects the toggle.
- **editor bridge:** `text_input(draft).on_input(SetDraft).on_submit(Add)` — controlled value flows model→editor via `apply()` (no `TextChanged`/`EditLog`); `Add` reads+clears; `clear` = `SelectAll`+`Delete`.
- **replay:** record an add+toggle+remove session; replay the unified log into a fresh same-seed app; assert `replayed_model == recorded_model` (state-identical), the reconciled keyed tree re-derives to match, and **only harmless off-log leaf/editor entries dead-letter** (zero model-targeted folds dead-letter). This is the headline architectural property — whole-UI replay as a property of the single model.
- **conditional:** pin title + toggle button + a later sibling; toggle a `when` panel on/off twice → all three keep entity ids while the slot alternates `Empty`↔content.
- **child-lift:** embed Counter twice via `map`; left `+`×3 / right `+`×1 → `(3,1)` isolated; a left `-` never touches the right; `view`/`update` reused verbatim.
- **styling (P1):** a token-driven style change (e.g. a `Color::Accent` background that depends on model state) **patches in place** (same entity id; `FlexGap`/`BoxModel`/`Background` updated) — proves #9.

### Demos as fixtures

Counter + TodoMVC (P1), scaling (P2) are shipped as `examples/`, each with a windowed bin and a headless GPU-capture bin, and each drives the *same* authored `view`/`update` the tests do. Every visual claim is confirmed by viewing the captured PNGs (per the prototype's re-verify-yourself rule).

**Known cosmetic issue (out of scope).** The TodoMVC checkbox ✓ (U+2713) renders as **tofu** because the default font lacks the glyph — a pre-existing default-font gap (same class as the widget-catalog finding), **not** introduced by `buiy_view`. The GPU-capture PNGs will show it; it is not a regression.

### The W4 go/no-go gate (the prototype skipped this — a real can-fail gate)

**Question:** does rebuild-then-diff defeat `set_if_neq` at steady state? Add **`ViewWorkCounters { reconciles, nodes_spawned, nodes_despawned, nodes_patched }`** (modeled on `MvuWorkCounters`/`RenderWorkCounters`, reset before the reconcile each frame, host-independent). **`nodes_patched` counts actual value-changing writes** — a `set_if_neq` that leaves a component unchanged does **not** increment it, so a walk-and-write-every-node reconciler cannot pass by touching everything. Gate assertions on a **settled** app:

1. **Idle frame:** no model change ⇒ `reconciles == 0` (reconciler is `Changed<M>`-gated; `set_if_neq` leaves an idempotent fold untripped, so it never even runs). This is the load-bearing proof the funnel's `set_if_neq` discipline carries through the reconciler.
2. **Idempotent fold** (enqueue a no-op `Msg`): `MvuWorkCounters.models_mutated == 0` **and** `MvuWorkCounters.binds_fired == 0` **and** `ViewWorkCounters.reconciles == 0` — the no-op does not cascade to a reconcile.
3. **Localized value change** (`Inc`): reconcile runs once, `nodes_spawned == 0 && nodes_despawned == 0` (patch-in-place; no rebuild), and the write set is **bounded to the changed subtree, not the tree size** — `Inc` patches **exactly the one `Count` label**, so `nodes_patched == 1` (not the loose `>= 1`, which a whole-tree rewalk that value-writes the changed node would also satisfy).
4. **Downstream bound (the load-bearing defeats-`set_if_neq` check):** the localized change's downstream **layout dirty-set** must stay **bounded to the changed subtree** — patching the one label must **not** dirty-recompute the whole tree's layout (exactly what a float-noise or walk-everything reconciler would trigger, and what would defeat the funnel's `set_if_neq` discipline). Concretely: `nodes_patched == 1` **and** the layout dirty-count stays a small constant. **As-built refinement (#14):** the metric is `buiy_core::layout::SyncStylesIterCount` (the headless per-frame layout re-translate count), asserted **structurally** (`max < node_count`, and in practice `<= 3`) rather than against `RenderWorkCounters` — the render counters need the GPU app, so the headless W4 gate binds the layout dirty-count directly. A regression — full-subtree rebuild on a value change, a reconcile on an idle frame, or a whole-tree layout re-dirty — **reddens the gate**.

**If the gate reds** (steady-state rebuild storms), the memoization fix (memoize unchanged subtrees / skip clean children) lands as a **bounded fast-follow PR**, *not* by growing PR1 — PR1 stays the fixed-scope surface. The gate must be **green to bless the surface**, but the remedy does not expand the first PR. This is the go/no-go the final owns.

### Mechanical gate

The full workspace check (`cargo fmt` + `clippy -D warnings` + `doc -D warnings` + headless `nextest`) plus the GPU `--ignored` lane for the capture fixtures, per `CLAUDE.md`.

---

## §6. Rejected alternatives (named)

- **`bsn!`-native macro authoring tree.** A macro-DSL scene tree as the app-author view surface. Rejected **6–0 by the LLM-authorability panel**: worse training priors than the near-Iced view-function, a macro-vs-Rust context split, and worse compiler errors. `bsn!` remains the *component-authoring* path (it is `Bundle`-shaped, not a message-routed view); it is not the reactive app surface. This spec does not touch it.
- **Fine-grained signals** (SolidJS/Leptos-style reactive graph). Rejected — it **fights MVU**: signals want many independent reactive cells mutated anywhere, while MVU's whole value proposition is one ordered, recordable funnel with a single drain per model. Signals would fragment the message log and forfeit the whole-UI-replay property that is this surface's headline result.
- **Central-`Msg`-enum-only** (one giant app `Msg`, no `map`/no sub-state). Rejected — no composition story: every reusable component's messages must be hand-merged into the root enum, and there is no `Html.map` to isolate sub-state. `Element::map` + parent-owned sub-state gives modular composition while *keeping* the single-model replay property.
- **Per-widget stored closures** (`on_press(|| ..)` boxed `Fn` on every entity). Rejected as the default — a stored closure can capture a `Res` snapshot that **diverges on a fresh-process replay**, breaking determinism, and it's un-`Reflect`-able so it can't be recorded. The value/bare-fn rule (§2) is replay-safe by type. (A *pure* boxed `Fn` for capturing `on_input` is the narrow, opt-in P3 exception, purity-checked — not the default.)

---

## Appendix — file map (PR1)

```
crates/buiy_view/
├── Cargo.toml                     (deps: buiy_core, buiy_widgets, bevy)
├── src/lib.rs                     Element/Kind, builders, macros, tokens,
│                                  routers, reconciler, ViewSlot, ui(), work-counters
examples/
├── counter_view/                 Counter re-authored (windowed + capture bins)
└── todomvc_view/                 TodoMVC re-authored (windowed + capture bins)
crates/buiy/src/lib.rs            + `pub mod view` re-exporting buiy_view
                                 (the Element-returning builders live under
                                  `buiy::view`, NOT the flat prelude — §1
                                  name-collision constraint)
```

The audited port: `examples/safer_v_proto/src/lib.rs` (throwaway) is the validated base for `crates/buiy_view/src/lib.rs`; the REFINE items (#7–#12, #14) are re-implemented as deliberate commits on top of the ported KEEP work.
