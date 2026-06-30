# Elm-bevyified State Mgmt — Prototype-2 Dev Journal (the Bevy-NATIVE pass)

> **PROTOTYPE-2 — exploratory, DO NOT MERGE.** The deliverable is this journal +
> the prototype-2 retrospective. The code (`examples/mvu_native/`) is an unmerged
> reference. Base: `origin/main@59cd50e` (same as prototype-1, so validated commits
> stay cherry-pickable into the final).

**Goal:** prototype-1 validated the *bespoke* runtime (Inbox/drain/OnOutput/InFlight/
MsgLog) — 33/33 green + byte-identical replay + clean GUI. It did **not** build the
**Bevy-native/leveraged** decisions the design locked. Prototype-2 builds & RUNs the
leveraged version to confirm it before the spec becomes final. Charter = spec § 13:

1. **V-B reducer-as-constrained-system** — `add_reducer(fn(&mut Model, Msg, env) -> Cmd)`
   with purity **type-enforced** (does it compile, and does it actually reject impure
   params?).
2. **`Messages<M>` inbox, our drain owns the record tap** (timing vs `MessageUpdates` GC).
3. **`EntityEvent`/observer routing + upward channel as `observe`** (enqueue-only rule).
4. **Yew-style `Callback<T>`** funnel-dispatch (replay-safe, `#[reflect(ignore)]`).
5. **`Reflect` log + `LogicalId`** (cross-process re-fold byte-identical).

Stack: Bevy **0.19.0** (lock resolved `0.19.0-rc.3` req → `0.19.0` final), edition 2024,
Rust 1.95. Real display `:0` + AMD RX 6700 XT for the GUI wave.

---

## Running log

### 2026-06-26 — Wave 0 prelude: API research (the crux's premise was wrong)

Before writing a line, dispatched an agent to read the **actual 0.19 source** for the
five APIs the charter rides on. One finding **invalidates a locked spec premise** and
is the headline learning of this prototype so far:

- **`Commands: ReadOnlySystemParam` in 0.19.** (`bevy_ecs/src/system/commands/mod.rs`:
  `unsafe impl ReadOnlySystemParam for Commands` — "only reads Entities", because the
  spawn/despawn is *deferred*.) The spec (§ 3, § 12) says reducer purity is "enforced by
  `ReadOnlySystemParam`." **It is not enough:** a reducer `fn update(&mut M, Msg, mut c:
  Commands)` would compile and could `c.spawn(...)`/`c.despawn(...)` — unrecorded
  structural mutation outside the funnel → replay diverges. `ReadOnlySystemParam`
  correctly rejects `ResMut`/`Query<&mut>`/`MessageWriter`, but **not `Commands`**.
  → **Enforcement must be a sealed allowlist trait** (`PureEnv`): bless `Res`/`Query<&T>`/
  `Local`/`()`/tuples, never `Commands`. Secure by construction (allowlist, not denylist).
  This is a REDESIGN finding for the final (the spec's "ReadOnlySystemParam enforces
  purity" line must be corrected to "a sealed `PureEnv` allowlist enforces purity").
- **Messages GC:** `message_update_system` runs in `First`, double-buffers; a message
  written in `Update` is readable in N and N+1, dropped at `First` of N+2. So our drain
  reading later in the *same* `Update`, ordered `.after` the writer, is always safe — no
  race with GC.
- **Generic env param into the drain closure:** use `StaticSystemParam<P>` (its whole
  purpose is "arbitrary SystemParam args to function systems"); sidesteps the
  "implementation is not general enough" HRTB trap. `StaticSystemParam<P>: ReadOnlySystemParam`
  iff `P` is.
- **Routing:** `#[derive(EntityEvent)] #[entity_event(propagate)]` (Traversal defaults to
  `&'static ChildOf`); observer param is **`On<E>`** (not `Trigger`); `trigger_targets` is
  gone → `commands.entity(e).trigger(|entity| Ev{entity})`; `on.propagate(false)` stops it.
- **Reflect log:** `TypedReflectSerializer`/`TypedReflectDeserializer` (+ `FromReflect` to
  go `Box<dyn PartialReflect>` → concrete); `ron` (0.12, already in the lockfile) as the
  format; `app.register_type::<M>()`.
- Generic mutable component access bound: `M: Component<Mutability = Mutable>`
  (`bevy_ecs::component::Mutable`).

Decision for the prototype's reducer shape (so the env item can be shared across the
many per-message reducer calls in one drain run): **env is ONE `SystemParam` passed by
reference** — `fn update(&mut M, Msg, env: &MyEnv) -> Cmd` where `MyEnv` is a
`#[derive(SystemParam)]` struct that is `PureEnv`; the no-env case is a separate
overload `fn update(&mut M, Msg) -> Cmd`. (Bevy's variadic IntoSystem-style bare-param
signature is possible but HRTB-fragile and re-borrow-hostile in a per-message loop;
grouped-env-by-ref is the robust choice. Note for the final.)

### 2026-06-26 — Waves 0 + 1: V-B reducer + inbox + drain + record tap (GREEN)

Built `runtime.rs`: `Model` (assoc-types-only), `Envelope<M>` inbox (Bevy `Messages`,
manual `Message`/`Clone` impls), `Cmd{None,Emit,Batch}`, the sealed `PureEnv` allowlist,
the `Reducer<M,E>` trait, `add_model`/`add_reducer`/`add_reducer_env`, the single ordered
drain (`MvuSet::Drain`), `LogicalId`, `MsgLog` (Reflect-serialized record tap). Counter
demo + 6 tests.

- **Ran it (headless):** `cargo test -p mvu_native` → **6/6 green** (5 unit + 1
  compile_fail doctest). Validated: the drain folds via the V-B reducer; run-to-completion
  `Emit` re-folds within one pass; per-instance state (two Counters, one reducer type);
  the drain records every fold to the `LogicalId`-keyed, RON-serialized log; the
  env-reading reducer reads `&Res<Step>`.
- **The crux compiled.** `add_reducer_env::<Counter, Res<Step>, _>(fn(&mut Counter,
  CounterMsg, &Res<Step>) -> Cmd)` works. Two things made it compile where the naive
  approach wouldn't:
  1. **Env passed by shared `&` into the reducer**, fetched ONCE per drain as
     `StaticSystemParam<E>` then reused across every fold (SystemParam items aren't
     `Clone`, so by-value-per-message is impossible). `StaticSystemParam` is the
     documented escape from the "implementation is not general enough" HRTB trap for a
     generic env param in a closure-system — confirmed, it Just Works.
  2. **Env type `E` given by turbofish** (`::<Counter, Res<Step>, _>`). Inference can't
     run backwards through the `SystemParamItem<E>` associated-type projection, so the
     bare-param Bevy-`IntoSystem` signature isn't reachable without the variadic-macro +
     double-`FnMut`-bound machinery. Turbofish is the robust prototype choice; a v1
     could add the macro for `fn update(&mut M, Msg, Res<Step>)` ergonomics. **REFINE.**
- **Purity gate PROVEN (the headline).** A reducer taking `&Commands` as its env does
  **not compile** — exact error: ``the trait bound `Commands: PureEnv` is not
  satisfied``, pointed at the `E: PureEnv` bound, with a help-list of the blessed types
  (`()`, `Res`, `Query`, tuples). This is the real enforcement the spec's
  `ReadOnlySystemParam` premise could not give (Commands satisfies that). The sealed
  allowlist is the load-bearing mechanism. **The spec § 3/§ 12 enforcement line must be
  corrected in the final → carried as a REDESIGN.**
- **Messages GC timing — confirmed safe in practice:** writing to `Messages<Envelope>`
  then `app.update()` (First GCs, then Update drains) leaves the message readable by our
  drain the same frame; tests fold the just-written messages with zero loss. The
  `.after`/set-ordering the research predicted holds.
- Surprised by: `AppTypeRegistry` is in `bevy::prelude` (not `bevy::reflect`). Minor.

### 2026-06-26 — Wave 2: EntityEvent/observer routing up `ChildOf` (GREEN)

Built `routing.rs`: `OnPressMsg<M>` (authoring sugar), a generic propagating
`#[derive(EntityEvent)] #[entity_event(propagate, auto_propagate)] Routed<M>`, a bridge
system (buiy `OnPress` message → trigger `Routed<M>`), and ONE global observer per `M`
that enqueues at the nearest model ancestor and stops.

- **Ran it (headless):** **7/7 green** (added 2 routing tests). A press on a 3-level-deep
  button bubbles up `ChildOf` to the root model and enqueues there; with nested models,
  the INNER one wins and the outer is untouched (propagation halts at the first owner).
- **Generic `EntityEvent` derive Just Works** — the proto-1 retrospective worried it might
  fight generics; it didn't. `Routed<M: Model>` derived cleanly.
- **The bubbled current-target is readable.** Source confirmed + test-verified: the
  propagation engine calls `event.set_event_target(current)` at each step, so the
  observer reads `on.event().entity` to know where it currently is (NOT
  `original_event_target`, which stays at the source button). `on.propagate(false)` halts.
- **Enqueue-only rule held cleanly.** The bridge only *triggers*; the observer only
  *enqueues*. No reducer is reachable from either — the determinism stays at the drain.
- This is strictly more Bevy-native than proto-1's manual `child_of` walk-in-a-system, and
  matches its routing semantics exactly. **KEEP** (replaces proto-1's hand-walk).
- Latency note: bridge (Enqueue set) → `commands.trigger` flush → observer enqueue flush →
  Drain can span a couple frames depending on where Bevy inserts command flushes; the
  tests pump 3 frames. Pin the flush points explicitly in the final (a designed concern,
  per retrospective REDESIGN #2). **REFINE.**

### 2026-06-26 — Wave 3: Yew-style typed `Callback<T>` (GREEN)

Built `callback.rs`: `Callback<T>` (a cloneable `Arc<dyn Fn(T, &mut Commands)>` sink) +
`callback::<T, M>(parent, map)` that builds the Yew adoption wiring (map child payload →
parent `Msg`, enqueue into the parent funnel). Demo: a `Form` parent adopts an
`EditorWiring` child.

- **Ran it (headless):** **9/9 green.** Firing a child's `Callback<String>` enqueues
  `FormMsg::Submit("buy milk")` into the parent's funnel; the drain folds it into the
  parent and the log records the PARENT message (`Submit`, `buy milk`) — never a direct
  mutation, never the closure.
- **Replay-safety proven.** The `#[reflect(ignore)]` `on_commit` field is skipped by
  `TypedReflectSerializer`: the widget serializes to just its `label` (`"todo"`); the RON
  contains neither `Submit` nor `on_commit`. So a state/log snapshot never tries to
  serialize a function. Gave `Callback` a no-op `Default` so the ignored field reconstructs
  cleanly at spawn. **KEEP.**
- `RunSystemOnce` (`bevy::ecs::system::RunSystemOnce`) drives the one-shot fire in the test.

### 2026-06-26 — Wave 4: cross-process Reflect log + LogicalId (GREEN — the capstone)

Built `bin/replay_harness.rs`: `record` runs a 2-actor interleaved script (+ a `TickTo`
Emit chain), persists the `Reflect`-serialized log (JSON of `{lid, seq, ron}`) and the
final state; `replay` is a **separate process** that reloads, re-folds in seq order
dropping every `Cmd`, and compares.

- **Ran it as TWO real processes:** record (entities `9v0`/`10v0`) → `{1:(value:13),
  2:(value:99)}`; replay (a fresh process with **no ECS and no `Entity` ids at all** —
  actors are a `BTreeMap<LogicalId, Counter>`) → **byte-identical**, exit 0.
- **Validates in one shot:** (a) `Reflect` log round-trips cross-process via RON
  (`TypedReflectSerializer`/`Deserializer` + `FromReflect`); (b) **`LogicalId` addressing**
  — replay never touches `Entity`, only the stable logical ids, yet lands every fold on
  the right actor; (c) **ReplayMode** (drop `Cmd`s) reproduces the Emit-flattened `TickTo`
  sequence exactly, because the drain recorded every fold including the Emits.
- **CI-faithful:** the harness built under `--locked` with no lockfile change (ron 0.12 /
  serde / serde_json were already resolved transitively). **KEEP.**
- Note: the harness folds `counter_update` directly (not through the ECS drain) on replay
  — fine for the pure-state proof, but the final's `ReplayMode` should re-fold *through the
  real drain* (so binds/views also reproduce), gated by a `bevy_state` Replay state.
  **REFINE.**

### 2026-06-26 — Wave 5: the REAL GUI smoke (PASS — ran it, didn't trust green)

Built `bin/mvu_native_gui.rs`: `DefaultPlugins` + `BuiyPlugin` + `MvuPlugin`, a `Counter`
container with a real `Button` widget child (`OnPressMsg<Counter>(Increment)`) + a `Text`
label bound on `Changed<Counter>`. Self-drives by emitting the `OnPress` activations buiy's
pointer layer emits on a click, logs the model each change, exits at frame 220.

- **RAN IT on `:0` + RX 6700 XT:** counter incremented `0 → 1 → 2 → 3 → 4 → 5`, exactly once
  per emitted `OnPress`, **clean exit 0, no panic, no render-world crash.** The full
  Bevy-native chain fired in the real loop: `OnPress` → `bridge_press` → `commands.trigger`
  → bubbling `Routed` observer up `ChildOf` → `enqueue` → drain fold → `bind` → bound label.
- **The proto-1 failure mode did NOT recur.** A real `Button` + `Text` rendered every frame
  with no unshaped-text extract crash — the widget-catalog `#[require(Node)]` fix held.
- Boundary (honest): the smoke synthesizes `OnPress` (the canonical activation sink) rather
  than a literal pointer hit-test — that handoff is buiy's pipeline, not the MVU runtime's.
  Noted for the final's live-interaction tier. **REFINE.**

## Final verdict

**Prototype-2 COMPLETE + verified.** All five § 13 charter items built & run; **10/10 tests
green** (9 unit + 1 compile_fail) + **byte-identical cross-process replay** + a **clean real
GUI** on a real adapter. One locked spec premise was invalidated and corrected by building:
**purity must be a sealed `PureEnv` allowlist, not `ReadOnlySystemParam`** (Commands is
read-only in 0.19). See the retrospective for the full keep/refine/redesign + the spec edits
the final must make. Code is DO-NOT-MERGE reference; this journal + the retrospective are the
deliverable.

### 2026-06-26 — Follow-on: app-wiring ergonomics (one call, model inferred) (GREEN)

The 4-calls-per-model wiring (`register_type` + `add_model` + `add_reducer::<M,_>` +
`add_routing::<M>`) is a footgun: forget `register_type` → the log serializer panics at
runtime; forget `add_routing` → presses are silently dead (the "looks wired, doesn't
respond" trap the widget-catalog/parity campaigns kept hitting). Collapsed it to one call,
with the model type **inferred from the reducer**:

```rust
app.mvu_model(counter_update).with_routing();   // was 4 calls + a turbofish
```

- **How:** `IntoModelReducer<fn(&mut M, M::Msg)>` — the marker fn-type carries `M` into the
  trait reference (Bevy's `IntoSystem` trick), so `M` is inferable (also kills the no-env
  `add_reducer::<Counter,_>` turbofish). `mvu_model` returns a `ModelWiring<'_, M>` handle so
  `.with_routing()` needs no turbofish either. Tightened `Model: Reflect + GetTypeRegistration`
  (models need Reflect for state snapshots anyway) to auto-register.
- **Ran it:** **10/10 green** incl. a new `one_call_wiring_infers_model` (press→fold
  end-to-end with zero `::<Counter>` annotations); all bins still build under the tightened
  bound.
- **Residual:** env reducers still name `E` (`add_reducer_env::<_, Res<Step>, _>`) — the
  projection-inference wall (REFINE #1); `with_routing` assumes `BuiyPlugin` (pure-ECS tests
  use the low-level pieces). **KEEP this ergonomic shape for the final.**
