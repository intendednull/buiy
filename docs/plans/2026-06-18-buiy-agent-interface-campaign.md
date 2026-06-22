# Buiy Agent-Interface — Campaign Plan

> **For agentic workers:** This is the **coordination/sequencing** plan for the
> multi-phase agent-interface campaign. Each phase has its own detailed,
> bite-sized TDD plan (written just-in-time — see *Per-phase plans* below).
> Execute each phase's plan with `superpowers:subagent-driven-development`,
> review-gated between phases.

**Date:** 2026-06-18
**Status:** active
**Spec:** docs/specs/2026-06-18-buiy-agent-interface-design/README.md

**Goal:** Make Buiy's AccessKit semantic tree richly decomposed and
**bidirectional** — one canonical tree serving screen readers, Buiy's own
headless test driver, and (Phase 2, opt-in) an external LLM agent — driven
through one Action ingress, one generic `perform` primitive, and one in-process
inspect/control contract.

**Architecture:** Decomposed `A11yStates`/`A11yRelations` components → a flat
0.24-setter "derive fold" → real ECS-tree nesting → one inbound `Action` ingress
(the existing `bevy_winit` `MessageReader<ActionRequestWrapper>`, no competing
`ActionHandler`) → a transport-agnostic in-process contract (`a11y::inprocess`)
that doubles as the headless test driver. `buiy_mcp` (Phase 2) wraps that
contract over a socket, unchanged. Accessibility leads; agent-drivability rides
the same nodes.

**Tech Stack:** Rust, Bevy ECS, `accesskit` + `accesskit_winit` (+ new
`accesskit_consumer`), `bevy_winit`. Target **accesskit 0.24 / Bevy 0.19-rc.3**.

---

## Base — accesskit 0.24 / Bevy 0.19-rc.3 (the bump landed) (LOCKED #7)

The spec targets **accesskit 0.24 / Bevy 0.19-rc.3**, and that is now the
**current base**: the BSN/0.19 bump LANDED (PR #70, `main` @ `3b3b0ba` —
Bevy 0.19.0-rc.3, accesskit 0.24, accesskit_winit 0.32, plus the new `buiy_bsn`
crate). This branch is rebased onto that base, so there is **no remaining
version gate** — every phase simply targets the current 0.24 surface.

**Rule:** at implementation time, **verify every 0.24 setter/Action signature
against the resolved deps (`cargo tree` / `cargo doc`), not docs.rs** (`Cargo.lock`
is gitignored here, so it regenerates from the `Cargo.toml` pins on build).

**Phase 0 is version-stable.** P0's three changes (`entity_for_node_id`, the
snapshot off-by-one fix, the `A11yRole` additions) carry **no 0.24-specific API
risk**: the new fn is pure, the serializer fix is pure, and the new `Role`
variants (`CheckBox`/`Switch`/`Slider`/`TextInput`/`MultilineTextInput`/`Region`/
`Group`) exist across the 0.18→0.19 lines alike. P0 runs on the current
0.19-rc.3/0.24 base; it happens to be version-stable, so it carries no migration
risk. (See the P0 plan's opening note.)

---

## Phase sequence

Each phase produces working, testable software on its own and ends at a
**fresh-agent review gate** (the project's default research→spec→plan→execute
discipline: review after each wave, verify don't just read). Each phase's
detailed plan is written just-in-time once its predecessor lands and the 0.24
surface is confirmed against the resolved deps (`cargo tree`/`cargo doc`).

| Phase | Goal | Detailed plan | Touches 0.24 API? |
|---|---|---|---|
| **P0** | Addressing + serializer fix + role additions (no behavior change) | `2026-06-18-buiy-agent-interface-p0-addressing.md` | No (version-stable) |
| **P1a** | Decomposed component surface (outbound): states + relations + derive fold + ACCNAME | _(written after P0 lands)_ | Yes |
| **P1b** | Real ECS nesting + `owns` overlay + hidden prune; land gate #12 invariants | _(written after P1a)_ | Yes |
| **P1c** | Inbound router + `A11yContract` + in-process driver; `EditCommand::SetSelection`; button keyboard activation | _(written after P1b)_ | Yes |
| **P1d** | APG widgets (Checkbox/Switch/Slider/Disclosure/Accordion/Dialog/Tooltip) + TextInput role upgrade | _(written after P1c)_ | Yes |
| **P2** | `buiy_mcp` opt-in transport + named follow-ups | _(written after P1d)_ | Yes |

### P0 — Addressing + serializer fix + role additions
- **Deliverables:** `entity_for_node_id(NodeId) -> Option<Entity>` (inverts the
  `+1`); fix `buiy_verify::a11y::snapshot_tree` to emit `node_id_for(entity).0`
  (the canonical ref); extend `A11yRole` with the 7 new variants, updating
  **both** stringifiers (`translate::role_to_accesskit`, `a11y::role_to_str`) +
  the `KNOWN_ROLES` test array in the same change.
- **Review gate:** gate #3 stays green (the snapshot serializer + role table);
  no behavior change to the live tree.
- **Detailed plan:** `docs/plans/2026-06-18-buiy-agent-interface-p0-addressing.md`.

### P1a — Decomposed component surface (outbound)
- **Deliverables:** `a11y/states.rs` (the ~21 decomposed state components) +
  `a11y/relations.rs` (`A11yRelations`), `register_type`'d in `A11yPlugin`;
  widen `A11yNodeView` + the `build_tree` query tuple; rewrite `to_accesskit_node`
  as the flat 0.24-setter derive fold (incl. `set_live_atomic`, role→implicit-live
  derivation); `compute_accessible_name` (ACCNAME 1.2) as a fn in `buiy_core`.
- **Review gate:** gate #3 tree-snapshot over the in-process `accesskit_consumer`
  path (added as a dep this phase — see inprocess-api.md). Every new state
  component + role variant ships its #3 fixture.
- **Spec:** `semantic-tree.md` §1–§6.

### P1b — Real nesting
- **Deliverables:** `build_tree` reads `ChildOf`/`Children` filtered via
  `nearest_a11y_ancestor` + `A11yRelations.owns` re-parent override (cycle-drop
  guard) + `A11yHidden`/inert prune; `build_tree_update` emits parent→children
  edges; root keys off the window entity. Merged/unmerged projection knob. Land
  gate #12 invariants (no orphans / focus-reachable / accessible-name-present)
  as proptests owned by the a11y subsystem.
- **Review gate:** gate #12 proptests green; gate #3 nesting fixtures.
- **Spec:** `semantic-tree.md` §7–§8.

### P1c — Inbound router + `A11yContract` + in-process driver
- **Deliverables:** `A11yContract` trait + role→contract registry (`contract.rs`);
  `action.rs` — `route_action_requests` draining `MessageReader<ActionRequestWrapper>`,
  `entity_for_node_id`, the liveness + live per-instance capability guard,
  `dispatch_action_request` free fn dispatching every 0.24 `Action` into the real
  Focus/`OnPress`/`EditCommand`/slider/expanded/tooltip sinks, ordered explicitly
  within `BuiySet::Input`; the new **`EditCommand::SetSelection { anchor, focus }`**
  variant (today's `Motion`-based set can't place an absolute range — real editor
  work); button keyboard activation (Enter+Space → `OnPress`, closing the two
  `button.rs` TODOs); `inprocess.rs` — `snapshot`/`perform`/`get_by_role`/
  `act_when_actionable`/`wait_for` + the headless injection seam.
- **Review gate:** gates **#3, #4** (A11yLive announcements), **#6** (input replay
  through the seam — *the* locked test driver), **#7** (APG keyboard-contract
  conformance incl. Enter-vs-Space asymmetry).
- **Spec:** `action-router.md`, `widget-contracts.md`, `inprocess-api.md`.

### P1d — Widgets to exercise the surface
- **Deliverables:** Checkbox, Switch, Slider, Disclosure/Accordion, Dialog,
  Tooltip-trigger in `buiy_widgets`, each wiring role + decomposed state
  components + an `A11yContract` impl; TextInput upgraded off the `A11yRole::Text`
  stopgap (role split + `A11yTextValue`/field state). Each ships its #3/#7
  fixtures (both Enter and Space asserted).
- **Review gate:** gates #3/#7 per widget.
- **Spec:** `widget-contracts.md`.

### P2 — Opt-in transport
- **Deliverables:** `buiy_mcp` crate (socket transport, MCP envelope, capability
  tiers, push-deltas via change detection, the structured-arg RPC lane escaping
  `CustomAction`'s i32 limit). Named follow-ups layered here: author-supplied
  test-ids over the NodeId ref; multi-window per-`WindowId` tree keying;
  stacking-aware `hit_test` for the actionability `HitTargetable` gate; lazy
  `TreeUpdate` diffing gated on `AccessibilityRequested`.
- **Review gate:** opt-in build green; the in-process contract unchanged.
- **Spec:** `mcp-companion.md`, `phasing.md` (follow-ups).

---

## Per-phase plans (just-in-time)

Per the project's campaign convention (layout/text/render campaigns) and the
spec's own instruction to **verify 0.24 signatures against the resolved deps
(`cargo tree`/`cargo doc`) at implementation time**, the detailed bite-sized TDD
plan for each 0.24-touching phase (P1a onward) is written **after its predecessor
lands and the 0.24 surface is confirmed** — not up front against speculative
docs.rs signatures. P0's plan is written now (it is version-stable).

To write the next phase's plan: re-invoke `superpowers:writing-plans` against the
relevant spec section(s) with the predecessor's landed code as ground truth.

## Verification (every phase)

The project check command before any commit (from `CLAUDE.md`):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

All agent-interface gates (#3/#4/#6/#7/#12) run **headless** (no winit adapter,
no GPU) via the `inprocess.rs` direct seam over `build_tree_update(...) ->
accesskit_consumer::Tree`. The GPU `--ignored` lane is **not** exercised by this
campaign.
