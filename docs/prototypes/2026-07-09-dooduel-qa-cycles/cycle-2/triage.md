# Dooduel QA — Cycle 2 triage

**Date:** 2026-07-10 · **Build:** `cfad5f8` (all cycle-1 fixes in) · **Room:** WGMBNQ · **Config:** rounds=1, draw=240 / pick=120 / reveal=60, bots=false · 4 LLM seats, **rotated archetypes** (seat-0 Blair=mechanic, seat-1 Cruz=chaos, seat-2 Dana=naive, seat-3 Emmy=host+visual).

**Result:** full match to podium, all invariants held. Podium **Dana 956 · Cruz 816 · Emmy 596 · Blair 187**. Words OWL/KITE/ROBOT/ANCHOR; every seat drew once; per-seat secrecy held; podium correct. Notably **the firewalled naive seat (Dana) won** — playing engaged without stalling.

**Disposition: NOT clean** — 3 new pre-existing findings (S2 hint-schedule, S3 empty-pills, S4 lobby-font) that cycle-1's lighter play didn't surface. **ZERO regressions** — every cycle-1 fix re-verified HELD by a different seat than found it.

---

## ★ Regression re-verify — ALL cycle-1 + prior fixes HELD (rotated seats)
Re-verified live by ≥2 seats each (Emmy visual + Blair mechanic + Cruz + Dana naive):
- **KI-29 (framework countdown render fix — the cycle-1 headline): HELD** — countdown NUMBER ticks each second in screen.png, matches ui.md, no phase-long freeze. Confirmed by all 4 seats. **The Track-A framework fix works end-to-end through the driver** (the re-verify Track A deferred to cycle 2).
- **KI-30 (guess-draft clears): HELD** — Emmy's clean isolated probe ("zztest" cleared to placeholder on transition).
- **KI-31 (canvas blanks on turn boundary): HELD** — Emmy ×2, Cruz.
- **KI-25 (no in-game toggle) / KI-26 / KI-27 (set_value folds non-empty): HELD** — all seats.
- **KI-01 (live scoreboard) / KI-11 (proportional payout): HELD + CONFIRMED** — 8 award data points across 4 reveals match the formula exactly; KI-11 proven proportional again (2/3→+67, 3/3→+100).
- **KI-02 (real messages render): HELD** — every named message showed text (the empty pills are a DISTINCT bug, see C2-02).
- **KI-28 (Continue→NotFound expected): HELD** — clean no-op, not filed.
- Mechanics audit SOLID again: guesser `0.82^order` decay, near-miss "So close!" private nudges, messy-input normalization, undo/clear tools, post-lock protection — all correct.

## Findings ledger (new)

### C2-03 — Hint schedule off-by-one: hints reveal ~44s LATE — **S2, dooduel_core** — Blair (mechanic)
- Measured across OWL/ROBOT/ANCHOR: first hint at ~100 s-left (not the design's 144), second at ~57 (not 100). `game.rs:452` loops `for i in 1..=hint_count` with `secs_left=total*(0.6-i*0.18)` → i=1,2 → 100/57. The `0.6` base (i=0's value) is never used as a threshold — a 0-based formula looped 1-based. Skews the early game "help-starved" (multiple seats felt it).
- **ACTION (Track C2-A):** adjudicate vs the design spec (0-based intended → fix the loop to `0..hint_count`; else correct the charter doc). Off-by-one fix expected. Test-first, replay-safe.

### C2-02 — Spurious empty "guessed" chat pills at high volume — **S3, view** — ALL 4 SEATS
- At ~20 chat rows, TWO empty light-green pills render at the chat tail with **NO model text** (ui.md empty). **NOT KI-02** — the disambiguating fact (Blair + Emmy): every REAL/named message rendered its text fine; these pills carry no content. So it's spurious phantom rows, not lost content (no S2 information loss). Only appears at high chat volume (cycle-1's shorter chat never showed it).
- **ACTION (Track C2-B):** investigate the chat-feed view/model — empty-content messages pushed? phantom keyed-list rows? Root-cause + fix (app if app-rooted; STOP+report if framework-rooted).
- Evidence: `empty-chat-pills-a/b.png`, `C2-S1-emptypills-A.png`, `C2-S3-01-empty-chat-pills.png`.

### C2-04 — Lobby room-code font wrong (Caveat, not Geist Mono) — **S4, view** — Emmy (visual)
- The Lobby invite room code renders in hand-drawn italic Caveat; room codes are pinned to Geist Mono (the in-game top-bar code is upright/mono — an inconsistency).
- **ACTION (Track C2-B):** fix the lobby code font token to the mono token. Small.
- Evidence: `C2-S3-02-lobby-roomcode-font.png`.

## Not-a-bug / withdrawn / known
- **C2-01 (Dana "stale panel header"):** NOT a bug — the right "Guess the word" panel is the scrolling CHAT LOG, not a persistent subtitle; "Round 1/1 — Emmy is drawing" is a historical chat line and "…Blair is drawing" correctly appears below it. The "Round 1/1" repeat is the known **KI-08**.
- **Reveal ~60s window (Cruz C2-S1-02):** WITHDRAWN by Cruz — it's THIS cycle's configured `reveal_seconds=60`, not the default 6s. Config, not a defect. (Tuning note: 60s is a lot of dead-time; tighten reveal in future cycles and rely on Continue.)
- **Countdown 78→212 "jump" (Dana):** expected per-phase timer reset (each phase has its own duration), not the KI-29 freeze.
- **Light→dark entry (Dana):** the in-game always-dark ~60px top bar (on-spec), read as jarring by a first-timer — UX-polish note.

## UX / polish backlog (S4–S5, not cycle-dirtying)
- Dead-time waiting on slow/AFK agent seats — the naive seat's #1 annoyance ("what would make a real person tab away"). Largely AGENT-latency (harness), but the app could shorten dead time (drawer-idle early-out = KI-07; tighter reveal). Real-player pacing is faster.
- Home "▶ Play" more prominent than the "Join a room" a networked player needs; "solo demo" copy contradicts joining friends (KI-09 known); numbered-only color swatches ("Color 5" = orange, not obvious); no confetti observed at podium (likely animated, unverified from static frames).

## Harness notes (cycle-3)
- The `shot`-before-visual-read + phase-via-status lessons worked (joins were faster, phase detection cleaner). Keep.
- Chaos drawer-probe coverage STILL incomplete: the 2/3-guessed draw-timer clamp (123s→~22s) ends the drawer turn fast, so Clear/OOB didn't run again. Next chaos seat: batch ALL destructive probes (Undo+Clear+OOB) in ONE burst on the first legible stroke, before others guess.
- Reveal 60s → consider tightening to ~25-30s for cycle 3 (less dead-time; Continue still short-circuits).

## Fix plan
- **Track C2-A:** C2-03 hint schedule (dooduel_core) — adjudicate + fix, staged + reviewed.
- **Track C2-B:** C2-02 empty pills (investigate view) + C2-04 lobby font — staged + reviewed.
- Then **cycle 3** (rotate: seat0=chaos, seat1=naive, seat2=host+visual, seat3=mechanic) re-verifies these + all prior fixes; tighten reveal timer.
