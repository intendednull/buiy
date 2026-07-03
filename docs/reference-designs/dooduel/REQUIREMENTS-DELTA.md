# Requirements delta — audited *Scribbl* snapshot → archived *Dooduel* bundle

**What this is.** The [gap audit](../../reports/2026-07-01-scribbl-app-capability-gap-audit.md)
§1 ("What Scribbl is") described a **previous** design snapshot (files then named
`Scribbl - Game Spec.dc.html` + `Scribbl Prototype.dc.html`). This bundle archives the
**renamed + re-designed** target (**Dooduel**, game-spec v1.1). This file records only what
**changed** relative to that audited §1 description; areas that are the same are marked
*unchanged* and not re-described. It is part of the archive (design input, not a spec); the
Buiy build targets the bundle, and this delta is the reconciliation note.

Source of truth for the delta is the archived HTML/CSS/JS, **not** the two screenshots — see
the "Stale screenshots" note at the end.

---

## 1. Naming & branding — CHANGED

- **Scribbl → Dooduel.** Tagline "Draw, guess, repeat" / "Draw it. Guess it. Repeat! 😄".
  Positioning is the same: free/open-source skribbl.io clone, no account, 2–12 players,
  browser + desktop.
- **A full visual "character pass" is now specified** (Game Spec §13 decision 3 — marked
  *Decided*). The audit §1 said nothing about visual identity beyond "light/dark theme
  toggle." Dooduel locks a concrete art direction, and the prototype implements it:
  - single **purple** accent in both themes (see §7),
  - **hand-drawn type pairing** (Caveat + Shantell Sans; see §8),
  - **sketchy ink borders** — 2.5–3px solid `var(--ink)` outlines with **wobbly, asymmetric
    border-radii** (e.g. `26px 32px 24px 30px/30px 22px 32px 24px`) and dashed-border motifs,
  - **chunky "3D-press" buttons** (`box-shadow: 0 5px 0 var(--ink)`, press translates down and
    collapses the shadow),
  - **confetti + a podium finale**, and
  - **emoji-forward copy throughout** (🎨😄🎉🚪🏆✏️…).
  Note: this deliberately **breaks two Protokit house rules** — Protokit's readme says "No
  emoji in UI chrome" and ships a neutral, tool-like base; Dooduel overrides both on purpose.
  Protokit is the substrate; Dooduel is the skin on top.

## 2. Game rules — MOSTLY UNCHANGED (formulas now pinned)

- *Unchanged:* three-phase turn `pick → draw → reveal`; durations **pick 10s / draw 80s
  (configurable) / reveal 6s**; drawer rotates one turn per player per round; turn ends early
  once everyone has guessed; auto-pick a random word on pick-timeout and auto-advance on
  reveal-timeout; hints flip open at time thresholds; win = most points after all rounds.
- *Now concrete (was "time- and order-scaled" in the audit):*
  - **Guesser points** `= max(20, round((50 + 450 · fracTimeLeft) · 0.82^order))` — order is
    0-based guess order, `fracTimeLeft = drawSecondsLeft / totalDrawSeconds`.
  - **Drawer points** `= round(100 · correctCount / guesserCount)` (0 if nobody guessed).
  - Hints revealed = `clamp(hintCount, 0, letterCount−1)`, default 2; thresholds at
    `floor(totalDraw · (0.6 − i·0.18))` seconds-left.
- *Unchanged guess handling:* normalize `trim().toLowerCase().replace(/[^a-z0-9]/g,'')`; exact
  match locks + scores + hides from others; near-miss (Levenshtein small, length diff ≤ 2)
  fires a private **"So close! 👀"** toast.
- *New detail in the written spec (all deferred past MVP):* word modes **Normal / Hidden /
  Combination**; a room-settings table (players 2–20 **default 8**, draw 15–240s, rounds 2–10
  **default 3**, word count 1–5 default 3, hints 0–5 default 2, ~26 languages, custom word
  lists). MVP still ships Normal-mode English only. The interactive prototype's own rule knobs:
  rounds default **2** (1–4), draw 80s (30–150), hints 2 (0–3).

## 3. Screens & navigation — ELABORATED + one NEW surface

- *Unchanged flow:* Home → (Play public **or** Create-private → Lobby) / Join-code →
  In-game loop → Final podium; overlays for word-pick, turn-end reveal, "waiting for drawer",
  toasts, theme toggle.
- **Elaborations:** Home now has explicit **Create-a-room** and **Join-a-room** buttons plus a
  "You'll play with" mocked-roster preview; a dedicated **Join room** screen (code field,
  "any code works" in the solo demo); Lobby has a **copy-invite-link** control, a **host-gated
  Start** button, and a distinct **joiner "waiting for host"** state.
- **NEW surface: an Avatar editor** (modal / mobile sheet), reached from a ✏️ badge on Home —
  see §5. Not present in the audited §1.
- MVP screen set (Game Spec §11): Home, In-game, Word-pick, Turn-end reveal, Final podium =
  **MVP**; **Lobby = "Later"** in the spec, though the prototype already renders it.

## 4. Layout — UNCHANGED intent; prototype now ships both shapes (spec text lags)

- *Unchanged:* desktop is a 3-column grid **scoreboard | canvas | chat** (concretely
  `240px | 1fr | 300px`), a **fixed dark top bar** (`--ink-panel`, 60px) carrying the seat
  switcher; mobile stacks.
- **Worth flagging (internal inconsistency):** Game Spec §13 decision 4 says *"Desktop-first…
  No responsive/mobile pass in this prototype."* But the **prototype contradicts its own spec**
  — it ships a **full mobile variant** (`In-game (mobile)`, ≤430px stacked column: header card
  with a small timer ring, horizontally-scrolling scoreboard strips, ~280px canvas, stacked
  toolbar, 300px chat) selected by a `layout: desktop|mobile` prop, and it re-fits the canvas on
  layout change. Protokit's shell additionally provides desktop-sidebar↔mobile-bottom-nav reflow
  and **phone-card framing** (`@media (min-width:560px)` wraps the mobile app in a 420px device
  frame; full-bleed under 560px). **Net for the campaign: the mobile-web target is already
  designed, not deferred** — treat the spec's "desktop-only" line as stale.

## 5. Player / avatar model — SUBSTANTIALLY NEW (`DoodleAvatar`)

The audited model implied avatars as SVG **faces** ("customizable color, eyes, mouth" in the
full-game spec §9; letter-initial badges in the old screenshots). **Dooduel replaces this** with
a new `DoodleAvatar` component:

- A player's avatar is a **hand-drawn doodle icon on a tinted circular badge** (white 2.6px
  stroke, 1.5px dark ring, 40×40 viewBox). **22 icons**: cat, star, rocket, sun, cactus, dino,
  penguin, octopus, icecream, balloon, robot, ghost, cloud, mushroom, butterfly, fish, umbrella,
  snowman, owl, flower, turtle, heart — each composed from stroked SVG **paths/lines/circles/
  ellipses/dots** (exactly the primitive set the audit's capability matrix said `Icon` already
  renders statically).
- **Deterministic from the player name**: `icon = ICONS[hash(name) % 22]`,
  `tint = TINTS[hash(name+'-tint') % 10]` (10 muted tints). Same name ⇒ same avatar, no state.
- **Three ways to set your own** (Avatar editor): pick from a **gallery** (icon × tint grid),
  **draw your own** on a **220×220 paint canvas** (color swatches, 4 brush sizes, eraser, undo,
  clear → saved as an image `src`, persisted to `localStorage`), or reset to a random doodle.
- **Load-bearing consequence:** this introduces a **second paint surface** (the avatar canvas)
  beyond the game canvas — the drawing-canvas subsystem (audit G0) must serve **both**. Avatars
  are used everywhere: Home, seat switcher, scoreboard, lobby, reveal rows, podium.

## 6. Multiplayer / seat model — UNCHANGED

Still the **solo demo with a dev-only "playing as" seat-switcher** — one human seat-hops every
mocked player; canvas/chat/scores/timers are global state that runs regardless of the active
seat; **each turn auto-jumps the seat to the current drawer** (confirmed: `beginTurn` sets
`viewingAsId = drawer.id`); guessing is opt-in per seat; "same human already knows the word" is
an accepted quirk. No bots, no real networking (all deferred). This is identical to audit §1 and
Game Spec §13 decisions 1–2.

## 7. Theming — SHARPENED (purple, light-default, persisted)

- *Unchanged idea:* light/dark + accent, always-toggleable (toggle now pinned bottom-right).
- **Now specified precisely** (was flagged in the audit as a light-theme stub + no persistence):
  - Full Protokit light **and** dark token ladders ship (surfaces, ink, hairlines, `--pos/
    --warn/--danger` + tints, an always-dark `--ink-panel` hero used by the in-game top bar,
    `--glass-*` bar fills).
  - **Accent overridden to purple by inline JS**: light `#7C4FE0` (press `#6438C2`, tint
    `#ECE3FB`), dark `#A78BFA` (press `#C9B8FF`). Protokit's own default is indigo `#3a63ee`;
    Dooduel swaps it. (The Game Spec doc chrome uses a slightly different purple `#5b46e5`; both
    are "the purple," minor.)
  - **Default theme is LIGHT** (`getInitialTheme() → 'light'`), and theme + custom avatar are
    **persisted to `localStorage`**. Confirms the audit's G7 (light theme + toggle + persistence)
    as a real requirement, not optional.

## 8. Typography — CHANGED (hand-drawn faces over Geist)

- Protokit base is **Geist / Geist Mono** (body at the 450 "book" weight; mono for numbers,
  codes, timestamps, the uppercase eyebrow). *That still underlies numeric/label text.*
- **Dooduel promotes hand-drawn display faces**: **Caveat** (600/700) and **Shantell Sans**
  (400–700) loaded on top, driven through `--font-display` (which Protokit intentionally leaves
  as its own swappable var). Headlines, scores, timers, buttons render in the display face; the
  playful, marker-like feel comes from here. (Fonts load from Google Fonts — a wasm/offline
  concern for the Buiy build to plan for.)

## 9. Animation / motion — token base UNCHANGED; app adds playful motion

- *Unchanged token base:* one primary ease `cubic-bezier(.32,.72,.32,1)` (+ ease-out/ease-in),
  durations `.12 / .18 / .28 / .36s`, `translateY(1px)` presses, `prefers-reduced-motion`
  respected.
- **App-level additions** (Protokit says "no infinite decorative loops"; Dooduel adds a few):
  `bounce-dot` waiting loaders, `score-flash-up` (+points floats off the scoreboard),
  `party-pop-in` dialog entrances, and a real **canvas particle confetti** system (6 colors;
  bursts of ~46/64 on correct guesses and reveal, ~130+90+90 at the final podium).

## 10. Drawing canvas / tools — brush + FILL now (bucket added), rest UNCHANGED

- *Unchanged:* freehand brush, eraser (paints white, 1.6× width), **4 brush sizes** `[3,6,11,18]`,
  a **16-color palette**, **undo** (snapshot ring) and **clear**; pointer-driven; canvas is an
  HTML5 2D canvas in the prototype.
- **NEW: a Fill / bucket tool** — the toolbar is now a 3-way Segmented **brush / fill / eraser**,
  and the prototype implements a real stack-based **flood-fill** (`getImageData` → match →
  fill). The audit §1 listed only brush/eraser; **fill is net-new** and (like the avatar canvas)
  pushes on the paint subsystem's requirements (region read-back / pixel ops, not just strokes).
- Palette (16): `#14161b #ffffff #9aa0aa #b3261e #e8453f #f08a3c #f4c20d #8bc34a #1c8a52 #1f9e8d
  #2f9bdb #3a63ee #5b46e5 #9333ea #e0529c #8a5a35`. Full 20+ palette is deferred.

## 11. Other new / removed

- **New:** confetti subsystem (§9); avatar editor + avatar paint canvas (§5); flood-fill (§10);
  copy-invite-link + host/joiner lobby states (§3); a 52-word built-in pool with no-repeat
  selection; light-default + persistence (§7).
- **Removed / not carried:** the letter-initial avatar badges and the "face with eyes/mouth"
  avatar direction (superseded by `DoodleAvatar`); the plain Geist-only look (superseded by the
  character pass).
- *Deferred (explicit "full game", not MVP):* real networking/rooms/invite, public matchmaking,
  word modes, custom word-list editor, 20+ colors, non-English languages, votekick, like/dislike,
  accounts/profiles/stats.

---

## Stale screenshots — READ THIS

`screenshots/violet-theme.png` and `screenshots/mobile-check.png` **do not depict the current
Dooduel design.** Both render the **pre-rebrand snapshot**: the H1 still says **"Scribbl"**,
avatars are **letter-initial badges** (M/P/T/S in flat colored circles, not `DoodleAvatar`
doodles), buttons are flat (no 3D-press), the type is Geist (no Caveat/Shantell), and the accent
reads blue/indigo — despite the "violet-theme" filename. They are only loosely useful (layout
skeleton, the in-game top-bar + word-slots + scoreboard arrangement). **For visual parity use the
archived HTML/CSS/JS, which is the current target; do not treat these PNGs as ground truth.**

## Note on AI-agent-directed content (per the DesignSync security convention)

Scanned the whole bundle for text that instructs an AI/agent or attempts prompt-injection.
**None of malicious or injection character was found.** The only agent-addressed prose is
Protokit's `_ds/.../readme.md`, which opens "*Protokit is a scaffold… it gives a design agent
everything needed…*" and lays out voice/copy and visual house rules — ordinary design-system
usage guidance aimed at whoever builds a demo, not an instruction to *this* agent. The companion
`_adherence.oxlintrc.json` is a design-adherence **lint** config (closed prop/variant enums);
`_ds_manifest.json` is component/token metadata. Reported for transparency; **not** followed as
instructions — and note the readme's own rules (no emoji, neutral base) are ones Dooduel
deliberately overrides.
