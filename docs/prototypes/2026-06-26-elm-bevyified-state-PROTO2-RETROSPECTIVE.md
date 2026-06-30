# Prototype-2 Retrospective — the Bevy-NATIVE Elm-bevyified MVU runtime

> **Prototype-first-development gate deliverable (pass 2).** Prototype-1 validated the
> *bespoke* runtime; prototype-2 (`examples/mvu_native/`, throwaway, DO-NOT-MERGE) built &
> ran the **Bevy-native/leveraged** half the design locked but had never compiled. This
> retrospective + the journal are the product; the code is an unmerged reference.
> Base: `origin/main@59cd50e` (shared with proto-1 → validated shapes stay cherry-pickable).
> Validation: **10/10 tests green** (9 headless unit + 1 `compile_fail` doctest) + a
> **byte-identical cross-PROCESS** record→replay + a **real GUI** that drives a press
> through the full render loop on an RX 6700 XT with no crash.

## Verdict

**The leveraged design is SOUND — with one locked premise corrected.** Every § 13 charter
item compiled and *ran*:

1. V-B reducer-as-constrained-system (`add_reducer` / `add_reducer_env`) ✓
2. `Messages<M>` inbox with our drain owning the record tap ✓
3. `EntityEvent`/observer routing bubbling up `ChildOf` (enqueue-only) ✓
4. Yew-style `Callback<T>` funnel-dispatch (`#[reflect(ignore)]`, replay-safe) ✓
5. `Reflect` log + `LogicalId` cross-process byte-identical re-fold ✓

The one correction is consequential and is exactly what a prototype is for: **the spec's
purity-enforcement mechanism (`ReadOnlySystemParam`) does not work** — see REDESIGN #1. The
*design* (each widget an actor, Bevy the runtime, a recordable Msg stream → deterministic
tests + time-travel + agent-driving) is re-confirmed on the leveraged substrate; only the
enforcement *mechanism* changes.

## Validated — KEEP (port with re-derived rationale)

1. **`Messages<M>` inbox + our drain as the sole reader/record tap.** Bevy's buffered
   `Messages<Envelope<M>>` is the transport; our `MvuSet::Drain` system is the only reader.
   GC timing confirmed safe: `message_update_system` runs in `First`, so a message written
   in `Update` survives to our same-frame drain — `.after`/set ordering suffices. The
   determinism lives at the drain, not the queue.
2. **The single ordered drain + run-to-completion `Emit` + record-every-fold.** One drain
   folds the inbox; `Cmd::Emit` re-enters the local work queue so a fold-back chain
   completes deterministically in one pass; every fold (incl. emits) is recorded. This
   flattened log is what makes replay self-contained.
3. **Bevy-native routing: a propagating `EntityEvent` + one global observer per `M`.**
   Replaces proto-1's manual `ChildOf` walk-in-a-system. The generic `#[derive(EntityEvent)]
   #[entity_event(propagate, auto_propagate)] Routed<M>` compiled cleanly; the engine
   rewrites the event target at each bubble step (`set_event_target`) so the observer reads
   `on.event().entity` to know where it is, enqueues at the first model owner, and halts
   (`on.propagate(false)`). Nearest-ancestor semantics match proto-1 exactly, more natively.
4. **Yew `Callback<T>` as the child→parent surface.** A cloneable `Arc<dyn Fn(T, &mut
   Commands)>` sink built by `callback::<T, M>(parent, map)`; firing it resolves to a parent
   `Msg` enqueued into the funnel — never a direct mutation. Stored `#[reflect(ignore)]`, so
   a Reflect snapshot skips the closure (proven: the widget serialized to just its data
   field). The logged thing is the resulting parent `Msg`. Replay-safe.
5. **`Reflect` log + stable `LogicalId` → cross-process byte-identical replay.** Two real
   processes: record (Entity ids `9v0`/`10v0`) and replay (a fresh process with **no ECS and
   no Entity ids at all** — actors keyed purely by `LogicalId`) produced identical state.
   RON via `TypedReflectSerializer`/`Deserializer` + `FromReflect`. The headline thesis
   holds on the leveraged substrate.
6. **The V-B reducer-as-constrained-system pattern.** A free fn `fn(&mut M, M::Msg[, &Env])
   -> Cmd` registered via `add_reducer`/`add_reducer_env`; the drain fetches the env once as
   `StaticSystemParam<E>` and reuses `&env` across every fold. No `World` in the reducer ⇒
   purity is structural. Keep the *shape*; change the *enforcement* (below).

## REDESIGN — the spec-invalidating finding

1. **Purity must be enforced by a sealed `PureEnv` allowlist, NOT `ReadOnlySystemParam`.**
   The spec (§ 3, § 12) says reducer purity is "enforced by `ReadOnlySystemParam`." In Bevy
   0.19 **`Commands: ReadOnlySystemParam`** (it only *defers* its spawn/despawn), so that
   bound would let a reducer `Commands::spawn`/`despawn` — unrecorded structural mutation
   outside the funnel → replay diverges, silently. `ReadOnlySystemParam` does correctly
   reject `ResMut`/`Query<&mut>`/`MessageWriter`, but the `Commands` hole is fatal to the
   determinism thesis. **Fix (built & proven):** a sealed `PureEnv` allowlist — `()`, `Res`,
   read-only `Query`, tuples are blessed; `Commands` simply isn't, and the orphan rule stops
   anyone blessing it. A `&Commands` env now fails to compile with the exact error ``the
   trait bound `Commands: PureEnv` is not satisfied``. **The final's spec § 3/§ 12 must be
   amended: "enforced by a sealed `PureEnv` allowlist."**

## REFINE (final does differently — full-picture reason)

1. **Reducer env ergonomics.** The prototype passes the env by shared reference and names
   `E` via turbofish (`add_reducer_env::<Counter, Res<Step>, _>`) because inference cannot
   run backwards through the `SystemParamItem<E>` associated-type projection, and the env
   item isn't `Clone` (so it must be reused by `&` across folds, not moved per-fold). The
   spec's bare-variadic signature (`fn update(&mut M, Msg, Res<Step>)`) is reachable only
   with Bevy's `IntoSystem`-style variadic macro + the `for<'a> &'a mut Func: FnMut(P) +
   FnMut(SystemParamItem<P>)` double-bound. **Final:** decide between (a) shipping that macro
   for bare-param ergonomics, or (b) accepting `&Env` + a `#[derive(PureEnv)]` for user env
   structs (the prototype only blesses primitives + tuples). Lean (b): simpler, and the env
   struct reads well.
2. **Command-flush ordering across the edge.** bridge (`Enqueue`) → `commands.trigger` flush
   → observer enqueue flush → `Drain` can span a couple frames depending on where Bevy
   inserts `apply_deferred`. The GUI tolerated it (a press settled in ≤2 frames), but the
   final must **pin the flush points** — `MvuSet::{Enqueue, Drain, Bind}` ordered against
   `BuiySet`, with explicit syncs — so latency is one designed frame, not emergent. (Matches
   proto-1 retrospective REDESIGN #2.)
3. **`ReplayMode` should re-fold THROUGH the real drain.** The harness folds `counter_update`
   directly — fine for the pure-state proof, but the final's replay must re-emit logged msgs
   into the inbox and run the *real* drain (so `bind`s/derived views also reproduce), gated
   by a `bevy_state` `Replay` state with `run_if(in_state(Replay))` on effect application.
4. **`Cmd::task` (async) re-integration.** Intentionally out of proto-2 scope (validated in
   proto-1). The final must fold the proto-1 `InFlight<M>` + poll + takeLatest back onto this
   drain (poll system enqueues the result as a normal `Envelope`, so it records + replays).
5. **`bind` is a hand-written demo system here** (`Changed<Counter>` → child `Text`). The
   final's bind/derive ergonomics (`bind(|m| …)` + a `derive!` helper) are still open from
   the proto-1 retrospective.

## Framework / Bevy issues surfaced by running

- **No buiy/bevy bugs.** The real render world ran clean: a `Counter` container with a real
  `Button` widget child + a `Text` label rendered every frame with **no extract crash** —
  proto-1's unshaped-text-at-extract crash did **not** recur for plain `Text` labels (the
  widget-catalog `#[require(Node)]` fix on `Text` held).
- The load-bearing *finding* is not a bug but the API fact above (`Commands:
  ReadOnlySystemParam`) — which only a compile-and-run prototype would have caught before it
  shipped as a silent replay-correctness hole.
- 0.19 friction worth documenting (not bugs): generic env param into a closure-system needs
  `StaticSystemParam` (HRTB trap otherwise); `AppTypeRegistry` is in `bevy::prelude`;
  `Children::iter()` yields `Entity`; deserializers return `Box<dyn PartialReflect>` (→
  `FromReflect`); `serde_json`/`ron` already transitive (no lockfile churn under `--locked`).

## Residual gaps for the final to close

- The `#[derive(PureEnv)]` for user env structs (only primitives + tuples blessed now).
- Async effects (`Cmd::task`/`InFlight`/takeLatest) re-integrated onto the new drain.
- `ReplayMode` through the real drain + a `bevy_state` `Replay` gate.
- `LogicalId` assignment strategy aligned to the agent-interface test-id space (proto used
  manual ids).
- The real pointer→`OnPress` hit-test path (the smoke synthesizes `OnPress`, the canonical
  activation sink — buiy's concern, but the final's live-interaction test tier should cover
  the click→fold round-trip end to end).
- `bind`/derived-view ergonomics (carried from proto-1).

## Build strategy (for the human-gated final — NOT started)

- Both prototypes share base `origin/main@59cd50e`; the KEEP shapes are cherry-pickable.
- Final = **hybrid port**: port the KEEP runtime (Messages-inbox + drain + record tap;
  `EntityEvent` routing; `Callback`; `Reflect` log + `LogicalId`; the V-B reducer pattern)
  with re-derived rationale; implement the REDESIGN (sealed `PureEnv` replaces
  `ReadOnlySystemParam`) and the REFINE items deliberately.
- **Placement** stays the locked decision: opt-in `buiy_mvu` crate.

## What changed vs the draft spec (feed into the final's research)

1. **§ 3 / § 12 purity line is wrong** — `ReadOnlySystemParam` → **sealed `PureEnv`
   allowlist**. (REDESIGN #1.) Highest-priority spec edit.
2. **§ 3 reducer signature** — the bare-variadic `fn(&mut M, Msg, Res<Step>)` is not freely
   inferable; document the `&Env`+turbofish (or variadic-macro) reality. (REFINE #1.)
3. **§ 4 / § 7 routing** — proto-1's manual walk is superseded by the `EntityEvent`+observer
   path; both proved equivalent, the native one wins. Confirm in the spec.
4. Everything else in §§ 4–10 (transport, drain, callbacks, Reflect log, LogicalId, modes,
   placement) is **confirmed as written**.
