# Dooduel QA — the finding template (copy-paste this)

Every bug you file is **one fenced ```yaml block**, appended to your seat report
under a `## Findings` heading. Machine-parsable keys, human-readable values. This
file is the source of truth for the fields, the severity ladder, and a worked
example — copy the block, fill every key. (Extracted from the QA charter §2/§3.)

Before you file anything: run the **known-issues check** (see the bottom of
`COMMON.md`). If the symptom matches a `KI-nn` row in
`../known-issues.md`, set `known_issue: yes:KI-nn` and only file if it is
**materially worse** than the logged entry (attach the delta) or a **§1
regression-watch** item recurred (that is a real regression, S2 minimum).

---

## The template

```yaml
id: C<cycle>-S<seat>-<nn>            # e.g. C1-S2-03 — cycle number, seat number, running count
severity: S1|S2|S3|S4|S5            # see the ladder below
dimension: visual|mechanics|ux|robustness|feel
screen: home|join|lobby|in_game|word_pick|reveal|podium|avatar_editor|cross-screen
seat: <0-3> (<Name>)                # and your role at the time: drawer|guesser|host
build: <git short sha under test>   # the orchestrator gives you this in your launch message
turn: <round.turn or "lobby"/"pre-match"/"podium">
repro:                              # numbered, from a known state, the EXACT commands you sent
  - "1. As guesser in Drawing phase, ui.md countdown read 143 s-left"
  - "2. Observed the first hint letter appear in the word slots"
expected: >                        # what SHOULD happen, and CITE the source of that expectation
  With draw_seconds=240 + hints=2, the first hint reveals at
  floor(240*(0.6 - 0*0.18)) = 144 s-left (charter §1.B hint formula).
actual: >
  The first letter appeared at 143 s-left. (Within 1 poll — likely fine; logging
  to confirm the schedule tracks the widened timer, not the charter's 150 s example.)
evidence:                          # files IN YOUR SEAT DIR — copy the relevant screen.png to findings/<id>.png
  - findings/C1-S1-04.png
  - "ui.md excerpt: word slots '_ _ B _ _', countdown 143"
confidence: high|medium|low         # low = couldn't reproduce twice, or might be a harness artifact
known_issue: no|yes:KI-nn          # checked against ../known-issues.md BEFORE filing
suspected_layer: app|dooduel_core|server|framework(buiy)|harness|unsure
notes: optional free text
```

**Evidence discipline.** Your evidence must be things a *player* could see: copy
the relevant `screen.png` into `findings/<id>.png` in **your own** seat dir at the
moment you observed the bug, and quote the `ui.md` lines. Never cite another seat's
dir or any repo file — your findings come only from what your seat could observe
(the honesty contract, `COMMON.md`).

---

## Severity ladder (classify by *player-observable consequence*, not suspected code)

| Sev | Name | Rule (Dooduel-specific) |
|---|---|---|
| **S1** | Crash / progress-blocker / integrity breach | App or server crash/hang; the match cannot reach the podium; a seat is permanently wedged with no supported recovery; **ANY pre-reveal word leak to a guesser** (spec §5 load-bearing property — always S1, however cosmetic the leak path looks); persisted avatar/theme destroyed. |
| **S2** | Wrong authoritative state | The game continues but the *truth* is wrong: scoring off the pinned formula, wrong turn order, canvas desync between seats, hint schedule wrong/missing, guessed-flag or roster wrong, timers drifting off wall-clock, reconnect losing state it should keep. **A recurrence of any `../known-issues.md` §1 regression-watch item is S2 minimum.** |
| **S3** | Degraded but workaroundable | A feature misbehaves but there is a path around it: an action needs a retry; a wrong-but-recoverable screen; a flow dead-ends but Back escapes; feedback missing so a player acts blind. Most UX-confusion that made you take a **wrong action** lands here. |
| **S4** | Cosmetic deviation from the reference design | Visual deltas vs the design bundle that don't mislead: wrong tint, off-spacing, wrong font on one label, missing wobble, misaligned badge. (A visual issue that *misleads gameplay* — e.g. the scoreboard showing wrong numbers — is **S2**, not S4.) |
| **S5** | Subjective polish / feel | Pacing gripes, delight opportunities, copy tone, animation wishes, "this would feel better if…". Never blocks a cycle; feeds the polish backlog. |

**Tie-breaks.** When torn between two levels, take the **higher** one and set
`confidence: low`. Harness artifacts (your own liveness stalls, a stale `ui.md`
read, a driver `NotFound` you caused by a typo) are **never** S1–S5 — put them in
the `## Harness notes` section of your report, not `## Findings`.

---

## Worked example (a real, well-formed finding)

```yaml
id: C1-S1-02
severity: S2
dimension: mechanics
screen: in_game
seat: 1 (Priya)
build: 56be0b0
turn: 1.3
repro:
  - "1. As guesser, I guessed 'ROBOT' correctly; ui.md countdown read 118 s-left."
  - "2. fracTimeLeft = 118/240 = 0.4917; I was guess order 0 (first correct)."
  - "3. Expected award = max(20, round((50 + 450*0.4917) * 0.82^0)) = round(271.3) = 271."
  - "4. Read my '— N pts' on the scoreboard before (0) and after (250)."
expected: >
  Guesser award = max(20, round((50 + 450*fracTimeLeft) * 0.82^order)) with
  order 0-based, fracTimeLeft = drawSecondsLeft/totalDraw (charter §1.B). For
  118 s-left of 240, order 0 => 271 points.
actual: >
  My score rose by 250, not 271. Either fracTimeLeft is being measured against a
  different total than the configured draw_seconds, or the award rounds down.
evidence:
  - findings/C1-S1-02.png                # scoreboard showing my 250 after the guess
  - "ui.md: '## Players — Priya — 250 pts (you, guessed ✓)'; countdown was 118 at guess"
confidence: medium                       # single observation; will re-measure next turn
known_issue: no
suspected_layer: dooduel_core
notes: >
  Not KI-01 (the scoreboard DID move, so the stuck-at-0 bug is not recurring) —
  this is an arithmetic delta, a distinct finding. Will log a second data point.
```
