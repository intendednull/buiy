# Dooduel QA playtest cycles — campaign journal

**Date:** 2026-07-09
**Status:** active
**Branch:** `feat/dooduel-multiplayer-m1` (worktree `dooduel-app2`)

> Living journal of the Dooduel QA playtest-cycles campaign. Like the PROTO1
> journal, the learning captured here is a first-class deliverable: the campaign's
> retrospective should decide whether a reusable **qa-playtest-cycles skill** falls
> out of it.

## Charter

Multi-agent quality-assurance playtests of the Dooduel app as it stands on this
branch (M1 networked multiplayer + the multi-page atlas fix):

- **All seats are LLM agents.** Each agent visually sees the app (screenshots)
  and interacts only through **supported buiy/app interaction surfaces** — no
  out-of-band pokes at internals.
- **What they report:** visual bugs, mechanical/game-logic bugs, UX friction —
  how the game *feels*, how it flows, anything confusing — not just hard defects.
- **Cycle loop:** playtest → triage findings → fix real issues via a full
  `/staged-development` cycle each → verify → next playtest. Repeat until a cycle
  reports no new issues, or at least 5 cycles.
- **Meta-deliverable:** journal the process, reflect at the end, and distill a
  reusable skill for agent-driven QA cycles if the shape proves general.

Standing constraints: nothing pushed/PR'd/merged without the user's explicit go
(local commits are fine); fixes are executed by subagents, gated per
staged-development.

## Method skeleton (evolves as we learn)

1. **Research** — playtest archaeology (how the 2026-07-04 acceptance playtest
   worked), current app/M1 state, the agent eyes-and-hands surface
   (`apps/dooduel_mcp` + probe), and a QA charter (dimensions, severity taxonomy,
   report template, exit criterion).
2. **Harness setup** — a repeatable "spin up a match with N agent seats" recipe +
   per-seat playtester briefing template.
3. **Cycles** — run, collect structured findings, dedupe/triage against known
   issues, fix (staged-development per real issue or batched), re-run.
4. **Retrospective → skill.**

## Running log

### 2026-07-09 — campaign start

- Environment: branch `feat/dooduel-multiplayer-m1` clean at `fd7dc9e` (atlas-bind
  cycle complete, unpushed). App lives in `apps/{dooduel,dooduel_core,dooduel_server,dooduel_mcp}`.
  The PROTO1 playtest evidence + the 2026-07-04 acceptance playtest report are
  in-repo; the `dooduel-proto1` worktree still exists for archaeology.
- Noted up front: the M1 plan **retired the file-protocol `playtest_host`** in
  favor of `dooduel_mcp` (hand-rolled stdio MCP headless client) — the QA harness
  should ride the supported MCP surface, which is itself M1 deliverable code under
  test (dogfooding the agent interface is part of the QA).
- Launched the 4-agent research fleet (playtest-archaeology, app-state,
  interaction-surface, qa-charter) under the reliable-agent-fleet contract;
  reports land in the job tmp dir, digests come back structured.

### 2026-07-09 — research returned (4/4), harness shape decided

- **Archaeology:** THREE prior multi-agent playtests over two harness
  generations (PROTO1 + 2026-07-04 acceptance on the retired file-protocol
  `playtest_host`; the 2026-07-05/06 M1 acceptance on this branch's real
  networked stack — human GUI host + 3 `dooduel_mcp` seats via `seat_driver.py`).
  Hard-won lessons now baked into our protocol: seat agents need persistent
  FOREGROUND poll loops with explicit timeouts (background wakes arrive batched —
  cost seat 3 three of four turns); widen phase timers for agent play; the exact
  seat briefing prompts were never preserved (this time they are deliverables);
  `draw_stroke` rejects float coords; room-code O/0 ambiguity is a live trap.
- **The gap vs this campaign's bar:** in every prior run the seat agents NEVER
  saw the rendered GUI — they played from canvas.png + honest text views; the
  visual bugs (empty chat pills, stuck scoreboard) were caught only by human
  screenshots. This campaign requires agents that SEE the app and CLICK it.
- **DECISION — per-seat GUI agent driver:** a new dev-tool binary running the
  REAL dooduel client (real view/MVU/NetPlugin vs a real `dooduel_server`),
  rendering offscreen (camera → RenderTarget::Image → PNG readback, the
  `capture.rs` pattern) with `BuiyProbePlugin` + the a11y in-process driver for
  semantic-tree "eyes" (`ui.md`) and supported typed verbs + synthetic pointer
  for "hands" (`gui_networked.rs` + `buiy_verify` pointer-harness patterns),
  bridged to the agent's file world like `seat_driver.py` (screen.png + ui.md
  out, commands.jsonl in). ALL seats are agents — the host seat creates/starts
  the room by clicking the real UI, which closes the "MCP is join-only, no
  human-free launcher" gap with zero launcher code and puts the Create/Lobby
  flow under QA every cycle. Rejected: dooduel_mcp-only eyes (fails visual QA);
  OS-window capture + xdotool (winit synthetic-input risk, focus contention
  across concurrently-typing seats, pixel-coordinate fragility); web build +
  browser automation (heavier, web-specific quirks — deferred as a possible
  later web-lane cycle).
- **Charter highlights (full report in job tmp):** 5 QA dimensions (visual vs
  the reference bundle — the two PNGs are stale pre-rebrand, the HTML+tokens are
  ground truth; mechanics vs the pinned scoring/hint/normalization formulas;
  UX/confusion; a 13-probe robustness matrix with spec-pinned outcomes; feel),
  S1–S5 severity (pre-reveal word leak = automatic S1; regression of a RESOLVED
  item = S2 minimum), YAML finding template with known-issue dedupe +
  suspected-layer, 4 seat archetypes (host/visual-auditor, mechanic-auditor,
  chaos, naive first-timer) rotated per cycle, cycle-clean = zero new S1–S3 +
  zero regressions + probes pass, exit at first clean cycle or ≥5.
- No single known-issues list exists (findings scattered across follow-ups.md,
  the 2026-07-04 report ledgers, M1 spec §11 quirks, retro §6) — consolidation
  dispatched as a prerequisite so playtesters don't re-report accepted quirks.
  Also noted: M1 plan W6.2/W6.3 (formal acceptance report) never closed; this
  campaign's cycle 1 can double as that evidence — flag to the user at close-out.
- Dispatched in parallel: the harness spec draft (to be reviewer-gated per
  staged-development before any implementation) + the known-issues consolidation.

### 2026-07-09 — known-issues list landed (code-verified)

- `2026-07-09-dooduel-qa-cycles/known-issues.md`: 2 regression-watch (scoreboard
  fix @ 9eb2085, chat-pills atlas fix), 7 known-open, 15 accepted-by-design,
  M2–M6 scope table. The four ambiguous 2026-07-04 ledger items were verified
  against M1 code: AFK-drawer early-out + turn-N-of-M label genuinely OPEN
  (KI-07/KI-08); the Reveal→Continue "unevenness" and drawer-side wrong-guess
  "lag" are NOT reproducible in the M1 authority — both were artifacts of the
  retired file-protocol harness (single any-seat Continue rule at
  session.rs:535-540; wrong guesses broadcast Recipient::All same-tick at
  session.rs:811-819). Playtesters re-verify those live but don't presume broken.
- **Skill note:** "verify the known-issues ledger against current code before
  suppressing" earned its keep immediately — 2 of 4 carried-forward complaints
  would have been suppressed as open bugs when they're actually fixed-by-
  construction (harness artifacts), and re-reporting them would have wasted a
  fix cycle.

### 2026-07-10 — seat-driver spec gated + approved (rev-2, active)

- `docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md` drafted, reviewed by
  two fresh-context reviewers (feasibility-adversarial + design/right-sizing) —
  both APPROVE-WITH-FIXES, no blockers; 12 consolidated fixes folded as rev-2
  and re-verified against code by the drafter; status flipped to active.
- The gate caught three would-have-shipped defects by reading code: the Join
  flow was mis-specified (the Home button only navigates; the real CTA is
  "Join room" on the Join screen — the scripted smoke would have NotFound'd),
  N seat processes would have raced ONE `~/.config/dooduel/state.json` and
  clobbered the developer's real profile (fix: per-seat `DOODUEL_STATE_DIR`),
  and the plugin composition needed `BuiyHeadlessPlugin` + explicit picking
  re-adds (full `BuiyPlugin` drags winit-coupled AccessKit/pointer plugins).
- The feasibility reviewer STRENGTHENED the design: traced the two-camera
  no-panic path through bevy 0.19 source (surfaceless window never extracted;
  targetless view never iterated by `buiy_pass`), downgrading C1 from "unproven
  assumption" to "well-supported; smoke confirms".
- **Skill notes:** (1) reviewers who verify *names/labels* against view code
  catch a class the architecture review misses (Join-flow, brush-size labels,
  state-dependent toggle names) — playtest-harness specs need a "label audit".
  (2) Both reviewers idled without delivering; the SendMessage nudge recovered
  both verdicts — bake "expect idle-without-report, ping for the deliverable"
  into the orchestration runbook. (3) The drafter-folds-fixes pattern (reviewers
  → consolidated list → original drafter revises with held context) produced a
  clean rev-2 in one pass.

### 2026-07-10 — briefings + runbook drafted; plan under review

- Implementation plan drafted (4 waves / 7 tasks, risk front-loaded: W0
  checkpoint-1 spike needs NO server — Create→Lobby is a synchronous reducer
  transition; W2 smoke locates binaries via current_exe() arithmetic since
  CARGO_BIN_EXE doesn't cross packages or cover examples). Fresh plan reviewer
  dispatched; drafter holding for the fixes round.
- Briefings + runbook landed (7 files): COMMON (protocol + the `say()` bash ack
  helper that operationalizes "foreground poll with explicit timeouts" as
  copy-paste code), 4 archetype supplements, finding template, RUNBOOK. Spot-
  check vs spec: verbs/names exact.
- **Load-bearing briefing decisions:** (1) QA timers widened to draw 240 /
  pick 120 / reveal 60 (supersedes the charter's 150/30/12 — the GUI driver is
  slower than text seats: each drawing is many acked toolbar round-trips);
  recalibrate from cycle-1 experience. (2) Probes split along the honesty
  contract — seats get only self-reachable probes; the orchestrator owns
  cross-seat canvas byte-diffs + process-control probes (the charter had
  assigned seats checks they physically can't honor without reading other
  seat dirs). (3) The naive-first-timer seat stays deliberately under-briefed.
- KI-16 amended: the podium renders a visible-but-inert "Play again" in
  networked play (podium.rs:76) — suppressed as a false-positive "dead button".
- **Skill note:** briefing-drafting surfaced charter/spec inconsistencies
  (timer values, hint-schedule examples pinned to a specific timer config) —
  a "recompute worked examples against the actual run config" step belongs in
  the eventual skill.

### 2026-07-10 — plan gated (BLOCK → rev-2), ready to build

- Fresh plan reviewer returned **BLOCK**: the W0 spike's "no server needed —
  Create→Lobby is synchronous" premise was FALSE — its own proof cited
  `capture.rs`, which runs `dooduel::install` (no networking), while the driver
  runs `install_runtime`: CreateRoom → NetState::Joining → WsClientPlugin
  connects → ECONNREFUSED → bounced back to Home with a toast. The spike would
  have failed against a WORKING C1 and its diagnostic mis-attributed the
  failure to the render risk. Fix: retarget the spike click to "Join a room"
  (GoJoin — pure reducer navigation). Also caught: the W2 "PNG changed" ink
  proxy is confounded by the ~1 Hz countdown repaint (fix: crop the header
  rows before diffing) + 4 minors (temp-root leak, missing doc gate,
  consumed-K blank-line desync → spec rev-2.1 clarification, a hard artifact
  gate for the W1→W2 parser calibration).
- Drafter's fold flagged one grounded deviation: the reviewer's crop-via-ui.md
  mechanism is impossible — the game canvas is a role-less raster, absent from
  both the role tree and the text section (reconcile.rs Kind::Raster;
  report.rs `(None,None) => continue`) — substituted a header-row crop.
  Verified both citations myself before accepting.
- **Skill notes:** (1) "proof by a test that runs a DIFFERENT composition" is
  a recurring trap — a cited precedent must run the same plugin/config set as
  the thing being specced. (2) A reviewer's diagnosis can be right while its
  fix mechanism rests on a wrong assumption — the folder must re-verify FIXES
  against code too, not just apply them. (3) Assertion proxies (PNG changed)
  need a confound audit: what ELSE changes the observed artifact on its own?

### 2026-07-10 — W0 spike PASSED: C1 retired on real hardware

- Commit `0704f62`: apps/dooduel/examples/qa_seat.rs (304 lines, plan skeleton
  near-verbatim). All gates green (build/run/fmt/clippy/headless tests) and I
  verified the artifacts first-hand: screen.png = the real rendered Join
  screen at 1280×800 (offscreen readback), ui.md = the real role tree + text
  section, driver.log = `consumed: 0 → click Join a room → Ok (Home → Join)`.
  The novel composition (headless RenderPlugin + picking + two cameras +
  install_runtime) works — **the harness's one residual design risk is
  retired**; none of the diagnostic-ladder failure branches fired.
- Dark-theme boot mystery RESOLVED as a harness-stage artifact, not a bug:
  W0 doesn't set DOODUEL_STATE_DIR yet (W1 does), so it READ the developer's
  real ~/.config/dooduel profile (theme=dark). Read-only — no writes. A fresh
  state dir boots light (the design default). Watch for this in cycle 1
  anyway: fresh seat dirs must boot LIGHT.
- **Skill note:** the orchestrator verifying artifacts first-hand (reading the
  actual PNG, not trusting "it passed") + the implementer's decisive-line-per-
  gate report format is a cheap, strong wave gate. Also: implementers keep
  idling without reporting — the ping-for-report step is now routine.

### 2026-07-10 — W1 interrupted by the session usage limit (resumable, no loss)

- Tasks 1.1/1.2/1.3 LANDED before the limit hit (`b02b935` file protocol,
  `789e6e5` verbs + canvas mapping, `caa58bd` CLI + loop + env isolation).
  Task 1.4 (the live 2-seat gate) was mid-run: server had started with a QA
  config, seat dirs created, logs still empty (gate being restarted), no
  leaked processes.
- One UNCOMMITTED fix left in the tree (deliberately kept dirty): SETTLE_FRAMES
  — pump 8 frames after each applied command so a batched
  `click nav → set_value → click submit` doesn't run set_value against a
  not-yet-initialized field (the gate caught an empty-code join). Well-reasoned
  and mirrors the proven settle pattern, but the dead agent never re-ran the
  gate with it — the resumed implementer must VERIFY it live before committing.
- Plan on resume (after the ~2:50am PT window reset): fresh implementer
  finishes Task 1.4 (verify settle fix → full 2-seat script → ink verified in
  both seats' screen.png → save ui-samples.txt → commit), then W1 review, then
  W2/W3.
- **Skill note:** long implementation waves need to be RESUMABLE — per-task
  commits (the plan's granularity) meant a hard mid-wave interruption cost
  almost nothing; the only limbo was one uncommitted fix, and the
  leave-dirty-verify-then-commit rule handles exactly that.

### 2026-07-10 — W1 gate: driver proven through DRAW; the harness catches its
### first two real bugs before cycle 1 (commit `4b421a7`)

- Task 1.4 live 2-seat gate: create → code-from-ui.md (39R5OM) → corrected
  Join flow → 2-player rosters → Start → word pick (UMBRELLA) → toolbar clicks
  → strokes. **Op-log sync confirmed** (the same dark-red X in BOTH seats'
  screenshots) and **per-seat redaction held** (guesser sees 8 blank slots).
  Calibration samples committed (ui-samples.txt) — W2's hard prerequisite met.
- **Misdiagnosis corrected:** the inherited SETTLE_FRAMES "fix" could never
  work (a missing signal can't be waited into existence). Kept at 8 with an
  honest re-justification (screen-readiness between batched nav→act commands).
  The real fix: the driver re-emits `TextChanged` after `set_value`.
- **FINDING 1 (app, HIGH — blocks the guess checkpoint):** the floating theme
  toggle (@1172,730 88×50) occludes the chat Send button (@1194,736 41×56) at
  the 1280×800 default — Send's pointer center lands inside the toggle, so
  "Send" flipped seat-1 to Dark instead of submitting. The exact occluded-hit
  class the driver exists to catch; `theme_toggle.rs` checks the toggle is
  clickable but not that it avoids occluding in-game controls. guess→chat→
  score through the driver is therefore UNVERIFIED (honestly reported, not
  papered over). → staged fix Track 1.
- **FINDING 2 (framework):** `probe::set_value` / any AT `SetValue` performs
  untracked `EditCommand::SelectAll+Insert` (`contract.rs:463/496/506`) and
  never emits `TextChanged` — the only emitters are the keyboard
  (`input.rs:741`) and IME (`ime.rs:583`) paths — so buiy_view's
  `route_text_input` (`router.rs:80`) never fires `on_input`, the model never
  folds, and `reconcile.rs:1552` clobbers the edit on the next rebuild.
  Affects ALL buiy_view text inputs; existing tests assert editor/a11y state,
  never model fold. Spec §res-Q5's "fires on_input" claim was wrong. → staged
  fix Track 2 (framework emit mirroring the keyboard path + spec correction +
  drop the driver workaround).
- OBS (log, don't fix): the guesser sees the full drawing toolbar in-game
  (design check for cycle 1 vs the reference bundle); both 150s draw windows
  timed out during root-causing — reconfirms widened timers for LLM seats.
- **Decisions:** W2 HOLDS until both tracks land + a combined live re-gate
  (incl. guess→chat→score). Tracks run sequentially in this worktree (two
  agents committing concurrently to one index is a real race). Fixing the bug
  beats demoting the checkpoint.
- **Skill note:** the harness build IS a QA pass — building agent eyes/hands
  against the real UI surfaced an occlusion bug and a framework AT gap before
  any playtest ran. Budget for "the harness finds bugs while being built."

### 2026-07-10 — Track 1 (Send occlusion) FIXED + review-approved

- Root cause: the toggle is stacked at the view ROOT over every screen;
  in-game is the one screen whose content fills the bottom-right corner. The
  top-layer toggle wins the pick while the global glyph tier paints "Send"
  above it — pick≠paint is a documented framework trait (view/mod.rs:8-11);
  the app-level corner collision is the defect. Reference design keeps its
  toggle in-game but clears Send by ~1px at exactly 800px height (fragile
  accident, not a pattern to copy — our pick rect is 16px taller anyway).
- Fix `e891000` (+ RED test/design note `d258825`): suppress the toggle on
  InGame via `when(...)` — `when(false)` yields a zero-paint, non-pickable
  `Element::empty()` (reviewer-verified). Theme remains settable on all menu
  screens and persists into the match. Design note:
  specs/2026-07-10-dooduel-theme-toggle-occlusion-design.md. Review:
  APPROVE-WITH-FIXES — README index line (done) + strengthen the test with
  the positive SubmitGuess signal (input-cleared assertion; folding now).
- Adjacent smells logged, untouched: the glyph-single-tier pick≠paint trait
  (framework; candidate follow-ups entry at close-out); `button()` padding
  makes the toggle's pick rect 88×50 vs the authored 72×34 pill; our in-game
  chat pane sits ~30px lower than the design's. The latter two are cycle-1
  triage candidates.
- **Skill note:** regression tests for occlusion bugs need the POSITIVE
  signal (the intended handler fired), not just the negative (the wrong one
  didn't) — the negative-only form misses a future different occluder.

### 2026-07-10 — Track 2 landed (APPROVE) + live re-gate PASS: W1 fully done

- Track 2 (framework): `be97c94` RED at the buiy_view model-fold tier →
  `23540a0` fix (capture EditOutcome.value_changed in honor_text_set_value,
  write TextChanged mirroring the keyboard path, value-gated) → `7931f22`
  driver-workaround removal + spec rev-2.2 → `5f4032b` reviewer-suggested
  coverage (emit-count semantics at buiy_core: change=1/identical=0/
  delete-clear=1/empty-noop=0; empty-clear fold at buiy_view). Review:
  APPROVE — reviewer independently reproduced the RED at the pre-fix commit,
  traced same-frame delivery on both dispatch paths, and proved no
  emit→fold→reseed loop (the reconciler's controlled-set uses the
  non-emitting seam by design). Full workspace 2287/0.
- **Live re-gate (`ca9f064` plan note): PASS end-to-end** — 2-turn match with
  role swap; both fixes confirmed live; redaction both directions; spoiler-
  safe guess chat; scoring live (+182/+100; podium 387/282); evidence PNGs
  kept. The W1 guess→chat→score checkpoint is VERIFIED; W2 unblocked.
- New harness finding from the re-gate: set_value + Send in ONE batch can
  race the fold (an empty guess submits). The briefings' say()-per-ack
  discipline already prevents it for seat agents; W2's smoke must emit them
  as separate settled steps; candidate driver polish (verify [value=…] before
  acking set_value) deferred.
- Driver code review note: the fixes + plan skeletons got dedicated fresh
  reviews; the driver example as-built gets its review folded into the W2
  wave review (dev-tool tier; black-box exercised by the smoke).
- **Skill note:** a timer-starved first match (0-0, image-analysis wall-clock
  ate the 150s draw window) re-proved the "widen timers for LLM seats" rule
  with hard evidence — the re-gate agent's own experience mirrors what seat
  agents will hit in cycles.

### 2026-07-10 — W2 smoke interrupted by session limit (resumable, dirty)

- W2 implementer hit its session limit and died before committing/reporting.
  It left apps/dooduel/tests/qa_seat_smoke.rs UNCOMMITTED (412 lines).
  Orchestrator assessment: it COMPILES (--no-run Finished), clippy-clean, no
  unfinished markers; temp-dir evidence showed a real 3-seat run reaching
  Host-draw + Priya/Theo joins via the corrected flow — so it is feature-
  complete and its multi-seat logic works. BUT `cargo fmt --check` flags a
  diff (agent hadn't fmt'd), so it is NOT gate-clean → left dirty, not
  checkpoint-committed (and the orchestrator does not run fmt on source
  itself — the delegation bright line). Orphaned /tmp/qa-smoke-* dirs cleaned.
- Resume plan: a fresh agent runs fmt, runs the #[ignore] GPU smoke twice,
  calibrates HEADER_SKIP_PX against a real screenshot, verifies PASS, then
  commits. If the account is still session-limited (reset ~7:50am PT), a
  scheduled wakeup dispatches it after the window.
- **Skill note:** "compiles + clippy-clean + evidence-of-a-real-run" is a
  useful cheap orchestrator triage to distinguish a nearly-done dirty file
  from a half-written one — but fmt-dirty means don't checkpoint-commit; hand
  the leave-dirty file to the resume agent (the delegation rule keeps the
  orchestrator from fmt-ing/verifying implementation inline).

### 2026-07-10 — W2 smoke resumed → found bug #3 (deferred W2, opened Track 3)

- Resume agent (w2b): fmt'd the smoke, verified HEADER_SKIP_PX=220 against real
  GPU frames (countdown rows 127–182, ink ≥220 — clean separation), confirmed
  amendment (A) was already present. Checkpoints 1–5 PASS both runs; checkpoint
  6 (the guess) FAILS deterministically (both runs, 44/45s). It correctly did
  NOT self-approve a demotion — STOPPED and reported.
- **FINDING 3 (framework, blocks reliable cycles):** `probe::set_value` into
  the in-game chat `text_input` never folds. Root cause (w2b + orchestrator
  hypothesis, to be confirmed by Track 3): the field is a long-lived CONTROLLED
  input; the in-game screen rebuilds ~1/s (countdown Tick), and each rebuild's
  controlled-set reconcile `set_editor_value` (reconcile.rs:1552) forces
  editor←model("") via the silent `apply` seam (no TextChanged), clobbering the
  set_value edit before `route_text_input` folds it. Static/fresh-navigated
  fields (Join) escape — which is why the W1 re-gate's guesses eventually
  landed (and why one "vanished"). Same bug-family as #98, one layer deeper.
  Correlated: the in-game chat placeholder renders stale ("Waiting for the
  word…" during Drawing). Failing assertion: qa_seat_smoke.rs:404
  `wait_value` (Priya, expected the lowercased word).
- **DECISION — FIX, don't demote (Track 3).** Every playtest guesser types via
  set_value into this exact field, so a racy fold makes every cycle's guessing
  flaky — a blocker for the harness, not a deferrable test nicety. Demoting
  checkpoint 6 would ship the campaign on an unreliable guess path. The smoke
  test stays dirty+uncommitted (correct as written); it passes once the fold
  is fixed, then W2 commits.
- Track 3 = staged framework fix (systematic-debugging root-cause → design note,
  since ordering-vs-clobber-guard-vs-fold-at-source is a real 2+-approach
  decision → RED at the buiy_view rebuilding-controlled-input tier → fix →
  review). Evidence handed off (the two /tmp/qa-smoke-* dirs + header-verify).
- **Skill note:** the harness has now found THREE real bugs (app occlusion +
  two framework AT-fold bugs) BEFORE cycle 1 — the "the harness finds bugs
  while being built" budget line is not hypothetical. Each was caught because
  the driver exercises the REAL widget/AT surface the protocol seats bypass.
  Also: the smoke test itself (an automated multi-seat GUI runner) is a
  higher-signal gate than a human playtest for THIS class — deterministic,
  re-runnable, bisectable.

### 2026-07-10 — Track 3 fixed + APPROVED; W2 smoke fully green

- Track 3: `9bb9feb` RED (controlled input on a per-frame-rebuilding view) →
  `e81b91f` fix (b) the `PendingProgrammaticEdit` marker: set by
  honor_text_set_value alongside the #98 TextChanged emit, honored by the
  controlled reconcile to skip its one clobber, cleared by route_text_input on
  every drained TextChanged → `d2f4863` the separate stale-placeholder fix (the
  reconcile patch branch never re-patched the placeholder across a phase
  change). Design note: specs/2026-07-10-dooduel-controlled-input-setvalue-fold-design.md.
- Root cause PROVEN: reconcile (ViewSet::Reconcile, .before(Layout)) runs
  front-of-frame ahead of the AT fold (route_text_input, late MvuSet::Enqueue);
  on a Changed<M>-every-frame screen it re-asserts the stale model over the
  un-folded edit before the fold reads it — the Changed<M> early-out is exactly
  why static Join escapes and the ticking in-game screen doesn't.
- The (d)→(b) discovery is the lesson: the agent implemented rejected-option
  (d) "push only on prop change" first, the FULL suite caught it breaking
  on_submit_with (whose post-submit clear relies on the reconcile re-asserting
  a constant "" — so "prop unchanged" ≠ "leave editor alone"; the right
  discriminator is "in-flight un-folded edit"). The test suite disproving a
  plausible fix is the mechanism working.
- Review: APPROVE — all four marker-lifecycle cases traced stuck-free, silent
  seam intact, #98/emit-count/on_submit_with/keyboard/IME all green, MT-safe;
  reviewer independently ran the W2 smoke green (checkpoint 6, guess scores
  +488). Non-blocking N1: a benign Bevy warn on the rare set_value-then-
  navigate-away edge (remove queued on a despawned entity) — logged, not fixed.
- **W2 smoke checkpoint 6 now PASSES** → the guess path is robust for cycles.
  Smoke test committed; W2 done.
- **Skill note (campaign-level):** 3 bugs, 3 clean staged fixes, each with an
  adversarial fresh-review gate, all BEFORE cycle 1. The harness-build phase
  doubles as the deepest QA pass — it exercises framework AT paths that neither
  the human playtest nor the protocol seats reach. Bake "expect the harness
  build to surface framework bugs; fix them staged, don't demote the checks"
  into the skill.

### 2026-07-10 — CYCLE 1: first 4-agent live playtest COMPLETE

- A full networked match to the podium (Ada 804 / Priya 484 / Sam 432 / Theo
  286), 4 LLM seats on the `qa_seat` GUI driver, wide timers (240/120/60). All
  standing invariants held: every seat drew once (octopus/cupcake/penguin/
  bicycle), every word guessed, per-seat word secrecy throughout, podium
  correct. Triage: `cycle-1/triage.md`; evidence committed `cd39589`.
- **Mechanics audited SOLID** (Theo): 4 exact drawer-payout data points, correct
  `0.82^order` guesser decay, near-miss "So close!" private nudges (verified,
  never broadcast), literal wrong-guess broadcast, post-lock protection, timer
  tracks wall-clock. **All regression-watch items GREEN live** (KI-01/02/25/26/
  27) — the 3 harness-found fixes re-confirmed by multiple seats in a real match.
- **Findings:** F1 (S3, headline, ALL 4 SEATS) in-game countdown NUMBER freezes
  in screen.png while the model clock ticks — app-render-invalidation vs
  driver-readback-staleness UNDECIDED → Track A adjudicates on the live app
  (verdict decides whether the visual-QA lane is trustworthy). F1b canvas-not-
  cleared-between-turns (adjudicate w/ F1). F2 (S4) stale guess-draft persists
  across turns (masks the phase placeholder) → Track B app fix. F3 KI-11 was a
  STALE DOC (payout is proportional `round(100·correct/guessers)`, verified
  game.rs:351 — corrected + KI-28 added for the expected Continue→NotFound).
  F4 dashed-box (likely intended aesthetic; verify). UX-polish backlog logged.
- **Disposition: NOT clean** (new S3 + S4) → fix, then cycle 2 (rotated
  archetypes re-verify by a different seat).
- **Skill notes:** (1) 4 independent seats corroborating one symptom (the
  countdown freeze) is far stronger signal than one — the multi-archetype fleet
  earns its cost. (2) The naive seat (Ada) surfaced the two most visceral
  findings (frozen timer, canvas-not-clearing) precisely BECAUSE she watched
  screens without QA-dimension steering — the under-briefed seat is a feature.
  (3) The mechanic seat's deliberate arithmetic audit turned a 3-seat "S2 bug"
  (proportional payout) into a "the DOC is wrong, game is right" — a scoring
  audit needs the actual formula, else correct behavior reads as a bug. (4)
  Harness lessons to fold into briefings before cycle 2: force `shot` before
  visual reads (passive screen.png lags); detect phase via status/word-slots
  not the placeholder (F2 masks it); tighten guesser cadence; front-load chaos
  drawer-probes. (5) A finding that might be a HARNESS artifact (F1) still gets
  first-class adjudication — if the eyes lie, every visual finding is suspect.

### 2026-07-10 — cycle-1 fixes LANDED (all 3 staged + review-approved)

- **Track A / F1 (framework render — the campaign's biggest single find):**
  `951686a` — the countdown-number freeze was a REAL render-invalidation bug,
  reproduced on the live windowed app with zero input (frozen 9,9,9,9… for a
  whole picking phase while the ring drained). Root cause: `extract_buiy_glyphs`
  gated glyph re-extraction on `Changed<ComputedTextLayout>` (geometry only), so
  a Text whose content changes without changing geometry (a ticking number in
  Caveat) never re-extracted → stale glyphs. Fix: add `Changed<Text>` to the
  gate, mirroring the existing placeholder-buffer precedent. Review APPROVE:
  both GPU legs green (buiy_core 87/0, buiy_verify 24/0), ZERO golden shifts
  (correct — single-frame goldens can't fire the changed-text term), MT-perf
  splice intact (content-only change → one Patch, never a wholesale Full, O(0)
  steady), workspace 2293/0. RED confirmed by revert.
- **Track B / F2 (app):** `5320ae3` — clear the guess-draft (`chat_input`) on
  `PhaseChanged`, so a stale un-submitted guess no longer persists across turns
  (and no longer masks the phase placeholder). **F1b (app):** `b852ec2` — blank
  the LOCAL canvas on the falling edge out of Drawing/Reveal (server op-log was
  already cleared per turn; only the local pixel buffer lingered into the next
  Picking). Both RED→GREEN, canvas_e2e 10/10 (drawer-optimistic-ink + reseed
  unaffected), review APPROVE. **F4** dashed-box = intended aesthetic (minor
  field-inset nit → follow-up). Reconnect-draft asymmetry → follow-up.
- Ledger: KI-29 (countdown render), KI-30 (guess-draft), KI-31 (canvas-clear)
  added to §1 regression-watch — cycle 2 re-verifies each by a rotated seat.
- **THE headline lesson for the skill:** the live agent playtest found a real
  FRAMEWORK render bug the ENTIRE existing verification suite missed — because
  goldens/reftests are single-frame and this bug only manifests over TIME (text
  changing while geometry is stable). Static verification is structurally blind
  to temporal render defects; a live playtest is the only gate that observes
  them. Generalizes far past Dooduel (any live-updating label).
- **NEXT: cycle 2** — rotate archetypes (seat0=mechanic, seat1=chaos,
  seat2=naive, seat3=host+visual), rebuild (app+framework changed), re-verify
  KI-29/30/31 + the earlier fixes live by different seats, hunt new issues.

### 2026-07-10 — CYCLE 2 close-out (rotated re-verify; fixes held; 2 real findings)

- Full match, rotated archetypes, ALL cycle-1 + prior fixes re-verified HELD by a
  different seat than found them (KI-29 countdown-render confirmed live end-to-end
  — the re-verify Track A deferred), ZERO regressions, visuals ON-SPEC, mechanics
  SOLID. Naive seat (Dana) won without stalling. Triage: `cycle-2/triage.md`.
- Three findings → two dismissed, ONE real fixed, ONE deferred:
  - **C2-03 hint-schedule "off-by-one" (S2) = NOT A BUG.** The 1-based schedule
    is the intended design (FINAL spec §5(b) DECISION, explicit "not a porting
    error" note); the false positive came from a STALE QA BRIEFING (the mechanic
    briefing I had drafted stated the formula 0-based). Briefing + finding-template
    corrected (`7b05679`); KI-32 added (`ba105aa`). **I made the same wrong
    adjudication** from the formula's structure — the agent's adjudicate-against-
    the-spec-FIRST discipline caught it and prevented a regressing "fix" that would
    have broken 2 green design-encoding tests.
  - **C2-04 lobby font (S4) = FIXED** (`006b9b6`): lobby code was Caveat
    (`FONT_DISPLAY`), now `FONT_BODY` — matches the in-game code. (Emmy's "should
    be Geist Mono" was itself slightly stale: the app has no Geist Mono; Shantell
    Sans body is the mono-role stand-in.) KI-33 regression-watch.
  - **C2-02 empty chat pills (S3) = REAL framework render bug, DEFERRED.** App
    proven clean + guarded (`4ae7764`: 24-row networked repro, keyed_rows == model
    chat); the content-less green pills are a GPU render-layer artifact (stale/extra
    quad at volume, KI-02 atlas family). Localized but NOT fixed here — deferred to
    a framework render track (KI-34 + follow-up). Honest scope call: the playtest's
    job (find + localize) is done; the framework GPU fix is a separate focused effort.
- **Skill notes:** (1) TWO of cycle-2's three "bugs" were DOC/expectation errors
  (stale briefing formula), not code bugs — a QA campaign generates false positives
  from its own docs; the adjudicate-against-the-spec-first gate is what separates
  them, and even the ORCHESTRATOR must not skip it (I didn't, and was wrong). (2)
  The rotation works: a fix found by seat A is re-verified by seat B — cycle 2
  confirmed all of cycle-1's fixes hold. (3) The naive seat remains the highest-
  yield finder (won the match AND found the 2 real findings). (4) An app-layer
  agent PROVING app-cleanliness and localizing a bug to the framework (with a
  committed guard) is a valid, valuable outcome even without a fix — it turns a
  vague "empty pills" into a precise, tracked framework render defect.

## Skill-distillation notes (seed questions)

- What makes an agent playtest *reliable*? (settle-waits vs stale screenshots,
  seat orchestration, turn timing, evidence capture.)
- How to keep findings high-signal: structured report contract per seat, severity
  taxonomy, known-issues list to suppress re-reports, dedupe across seats.
- Scripted per-seat probe assignments vs free play — what mix finds the most?
- What's the right exit criterion ("cycle clean") and how many cycles does it
  actually take for a real app?
- Which parts are Dooduel-specific vs general enough for a skill?
