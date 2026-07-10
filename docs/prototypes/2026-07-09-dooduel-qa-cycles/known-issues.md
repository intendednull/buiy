# Dooduel QA — known-issues lookup (playtest suppression + regression watch)

**Date:** 2026-07-09 · **Status:** active · **Branch:** `feat/dooduel-multiplayer-m1`

The authoritative list for the QA playtest cycles. Playtest agents match every live
finding against this table BEFORE reporting: **§1** items must be actively re-verified
(a recurrence is S2 minimum); **§2/§3/§4** items must NOT be re-reported (they are
already logged, accepted, or deferred out of scope). If a §2 item looks materially
worse than logged, attach the new evidence to the existing id — do not open a new one.
Each item has a stable `KI-nn` id and a one-line symptom to match against. Updated
between cycles.

---

## §1 — REGRESSION WATCH (recently fixed — re-verify live; recurrence = S2 minimum)

| id | symptom to match | fixing commit(s) | how to re-verify live |
|---|---|---|---|
| **KI-01** | Live scoreboard `— N pts` stuck at 0 all match (only correct at podium); `guessed ✓` badge sticky (a seat that guessed once reads "guessed" every later turn) | `9eb2085` (broadcast `Roster` on score + turn changes) | Watch a seat's `— N pts` climb the instant it guesses (not only at the podium); confirm every seat's `guessed ✓` clears at each new turn's Picking. Check in BOTH the GUI scoreboard and the `dooduel_mcp` seat view. |
| **KI-02** | In-game chat: newest rows render as empty colored pills (bubble paints, text blank) once chat accumulates many messages | `90b1e44` `9181f63` `c592927` `fd7dc9e` (multi-page coverage atlas bind) | Play a long, chat-heavy turn (many guesses across scoreboard+word+chat sizes) until the working set crosses one atlas page, then confirm the NEWEST chat rows show real text, not blank pills. |

---

## §2 — KNOWN OPEN (do not re-report; add evidence only if severity is worse than logged)

| id | symptom to match | source |
|---|---|---|
| **KI-03** | Room code mixes ambiguous glyphs (`0/O`, `1/I/l`, `8/B`…); a read-aloud code (`SD1CI0`) is mis-entered → `RoomNotFound` | follow-ups.md "color-code digits vs letters in the room code — OPEN" |
| **KI-04** | Turn countdown (seat-view `~Ns left` line + GUI ring/label) jumps in discrete ~1 s steps instead of a smooth sweep | follow-ups.md "smoothly animate the turn countdown timer — OPEN" |
| **KI-05** | Lobby room code is a static label — can't drag-select / `Ctrl+C` it; only the "Copy" button works | follow-ups.md "Mouse text-selection on non-editable text — CHARTERED" (Copy button is the stop-gap) |
| **KI-06** | After a screen swap that despawns the focused widget, keyboard focus is lost (lands on window root) until the next Tab | follow-ups.md "Reconcile `FocusedEntity` when its target despawns — OPEN" |
| **KI-07** | An AFK/idle drawer (draws 0 ink) burns the FULL pick+draw timers — no drawer-idle early-out (no "no ink by T s → skip") | report 2026-07-04 §Findings ledger #1; VERIFIED-OPEN (§5) |
| **KI-08** | Turn header reads "Round 1 of 1 — X is drawing" and repeats identically every turn — no "Turn N of M" indicator | report 2026-07-04 §Findings ledger #3; VERIFIED-OPEN (§5) |
| **KI-09** | Home screen still says "No real opponents yet — this is a solo demo. Switch seats in-game to play everyone." (stale copy; SwitchSeat is gone from networked play) | `apps/dooduel/src/view/home.rs:79`; confirmed present (§5) |

---

## §3 — ACCEPTED QUIRKS / BY-DESIGN (never report)

| id | quirk | why accepted | source |
|---|---|---|---|
| **KI-10** | During Reveal, ANY seat's Continue advances the turn immediately (first-to-continue wins) | Accepted M1 private-room semantics; single consistent rule (§5, item C) | spec §3.2, §11 |
| **KI-11** | Drawer payout is a flat +100 regardless of guesser count/speed — the best drawer can finish last | Formula operates exactly as specced; a rebalance is a logged design question, not a bug | report 2026-07-04 §Findings ledger #2 (`dooduel-followup:drawer-payout-balance`) |
| **KI-12** | An empty-canvas turn resolves as a NORMAL scored round on hint reveals alone; the silent drawer still earns drawer credit | Open design question (whether hints may fully substitute for a drawing) — accepted for M1, not a defect | report 2026-07-04 §Findings ledger #1 (companion question) |
| **KI-13** | A WRONG guess that contains the secret word as a substring is broadcast literally in chat | Accepted M1 leak; censoring is M5 | spec §5.3 |
| **KI-14** | Connections are `ws://` only (no TLS) | `wss://` is deliberately M3 (cargo-deny landmine) | spec §6.2, §11 |
| **KI-15** | A FRESH Join while a match is running is rejected with `MatchInProgress` | M1 seats new players only in the lobby; mid-match seating is reconnect-only | spec §3.2, §11 |
| **KI-16** | No in-place "Play again" from the networked podium (must leave + re-create) | `StartMatch`-from-`Final` deferred to M2 | spec §11 |
| **KI-17** | Drawer vacates mid-Picking → the turn runs out its draw-window timeout (no new drawer draws) rather than skipping | Auto-pick timeout advances it; the skip-to-next-drawer fix is deferred | spec §11 |
| **KI-18** | In a 2-player room, the last guesser leaving mid-Drawing runs the draw window out before the turn ends | The lone drawer can't trigger an early end; immediate-end fix deferred | spec §11 |
| **KI-19** | MCP agents can only `join_room` — they cannot create a room | Agents are join-only clients (no `create` tool) | spec §7 |
| **KI-20** | A custom hand-drawn avatar is not reflected on other clients | Custom avatar not sent over the wire in M1 | spec §11 |
| **KI-21** | The turn timer RING steps once per second (not a smooth sweep) | MATCHES the design bundle — the design binds `strokeDashoffset` to a 1 Hz `setInterval` with no CSS transition; nothing owed (distinct from KI-04's smooth-polish wish) | final §5.f (finding #23) |
| **KI-22** | Podium places the winner CENTER + tallest (not the design code's literal 2nd-place-tallest) | Intentional documented deviation, grounded by the live skribbl.io podium | final §5.g |
| **KI-23** | Emoji (`🎨🎉🥇✏️✅👑…`) render as `.notdef` tofu / are dropped | Color emoji (COLR/CBDT/sbix) is deferred as its own future campaign | retro §6; final §5.f |
| **KI-24** | Radial-gradient backgrounds render flat; the rotated "free & open source" ribbon is dropped; per-axis elliptical wobble corners render circular | Character-layer residuals — attempt-if-cheap, defer-with-note; invisible at Dooduel's ±3px wobble | retro §6; final §5.f |

---

## §4 — OUT OF M1 SCOPE (never report — deferred features, not bugs)

| milestone | deferred features an agent might mistake for missing/broken |
|---|---|
| **M2** — rooms/settings | room-settings UI (rounds / draw-time / hints / max players), lobby settings panel, copy-invite-link polish, mid-match spectator/new-player join, in-place podium rematch, `MAX_SEATS` as a host setting |
| **M3** — matchmaking/hardening | public quick-match queue behind "▶ Play", TLS (`wss://`), per-IP limiter eviction, per-turn canvas op-count cap |
| **M4** — content depth | word modes (Normal / Hidden / Combination), custom word lists, languages, the full 20+ color palette, wire-encoding optimization |
| **M5** — social/moderation | guess-censoring (KI-13), votekick, like/dislike, live presence status, spoiler-safe post-guess chat |
| **M6** — transport | P2P (WebRTC data-channel) play |

Source: spec §1.2, §11.

---

## §5 — VERIFICATION NOTES (fix-status of the ambiguous 2026-07-04 items, checked on this branch)

The four game-polish items in the 2026-07-04 report were observed on the pre-M1 FINAL
app + the file-protocol `playtest_host`. The M1 branch reworked the authority into
`dooduel_core` (`Session` + pure `Game`), so these were re-checked against the new code.

- **A · AFK/idle-drawer early-out → VERIFIED-OPEN (KI-07).** No ink/idle/AFK concept
  exists. The Drawing-phase tick ends the turn only on the full timer or all-guessed:
  `apps/dooduel_core/src/game.rs:490-499` computes `draw_seconds_left` + reveal mask +
  `if elapsed >= total { end_turn() }`, with no stroke/ink check. The only forced end
  (`force_end_turn`, `game.rs:683`) fires solely for a *dropped* drawer past grace
  (`session.rs:628`), not an idle-but-present one. No fixing commit.

- **B · Repeating/stuck turn label → VERIFIED-OPEN (KI-08).** The header renders the
  round only, and no turn-index accessor exists to build a "Turn N of M" line.
  Mobile header `apps/dooduel/src/view/in_game.rs:122` → `text!("Round {} of {}", …)`;
  desktop `in_game.rs:319` → `"Round {} / {}"`. The "X is drawing" string is a per-round
  System chat banner (`game.rs:430`, `"Round {} of {} — {} is drawing"`), which repeats
  every turn within a round. No fixing commit.

- **C · Reveal→Continue contract → NOT REPRODUCIBLE IN CODE / single consistent rule
  (KI-10).** The M1 authority applies ONE uniform rule to every reveal: any seat may
  Continue and the first advances immediately, else the reveal timer auto-advances.
  `session.rs:535-540` (`on_continue`: gate `phase == Reveal`, then `game.continue_now()`,
  comment "Any seat may advance"); `game.rs:607` `continue_now` is a no-op outside Reveal;
  timer fallback `game.rs:501-507` (`REVEAL_SECS = 6`). Pinned by test
  `continue_gate_reveal_only` (`session.rs:1869`). The 2026-07-04 "unevenness" was seen
  on the file-protocol harness, not this core — re-verify live, but do not presume broken.

- **D · Drawer-side wrong-guess chat lag → NO GAP IN CODE (satisfied).** A wrong guess
  pushes a SHARED chat line (`game.rs:557` `push_chat(ChatKind::Guess, "{name}: {raw}")`,
  `to: None`) broadcast to `Recipient::All` — drawer included — in the same `resync` pass
  on the same tick (`session.rs:326` unconditional `resync`, chat diff broadcasts new
  shared lines at `session.rs:811-819`; only *private* near-miss nudges are seat-filtered).
  No per-recipient batch/digest exists. The reported "lag" was a `playtest_host` digest
  artifact, not the game logic — re-verify live, but do not presume broken.

- **Stale Home copy → CONFIRMED PRESENT (KI-09).** `apps/dooduel/src/view/home.rs:79`
  still renders "No real opponents yet — this is a solo demo. Switch seats in-game to
  play everyone." (SwitchSeat is removed from networked play per spec §11).

**Verdict summary:** A and B are genuinely open (no mechanism / no turn counter). C and D
are already satisfied by the M1 `dooduel_core` authority (single Continue rule; prompt
All-broadcast of shared wrong guesses) — no dedicated post-report fix, they were correct
by construction; their original symptoms are suspected harness artifacts and warrant a
light live re-check, not a bug report unless reproduced against the live server.
