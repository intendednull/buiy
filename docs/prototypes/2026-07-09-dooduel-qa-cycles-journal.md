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

## Skill-distillation notes (seed questions)

- What makes an agent playtest *reliable*? (settle-waits vs stale screenshots,
  seat orchestration, turn timing, evidence capture.)
- How to keep findings high-signal: structured report contract per seat, severity
  taxonomy, known-issues list to suppress re-reports, dedupe across seats.
- Scripted per-seat probe assignments vs free play — what mix finds the most?
- What's the right exit criterion ("cycle clean") and how many cycles does it
  actually take for a real app?
- Which parts are Dooduel-specific vs general enough for a skill?
