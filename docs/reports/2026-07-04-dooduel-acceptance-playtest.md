# Dooduel FINAL — acceptance playtest & campaign close-out

**Date:** 2026-07-04 · **Status:** acceptance run COMPLETE — bar met
**Campaign:** the Dooduel whole-app prototype-first campaign
(charter `docs/prototypes/2026-07-01-scribbl-campaign-charter.md`; spec
`docs/specs/2026-07-03-dooduel-final-design.md`; plan `docs/plans/2026-07-03-dooduel-final.md`).
**Evidence:** `2026-07-04-dooduel-acceptance-playtest-assets/` (match chat, host log,
final state, per-seat player reports).

## Verdict

**The campaign's acceptance bar is met on the FINAL build.** Four LLM agents, each
playing a different seat through the graduated `playtest_host` file protocol, played a
complete real match of the production `apps/dooduel` (main @ `24fa997`, PR #139):
every seat's word was guessed by at least one opponent, art recognition worked from
`canvas.png` alone, the hint system carried a degraded turn, per-seat honesty held
(no secret-word leaks; private near-miss feedback stayed private), and the App-2
gameplay fixes were observed working live. All reporting players rated the match
genuinely fun.

## The match record (1 round · 4 turns · ~35 min wall-clock)

| Turn | Drawer | Word | Outcome |
|---|---|---|---|
| 1 | Alex (seat 0) | RAINBOW | drew 6 nested ROYGBIV arcs + clouds; **all 3 guessed** (the only all-guessed word) |
| 2 | Priya (seat 1) | CASTLE | crenellated towers + walls + gate; Theo guessed mid-drawing |
| 3 | Theo (seat 2) | SAXOPHONE | brass body/U-bend/bell/keys, iterated against canvas.png; Priya guessed from a ~9k-ink partial |
| 4 | Sam (seat 3) | GUITAR | **drawer went silent (0 ink)** — the hint reveals (`_ _ I _ _ R`) carried the turn; Theo + Priya still guessed |

**Podium:** Priya 924 · Theo 912 · Sam 294 · Alex 100 (a 12-point title race decided
~4s before the final buzzer — Priya's wrong "spider" then "guitar").

## What the run verified (against the campaign goal)

- **Multi-agent playtest, each agent a different player** — the goal's literal bar. Four
  independent agents; honesty contract enforced (each read only its own per-seat view +
  shared chat/canvas/state).
- **Draw → recognize loop:** three drawn words recognized from incremental stroke art
  alone; the drawer could iterate by re-reading its own canvas (players called this the
  core loop "genuinely engaging").
- **App-2 fixes live:** the drawer watched `guessed_count` climb + green rows arrive
  mid-turn (bug 3 fix confirmed by seat 0); a wrong guess ("Priya: spider") surfaced in
  shared chat (bug 4); hint letters revealed on the design schedule and rescued a stalled
  turn (bug 5); wall-clock countdowns (bug 6); no stale drawer at the podium and the
  round label clamps (bugs 1–2).
- **Graceful degradation:** an entirely silent drawer did not wedge the match — hints +
  the turn timer completed the turn, guessers still scored.
- **The host protocol held:** funnel-routed commands with honest rejections (wrong-phase
  `continue`/`guess` rejected and logged, never crashing), per-stroke canvas streaming,
  per-seat view refresh.

## Findings ledger (new items from this run)

**Game polish (app-level follow-ups):**
1. **AFK/idle-drawer handling** (highest player-priority): a non-drawing drawer burns the
   full pick+draw timers (~10 min dead wall-clock here). Wanted: drawer-idle early-out
   (e.g. no ink by T seconds → skip) analogous to the existing all-guessed early turn end.
   Companion design question (seat 3): the empty-canvas turn resolved as a NORMAL scored
   round on hint reveals alone, and the silent drawer still earned drawer credit - decide
   intentionally whether hints may fully substitute for a drawing and whether an idle
   drawer should score.
2. **Drawer payout balance — live evidence for the pre-named follow-up
   (`dooduel-followup:drawer-payout-balance`):** Alex's perfect turn (all 3 guessed) paid
   the design-literal 100 while guessers banked 300–450/word; the best drawer finished
   last. The formula operated exactly as specced — the spec's own balance caveat now has
   match data.
3. **Turn label:** "Round 1 of 1 — X is drawing" repeats for all 4 turns and reads stuck;
   a "Turn N of M" line would fix it (display-only).
4. **Reveal→continue contract is uneven:** `can_continue` was offered on some turns and
   timer-elapsed on others; players found it confusing. Define one rule (e.g. everyone can
   continue during every reveal; majority advances early).
5. **Drawer-side wrong-guess visibility:** the drawer sees guessed-rows live, but seat 2
   reports shared wrong-guess rows lagging into the drawer's chat digest during its own
   turn. Verify the drawer's per-seat chat includes shared wrong guesses promptly.

**Agent-harness notes (playtest infrastructure, not the app):**
- Seat agents idle after single actions; a match needs one persistent poll-act loop per
  agent across ALL turns (the orchestrator had to metronome via idle notifications).
  Seat 3's silent draw turn was an agent-liveness artifact, not an app failure.
  Root-caused first-person (seat 3): its background condition-poll EXITED on time
  (turn 2, can_guess=true) but the completion notification reached the agent batched at
  match end, so it missed 3 of 4 turns. This is the same lost-background-wake harness
  bug that stalled build lanes throughout the campaign; agent players need an
  event-driven "your move" push (or long FOREGROUND poll loops), never
  background-task wakes.
- Long poll loops hit the 2-minute default bash timeout; players worked around with
  explicit timeouts.

## Framework follow-ups ledger (accumulated over the whole campaign)

Candidates for the post-campaign backlog, in rough priority order:
1. **Glyph tier ignores stacking (F10 candidate, headline):** one global glyph tier draws
   all text after all quads — a top-layer modal cannot occlude base-layer text. This
   blocked the spec's avatar-editor-as-modal re-decision (App-1 reverted to the
   prototype's full-screen editor). Fix directions: per-stacking-context glyph
   partitions, or top_layer composites its subtree as a group.
2. **`icon()` press route:** the `Kind::Icon` reconcile arm skips `apply_pressable`, so
   `.on_press` on an icon is silently inert (App-1 worked around with pressable
   containers). Wire the icon arm like the raster arm.
3. **`on_submit_with` × ticking models:** the reconciler re-patches an uncontrolled
   `text_input` value every tick, clobbering in-progress typing — F7's capturing submit
   is unusable on a ticking model without a controlled input (composition gap; App-1
   kept the controlled `on_input` pattern).
4. **`raster()` styling lowering:** `Kind::Raster` skips `apply_border`, so
   `.radius()`/`.border()` on a raster element do not lower (App-1 stamps `Border` via an
   `Added<RasterImage>` observer for the round avatar).
5. **Effect-group-nested rasters drop** (F4a documented boundary): the off-screen group
   pass composites quads+glyphs only — a raster inside a translucent/blurred top-layer
   panel disappears; App-1's editor panel must stay opaque.
6. **Toast widget (audit G6):** the private near-miss nudge is a chat pill because no
   transient-toast subsystem exists; a real auto-dismiss Toast + stack remains open.
7. Smaller: per-axis elliptical band corners (invisible at ±3px wobble), radial-gradient
   backgrounds, the rotated ribbon decoration (`.rotate()` enabler shipped in F4b),
   color-emoji COLR (deferred as its own campaign, spec §5.f).

## Web milestones (evidence: `…-assets/web/`)

Run against `apps/dooduel/dooduel_web` headless (Chromium; SwiftShader WebGL2 + WebGPU):

1. **Dual-backend wasm build — PASS** (trunk, webgpu + webgl2 features, dev artifacts 116/121 MB).
2. **Boots + renders Home on BOTH backends — PASS** (full Home card + violet CTA, zero console
   errors; WebGPU also cleared the strict Tint shader gate).
3. **F9 both-knobs HiDPI gate — PASS at dsf 2 AND 3** (phone 390x844: backing == CSS x dpr,
   logical == CSS, no overflow) — confirms F9 for the app itself.
4. **Dynamic-resize transient — REPRODUCED, real and persistent in headless:** resizing after
   boot confines rendering to a sub-region (hard dark chrome band on the grown edge,
   900x640 -> 560x900 leaves the right edge 100% dark; never settles through t=20s), while a
   FRESH LOAD at the identical size renders correctly (1.7% edge) — the fault is exclusively
   the post-boot resize path (dpr=1, not a dpr artifact). Headless resize emulation is not
   fully faithful (per the F9 report), so the REAL-DEVICE gate must confirm real-bug vs
   emulation-artifact — logged in the follow-ups ledger as the surface-reconfigure item.
5. **Interaction folds on BOTH backends — PASS** (theme-toggle click folds SetTheme:
   card white -> dark, pill Light -> Dark; Play click navigates Home -> word-pick).

## Campaign accounting (Phase B FINAL)

Fifteen PRs merged to main in the FINAL (2026-07-03 → 2026-07-04): lineage docs #126;
framework F7 #127, F2 #128, F8 #130, F1 #131, F5 #132, F3 #133, F6 #134, F4a #135,
F9 #136, F4b #137; app waves App-1 #138, App-2 #139 (+ the externally-merged #129), and the acceptance docs PR carrying this report closes the series.
Every framework feature shipped general-purpose and was dogfooded by the app; every
wave carried red-first or byte-stability verification; the acceptance run above closes
the loop the prototype opened.
