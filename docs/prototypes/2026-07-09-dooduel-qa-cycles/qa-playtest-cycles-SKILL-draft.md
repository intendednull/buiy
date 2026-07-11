---
name: qa-playtest-cycles
description: Use when QA-testing an interactive app (a game, a rich UI, a multi-user flow) by putting LLM agents in the seats — agents that SEE the rendered UI and ACT through the real input surface, hunting visual/mechanical/UX bugs a human pass or static tests miss. Drives the pipeline: research prior playtests → build the agent eyes/hands harness (building it IS a QA pass) → write archetype briefings + a suppression ledger → run rotate-and-re-verify cycles gated by adjudicate-before-fix, until a cycle is clean or a cycle-count floor.
---

# Agent-driven QA playtest cycles

> **DRAFT** — proposed for promotion to a personal skill (`~/.claude-personal/skills/`)
> or a project skill; the orchestrator decides final placement after review. Worked
> example: the Dooduel campaign in `docs/prototypes/2026-07-09-dooduel-qa-cycles/`
> (+ journal, retrospective, and seat-driver spec).

## Overview

Put N LLM agents into one real session of the app under test, each holding a
different seat/role. Each agent **sees** what a real user sees (a rendered
screenshot + a raw semantic snapshot) and **acts** only through the real input
surface (a synthetic pointer / typed input via a file-command protocol) — no
out-of-band pokes at internals. Agents report visual, mechanical, and UX/feel bugs.
The orchestrator triages, fixes the real ones through staged-development, and re-runs
with rotated roles so every fix is re-verified by a different agent.

**Why this finds bugs other passes miss:** static verification (goldens, reftests,
unit tests) is single-frame and single-path — structurally blind to *temporal*
defects (a label that changes while its geometry is stable) and to *emergent*
interaction bugs (occluded hit-targets, cross-turn stale state, volume-dependent
render artifacts). A live agent playtest is the only gate that observes them.

## When to use

- QA-testing an interactive app with an **agent-drivable UI** — a game, a rich
  editor, a multi-user/networked flow, anything where "does it feel right and behave
  right in a real session" matters and a human QA pass is expensive to repeat.
- You can render the real client's UI to an image and drive its real input path
  in-process (or via automation) — the prerequisite for agent eyes/hands.
- You want repeatable, bisectable QA that also generates a feel/UX report, not just
  a pass/fail.

**Don't use for:** pure headless logic (unit/property tests are cheaper and cover
it), one-shot "does this button work" checks, or apps with no drivable UI surface.

## The pipeline

### 1. Research prior playtests

Before building anything, learn what came before. Dispatch a small parallel fleet
(under `reliable-agent-fleet`) over: prior playtest runs and their hard-won lessons,
the current app state under test, the **agent interaction surface** (what can eyes
see, what can hands do), and a QA charter. Extract the charter: QA **dimensions**
(visual / mechanics / UX / robustness / feel), a **severity ladder**, a **finding
template**, and the **exit criterion**. Mine prior runs for reliability lessons —
they are almost always about *seat orchestration* (idle stalls, batched wakes, timing).

### 2. Build the agent eyes/hands harness — this IS a QA pass

The harness is a dev-tool that runs the **real client** and bridges it to each
agent's file world:

- **Eyes — pixels:** render the real UI offscreen to an image + read it back to a
  `screen.png`. The agent must see what a *user* sees, at real resolution.
- **Eyes — semantic:** emit the **raw** accessibility/semantic tree (roles + names +
  on-screen text) to a `ui.md`. **Raw, not summarized** — a "you can now…" affordance
  summary hides exactly the visual bugs you are hunting. Orientation guidance lives in
  the briefings, never in the tool output.
- **Hands — real input:** act by role+name through the **real synthetic pointer /
  hit-test** (not a semantic "typed click" that bypasses hit-testing — that path
  cannot catch occluded-hit / pick≠paint bugs). Support the primitives the real UI
  uses (click a widget, type into a field, drag/stroke), plus `shot` (force a refresh)
  and `quit`. A `NotFound`/miss is a genuine QA signal, never swallowed.
- **Protocol:** append-only `commands.jsonl` in, atomic-write `screen.png`/`ui.md` +
  an append-only `driver.log` out, each command acked with a monotonic index so the
  agent can block until its action landed. One process per seat; isolate per-seat
  state (env override, not a shared config file — N seats racing one file clobbers it).

**Prove the harness before any cycle** with a committed smoke test that self-spawns
the real backend and drives boot → readback → snapshot → a resolving click → the core
loop, with **≥3 concurrent seats** to validate the multi-instance regime.

**Expect the build to find real bugs.** Driving the real widget/input surface
exercises paths neither a human pass nor a headless test reaches. In the worked
example the harness build alone surfaced three real bugs (an occluded-hit, two
input-fold bugs) before cycle 1. **Fix them staged; don't demote the check that
caught them.** Gate the spec and plan with fresh reviewers — a playtest-harness spec
especially needs a **label audit** (verify the exact widget names/labels against the
real view code; a scripted flow that clicks a wrong name fails silently).

### 3. Write archetype briefings + a suppression ledger

- **A shared briefing (`COMMON.md`):** the honesty contract (a seat's dir is its
  entire world — no reading other seats, transcripts, or source), the eyes/hands
  protocol with a copy-paste **foreground-ack helper**, the loop discipline, the phase
  map, the report contract, and the known-issues check.
- **One supplement per archetype** (§Archetypes).
- **A finding template** with machine-parsable keys + the severity ladder, keyed to
  **player-observable consequence**, not suspected code.
- **A suppression + regression-watch ledger (`known-issues.md`):** §1 recently-fixed
  (re-verify live; a recurrence is a real regression), §2 known-open (don't re-report),
  §3 accepted-by-design (never report), §4 out-of-scope (absence is not a bug).
  **Verify every suppression against current code** — fixed-by-construction items
  re-filed as bugs waste a fix cycle.

### 4. Run cycles

For each cycle: **launch → metronome → collect → triage → fix staged → rotate → repeat.**

1. **Launch.** Widen the app's phase/timeouts — agents are much slower than humans
   (they reason over a screenshot each poll). One backend, N seat processes, N seat
   agents; each agent gets **only** its shared briefing, its archetype supplement, its
   seat-dir path, and the build sha under test.
2. **Metronome.** The #1 historical failure is a seat that acts once and goes idle
   (background wakes arrive batched). Tail the backend transcript + each seat's log;
   when a seat is due to act and hasn't, **`SendMessage`-nudge it**. Expect seats to
   go idle *without delivering their report* — nudge every seat to deliver at the end.
3. **Collect** each seat's findings + session/feel report + evidence.
4. **Triage (orchestrator).** You hold every seat dir, so you do the cross-seat work
   the honesty contract forbids the seats: **dedupe** (2+ seats = one higher-confidence
   finding), **known-issues check**, **cross-seat diffs** (e.g. compare shared state
   across seat dirs), **classify** severity + suspected layer. Write `cycle-N/triage.md`.
5. **Fix staged.** Each real S1–S3 goes through a full staged-development cycle,
   executed by subagents and gated by fresh review. **Adjudicate against the spec
   FIRST** (§Reliability rules). S4/S5 feed a polish backlog and do not dirty a cycle.
6. **Update the ledger.** Move each newly-fixed item into §1 regression-watch so the
   next cycle re-verifies it.
7. **Rotate + repeat.** Bump the cycle, rotate archetypes (§Archetypes), fresh match
   dir, fresh agents. Re-verify the fixes shipped since the last cycle.

### 5. Exit

Stop at the **first clean cycle, or after a cycle-count floor** (e.g. ≥5), whichever
comes first. A cycle is **clean** when: zero new S1–S3 findings; zero regressions
(every fix since the last cycle re-verified by a *different* seat); all scripted
probes hit their pinned outcomes; and the standing session invariants hold end to
end. Hand over the residual S4/S5 backlog and any consciously-deferred items as a
triaged ledger in a retrospective.

## Archetypes

Four complementary lenses. Give each one a supplement on top of the shared briefing;
each still plays honestly (takes its turns).

- **Host / visual auditor** — drives the session start; sweeps every screen against
  the reference design (color, type, layout, overlap); owns the visual regression-watch.
- **Mechanic auditor** — recomputes the numbers (scoring, timers, schedules,
  normalization) against the pinned formulas; the deliberate arithmetic audit is what
  turns "the game is wrong" into "the *doc* is wrong, the game is right." Slower; budget
  for it.
- **Chaos / robustness** — runs the destructive-probe matrix (bad input, wrong-phase
  actions, spam, out-of-bounds), each with a pinned expected outcome; a probe that hits
  its expected outcome is a PASS, not a finding. **Front-load destructive probes** —
  windows close fast.
- **Naive first-timer — FIREWALLED.** Gets *only* the shared briefing + its own
  supplement — **never** the charter, reference design, other supplements, or the
  known-issues list. Its whole value is uncoached fresh eyes; any of those docs spoils
  it. Empirically the **highest-yield finder** — it surfaces the most visceral bugs
  precisely because it watches without QA-dimension steering. Its main deliverable is a
  free-text confusion/feel log.

**Rotation.** `archetype(seat, cycle) = (seat + cycle − 1) mod N` — each cycle shifts
by one so a fix found by seat A is re-verified by seat B, and every seat eventually
plays every role. This is what makes "clean" trustworthy.

## Reliability rules (each paid for by a past failure)

- **Foreground, bounded poll loops.** One persistent foreground poll-act loop per seat
  for the whole session; every wait is a `timeout`, never a background/detached job —
  batched wakes make a seat miss its turns.
- **Shot-before-visual-read.** The passive screenshot refresh lags; force a `shot` and
  wait for its ack before judging *anything* visual.
- **Phase-via-status, not placeholder.** Detect state from stable status/structure, not
  from a field whose stale content a bug can mask.
- **Lock the primary action first.** The observe→reason→act latency can exceed a
  turn's remaining time; do the scoring/committing action immediately, exploratory
  probes after.
- **Adjudicate against the spec BEFORE fixing — including the orchestrator's own
  confident reads.** A QA campaign manufactures false positives from its *own* stale
  docs (a stale ledger, a stale briefing). Adjudication is the separator; in the worked
  example it caught the orchestrator itself making a wrong call that would have
  regressed two green tests.
- **Structured findings + a severity ladder keyed to player-observable consequence.**
  Machine-parsable keys, tie-break to the higher severity at low confidence, harness
  artifacts are never findings (they go in a separate harness-notes section).
- **Report via final-text + SendMessage.** Seat agents may be blocked from writing
  report files and reliably go idle without delivering — deliver the report as the
  final message text plus a `SendMessage` to the orchestrator; copy evidence images
  into the (unblocked) evidence path.
- **The harness build finds bugs; the smoke is a high-signal gate.** An automated
  multi-seat GUI runner is deterministic, re-runnable, and bisectable — a stronger gate
  than a human pass for interaction/render-invalidation classes.

## Pitfalls

- **Protocol-only eyes miss every visual bug.** If agents read a text view but never
  the rendered pixels, the whole visual-QA lane is a no-op — this is the gap the method
  exists to close.
- **A summarized semantic view re-hides the bugs.** Keep the semantic snapshot raw.
- **Stale QA docs generate false S2s.** Keep the known-issues ledger and the archetype
  briefings code-current; a stale worked-example formula in a briefing produces a
  confident false positive.
- **Seats go idle silently.** Budget orchestrator metronome labor; nudge to act and to
  deliver.
- **A finding might be a *harness* artifact.** Give it first-class adjudication anyway
  (is it the eyes lying, or the app?) — if the eyes can lie, every visual finding is
  suspect until proven.
- **Localizing without fixing is a valid outcome.** An app-layer seat proving
  app-cleanliness and handing a precise, guarded framework defect downstream is real
  progress — don't force a same-cycle fix across a layer boundary.
- **Long waves get interrupted.** Commit per-task so a mid-wave interruption is nearly
  free to resume; hand any left-dirty fix to the resume agent with a verify-then-commit
  rule.

## Composes with

`reliable-agent-fleet` (the research + seat fan-outs), `staged-development` (each
fix), `subagent-driven-development` (fix execution + gating),
`prototype-first-development` (the harness build as a learn-by-running phase),
`verification-before-completion` (the orchestrator verifying artifacts first-hand).
