# MVU-as-core — FINAL research synthesis

**Date:** 2026-06-29
**Stage:** FINAL (production, merge-targeted) /staged-development — Stage 1 Research
**Base:** clean `origin/main` @ 4010753 (incl. WASM/WebGPU PR #85)
**Prototype reference (read-only):** worktree `mvu-core` (proto-3, audited, DO NOT MERGE)
**Seed:** `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-RETROSPECTIVE.md`

Already established (NOT re-litigated): bet VIABLE; central thesis PROVEN
(byte-identical editor replay via command-sourcing; whole-UI replay of
widget-internal state; `set_if_neq` perf cheap, ~525 instr/fold). Placement =
**CORE**. Tiers (router-leaf / stateful-leaf = drain-sole-writer / machine =
Model+reducer / raw-ECS hatch) = **KEEP**. This research resolves the FINAL's OPEN
pressure points with first-hand code evidence.

---

## DECISIONS TABLE

| # | Pressure point | Decision | Lead evidence (file:line) | Residual open-for-spec |
|---|----------------|----------|---------------------------|------------------------|
| 1 | **AT synchronous act-then-observe seam** | Adopt **(b) inline mini-drain**: extract the drain's per-msg body into one shared `fold_one_inline<M,F>` called by BOTH the batch drain and `dispatch_action_request`. **CORRECTION:** the real contract is *live-component-synchronous + perform-then-update*, NOT "visible in the same `perform()` snapshot" (snapshot reads cached build_tree views). | `inprocess.rs:387-388` (no update between dispatch+snapshot); `:328-331,:340,:259` (snapshot reads CACHED views); `a11y_inprocess.rs:462-501` (live-sync + update-then-snapshot); proto `mvu/mod.rs:607-653` (body to extract) | Env-determinism invariant unenforced (needs guard); rewrite the acceptance test (live-sync + post-update snapshot); design the `InlineActionRegistry` hook for the W5 machine-tier set-verb gap |
| 2 | **Drain-slot extended to the machine tier** (non-AT path) | **UNDECIDED — spec must decide.** Leaf folds early (`.after(Picking).before(A11yUpdate)`); RD1's inline mini-drain fixes only the AT-action read-back. The prototype menu KEEPS the late `MvuSet::Drain` `.after(A11yUpdate)`, so keyboard/pointer-driven `MenuMsg` open lags the a11y tree one frame (the W2/W5 class). | proto `mvu/mod.rs:764-766`; `widgets/lib.rs:225` ("MACHINE/Menu tier keeps the late `MvuSet::Drain`") | Move MenuModel's drain to the early slot (like the leaf) OR document why the inline seam covers the only synchronous path — reconcile against retrospective REFINE #1 |
| 3 | **L1 funnel-routed hot-path perf gate** (open-Q11) | Canonical signal = **idempotent-dominant per-frame caret-blink Tick** via a dedicated `BlinkLeaf` bench model + a net-new per-frame "moral subscription". HARD binary gate (steady frame ⇒ `models_mutated==binds_fired==node_rebuilds==0`) + soft iai ceiling (~0.03% of 16M). Commit to the **render-coupled** `node_rebuilds` test. **Do NOT migrate production caret-blink.** | `text/visual.rs:75-108` (edge-gated, not routed); `text/extract.rs:250` (`Changed<CaretVisual>`); `render/counters.rs:24-28`; `tests/crosscut/mvu.rs:139-158` (idempotent no-cascade proof) | Max simultaneous per-frame signals × per-signal cost vs 16M; pre-written "narrowed framing" fallback if the gate fails; iai dev-only deny exception is a prereq |
| 4a | **Structural-ops-on-log scope** | **ROADMAP.** Promise only **DERIVED** structure (keyed-reconcile of on-log Model collection by stable domain-id); IMPERATIVE off-Model spawn/despawn is out-of-boundary. Choose DERIVE over recording raw Entity-bearing ops. | proto `replay.rs:149` (resolver computed once), `:152-153` (silent dead-letter); `mvu_whole_ui_replay.rs:459-528` (off-funnel spawn = not recreated) | Port-fix: rebuild the resolver after structural change + make dead-letter loud/typed; DERIVED replay is asserted-not-proven (ship a keyed-list fixture or downgrade wording) |
| 4b | **Keyed Subscription scope** | **ROADMAP/DEFERRABLE for v1.** Spec the minimal Iced-validated shape now (stable-hash key + per-frame active-set diff; emissions through enqueue→drain, logged with origin tag; replay re-feeds, never re-runs); bundle with `Cmd::task`. Bake the Envelope origin tag into the v1 log format. | proto `mvu/mod.rs:113-124` (Cmd None/Emit/Batch only); `record.rs:213-222,287` (IME tapped, carries `now`); `mvu/mod.rs:150-156` (LoggedEntry no origin field) | Payload-carries-nondeterminism must be a TESTED invariant; state the v1 trigger (timer/OS drives MODEL state ⇒ Subscription required); audit Dialog/Popover machines for timer inputs |
| 5 | **Hybrid port map + base reconciliation** | 4 new MVU files **PORT-AS-IS** (wasm-clean); 5 wasm-touched files **RECONCILE** (preserve base clipboard cfg-gates); `menu.rs` + gallery **REDESIGN** onto `GalleryPlugin`. | base `text/edit/mod.rs:34-35`, `text/mod.rs:304-314` (cfg-gates to preserve); `examples/buiy_gallery/src/lib.rs:130-152` (GalleryPlugin) | Gallery W6 chain-split re-derive inside `TodoMvcPlugin` + re-verify `gallery_web`; menu mis-merge could resurrect a 2nd `A11yExpanded` writer |
| 6 | **cargo-deny dev-only advisory exception** | Add **TWO** ignores (not one): RUSTSEC-2026-0173 (proc-macro-error2) **AND** RUSTSEC-2025-0141 (bincode 1.3.3). Optionally cfg-gate the iai dev-dep out of the wasm graph (do both). | cargo-deny 0.19.4 on proto: both advisories via iai-callgrind; base `deny.toml:45-52` (only RUSTSEC-2024-0436 today) | **Pre-existing base failure** RUSTSEC-2026-0192 (ttf-parser via bevy_winit) blocks the deny gate independent of the port — own commit, must be ASSIGNED |
| 7 | **WASM-cleanliness of the substrate** | **CONFIRMED CLEAN.** Zero `thread`/`Instant`/`SystemTime`/`rayon` in the 4 MVU files; primitives all wasm-safe; `now` from bevy `Time::elapsed()`. Port introduces ZERO new wasm obstacle. | base `input.rs:666` (`Time::elapsed`), `clipboard.rs:143` (MemClipboard not gated); grep = NONE | Reconciliation must preserve base wasm cfg-gates (don't regress to unconditional `ArboardClipboard` re-export) |
| 8 | **Migration ledger / secondary readers** | Three tiers: LEAF readers untouched (writers reroute + schedule ripple); MENU deletes 2 sync systems + projects via bind; EDITOR additive (zero rewire). **Must-fix:** close the unrerouted `toggle_all_rows` runtime multi-writer. | base `lib.rs:1312-1313` (`t.0 = next` unrerouted); `inspector.rs:725` (Menu "open" hardcoded "false") | Headless-invisible inspector desync needs a live-interaction test; at-spawn seeds + `set_value` editor seeds replay status; ~20 menu test sites for bind-lag |
| 9 | **Dismiss un-invert** | **Spec decision, not mechanical.** Port the one-branch `With<MenuModel>` coupling as a stopgap, OR design a generic dismiss-through-the-funnel hook so the role-agnostic substrate stays model-agnostic. | base `dismiss.rs:76-77` (role-agnostic `close_overlay` → `CssVisibility::Hidden`); proto dismiss.rs diff (the coupling + its own smell note) | The generic hook design surface (dyn dismiss trait / per-overlay close-Msg registration) is unspecified — design it or log a follow-up |

---

## Per-dimension detail

Full findings in the sibling files:

- **RD1 — AT seam** → `RD1-at-seam.md` (carries the load-bearing CORRECTION)
- **RD2 — perf gate** → `RD2-perf-gate.md`
- **RD3 — replay scope** → `RD3-replay-scope.md`
- **RD4 — port map / deny / wasm** → `RD4-port-map.md`
- **RD5 — migration ledger** → `RD5-migration-ledger.md`

### RD1 highlight — the one factual correction

The original RD1 finding claimed `(b)` "restores synchronous read-back" with a
slider AT Increment "visible in the SAME `perform()` snapshot." **That is false**
for every `build_tree`-projected field (value/expand/text). `perform` snapshots
through the CACHED `A11yTreeBuilder` views (`inprocess.rs:328-331`), refreshed only
during `app.update()`; only `focus` is live (`:336-339`). The existing test
`driver_increment_on_slider_raises_now_by_step` (`a11y_inprocess.rs:462-501`) reads
the **live** `A11yValue.now == 35` synchronously (lines 489-494) but **requires
`app.update()`** before the snapshot assertion — the "perform-then-update contract."

**This does NOT overturn the (b) choice.** There is a genuine synchronous
requirement — the *live component* must mutate at dispatch-return, which an
enqueue→batch-drain seam would defer. `fold_one_inline` satisfies it AND closes L5
(records the Msg). The spec must (1) replace the false rationale with the real
live-sync + perform-then-update contract, and (2) rewrite the acceptance test.

---

## HONEST gaps (completeness critic + this synthesis)

Two pressure points are **PARTIAL**, surfaced as `open_for_spec` rather than papered
over:

1. **Machine-tier drain slot (decision #2) is genuinely undecided.** RD1's inline
   mini-drain covers the AT-action read-back only. The prototype menu's regular
   batch drain stays late (`.after(A11yUpdate)`), so the non-AT keyboard/pointer
   `MenuMsg` path retains the one-frame lag. The retrospective REFINE #1 literal ask
   ("extend the early caller-chosen slot to the machine tier") is unreconciled with
   RD1's inline answer — **the SPEC must decide**, not the research.

2. **Generic dismiss-through-the-funnel hook (decision #9) is not designed.** RD4/RD5
   identify the coupling and recommend a generic hook but present only the
   one-branch `With<MenuModel>` stopgap. The generic mechanism is net-new design
   surface for the spec.

Three further must-fixes the research elevates above "port action":

3. **Pre-existing base deny failure** (RUSTSEC-2026-0192, ttf-parser via bevy_winit)
   — the FINAL base ALREADY fails `cargo deny check advisories` independent of the
   port. Own commit, must be assigned, or the deny gate stays red.

4. **`toggle_all_rows` runtime multi-writer** left unrerouted in the prototype
   (`lib.rs:1312-1313`) — contradicts the "single-writer proven" headline; the FINAL
   must close it.

5. **DERIVED-structure replay is asserted-not-proven** — no list-machine fixture
   exists. Either ship a minimal keyed-list fixture + the resolver-rebuild fix +
   loud dead-letter, or downgrade the guarantee wording.

**Coverage shortfall:** none (all 9 pressure points + the 8 named dimensions
covered; the orchestrator reported no missing dimensions).

---

## Carry-forward to the SPEC

### A. Decision log (paste into spec §0)

The 9 rows of the DECISIONS TABLE above are the decision log. Decisions #2 and #9
are explicitly OPEN for the spec to resolve; #1's rationale carries the RD1
correction; #6 requires BOTH advisory ignores; #3 commits to the render-coupled
`node_rebuilds` test + a pre-written fallback paragraph.

### B. Scoped replay-guarantee statement (verbatim, ready to paste)

> Buiy records and replays the MVU-governed subtree, not the whole world. With
> recording on, every message folded through the single ordered drain — widget
> activation/value/expand folds and the editor's resolved EditCommand/IME stream —
> is logged against its stable LogicalId in one global sequence. Replaying that log
> into a fresh app built from the SAME seed scene reproduces every funneled
> widget-internal state (toggle/value/expand, focus transitions, and the editor's
> buffer + caret + selection) BYTE-IDENTICALLY. The guarantee is scoped and
> conditional, not unconditional whole-UI: (a) it covers entities present in the
> seed plus structure that is a deterministic keyed-reconcile of on-log Model state;
> imperative spawn/despawn performed outside the funnel is off-log and is NOT
> reconstructed; (b) state written by escape-hatched raw-ECS systems (entities with
> no Model, direct component writes) is outside the boundary and is reconstructed
> only to its seed value; (c) replay re-feeds logged effect/subscription results and
> never re-runs effects or re-subscribes, so nondeterministic input (time, OS
> clipboard, async results) is reproduced only insofar as it was captured as a
> logged Msg payload. A debug-build write-outside-the-funnel auditor makes the
> boundary detectable rather than silent.

### C. Port map (the sequence the plan executes)

1. **deny.toml** — add the two iai advisory ignores; SEPARATE commit for the
   pre-existing ttf-parser failure.
2. **Substrate** — port `mvu/mod.rs`, `mvu/leaf.rs`, `replay.rs` verbatim; wire
   `buiy_core/lib.rs` + `buiy_bench_support/lib.rs`; add `ron` + `iai-callgrind`
   workspace deps + benches. Extract `fold_one_inline` (RD1).
3. **Editor** — port `record.rs` + re-apply the `input.rs`/`ime.rs` record-tap hunks;
   reconcile `text/mod.rs` + `text/edit/mod.rs` PRESERVING wasm cfg-gates.
4. **Leaf** — reroute `advance_toggle_on_press` + `toggle_all_rows` (must-fix) +
   decide the 3 seeds; port the schedule split.
5. **Menu machine** — REDESIGN `menu.rs` (MenuModel/reducer/bind/route); delete
   `sync_menu_open`/`sync_menu_dismissed`; add the `InlineActionRegistry` hook (RD1
   W5 gap); resolve the dismiss coupling (decision #9); fix the inspector desync +
   add a live-interaction test; decide the machine drain slot (decision #2).
6. **Gallery** — re-apply the W6 chain-split inside `GalleryPlugin`/`TodoMvcPlugin`;
   reconcile `mvu_whole_ui_replay.rs`; re-verify `gallery_web` (wasm).
7. **Perf gate** — add the `BlinkLeaf` fixture + `mvu_blink_cadence` iai bench +
   `blink_funneled_node_rebuilds_zero` render-coupled crosscut.
8. **Verify** — full headless gate + GPU lane + `cargo deny check` (green incl. the
   ignores) + a wasm `gallery_web` build.

### D. Go/No-Go perf gate (decision #3, exact)

- **HARD BINARY:** on every steady (non-flip) caret-blink frame routed through the
  real funnel, `models_mutated == 0 && binds_fired == 0 && node_rebuilds == 0`
  (EXACT integers); on the flip frame each `== 1`. Any steady-frame nonzero ⇒
  audit #6 regression ⇒ the maximalist framing dies (fall to the narrowed framing).
- **SOFT iai ceiling:** one steady blink tick's funnel fixed cost ≤ ~5K instr
  (≈ 0.03% of the ~16M weak-machine frame budget); the captured Ir is the
  per-frame-routed-signal fixed cost that bounds how many such signals fit per
  weak/wasm-single-threaded frame.
- **Fallback paragraph (pre-written):** if the gate fails, the framing narrows to
  *"the funnel governs input-sourced state; timer/animation render-prep signals stay
  out (caret-blink stays the edge-gated `write_caret_blink`)."*
