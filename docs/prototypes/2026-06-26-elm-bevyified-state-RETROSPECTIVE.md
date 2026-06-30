# Prototype Retrospective — Elm-bevyified State Management for Buiy

> **Prototype-first-development gate deliverable.** Seeds the human-gated final
> brainstorm. The code (`examples/mvu_spike/`, throwaway, DO-NOT-MERGE) is an
> unmerged reference; this retrospective + the journal are the product.
> Base: `origin/main@59cd50e`. Validation: **33/33 headless tests green** + clean
> GUI boots on a real RX 6700 XT + **byte-identical record→replay**.

## Verdict

**The Elm-bevyified MVU surface is SOUND on Bevy 0.19.** All seven open questions
(Q1 routing, Q2 composition/derived, Q3 parent↔child, Q4 effects, Q5 view, Q6
collections, Q7 record/replay) were resolved by **building and running** — across
nested counters, a real-widget TodoMVC, an async search, an edit-in-place
composite, and a record/replay harness. The most novel/risky territory all held.
The design's headline thesis — *each widget is an actor, Bevy is the actor
runtime, and a recordable Msg stream gives deterministic tests + time-travel +
agent-driving for free* — is validated.

## Validated — KEEP (port with re-derived rationale)

1. **`update(&mut self, Msg) -> Cmd` purity boundary — the load-bearing invariant.**
   No `World` in scope ⇒ Rust's type system *structurally forbids* spawn / sibling
   write / `Commands` inside `update`. W4 proved replay needs ONLY this purity
   (zero effect re-execution) to reproduce byte-identical state. This one property
   is what unlocks deterministic tests, record/replay, and agent-driving. Keep exactly.
2. **Model = the entity's components; many small per-entity MVU loops composed by
   the ECS tree** — no god-Model, no `Msg.map`. Composition via the hierarchy works;
   it deletes Iced's central-enum ceiling.
3. **Routing = ancestor-walk over `ChildOf` (authoring sugar) resolving to a concrete
   entity, gated by the one-Msg-type ↔ one-Model-type invariant.** Unambiguous across
   3 levels; skip-immediate-parent works; reparent re-resolves tick-exact (walks
   `ChildOf` upward, not `Children`). `route_to` (explicit address) is the cross-tree
   escape hatch. Keep both tiers.
4. **Effects = descriptor values; `Task` stored as `InFlight<M>` on the originating
   entity (entity = fold-back address); result folds back THROUGH `update`** (so
   `Changed` trips binds). Validated live in the GUI (async round-trip). drop = cancel
   is free (despawn / supersede / takeLatest).
5. **Dynamic collections = keyed reconcile by DOMAIN id (`TodoId`); Entity = identity.**
   add = spawn / remove = despawn; the entity IS the transient-state container — a
   half-typed edit survives insert/delete. Subsumes Relm4 DynamicIndex / Iced keyed.
6. **Record/replay = a replayable Msg log + `ReplayMode` (drop Cmds) + re-fold from
   init.** Byte-identical replay holds; self-contained because effect results fold
   back AS Msgs (strictly beats Elm-drops-Cmds + Redux-re-runs-effects). This is the
   headline justification for the whole discipline; it is the write-side dual of the
   agent-interface "one tree, N consumers" → **"one Msg log, N consumers."**
7. **Two-tier parent↔child**: translator-by-default (`OnPressMsg` up the walk, zero
   plumbing — ~95% of cases) + OutMsg (`OnOutput<M>` per adoption edge for
   self-contained composites; child Msg stays opaque; **closure count = edges, not
   depth**). Keep both.
8. **`bind` = `Changed<Model>`-gated prop write (`set_if_neq`) for fixed-shape views;
   reconcile for structure; no VDOM/diff.** Q5 answered: no-diff is right — bind for
   props, reconcile for structure.

## REFINE (final does differently — full-picture reason)

1. **Log addressing (W4).** The spike stores raw `Entity` in log entries — works only
   because record+replay spawn in identical order. Final must key by a **stable
   LogicalId aligned with the agent-interface test-id space** (or resolve via `With<M>`
   at replay) so cross-session replay / agent-driving lands on the right entity.
2. **OutMsg delivery latency (W2b).** `OnOutput` delivers next-frame (`MvuSet::Deliver`
   after `Drain`); compounds for deep chains (≈4 ticks / 2 hops). Final: an
   `add_chained_output::<Child,Parent>` ordering pinning child-deliver before
   parent-drain, halving per-hop latency.
3. **`bind` resolution (W1).** `bind_text` ancestor-walks / builds a `HashSet` per
   changed model per frame → O(n²) at scale. Final: store the model `Entity` on the
   bind component (direct lookup).
4. **Derived-state cost (Q2).** Each derived view field = 1 `Changed`-gated system +
   1 marker component; fine at 1–5, a scaffolding tax at 20+. Final: consider a
   `derive!`/`bind_query!` helper that generates the system+marker. Make the rule
   explicit: descendant derivations may use `bind` (ancestor-walk); sibling/cross-
   subtree need a dedicated system.
5. **Controlled vs self-updating widgets (W2; user chose per-widget).** The real
   `Checkbox` self-advances `A11yToggled` before MVU routing (double-write; one-frame
   flicker if the model rejects the toggle). Final: **suppress `advance_toggle_on_press`
   when a controlled marker (`OnPressMsg<M>`) is present** — clean controlled path.
6. **OutputModel as a subtrait, not an associated-type default** (defaults aren't
   stable in Rust 1.95). Cleaner anyway ("this model emits output" is explicit in the
   type system). Translator must be `Arc`-wrapped (borrow-checker); document the
   Bevy-0.19 pattern.

## REDESIGN (the biggest — full-picture)

1. **Agent-interface / Action lowering must route THROUGH `update`/the Msg path.**
   Today the in-process inspection driver's `dispatch_action_request` pokes
   Focus/OnPress/EditCommand sinks DIRECTLY, and `set_focus`/`set_value` write
   Focus/AT state OUTSIDE the Msg path. For fully-reproducible record/replay +
   agent-driving (the whole debuggability thesis), those writes must become
   Msg-addressed (or seeded as initial conditions). This is the clearest actionable
   redesign the prototype surfaced, and it couples this design to the agent-interface
   campaign. (Will require an update to `docs/specs/2026-06-18-buiy-agent-interface-design/`.)
2. **Run-to-completion drain ordering is a DESIGNED concern, not emergent.** Bevy
   observers run depth-first/immediately while Messages drain at sync points; the
   single ordered drain + next-frame fold must be an explicit `MvuSet` ordered against
   `BuiySet` (the campaign already drew blood here: "reshape ordering vs
   focus_lifecycle"). The final must pin the `MvuSet` ordering precisely.

## Framework / Bevy BUGS surfaced by running

- **None in buiy/bevy.** The Wave-1 `B0004` was OUR tree-authoring (model entities
  weren't `Node`s) — caught only by running the GUI ("always RUN the GUI", re-confirmed).
- Bevy-0.19 friction (not bugs, but document): `Component<Mutability = Mutable>` bound
  for generic `Query<&mut M>`; `Children::iter()` yields `Entity` directly (drop
  `.copied()`); associated-type defaults unstable on Rust 1.95; `AsyncComputeTaskPool`
  needs `TaskPoolPlugin` (in `MinimalPlugins`); `block_on(poll_once)` + remove-on-Some
  (re-poll panics); cancellation is cooperative (no-yield CPU tasks don't cancel
  mid-compute); `FocusPlugin::handle_tab` needs `ButtonInput<KeyCode>` initialized.

## Residual gaps (final to decide/close)

- **Visual reorder**: reconcile appends by spawn order, not `items` order; reorder
  needs `reorder_children` (preserves entity identity). Keyed-identity property already
  holds; only the visual ordering is unwired.
- **Concurrent effects per model**: single-`InFlight`-per-model (takeLatest) is the v1
  default; concurrency needs child effect-entities or a multi-slot.
- **Designed-but-not-built**: subscriptions, `Cmd::stream` (timers/progress),
  `Cmd::batch/sequence` atomicity, supervision (panicking-reducer blast radius),
  `bsn!` authoring ergonomics for the surface, MCP/Msg serialization for cross-process
  replay.

## Build strategy (for the human-gated final — NOT started)

- Validated decisions are cherry-pickable from the shared base (`origin/main@59cd50e`).
  Final = **hybrid port**: port the KEEP shapes with re-derived rationale; implement
  the REFINE/REDESIGN deliberately.
- **Open placement decision**: where does the runtime land — `buiy_core`, or an opt-in
  `buiy_state`/`buiy_mvu` crate? The "tools over ECS, Buiy does not own state" framing
  argues for an **opt-in crate** layered on the existing change-detection substrate.
- Prior-art folder actions queued (post-gate): NEW `docs/prior-art/relm4/` (highest
  value), capture Elm/Redux time-travel, refresh `iced/` (0.14 devtools + Component
  deprecation), optional `gpui/actor-model.md`.

## The decisions for the human gate (brainstorm these)

1. Runtime placement: opt-in `buiy_mvu` crate vs `buiy_core`.
2. Log/replay addressing: LogicalId vs `With<M>` resolution; how tightly to couple to
   the agent-interface test-id space.
3. The REDESIGN: route agent-interface Action lowering through `update` — in scope for
   the first state spec, or a follow-up that the spec just makes room for?
4. Controlled-vs-self-updating default per widget + the `advance_toggle_on_press`
   suppression mechanism.
5. Derived-state ergonomics: hand-written systems vs a `derive!`/`bind_query!` helper —
   how far to push sugar in v1.
6. How much of the effect algebra (`stream`/`batch`/`sequence`/structural) + subscriptions
   to commit to in v1 vs defer.
7. Scope of the first spec: widget-internal state + the binding seam only, or the full
   MVU runtime as a shipped tool.
