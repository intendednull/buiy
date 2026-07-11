# Dooduel QA — Cycle 3 triage — ★ CLEAN CYCLE ★

**Date:** 2026-07-10 · **Build:** `67c981b` (all cycle-1/2 fixes in) · **Room:** 795PCA · **Config:** rounds=1, draw=240 / pick=120 / **reveal=30** (tightened from cycle-2's 60), bots=false · 4 LLM seats, **rotated archetypes** (seat-0 Finn=chaos, seat-1 Gwen=naive, seat-2 Hugo=host+visual, seat-3 Iris=mechanic).

**Result:** full match to podium, all invariants held. Podium **Gwen 675 · Iris 575 · Finn 559 · Hugo 388**. Words xylophone/sandwich/bicycle/volcano; every seat drew once; per-seat secrecy held; podium correct. The **firewalled naive seat (Gwen) won again**.

## ★ Disposition: CLEAN — campaign exit criterion MET

Per the runbook's cycle-clean criterion, all four conditions hold:
1. **Zero new S1–S3 findings.** The only S2 candidate (Hugo's C3-S2-01 sandwich scoring) was **RETRACTED by the reporter** after a deliberate repro; everything else is S4/S5 UX-polish or already-known. See below.
2. **Zero regressions.** Every cycle-1/2 fix re-verified HELD by ≥2 rotated seats (full tally below).
3. **All scripted probes pass** — including, for the FIRST time, the full destructive drawer set (Undo + Clear + OOB), all clean.
4. **Standing invariants hold** — every seat drew once; every word guessed or timed-out cleanly; per-seat word secrecy throughout; podium ordering correct.

**This is the first clean cycle → the campaign's primary exit condition ("until no issues are reported") is met at cycle 3** (before the ≥5 floor).

## The one S2 candidate — RETRACTED (not a bug)

**C3-S2-01 (Hugo) — a sandwich-turn guess that didn't score.** Hugo's `set_value`+Send returned Ok but he wasn't credited; drawer Iris got +67 (2/3). **Retracted by Hugo via repro:** on turn 4 (volcano) he deliberately guessed LAST → scored +93, was announced, drawer got full 3/3 → so the turn-ending guess scores correctly. The sandwich case ended on the **draw TIMER** (only 2 of 3 guessed — no early end), and Hugo's slow observe-act cadence landed his guess at/after the timer expiry → a correctly-rejected LATE guess. **Corroborated three ways:** (a) Hugo's own repro; (b) Iris's drawer-view (no "Hugo guessed" line ever appeared; +67 = round(100·2/3) is exact for 2/3); (c) the same match had a SECOND independent 2/3 turn (Iris herself missed xylophone to slow cadence). This is the recurring "slow agent cadence loses a guess to the turn-end timer" harness pattern (cycles 1–2 too), not a scoring defect. A parallel code-level adjudication of the turn-end/guess-processing tick order was run to confirm no timely-guess-drop race exists.

## Regression re-verify — ALL HELD (rotated seats, multiple confirmations each)
KI-01 (live scoreboard + badge-clear) · KI-02 (real chat messages render text at ~20+ rows) · KI-11 (proportional drawer payout — verified 2/3→+67 ×2, 3/3→+100 ×2, all exact) · KI-25 (no in-game theme toggle; present Home/Podium) · KI-26/27 (set_value guesses fold non-empty + submit) · KI-29 (countdown NUMBER ticks each poll, no phase-freeze — the framework render fix) · KI-30 (guess-draft clears on transition) · KI-31 (canvas blank at each turn boundary) · KI-33 (lobby room code upright body face — cycle-2's C2-04 fix). Mechanics audited SOLID (Iris): all four final totals reconcile as exact sums of per-turn awards; guesser 0.82^order decay; near-miss "So close!" private; post-lock rejection clean. **The corrected mechanic briefing worked: Iris verified the 1-based hint schedule (KI-32) as CORRECT (~100 s-left) and did NOT re-report the cycle-2 stale-briefing false positive.** KI-34 (deferred empty-pill framework artifact) present-as-expected, not re-filed.

## Chaos probes (Finn) — ALL PASS, incl. the first full destructive-drawer coverage
bad room code · wrong-phase Continue (KI-28 no-op) · post-guess re-submit (swallowed) · guess-spam ×5 (all broadcast, no drop/disconnect) · podium Play-again (graceful→Home, KI-16) · **Undo spam + Clear + OOB stroke** (front-loaded on the volcano draw — the coverage cycles 1–2 missed; all clean, authority rejects OOB, no crash/divergence).

## UX / polish + S5 (do NOT dirty a clean cycle; backlog)
- **Dead-time / no drawer-liveness feedback** — the naive seat's #1 gripe in BOTH cycle 2 (Dana) and cycle 3 (Gwen): every drawer opened with 40–75s of "Hang tight!"/blank canvas with no signal whether the drawer is active or idle. Largely AGENT latency (real players are faster), but the app-side mitigations are real: the AFK/idle-drawer early-out (**KI-07**) + drawer-active feedback. Elevated to the retrospective as the top UX theme.
- "Round 1 of 1" every turn → unclear match length (**KI-08**, known-open).
- Stray dashed rectangle overlapping the room-code field (Join + Lobby) — corroborates the already-logged S5 dashed-box field-inset follow-up (Gwen + F4).
- Home "▶ Play" (solo) more prominent than "Join a room" — UX hierarchy (known backlog).
- **No confetti observed** at podium or on correct-guess reveals (Hugo, S5 low-conf) — the `ConfettiPlugin` IS in the runtime, so likely an animated burst missed between static shots; **flag to verify** (a follow-up), not a confirmed defect.

## Evidence
`cycle-3/evidence/` — podium shots, the C3-S2-01 (retracted) sandwich vs bicycle-contrast, drawer-payout. Transcript: `cycle-3/server-transcript.log`.
