# Seat supplement — Host + Visual Auditor

> Read `COMMON.md` first. This adds your archetype's job on top of it.

You are the **host** of this match and the campaign's **visual QA** — the eyes for
every pixel. You create the room, gate the start, and sweep every screen against
the reference design.

## Host flow (do this first)

1. On **Home**, click `"Create a room"` — you become host in the lobby.
2. Read the **room code** from `ui.md`'s `--- text & layout ---` section (it is bare
   text — spec §res-Q3) and confirm it against `screen.png`.
3. **Publish it immediately** so the others can join — write it to the shared file
   the orchestrator gave you the path for:
   `printf '%s\n' "<CODE>" > <shared_dir>/room-code.txt`.
4. Wait in the lobby until the roster shows **all** expected players (the orchestrator
   tells you how many seats), then click `"▶ Start game"` (host-gated).

## Per-screen visual sweep (before + during the match)

Ground truth is the archived design bundle, not the stale PNGs (charter §1.A). Sweep
`home / join / lobby / in_game / reveal / podium / avatar_editor` and file `S4` for
cosmetic deltas, `S2` if a visual issue *misleads gameplay* (e.g. wrong scoreboard
numbers). The pinned identity checks:

- **Accent — single PURPLE.** Light `#7C4FE0` (press `#6438C2`, tint `#ECE3FB`),
  dark `#A78BFA` (press `#C9B8FF`). **Default theme is LIGHT.** No other accent hue.
- **Theme toggle** — pinned **bottom-right, always available** on every screen; its
  name is state-dependent (`"Dark"` while dark, `"Light"` while light) and **flips on
  click** (spec §3.1). Click once, **re-read `ui.md`** (never click the same name
  twice), confirm the whole app recolors and stays legible in both themes, and that
  the choice persists.
- **Typography** — hand-drawn **Caveat (600/700) + Shantell Sans** for headlines /
  scores / timers / buttons; **Geist Mono** for numbers, room codes, timestamps. A
  code or score in the wrong face is a finding.
- **Character pass** — sketchy 2.5–3 px `--ink` outlines with wobbly asymmetric radii;
  chunky **3D-press** buttons (`box-shadow 0 5px 0 --ink`, press-down collapse on
  click); rounded pills/cards; **confetti** on correct guesses + podium.
- **Avatars** — `DoodleAvatar`, **deterministic from player name**: the same name must
  render an identical avatar on Home, scoreboard, lobby, reveal rows, and podium.
- **In-game layout (desktop)** — 3 columns `240px | 1fr | 300px` (scoreboard | canvas
  + toolbar | chat); fixed **always-dark 60 px top bar**; word-slot row; 16-swatch
  palette; 4 brush sizes; 3-way Brush/Fill/Eraser toolbar.
- **Do NOT file** the 1 Hz timer-ring step (KI-21, matches the design), the smooth-
  countdown wish (KI-04, known-open), or the winner-center-tallest podium (KI-22).
- General drift: clipped/overlapping text, wrong radii, missing borders, mis-centered
  overlays — check at **both** themes.

## Your regression-watch assignments (known-issues §1 — re-verify LIVE)

- **KI-02 (chat empty-pills) — the single most valuable probe this cycle.** Seat 1
  drives chat volume high; **you** confirm, late in a chat-heavy match, that the
  **newest** chat rows show **real text**, not blank colored pills. Inspect
  `screen.png` directly — a blank pill is a bubble painted with no glyphs inside.
  **Recurrence = S2** (regression), file with `known_issue: yes:KI-02`.
- **KI-01 (live scoreboard).** Watch a seat's `— N pts` climb the **instant** it
  guesses (not only at the podium), and confirm every seat's `guessed ✓` badge
  **clears** at each new turn's Picking. You own the *visual* read of the scoreboard
  (Seat 1 recomputes the arithmetic). Recurrence = S2.

## Also

- **Avatar editor** (charter §4): open it via the ✏️ badge on Home; check the preset
  gallery renders, the draw-your-own scratch surface, and reset. Custom avatars don't
  cross the wire (KI-20) — don't file "others don't see my custom avatar".
- **Play your own turns honestly** (draw / guess) and report GUI feel like every seat.
