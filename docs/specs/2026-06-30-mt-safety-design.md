# MT-safety: correct under Bevy's `multi_threaded` executor — design

Date: 2026-06-30
Status: proposed (human-review-gated)
Prototype: `worktree-mt-experiment` (off `4010753`) — DO NOT MERGE; see job journal.

## Problem

Buiy builds Bevy with `default-features = false` and **without** `multi_threaded`, so
Buiy's own builds run the single-threaded ECS executor. But **downstream consumers
re-enable `multi_threaded` via Cargo feature unification**: a stock `bevy = "0.19"`
dep pulls `default → 2d/3d/ui → default_platform → multi_threaded`, and unification
turns it ON for the whole graph including Buiy. Buiy never forces the executor kind, so
**Buiy's systems already run under the multithreaded executor in typical consumer apps**
— a config Buiy has never validated. Empirically (prototype) the workspace is very close
to MT-safe, but has real bugs there.

Goal: make Buiy **correct under the MT executor** (good-library-citizen posture) while
**keeping single-threaded the default** for Buiy's own builds. NOT a goal: making Buiy
faster via threads (the prototype measured MT 10–45% *slower* for Buiy's serial,
`NonSend`-pinned hot path; wasm is hard single-threaded; the perf gates are serial-work
metrics). MT is native-only headroom we are NOT chasing here — only *safety*.

## Decisions

**D1 — Single-threaded default; opt-in `multi_threaded` feature passthrough.**
Do NOT add `multi_threaded` to Buiy's default bevy features. Add an OFF-by-default
feature `multi_threaded = ["bevy/multi_threaded"]` on `buiy_core` (and re-exported by the
`buiy` umbrella) so a CI lane and consumers can build/test Buiy under MT explicitly.
Rationale: preserves the single-threaded default (wasm floor, gate determinism, perf) AND
makes MT a first-class, testable configuration. Rejected: flipping it on by default (the
prototype did this to learn — slower, breaks wasm posture, taxes gates); leaving no
feature (then the MT config consumers ship is untestable in CI).

**D2 — a11y `push_tree_updates` pinned to the main thread.** Add `_main_thread:
NonSendMarker`. `ACCESS_KIT_ADAPTERS` is a bevy_winit main-thread-owned thread-local; a
param-less system can be scheduled on a worker thread where the TLS is empty → AccessKit
updates silently dropped. The marker is the sanctioned pin (same mechanism as
`NonSend<LayoutTree>`). Real bug; no existing test covers it (see D8 verification).

**D3 — Editor shape-coherence: unconditional terminal reshape pass.** Add a second,
unconditional `reshape_edited_editors` registration in `Last` (before the
`debug_assert_shape_coherence`), keeping the existing after-`Input` registration as the
caret-fresh optimization. Fixes the whole class of "post-Input editor-buffer mutator left
the buffer unshaped" — including app-level mutators outside `text/**` (the gallery
`apply_intents` todo-clear that fails under MT) AND the release-build extract no-paint.
Rejected: an `EditorBufferMutate` SystemSet that every mutator must join — requires opt-in
discipline, the exact thing whose absence caused the bug; a per-system `.before` edge —
whack-a-mole + exposes internals. The terminal pass is lock-free / no-op on coherent frames.

**D4 — dhat alloc-budget gate scoped to single-threaded (`#[cfg(not(multi_threaded))]`).**
The MT executor heap-allocates per frame for task dispatch, inflating the process-wide dhat
measurement (155 idle vs budget 50) — NOT a Buiy regression. This gate is a *single-threaded
deterministic* measurement (dhat `testing()` mode) of Buiy's per-frame allocation contract,
which is identical under either executor (Buiy's own allocations don't change with the
executor). The gate runs every CI run via the default `test` job; the MT CI lane proves
CORRECTNESS, not allocation budgets, so it compiles out this binary.

*Re-decided from the spec's first draft (prototype-first re-decision on evidence):* the
original plan pinned `PipelineHarness` schedules to `SingleThreadedExecutor`. Empirically
that **leaks** — it cut idle 155→69 but couldn't reach the 33-block baseline, because not
every per-frame schedule (`Main` orchestration / `App::finish`-added schedules) is reachable
from the harness constructor's pin loop. A tight per-frame budget can't absorb residual
executor noise, so chasing the pin is fragile. Scoping the gate is robust and honest.
Rejected: raising the budget (hides real regressions under executor noise); the leaky
harness pin (above). Trade-off accepted: Buiy's allocation contract is not *re-verified*
under MT — acceptable because it is executor-independent and fully verified single-threaded
every CI run.

**D5 — H1 Theme writer ordering.** Put `apply_set_accent` and `apply_forced_colors_theme`
in one ordered set with `apply_set_accent.before(apply_forced_colors_theme)`, so a
coinciding accent change folds into the theme that forced-colors saves/restores. Today
`apply_set_accent` is bare-registered with no order edge → same-frame coincidence yields an
order-dependent persisted theme (possible high-contrast WCAG violation). Real wrong-result.

**D6 — H2 FocusedEntity writer ordering.** All `ResMut<FocusedEntity>` movers (core
`handle_tab`/`route_action_requests` + widget `sync_menu_open`/`apply_dialog_modal_state`/
`resolve_pending_focus`) join a shared `FocusWrite` ordering set so same-frame
nav-vs-overlay focus is deterministic. Cross-crate (buiy_core defines the set; buiy_widgets
joins it).

**D7 — H4/H5 cheap ordering edges.** Order `slider_keyboard` vs the keyboard readers; order
the dialog close handlers `.after(open_dialog_on_invoker_press)`. Low/no-current-trigger but
trivial and removes the ambiguity.

**D8 — Verification / proof.** (a) CI lane: a job building + running the headless suite with
`--features multi_threaded`. (b) Enable `ScheduleBuildSettings::ambiguity_detection` over the
app schedules in a test, as an OBJECTIVE check that no unordered-conflicting system pairs
remain (stronger than manual audit). (c) a11y (D2): add a test asserting the system is
main-thread-pinned (or document the structural guarantee) — closes the one coverage gap the
suite leaves. Prove: headless suite green BOTH default (single-threaded) AND `--features
multi_threaded`.

**D9 — Out of scope / filed (surfaced, not fixed here).** H6 (render dirty-bit duplication —
latent, correct today), H7 (`clear_warned_once_on_exit` dead code — fix when plan D7 wires
the exit lifecycle), **H8 (placeholder `FontsGeneration` staleness — a separate
single-threaded bug, NOT MT)**. Recorded in `docs/reports/` follow-ups. Minor: correct the
stale `font_system.rs` "exactly three lock sites" doc.

## Implementation disposition (this PR vs deferred)

The user's selected scope was "fix a11y, audit, add a CI lane, prove." Delivered in this PR:
- **D1 / D1b** feature passthrough (`buiy_core` + `buiy` umbrella) — landed.
- **D2** a11y `NonSendMarker` pin **+ a `!Send` regression test** (`adapter.rs` `tests`) — landed.
- **D3** terminal `Last` reshape pass — landed; **proven**: the gallery editor-clear test that
  failed under MT now passes.
- **D4** dhat gate scoped single-threaded — landed; **proven** both ways (runs+passes default,
  compiles out under MT).
- **D8** CI `test-multi-threaded` lane (headless suite under `--features multi_threaded`) — landed.
- **D9** doc-comment correction (`font_system.rs`) — landed; H6/H7/H8 filed in
  `docs/reports/2026-06-30-mt-safety-followups.md`.

**Surfaced with designed fixes, DEFERRED to the human review gate** (real but *latent* — no test
triggers them, so they don't block the proven-green suite; folding them in would balloon an
autonomous PR beyond the selected scope):
- **D5 (H1 Theme)**, **D6 (H2 FocusedEntity)** — real same-frame wrong-results; fixes designed
  above. Recommend folding in or a fast follow-up — reviewer's call.
- **D7 (H4/H5)** — low / no current trigger.
- **Ambiguity-detection gate** (D8 idea): NOT added as a permanent CI gate — Bevy's detector
  flags many *benign* unordered pairs, so a zero-assert gate needs a high-maintenance allowlist
  and would currently red on the deferred H1/H2/H4/H5. Recommended as a one-shot audit tool, not
  a gate. The `test-multi-threaded` lane is the concrete CI gate instead.

## Non-goals
- No threads dependency enters the production graph (the feature only flips bevy's flag).
- No perf claims; no change to the default single-threaded behavior or the wasm posture.
- Not fixing every ordering ambiguity Bevy's detector would flag — only the real hazards, and
  the latent ones are surfaced for a scoped decision rather than swept in unreviewed.
