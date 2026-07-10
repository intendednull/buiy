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

## Skill-distillation notes (seed questions)

- What makes an agent playtest *reliable*? (settle-waits vs stale screenshots,
  seat orchestration, turn timing, evidence capture.)
- How to keep findings high-signal: structured report contract per seat, severity
  taxonomy, known-issues list to suppress re-reports, dedupe across seats.
- Scripted per-seat probe assignments vs free play — what mix finds the most?
- What's the right exit criterion ("cycle clean") and how many cycles does it
  actually take for a real app?
- Which parts are Dooduel-specific vs general enough for a skill?
