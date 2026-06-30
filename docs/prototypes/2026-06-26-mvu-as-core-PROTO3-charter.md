# Prototype-3 Charter — MVU as the CORE, the primary interface to Buiy

> **Kickoff brief for a full `/staged-development` flow (research → spec → plan →
> execute, gated).** This is NOT "port prototype-2 into core." It is a from-research
> **re-decision** of the whole state-management paradigm under a new positioning, with
> the freedom to **redesign `buiy_core` itself** to fit. The proto-1/2 KEEP shapes are
> *inputs to re-decide*, not a foregone port.

## The shift

| Before (locked in the 2026-06-26 draft) | Proto-3 bet |
|---|---|
| Opt-in `buiy_mvu` crate, layered on top | **MVU is the PRIMARY interface to Buiy** |
| "Tools over the ECS; Buiy doesn't own app state" | The **recordable message substrate is core**; widgets route through the funnel |
| Substrate + surface both optional | Substrate core; surface core/default; **core may be redesigned to fit** |

## Why now — the trigger (from the proto-2 placement re-decision)

Optional placement **caps the headline thesis** ("one Msg log, N consumers → deterministic
tests + time-travel + agent-driving") at the *app* boundary. Three independent signals,
surfaced by building both prototypes, all point at the substrate wanting to be **core**:

1. **The `OnPress` coupling** — routing already reaches into `buiy_core::interaction`.
2. **Proto-2 REDESIGN #1 (agent actions through `update`)** — fully-reproducible
   agent-driving needs `buiy_core`'s in-process driver to lower actions *through* the
   funnel; an optional top-layer can't make core depend on it (dependency points the wrong
   way). The AccessKit read-tree is *already* core; the Msg write-log wants to be co-located.
3. **The recurring un-reflected `TextEditState` crux** (named identically by the hot-reload
   research) — widget-internal state (focus, edit buffer, selection, IME, scroll) lives in
   `buiy_core` and is **invisible to an optional MVU log**, so whole-UI replay/hot-reload is
   structurally impossible while the substrate is optional.

→ Decision to explore: **make the substrate core so the log is complete.** Proto-3 builds &
runs that to see what tight coupling actually buys — and costs.

## Advantages to EXPLORE (the upside of tight coupling)

1. **Complete recordable stream** — widget-internal state flows through the funnel ⇒
   whole-UI record / byte-identical replay / agent-driving / hot-reload (closes the
   `TextEditState` crux).
2. **Uniform control model** — widgets are controlled *through* the funnel; kills the
   self-update-vs-controlled double-write / one-frame flicker (proto-1 REFINE #5, the
   `Checkbox` `advance_toggle_on_press` race).
3. **Agent-interface unification** — action lowering through `update` becomes *natural*:
   one AccessKit read-tree + one Msg write-log, co-located in core (the write-side dual of
   the existing semantic tree).
4. **Core redesign opportunity** — reshape `buiy_core`'s interaction / focus / text-edit /
   a11y from ad-hoc systems into **MVU actors** where it genuinely simplifies them.

## What to RE-DECIDE on the new paradigm (inherit nothing blindly)

- **Authoring-surface placement** — now core/default (not opt-in). What stays a separable
  module vs. fully core?
- **Reducer ergonomics** — with core commitment, is the Bevy-`IntoSystem`-style **variadic
  macro** now worth building (infer the env, drop the turbofish, bare-param signatures)?
  (Proto-2 REFINE #1.)
- **Widget granularity** — does *every* widget become a `Model` + reducer, or do leaf
  widgets stay imperative and only *route*? (Scale + ergonomics hinge on this.)
- **`PureEnv`** enforcement core-wide (the proto-2 REDESIGN — sealed allowlist, not
  `ReadOnlySystemParam`), + a `#[derive(PureEnv)]` for user env structs.
- **`Cmd` algebra** — re-integrate `task`/`done`/`batch` (+ `stream`?) onto the core drain;
  + **dead-letter** (loud, typed) and **`catch_unwind` reducer supervision** as core concerns.
- **`LogicalId`** unified with the agent-interface **test-id space** (one identity space).
- **Migration model** — rewrite vs. incremental adoption of the mature widget set
  (`Button`/`Checkbox`/`Switch`/`Slider`/`TextField`/`Menu`/`Dialog`/`ScrollArea`/…).

## Hard questions / risks the research MUST hit

- **PERFORMANCE (load-bearing).** A funnel/drain/record on *every* interaction, Reflect-
  serialize cost on the hot path, mailbox/model overhead at thousands of widgets — vs. the
  **60 Hz hard floor on weak machines** the perf campaign defends. Needs: recording opt-out
  / sampling for hot paths, and hw-independent gates (iai-callgrind). Could be the thing
  that kills "every widget is an actor."
- **MIGRATION COST.** `buiy_core` is *mature* (layout, render, text, editing, a11y, widgets
  all built + verified). MVU-ifying it is a large reshape — scope it explicitly; a big-bang
  rewrite is likely wrong.
- **ESCAPE HATCH.** MVU-primary must still let power users drop to raw ECS systems — don't
  trap them in the paradigm.
- **WASM.** Reflect-everything + the bus must add **zero new wasm obstacles** (the wasm
  campaign constraint).
- **SCALE.** Per-entity model + mailbox + drain at thousands of widgets — memory + cost.

## Process — full `/staged-development`

1. **Research** — seeded by: this charter, both retrospectives, the draft spec (+ the
   `PureEnv` correction), and prior art. Re-open every decision.
2. **Spec** — the new **core-MVU target**; **supersedes** `docs/specs/2026-06-26-buiy-state-management-design.md`.
3. **Plan** — the migration/build steps (incremental, gated).
4. **Execute** — gated, fan-outs under `reliable-agent-fleet`; human gates at each stage.

Because this is the primary-interface bet, the spec it produces is the *real target*, not a
throwaway's notes — but the gates decide what merges.

## Logistics (read before starting)

- **Cut a FRESH worktree off CURRENT `origin/main`.** Core has moved since the proto-1/2
  base (`59cd50e` → main is `7752c01`+ per the parity/perf campaigns). Do **not** build on
  the stale `state-mgmt-elm-prototype` worktree — `git fetch` first, branch from
  `origin/main`.
- **Prior artifacts to seed research** (in the `state-mgmt-elm-prototype` worktree; commit
  recommended for durability — see the memory note):
  - Proto-1 (bespoke): `examples/mvu_spike/` + journal + retrospective.
  - Proto-2 (Bevy-native): `examples/mvu_native/` + PROTO2 journal + retrospective.
  - Draft spec: `docs/specs/2026-06-26-buiy-state-management-design.md`.
- **KEEP shapes proven across both protos** (re-decide, then likely port): `Messages`-inbox
  + ordered drain + record tap; `EntityEvent` routing; Yew `Callback`; `Reflect` log +
  `LogicalId`; the V-B reducer + sealed `PureEnv`; the `mvu_model(reducer).with_routing()`
  one-call wiring (model inferred via the `IntoSystem`-marker trick).
- **Prior-art folders to create/refresh during research** (`researching-prior-art`):
  **NEW — Rust MVU/reactive cores: `xilem/` (highest value — a Rust reactive UI core),
  `floem/`, `druid/`** (Linebender data-oriented); `relm4/` (still queued); refresh `iced/`
  (Elm-core); capture Elm/Redux time-travel; `gpui/actor-model.md` (Zed's actor core).

## First move post-compaction

Do **not** build. Start `/staged-development` at the **research** stage with this charter +
both retrospectives + the draft spec as inputs, and the prior-art creation above. Gate on
the research synthesis before writing the new spec.
