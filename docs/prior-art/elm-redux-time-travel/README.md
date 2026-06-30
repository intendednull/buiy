**Date:** 2026-06-26
**Status:** active
**Subject:** The Elm + Redux time-travel/replay lineage — the canonical prior art for Buiy's "one Msg log, N consumers → deterministic tests + time-travel + replay" thesis

# Elm / Redux / Redux-DevTools time-travel

This is the direct ancestor of Buiy's prototype-3 replay thesis. The Elm Architecture (TEA) and Redux are the two systems that proved, in production and at scale, that **a pure state-transition function + a recorded log of the events that drive it = deterministic replay, time-travel debugging, and import/export of whole sessions.** Buiy's "one Msg log, N consumers" is a near-exact restatement of Redux's "single source of truth + actions are the only way to change state, and they can be logged, serialized, stored, and later replayed" ([Redux Three Principles](https://redux.js.org/understanding/thinking-in-redux/three-principles)).

This folder is a **scoped capture** (two files), not the full 7-stage prior-art treatment — it exists to feed the MVU-as-core spike with the hard-won lessons of the systems that did this first, including the ones they failed at.

## Why this matters for Buiy proto-3

The proto-3 charter bets that making the Msg substrate **core** (not an opt-in `buiy_mvu` crate) makes the recorded log *complete* — capturing widget-internal state (focus, edit buffer, IME, scroll) that an app-boundary log cannot see. Elm and Redux are the empirical record of what that bet buys and what it costs:

- **What it buys** is real and proven: Elm's debugger replays "the entire history of your application by replaying its messages" ([Elm guide — effects](https://guide.elm-lang.org/effects/)); Redux DevTools' time-travel, jump, skip, and import/export are a daily-driver feature for a generation of web developers.
- **What it costs** is also proven, and the charter must respect it: the *single source of truth* is load-bearing. The instant any state lives **outside** the recorded log — React component-local state, a non-serializable value, an effect that mutates the outside world — time travel silently degrades or breaks. This is precisely the `TextEditState` crux the charter names, and Redux's 10-year history is the cautionary evidence.

## Honest assessment

- **The thesis is sound and battle-tested — but it is a property of the *whole system*, not of the log alone.** Replay is byte-identical only if (1) the transition function is pure, (2) every input that drives it is in the log, and (3) the initial environment is reconstructed identically. Miss any one and replay diverges *silently*. Elm enforces (1) at the language level (no side effects in `update`, effects are `Cmd` values run by the runtime). Redux enforces nothing — it *recommends* purity and serializability via lint/style rules, and the ecosystem is littered with bugs from violating them. Buiy sits between: Rust's type system can enforce purity (the proto-2 sealed `PureEnv`), but cannot enforce "every input is in the log" — that is an architectural obligation the core-substrate bet takes on.

- **Time-travel restores the *model*, never the *world*.** Both systems replay state, not side effects. Elm's runtime does **not** re-run `Cmd`s on rewind; Redux replays only reducers, never the thunk/saga middleware that performed the I/O ([Redux FAQ — async](https://redux.js.org/faq/actions)). "The state time-travels; the universe does not." For Buiy this is the permanent ceiling: replaying the Msg log reconstructs the UI's *model*, but cannot un-send a network request or un-write a file. The design must make this boundary explicit (effects-as-data, applied only in record mode), not pretend it away.

- **The hardest, most-cited failure is non-serializable state.** Promises, functions, class instances, `Symbol`s, `Map`/`Set` in the store or in an action turn DevTools snapshots into `{}` and make time-travel "useless" ([tejastn10](https://www.tejastn10.com/blog/21)). Redux Toolkit ships a `serializableCheck` middleware specifically to catch this at dev time. Buiy's `Reflect` log is the structural analogue — and the proto-2 callback solution (`#[reflect(ignore)]` on the `Arc<dyn Fn>` wiring, reconstructed at spawn) is exactly the right move, because it keeps the *non-serializable closure out of the log* while keeping the *serializable resulting Msg in it*.

- **History is not free.** Storing every action + every resulting state is O(n) memory and O(n) recompute-on-invalidation. Redux DevTools defaults to `maxAge: 50` and ships `actionSanitizer`/`stateSanitizer` because unbounded history leaks past 1 GB and OOM-crashes Android ([redux-devtools-extension Arguments](https://github.com/zalmoxisus/redux-devtools-extension/blob/master/docs/API/Arguments.md)). This is a direct hit on the charter's **60 Hz hard-floor / weak-machine / thousands-of-widgets** risk: an always-on, always-complete core log is a *memory and CPU* liability unless it is bounded, sampled, or opt-out-able on hot paths.

- **Single global Model vs. Buiy's per-entity decomposition is the deepest structural divergence.** Redux and Elm have **one** state tree; "jump to state N" is therefore either O(1) (DevTools precomputes and caches every intermediate state in `computedStates`) or a single re-fold from `init`. Buiy has **N** per-entity Models and N Msg types (one Msg ↔ one Model, hard invariant). There is no single state object to snapshot — so "jump to frame N" requires either (a) re-folding *all* actors from init in seq order, or (b) snapshotting *all* Models. The single-source-of-truth property that makes Redux's undo/redo "trivial" does **not** come for free in a decomposed model; Buiy must *manufacture* it from the ordered cross-actor log. This is the single most important thing this prior art teaches the proto-3 spec.

## Key facts (verified 2026-06-26)

| Fact | Value |
|---|---|
| Elm time-travel debugger origin | Laszlo Pandy, ~2013–2014; inspired by Bret Victor's *Inventing on Principle* ([Wadler's blog](https://wadler.blogspot.com/2014/05/elms-time-travelling-debugger.html)) |
| Elm debugger delivery (0.15–0.17) | `elm-reactor`, opened via the wrench icon or `?debug` — hot-swapped code while preserving recorded events ([hackage elm-reactor](https://hackage.haskell.org/package/elm-reactor)) |
| Elm debugger today (0.19) | Built-in `--debug` (via `elm make --debug` / `elm reactor`): message log + click-to-time-travel + JSON history import/export. The *standalone full time-travel-in-reactor* of earlier versions was reduced ([Elm discourse](https://discourse.elm-lang.org/t/time-travelling-debugger-with-elm-reactor-0-19/2036)) |
| Elm replay mechanism | `update` is pure; the runtime re-runs `update` for each recorded `Msg` in sequence from `init` ([Elm guide effects](https://guide.elm-lang.org/effects/)) |
| Elm history export format | JSON: a `metadata` section (type defs, for version-mismatch warnings) + a `history` array of messages; constructors encoded with `"$"` tag key, args in `"a"`,`"b"`,`"c"`… ([eSpark blog](https://medium.com/espark-engineering-blog/understanding-an-elm-0-19-history-export-1bca38613840)) |
| Elm export gap (critical) | **Flags used to compute the initial model are NOT included in the export** — replay across sessions/environments can diverge ([eSpark blog](https://medium.com/espark-engineering-blog/understanding-an-elm-0-19-history-export-1bca38613840)) |
| Elm serializability constraint | Functions cannot be serialized; `Msg` must contain only serializable data, or import/export fails ([elm-mdl #318](https://github.com/debois/elm-mdl/issues/318)) |
| Redux three principles | (1) single source of truth; (2) state is read-only — only an action changes it; (3) changes via pure reducers ([Redux](https://redux.js.org/understanding/thinking-in-redux/three-principles)) |
| Redux replay enabler | "As actions are just plain objects, they can be logged, serialized, stored, and later replayed" — quoted directly from principle (2) |
| Redux core history | Redux core does **not** store action history; the **DevTools store enhancer (`instrument()`)** does |
| DevTools lifted state | `actionsById`, `stagedActionIds`, `computedStates[]`, `currentStateIndex`, `skippedActionIds` ([hmos.dev](https://hmos.dev/en/how-to-time-travel-debugging-at-redux-devtools)) |
| DevTools recompute | `recomputeStates` replays `reducer(state, action)` from `minInvalidatedStateIndex` forward, caching each result in `computedStates` |
| DevTools JUMP | `JUMP_TO_STATE` reassigns `currentStateIndex` — O(1), reads the *precomputed* state, no re-execution |
| DevTools SKIP | `TOGGLE_ACTION` adds an id to `skippedActionIds`, then recomputes from that point as if the action never fired |
| DevTools UX verbs | jump, skip, pause, lock, persist, export, import, reorder, dispatch, commit (squash history → new base), sweep (drop disabled), reset, revert ([redux-devtools-extension](https://github.com/zalmoxisus/redux-devtools-extension/blob/master/docs/API/Arguments.md)) |
| DevTools history cap | `maxAge` default **50**; `actionSanitizer`/`stateSanitizer` strip bulky payloads; unbounded → >1 GB memory, Android OOM |
| What breaks time travel | non-serializable state/actions; mutated (shared-ref) state; side effects in reducers; state living outside the store ([tejastn10](https://www.tejastn10.com/blog/21), [Redux style guide](https://redux.js.org/style-guide/)) |

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, honest assessment, the four task questions (a–d) answered below, key facts, framing disclosure, sources. |
| [`lessons-for-buiy-mvu.md`](lessons-for-buiy-mvu.md) | **The consult-this-when-designing decision file.** KEEP / AVOID for Buiy's core `Reflect`-log + `LogicalId` replay — what makes replay byte-identical, where side effects leak, what the DevTools UX teaches whole-UI time-travel. Grounded in the proto-2 code (`examples/mvu_native/src/runtime.rs`). |

---

## (a) What time-travel debugging actually REQUIRES

Time-travel/replay is not a feature you bolt on; it is an emergent property of four invariants holding simultaneously. Both Elm and Redux converge on the same four:

1. **A pure transition function.** `update : Msg -> Model -> Model` (Elm) / `reducer(state, action) -> state` (Redux) must be a deterministic function of its inputs with no side effects. Replay = re-running this function over the recorded inputs. Elm enforces this at the *language* level — `update` literally cannot perform I/O; it returns `(Model, Cmd Msg)` and the runtime performs the `Cmd` ([Elm guide effects](https://guide.elm-lang.org/effects/)). Redux enforces it only by *convention* (principle 3 + the style guide), and violations are a top source of "time travel is broken" bug reports.

2. **Serializable, replayable events.** The thing you record must be plain data. Redux: "actions are just plain objects … they can be logged, serialized, stored, and later replayed" ([three principles](https://redux.js.org/understanding/thinking-in-redux/three-principles)). Elm: `Msg` must contain no functions or it cannot be exported/imported ([elm-mdl #318](https://github.com/debois/elm-mdl/issues/318)). The corollary that bites everyone: **the moment your event carries a closure, a `Promise`, or a live handle, replay breaks.**

3. **Side effects isolated as *data*, executed only at the boundary.** Elm's `Cmd`/`Sub` are the canonical model: `update` *describes* effects as values; the runtime *executes* them and feeds results back in as new `Msg`s. The "Safe Area" (pure) is cleanly separated from the "Unsafe Area" (effects). Critically, the *result* of an effect re-enters as a recorded `Msg`, so the log stays self-contained — you replay the *result*, you do not re-run the *effect*. Redux pushes effects into middleware (thunk/saga); reducers stay pure, and replay re-runs only reducers. (See the well-known **Effect pattern** ([reasonableapproximation](https://reasonableapproximation.net/2019/10/20/the-effect-pattern.html)): even Elm's `Cmd` is *too opaque* — you cannot inspect or unit-test it — so disciplined teams return a custom inspectable `Effect` enum from `update` and interpret it into `Cmd` at the boundary. This is exactly Buiy's `Cmd::{none, emit, batch, task}` enum.)

4. **A reconstructible initial environment.** Replay starts from `init`. If `init` depends on inputs that are *not* in the log, a fresh-session replay diverges. Elm's history export famously **omits flags** ([eSpark blog](https://medium.com/espark-engineering-blog/understanding-an-elm-0-19-history-export-1bca38613840)) — so importing a history into a differently-flagged session can produce a different model. Buiy's design names this directly: the replay contract is "deterministic given the *seeded* environment," env reads reconstructed at replay-start "like Elm's init flags" (draft spec § 3).

## (b) How Redux DevTools implements record/replay/jump/import-export — and its LIMITS

**Implementation.** DevTools is a Redux **store enhancer** (`instrument()`) that wraps the real store in a *lifted* store. The lifted state holds ([hmos.dev](https://hmos.dev/en/how-to-time-travel-debugging-at-redux-devtools)):

- `actionsById` — every dispatched action, wrapped as `PERFORM_ACTION` with metadata (timestamp).
- `stagedActionIds` — the ordered list of action ids.
- `computedStates[]` — the cached state *after each action* (this is the key to O(1) jump).
- `currentStateIndex` — which `computedStates` entry the app currently shows.
- `skippedActionIds` — actions toggled off.

The engine `recomputeStates` replays `reducer(state, action)` starting from `minInvalidatedStateIndex` and caches each result. **Record** = append to `stagedActionIds`/`computedStates`. **Replay/jump** (`JUMP_TO_STATE`) = reassign `currentStateIndex` to a cached state — no re-execution. **Skip** (`TOGGLE_ACTION`) = add to `skippedActionIds`, recompute from there as if the action never happened. **Import/export** = serialize the whole lifted state to JSON and rehydrate it. Plus the workflow verbs: pause (stop recording), lock (freeze app but keep time-travel), persist (survive reload), commit (squash history, current state becomes the new base), sweep (drop disabled actions), reset/revert.

**Limits (all directly relevant to Buiy):**

- **Non-serializable state/actions.** Promises, functions, class instances, `Symbol`s, `Map`/`Set` → snapshots become `{}`, history can't rehydrate, time-travel is "useless" ([tejastn10](https://www.tejastn10.com/blog/21)). RTK ships `serializableCheck` to catch it.
- **Side effects are not reproduced.** Replay re-runs reducers only; the thunk/saga that did the I/O does not re-fire. State is restored; the world is not.
- **State outside the store is invisible.** React component-local state, refs, anything not dispatched-through-an-action does not time-travel. This is the structural one (see (c)).
- **Mutated state breaks it.** If a reducer mutates instead of returning a new object, `computedStates` entries share references and history corrupts — "the most common cause of bugs in Redux … will also break time-travel."
- **Memory/CPU.** Full history is O(n) memory + O(n) recompute on invalidation; hence `maxAge: 50` + sanitizers + the >1 GB / Android-OOM warnings.
- **Async ordering.** Because the store "knows nothing about async," effect results arrive as later actions; replay reproduces them *as recorded*, but live re-dispatch during a jump can interleave unexpectedly (the source of many "time travel doesn't work with my effects" issues, e.g. [ngrx/store-devtools #33](https://github.com/ngrx/store-devtools/issues/33)).

## (c) "Single source of truth" + action-log ↔ Buiy's "one Msg log, N consumers"

Redux principle (1) — *single source of truth* — is the foundation that makes undo/redo and time-travel "trivial," because **all** state is in one tree and **every** change is an action in one ordered log. Buiy's "one Msg log, N consumers" maps the *log* faithfully (an ordered, recorded stream of `(LogicalId, Msg, seq)`) but **deliberately rejects the single tree**: state is decomposed per-entity (one Msg ↔ one Model). The consequences:

- **What maps cleanly:** the ordered, append-only, serializable event log; "actions/Msgs are the only way to change state"; consumers (tests, time-travel, agent-driver, hot-reload) reading the *same* log. This is a strong, validated design.
- **What does NOT come for free:** Redux gets "jump to any past state in O(1)" because it caches the *single* whole-state object after each action. Buiy has no single object — so the *core substrate bet is what makes the log complete enough to even attempt this*. The charter's core insight is correct: an **opt-in app-boundary** log cannot see widget-internal state, so whole-UI time-travel is structurally impossible while the substrate is optional. Making it core is the Redux "single source of truth" property *reconstructed* over a decomposed model: the log is the source of truth, and the N Models are a *derived cache* you rebuild by folding.
- **The cost of decomposition:** "jump to frame N" is O(total folds) to re-derive, or O(total live Models) to snapshot. Redux's `computedStates` cache is not directly portable because there is no single state to cache cheaply. This is the central performance/feasibility question the spec must answer (see lessons § AVOID-2, § AVOID-5).

## (d) What breaks time-travel in practice, and how each system mitigates

| Breakage | Elm mitigation | Redux/DevTools mitigation | Buiy obligation |
|---|---|---|---|
| **Impurity in the transition fn** | Language-enforced: `update` cannot do I/O | Convention + style-guide lint; violations ship bugs | Type-enforce via sealed `PureEnv` allowlist (proto-2) — stronger than Redux, no `World`/`Commands` in reducer |
| **Non-serializable events** | `Msg` must be function-free or export fails | `serializableCheck` middleware; "keep only plain data" | `Reflect`-derive on every `Msg`; closures kept out of the log via `#[reflect(ignore)]` callbacks |
| **Side effects not reproduced** | Runtime does not re-run `Cmd` on rewind; effect *results* re-enter as `Msg` | Replay runs reducers only, not middleware | Effects-as-data `Cmd`; result folds back *as a recorded Msg*; gate effect application off in replay mode |
| **State outside the log** | Everything is in the Model (no escape hatch in pure Elm) | Component-local state is the #1 invisible-state trap | **Core substrate** so widget-internal state (focus/edit/IME/scroll) flows through the funnel — the charter's whole thesis |
| **Mutated/shared-ref state** | Immutable by default | Immer / "return new state" rule | Rust ownership + `&mut Model`-only mutation in the drain; no aliasing |
| **Unbounded history cost** | (debugger is dev-only; sessions short) | `maxAge: 50`, `actionSanitizer`/`stateSanitizer`, commit/sweep | Bound/sample/opt-out the core log on hot paths — the 60 Hz floor demands it |
| **Init env not captured** | Flags omitted from export → divergence | Import replaces whole state, sidesteps init | Seed/reconstruct env at replay-start (draft spec § 3) |

## Framing disclosure

This is a deliberately **narrow, two-file** capture commissioned for the proto-3 MVU-as-core research spike, not the full `researching-prior-art` 7-stage folder. It privileges the *replay/time-travel* axis over Elm/Redux's broader stories (their view layers, ecosystem, governance) because that axis is what proto-3 must learn from. Two consequences for the reader: (1) the "honest assessment" leans critical *by design* — the charter rule is "do not rubber-stamp," so failure modes are foregrounded; (2) all claims are sourced to public docs/issues/blog posts and the proto-2 code in the sibling `state-mgmt-elm-prototype` worktree, not to first-hand experiments by this author. Where Elm/Redux behavior is version-specific (Elm 0.18 vs 0.19 debugger; Redux-core vs DevTools-enhancer) the version is named. If proto-3 advances to a real spec, this folder is a candidate to promote to the full treatment alongside `relm4/`, refreshed `iced/`, and `gpui/actor-model.md`.

## Sources

- Redux — Three Principles — https://redux.js.org/understanding/thinking-in-redux/three-principles
- Redux — Style Guide (serializable state, no side effects in reducers) — https://redux.js.org/style-guide/
- Redux — FAQ: Actions (async, non-serializable in middleware-stopped actions) — https://redux.js.org/faq/actions
- How to time-travel debug at redux-devtools (lifted store internals) — https://hmos.dev/en/how-to-time-travel-debugging-at-redux-devtools
- redux-devtools-extension — Arguments (maxAge, sanitizers) — https://github.com/zalmoxisus/redux-devtools-extension/blob/master/docs/API/Arguments.md
- Redux DevTools — Reset/Revert/Sweep/Commit docs PR — https://github.com/reduxjs/redux-devtools/pull/231/files
- "Why Redux Doesn't Allow Non-Serializable Data" — https://www.tejastn10.com/blog/21
- ngrx/store-devtools #33 — time travel + effects — https://github.com/ngrx/store-devtools/issues/33
- Elm guide — The Elm Architecture: effects (`Cmd`/`Sub`, purity, replay) — https://guide.elm-lang.org/effects/
- Elm discourse — time-travelling debugger with elm reactor 0.19 — https://discourse.elm-lang.org/t/time-travelling-debugger-with-elm-reactor-0-19/2036
- Wadler's blog — Elm's time-travelling debugger (Laszlo Pandy origin) — https://wadler.blogspot.com/2014/05/elms-time-travelling-debugger.html
- hackage — elm-reactor — https://hackage.haskell.org/package/elm-reactor
- "Understanding an Elm 0.19 History Export" (export format, flags omitted) — https://medium.com/espark-engineering-blog/understanding-an-elm-0-19-history-export-1bca38613840
- elm/compiler #1828 — 0.19 debug history export/import broken — https://github.com/elm/compiler/issues/1828
- "The Effect pattern: Transparent updates in Elm" — https://reasonableapproximation.net/2019/10/20/the-effect-pattern.html
- Proto-2 runtime (sibling worktree) — `examples/mvu_native/src/runtime.rs`
- Buiy draft state-mgmt design — `docs/specs/2026-06-26-buiy-state-management-design.md`
</content>
