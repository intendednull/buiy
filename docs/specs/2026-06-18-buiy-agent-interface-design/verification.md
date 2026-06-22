# Verification

**Date:** 2026-06-18
**Status:** draft

How the agent-interface design is verified. Every gate runs **HEADLESS** in CI — no `winit` adapter, no GPU, no display — by driving the in-process seam ([inprocess-api.md](inprocess-api.md)) directly: `snapshot()` calls `build_tree` then `build_tree_update(builder.snapshot(), focus)` and feeds the resulting `accesskit::TreeUpdate` into an [`accesskit_consumer::Tree`](#accesskit_consumer-dependency), the same consumer a real AT drives. Control flows through `dispatch_action_request(&mut World, &ActionRequest)` (the locked headless seam, [action-router.md](action-router.md)) — which mints/replays `ActionRequestWrapper` **as Bevy messages** — so the full inbound→sink→outbound loop runs with zero windowing or rendering.

Reuses the foundation's `accesskit_consumer`-driven gates rather than inventing a parallel harness: gates **#3/#4/#6/#7** plus the **#12** invariants, defined in [`../2026-05-07-buiy-foundation/verification.md`](../2026-05-07-buiy-foundation/verification.md) and committed to in [`../2026-05-07-buiy-foundation/accessibility.md`](../2026-05-07-buiy-foundation/accessibility.md). This spec lights them for the decomposed component surface, the router, and the per-widget APG contracts.

Gates land in `crates/buiy_core/src/a11y/inprocess.rs`, driven from `buiy_verify`. See [semantic-tree.md](semantic-tree.md) (the model under test), [widget-contracts.md](widget-contracts.md) (the contracts the gates enforce), [phasing.md](phasing.md) (which gate lights when), and [README.md](README.md) (index + the transport research-debt note).

Prior art: the one-tree/N-consumers reuse discipline ([../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md), [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md): the same `accesskit_consumer` view serves AT, agent, and test — no fourth model); the `accesskit_consumer` tree model and Node API ([../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md), [../../prior-art/accesskit/api.md](../../prior-art/accesskit/api.md)); the APG keyboard contracts ([../../prior-art/wai-aria-apg/keyboard-contracts.md](../../prior-art/wai-aria-apg/keyboard-contracts.md), [../../prior-art/wai-aria-apg/patterns-catalog.md](../../prior-art/wai-aria-apg/patterns-catalog.md)); the live-region derivation ([../../prior-art/wai-aria-apg/live-regions.md](../../prior-art/wai-aria-apg/live-regions.md)); the megacomponent anti-pattern ([../../prior-art/bevy-a11y/component-model-incident.md](../../prior-art/bevy-a11y/component-model-incident.md)).

---

## Verification-first discipline

Follow the lowest-tier rule (`using-buiy-verification`, [`../2026-06-15-buiy-verification-design/README.md`](../2026-06-15-buiy-verification-design/README.md)): **add a test at the lowest tier that can observe the bug.** This design adds a new lowest tier:

```
layout-snapshot < display-list-snapshot < invariant < reftest < golden
        ^ NEW: semantic-tree snapshot (role/name/description/state/relations/actions/ref)
```

The **semantic-tree snapshot** ([semantic-tree.md](semantic-tree.md), [inprocess-api.md](inprocess-api.md)) is the new floor — every role/name/description/state/relation/action/ref regression, no rasterization, ~2–5KB deterministic structured tree. Rules:

- A role/name/description/state/relation/advertised-action/`ref` regression is a **semantic-tree snapshot** bug (#3) — never a reftest or golden.
- An announcement string/order regression is an **announcement snapshot** bug (#4).
- A "key X did the wrong thing" regression is **APG-conformance** (#7), driven through the action seam (#6).
- A structural property (orphan, unnamed focusable, unreachable focus) is an **invariant** bug (#12, proptest).
- **Reftest/golden are reserved for the rasterization residue only** (focus-ring paint, caret blink, slider thumb position). They run in the additive GPU `--ignored` lane (Tiers 4–5), which must pass on a GPU host while the headless gate stays green without an adapter.

Because the new tier and #3/#4/#6/#7/#12 are all headless, the **entire design is verifiable in the no-adapter CI lane** — the GPU lane only re-confirms on-screen residue.

---

## `accesskit_consumer` dependency {#accesskit_consumer-dependency}

The gates read an `accesskit_consumer::Tree`, **not currently a declared dependency**. Phase 1 adds it to `crates/buiy_core/Cargo.toml`, **pinned to the version matching the resolved `accesskit` 0.24** (verify via `cargo tree`/`cargo doc`, not docs.rs — the registry has 0.36/0.37; `Cargo.lock` is gitignored here). A normal (non-dev) dependency because `inprocess.rs` ships in the library. Run `cargo deny check` when adding the dependency; confirm no skew against `accesskit` 0.24 / `bevy_winit` 0.19-rc.3. The 0.24 vocabulary (setters, the 22-variant `Action` enum, `accesskit_consumer`) is the current base — the Bevy 0.19-rc.3/wgpu 29 bump landed (PR #70) and this branch sits on it.

---

## Gate #3 — AccessKit tree snapshots (per widget × state)

**Proves:** the decomposed surface lowers to a correct AccessKit tree. Maps to **WCAG 4.1.2 (Name, Role, Value)** ([../../prior-art/wai-aria-apg/wcag-22-aa-mapping.md](../../prior-art/wai-aria-apg/wcag-22-aa-mapping.md)).

**How:** per widget × state, `snapshot(world, TreeView::Unmerged)` → `SemanticTree` read through `accesskit_consumer::Tree`. Asserts per node: `role + name + description + states + relations + actions + ref`, where `states` is the present-only projection of the decomposed components, `relations` is `A11yRelations` resolved to `NodeId`s, `actions` is the contract-advertised set, and `ref` is `node_id_for(entity).0` (= `entity.to_bits()+1`).

**Coverage rule (load-bearing):** **every new state component and every new `A11yRole` variant ships its own #3 fixture in the same change.** The stringifier lockstep (`translate::role_to_accesskit` AND `buiy_verify::a11y::role_to_str` add the new arm together) is a #3 concern — a forgotten arm surfaces as `"Unknown"` (the `_ => "Unknown"` wildcard would otherwise mask it).

**Ref correctness:** Phase 0 fixes `buiy_verify::a11y::snapshot_tree` (emits raw `n.entity.to_bits()` at a11y.rs:47) to emit `node_id_for(n.entity).0`, re-blessing affected goldens. Without it a #3 snapshot and a #6 replay disagree on addressing.

---

## Gate #4 — announcement-output snapshots

**Proves:** live-region utterances fire with the correct **string** and **order**.

**How:** the announcer surface is `A11yLive { politeness, atomic }` plus the **role-implied live derivation** — the derive fold maps role to implicit politeness/atomic *before* applying any author override ([../../prior-art/wai-aria-apg/live-regions.md](../../prior-art/wai-aria-apg/live-regions.md)): alert⇒assertive+atomic, status⇒polite+atomic, log⇒polite, timer⇒its APG-implied politeness. An alert dialog carrying `A11yRole` but no explicit `A11yLive` must still announce assertively.

**Component note:** `atomic` lowers via `Node::set_live_atomic` — **NOT** `set_atomic`, which does not exist in 0.24 (accesskit-0.24.1 lib.rs:1806). Politeness via `set_live`.

The fixture drives a state change through the headless seam, reads the utterance(s) via the same `accesskit_consumer` view, and asserts both the exact string and the order. Exercised: slider value-change (inbound `Increment`/`Decrement`/`SetValue` mutates `A11yValue.now`; assert the value-text string follows the focus announcement) and checkbox-toggle (inbound `Click` advances `A11yToggled`; assert checked/unchecked/mixed in order). Both inbound-driven, so #4 also exercises the router end-to-end.

---

## Gate #6 — synthesized input replay

**Proves:** an inbound `ActionRequest` produces the correct transition through the real pipeline. The locked "Buiy's own test driver."

**How:** the headless seam (`dispatch_action_request(world, req)`) is the per-request body of `route_action_requests`. The driver either (1) **mints an `ActionRequestWrapper` and writes it as a Bevy message** into the existing `MessageReader<ActionRequestWrapper>` channel (exercising the real system end-to-end — the same channel `bevy_winit::poll_receivers` fills, LOCKED #6, no competing handler); or (2) **calls `dispatch_action_request` directly** (tighter unit assertions, sidesteps the one-frame `poll_receivers` latency). Then ticks until settle, re-snapshots, asserts the transition. Keyboard/pointer **Bevy events** injected into the same app feed this gate too.

**Same-frame ordering:** the guarantee that synthesized `OnPress`/`FocusedEntity`/`EditCommand`/value effects are consumed the same frame depends on **explicit intra-`BuiySet::Input` ordering** — Bevy does not order systems within a set automatically. The router declares `.before(emit_on_press_on_click)`/`.before(handle_tab)`/`.before(apply_keyboard_edits)`/`.before(focus_on_click)` (or a first-in-`Input` sub-set). (Cross-set ordering is already correct — the `BuiySet`s are `.chain()`ed at lib.rs:87-93 with `Input` before `A11yUpdate`.) #6 fixtures assert the same-frame transition. (Detailed in [action-router.md](action-router.md).)

**Capability re-check:** a #6 fixture replays `SetValue` against a **read-only** `TextInput` and asserts a typed `ActionError` (guidance/no-op), the text unchanged — the Compose precedent. Verifies the router's live per-instance filter (re-reads `A11yReadOnly`/`A11yDisabled`/live `A11yValue` bounds), on top of `apply_tracked`'s own `read_only` rejection. Same fixture form covers `A11yDisabled` dropping every verb and a slider `SetValue` clamped to `[min,max]`.

**Text-action prerequisite:** `SetTextSelection` and `ReplaceSelectedText` cannot lower through today's `EditCommand` (`crates/buiy_core/src/text/edit/command.rs:21`) — it has `Motion`/`Insert`/`Backspace`/`Delete`/`Enter`/`Cut`/`Copy`/`Paste`/`Undo`/`Redo`/`SelectAll`/`Escape`/`Submit` but **no absolute-selection variant**, and `Motion` is directional. These two require a **new `EditCommand::SetSelection { anchor, focus }`** (and `dispatch_action_request` assembling `&mut FontSystem + &mut EditContext`). Their #6/#7 fixtures land **with** that variant; until it exists they assert `ActionError::Unsupported`. `SetValue` (text) lowers fine today via `SelectAll` + `Insert` (both exist). See [widget-contracts.md](widget-contracts.md).

---

## Gate #7 — APG keyboard-contract conformance

**Proves:** every documented key per widget produces the APG-correct transition. AccessKit models role/state/verbs but **not key bindings** — Buiy implements the keyboard contract consumer-side, so this gate verifies Buiy's key→action mapping ([../../prior-art/wai-aria-apg/keyboard-contracts.md](../../prior-art/wai-aria-apg/keyboard-contracts.md)).

**How:** replay every documented key as a Bevy `KeyboardInput`, assert the transition (or correct non-transition) via the post-state `SemanticTree` through `accesskit_consumer`. **Both Enter and Space replayed for every activatable widget:**

| Widget | Keys | APG-correct |
|---|---|---|
| Button | Enter **and** Space | both activate → `OnPress` (closes the two `button.rs` TODOs) |
| Checkbox | Enter **and** Space | **Space only** toggles `A11yToggled`; **Enter does nothing** |
| Switch | Enter **and** Space | Space toggles `A11yToggled` (binary) |
| Disclosure/accordion | Enter/Space → Expand/Collapse | toggles `A11yExpanded`; `Expand`/`Collapse` honored in addition to `Click` |
| Slider | Right/Up, Left/Down, Home, End, PageUp/Dn | inc/dec by `step`, →min, →max, ±`jump` (clamped) |

The Checkbox "Enter does nothing" assertion is canonical — a Space-only fixture would let an erroneous Enter-handler ship.

---

## Gate #12 — a11y-tree invariants (proptests)

**Proves:** the tree is structurally sound for *any* composition. **Owned by the a11y subsystem** — these land in `crates/buiy_core/src/a11y/inprocess.rs` (or a sibling `invariants` module), **not** `buiy_verify` (which drives them as a tier). Each generates a random a11y graph (ChildOf + `owns` overrides + `A11yHidden` prunes), builds via the in-process seam, asserts: (1) **no orphans** — every node reachable from root; (2) **every focusable has an accessible name** — non-empty `compute_accessible_name` (the structural counterpart to WCAG 4.1.2); (3) **focus reachable** — the focus target is a node in the tree, the tab sequence reaches every focusable. The `owns` re-parent + ChildOf filtering edge cases (cycles, duplicate-parent) are exercised; the once-logged drop-on-cycle guard ([semantic-tree.md](semantic-tree.md)) keeps the tree acyclic under every generated graph.

---

## The lockstep check (advertise = honor)

**Advertisement and honoring share one source of truth.** `A11yContract::actions()` drives outbound `add_action`; the same contract's `honor()` + the router's live filter drive inbound dispatch. So: **advertise-without-honor** is caught by **gate #7** (the APG fixture replays the advertised key, no honoring arm, the transition doesn't happen, the test fails); **honor-without-advertise** can **never fire** (the inbound guard re-validates advertisement before dispatch → `Unsupported`). Gate #7 is the mechanism that makes the advertise/honor contract self-checking.

---

## Scoped-honest limitations (do not over-claim)

- **Actionability `HitTargetable` depends on stacking-aware hit-testing landing first.** `picking::hit_test` (`picking/mod.rs:37`) returns the smallest-AABB entity — stacking/top-layer/z-order UNAWARE. The actionability gate ([inprocess-api.md](inprocess-api.md)) cannot today prove "not obscured by a modal/top-layer/tooltip." Phase 1 ships `HitTargetable` **AABB-only** with the limitation documented; the stacking-aware upgrade is an honest follow-up. Do not present `hit_test` as already answering the overlay question.
- **`SetTextSelection`/`ReplaceSelectedText`** are scoped out until `EditCommand::SetSelection { anchor, focus }` exists (above); their fixtures assert `ActionError::Unsupported` in the interim.

---

## Running the gates

All of #3/#4/#6/#7/#12 run in the **headless** workspace gate — no adapter, no GPU. Before any commit, run the project's full check command (CLAUDE.md § Build & Test) and resolve every warning:

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

(`xvfb-run -a` is harmless but not required.) Run `cargo deny check` when adding `accesskit_consumer`. The GPU `--ignored` lane (`cargo test -p buiy_core -j 2 -- --ignored --test-threads=1`) is additive and only re-confirms residue. A gate is verified when its fixture **runs and passes**, read through `accesskit_consumer` — never "should work" (`superpowers:verification-before-completion`).

---

## Cross-links

- [README.md](README.md) — index, locked decisions, rejected alternatives.
- [semantic-tree.md](semantic-tree.md) — components, role-implied live, the tree the gates snapshot.
- [action-router.md](action-router.md) — `dispatch_action_request`, the intra-`Input` ordering, the live filter.
- [widget-contracts.md](widget-contracts.md) — the per-widget contracts and the `EditCommand::SetSelection` prerequisite.
- [inprocess-api.md](inprocess-api.md) — the `snapshot`/`perform`/`act_when_actionable` seam.
- [phasing.md](phasing.md) — which gate lights in Phase 0 / 1a–1d.
