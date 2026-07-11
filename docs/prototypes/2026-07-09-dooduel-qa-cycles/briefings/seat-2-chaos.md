# Seat supplement — Chaos / Robustness

> Read `COMMON.md` first. This adds your archetype's job on top of it.

You do **controlled damage**. You own the robustness matrix (charter §1.D) for the
probes reachable through the real UI + this driver. Every probe has a spec-pinned
expected outcome: **do it, compare, and file a finding only on a deviation.** A
probe that produced its expected outcome is a **PASS** — record it in your report so
the orchestrator can mark that probe green, but it is not a finding.

**Scheduling (charter §4):** at most **one** destructive probe per turn, and
**never** during another seat's scripted measurement (don't spam-guess while Seat 1
is timing a hint reveal — space them out). You still draw your own honest turn.

**Front-load your destructive drawer probes (cycle-1).** Your canvas-destructive
probes — Undo, Clear, the out-of-bounds stroke — run on **your** own draw turn, and
turns end fast once the other seats guess (in cycle 1 the chaos seat's turn ended
before it could run Undo/Clear). So the moment your drawing is legible enough to
guess, run those destructive probes **right away**, early in your draw window, before
the turn can end out from under you.

## Probes you run directly through your driver

| Probe | How | Expected (cite this in a finding) |
|---|---|---|
| **Bad room code** | *Before* joining the real room: click `"Join a room"` → `set_value` a garbage code (e.g. `"ZZZZZZ"`) → click `"Join room"`. Then join the REAL code. | `Error{RoomNotFound}`, a human-readable message, never creates a room (§3.2; the O/0 flavor is KI-03). |
| **Wrong-phase click** | As a guesser, try to `Continue` during Drawing, or guess during Reveal; as a non-drawer, look for a pick button (there is none → `NotFound`). | Honest `Error`/no-op, never a partial application or crash (§5.2). |
| **Undo/Clear spam** | On **your** draw turn: rapid repeated `"Undo"` then `"Clear"` clicks. | Every op resolves cleanly; your canvas never corrupts/crashes (§1.D). |
| **Out-of-bounds stroke** | On **your** draw turn: `stroke` with points outside `0..720 × 0..450`, e.g. `[[-50,-50],[900,600]]`. | Client clamps / authority rejects; canvas never diverges/crashes (§3.1). |
| **Guess spam** | As a guesser: many genuine guesses in quick succession. | All folded honestly; wrong ones broadcast; **no** disconnect (rate cap 240/s, §3.1). |

## Probes that need process control — coordinate with the orchestrator, then OBSERVE

You cannot spawn or kill processes yourself. Ask the orchestrator to run these, then
report only what **your** seat sees:

- **5th-process mid-match join** → expect `Error{MatchInProgress}` (KI-15, accepted).
  Verify it is *graceful*; do **not** file the rejection itself.
- **Leave / reconnect.** This driver has **no auto-reconnect** (spec §res-Q8): a
  dropped socket loses the seat for the match, and a fresh rejoin is `MatchInProgress`.
  So a "kill + restart within grace" probe **cannot** re-attach in v1 — that is
  expected, not a bug. You *may* click `"Leave"` yourself, but **only late** (after
  your honest draw turn), since it removes you from the match — coordinate the timing
  with the orchestrator so the match still reaches the podium with the remaining seats
  (occupancy ≥ 2 continues; §2.3.3 rotation skips the vacated seat).

## Known-issue traps to NOT mistake for bugs

- The podium `"Play again"` button is **present** on the networked podium, but
  in-place rematch is deferred (**KI-16**). Clicking it will **not** start a new
  networked match — accepted, don't file. **Do** file only if it **crashes** or shows
  a misleading state.
- Room-code glyph ambiguity (`O/0`, `1/I/l`) is **KI-03** (known-open) — don't
  re-file; if it bites you, that is expected friction.
