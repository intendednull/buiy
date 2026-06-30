# Prototype-3 — MVU as core: the build-to-learn design

> **This is a PROTOTYPE (prototype-first-development). DO NOT MERGE.** The deliverable is
> *learning* — this design + the [journal](2026-06-26-mvu-as-core-PROTO3-journal.md) + the
> retrospective. We build the **full maximalist bet** on purpose, **run it**, and **measure**
> it, so the FINAL (the real, merge-gated spec) is re-decided from evidence rather than argued
> on paper.

**Worktree:** `mvu-core`, branch `worktree-mvu-core`, off `origin/main` @ `5c0da9f`.
**Inputs:** the research [SYNTHESIS](../reports/2026-06-26-mvu-as-core-research/SYNTHESIS.md)
(decisions D1–D10 + risk verdicts), the 7 survey artifacts + the prior-art folders
(`druid/`, `relm4/`, `elm-redux-time-travel/` + the existing reactive-cores corpus), the
proto-3 charter, and the two research gates (both `pass-with-fixes`).

---

## 1. The bet (build it, don't hedge it)

As chartered: **MVU is the primary interface to Buiy.** A recordable message substrate lives
in `buiy_core`; widgets route through one ordered funnel; the log is complete over the
MVU-governed subtree → whole-UI **record/replay + agent-drive + hot-reload**. The mature
widget set is migrated **internally** (seam, not rewrite). We are not deciding *whether* this
is worth it by argument — the prototype is how we find out.

## 2. Why a prototype answers the gate (not a smaller scope)

The adversarial research gate's three load-bearing objections are all **empirical**, so the
honest response is to build and measure, not to shrink:

| Gate objection | How the prototype answers it (a *learning goal*, not a paper claim) |
|---|---|
| **No killer use case** for widget-internal completeness | **L2** — build whole-UI record/replay that captures caret/scroll/focus and replays byte-identical; judge if it's real and valuable by *using* it. |
| **Perf "FEASIBLE" is unmeasured** (the iai weak-machine pricer doesn't exist) | **L1** — build the pricer; measure the funneled hot path (caret-blink, scroll, slider, IME) under `set_if_neq`. A *number*, go/no-go. |
| **Single-writer win is cheap + bundled** to justify the expensive bet | **L3/L4** — build single-writer *and* the full substrate separately; the journal records what each actually cost and bought, so the retrospective can un-bundle them. |

## 3. What we build (the design — detail in SYNTHESIS D1–D10)

Crystallized from the synthesis; the synthesis holds the rationale + runner-ups. The
prototype builds these to learn, refining as running reveals problems:

- **`buiy_core::mvu` substrate** (D1): funnel + **single ordered drain** (a *system*, never an
  observer) + `Model`/`Cmd` + **sealed `PureEnv`** (allowlist, not `ReadOnlySystemParam`) +
  `LogicalId`. Drain no-ops when no `Model` is present.
- **`set_if_neq` drain discipline + `MvuWorkCounters`** (D10/PERF, the load-bearing rule): the
  drain `deref_mut`s a model only on a real change; binds fire only on `Changed<Model>`.
  Counters (`drain_folds`, `messages_recorded`, `models_mutated`, `binds_fired`,
  `emits_refolded`) gate idle == all-0, idempotent-fold == `models_mutated == 0`.
- **Tiered granularity** (D2) — *every widget an actor is REJECTED*:
  - **Router leaf** (`Button`): no model, routes a Msg.
  - **Stateful leaf** (`Checkbox`/`Switch`/`Slider`/`Disclosure`/`ScrollArea`): keep the
    existing single-source-of-truth component; **the drain is the SOLE writer**; one shared
    role-keyed reducer; no per-entity `Model` trait.
  - **Machine** (`Menu`/`Dialog`/`Popover`): a real `Model`+reducer owning multi-field state.
  - **Composite / raw-ECS**: the escape hatch, outside the replay boundary.
- **Single-writer discipline** (D3): light-dismiss/press/Escape/AT/controlled-parent all
  **enqueue**; one reducer writes the one field. Deletes `menu::sync_menu_dismissed`; kills the
  gallery `A11yToggled` race. Flicker cannot occur by construction.
- **Schedule integration** (D10): `MvuSet` folded into `BuiySet`
  (`Enqueue → ApplyDeferred → Drain → Bind`), pinned; collapse proto-2's `OnPress → Routed<M>`
  two-hop; the editor drain pinned **before** `reshape_edited_editors`.
- **`Cmd` algebra** (D6): `none`/`done`/`task`/`batch` + a keyed **`Subscription`** (for
  timer/IME/OS sources — required for replay completeness); `InFlight`/takeLatest; `task`
  folds its result **back through the drain** as a recorded Msg; envelope **origin tags**
  (`User`/`Command`/`Folded`/`Subscription`); **dead-letter** (loud, typed); **`catch_unwind`
  supervision** (cfg-gated native).
- **Text-edit** (H5): **command-sourcing** — record the resolved `EditCommand`/IME stream,
  re-fold from a seed; a small `TextEditSnapshot` projection for hot-reload. The editor is the
  documented **PureEnv exemption** (impure: `&mut FontSystem` + clipboard) — an imperative
  routing leaf, determinism guaranteed at the boundary. Needs a Buiy-owned `Reflect` `Motion`
  + `ImeCommand` mirror (cosmic types are foreign).
- **Record / replay** (H7): `RecordMode { Off, Ring(n), Full }` **default OFF + lazy** (typed
  Msg in a bounded ring; RON only at export). Replay drops `Cmd`s + re-feeds logged
  effect-results; env seeded at record-start (Elm-flags) + an invariance assertion.
- **Identity** (D7): one author-assignable `LogicalId` **layered over** the AT `NodeId`
  (resolver registry; deterministic structural fallback — no `uuid`/random, replay + wasm
  demand it).

## 4. Learning goals (the prototype's real product)

Every wave updates the journal against these. The retrospective answers each with evidence.

- **L1 — PERF (load-bearing, go/no-go).** Build the **iai-callgrind weak-machine pricer**
  (`pipeline_iai.rs` is net-new — does not exist on main): `mvu_idle/{N types}`,
  `mvu_one_message`, `mvu_fold_storm/{1,10,100}`, `mvu_record_off_vs_on`. Then measure the
  **funneled hot path** — caret-blink (the canonical high-frequency internal signal),
  scroll-drag, slider-drag, IME preedit — and prove (or fail to prove) `node_rebuilds == 0`
  under `set_if_neq` (resolve open-Q11). Prove the idle floor is **flat in widget count**.
- **L2 — KILLER USE CASE.** A whole-UI record/replay (and a minimal time-travel scrub) that
  captures **widget-internal** state (caret position, scroll offset, focus) and replays
  byte-identical — the thing the app-boundary log provably cannot do. Judgment: is it real
  and valuable, or does every concession (editor-exempt, focus-special, hatch-outside) hollow
  it out? *Use it and decide.*
- **L3 — MIGRATION (seam vs rewrite).** Pilot the seam on **text-edit** (the crux) and one
  **machine** (`Menu`). Does rerouting *writers* (not redesigning data) actually hold? **Run
  the gallery** every wave.
- **L4 — COMPLETENESS DELTA.** On a real MVU-ified scene, what completeness does core-primary
  deliver *after* the editor/focus/hatch concessions — and what did it cost vs. the
  separable single-writer win?
- **L5 — REAL-USER-INPUT COMPLETENESS.** Does real pointer/keyboard input enter the funnel as
  logged Msgs, or does "record the `ActionRequest` stream" capture only agent/AT actions?
  (Today pointer/keyboard write `OnPress` directly and never mint an `ActionRequest`.)
- **L6 — ESCAPE-HATCH TRAP.** For a *migrated* widget, does a power-user `&mut A11yToggled`
  write race the drain (re-creating the multi-writer bug)? What's the honest hatch contract?

## 5. Verification (RUN — don't trust green)

Headless green ≠ works (the standing Buiy lesson). Every wave:
- **Run the artifact** — the widget gallery / a todomvc; watch the real GUI on the AMD GPU.
- **The iai pricer** is a first-class deliverable, not an afterthought (L1).
- **GPU lavapipe lane** on any machine/widget conversion (the Menu anti-clobber precedent — a
  reconciliation regression the headless gate cannot catch).
- The **live-interaction test tier** (real shell + picking + synthetic clicks) for routed
  interaction.

## 6. Wave plan (front-load the load-bearing risk)

Driven **sequentially in the warm `mvu-core` worktree** (core changes are interdependent;
isolated cold worktrees + merges = conflict hell). RUN + journal each wave.

| Wave | Build | Proves |
|---|---|---|
| **W1** | `buiy_core::mvu` substrate (funnel/drain/Model/Cmd/PureEnv) + `set_if_neq` + `MvuWorkCounters` + **the iai pricer** | L1 first cut: idle floor, one-message, fold-storm, record off-vs-on |
| **W2** | Single-writer discipline + the tiered leaf (Checkbox/Switch/Slider drain-sole-writer) | L3/L4 (cheap win, un-bundled); gallery runs, no flicker |
| **W3** | Text-edit pilot: command-sourcing (`EditCommand`/`Motion`/`ImeCommand`) + `TextEditSnapshot` | L3 crux; editor-as-routing-leaf; **measure caret-blink funneled** (L1) |
| **W4** | Record/replay (default-OFF lazy) + the whole-UI replay **killer-use-case demo** | L2; **measure scroll/slider funneled** (L1); L5 (real input recorded?) |
| **W5** | One machine (`Menu`) → `Model`+reducer; agent-drive a routed action through `update` | L3 machine tier; GPU lavapipe gate; L6 hatch contract |

Scope is elastic — if L1 at W1/W3 says the funneled hot path **can't hold 60 Hz**, that's a
*successful* prototype outcome (it kills the maximalist framing early, with a number). We
follow the evidence.

## 7. Gate's must-fixes folded in

- Correct the **mis-cited 3.18 ms / 5000-node** figure in `SYNTHESIS.md` (the audit has
  10.87 ms@128, 45.38 ms@512, ~85 µs/node) — done at spec-stage cleanup, labeled as estimate.
- **Structural-ops-off-log** + **real-user-input completeness** are L-goals here (W4) and
  become resolved decisions in the FINAL spec, not open questions.
- Pull the **Elm/Redux DevTools verb algebra** (jump/skip/pause/lock/commit/sweep/dispatch/
  import-export; `dispatch` == the agent-drive write-path) into the W4 replay demo.
- **Docs-index** (`docs/README.md`) reconciliation for the research + new prior-art folders +
  this prototype — folded into the retrospective-stage docs pass.

## 8. Exit → retrospective → FINAL

When the waves have run + measured, write the **retrospective** (keep/refine/redesign every
decision, with the numbers). It seeds the **FINAL** staged-development pass whose spec
supersedes `docs/specs/2026-06-26-buiy-state-management-design.md` and is **merge-gated on
human review**. The prototype itself never merges.
