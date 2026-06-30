# Prototype-3 retrospective — MVU as core: for the FINAL

> **The prototype's real deliverable.** Synthesizes the [journal](2026-06-26-mvu-as-core-PROTO3-journal.md)
> (Stage 0 + W1–W5) into keep / refine / redesign for the FINAL staged-development pass. The
> prototype code (in the `worktree-mvu-core` branch, off `origin/main` @ `5c0da9f`) is an
> **unmerged reference — DO NOT MERGE.** The FINAL re-decides every choice with this in hand.

## Verdict

**The bet is VIABLE, and the central thesis is PROVEN — but the maximalist framing is tempered
by evidence, exactly where the adversarial research gate predicted.**

What was proven by building + running + measuring:
- **The editor crux (W3):** editor state replays **byte-identically** (value + caret + selection)
  by re-folding a recorded command stream, *without ever serializing `cosmic_text::Editor`*. The
  un-reflected `TextEditState` — the thing that supposedly made whole-UI replay impossible — does
  not block it. This was the campaign's motivating risk; it is retired.
- **The killer use case (W4):** a whole-UI session of **real** input (clicks → toggles, raw
  keyboard/IME → editor) replays byte-identically over the MVU-governed subtree — a capability an
  **app-boundary-only log cannot deliver** (it includes widget-*internal* state). This is the
  evidence the adversarial gate demanded for "core over opt-in."
- **The perf bet (W1):** the real risk was *the drain defeating change-detection*, not
  Reflect-serialize. `set_if_neq` fixes it by construction (idempotent fold ⇒ `models_mutated==0`,
  no `Changed<M>` cascade), and the substrate is *measured* cheap (~525 instr/fold; recording
  default-OFF pays zero).

What the evidence tempered:
- **The machine-tier migration cost is real and the per-widget win modest (W5)** — the adversarial
  gate's "most dangerous, smallest win" was correct for `Menu`.
- **The agent-interface write-side unification hits a consistent wall:** the core AT synchronous
  act-then-observe seam (`dispatch_action_request`) cannot be funneled from a funnel layered
  *above* core (the W2 slider scope-out **and** the W5 AT-`Expand` gap are the *same* wall).
- **The replay guarantee is SCOPED** (H6): the MVU-governed subtree only — structural ops
  (spawn/despawn) and escape-hatched raw-ECS writes are out, proven by a failing-in-the-flesh test.

**Recommendation for the FINAL:** ship the substrate **in core** (placement now *evidence-backed*,
not asserted — the dependency direction + the demonstrated killer use case justify it over
opt-in), with the proven tiers, **but solve two pressure points first** (the early/caller-chosen
drain slot, and the inline-draining AT seam) before migrating more machines, and **scope the
replay guarantee honestly**. The "MVU as THE primary interface" framing is *earned for the
substrate + leaf + editor*; it is *conditional* for the machine tier (justified only once the
AT-seam + drain-slot refinements amortize it across Dialog/Popover).

---

## Validated — KEEP (port as-is; re-derive the rationale in the FINAL)

| Decision | Evidence | Wave |
|---|---|---|
| The substrate: `Model`/`Cmd`/`Envelope`/`enqueue`/sealed `PureEnv`/the **single ordered drain** in `buiy_core::mvu` | 6 substrate tests; no leak of structural mutation | W1 |
| **`set_if_neq` drain discipline + `MvuWorkCounters`** (THE perf rule) | idempotent fold ⇒ `models_mutated==0`, no bind cascade — proven; ~525 instr/fold (iai) | W1 |
| **`RecordMode` default-OFF** (+ lazy intent) — production pays zero | record-off vs full: 243µs vs 686µs (criterion), 216k vs 788k instr (iai) | W1/W4 |
| **Stateful-leaf tier:** existing component AS the model + one shared role-keyed reducer + **drain = sole writer** | flicker impossible by construction; 1503 + a11y-driver tests green; GPU checked | W2 |
| **Command-sourcing for the editor** (record resolved `EditCommand`/IME, re-fold); editor = **PureEnv-exempt routing leaf** | byte-identical re-fold (incl. a non-circular live-tap test); 154/154 editing suite | W3 |
| **Unified record switch + global seq + `replay_into`** (whole-UI interleaved replay) | byte-identical whole-UI session replay | W4 |
| **Machine tier** (`Model`+reducer; active-by-index, focus-return derived) deletes reconciliation | `sync_menu_dismissed` + `sync_menu_open` + ~187 LOC gone; 15/15 menu tests; clobber-free GPU | W5 |
| **`LogicalId`-keyed log** (replay-portable; never `Entity`) | cross-process re-fold works (proto-2 precedent + W3/W4) | W1/W4 |

## REFINE / REDESIGN (the FINAL does these differently — with the full-picture reason)

1. **The leaf tier wants a LIGHTER primitive with a caller-chosen EARLY drain slot**
   (`add_leaf_writer::<Component,Msg>(slot, reducer)` / `add_model_in_set`). **Two waves
   converged on this:** W2 (leaf state is read same-frame by `A11yUpdate` + app systems, so the
   W1 *late* drain makes those reads one frame stale) and W5 (the same late pin gives the menu a
   1-frame AT-tree lag). The leaf also uses **none** of `Cmd`/`Emit`/`PureEnv`/env. → The FINAL's
   leaf tier = "component + Msg + a `set_if_neq` drain at a caller-chosen set," **not** the full
   `Model`/`Envelope` surface. **This is the #1 refinement — and the prototype VALIDATED it (W6):** built `add_reducer_in_set`
   (the caller-chosen drain slot) + a `ToggleLeafSet` drain `.after(Picking).before(A11yUpdate)`;
   the a11y tree now reflects an AT-driver toggle **same-frame**, and the full headless workspace
   gate is green (1845/0). The FINAL **ports** it, and should extend the same caller-chosen slot to
   the machine tier's 1-frame AT-lag.

2. **The AT synchronous act-then-observe seam must drain inline, or the funnel must be fully in
   core.** `dispatch_action_request` (core) lowers AT set-verbs (`SetValue`, `Expand`) and reads
   back synchronously; a deferred enqueue→drain lands a frame late and breaks the in-process
   driver's contract. So a `buiy_widgets`-layered funnel **cannot** route AT set-verbs (W2 slider
   scoped out; W5 AT-`Expand` sets `aria-expanded` but does NOT open the menu — advertised but
   inert). This is **the biggest unsolved design problem** and a precondition for the
   agent-interface write-side unification (H4 signal 2). The FINAL must either (a) put the funnel
   + the convertible widgets' models fully in core, or (b) give `dispatch_action_request` an
   inline mini-drain. **Decide this before converting more widgets.**

3. **Structural ops (spawn/despawn) must be on-log or provably derivable** for replay beyond a
   fixed widget set (H8(i)/open-Q1) — W4 has a failing test characterizing the gap.

4. **Add a keyed `Subscription` primitive** (timer/IME/OS sources) for replay completeness
   (H8(ii)) — not built in the prototype.

5. **Fuller `EditLog`+`MsgLog` unification** — W4 did a shared-seq, two-log, in-process merge;
   the FINAL wants cross-process export/import (one serialized `UnifiedLog`).

6. **Un-invert `dismiss.rs`** (W5 had to reference the concrete `MenuModel`) via a generic
   funnel-dismiss hook; and **the variadic reducer macro** (D5) stays deferred until the surface
   is exercised.

## Residual gaps for the FINAL to close

- **L1 funneled hot-path — the ONE perf question the prototype did NOT answer.** Caret-blink /
  scroll are render-prep / layout-written, not funnel-routed; routing a high-frequency
  widget-internal signal through the funnel and proving `node_rebuilds==0` under `set_if_neq`
  (open-Q11) is a real re-architecture, deferred across **all** waves. The substrate-level
  no-cascade property is proven; the *end-to-end funnel-routed* measurement is the FINAL's gate.
  (The iai weak-machine pricer is now built + runnable here — `valgrind` + `iai-callgrind-runner
  0.16.1` installed — so the FINAL inherits the instrument.)
- **AT set-verb routing** (the wall above); **Dialog/Popover** machines; **PureEnv `Local` +
  `#[derive(PureEnv)]`**; **dead-letter + `catch_unwind` supervision** (native-gated);
  cross-process machine replay.
- **The iai pricer's supply chain** — `iai-callgrind` (the hw-independent weak-machine pricer,
  **dev-only**, never in the production/wasm graph) pulls **unmaintained `proc-macro-error2`** (a
  RUSTSEC unmaintained advisory — "no safe upgrade available") via `iai-callgrind-macros`, so
  `cargo deny check` ⇒ **advisories FAILED** (bans/licenses/sources OK). The FINAL must add a
  **documented dev-only `cargo deny` advisory exception**, or pick an alternative hw-independent
  perf instrument. (Surfaced by running the supply-chain gate as the last verification — green on
  every other axis.)
- **Migration ripple to secondary readers** — a concrete, illustrative cost (see below).

## Framework / system findings surfaced by RUNNING it (the prototype's whole point)

- **The host toolchain "bad RAM" scare was memory-pressure OOM, not hardware.** 62 GB RAM but
  ~40 GB in use, **0 swap**, 16-way `mold` → nondeterministic rustc/LLVM ICEs. **`CARGO_BUILD_JOBS=4`**
  gives clean builds (30 s, zero ICE). Discipline for the FINAL: cap build jobs, especially opt>0.
- **Activating the MVU chain flips latent schedule-ordering ambiguities elsewhere.** W2: turning
  on `MvuCorePlugin` perturbed the executor topo-sort and exposed a gallery editor-buffer mutator
  (`apply_intents`) that violated the `reshape_edited_editors` contract by *luck* before.
  Generalizable cost: every editor-buffer mutator must make the reshape ordering explicit.
- **The migration ripple extends past the widget to every reader/reference of its old state.**
  W5: after `Menu` → `MenuModel`, the gallery still carries **stale comments referencing the
  deleted `sync_menu_open`** (lib.rs:2515/2779/2863) and its inspector "open" readout is
  **desynced** (renders "open: false" while the menu is visibly open — caught only by *viewing the
  GPU capture*, not by the headless tests). Cosmetic here, but it is the migration-cost concern in
  miniature: MVU-ifying a widget obligates rewiring every consumer of its prior contract.
- **Record tap as a pure read-tap adds no ordering edge** (W3) — distinct from a *mutator* (W2);
  worth keeping as a design rule (taps observe, they don't perturb the schedule).
- **Entity-id-bearing layout snapshots re-bless whenever a plugin adds a resource** (W1→W5 each
  drifted the same 3, always entity-id-only) — a standing cosmetic tax; the FINAL should give
  those nodes stable test-ids.

## How the prototype answers the adversarial research gate's must-fixes

| Gate must-fix | Prototype's evidence-based answer |
|---|---|
| **Establish the killer use case** | DONE — whole-UI replay of widget-internal state (W4), scoped to the subtree. An app-boundary log cannot produce it. |
| **Measure the funneled hot path before committing** | PARTIAL — substrate measured cheap (~525 instr/fold); the end-to-end funnel-routed high-freq measurement (open-Q11) is the one deferred item and the FINAL's go/no-go gate. |
| **Price the mandatory overhead** | Substrate is `O(N_model_types)` idle + cheap-when-`RecordMode::Off`; the real added cost is the schedule stages + the per-active-frame `ApplyDeferred` — measure on weak/wasm in the FINAL. |
| **Quantify the marginal completeness over opt-in** | The killer use case IS the delta (widget-internal replay) — but it is scoped (no structural ops, AT set-verbs inert), so the FINAL must state the guarantee precisely, not as "whole-UI, unconditional." |
| **Reconcile sole-writer vs the escape hatch** | DONE (W5 L6) — a migrated machine is controllable ONLY by enqueueing a Msg; direct writes to the model or projection desync/lose. Documented boundary. |
| **Separate the cheap single-writer win from the expensive bet** | The single-writer win is real and separable (W2/W5), BUT the prototype shows the *core substrate* delivers the killer use case the single-writer refactor alone cannot. The FINAL should still stage them so each stands on its own. |

## Build strategy for the FINAL

- **Hybrid port from this shared base** (`origin/main` @ `5c0da9f`): cherry-pick / audited-port the
  KEEP work (substrate, leaf tier, editor command-sourcing, unified replay, the iai pricer); the
  prototype's commits are cleanly separable.
- **Solve the two REFINE pressure points FIRST** — the caller-chosen **early drain slot** and the
  **inline-draining AT seam** — before any further machine migration; they are what amortize the
  machine-tier cost and unblock the agent-interface unification.
- **Then** convert machines one-per-PR, each gated on the **GPU lane + the live-interaction tier**
  (the Menu anti-clobber precedent).
- **Scope the spec's replay guarantee** to the MVU-governed subtree; put structural-ops-on-log and
  the `Subscription` primitive on the roadmap as the path to a broader guarantee.
- The FINAL spec **supersedes** `docs/specs/2026-06-26-buiy-state-management-design.md`; placement
  = **core**, now evidence-backed; record the re-decides (tiered granularity, editor-exempt,
  recording-default-OFF, the early-drain-slot, the AT-seam) in its decision log.

## Provenance

- Journal: `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-journal.md` (Stage 0 + W1–W5, each
  RUN + verified by the orchestrator, not trusted from agent self-reports).
- Research synthesis (D1–D10 + risk verdicts) + the two gates: `docs/reports/2026-06-26-mvu-as-core-research/`.
- Prototype code (unmerged): `crates/buiy_core/src/mvu/`, `crates/buiy_core/src/replay.rs`,
  `crates/buiy_core/src/text/edit/record.rs`, `crates/buiy_widgets/src/{menu.rs,dismiss.rs}`,
  the W1–W5 tests, the iai pricer (`benches/mvu_iai.rs`), and the bench scenes.
- GPU evidence (viewed): `docs/reports/parity-proto-assets/{c1-shell.png, w5-menu-open.png}`.
