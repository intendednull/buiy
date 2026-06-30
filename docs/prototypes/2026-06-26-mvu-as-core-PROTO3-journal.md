# MVU-as-core (prototype-3) — Dev Journal

> **PROTOTYPE — exploratory, DO NOT MERGE.** The deliverable is this journal + the
> retrospective. The code is an unmerged reference.

**Goal:** build the full maximalist bet — MVU as Buiy's core/primary interface (recordable
substrate in `buiy_core`, widget set migrated internally) — and **run + measure** it, so the
FINAL is re-decided from evidence. See
[the build-to-learn design](2026-06-26-mvu-as-core-PROTO3-design.md).
**Worktree:** `worktree-mvu-core`, off `origin/main` @ `5c0da9f`.
**Learning goals:** L1 perf (the iai pricer + funneled hot path — go/no-go) · L2 killer use
case (whole-UI replay of widget-internal state) · L3 migration (seam vs rewrite; text-edit +
Menu) · L4 completeness delta · L5 real-user-input completeness · L6 escape-hatch trap.

## Running log

### 2026-06-26 — Stage 0: research + design (this worktree set up)
- Built: the research stage (13-agent fleet, gated) → `SYNTHESIS.md` (D1–D10 + risk verdicts)
  and the [build-to-learn design](2026-06-26-mvu-as-core-PROTO3-design.md). Both research
  gates `pass-with-fixes`; the adversarial gate's objections converted into L1–L6.
- Decided (with the user): this is a **prototype** — answer "is the maximalist bet earned?"
  by **building + measuring**, not by hedging the scope. Full bet, built to learn.
- Next: **W1** — `buiy_core::mvu` substrate + `set_if_neq` + `MvuWorkCounters` + the iai pricer;
  first L1 numbers.

### 2026-06-26 — W1: `buiy_core::mvu` substrate + change-detection discipline + perf instruments
- **Built:** `crates/buiy_core/src/mvu/mod.rs` — proto-2's runtime ported into core
  (`Model`/`Envelope`/`Cmd{None,Emit,Batch}`/`LogicalId`/`MsgLog`/`MvuSet`/`enqueue`/sealed
  `PureEnv`/`Reducer`/`MvuAppExt`/`mvu_model`) + the W1 additions: **`set_if_neq` drain
  discipline**, **`MvuWorkCounters`**, **`RecordMode{Off,Ring,Full}`** (default **Off**), and
  `MvuCorePlugin` folding `MvuSet` (Enqueue → pinned `ApplyDeferred` → Drain → Bind) into
  `BuiySet` (after `A11yUpdate`, before `Render`). Plugin is **separate from `CorePlugin`**
  (cheap-when-absent). + a criterion bench, an iai-callgrind scaffold, shared bench scenes.
- **Ran + verified (I re-ran the tests myself — did not trust the agent's self-report):** 6/6
  crosscut MVU tests green. **The L1 first-cut result:** `set_if_neq` works — an idempotent
  fold (`Add(0)`) gives `drain_folds==1` but **`models_mutated==0` AND `binds_fired==0`**: a
  no-op fold does NOT trip `Changed<M>` and does NOT cascade to the bind stage. That is the
  H3 fix (the drain defeating change-detection was the real perf killer) **proven by
  construction**. Also: idle ⇒ all counters 0; one-message ⇒ folds once + mutates + binds;
  `TickTo` Emit ⇒ run-to-completion in one drain pass (`emits_refolded==3`); `RecordMode::Off`
  records nothing, `Full` records one RON entry keyed by `LogicalId`.
- **Criterion (directional, opt-0 — see toolchain note):** idle floor ~linear in **model-type**
  count (~8.7 µs/type → `O(N_model_types)`, NOT per-instance — confirms the SCALE verdict);
  ~2 µs marginal/fold; **recording off-vs-full = 243 µs vs 686 µs** on a 100-msg storm
  (~4.4 µs/fold RON serialize) → concretely confirms H7 "default-OFF pays zero."
- **Found (environment, not code): the host toolchain is memory-pressure-unstable, not broken
  hardware.** The agent hit nondeterministic rustc/LLVM SIGSEGV/ICE across ~6 crates at all opt
  levels and suspected bad RAM. Root cause is almost certainly **OOM under 16-way parallel
  `mold`**: 62 GB RAM but ~40 GB already in use, **0 swap**, ~22 GB available. Capping
  **`CARGO_BUILD_JOBS=4`** → `buiy_core` compiled clean in 30 s, 6/6 green, zero ICE.
  → **Discipline for the rest of the prototype: build with `CARGO_BUILD_JOBS≈4`** (CLAUDE.md
  already prescribes `-j 2` for link-OOM). opt>0 builds (the iai pricer, real perf) are the
  risky ones — cap jobs there especially.
- **iai pricer (RAN — hardware-independent instruction counts):** valgrind 3.25.1 +
  `iai-callgrind-runner 0.16.1`; 9 benches, 0 regressions; the opt-3 build completed clean under
  `CARGO_BUILD_JOBS=4` (further corroborating memory-pressure, not hardware). **Per-fold marginal
  ≈ ~525 instructions** (fold-storm 1→100: 164,137 → 216,347 instr) — trivial vs the ~16M-instr
  weak-machine frame budget; folds are essentially free (the synthesis's estimate, now measured).
  **Recording Full ≈ 3.6× Off** (788,188 vs 216,345 instr) — confirms H7 at the instruction
  level. Per-model-TYPE registration ≈ ~30k instr/type (idle build, 4→16 types: 246k→605k) —
  scales with type count, not instances (SCALE verdict). **Caveat (do NOT overclaim):** these
  measure the COUNTER substrate primitive, NOT the funneled hot path (caret-blink/scroll/slider)
  — that is the **W3/W4** L1 question. W1 proves the substrate is cheap; the hot-path stays open.
  Minor: a non-fatal iai version-detection warning (pin `iai-callgrind = "=0.16.1"` to silence).
- **Surprises / deltas vs proto-2 (agent-reported):** counter reset needed its own
  `reset_mvu_counters` system in `Enqueue` (the drain early-returns on an empty inbox, so it
  can't self-reset); freshly-spawned models are `Changed` on frame 1, so tests `settle()` 3
  frames before idle asserts (the render-gate discipline); `ModelWiring::with_routing()`
  dropped (routing is W2) → an `app()` accessor instead; `register_type::<LogicalId>()` added
  for replay; corrected three iai-callgrind 0.16 API facts.
- **If we did this again:** cap build jobs from the FIRST command — the agent burned time on
  retry-loops fighting OOM before diagnosing it.
- **Next: W2** — single-writer discipline + the tiered **stateful-leaf** (Checkbox/Switch/Slider
  as drain-sole-writer); delete `sync_menu_dismissed`; **RUN the gallery** (no flicker). Plus
  run the iai pricer once the runner lands.

### 2026-06-26 — W2: the stateful-leaf tier (drain = sole writer) + the leaf-primitive learning
- **Built:** `crates/buiy_core/src/mvu/leaf.rs` — `ToggleMsg{Toggle,Set(bool)}`, `impl Model for
  A11yToggled` (it already satisfied every bound — zero new derives), one shared `toggle_reducer`,
  `register_toggle_leaf`. Rerouted `advance_toggle_on_press` to **enqueue** (not mutate) in
  `MvuSet::Enqueue`; checkbox/switch visuals reordered `.after(MvuSet::Drain)`. **Checkbox +
  Switch** converted (shared `A11yToggled`); the drain is now the **sole writer**.
- **Ran + verified (I re-ran 28 tests + regenerated the GPU screenshot + VIEWED it):** all green —
  `leaf_redundant_set_is_a_noop_no_cascade` (**`Set(current)` ⇒ `models_mutated==0`, no `Changed`
  cascade → the flicker cannot occur**), `leaf_toggle_flips_once_and_drain_is_sole_writer`, the
  a11y **in-process driver** toggle tests (AT path intact), `buiy_widgets` checkbox/switch
  press+visual, and the gallery **synthetic-click** tests (`todo_checkbox_click_toggles_completion`,
  `showcase_switch_click_toggles_it`). Full gate: 1503 `buiy_widgets`+`buiy_core` + 53 gallery,
  green. **GPU render** (`capture_shell`, regenerated by me): the completed todo paints **checked +
  line-through**, header "2 left", inspector "total 3 / remaining 2 / completed 1" — one source of
  truth drives visual + a11y + derived count, consistently. `docs/reports/parity-proto-assets/c1-shell.png`.
- **KEY LEARNING (sharpens D2 — the leaf tier wants a LIGHTER primitive, for a non-obvious
  reason):** reusing the full `Model`/`Envelope`/`Cmd`/`PureEnv` machinery for a leaf is mis-fit —
  **NOT on ceremony** (that was clean; the feared per-instance tax never appeared — the existing
  component *is* the model, one shared reducer, per-type transport) but on the **SCHEDULE SLOT**.
  W1 hard-pins the drain LATE (`MvuSet::Drain .after(A11yUpdate)`) — correct for a *machine* whose
  model feeds a later bind. But a **leaf's** state is read **same-frame** by the a11y outbound fold
  (`A11yUpdate`) and by app systems ordered `.before(A11yUpdate)` (the gallery's count/filter), so a
  late drain makes those reads **one frame stale**. The leaf wants its drain **EARLY — at the
  activation stage** (where `advance_toggle_on_press` wrote). And it uses **none** of
  `Cmd`/`Emit`/`Batch`/`PureEnv`/env (always `Cmd::none`). → The leaf tier wants
  `add_leaf_writer::<Component, Msg>(slot, reducer)`: a `set_if_neq` drain at a **caller-chosen
  set** (default the activation stage) + the `MvuWorkCounters` gate, with the `Model`/effect/env
  surface **dropped**. This is a concrete refinement to D2 for the FINAL spec.
- **Slider SCOPED OUT (evidence-backed):** `A11yValue` is written only via `dispatch_action_request`
  — the synchronous `&mut World` AT lowering seam that the in-process driver calls and reads back
  **synchronously** (act-then-observe). A deferred enqueue→drain lands the value one frame late and
  breaks that contract (synthesis open-Q6). Proper conversion needs the driver to tick or the
  dispatch to drain inline — a real redesign, deferred to W3+.
- **Ripple / friction (feeds D8/D10):** (a) **late-drain staleness** (above) — the gallery's
  `TodoMvcPlugin` chained `.before(A11yUpdate)` *because* `A11yToggled` used to be written in
  `Input`. (b) **Non-local schedule perturbation (a real generalizable cost):** activating
  `MvuCorePlugin` perturbed the executor topo-sort and surfaced a **latent** reshape-ordering
  ambiguity — `apply_intents` clears the add-field editor via `set_value` (un-shaping the buffer)
  without ordering `.before(reshape_edited_editors)`; it passed by scheduler *luck* before.
  Root-caused (stash-test: passes on base, fails with W2) + fixed by the gallery honoring the
  documented contract (`text/mod.rs:~202`). **Lesson:** turning on the late MVU chain in an app can
  flip previously-ambiguous orderings elsewhere — every editor-buffer mutator must make the reshape
  contract explicit. (c) 3 gallery layout snapshots embed `entity#NNN` IDs; `MvuCorePlugin` shifts
  allocation +3 → re-blessed (verified **entity-ID-only**; all `pos=`/`size=` byte-identical).
- **L6 hatch-trap exemplar found in the flesh:** the gallery's `toggle_all_todo_checkboxes` is a
  genuine *runtime* 2nd writer of `A11yToggled` (alongside the now-drain-only widget path) — exactly
  the migrated-widget escape-hatch trap (D9/L6). Construction-time seeds are legit initial conditions
  (Elm-flags, D9); the runtime 2nd-writer is the real trap. Deferred — it is the L6 study.
- **If we did this again:** the early-drain need would have been caught up front by listing each
  state component's **same-frame readers** before choosing the drain slot.
- **Next: W3 — the text-edit crux** (command-sourcing: `EditCommand`/IME stream + `TextEditSnapshot`;
  editor as the PureEnv-exempt routing leaf). The **drain-before-`reshape_edited_editors`** ordering
  is now **doubly flagged** (synthesis D10 + the W2 reshape ripple) — get it right first.

### 2026-06-26 — W3: the text-edit crux — command-sourcing record/replay (H5)
- **Built:** `crates/buiy_core/src/text/edit/record.rs` — the Buiy-owned `Reflect` mirror of the
  editor's *resolved* input vocabulary + the recordable log + the replay fold:
  **`MotionMirror`** (lossless `From`/`to` over all 22 cosmic `Motion` variants, incl. the
  data-carrying `LayoutCursor`/`Vertical`/`GotoLine`), **`RecordedEdit`** (the resolved
  keyboard/clipboard verbs *interleaved with* the IME sub-events in ONE ordered stream;
  `Paste(String)` carries the **resolved** clipboard text and the IME variants carry their own
  `value`/`cursor` — the impure reads hoisted into the recorded form), a parallel
  **`EditLog`** resource keyed by `LogicalId` (typed entries, reusing `mvu::RecordMode`,
  default **Off**), and **`TextEditState::apply_recorded`** (the replay fold: keyboard verbs
  lower back through the SAME `apply_tracked`; IME drives the same `splice`/`commit`/`remove`
  primitives; Paste re-feeds the recorded text via a throwaway clipboard). **Record tap** added
  at the two real apply sites — `apply_keyboard_edits` (resolved `EditCommand`) and `apply_ime`
  (each acted-on `Ime` sub-event) — as `Option<ResMut<EditLog>>` + `Query<&LogicalId>`, gated by
  `RecordMode` (zero work when Off/absent). The editor stays an **imperative routing leaf** — the
  tap only READS state + appends; it converts nothing into a reducer.
- **Ran + verified (re-ran every gate myself + viewed the GPU capture):** **CRUX PROVEN.**
  `replay_refold_is_byte_identical` and `live_tap_records_resolved_stream` both green: re-folding
  the recorded stream into a FRESH editor from the same seed + same `FontSystem` reconstructs
  **value + caret + selection byte-identically**, the cosmic `Editor` never serialized. The live
  test is non-circular — it drives the editor with RAW `KeyboardInput`/`Ime` (incl. shift-select,
  Ctrl/Cmd-V paste, IME preedit+commit), the tap records the *resolved* stream (asserted:
  `Paste("PASTE")` carries its text, `ImeCommit("world")`, `Motion(Left, extend=true)`), and
  replaying the captured `EditLog` reproduces the live editor byte-for-byte.
  `recorded_edit_reflect_ron_roundtrips` proves the vocab persists cross-process (Reflect→RON→Reflect).
  `record_mode_off_pays_zero` proves default-Off records nothing on a live editing frame.
  Full gates: **`buiy_core` 1431/1431** (the whole delicate text/editing suite, incl.
  `text_coherence_property::no_edit_sequence_leaves_the_editor_unshaped_at_frame_end` — the
  reshape-coherence proptest + the `Last`-schedule `debug_assert_shape_coherence` mirror), plus
  **gallery 53/53** and **widgets 76/76**. The **GPU shell capture** (`capture_shell`,
  regenerated + viewed) renders the editor/add-field + todo list cleanly — no unshaped-buffer
  extract crash (the proto-1 lesson held: recording is OFF by default, so W3 is behaviorally inert
  in the live app).
- **ORDERING (the doubly-flagged concern — synthesis D10 + the W2 reshape ripple):** the tap is a
  **pure read-tap, not a buffer mutator** — it adds NO new post-`TextCommit` editor-buffer
  mutation, so the `reshape_edited_editors` contract (`text/mod.rs:~202`) needs **no new edge** and
  nothing flips. Confirmed by the coherence proptest + the headless `debug_assert_shape_coherence`
  staying green. (This is the key insight that defused the flagged hazard: the W2 ripple was a
  *mutator* — `set_value` un-shaping the buffer — whereas a record tap touches no buffer.)
- **Editor-as-PureEnv-exempt-routing-leaf — held cleanly.** No strain: the editor never became a
  `Model`/reducer; `TextEditState` stays `#[derive(Component)]`-only (never Reflect). The two impure
  reads (Paste = OS clipboard; the whole fold = `&mut FontSystem`) are handled exactly as H5
  predicted — the clipboard read is hoisted into `RecordedEdit::Paste(text)`, and the FontSystem is
  the determinism *boundary* (replay locks the SAME engine: an `Arc` clone). Determinism for the
  *test* motions is even cleaner than feared: cosmic's `cursor_motion` shapes horizontal motions
  on-demand via `line_shape` (FontSystem-only, width-independent), so a widthless replay editor
  folds Left/Right/shift-select identically with no manual reshape (only `Up`/`Down` would need the
  width — avoided in the crux sequence).
- **Mirror cost (D2 answered with numbers):** the `Motion` mirror is **22 variants**, **zero-loss**
  (3 carry data — `LayoutCursor{line,layout,glyph}`, `Vertical(i32)`, `GotoLine(usize)`); the keymap
  only emits ~12 today but mirroring all 22 is the same cost and future-proofs the log. The IME
  mirror is 3 variants (`ImePreedit{value,cursor}`/`ImeCommit`/`ImeCancel`). **Only ONE command
  class needed resolved-effect recording: `Paste`** (OS clipboard inside the fold). `Cut`/`Copy`
  need none — `Cut`'s deletion is deterministic and the clipboard WRITE is outside the editor's
  state; `Copy` changes no editor state.
- **Log decision (sharpens D2/H7):** a **separate `EditLog`, NOT `mvu::MsgLog`** — justified: the
  editor is the documented PureEnv exemption and is NOT a `Model`, so it has no `Envelope<M>`/`Msg`
  for `MsgLog`; its stream is one ordered `RecordedEdit` vocab interleaving keyboard + IME. Storing
  entries **typed** (not eager-RON like `MsgLog`) actually pre-figures the better W4 design
  (`MsgLog` TODO: "typed ring, RON only at export") and pays no per-keystroke serialize cost.
  The FINAL should **unify** `EditLog` + `MsgLog` under one global seq/record-switch for whole-UI
  *interleaved* replay (deferred — the W4 capstone).
- **Surprise / friction (feeds D8/D10):** `init_resource::<EditLog>()` in `BuiyTextPlugin`
  deterministically shifts gallery entity-index allocation by **+1** (isolated by a bisect:
  init-resource is the cause, not the `register_type`s) → the SAME 3 layout snapshots W2 re-blessed
  (`modal_screen`/`overlay_menu_screen`/`shell_skeleton`) drifted again. Verified **entity-id-only**
  (every `pos=`/`size=` byte-identical across all diffs) and re-blessed via `cargo insta accept`.
  Lesson: any new plugin-provided resource re-touches the entity-id-bearing layout snapshots — a
  recurring cosmetic cost of these snapshots embedding raw `entity#NNN`.
- **Deferred (journaled, not done):** **`TextEditSnapshot`** logical projection (value + caret-with-
  affinity + selection + preedit + undo-as-`ChangeItem`s) for HOT-RELOAD — a separate concern from
  replay-from-seed (H5/D3); not needed to prove the crux, deferred. **L1 caret-blink funneled
  measurement** — largely W4 (the blink is a `write_caret_blink` render-prep writer, not yet
  funnel-routed); the cheap `set_if_neq` no-cascade property is already proven at the substrate in
  W1/W2. IME compose-over-selection + cancel edge cases beyond the one cancel exercised here.
  Pre-existing: the prototype branch's **rustdoc gate** is red on W1/W2 broken intra-doc links
  (`enqueue`, `A11yToggled` in the untracked `mvu`/`a11y` docs) — not a W3 regression; W3's new docs
  are link-clean.
- **If we did this again:** list the entity-id-bearing layout snapshots up front — every wave that
  adds a plugin resource re-blesses them, and confirming entity-id-only each time is the tax of
  raw `entity#NNN` in snapshots (a follow-up: stable test-ids for those nodes would end it).
- **Next: W4** — record/replay (default-OFF lazy ring) + the whole-UI replay killer-use-case demo
  (unify `EditLog`+`MsgLog`); **measure scroll/slider funneled (L1)**; L5 (is real input recorded?).

### 2026-06-26 — W4: whole-UI record/replay — the L2 killer use case (unify the two logs)
- **Built:** the **unification** + the **whole-UI replay capability** + the **killer-use-case
  proof**.
  - `crates/buiy_core/src/mvu/mod.rs` — **`RecordSession`** = the ONE shared record switch +
    ONE global monotonic sequence. Stripped `MsgLog`'s own `mode`/`seq` (it now holds only
    `entries`); the drain stamps every fold with `session.tick_seq()` (returns `None` ⇒ zero
    work when `Off`). Added **`ReplayRegistry`** + a per-model **replay applier** registered in
    `add_model::<M>` (deserialize a logged RON Msg → `M::Msg` via the `TypeRegistry`, write
    `Envelope<M>` to the inbox — the cross-process-capable "drain path" for replay). `MvuCorePlugin`
    inits both new resources.
  - `crates/buiy_core/src/text/edit/record.rs` — `EditLog` likewise stripped of `mode`/`seq`
    (`start`/`is_recording` deleted); `record(seq, …)` takes the **global** seq. The two taps —
    `input.rs` (`apply_keyboard_edits`) and `ime.rs` (`apply_ime`, 4 sites) — now take
    `Option<ResMut<RecordSession>>` and stamp the shared seq. `BuiyTextPlugin` inits
    `RecordSession` **idempotently** (so the text stack OR the MVU chain alone provides the switch).
  - `crates/buiy_core/src/replay.rs` (**NEW**) — `UnifiedEntry` + `unified_stream(&MsgLog, &EditLog)`
    (merge the two logs into ONE stream totally ordered by global seq — a read-side view, not a
    third store) + `replay_into(app, &MsgLog, &EditLog)` (walk in seq order; widget entries
    re-enqueue+drain via the `ReplayRegistry`; editor entries re-fold via `apply_recorded`;
    recording forced OFF; `LogicalId→Entity` resolver; dead-letter skip).
  - `examples/buiy_gallery/tests/mvu_whole_ui_replay.rs` (**NEW**) — the killer-use-case test +
    the structural-ops gap test, on a controlled scene (2 checkboxes + a switch + a seeded
    multi-line editor, each `LogicalId`-tagged) under the **REAL `BuiyPlugin`** (layout + picking +
    MVU + editor), modeled on the gallery live-interaction tier.
- **THE KILLER-USE-CASE RESULT — whole-UI replay IS BYTE-IDENTICAL (I ran it + read the values).**
  Drove a multi-step session of REAL synthetic input with recording ON — **real synthetic pointer
  clicks** (move→press→release through the live picking backend → `Pointer<Click>` → `OnPress` →
  `advance_toggle_on_press` enqueues `ToggleMsg` → the drain folds + records) **interleaved with
  raw `KeyboardInput`/`Ime`** to the focused editor (insert / arrow / shift-select / backspace /
  Ctrl-V paste / IME preedit+commit). Then **replayed the unified log into a FRESH scene app from
  the same seed** and asserted the WHOLE UI byte-identical. **Exact assertion (record == replay):**
  `WholeUi { cb_a: False, cb_b: True, switch: True, editor: EditorSnap { value: "HiZZworldc",
  caret: "Cursor { line: 0, index: 9, affinity: Before }", selection: "TextSelection { primary:
  SelectionRange { anchor: …index: 9…, active: …index: 9… }, secondary: [] }" } }` — i.e. **every
  toggle state AND the editor value + caret + selection reconstructed exactly**. This is the
  widget-**internal** completeness an app-boundary log provably cannot deliver (it never sees the
  checkbox fold or the editor command stream). The test also asserts the unification directly: **4
  widget folds + ≥6 editor commands share ONE contiguous global sequence `0..n`, genuinely
  interleaved** (a widget fold sits between editor commands), so the merge order is load-bearing.
- **The unification design — shared-seq TWO logs, NOT one `UnifiedLog` (justified).** Kept `MsgLog`
  (generic eager-RON) and `EditLog` (typed `RecordedEdit`) as separate storage; unified them by
  ONE `RecordSession` (switch + global seq) and a read-side `unified_stream` merge. **Why not one
  enum store:** (a) it would force the editor log — which is NOT a `Model` and has no `Envelope<M>`
  — into the widget-Msg shape, or RON-ify the typed editor stream (losing the per-keystroke
  zero-serialize property W3 won); (b) it would make the low layer `mvu` depend *upward* on
  `text::edit::RecordedEdit` (an inverted dependency). The shared-seq design keeps each log's
  natural form, keeps `mvu` below `text`, and still yields one totally-ordered interleaved stream
  (each log is individually seq-sorted ⇒ a trivial merge). The replay routine lives in a new
  `crate::replay` module that sits *above* both — the honest home for a capability that spans them.
- **L5 — real input IS captured + replayable, at the resolved-Msg/command level.** Pointer clicks
  enter the funnel only after core lowers them (`Pointer<Click>` → `OnPress` → `ToggleMsg`), so the
  WIDGET stream is recorded at the **resolved `ToggleMsg`** level (raw pointer events never enter
  the funnel — the design's open L5 point, now answered with a number: 4 clicks → 4 recorded
  `ToggleMsg` folds). The EDITOR stream is driven by **raw `KeyboardInput`/`Ime`** and recorded at
  the **resolved `EditCommand`/IME** level (the W3 command-sourcing tap). Both replay byte-identically.
  So: real input is captured, but at the resolved-verb boundary, not as raw OS events — which is
  exactly enough for whole-UI replay and is the right granularity (raw events would re-resolve
  non-deterministically against keymap/layout).
- **The structural-ops gap — characterized, NOT papered over (H8(i) / H6 scoped guarantee).** Chose
  option (a): the killer scene is a **FIXED widget set** (no spawn/despawn) so the core capability
  is proven cleanly, AND a second test
  (`structural_ops_are_off_log_replay_does_not_recreate_a_spawn`) records a session, then performs a
  **structural op OUTSIDE the funnel** (spawn a checkbox with `LogicalId(999)` directly) and shows:
  (i) the spawn produced **ZERO log entries** (it never entered the funnel), and (ii) replay into a
  fresh app reproduces the on-log fold (`cb_a` True) **but does NOT recreate the spawned entity**
  (no `LogicalId(999)` in the replay app). So **whole-UI replay covers**: every funneled widget
  fold + every editor command over the entities present in the seed (the MVU-governed subtree).
  **It does NOT cover**: spawn/despawn (structural ops are off-log — keyed-reconcile happens in
  systems, not as logged Msgs) and any escape-hatched raw-ECS write. This is exactly the scoped
  guarantee H6 demands ("complete + byte-identical over the MVU-governed subtree"), and the
  FINAL's open-Q1 decision (record structural ops on-log, or prove structure is a pure function of
  on-log parent state) is now backed by a failing-in-the-flesh demonstration, not a paper worry.
- **L1 hot-path — DEFERRED, precise reason.** Routing one high-frequency widget-internal signal
  (caret-blink / scroll-offset) through the funnel to measure `node_rebuilds == 0` is a real
  re-architecture (caret-blink is a `write_caret_blink` render-prep writer; scroll offset is a
  layout-written field — neither is funnel-routed today), out of W4's unification scope. The
  load-bearing `set_if_neq` no-cascade property is already proven at the substrate (W1/W2 idempotent
  fold ⇒ `models_mutated == 0`, `binds_fired == 0`) AND is exercised on REAL widget folds by this
  wave (the redundant-`Set` no-op). The remaining open-Q11 question (does a *funnel-routed* hot
  signal stay at `node_rebuilds == 0` end-to-end through extract) is a W5/FINAL measurement.
- **Ran + verified (all gates, myself):** **`cargo nextest` = 1562 passed / 71 skipped** (the GPU
  `#[ignore]` lane) across `buiy_core` + `buiy_widgets` + `buiy_gallery` — incl. the 2 new W4 tests,
  the whole W1/W2/W3 substrate + the delicate text/editing suite, and the editor record/replay
  crux. `cargo fmt` clean; `cargo clippy -p buiy_core --all-targets` + `-p buiy_gallery --tests` +
  `-p buiy_bench_support` clean; **`RUSTDOCFLAGS="-D warnings" cargo doc -p buiy_core --no-deps`
  GREEN** (the W1/W2 broken-link debt is now fixed too — explicit `crate::path` links in the
  `//!` module docs, bare links in item docs). **GPU capture regenerated + VIEWED**
  (`capture_shell` → `c1-shell.png`): the TodoMVC shell renders cleanly (completed todo checked +
  line-through, "2 left", inspector 3/2/1) — W4 is behaviorally **inert** in the live app
  (recording default-OFF), the proto-1 lesson held.
- **Surprise / friction (feeds D8/D10 — the recurring snapshot tax, now 4× confirmed):** the two
  new `MvuCorePlugin` resources (`RecordSession` + `ReplayRegistry`) again shifted gallery
  entity-index allocation (`init_resource` perturbs it — the W2/W3 finding), re-drifting the SAME 3
  entity-id-bearing layout snapshots (`modal_screen` / `overlay_menu_screen` / `shell_skeleton`).
  Verified **entity-id-only** (every `pos=`/`size=` byte-identical; only `entity#NNN` renumbered)
  and re-blessed. This is now a *certain* per-wave cost — the follow-up (stable test-ids for those
  nodes instead of raw `entity#NNN`) would end it and should be in the FINAL's hygiene list.
- **If we did this again:** the `RecordSession`-as-shared-resource design (vs threading a seq through
  both logs) was the right call and fell out cleanly — start there. The replay-applier registry is
  the one piece worth pre-deciding: storing widget Msgs as eager-RON (W1) made replay need a
  per-type RON→`Envelope<M>` dispatcher; a typed ring (`Box<dyn Reflect>`) would still need the
  per-type `Envelope<M>` write, so RON+registry is the honest cross-process path either way.
- **Deferred to W5 / the FINAL:** (1) **L1 hot-path** funneled-signal `node_rebuilds==0` measurement
  (above). (2) **Structural-ops-on-log** (H8(i) cure) — the gap is now demonstrated; the FINAL must
  pick on-log-vs-derivable. (3) **The `Subscription` primitive** (H8(ii)) — timer/OS sources still
  can't be funnel-logged (IME is, via the W3 apply-site tap). (4) **Cross-process file export** —
  both stores are Reflect/RON-capable and W3 proved `RecordedEdit` RON round-trips, but a single
  serialized `UnifiedLog` file format + import is unbuilt (the demo replays in-process). (5)
  **W5: the machine tier** (`Menu` → `Model`+reducer; agent-drive; the L6 hatch contract).

### 2026-06-26 — W5: the machine tier (`Menu` → `Model`+reducer) — L3 machine / L6 hatch
- **Built:** `crates/buiy_widgets/src/menu.rs` — the exemplar machine. **`MenuModel`**
  `{ open: bool, active: Option<usize>, dismissed: Option<DismissReason> }` (the active
  item is an **index**, NOT an `Entity`, so the recorded `MenuMsg` log stays entity-free
  + replay-portable; the focus-return target is *derived* — structurally the controlling
  button — not stored), folded by **`menu_reducer`** over **`MenuMsg`**
  `{ Open, Close(DismissReason), Toggle, Highlight(usize) }` (`Toggle` returns
  `Cmd::Emit(Open|Close)` — the W1 effect-as-value re-fold, so the press decision is one
  fold and the *resolved* verb is what lands in the log). `MenuModel` is `#[require]`d by
  `Menu` (every menu, incl. the gallery's bare-marker idiom, gets it, default closed) and
  registered via `app.mvu_model(menu_reducer)`. **One bind — `bind_menu_model`** (reacts
  to `Changed<MenuModel>`, `set_if_neq`) PROJECTS the model onto the menu `CssVisibility`
  + `active_descendant` + the button's `A11yExpanded` + focus. **Every widget-level writer
  rerouted to ENQUEUE:** press (`route_menu_press`, replacing the menu's half of
  `advance_expanded_on_press` — the menu button is now `Without<MenuButton>`-excluded from
  it), keyboard nav (Arrow/Home/End→`Highlight`, Enter/Space→`OnPress`+`Close(Activated)`,
  Escape→`Close(Escape)`), item click (→`Close(Activated)`), and the generic light-dismiss
  (`dismiss.rs` gained a menu-aware branch: a `MenuModel` overlay enqueues `Close` instead
  of writing `CssVisibility`). Updated `capture_shell` to force the menu open via the
  **model** (the projections are now bind-owned).
- **`sync_menu_dismissed` DELETED cleanly — the named D3 cure-target is gone, AND so is
  `sync_menu_open`.** The light-dismiss no longer writes the menu's `CssVisibility` behind
  the button's back (it enqueues `Close`), so the two-writers-need-a-reconciler condition
  `sync_menu_dismissed` existed for **cannot arise by construction**; `sync_menu_open` is
  replaced by the projection. Collapsed alongside: `close_menu`, `set_active`, `index_of`,
  `first_menu_item`, `menu_of_item`, the two `type` aliases (~187 LOC of lifecycle +
  reconciliation deleted).
- **Ran + verified (re-ran every gate myself + regenerated + VIEWED the GPU capture):**
  `buiy_widgets::menu` 8/8, the new `menu_machine_w5` 4/4, `buiy_verify` `menu_dismiss_c5c`
  4/4 (incl. `press_outside…dismisses…and_collapses_the_button` — the old reconciliation
  test, now passing *through the funnel*), the gallery **live-interaction** tier 17/17
  (the 3 real-shell menu flows: button-click-open → keyboard-activate, item-click-activate,
  outside-click-dismiss — all green; the disclosure test confirms `advance_expanded_on_press`
  still works after the `Without<MenuButton>` exclusion). **Full `cargo nextest` = 1562
  passed / 71 skipped** across `buiy_core`+`buiy_widgets`+`buiy_gallery`. fmt clean; clippy
  clean on `buiy_core`+`buiy_widgets`; **`RUSTDOCFLAGS="-D warnings" cargo doc` GREEN for
  `buiy_core` AND `buiy_widgets`** (fixed a pre-existing W2 `toggle_reducer` broken link).
  **GPU (RX 6700 XT):** the buiy_core **top-layer paint** `#[ignore]` goldens (the exact
  menu-dropdown descendant-fill + text-paint path) 2/2; the buiy_verify GPU lane green; and
  the regenerated **`w5-menu-open.png`** (viewed) shows the dropdown open below the ⋮ with
  all 5 items inked (incl. the danger-red Delete), the active highlight ring, correct
  panel/border — **no clobber regression** (the anti-clobber precedent held).
- **THE MIGRATION COST — candid (the adversarial gate's challenge).** Churn: ~589 lines of
  `menu.rs` rewritten (~187 deleted), +55 `dismiss.rs`, + the registration. The
  reconciliation *did* collapse (2 sync systems → 1 projection bind), a genuine
  simplification. **But the win is real-yet-MODEST and bundled with THREE new costs the
  gate predicted:**
  1. **The core AT set-verb wall (the biggest finding).** `Expand`/`Collapse` are honored in
     **`buiy_core`** (`action.rs`), writing `A11yExpanded` **directly + synchronously**
     (act-then-observe). It **cannot** enqueue a `buiy_widgets` `MenuMsg` — the crate
     dependency points the wrong way — so it does **not** fold the model: AT `Expand` sets
     `aria-expanded` but does NOT open the menu (proven in `menu_machine_w5`). This is the
     **exact same synchronous-seam wall that scoped out the slider in W2**. AT `Click`
     (which converges on `OnPress`) *does* route. So the single-writer claim is **incomplete
     for a widget whose state is also a core-owned a11y component driven by the core AT
     seam** — you keep the win for the modalities that lower through `OnPress`, and accept an
     advertised-but-now-inert absolute set-verb (no test drives the menu via Expand/Collapse,
     so nothing breaks, but it is a real a11y gap, documented not papered).
  2. **A layering inversion in the dismiss substrate.** Making light-dismiss single-writer
     forced `dismiss.rs` (the generic popover substrate) to reference the concrete
     `MenuModel`/`MenuMsg` (`enqueue` is per-model-type-generic, so the substrate can't stay
     model-agnostic *and* funnel). One branch, same crate, but the substrate now knows about
     a consumer — the FINAL wants a generic "dismiss-through-the-funnel" hook.
  3. **The late-bind 1-frame AT lag + the harness L6 trap.** The menu's state is read by
     `A11yUpdate` (→ `aria-expanded`/`aria-activedescendant`), but the D10-pinned
     `MvuSet::Bind` runs **after** `A11yUpdate`, so the AT-tree projection lags the component
     by one frame (paint is fine — `CssVisibility` is written before `Render`). This is the
     **W2 leaf finding, now confirmed for the machine tier**: D10's late drain is wrong for
     *any* model read by `A11yUpdate`; leaf and machine both want an **early, caller-chosen
     drain slot**. Separately, the `capture_shell` forced-open trick *broke* — it forced the
     projection (pre-W5), which the bind now clobbers — the **L6 trap surfacing in the
     harness**; fixed by forcing the model.
  **Honest verdict:** the gate was right — this is "where migration is most dangerous and
  the concrete win is smallest." Deleting `sync_menu_dismissed` is genuine (the machine tier
  *is* where the reconciliation lived), and replay now covers menu folds for free. But it is
  **not** free: a substrate layering inversion, an un-funnelable core AT set-verb, and a
  1-frame AT lag come with it. The machine-tier win **justifies the cost only weakly** for a
  *single* widget; it would amortize better across Dialog/Popover (which share the same
  open/dismiss/focus-return shape), and would be materially stronger if the AT seam could
  drain inline (then the set-verbs route too). Recommend the FINAL convert machines **only
  with** (a) an early/caller-chosen drain slot, (b) a generic funnel-dismiss hook, and (c)
  a decision on inline-draining the AT seam — otherwise the per-widget cost outruns the win.
- **Agent-drive demo (D2 / H4-signal-2): YES — action lowered through `update`.** With
  `RecordSession::Full`, the in-process driver's `inprocess::click(menu_button)` (the real
  AT seam) → `OnPress` → `route_menu_press` enqueues `MenuMsg::Toggle` → the drain folds it
  through `menu_reducer` → model opens, and **both the `Toggle` and its `Cmd::Emit`-resolved
  `Open` are recorded in `MsgLog`** (the agent action entered the funnel as logged messages,
  reproducibly). The companion test proves the gap: AT `Expand` does NOT fold the model.
- **L6 hatch contract (for a migrated machine):** a direct write to the **projection**
  (`A11yExpanded`) does NOT control the menu — it desyncs (`A11yExpanded==true` while
  `MenuModel.open==false`, menu stays hidden) and is reclaimed by the bind on the next model
  change; a direct write to the **model** races the sole-writer drain (a same-frame enqueued
  `Highlight` overwrites a direct `active=` write). **The documented boundary: control a
  migrated machine ONLY by enqueueing a `MenuMsg`; direct writes to `MenuModel` or its
  projections are unsupported and race the drain** (the W2/L6 trap, now shown in the flesh +
  encountered live in the capture harness).
- **Surprise / friction:** the SAME entity-id snapshot tax (the 3 `MvuCorePlugin`-resource
  snapshots drifted +1/+7 from `add_message::<Envelope<MenuModel>>`) — verified entity-id-only
  and re-blessed, EXCEPT overlay/shell also carried **one** real change: a single zero-size,
  hidden `InfoTip` tooltip child node settled to `pos=0,0` (was `0,6`) — a deterministic
  (3×-stable), non-visual settle-timing shift from the W5 systems perturbing the executor
  topo-sort (the W2 "non-local schedule perturbation", now on a tooltip-anchor settle). Blessed
  with that caveat, not under the "entity-id-only" banner.
- **If we did this again:** build the **early/caller-chosen drain slot first** (the W2 leaf
  *and* the W5 machine both need it — it's the single highest-leverage D10 refinement), and
  decide the **AT-seam draining policy** before touching the machine (the slider + menu both
  hit the same wall — it is the load-bearing blocker for "agent actions through the funnel"
  for *absolute set-verbs*, as opposed to `OnPress`-converging `Click`).
- **Deferred to the FINAL:** (1) AT `Expand`/`Collapse` routing (needs `MenuModel`-in-core
  or an inline-draining AT seam — the W2 slider blocker too); (2) the early-drain
  `add_model_in_set(slot)` to kill the AT lag; (3) a generic funnel-dismiss hook (un-invert
  `dismiss.rs`); (4) `Dialog`/`Popover` machines (W5 did only `Menu` — D8 "one per gate");
  (5) cross-process replay of the menu (the folds record + RON-round-trip, but the unified
  whole-UI replay demo, W4, did not include a machine).

### 2026-06-27 — W6: the early caller-chosen drain slot (REFINE #1 — VALIDATED) + the closing gate
- **Why:** the closing **full-workspace** gate (`cargo nextest run --workspace` — which the per-wave
  runs had not covered; it includes `buiy_verify`) caught a regression: `buiy_verify`
  `s5_driver_click_toggles_the_switch` failed — after an AT-driver click the component `A11yToggled`
  folds same-frame (line 626 ✓) but the a11y **TREE** read `False` (line 630 ✗), because `A11yUpdate`
  builds the tree BEFORE the late-pinned leaf drain. The W2 late-drain staleness in the flesh — fixed
  at root cause, NOT by patching the test's tick budget.
- **Built (REFINE #1, now VALIDATED — not just hypothesized):** the substrate primitive
  `add_reducer_in_set::<M,F>(reducer, set)` (the caller-chosen drain slot; `add_reducer` delegates to
  it with the late `MvuSet::Drain` default, so machines are byte-for-byte unchanged). The toggle-leaf
  installs its drain in a new `ToggleLeafSet{Enqueue,Drain}` =
  `(Enqueue, Drain).chain().after(BuiySet::Picking).before(BuiySet::A11yUpdate)` with a pinned
  `ApplyDeferred` between — so click→OnPress→enqueue→fold completes BEFORE the a11y tree is built.
  `reset_mvu_counters` moved `.before(Picking)`.
- **Ran + verified (I independently re-ran the FULL workspace):** `cargo nextest run --workspace` =
  **1845 passed / 0 failed** (95 GPU `#[ignore]` skipped). The a11y tree reflects an AT-driver toggle
  **same-frame** now (`s5_driver_click_toggles_the_switch` + `todomvc_c8a::driver_click_checkbox`
  green). **REFINE #1 confirmed: the early caller-chosen drain slot resolves the staleness.**
- **Second-order ripple, fixed properly (not tick-patched):** the todomvc count chain
  (`apply_filter`/`update_count`) is ALSO a same-frame `A11yToggled` reader (already failing on the
  W6-base). Naive `.after(ToggleLeafSet::Drain)` cycled (its `apply_intents` must stay
  `.before(reshape_edited_editors)`, which runs before `Picking`). Fix: **split** the chain — intents
  pre-reshape; derived chrome (filter/count/restyle) `.after(apply_intents).after(ToggleLeafSet::Drain)
  .before(A11yUpdate)`. Showcase-switch + C4 visuals → `.after(ToggleLeafSet::Drain)`. Two layout
  snapshots' hidden-tooltip node converged one frame sooner (resting value proven on base too).
- **Machine/Menu drain left LATE (unchanged)** — correct; its separate 1-frame AT-lag stays a FINAL
  item (it also wants this caller-chosen slot).
- **PROTOTYPE COMPLETE + VERIFIED.** All waves built, RUN, and the full headless workspace gate is
  green (independently re-run); **and the GPU `#[ignore]` lane (both legs, real AMD GPU) is green
  too** — `buiy_core` 24 + `buiy_verify` 22 ignored tests, 0 failed; no render regression (incl. the
  Menu anti-clobber path), so verification is airtight (headless + GPU). The [retrospective](2026-06-26-mvu-as-core-PROTO3-RETROSPECTIVE.md)
  seeds the FINAL. **DO NOT MERGE** — the code is an audited reference for the FINAL's hybrid port.

<!-- Append every wave below. For each: Built / Ran the artifact → found / Surprised by / If we did this again. RUN the GUI + the pricer — green gates are not enough. -->
