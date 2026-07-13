# Dooduel QA — Cycle 1 triage

**Date:** 2026-07-10 · **Build:** `e768f8b` · **Room:** VQPQTB · **Config:** rounds=1, draw=240 / pick=120 / reveal=60, bots=false · 4 LLM seat agents on the `qa_seat` GUI driver.

**Result:** full match to podium, all invariants held. Podium **Ada 804 · Priya 484 · Sam 432 · Theo 286**. Turns: OCTOPUS (Priya) → CUPCAKE (Theo) → PENGUIN (Ada) → BICYCLE (Sam). Every seat drew once; every word guessed or hint-carried; per-seat word secrecy held throughout; podium ordering correct.

**Disposition: NOT clean** — one corroborated S3 (the countdown-number freeze, app-vs-harness TBD), one S3 candidate (canvas not clearing between turns), one S4 (stale guess-draft across turns). Fix → cycle 2 (rotated archetypes re-verify).

Seat→archetype (cycle 1 identity map): seat-0 Priya = host+visual · seat-1 Theo = mechanic · seat-2 Sam = chaos · seat-3 Ada = naive. (Server seat indices differ by join order: server 0=Priya, 1=Theo, 2=Ada, 3=Sam.)

---

## Deduped findings ledger

### F1 — In-game countdown NUMBER freezes in screen.png while the model clock ticks — **S3, ADJUDICATE (app vs harness)** — ALL 4 SEATS
- **Symptom:** the drawing/picking countdown *number* in `screen.png` sticks at a phase-start value (240 whole drawing phase; 99 whole picking; 57) while `ui.md`'s authoritative clock ticks down correctly, then the number "teleports" to the right value the instant a guess lands. The ring arc keeps animating.
- **Corroboration:** Ada C1-S3-01 (biggest confusion), Sam C1-S2-04, Priya C1-S0-02, Theo (harness note). Independent, all 4 seats.
- **The split:** Ada — "ring moves but the number is frozen in the SAME frame → render-invalidation, not capture lag." Priya — "identical screen.png/ui.md mtime → real render-vs-model divergence." Theo/Sam — "almost certainly screenshot-capture lag; recommend a native check." The whole-phase freeze (not a ~1s lag) + ring-current-but-number-stale argues **real render invalidation**, but the driver's readback-vs-snapshot timing can't be excluded from reading alone.
- **Why it's load-bearing:** if `screen.png` (the pixel eyes) can go stale vs the model while `ui.md` stays correct, the entire VISUAL-QA lane is suspect. Must be resolved before trusting cycle-2 visual findings.
- **ACTION (Track A):** run the native windowed `dooduel` on DISPLAY, watch the drawing-phase countdown number. Live-freezes → real app/framework render-invalidation bug (fix it). Live-ticks-but-screen.png-freezes → a `qa_seat` offscreen-render/readback-staleness bug (fix the driver; the visual lane depends on it). Root-cause + fix test-first either way. Adjudicate F1b in the same pass.
- Evidence: `timer-frozen-240-vs-202.png`, `timer-stale-99-vs-54.png`, `C1-S2-04-screen57-true{13,22}.png`, `C1-S0-02-penguin.png`.

### F1b — Canvas does not clear between turns — **S3 candidate, ADJUDICATE with F1** — Ada
- **Symptom:** the previous turn's drawing lingers behind the "Hang tight!" card during the *next* drawer's Picking phase; clears only when the new drawer starts. Reproduced twice.
- **Likely** the same offscreen-staleness family as F1 (screen.png not reflecting a cleared canvas) OR a real "op-log canvas not reset until the new drawer's first op" behavior. Adjudicate on the live app alongside F1.
- Evidence: `canvas-not-cleared-between-turns.png`.

### F2 — Stale unsent guess-draft persists across turn boundaries — **S4, app** — Priya + Theo
- **Symptom:** an un-submitted guess left in the chat input (`value="penguin"` / `" OcToPuS! "`) persists across the reveal, the next Picking, and even the seat's own drawer turn — clearing only when a guess finally locks, never on turn transition.
- **Consequence beyond cosmetic:** it MASKS the phase placeholder ("Type your guess…" / "Round over") — Priya's phase-detector idled ~120s on it. So it degrades the readable phase cue.
- **Suspected root (Theo):** the every-frame-rebuild controlled chat input (KI-27 lineage) — the guess-draft **model field** (`chat_input`) is not reset on turn/phase transition. Distinct from the Track-3 placeholder fix (that fixed the placeholder text; this is the retained VALUE).
- **ACTION (Track B):** reset the guess-draft model field on turn/phase transition (app reducer). Small, test-first.
- Evidence: `C1-S0-03-penguin-0pts.png` (also shows drawer-0-pts, a separate UX note).

### F3 — KI-11 ledger is STALE: drawer payout is proportional, not "flat +100" — **doc fix, not a game bug** — Priya + Sam + Theo
- **Finding:** payout = `round(100 · correct_guessers / total_guessers)` — verified with 4 exact data points (2/3→+67, 3/3→+100, 0/3→+0, 3/3→+100). This MATCHES the pinned skribbl formula and fixes KI-11's own "best drawer can finish last" complaint. The game is correct; KI-11's "flat +100 regardless of guesser count" wording is wrong.
- **ACTION:** correct KI-11 in `known-issues.md` (and check the spec/follow-ups for the same stale phrasing). No code change.

### F4 — Dashed-box over the room-code field reads as a glitch — **S5, visual, verify** — Ada
- Ada (naive) read the dashed rectangle around the Join/Lobby room-code field as a rendering glitch. Likely the intended "sketchy" dashed-border aesthetic, but the box appeared offset/oversized vs the inner field in the W0 capture too. Verify intended-vs-glitch during Track A's live pass; likely S5.
- Evidence: `dashed-box-roomcode.png`.

---

## UX / polish backlog (S4–S5, not cycle-dirtying)
- "▶ Play" (solo demo) is visually more prominent than "Join a room", the button a networked player actually needs (Ada).
- The "7XQ2KP" room-code placeholder looks like a real pre-filled code (Ada).
- Drawer shows 0 pts until reveal — reads as "not scoring" (Priya, Ada).
- "Round 1 of 1" repeats every turn with no "Turn N of M" progress cue — **already KI-08 (known-open)**; cycle 1 re-confirms it hurts first-timer orientation.
- Dead-time / pacing: turns ran the full window after most guessed; the PENGUIN turn scored 0-0-0 on a legible drawing — but this is **agent cadence** (~40s/observe-act cycle), not a game defect (Theo, Priya). Real players are faster.

## Regression watch — ALL GREEN (fixes confirmed live, multiple seats)
KI-01 (live scoreboard climbs per-guess, badges clear each turn) · KI-02 (~15–17 chat rows, all real text, no empty pills — but only moderate volume; a heavier-chat run is a stronger atlas test) · KI-25 (no theme toggle in-game, Send unobstructed + submits) · KI-26/KI-27 (every guess incl. a rapid burst folded real values, never empty). Theme toggle fully works + persists Podium→Home.

## Chaos probes (Sam) — all PASS
bad room code (clean error) · wrong-phase Continue (NotFound, clean no-op) · late/wrong-phase guess (silent no-op) · guess-spam burst (all folded, wrong ones broadcast, no disconnect) · out-of-bounds stroke (Ok, no crash). NOT REACHED: Undo/Clear spam (Sam's only draw turn ended before he ran it — front-load destructive drawer probes next cycle).

## Mechanics audit (Theo) — SOLID, no anomalies
Drawer payout, guesser `0.82^order` decay, near-miss "So close!" private nudges (verified twice, never broadcast), literal wrong-guess broadcast, post-lock "You already guessed it!" (no double-score). Timer tracks wall-clock (the early "timer drift" read was Theo's own latency — false alarm).

---

## Harness / briefing improvements (for cycle 2 — not game findings)
1. **`screen.png` passive-lag:** always emit `{"cmd":"shot"}` and wait for its ack before an important visual read (Priya, Theo). Bake into COMMON.md.
2. **Phase detection:** key on word-slots / "Round over" / status text, NOT the chat placeholder — F2's stale value masks the placeholder (Priya). Update briefings.
3. **Cadence:** ~40s/observe-act cost easy guesses. Guessers should fire near-miss + correct back-to-back with no read between, or lock first (Theo). Tighten the loop guidance.
4. **Continue→NotFound is EXPECTED** (KI-10 reveal race) — add a known-issues note so seats stop filing it.
5. **Chaos scheduling:** front-load destructive drawer probes right after the drawing is legible (Sam).

## Fix plan
- **Track A** (priority): adjudicate + fix F1 (+F1b, +F4 visual verify) — live app vs `qa_seat` driver, root-cause, staged fix.
- **Track B:** F2 stale guess-draft reset (app reducer), staged.
- **Doc:** F3 correct KI-11; add the Continue→NotFound known-issues note; fold the 5 harness/briefing improvements into COMMON.md + the briefings.
- Then **cycle 2** (rotated archetypes: seat0=mechanic, seat1=chaos, seat2=naive, seat3=host+visual) re-verifies every fix by a different seat.
