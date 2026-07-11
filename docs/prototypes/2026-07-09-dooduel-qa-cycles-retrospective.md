# Dooduel QA playtest cycles — retrospective

**Date:** 2026-07-10
**Status:** active
**Branch:** `feat/dooduel-multiplayer-m1` (worktree `dooduel-app2`)

> Retrospective for the Dooduel QA playtest-cycles campaign (journal:
> [`2026-07-09-dooduel-qa-cycles-journal.md`](2026-07-09-dooduel-qa-cycles-journal.md)).
> The campaign's stated meta-deliverable was to decide whether a reusable
> **qa-playtest-cycles skill** falls out of the process. It does — the draft is
> [`2026-07-09-dooduel-qa-cycles/qa-playtest-cycles-SKILL-draft.md`](2026-07-09-dooduel-qa-cycles/qa-playtest-cycles-SKILL-draft.md),
> proposed for promotion (placement recommendation in §7).

---

## 1. Verdict

**The campaign achieved its goal.** Agent-driven QA — LLM seats that *see* the
rendered game and *act* through the real UI — found and fixed real bugs that a
human pass or the existing static-verification suite structurally could not catch.
The headline proof: the live playtest surfaced a **framework render-invalidation
bug** (`951686a`) where a `Text` whose content changes without changing geometry
kept stale glyphs on screen — a defect that only manifests *over time*, so the
entire single-frame golden/reftest suite was blind to it. That is the load-bearing
result: a live playtest is the only gate that observes temporal render defects, and
it generalizes far past Dooduel (any live-updating label). Supporting evidence: the
firewalled naive seat was the highest-yield finder in both cycles; every fix was
re-verified live by a *different* archetype in the next cycle; and the harness-build
phase alone found three real bugs before cycle 1 even ran.

The campaign is not yet at its formal exit criterion (a clean cycle or ≥5 cycles).
Two cycles landed clean-of-regressions with all fixes holding; cycle 3 was in
flight at the time of writing.

---

## 2. What the campaign did

**Research (4-agent fleet).** Parallel agents covered playtest archaeology (three
prior multi-agent playtests across two harness generations), current M1 app state,
the agent eyes-and-hands interaction surface, and a QA charter (dimensions, severity
taxonomy, report template, exit criterion). The decisive finding: in *every* prior
run the seat agents never saw the rendered GUI — they played from a text `get_state`
view plus a canvas PNG, so GUI-only bugs (empty chat pills, a stuck scoreboard) were
caught only by ad-hoc human screenshots. Closing that gap became the campaign's
central requirement.

**Harness — the GUI seat driver (built, and it *was* a QA pass).** A new dev-tool
example (`apps/dooduel/examples/qa_seat.rs`) runs the *real* Dooduel client (real
`view`/MVU/`WsClientPlugin` against a real `dooduel_server`), renders **offscreen**
to a `RenderTarget::Image` with GPU readback for pixel "eyes," emits the raw a11y
semantic snapshot (`ui.md`) as the second eye, and acts through a file-command
protocol driving a **real synthetic `bevy_picking` pointer** for "hands." Its design
is pinned in [`docs/specs/2026-07-09-dooduel-qa-seat-driver-design.md`](../specs/2026-07-09-dooduel-qa-seat-driver-design.md)
(the two-camera picking+readback composition, the `#[ignore]` GPU-lane smoke). The
spec was reviewer-gated (two fresh reviewers, 12 folded fixes) and the plan reviewer
returned an outright **BLOCK** that saved a wasted spike. **Building the harness
found three real bugs before any playtest ran** — an app-layer occluded-hit (theme
toggle over the chat Send button, `e891000`) and two framework AT-fold bugs
(`SetValue` never emitting `TextChanged`, `23540a0`; a controlled input clobbered on
a per-frame-rebuilding screen, `e81b91f`) — each fixed via a full staged cycle with
an adversarial review gate, because the driver exercises the real widget/AT surface
the protocol seats bypass.

**Briefings + runbook.** A shared briefing (`COMMON.md`, including the `say()`
foreground-ack helper), four archetype supplements, a finding template with a
severity ladder, an orchestrator runbook, and a code-verified known-issues
suppression ledger. The known-issues consolidation immediately earned its keep:
2 of 4 carried-forward complaints were fixed-by-construction harness artifacts that
would have wasted a fix cycle if re-reported.

**Cycles 1–2 (cycle 3 in flight).** Each cycle ran a full 4-seat networked match to
the podium, collected structured findings, triaged them (dedupe, known-issues check,
severity/layer), fixed the real ones via staged-development, moved each fix into the
regression-watch ledger, and **rotated archetypes** so the next cycle re-verifies
each fix with a different seat. Cycle 1 found the headline framework render bug plus
two app fixes; cycle 2 re-verified all of cycle 1's fixes as HELD, found one real app
fix and one deferred framework defect, and dismissed two false positives by
adjudicating against the spec first.

---

## 3. Findings ledger (whole campaign)

### Real bugs found and fixed

| # | Finding | Class | Sev | Layer | Commit(s) | Ledger |
|---|---|---|---|---|---|---|
| 1 | Theme toggle occludes chat **Send** (pick wins, paint shows "Send") | occluded-hit / pick≠paint | HIGH | app | `e891000` (RED `d258825`, `6f5d161`) | KI-25 |
| 2 | AT `SetValue` never emits `TextChanged` → MVU model never folds (join/guess submit empty) | AT-fold gap | — | framework (buiy_core) | `23540a0` (RED `be97c94`, `7931f22`, `5f4032b`) | KI-26 |
| 3 | Controlled `text_input` on a per-frame-rebuilding screen clobbers un-folded AT set_value | reconcile-vs-fold ordering | — | framework (buiy_view) | `e81b91f` (RED `9bb9feb`, `d2f4863`) | KI-27 |
| 4 | **In-game countdown NUMBER freezes** while the model clock ticks (stale glyphs on geometry-stable text change) | temporal render-invalidation | S3 | **framework (render)** | `951686a` | KI-29 |
| 5 | Stale un-submitted guess-draft persists across turns, masks the phase placeholder | model-not-reset | S4 | app | `5320ae3` | KI-30 |
| 6 | Local canvas not cleared between turns (server op-log cleared; pixel buffer lingered) | stale local state | S3 | app | `b852ec2` | KI-31 |
| 7 | Lobby room-code font is Caveat (display) not the body/mono face | wrong font token | S4 | app (view) | `006b9b6` | KI-33 |

Bugs 1–3 were found **during the harness build**, before cycle 1. Bug 4 is the
campaign headline (a temporal defect goldens cannot see). All seven were RED→GREEN
test-first with a fresh-review gate.

### False positives dismissed by adjudication (both were STALE DOCS)

| # | Reported as | Truth | Root of the false positive |
|---|---|---|---|
| F3 | KI-11 "drawer payout is flat +100; best drawer can finish last" (S2) | Payout is **proportional** `round(100·correct/guessers)`, verified `game.rs:351`; matches the pinned skribbl formula. Game correct; the ledger wording was stale. | Stale entry in `known-issues.md`; corrected, no code change. |
| C2-03 | Hint schedule "off-by-one, ~44 s late" (S2) | **1-based schedule is the intended design** (FINAL spec §5(b) DECISION, explicit "not a porting error"); `game.rs:452` + 2 green design-encoding tests already encode it. | Stale QA **briefing** stated the formula 0-based; the orchestrator initially made the *same* wrong adjudication — the agent's adjudicate-against-the-spec-first discipline caught it. |

Adjudicating both against the spec **before** touching code prevented two regressing
"fixes" (C2-03's would have broken two green design-encoding tests). See §4 for why
this gate is non-optional even for the orchestrator.

### Deferred (tracked, not fixed here)

| id | Item | Why deferred |
|---|---|---|
| KI-34 | Empty "guessed" chat pills at high volume (C2-02) — a framework GPU render-layer artifact (stale/extra quad, no glyphs, no backing app node), same family as the KI-02 multi-page atlas lineage | App proven clean + guarded (`4ae7764`, 24-row networked repro asserting `keyed_rows == model chat`); the render-layer fix is a separate focused framework effort. The playtest's job — find + localize — is done. |
| KI-07 / KI-08 | AFK-drawer early-out; "Turn N of M" indicator | Genuinely-open known items (verified against M1 code), not campaign finds; cycle 1 re-confirmed KI-08 hurts first-timer orientation. |

Also dismissed as not-a-bug: F4 dashed-box around the room-code field (intended
sketchy aesthetic), C2-01 "stale panel header" (the right panel is the scrolling chat
log), and several config-artifact reads (60 s reveal window, per-phase timer resets).

---

## 4. What worked — the load-bearing techniques

- **GUI-render eyes, not protocol-only.** Every prior playtest read a denoised text
  view and missed every GUI-only bug. Rendering the *real* client offscreen and
  handing agents pixels + a **raw** semantic snapshot (no "you can now…" summary
  layer — that denoising is exactly what hid past visual bugs) is what made bugs 1, 4,
  6, and 7 observable at all. Driving a **real synthetic pointer** (not the a11y typed
  click) is what made the occluded-hit class (bug 1) catchable.
- **The archetype fleet, and the firewalled naive seat as the top finder.** Four
  archetypes — host/visual, mechanic, chaos, naive-first-timer — cover different bug
  classes. The naive seat, given *only* the shared briefing and its own supplement
  (never the charter, reference design, or known-issues), surfaced the two most
  visceral cycle-1 findings (frozen timer, canvas-not-clearing) precisely *because* it
  watched screens without QA-dimension steering. It won both matches without stalling.
  The under-briefed seat is a feature, not a compromise.
- **Multi-seat corroboration.** Four independent seats reporting one symptom (the
  countdown freeze, all 4 seats) is far stronger signal than one — it turned an
  "is-this-the-harness-lying" ambiguity into a high-confidence real-bug lead.
- **Rotation for cross-seat re-verification.** `archetype(seat, cycle) = (seat +
  cycle − 1) mod 4` guarantees a fix found by seat A is re-verified by seat B in the
  next cycle. Cycle 2 confirmed every cycle-1 fix HELD, by a different seat than found
  it — the discipline that lets "clean" mean something.
- **The harness build *is* the deepest QA pass.** Building agent eyes/hands against
  the real UI surfaced three framework/app bugs before cycle 1. Budget for it: the
  driver exercises AT/widget paths neither a human playtest nor the protocol seats
  reach, and its `#[ignore]` smoke test (a deterministic, re-runnable, bisectable
  multi-seat GUI runner) is a higher-signal gate than a human pass for that class.
- **Adjudicate against the spec BEFORE fixing.** Both cycle-2 "bugs" that weren't
  bugs were caught here — including one the orchestrator itself initially got wrong.
  A QA campaign generates false positives *from its own docs* (a stale ledger, a stale
  briefing); the adjudicate-first gate is what separates a real defect from a doc
  error, and it must be applied even when the orchestrator is confident.
- **The reliability rules (each earned by a prior failure).** Persistent
  **foreground** poll loops with bounded timeouts (background wakes arrive batched —
  cost one prior seat 3 of 4 turns); **force a `shot` before every important visual
  read** (the passive `screen.png` refresh lags); **detect phase via status/word-slots,
  not the placeholder** (a stale draft masks it); **lock the correct guess first,
  chat later** (the ~40 s observe-act cycle loses easy guesses to turn-end); **deliver
  the report as final text + a `SendMessage`** (seats reliably go idle without
  delivering, and the harness blocks them from writing `report.md`).

---

## 5. What didn't / friction

- **Seats idle without reporting.** The recurring #1 failure. Seats act once and go
  quiet; the orchestrator has to `SendMessage`-nudge each one to draw on its turn, to
  Continue at reveal, and to deliver its report at the end. The runbook now bakes in a
  metronome policy, but it is manual back-stop labor.
- **The mechanic seat is slow.** Recomputing scoring/hint arithmetic per turn made it
  the least efficient player; its deliberate audits are valuable but eat wall-clock.
- **Chaos drawer-probe coverage keeps falling short.** In both cycles the chaos seat's
  destructive drawer probes (Undo/Clear spam, out-of-bounds stroke) didn't all run —
  a turn ends fast once others guess (the 2/3-guessed draw-timer clamp), so the probe
  window closes. Fix carried into cycle 3: batch *all* destructive probes in one burst
  on the first legible stroke.
- **Stale QA docs generate false positives.** The stale known-issues ledger (F3) and
  the stale mechanic briefing (C2-03) each produced a false S2. The campaign's own
  documentation is a false-positive source that must be kept code-current.
- **Subagents blocked from writing `report.md`.** Seat agents cannot write
  `report`/`findings` `.md` files (a harness guard), so the report contract had to
  route through final-text + `SendMessage` + evidence-PNG copies. Discovered mid-cycle-1;
  now stated up front in the briefing.
- **Dead-time from AFK/slow agent latency.** The naive seat's #1 annoyance ("what
  would make a real person tab away") is largely agent-latency, not a game defect — but
  it inflates every cycle's wall-clock and is worth shortening with tighter reveal
  timers and (app-side) a drawer-idle early-out.
- **Session-limit interruptions mid-wave.** Two implementation waves were cut off by
  usage limits. Per-task commits made them nearly free to resume; the one hazard was a
  left-dirty uncommitted fix, handled by the leave-dirty-verify-then-commit rule.

---

## 6. Meta-lessons for a skill

1. **Agents need real render eyes, not a denoised text view.** Render the real client
   offscreen and hand over pixels + the *raw* semantic tree; a summarizer hides exactly
   the visual bugs you are hunting.
2. **Act through the real input path.** A synthetic pointer through the real hit-test
   catches occluded-hit / pick≠paint bugs that a semantic "typed click" cannot.
3. **Building the harness is the first (and often deepest) QA pass.** Budget for it to
   find framework bugs; fix them staged, don't demote the checks that caught them.
4. **A firewalled naive seat is the highest-yield finder.** Withhold the charter,
   reference, and known-issues from it; its uncoached confusion finds the most visceral
   bugs.
5. **Run a fleet of archetypes and corroborate across them.** Multiple independent
   seats hitting one symptom is a strong-signal lead and disambiguates harness-vs-app.
6. **Rotate archetypes across cycles so every fix is re-verified by a different seat.**
   That is what makes "cycle clean" trustworthy.
7. **Adjudicate every finding against the spec before writing a fix — even the
   orchestrator's own confident reads.** A QA campaign manufactures false positives from
   its own stale docs; adjudication is the separator, and it caught the orchestrator
   being wrong.
8. **Keep a suppression + regression-watch ledger, verified against current code.**
   Suppress accepted quirks; treat a regression-watch recurrence as a real regression;
   re-verify code before suppressing (fixed-by-construction items must not be re-filed).
9. **Reliability is a set of hard rules, each paid for by a past failure:** foreground
   bounded poll loops; shot-before-visual-read; phase-via-status-not-placeholder;
   lock-first cadence; report-via-final-text-and-SendMessage; expect-idle-and-nudge.
10. **Static verification is structurally blind to temporal defects.** A live playtest
    is the only gate that observes bugs that manifest over time (a label that changes
    while its geometry is stable) — this is the class of bug that justifies the whole
    method.
11. **Structured findings with a severity ladder keyed to player-observable
    consequence** (not suspected code) keep signal high and let the orchestrator triage
    fast; tie-break to the higher severity at low confidence.
12. **Localizing a bug to a layer, with a committed guard, is a valid outcome even
    without a fix.** An app-layer seat proving app-cleanliness and handing the framework
    a precise, tracked defect (C2-02 → KI-34) is real progress.

---

## 7. Skill placement recommendation

**Recommend promotion to a personal skill** (`~/.claude-personal/skills/`), not a
project skill. The method — build agent eyes/hands, write archetype briefings + a
suppression ledger, run rotate-and-re-verify cycles gated by adjudicate-before-fix —
is entirely general to *any* interactive app with an agent-drivable UI; nothing in the
distilled principles is Dooduel- or Buiy-specific (Dooduel is only the worked example).
It composes with the user's existing global skills (`reliable-agent-fleet`,
`staged-development`, `prototype-first-development`, `subagent-driven-development`)
rather than duplicating them, which is the pattern for personal-scope skills. Keep the
Dooduel campaign docs as the concrete worked example the skill points to. The
orchestrator makes the final placement call after review; the draft lives at
[`2026-07-09-dooduel-qa-cycles/qa-playtest-cycles-SKILL-draft.md`](2026-07-09-dooduel-qa-cycles/qa-playtest-cycles-SKILL-draft.md).
