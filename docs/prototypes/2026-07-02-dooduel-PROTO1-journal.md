# Dooduel — Prototype 1 Dev Journal

> **PROTOTYPE — exploratory, DO NOT MERGE.** The deliverable is this journal +
> the retrospective. Code is an unmerged reference for the FINAL's audited port.

**Goal:** build the ENTIRE Dooduel app (skribbl.io-clone draw-and-guess game) to
**exact design parity** — native desktop + web-desktop + web-mobile — developing
the Buiy framework features it needs inline (prototype quality), to learn what
the FINAL must build and what the dev experience of Buiy-as-a-game-UI is.
**Three products:** the app, the framework features, and DX feedback — journal
every issue, classified `framework-bug` / `missing-feature` / `DX-friction` /
`app-bug`, plus `game-seam` notes (Bevy game logic × Buiy UI).

**Worktree:** `worktree-dooduel-proto1`, off origin/main `a969cbf` (2026-07-02).

**Target/reference:**
- Design bundle (match exactly): `docs/reference-designs/dooduel/` — on the
  campaign docs branch `worktree-scribbl-campaign` (with `REQUIREMENTS-DELTA.md`).
- Charter: `docs/prototypes/2026-07-01-scribbl-campaign-charter.md` (same branch).
- Capability ground truth: `docs/reports/2026-07-02-dooduel-rebaseline-audit.md`
  (same branch) — G1/G2 delivered; G3 tick/Subscription is the lone Tier-1 gap;
  canvas is the sole net-new subsystem (GPU-side de-risked, reserved `Path`
  slot, RT machinery exists); playtest = probe eyes + pointer-harness hands +
  a never-proven stroke helper.

**Acceptance for the campaign's FINAL** (Phase A proves the path): design parity
+ verified working + **multi-agent playtest, each agent a different player**.

## Wave plan (learning-ordered: biggest unknowns first; re-order freely)

- **W0 skeleton** — `examples/dooduel` app crate; `ui()` Home-screen stub boots
  under `BuiyPlugin`; probe snapshot + offscreen screenshot harness working.
  Retire early: can a view subtree live under (or beside) a hand-authored shell?
- **W1 canvas spike (THE decide-by-prototype item)** — textured-node instance
  kind + CPU-authoritative paint buffer + dirty-region upload; brush strokes via
  `Pointer<Press/Drag/Release>`; eraser; flood fill. Decides accumulation-RT vs
  Path-channel. Proven by drawing with a real mouse + a GPU readback screenshot.
- **W2 game core (game-seam wave)** — MVU game model + phase machine
  (pick10→draw80→reveal6), `Res<Time>`→`enqueue(Tick)` driver, scoring formulas
  from the spec, word lists, mock players + solo seat-hop, guess
  normalize/exact/Levenshtein.
- **W3 screens: Home / Join / Create / Lobby** — protokit tokens (purple accent,
  light default), DoodleAvatar (22 doodle icons via Icon/lyon, name-hash), fonts
  (Caveat + Shantell Sans as .ttf).
- **W4 in-game screen to parity** — 3-pane layout (shell strategy from W0), top
  bar + seat-switcher, header (round/role/letter-slot hints/timer ring), canvas
  + toolbar (brush/fill/eraser, colors, sizes), scoreboard, auto-scroll chat.
- **W5 podium + confetti + avatar editor** — tween confetti burst; the 220×220
  draw-your-own avatar canvas (2nd consumer of W1).
- **W6 sketchy character pass + theming** — wobbly per-corner radii, 3D-press
  buttons (verify `LineStyle::Dashed` renders), light/dark toggle + persistence
  (web localStorage / native file; also persists custom avatar).
- **W7 web + mobile** — wasm build (dual backend), touch drawing, soft
  keyboard/IME on a real mobile browser, responsive shell (sidebar ↔ bottom
  nav, phone-card framing).
- **W8 playtest infra** — unified headless driver (probe + PointerHarness +
  net-new stroke helper), multi-agent playtest (drawer = harness agent,
  guessers = probe agents; shared-world sim first).

Every wave: **run the artifact** (native GUI where possible; probe
`snapshot_report` + offscreen GPU screenshot as the headless analog), then
journal. Web-mobile checks at W7 + any milestone touching input/layout.

## Running log

### 2026-07-02 — W0 skeleton
- Built: `examples/dooduel` (lib + windowed bin + headless GPU-capture bin,
  mirroring `counter_view`): one MVU model (`Dooduel { screen, player_name }`),
  `Screen::{Home, Lobby}` kind-swap in `view`, `ui()` install, probe smoke test
  (boot → snapshot → `click(Play)` → assert Lobby).
- Ran the artifact → found: **compile + probe test green FIRST TRY off nothing
  but AGENTS.md** (strong DX signal — the front door is accurate). Offscreen GPU
  captures (`home.png`/`lobby.png`) render correctly — text sizes, placeholder,
  buttons all paint. BUT: **the whole UI is pinned to the viewport's top-left**
  [missing-feature] — `.align_center()` centers children within the column's own
  width; there is no view-surface way to size/fill/center the ROOT in the
  window (re-baseline's "no sizing/justify modifiers" limit, now visually
  confirmed). Every Dooduel screen is viewport-centered in the design → this is
  the first framework feature the prototype forces (root fill + justify/center,
  or a proven mount-under-ECS-shell pattern).
- Surprised by / friction: probe + view + capture harness compose with zero
  fighting; `text!` / `on_press` / kind-swap all behave as documented.
  rust-analyzer flagged the new crate "unlinked" (stale-analyzer artifact —
  cargo is the truth). Dark theme in captures is the harness default; Dooduel's
  light-default flips at W6.
- If we did this again: same shape. Next: retire W0's second question (view
  subtree under a hand-authored shell?) BEFORE W4; W1 canvas spike first.

### 2026-07-02 — W1 canvas spike (render primitive + paint)

**Strategy decision (decide-by-building): CPU-authoritative RGBA buffer + a new
textured-node primitive.** The app owns a bevy `Image` (`Rgba8UnormSrgb`,
`RenderAssetUsages::all()`), paints into its CPU `data`, marks it changed; the
framework's new `RasterImage(Handle<Image>)` samples that texture onto a node's
layout rect. Rejected, with evidence:
- **GPU accumulation render-target** (stroke quads → an RT) — rejected: flood
  fill would become a GPU readback (it needs to *read* pixels), and undo/serialize
  for the FINAL want the buffer on the CPU anyway. The only thing the RT buys is
  avoiding the re-upload, which is a perf refinement we don't need at 720×450.
- **Reserved `Path` vector channel** — rejected: a paint canvas is fundamentally
  raster (freehand pixels + flood fill + eraser); flood-fill and eraser are
  *semantically wrong* on a vector scene, and no Path shader exists. `Path` stays
  reserved for real vector art.
  The CPU choice paid off exactly as predicted: flood fill is a ~40-line bounded
  scanline, eraser is a background-color stamp, brush is circle-stamp +
  line-interpolation — all pure, all unit-tested, all trivial.

- **Built (framework, `buiy_core`):** `render/raster.rs` + `raster.wgsl` — the
  `RasterImage` component, a `RasterInstance` (48 B) record, a distinct
  `BuiyRasterPipeline` (own shader + vertex layout, `@group(0)` view uniform +
  `@group(1)` per-node texture + a Nearest sampler), and its extract→prepare→draw
  glue. Followed the **band/gradient precedent — a distinct pipeline keyed by
  record, NOT a new `BuiyPrimitiveKind`** (the closed enum + the byte-stable quad
  path are untouched; the reserved `Path` slot is left for vector art). Wired
  `raster` into `BuiyViewPipelines` / `BuiySpecializedPipelines` and a fill-tier
  draw section in `buiy_pass`. [missing-feature — delivered; the app's ONE net-new
  subsystem]
- **Built (app, `examples/dooduel`):** `paint.rs` — the CPU paint state
  (`PaintCanvas`), `stamp_circle` / `stroke_segment` (line-interpolated) /
  `flood_fill` (stack scanline), eraser, Press/Drag/Release observers mapping the
  pointer to canvas-local pixels, and a `sync_canvas_to_image` mirror. The canvas
  is a **separate hand-spawned layout root**, decoupled from the MVU `ui()` root.
- **Ran the artifact** (`capture_canvas`, real GPU readback, in `target/dooduel-captures/canvas.png`):
  painted a synthetic red squiggle, a blue box flood-filled green, and a yellow
  bar with an erased notch → **every sample byte-exact** (red `[220,40,40]`, green
  `[40,180,80]`, blue `[40,90,220]`, yellow `[240,200,40]`, erased→paper
  `[255,255,255]`). The RasterImage primitive, the CPU→Image mirror, and the paint
  math all work end-to-end on the GPU.

Findings:
- **[framework-bug / game-seam — THE W4 risk] Paint order is by primitive TIER,
  globally, not per-stacking-context.** The raster draws in the fill tier; ALL
  glyphs draw in a later tier for the WHOLE view. So the Home screen's TEXT paints
  *over* the canvas wherever they overlap (visible on the left of canvas.png),
  while Home's quad backgrounds paint *under* it — regardless of which root/stacking
  context each belongs to. Cross-root / cross-context ordering is not expressible
  across the raster tier. For the in-game screen this means the canvas cannot be
  freely layered with UI chrome (e.g. some text below it, toolbar above) unless the
  canvas lives inside ONE stacking context with that chrome, or the compositor
  grows per-context passes. **Decide in W4:** the canvas must be *inside* the MVU
  view tree's stacking context, which forces the next question ↓.
- **[missing-feature — W4 blocker] No `view`-level way to place a `RasterImage`.**
  W1 hand-spawned the canvas as an ECS root *outside* the MVU tree (the only option
  today). The in-game screen wants the canvas *in* the 3-pane view layout. So W4
  needs either a `buiy_view` raster element, or a documented "mount an ECS subtree
  into a view slot" escape hatch (exactly W0's deferred second question — now
  load-bearing). This is the real coexistence gap; the "second root" trick is only
  a spike shortcut.
- **[framework-bug — latent] View-uniform coupling to the quad tier.** The view
  uniform is written only on a `quad_dirty` frame (`prepare_buiy_instances`), so a
  *node-less* raster-only view would never get one and wouldn't render. It works
  here only because the canvas node itself lands in the extract node list (as a
  `Color::NONE` node), flipping `ExtractedNodesView` on frame 1 → the uniform is
  written once and retained. A truly bare raster view is a latent gap; a fix would
  hoist the view-uniform write out of the quad gate.
- **[finding — good] `RenderAssetUsages::all()` makes the CPU loop correct.**
  Confirmed against bevy 0.19's `extract_render_asset`: `all()` (MAIN_WORLD |
  RENDER_WORLD) *clones* to the render world and the main-world `data` survives, so
  we keep painting into it every frame; `Assets::get_mut` fires `Modified` →
  re-extract → re-upload. (`RENDER_WORLD`-only would `data.take()` and the buffer
  would vanish after frame 1 — a trap for the FINAL to avoid.)
- **[finding — good] sRGB round-trips byte-exact.** `Rgba8UnormSrgb` canvas image
  + Nearest sampler + the sRGB view attachment → readback pixels equal the authored
  sRGB bytes exactly (no MSAA smear on solid interiors). Authoring colors as plain
  sRGB `[u8;4]` "just works".
- **[perf watch-item] Full-buffer re-upload per dirty frame** (~1.3 MB at
  720×450×4). Fine for the prototype; the FINAL should consider dirty-rect partial
  upload (or the GPU-RT accumulation we rejected) if a continuous drag at 60 Hz on a
  weak machine shows up in the frame budget. Not measured yet.
- **[DX — positive] The render spine is beautifully factored for extension.**
  Adding a whole new sampled primitive was mechanical: mirror band/gradient
  (instance record + `SpecializedRenderPipeline` + specialize in
  `register`/`prepare_buiy_view_pipelines` + a draw section), reuse the atlas
  `@group(1)` layout shape, done. `AGENTS.md`-level accuracy again — the pattern
  documented itself through the existing code.
- **[DX — minor friction] The monolithic `buiy_pass`.** A new primitive touches it
  in three spots (the empty-frame early-skip, the bind-groups-before-the-pass dance
  — device borrow vs. open pass, the `composite_bindings` precedent — and the tier
  draw). Discoverable but a new author must learn the "build bind groups before
  `begin_tracked_render_pass`" rule.
- **[finding] Coexistence mechanics are free.** A second layout root gets
  `ResolvedLayout` + `GlobalTransform` + picking with zero wiring; Buiy nodes are
  pickable by default (no `Pickable` needed), so the Press/Drag/Release observers
  are ready for the live windowed run.

Surprised by / friction: the sRGB exactness (expected ±1–2 from the 8-bit
linear↔sRGB round trip; got 0). The tier-paint-order behavior was the real
surprise — I expected the second root to compose as a unit; instead the render
pass dissolved both roots into global per-tier batches. That's the single most
important thing W1 taught for the game's architecture.

If we did this again: **build the canvas into the MVU view tree from the start**,
not as a side root — the side-root shortcut proved the primitive but hid the W4
layering problem for one wave. Resolve "how does a raster node enter a `view`"
(the deferred W0 question) *before* W4, because the in-game screen can't be laid
out without it.

**Open risks handed to W4 / W5:**
- **W4 (in-game screen):** (1) the tier-paint-order limit + (2) no view-level
  raster placement are coupled and blocking — the canvas must sit in one stacking
  context with its toolbar/header/chat. Decide the mount pattern first. (3)
  `to_pixel` reads `GlobalTransform + ResolvedLayout` so it *should* compose under
  the 3-pane shell, but is untested under scroll/scale.
- **W5 (avatar editor):** low risk — it's a 2nd `RasterImage` at 220×220. The
  primitive is size-agnostic and `RasterBuffers` already holds a *list* (N canvases
  each get their own image + `@group(1)` + draw), so two live canvases compose by
  construction. Smaller re-upload; Nearest + 1:1 mapping are ideal for a tiny
  avatar. The only open question is two canvases + the tier order (same as W4).

### 2026-07-02 — W2 game core (the game-seam wave)

**Built (app, `examples/dooduel`):**
- `game.rs` — the **pure game core** with zero framework coupling: the `Game`
  struct (whole match state), the `Phase` machine (`Idle → Picking → Drawing →
  Reveal → Final`), the pinned scoring formulas (`guesser_points` =
  `max(20, round((50+450·frac)·0.82^order))`, `drawer_points` =
  `round(100·correct/guessers)`), `normalize` + `levenshtein` + `is_close`
  (≤2 edits, ≤2 length diff), the 52-word pool with no-repeat selection, the hint
  schedule (`floor(total·(0.6−i·0.18))` seconds-left thresholds), and seeded
  deterministic bots. Every rule is a `&mut self` method — unit-testable with no
  ECS/GPU/clock.
- `lib.rs` — grew the MVU model to `Dooduel { screen, player_name, game: Game }`,
  the `Msg` enum (nav + `Tick(Duration)` + in-turn), a thin `update` shell over
  the `Game` methods, the `InGame`/`Podium` screens (header, blanks+hints word,
  scoreboard-as-seat-switcher, `on_submit` guess box, `keyed_column` chat log,
  pick-phase word buttons), and `GameClockPlugin` (the `Res<Time>` → `Msg::Tick`
  driver).

**Ran the artifact** (24/24 `-p dooduel` green; clippy+fmt clean). The probe
integration test boots GPU-free, navigates Home→Lobby→StartMatch→ChooseWord,
anchors the draw clock, hops the human to seat 1, and submits the exact word —
`snapshot_report` then shows the live screen (score **500** to the guesser, the
`★ 🎉 … guessed the word!` chat line, the full word `B I C Y C L E` visible to
the knower, blanks to others):
```
size=336x27 text="Round 1/2 — Drawing — 80s"
size=336x36 text="B I C Y C L E"
Button "Priya — 500 ✓  [you]" …
size=336x29 text="★ 🎉 Priya guessed the word!"
```

**Findings (classified):**
- **[game-seam — THE headline, positive] The `Tick(now)`-fold pattern is
  pleasant AND is the *right* design, not a workaround.** A plain
  `Res<Time>` system enqueues `Msg::Tick(now)` every frame; the reducer derives
  *everything* — countdowns, hint reveals, phase timeouts, bot fires — from
  `now − anchor`. It mirrors the blink perf fixture exactly, so the load-bearing
  perf property comes free: I store only DERIVED values (seconds-left change
  once/sec; the phase anchor changes once/phase; `reveal_mask` recomputed
  idempotently), never raw `now`, so a steady frame folds to a byte-identical
  model and `set_if_neq` absorbs it (no rebuild). The game logic is a *pure
  function of the Tick stream.*
- **[game-seam — the insight worth stealing] "Re-anchor on the next tick."**
  Every phase transition sets `phase_started_at = None`; the next `tick` stamps
  it to `now` and reads elapsed 0. This makes `Game::tick` the **sole owner of
  the clock**, so a transition triggered by a plain `Msg` (which has no
  timestamp — `StartMatch`, `ChooseWord`) and one triggered by a timeout (which
  does have `now`) share one path. No `now` needs to be threaded into button
  handlers. Cost: a ≤1-frame anchor delay before a countdown starts — invisible.
- **[game-seam] Bots fold through the same funnel as humans.** `tick` returns a
  `Vec<PendingGuess>`; `update` maps them to `Cmd::Batch(vec![Cmd::emit(Msg::Guess
  {…}), …])`. So a bot guess is a *real message* run-to-completion in the same
  drain, hitting the identical `Game::apply_guess` path a human `SubmitGuess`
  does. Idempotency is free: a correct guess locks the seat in `turn_guesses`, so
  the tick's `!already_guessed` filter fires each bot exactly once. This is a
  clean demonstration of `Cmd::Emit` as "the tick schedules an action."
- **[game-seam — testability win] The clock is swappable because it's a driver,
  not the logic.** `GameClockPlugin` (wall-clock) is deliberately *separate* from
  `install()` (pure logic), so tests drive the match with **injected virtual
  time** — `Msg::Tick(Duration::from_secs(n))` at chosen `n` — with zero
  wall-clock flakiness and instant full-match simulation. This answers the wave's
  "how do you advance `Res<Time>` headlessly?" question: *you don't* — you inject
  Ticks. (Unit tests skip the ECS entirely and call `Game::tick` directly.)
- **[missing-feature — the known G3 gap, re-characterized] No Subscription /
  timer API.** I *wanted* a declarative `Cmd::interval(1s, Msg)` /
  `Cmd::timeout(dur, Msg)` and hand-rolled it via Tick-folding. But building it
  taught me the wished-for shape is subtler than "a timer": a fired-once timer is
  *edge-triggered* (hard to replay, hard to keep `set_if_neq`-clean), whereas the
  poll-from-`now` fold is *level-triggered* (replayable, idempotent, perf-free).
  So the real G3 ask is **"a runtime-provided poll clock source as a Msg"** —
  which `GameClockPlugin` already is in 6 lines. The gap is ergonomic (every app
  re-hand-rolls the driver + the anchor arithmetic), not a capability hole. A
  framework `Cmd::tick_every`/a `ClockPlugin<M>` that enqueues `M::tick(now)`
  would remove the boilerplate without changing the (correct) poll semantics.
- **[DX-friction] `on_submit(msg)` can't carry the submitted text.** Unlike
  `on_input(fn(String)->Msg)`, `on_submit` takes a *static* `Msg`, so the typed
  guess must round-trip through a model field: `on_input → Msg::SetChatInput` →
  `on_submit → Msg::SubmitGuess` (which reads `chat_input`). Works, and it
  matches the editor's command-sourced model, but a `on_submit_with(fn(String)->
  Msg)` would delete the two-message dance.
- **[app-bug — mine, found by TDD] Two integration tests over-ticked past the
  reveal.** My first draft swept a full 80s draw window, but the game correctly
  *ends turns early* once all guessers are correct (~60s) and auto-advances
  through the 6s reveal into the next turn — so the assertion `phase == Reveal`
  failed because the game had moved on. The fix was to tick *until* the phase
  leaves `Drawing`, which is really the game working as designed. (Plus a bogus
  `is_close("rocket","robot")` — that pair is edit-distance 3, not 2.)
- **[decision — deviation from the archived design, journaled] Bots replace the
  pure seat-hop.** The design's solo demo has the human embody the drawer and
  *manually* seat-hop to guess for everyone; there are **no bots**. W2 keeps the
  auto-jump-to-drawer + `SwitchSeat` faithfully but **adds deterministic bots**
  as the other seats' guessers, so the match is *self-driving* — required for
  both a demoable single-human game and the campaign's multi-agent-playtest goal
  (each agent later drives a seat). Bots currently always guess correctly once
  (seeded fire time in the `[0.25,0.75]` draw band); wrong/near-miss bot chatter
  is a deferred knob. Also: `total_rounds` defaults to **2** (the interactive
  prototype's value), not the game-spec's 3 — a `Config` knob either way.
- **[finding — one-giant-reducer holds up] The nested-`Game` + thin-`update`
  split scaled cleanly.** ~10 reducer arms, all one-liners delegating to pure
  `Game` methods; the whole match machine is testable without the funnel. No
  sign of strain at this size. The single `Msg` enum carrying nav + clock +
  in-turn is still legible.

**Surprised by / friction:** how *little* the clock fought me — I expected to
need a timer subsystem and instead the blink pattern (built for a perf gate)
turned out to be the whole answer for real game timing. The one genuine snag was
`on_submit`'s missing text payload. The bot-as-emit path Just Worked on the first
run once the idempotency-via-`turn_guesses` insight clicked.

**If we did this again:** same shape. The Tick-fold + separate-clock-driver +
bots-as-emits trio is the load-bearing architecture; I'd lift `GameClockPlugin`
into a reusable `ClockPlugin<M>` on day one rather than hand-rolling `drive_tick`.

**Handed to W3 / W4:**
- The game core + a *minimal* in-game screen exist; **W3/W4 restyle to parity**
  (protokit tokens, 3-pane layout, top bar, timer ring). The current screen order
  is header → word → (pick buttons) → scoreboard/switcher → guess box → chat →
  Leave.
- **The canvas is still NOT in the view tree.** W2 added zero render surface —
  the game logic doesn't need one. W4 must still resolve W1's two coupled blockers
  (no `view`-level `RasterImage` placement + global tier paint-order) to put the
  drawing canvas *inside* the in-game stacking context. Nothing in W2 changed that
  calculus.
- `Game::word_display()` already returns blanks / hints / full-word-for-knowers;
  `scoreboard` doubles as the seat switcher (`SwitchSeat(i)`); `phase_label` +
  `countdown` feed the header. W4 can restyle against these accessors without
  touching the machine.
- **Deferred for later waves:** toasts, confetti, score-flash animations (visual,
  W5-ish); richer bot behavior (wrong/close chatter, personalities); the room-
  config knobs (rounds/draw/hints sliders); already-guessed players seeing the
  word is handled, but the drawer's "you're drawing, here's the word" affordance
  is just the header for now.

### 2026-07-02 — W3a view layout growth + canvas-in-view

The FRAMEWORK wave that unblocks the parity screens (W3) and the in-game screen
(W4): the `buiy_view` sizing/main-axis surface, the raster *element* (canvas in
the view tree), and the paint-order fix so overlays paint OVER a canvas. Prime
FINAL-candidate code — the `buiy_view`/`buiy_core` parts are kept clean.

**Built (framework, `buiy_view`):**
- **Sizing + main-axis modifiers on `Element`:** `.width(px)` / `.height(px)`
  (→ `BoxModel.width/height = Sizing::Length(Px)`), `.fill()` (→ both axes
  `Percent(100)`), `.grow()` (→ `FlexItem.grow = 1`, inserted on demand — `Node`
  does not `#[require]` `FlexItem`), `.justify_center()` / `.justify_between()`
  (→ `FlexParams.justify_content`, via a 3-variant `Justify` facade). Overlay
  authoring: `.top_layer()` (→ `Stacking.top_layer = Popover`) + `.fixed()`
  (→ `Position::Fixed`). All patched drift-only (`set_if_neq` / `!=`-guard),
  mirroring how `gap`/`padding` patch.
- **`raster(handle, w, h)` element** (`Kind::Raster`): reconciles to a `Node` +
  W1's `RasterImage` + fixed size. Handle patched **by identity** (`RasterImage`
  is not `PartialEq`), so an unrelated fold never re-uploads the texture; the
  entity is preserved (a reconciler test asserts the canvas entity + handle
  survive an unrelated model patch). [missing-feature — W1's W4 blocker #1, delivered]

**Built (framework, `buiy_core` render — the paint-order fix):** the raster tier
now **splits at the top-layer boundary** so an escaped top-layer overlay paints
OVER a canvas while the canvas still paints over its own in-flow ancestor/sibling
backgrounds. `extract_buiy_nodes` records the first top-layer painter
(`Stacking.top_layer`, read from the existing `NodePaintQuery` fan — no new system
param) as `ExtractedNodes.top_layer_node_start`; `prepare_buiy_instances` maps it
to a quad-instance boundary (`top_layer_quad_start`) via `node_quad_anchors`;
`node.rs` splices a `FlatDrawStep::Rasters` marker there through the extended
`interleave_flat_quads_gradients_rasters`. [framework-bug — W1's W4 risk, fixed]

**Ran the artifact** (`capture_w3a`, real GPU readback, 3 PNGs in
`target/dooduel-captures/`): **centered Home** (title rect center_x=512≈480,
top_y=182 — probe-verified + eyeballed); **canvas-in-view** (a red-flooded canvas
renders as a `raster(...)` element below the header/word); **overlay-over-canvas**
(the SAME canvas pixel is ink `[220,40,40]` while Drawing and overlay-blue
`[91,134,245]` while Picking — the top-layer scrim + "Pick a word" panel cover the
inked canvas). All three assertions + eyeball pass.

**Findings (classified):**
- **[missing-feature — delivered; the thinnest sizing surface] `Sizing` maps to
  four tiny methods.** The whole surface Dooduel needs is `.width/.height`
  (fixed), `.fill()` (100%), `.grow()` (flex-grow), `.justify_*`. No `Auto`,
  `min/max`, `stretch`, per-axis-fill, or a `Length` enum leaked into the view —
  `f32` logical px + one `Justify` facade is the whole vocabulary. Percentages
  are ONLY reachable via `.fill()` (the one case an app needs) — the app never
  writes `Percent`. This is the right altitude: the surface exposes *intents*
  (`fill`, `grow`, `center`), the reconciler owns the `Sizing`/`FlexParams`
  lowering.
- **[DX — positive] The reconciler did NOT fight.** Every new modifier is a
  `set_if_neq`/`!=`-guarded write in `apply_container_props`, folding into the
  existing `changed` count exactly like `gap`/`padding`. `.grow()` was the only
  wrinkle — `FlexItem` isn't a `Node` `#[require]`, so it is *inserted on demand*
  and toggled OFF by writing `grow = 0.0` (kept present, drift-safe — no
  `RemovedComponents` dependence). The raster kind slotted into the `spawn_node` /
  `patch_node` match with no special-casing.
- **[decision — raster ELEMENT beat mount-under-ECS-shell] Why the element won.**
  W0's deferred second question was "view subtree under a hand-authored shell, or
  a raster element?" The element won decisively: (a) it is ONE `Kind` + one
  reconciler arm (mechanical), vs. an escape-hatch that would need the reconciler
  to *not* reconcile a foreign subtree (a hole in the "view owns the tree"
  invariant); (b) the canvas gets `keyed`/`when`/lifecycle for free (it despawns
  on screen-swap, respawns on re-entry — `wire_canvas_node` re-homes the observers
  via `Added<RasterImage>`); (c) the handle rides the model, so the canvas is
  *part of the replayable MVU state*, not a side channel. The mount-under-shell
  hatch is only warranted for a genuinely foreign render surface (a 3D viewport),
  which a raster canvas is not.
- **[game-seam — the glue the rewire took] The canvas handle round-trips through
  the funnel.** `view(&Model)` can't touch `Assets<Image>`, so the handle MUST
  live on the model. The paint plugin creates the `Image` at `Startup`, then a
  6-line `announce_canvas` system enqueues `Msg::CanvasReady(handle)` once (a
  `Local` latch) — the handle folds in as a normal message. Observers bind to the
  reconciler-owned node via a separate `Added<RasterImage>` system (`commands
  .entity(e).observe(...)`), NOT `.observe()` at spawn (the app doesn't spawn the
  node). Net: 3 small systems (`setup_canvas` / `wire_canvas_node` /
  `announce_canvas`) replaced W1's one-shot root spawn. The stroke math is
  byte-for-byte untouched.
- **[framework-bug — FIXED, the headline] Paint order is now per-top-layer, not
  purely per-tier.** W1's finding was "the raster tier draws globally between
  quads and glyphs, so an overlay's quad backdrop paints UNDER the canvas." The
  fix draws in-flow quads → raster → **top-layer quads** → glyphs. Verified
  byte-identical for every existing view (the split is a no-op when there is no
  raster, and `top_layer_quad_start == u32::MAX` keeps the raster after all
  quads when there is no top-layer — 6 new interleave unit tests + 5 GPU goldens
  confirm). Only a raster + top-layer view changes, which is exactly the overlay
  case.
- **[framework — RESIDUAL paint-order limit, journaled precisely] The split
  assumes top-layer forms a CONTIGUOUS quad suffix.** `top_layer_node_start` is
  the FIRST top-layer painter; the boundary is its first quad instance. This is
  exact for ONE `ui()` root whose top-layer members escape to its `painters_z`
  tail (layout 6f) — the Dooduel case, and the P1 single-root `ui()` constraint
  guarantees it. It would be WRONG only if top-layer painters interleaved with
  in-flow ones across MULTIPLE stacking-context roots (an in-flow root assembled
  *after* a top-layer one): an in-flow quad after the boundary would then draw
  over the raster. Not reachable under one `ui()` root. Also: a NON-top-layer
  overlay (an in-flow sibling positioned over the canvas) still draws UNDER the
  canvas — overlays MUST be `.top_layer()`. The FINAL should re-decide toward the
  *general* fix: interleave rasters into the flat quad draw by each raster node's
  OWN `node_quad_anchor` (the exact gradient-bleed precedent), so a raster paints
  at its true stacking position with no top-layer special case.
- **[missing-feature — overlay POSITIONING, surfaced] `.fixed()` resolves against
  the ROOT content box, not the viewport.** A `.fixed().fill()` scrim fills the
  layout root's content box (minus its padding), NOT the ICB — so a full-viewport
  scrim needs a viewport-filling, unpadded root, and the fixed element lands at
  the root content ORIGIN (here `(24,24)` — the root's padding), not `(0,0)`.
  Buiy has no `inset`/`Absolute`-with-inset surface in `buiy_view` yet. For the
  W3a proof this is fine (the scrim covers the whole canvas, just not the padded
  window corner); W4's 3-pane shell + a `.inset()`/absolute-position modifier is
  the real fix for a pixel-perfect centered modal.
- **[finding — the latent view-uniform/quad_dirty bug does NOT bite] confirmed
  not reachable.** W1 flagged "a node-less raster-only view never gets a view
  uniform." But a `raster(...)` element IS a `Node` (it carries `RasterImage` on
  a layout node), so it lands in `ExtractedNodes` → `nodes.is_changed()` → the
  uniform writes. A raster with NO `Node` can't exist (`extract_buiy_rasters` is
  `With<Node>`). Left the hoist undone (nothing to fix); noted so the FINAL
  doesn't chase a phantom.
- **[DX — minor] No snapshot brittleness hit.** The buiy_verify gallery layout
  snapshots + the buiy_core headless suite passed unchanged (416/203/169/314 +
  verify 48/9/206) — the extract/prepare field additions are additive and the
  node.rs interleave is byte-identical, so no entity#/layout snap regen was
  needed. (The feared `INSTA_UPDATE` churn never materialized.)

**Surprised by / friction:** the paint-order fix landing byte-identical was the
pleasant surprise — framing it as "splice a `Rasters` step into the existing
gradient interleave at a boundary" (not "reorder the tiers") made it a pure
extension: `None`/`u32::MAX` boundaries reproduce the old draw exactly, so the
whole risk collapsed to "is the boundary computed right." The friction was the
overlay POSITIONING (fixed-vs-viewport), not the paint order — proving D took a
`.fixed()` that resolves against the padded root, so the scrim covers the canvas
but not the window corner. That's a layout-surface gap, orthogonal to the
paint-order win.

**If we did this again:** build the raster as a view element from the start
(W1's side-root shortcut hid the layering problem for a wave). And reach for the
**per-raster-anchor interleave** (the general fix) rather than the top-layer-suffix
split — the split is correct for Dooduel but the anchor interleave is the same
code shape the gradient tier already uses and carries no contiguity assumption.

**Handed to W3 / W4:**
- **W3 (parity screens):** the sizing surface (`.width/.height/.fill/.grow/
  .justify_*`) + `.top_layer()`/`.fixed()` are ready. Home/Lobby/Podium center via
  `.fill().justify_center().align_center()`. Still MISSING for pixel-parity: an
  `inset`/absolute-position surface (centered modals, anchored popovers), per-side
  padding/margin, `flex-shrink` control (a `.fill()` root flex-shrinks fixed-size
  children — the canvas squished 450→134 under `.fill()`; W4 wants the canvas at a
  fixed size inside a scroll/flex region, so a `.shrink(false)` or an explicit
  min-size is likely needed).
- **W4 (in-game 3-pane):** the canvas-in-view + overlay-over-canvas are PROVEN.
  The current InGame is still a single column (header → word → canvas → scoreboard
  → guess → chat → Leave) that overflows 540px — W4 replaces it with the 3-pane
  shell. The overlay pattern (`.fixed().fill().top_layer()` scrim centering a
  panel) is the word-pick / round-reveal template; give it proper centering once
  `.inset()` lands. The paint-order RESIDUAL (contiguous-suffix assumption) holds
  as long as there is ONE `ui()` root — true today.
- **W5 (avatar editor):** a second `raster(...)` element at 220×220 now composes
  by construction (the raster tier draws a list; two canvases each get their own
  `@group(1)` + draw). No new framework work expected.
- **FINAL re-decision handed up:** (a) the per-raster-anchor interleave (general
  paint order, retires the contiguous-suffix assumption); (b) a `buiy_view` inset/
  absolute-position surface; (c) flex-shrink control; (d) whether `.top_layer()`
  should also imply `.fixed()` + auto-centering (the modal ergonomic).

### 2026-07-03 — W3 pre-game screens (Home / Join / Lobby to parity)

Rebuilt the three pre-game screens to protokit parity: the light theme + purple
accent, the Caveat/Shantell font pairing, the 22-icon `DoodleAvatar`, and the
Home / Join / Lobby views. Verified by GPU capture (`home.png`/`lobby.png`/
`join.png`, 960×540) eyeballed against the design HTML. **Parity is strong** —
layout, colors, fonts, circular avatars, and rounded pills all match; the gaps are
the W6 character overlay (ink outlines, 3D-press, wobble) + emoji + a couple of
missing layout knobs, each classified below.

**Built (framework, `buiy_view` — prime FINAL-candidate surface):**
- **`Color::Custom(u8,u8,u8,u8)` + `Color::rgb(u8,u8,u8)`** — an exact-sRGB escape
  lowering to `ColorToken::Custom`. The 5-variant semantic facade can't name the
  protokit ladder (canvas/ink/ink-2/muted/status-green/white-on-accent), so every
  non-accent design color is pinned exact; the accent stays semantic
  (`Color::Accent` + a `SetAccent`-style startup write, so it re-themes for W6 dark).
- **`Radius::{Xl(22), Full(999)}`** — the card + pill/circle radii.
- **`Element::color(Color)`** — foreground for a Text / Button-label / Icon
  (→ `TextColor`). Without it *all* text was primary ink; this is what expresses a
  purple eyebrow, a muted caption, a white on-accent button label.
- **`Element::font(impl Into<String>)`** — per-text family (→ `FontFamily`), sans
  fallback appended.
- **`Element::border(width_px, Color)`** — a uniform solid border (→
  `BoxModel.border` width + a 4-side solid `Border`). Draws the design outlines +
  defines the cards.
- **`icon(path_d, size_px, stroke_width)` element (`Kind::Icon`)** — a
  `buiy_core::Icon` on a layout node; the SAME node also carries `.background` +
  `.radius`, so ONE node paints a tinted circular badge with the doodle stroked
  centered on top (fill tier under the icon-coverage tier).
- **Styleable buttons** — `.background/.radius/.border/.color/.font/.size/.width/
  .height/.grow` now apply to `Kind::Button` (fill/radius/size on the button, label
  color/font/size on its `ViewSlot` child). **Gated on "explicitly styled"** so an
  unstyled `button("x")` keeps every `buiy_widgets::Button` default — the shared-
  crate safety that keeps the counter/gallery goldens byte-identical.

**Built (framework, `buiy_core` render — THE headline fix):**
- **Borderless-rounded fill.** `pack_extracted` hard-coded `radius: 0.0` ("per-node
  corner radius is not yet on the extract record") — so **background fills never
  rounded**; widgets only *look* rounded because their border BAND masks the square
  fill corners. For Dooduel's large colored pills/circles that square poke is
  glaring. The `coverage`/quad shader already supports `PackedInstance.radius`, so
  I completed the stubbed path: added `ExtractedNode.radius`, resolved it in
  `extract_buiy_nodes` from a node's `Border.radius` **only when no border side
  paints** (a *borderless-rounded* node), min-clamped to `≤ min(half_w, half_h)`
  (a wide box pills, a square box circles — never the per-axis "lens"), and packed
  it into `PackedInstance.radius`. A **bordered** node keeps `radius 0` (its band
  traces the rounding) → **every existing display-list snapshot + GPU golden is
  byte-identical** (verified: buiy_core GPU 36 goldens + buiy_verify GPU 22, all
  byte-identical). This is what turned the square boxes in the first capture into
  clean purple pills + circular avatars.

**Built (app, `examples/dooduel`):**
- `theme.rs` — the protokit light color ladder as `Color::Custom` constants, the
  Caveat/Shantell font names, `DooduelThemePlugin` (purple accent + `register_bytes`
  the two `.ttf`s). Kept OUT of `install` so the GPU-free probe tests need no
  `Theme`/`FontRegistry`.
- `avatar.rs` — the 22 doodles ported as structured data + `hash_str` (design
  `hashStr` verbatim) + one-stroked-`d` generation. `doodle_avatar(name, px)` →
  one `icon(...)` badge.
- `lib.rs` — `Screen::Join`; `Msg::{CreateRoom, GoJoin, SetJoinCode, SubmitJoin}`
  (Play now starts the match directly, the design's primary CTA); `room_code`/
  `join_code`/`is_host` model fields; the `home`/`join`/`lobby` views + helpers
  (`card`/`primary_button`/`soft_button`/`quiet_button`/`eyebrow`/`title`/`badge`).
- `capture_dooduel.rs` — rewired to the light theme + `DooduelThemePlugin`, drives
  Home → Create → Lobby → Join, canvas-colored clear.

**Ran the artifact (per-screen parity, honest):**
- **Home** — strong. Purple circle logo mark + Caveat "Dooduel" wordmark; subtitle;
  editable circular avatar + name field; purple **Play pill** + two tinted soft
  pills; a "you'll play with" row of three circular doodle avatars + names. Matches
  the design's structure/spacing/colors. *Missing:* ink button outlines (see the
  bordered-rounded-fill gap), the 3D-press shadow (W6), the pencil glyph in the logo
  (emoji), the radial-gradient bg, the rotated "free & open source" ribbon (W6 deco).
- **Lobby** — strong. Purple "Private room" eyebrow, Caveat title, the room-code box
  (rounded, ink-bordered, code in Caveat + a purple "Copy link"), the 4-row roster
  (circular avatar + name + rounded tinted Host/Joined badge), the purple Start pill,
  Leave. *Missing:* same outline/shadow/ring items; the dashed room-code border.
- **Join** — strong. Back, "Join a room" eyebrow, Caveat "Enter a room code", body,
  the code field (shows the uppercased code), the purple Join pill, the note.
  *Missing:* centered code input (no text-align surface); ink outlines.

**Findings (classified):**
- **[framework-bug — FIXED, the headline] Background fills did not round.** Root
  cause + fix above. **Residual for the FINAL:** a fill rounds cleanly only WITHOUT
  a border — a *bordered* rounded fill still shows square-corner "ears" (the band
  rounds, the fill doesn't). So a rounded element with a VISIBLE border (the design's
  ink-outlined pills, the avatar's 1.5px dark ring) is **not yet expressible**; I use
  borderless-rounded for the colored pills/circles (clean) and reserve borders for
  the card (white fill "ears" are invisible on the light canvas). FINAL: round the
  fill even when a border paints (thread the same radius into both the fill quad and
  the band's inner mask).
- **[framework-bug — the border-radius LENS] Per-axis corner clamp.**
  `resolve_corner_radii` clamps each corner to `(half_w, half_h)` INDEPENDENTLY, so a
  large uniform radius on a wide-short box becomes an elliptical `220×28` corner → a
  pointed *lens*, not a pill (visible in the first W3 capture's bordered buttons). CSS's
  proportional radius reduction isn't implemented. My uniform-fill path sidesteps it
  (single min-clamped radius → pill/circle), but the **border BAND** still lenses.
  FINAL: implement the CSS radius-reduction factor in `resolve_corner_radii`.
- **[missing-feature — emoji tofu] No color-emoji support.** `🎨🎉🔗🚪👋✏✅👑😄`
  render as the font's `.notdef` box (the bundled faces are Latin-only; the coverage
  rasterizer is monochrome R8 — it cannot do COLR/CBDT/sbix color emoji). Geometric
  glyphs (`▶ ‹`) DO render (they're in the fonts), so I kept those and dropped the
  color emoji from the copy for clean captures. FINAL: pick an emoji strategy (an
  outline emoji font through the existing coverage path, or a color-emoji pipeline).
- **[missing-feature] No text-align, no drop-shadow, no gradient, no font-weight.**
  Centered copy renders left-aligned (button labels center via the widget default);
  the card's shadow + the radial-gradient bg are approximated (border-defined card,
  flat `--canvas`); the variable fonts load but `.font()` has no weight arg, so all
  text is the default-instance weight. Each is a thin follow-on modifier.
- **[missing-feature — the Color facade is too small] 5 roles vs ~a dozen.** Added
  `Color::Custom` + pinned the ladder in the app. FINAL: grow the facade to the
  theme's token vocabulary, OR keep the documented Custom escape.
- **[finding — light-palette visual verdict] The `default_light_theme` non-accent
  tokens are family-derived STUBS, not the protokit ladder.** `text_primary` (0.10
  gray) ≈ ink but not `#14161b`; there is no distinct `canvas` vs `surface`, no
  status/on-accent tier. Dooduel is its first visual consumer; I pinned the exact
  design colors via `Custom` rather than trust the stub. FINAL: give the light theme
  a real design ladder (or let the app own its palette, as here).
- **[DX — positive] The font pipeline just worked.** `register_bytes(name, Arc<Vec<u8>>,
  default())` + `.font("Caveat")` — both fonts `Loaded` with `faces=1` and NO
  family-name-mismatch warning (verified via a temp `FontRegistry::load_state`/`faces`
  probe); late-load `FontsGeneration` reshaped on the fly, so no ordering dance. sfnt
  `.ttf` accepted. Sole gotcha: the registered string must equal the font's INTERNAL
  family name — Shantell's typographic-family nameID-16 `"Shantell Sans"` matched;
  the legacy nameID-1 `"Shantell Sans Light"` would have missed.
- **[DX — icon authoring] 22 composite doodles → one stroked `Icon` each.** `Icon` is
  single-`d` + one stroke width + one paint (stroke XOR fill). Folded every primitive
  into ONE stroked `d`: `<path>`s verbatim, `<line>`→`M L`, `<circle>/<ellipse>/dot`→
  4 cubic beziers (kappa — deliberately NOT SVG arcs, whose `large-arc`/`sweep` FLAG
  args a blanket coordinate-scale would corrupt). Two chores: (a) the design's 40×40
  viewBox → `Icon`'s hard-pinned 24×24 (`ICON_VIEWBOX`) needs a per-app 0.6 coord+stroke
  scale — a `viewbox` arg on `Icon` would remove it; (b) filled dots became stroked
  rings (a stroke ≈ the dot diameter reads as a dot) — the one fidelity compromise of
  the single-stroked-icon fold. The `Kind::Icon` element + badge-on-same-node
  (bg+radius+icon coverage) is a clean, elegant addition.
- **[game-seam / decisions, journaled] Nav + lobby simplifications.** `Play` starts
  the match directly; `Create`/`Join` → Lobby. Room code is a deterministic name-hash
  (design uses `Math.random`; a prototype wants replay determinism, no real rooms).
  Host-gating + the joiner's auto-start (design's 750 ms / 3.2 s timers) are simplified
  to "both seats get an enabled Start" — the staggered join animation needs the G3
  timer/`Cmd::task` API. Copy-invite is inert (clipboard deferred: arboard native /
  async web).
- **[pre-existing debt — found, not mine] The GPU-lane system-COUNT meta-tests fail.**
  `render_smoke`/`render_prepare`/`render_compositor` assert `extract`/`render` system
  counts against fixed constants (`== 5`, `BUIY_RENDER_SYSTEM_COUNT`); W1/W3a's raster
  pipeline added `extract_buiy_rasters` (+ prepare/draw) that the constants were never
  updated for (delta 6 vs 5). NOT a W3 regression (my change adds zero systems; the GPU
  GOLDENS are byte-identical) — the earlier waves' GPU-lane run never caught it (the
  count-tests are `#[ignore]`). FINAL: bump the count constants for the raster systems.

**Surprised by / friction:** the first W3 capture rendered every colored button/badge/
avatar as a hard SQUARE (+ a lens-shaped border) — the moment that exposed the
no-rounded-fills gap, which the whole rounded design hinges on. Completing the stubbed
`PackedInstance.radius` path turned out small and (thanks to the borderless-only gate)
byte-identical for every existing golden. The pleasant surprise was the font pipeline
matching Shantell on the first try (I'd braced for the nameID-1/16 mismatch).

**If we did this again:** discover the rounded-fill gap FIRST (build one styled pill
before the whole screen). And reach for the borderless-rounded fill from the start
rather than borders — borders both lens (wide radius) AND leave colored ears.

**Handed to W4 (in-game 3-pane):**
- **Rounding rule:** use `.radius()` WITHOUT a border for a clean rounded panel/pill;
  a visible border re-squares the fill corners (ears) until the FINAL rounds bordered
  fills. The color/font/border/icon/pill helpers + the palette (`theme.rs`) are ready
  to reuse.
- The **dark top bar** (`--ink-panel`) needs a distinct dark chrome color
  (`Color::Custom` ~`#14161b` on the light app) + white-on-dark text/icons — the first
  place the seat-switcher avatars re-appear.
- The W3a canvas `raster(...)` element + the `.fixed().fill().top_layer()` overlay
  pattern are unchanged; the word-pick / reveal panels reuse them.
- Still MISSING for pixel-parity (carried from W3a + new): `inset`/absolute position,
  per-side padding, flex-shrink, text-align, drop-shadow, font-weight, bordered-rounded
  fills, the radius LENS fix, and emoji.

### 2026-07-03 — W4 in-game screen

The game's main screen to parity: the dark top bar + seat-switcher, the header
(round / role / word-slots / **timer ring**), the 3-pane body (scoreboard |
canvas + toolbar | chat), the toolbar wired to `paint.rs`, and the pick / reveal /
waiting overlays. The biggest wave; strongest parity yet. Verified by GPU capture
of a REAL match driven to four states + a GPU-free probe test.

**Built (framework, `buiy_view` — prime FINAL-candidate surface):**
- **Pressable `Icon` (the headline extension).** An `icon(...)` that carries
  `.on_press(msg)` + `.label(name)` becomes an activatable a11y button
  (`A11yRole::Button` + `A11yLabel` + the view's `PressAction`) — so a **clickable
  avatar chip** works, without a `Button` widget. It routes through BOTH modalities
  because the a11y **Button contract is role-keyed** (`contract_for(Button).honor(Click)`
  emits `OnPress` with no `Button` component needed): a real pointer click lowers to
  `OnPress` via `pointer_click_emits_on_press` (which gates on the activatable role),
  and a probe/AT `Action::Click` emits the same `OnPress` — both hit the view's
  `route_presses` → the `PressAction` → the funnel. The seat-switcher demanded this
  (the design's chips are avatar-only, no text). [missing-feature — delivered]
- **`.shrink(bool)`** (`FlexItem.shrink`) + **`raster()` defaults to `.shrink(false)`** —
  the W3a canvas-squish fix, delivered. (It didn't actually bite here — I sized the
  window so the center pane exceeds the canvas — but the pin is correct-by-construction
  insurance for a tighter layout.) [missing-feature — delivered]
- **`.align_start()`** (`AlignItems::FlexStart`) — the 3-pane row keeps each column at
  its natural height (a short scoreboard is not stretched to the 556px chat column),
  the design's `align-items:start`. A trivial third option beside the existing
  `align_center`/default-stretch. [missing-feature — delivered]

**Built (app, `examples/dooduel`):**
- `lib.rs` — the whole `in_game` screen + ~20 helpers, and a `ToolState` on the model
  (tool / color_idx / size_idx / clear_seq / undo_seq). The toolbar (segmented
  brush/fill/eraser, brush-size dots, 16 swatches, undo/clear) routes `SelectTool` /
  `SelectColor` / `SelectSize` / `ClearCanvas` / `UndoStroke` / `Continue` through the
  funnel — **tool selection is reducer-owned + replayable.**
- `paint.rs` — `sync_tools_to_canvas` mirrors the model's `ToolState` onto the
  `PaintCanvas` each frame; an `enabled` gate (drawer-only painting); a small undo
  ring; a per-turn auto-clear on the Picking→Drawing edge; the 16-color `PALETTE` +
  `BRUSH_SIZES`; `Reflect` on `Tool`.
- `game.rs` — `word_slots()` (per-letter reveal for the underlined word row) +
  `continue_now()` (the reveal "Continue →" skip).
- `capture_ingame.rs` — drives a REAL match to the four states (drawer-drawing with a
  painted house/tree/sun + a bucket-filled roof, guesser with blanks+hint+bot chat,
  the pick overlay, the reveal overlay) → four PNGs.

**Ran the artifact** (`target/dooduel-captures/ingame_*.png`, 1280×800, GPU readback,
eyeballed vs the design HTML — **strong parity all four**): the dark top bar with 4
clickable doodle-avatar chips (the viewed seat enlarged) + Leave; the header card
("Round 1 of 2", role badge, the "B I C Y C L E" underlined word row, the accent
timer ring + number); the scoreboard sorted high→low with rank/avatar/name/role-pill/
score; the framed canvas with the painted scene; the two-row toolbar; the chat with
green correct-guess bubbles; the centered pick + reveal modals over a dark scrim.
Probe test green: every toolbar/chrome button locatable by role+name, **clicking the
seat-1 avatar chip hops the seat** (the pressable-icon route, end-to-end), and a
Send-driven guess scores.

**Findings (classified):**
- **[missing-feature — worked around; TIMER-RING VERDICT, positive] The per-second
  arc-regen ring works and reads well (G4 resolved).** The ring is an `icon` whose
  arc `d` is a short-segment polyline of the remaining fraction (round-capped by the
  icon stroke → a smooth arc), regenerated **only when the displayed second changes**
  (`frac` derives from the integer seconds-left), so the content-addressed icon atlas
  re-rasters **at most once per second** — the G4 per-frame-churn risk is fully
  mitigated. Visually the ring **steps** down once per second vs the design's smooth
  CSS `dashoffset` transition; the step is a small, clearly-readable jump (verified in
  the guesser capture: a ~31% arc at 25/80s). The faint full BACKGROUND ring is omitted
  (a single-paint icon can't do the faint-ring + colored-arc two-layer). VERDICT: the
  arc-icon ring is a viable FINAL timer; smoothness (if wanted) needs a sub-second
  animate hook or a dedicated ring primitive, and the bg ring needs a 2nd layer.
- **[missing-feature — worked around; CHAT-SCROLL VERDICT] No view-level scroll
  container.** `ScrollArea` (buiy_widgets) is not reachable from a `view` element, and
  a `keyed_column` of ALL messages would overflow the fixed-height chat pane. Capped the
  chat to the **last 12 messages** (an auto-scroll-to-bottom stand-in — the newest are
  always shown). VERDICT: a `scroll_column` (`Overflow::Scroll` + auto-max `ScrollOffset`
  on append) is a real FINAL candidate; the cap is a clean prototype workaround whose
  only cost is losing scrollback.
- **[missing-feature — the toolbar wrap, found by RUNNING] No flex-wrap.** The design
  toolbar is ONE wrapping row (16 swatches wrap to a 2nd line). Buiy has no `FlexWrap`
  in the view, so a single row **overflowed horizontally into the chat pane** — the
  first capture caught it immediately (a headless snapshot would not have). Fixed by
  splitting into two explicit rows (controls, then swatches). FINAL: a `.wrap()`
  modifier restores single-row authoring.
- **[game-seam — tool-state ownership, the clean seam] Tool selection lives on the MVU
  model, projected model→canvas each frame.** The reducer owns `ToolState` (so tool
  changes replay), and `sync_tools_to_canvas` is the one-way projection the paint
  observers read. The out-of-model pixel buffer's imperative ops — clear, undo, the
  per-turn auto-clear — ride **monotonic request counters** (`clear_seq`/`undo_seq`)
  + a phase-edge `Local`, which the sync drains once each. This is the clean boundary
  between "MVU-governed UI state" and "a side render surface the framework can't fold
  into the model" — the same shape W1/W3a used for the canvas handle. Worked with zero
  friction.
- **[game-seam — per-turn clear] The canvas blanks on the Picking→Drawing edge** (a
  `Local<bool>` edge in the sync), so each turn's drawing starts fresh — the game phase
  drives a canvas side effect without threading a message.
- **[framework — the bordered-rounded-fill "ears", W3 residual, a deliberate parity
  call] Kept ink borders on the white panels.** The design's whole character is the
  2.5px ink outline, so I paired `.radius()` + `.border()` on every panel and accepted
  the faint square-corner "ears" (white-on-`--canvas`, subtle) rather than dropping the
  outline. This is the opposite of the task's "radius-without-border" caution, chosen
  because the outline IS the design and the ears are near-invisible on the light canvas
  (confirmed in the captures). FINAL: round the fill under a border (the W3 residual) so
  panels + the canvas frame clean up.
- **[missing-feature — a cluster of thin surface gaps, each worked around] No per-side
  padding** (the fixed-60px top bar fights `avatar + uniform padding`; used
  content-height + `Md` padding → ~62px bar); **no text-align** (word slots underline
  via a child 4px bar, not `border-bottom`; system chat lines centered by wrapping in an
  `align_center` column); **no rotation** (the pick tiles are not tilted ±3°);
  **no drop-shadow / 3D-press** (W6); **emoji tofu** (stripped from chat copy — the
  Latin fonts have no color-emoji glyphs; kept typographic punctuation below U+2300).
- **[missing-feature — pressable RESIDUAL] Only a leaf `Icon` is made pressable.** A
  clickable CONTAINER would need the click to land on the container itself (children on
  top intercept the hit + carry no role), so the pick-word tiles stay real `button`s
  (they have text labels anyway). And the per-seat **drawing/guessed corner badges** are
  deferred — they need an absolute-positioned overlay on the chip corner, which has no
  `inset` surface; the viewed seat is highlighted by SCALE instead (the design's 1.1×).
- **[app-bug — mine, found by eyeball] Two defects the first live render exposed:** the
  toolbar overflow (above) and chat emoji rendering as tofu boxes. Both fixed. The lesson
  is the recurring one — **RUN the artifact**; the horizontal overflow and the tofu were
  invisible to the headless probe.
- **[pre-existing — NOT W4, surfaced by the gates] (a)** `capture_w3a`'s Home-centering
  assertion (`cy > 60`) is now stale: the W3 Home rebuild made Home ~500px tall (title
  `top_y` 182→52), so the bin panics on the Home case before its canvas/paint-order
  cases (my diff does not touch `home()`). **(b)** The 3 GPU system-COUNT meta-tests
  (`render_smoke`/`render_prepare`/`render_compositor`) still assert `5` vs the actual
  `6` extract systems — the W1/W3a raster pipeline the constants were never bumped for
  (`left: 6, right: 5`); the **36 golden IMAGES are byte-identical**. Both were logged in
  the W3 journal; the FINAL should bump the count constants + refresh the `capture_w3a`
  Home assertion.

**Gate results:** `cargo test -p dooduel` **30/30**; `buiy_view` + `buiy_verify`
(206 snapshots) + `buiy_widgets` + `buiy` headless all green (additive view changes
left every existing layout/display-list snapshot byte-identical); `cargo fmt` clean;
`cargo clippy -D warnings` clean on `dooduel` + `buiy_view`; **GPU render goldens 36/36
byte-identical** (the 3 count-meta-test failures are pre-existing, above).

**Surprised by / friction:** the pressable-icon extension "just worked" through the
whole pointer + probe + a11y stack the moment I saw the **Button contract is role-keyed**
(it emits `OnPress` with no `Button` component) — I'd braced for having to attach a real
`Button`. The friction was the toolbar overflow (no flex-wrap), which only the LIVE
render exposed.

**If we did this again:** RUN the render right after the toolbar (the overflow would
have shown instantly), and reach for the two-row toolbar from the start rather than
fighting the missing wrap. Otherwise the same shape — the model-owned tool state +
model→canvas sync + pressable-icon trio is the load-bearing architecture.

**Handed to W5 (podium + confetti + avatar editor) / W6 (sketchy pass):**
- **W5:** the avatar editor is a **2nd `raster()` canvas** (220×220) + the same toolbar
  pattern — every piece exists (`PALETTE`, the brush-size dots, the swatch grid, the
  pressable route, the undo ring). The podium reuses `doodle_avatar` + `badge`; the
  reveal overlay's turn-result-row pattern + the `scrim()` helper are reusable modal
  scaffolding. Confetti + score-flash floats need the deferred G3 timer / animate hooks.
- **W6:** panels are ink-outlined but NOT wobble / 3D-press; the bordered-rounded-fill
  "ears" + the radius-lens are the W6/FINAL render fixes; the emoji strategy is still
  open; the timer ring's per-second STEP (vs smooth) is a W6/animate refinement.
- **FINAL re-decisions surfaced (W4-new):** `.wrap()` (flex-wrap), per-side padding,
  text-align, a view `scroll_column` (+ auto-scroll), rounding bordered fills, a
  clickable-CONTAINER press route (not just leaf icons), and `inset`/absolute for the
  seat corner-badges + pixel-perfect modal centering.

### 2026-07-03 — W5 podium + confetti + avatar editor

The final three surfaces: the **final-results podium** (ranked pedestals + the
custom avatar), the **celebration confetti** (a tweened raw-ECS overlay), and the
**avatar editor** (the 2nd `raster()` canvas — the two-surfaces question W1
predicted). The headline: **W5 needed ZERO framework changes** — W3a/W4's surface
was sufficient; the two real framework limits it hit (per-piece opacity-as-effect
-group, and a raster nested in a top-layer overlay) were both **worked around
app-side** and handed to the FINAL. Verified by GPU capture of all three states +
GPU-free probe tests.

**Built (app, `examples/dooduel` — no framework edits):**
- `paint.rs` — **generalized `PaintCanvas` (singleton `Resource`) → `PaintSurface`
  (a plain per-canvas struct) held in a keyed `PaintCanvases` resource
  (`HashMap<CanvasKind, PaintSurface>`)** + a `CanvasKind` marker component on each
  canvas node. The shared Press/Drag/Release observers read the node's `CanvasKind`
  and route the edit to `surfaces.get_mut(kind)`. Adds the avatar scratch (220×220)
  + a committed `saved_avatar` image; the tool-sync mirrors BOTH surfaces from the
  model and, on a `save_seq` bump, copies scratch→saved.
- `confetti.rs` — a decoupled `ConfettiPlugin`: a system reads `Dooduel.screen`,
  and on the rising edge into `Podium` hand-spawns ~110 colored-quad `Node` roots,
  each a `Background` + `Border`(radius) with a **fall `TranslateTween` + tumble
  `RotateTween`**; pieces despawn on the fall tween's `OnComplete`.
- `lib.rs` — the parity `podium()` (eyebrow / "{winner} wins!" / subtitle / the
  2nd|1st|3rd pedestals / rest-standings / Play-again+Home), the `avatar_editor()`
  screen (gallery + draw tabs, the 220×220 canvas, swatches / sizes / eraser / undo
  / clear / save), an `AvatarState` on the model + its reducer arms + the Home
  pencil affordance + the `avatar_el()` seat-avatar chooser (custom / preset /
  default).
- `capture_w5.rs` — drives the editor (doodle a blue face → save), Home (custom
  avatar in use), and a full match to the podium (confetti mid-burst) → 3 PNGs
  with pixel asserts (editor face blue px, home custom px, confetti coral+yellow px).

**Ran the artifact** (`target/dooduel-captures/w5_*.png`, 1000×680, GPU readback,
eyeballed vs the design — **strong parity all three**): the podium's stair-stepped
pedestals (winner center + tallest, accent purple, the "1"), the custom blue-smiley
avatar for the human seat, the rest-standings row, Play-again; the multicolor
confetti burst falling across the screen; the avatar editor with the blue smiley on
the 220×220 canvas + the full toolbar; Home showing the custom avatar beside the
pencil badge. Probe tests green (35/35): the pencil opens the editor, Save
round-trips the custom flag through the funnel, and a full virtual-tick match
reaches the Podium screen.

**Findings (classified):**
- **[game-seam / decision — THE two-canvas verdict, W1's prediction CONFIRMED] Two
  `raster()` surfaces compose by construction; a keyed resource beats a component.**
  W1 predicted "two canvases compose because `RasterBuffers` holds a list." True —
  the generalization was mechanical: ONE `PaintSurface` type, a `CanvasKind` enum
  key, observers route by the node's marker. **Chose a keyed `Resource` map over a
  `PaintSurface`-component-on-the-node** because the `buiy_view` reconciler despawns
  a `raster` node when its screen/overlay closes and respawns it on re-entry — a
  component would lose the pixels on that despawn, but the resource-held buffer
  survives (the game canvas across turns, the avatar across editor re-opens). The
  map generalizes to N canvases; named `game`/`avatar` fields would too but wouldn't
  advertise the scaling. Non-regressive: the game canvas is byte-identical
  (`capture_w3a` still reads `[220,40,40]` center+corner; `capture_ingame`'s painted
  scene renders unchanged).
- **[game-seam — the confetti seam] Confetti is a pure model-observing side effect,
  same shape as the paint sync.** The trigger system only READS `Dooduel.screen`
  (a `Local<bool>` rising-edge latch), never enqueues or mutates the model — so it
  doesn't touch the MVU funnel; the pieces are decoupled ECS entities the reconciler
  never owns. On podium exit it bulk-clears leftovers + resets the latch. **Spawn
  cost:** 110 entities in one frame produced **no visible hitch** — the mid-burst
  capture renders a full, dense burst, and the podium content is unaffected. This
  is the cleanest demonstration yet of "a screen edge drives a decoupled overlay."
- **[framework — THE headline finding] A per-particle opacity fade forms a
  per-piece EffectGroup — a compositor blow-up, not a fade.** The design fades each
  confetti particle near end-of-life. But a Buiy `Opacity < 1.0` forms an
  `EffectGroup` (an OFF-SCREEN composite boundary + a stacking context), so an
  `OpacityTween` per piece would spin up ~110 off-screen render targets at once.
  I dropped the fade (pieces hard-despawn at the bottom); `Translate`/`Rotate` only
  form a *cheap* stacking context, never an effect group, so the burst stays
  composite-free. **FINAL ask: a cheap per-quad particle alpha** (a fade that does
  NOT promote to a group) for any particle system on Buiy.
- **[framework — the W3a residual, now CONCRETELY bitten + worked around] A
  `raster()` nested in a top-layer overlay is HIDDEN under the overlay's own
  background.** W3a's paint-order fix draws in-flow quads → **raster (one global
  tier)** → top-layer quads → glyphs. The design's avatar editor is a modal Dialog;
  rendering it as a `.fixed().top_layer()` scrim put the draw canvas in the raster
  tier, so the top-layer PANEL background painted OVER it (the canvas vanished), and
  in-flow Home glyphs bled through the scrim (glyphs are per-context, but the panel
  couldn't obscure a raster it contained). **FIX: render the avatar editor as a full
  in-flow SCREEN** (not an overlay) — the canvas raster then paints over its in-flow
  panel exactly like the in-game canvas does, and there's no sibling bleed because
  it fully replaces Home. This is the app-side realization of W3a's deferred general
  fix; the FINAL must do the **per-raster-`node_quad_anchor` interleave** (the
  gradient-tier precedent) to allow a true modal-with-a-raster.
- **[DX — tween from app-land, positive with two gotchas] The tween API was
  pleasant; the harness wiring was the friction.** `Tween::new(from, to, dur,
  easing).with_on_complete()` + `TranslateTween`/`RotateTween` read cleanly, and
  driving `Translate`/`Rotate` on hand-spawned roots "just worked" through the
  transform bridge (confirming W1's "coexistence mechanics are free"). Two gotchas:
  (a) **`BuiyHeadlessPlugin` deliberately omits `AnimationPlugin`** (headless probes
  don't animate), so tweens are inert under it — `ConfettiPlugin` adds it defensively
  (`is_plugin_added` guard, idempotent across the windowed + capture bins); (b) in a
  tight headless `app.update()` loop `Time::delta()` is ~µs, so tweens barely
  advance — the capture **sleeps 16ms between frames** to let confetti fall into
  view. A FINAL **headless-animation harness wants a virtual-clock advance**
  (`Time::advance_by`) rather than real sleeps.
- **[missing-feature — raster has no rounded clip] Custom avatars render SQUARE.**
  The `raster` shader clips to a rectangular AABB only (no corner radius), so a drawn
  custom avatar is a square sticker while the stock doodles are circular (icon
  coverage). Visible + acceptable for the prototype; FINAL wants a **rounded clip for
  `raster`** (or an avatar circle-mask) so a custom pic reads as a round avatar.
- **[missing-feature — pressable raster, a W4-residual cousin] A custom-avatar
  raster can't carry `on_press`.** Only `icon`/`button` wire the press route (W4's
  leaf-only limit); a `raster` silently ignores `.on_press`. So clicking your OWN
  seat-0 chip (once it's a custom raster) can't switch seat, and the Home avatar
  itself isn't the edit affordance — a **separate pressable pencil `icon`** is. The
  pencil is placed *beside* the avatar (no absolute-inset surface for the design's
  corner badge). FINAL: a container/raster press route.
- **[missing-feature — the podium stair-step] `align-items:flex-end` isn't in the
  view; a top-spacer builds the podium instead.** The winner-highest silhouette
  wants a flex-end row; W5 gives each column a TOP SPACER of `(max_pedestal −
  this_pedestal)` so equal total heights bottom-align while the tops stagger — **no
  new cross-axis modifier needed** (keeping W5 pure app-land). Also surfaced:
  **per-corner radius** (pedestals want `14px 18px 0 0`, got uniform `Radius::Lg`)
  and the bordered-rounded-fill "ears" (W3 residual, accepted).
- **[decision — corrected a design-code quirk, journaled] Podium pedestal heights.**
  The design's `PODIUM_H = [92,124,72]` is indexed by *rank*, so the *rendered*
  prototype gives 2nd place the tallest block (a height/order indexing quirk in the
  design code). W5 corrects toward the obvious intent — the winner's center pedestal
  is tallest (`PEDESTAL_H = [124,92,72]` by place). A faithful port of the literal
  code would look wrong; noted so the FINAL doesn't re-import the bug.
- **[decision — avatar model + save semantics] `HumanAvatar` = Default | Preset |
  Custom, model-owned + replayable.** A gallery pick sets `Preset { icon, tint }`
  (a forced doodle); a drawn save sets `Custom` and **copies the scratch pixels into
  a separate `saved_avatar` image** — a snapshot, so re-opening + editing the scratch
  doesn't mutate the already-committed pic (the design's `toDataURL` snapshot). The
  editor's draft brush + open/tab are reducer-owned (like the in-game `ToolState`);
  the scratch's imperative ops (clear/undo/reset-on-open/save) ride monotonic `*_seq`
  counters the sync drains — the same MVU↔side-surface boundary W4 used.
- **[missing-feature — emoji, the recurring gap] Every W5 emoji is dropped or
  substituted.** 🥇🥈🥉 medals, 🎊/🏆 (podium), ✏️ (edit), ✅ (save), ✕ (close) — the
  Latin fonts have no color-emoji glyphs. The pedestal shows the rank NUMBER (no
  medal); the pencil is a stroked `icon` path; save/close read as text. Same FINAL
  emoji strategy ask as W3/W4.
- **[finding — the overlay pattern's hidden constraint, now understood] A top-layer
  scrim correctly DIMS + obscures underlying GLYPHS (the W4 reveal overlay proves
  it), but CANNOT obscure a raster it contains, and an in-flow raster can't sit
  under a modal.** So the rule for Buiy today: **a modal may contain text + quads,
  but a modal that contains a raster must be a full screen** until the per-anchor
  interleave lands.

**Gate results:** `cargo test -p dooduel` **35/35** (added: 2 avatar-reducer + 2
avatar/podium probe tests + 1 undo test); `cargo fmt` clean; `cargo clippy
--all-targets -D warnings` clean on `dooduel`; **NO framework crates touched**, so
the W3a GPU-golden gate is N/A (verified indirectly: `capture_w3a` byte-exact canvas
ink + `capture_ingame` painted scene both still render). All three W5 captures
render + assert (editor face 2521 blue px, home custom 127 px, confetti 793 coral+
yellow px).

**Surprised by / friction:** the raster-in-a-top-layer-overlay failure was the real
surprise — the FIRST live editor render showed Home text bleeding through AND the
draw canvas missing, which no headless probe would catch (the recurring **RUN the
artifact** lesson). Reframing the editor as a full in-flow screen fixed both at once
and is arguably cleaner. The pleasant surprise was that the whole wave needed **zero
framework edits** — the keyed-canvas generalization, the confetti overlay, and the
podium all sat on the W3a/W4 surface.

**If we did this again:** render the editor as a screen from the start (the modal
framing cost a debug cycle), and reach for the top-spacer podium immediately rather
than hunting for a flex-end modifier. Keep the keyed `PaintCanvases` from day one —
it's the right shape and cost nothing.

**Handed to W6 (sketchy pass + theming + persistence) / W7 (web):**
- **W6:** the podium pedestals + editor panel are ink-outlined but NOT wobble /
  3D-press; the bordered-rounded "ears" + per-corner-radius (`14px 18px 0 0`
  pedestals) are the W6/FINAL render fixes; confetti could gain the score-flash
  floats once a per-quad particle alpha exists; a smooth (vs stepped) fade needs it
  too. **Persistence (W6):** the custom avatar (`HumanAvatar` + the saved image
  pixels) + the theme toggle must persist — web localStorage / native file; the
  design keys the avatar under `dooduel-proto-avatar`.
- **W7 (web):** the avatar editor + the game canvas are BOTH pointer-driven
  `raster` surfaces — touch drawing + the mobile bottom-sheet framing of the editor
  are the web-mobile work; confetti is wasm-safe (raw quads + tweens, no threads).
- **FINAL re-decisions surfaced (W5-new):** (1) the **per-raster-`node_quad_anchor`
  interleave** (retire the raster-in-overlay limit → a true modal-with-a-raster);
  (2) a **cheap per-quad particle alpha** (fade without an EffectGroup); (3) a
  **rounded clip for `raster`** (round custom avatars); (4) a **headless
  virtual-clock advance** for animation captures; (5) `align-items:flex-end`; (6) a
  raster/container press route; (7) per-corner radius. None blocked W5 — all were
  worked around app-side.

### 2026-07-03 — W6 sketchy character pass + theming + persistence

The wave that gives Dooduel its hand-drawn identity AND carries the TWO render
fixes every prior wave pointed at (the bordered-rounded "ears" + the per-corner
radius). The render + `buiy_view` parts are kept FINAL-candidate clean; the
app-side theming/persistence is prototype. **Headline: both render fixes landed
with ZERO existing-golden churn** (nothing to rebless — the framework's own
goldens never exercised a bordered-rounded fill, so the fix is proven only by the
Dooduel captures), and the light/dark toggle + native persistence both round-trip
verified end-to-end on the RX 6700 XT.

**Built (framework, `buiy_core` render — prime FINAL candidates):**
- **Render fix 1 — bordered-rounded fill ("ears"), the W3/W4 residual, FIXED.**
  `extract_buiy_nodes` now packs the FILL quad's uniform radius to the border
  band's **inner radius** whenever a border paints (was hard-`0`, leaving the
  fill square while only the band traced the rounding → square "ears" poking past
  a rounded outline). For a UNIFORM border every inner corner is equal, so this is
  EXACT (the fill boundary coincides with the band inner edge — a clean rounded
  panel/pill). Reuses `ExtractedBorder.inner_radius` (already CSS-correctly shrunk
  per corner), takes the min for the single-radius `PackedInstance` slot. A SQUARE
  bordered box has `inner_radius == [0;8]` ⇒ still packs `0` ⇒ byte-identical.
- **Render fix 2 — per-corner band radii + the radius-LENS, both in `band.wgsl`.**
  The band shader was reading `outer_radius_tl_tr.x` (TL only) and using it
  circularly, so (a) a per-corner wobble was impossible and (b) a wide
  `border-radius:9999px` box drew a pointed **lens** (rx clamps to `half_w` ≈ 250,
  ry to `half_h` ≈ 28; a circular r=250 SDF on a 56-tall box is an eye shape). Now
  the fragment selects each corner's radius by quadrant (the standard per-corner
  rounded-box SDF) and uses **`min(rx, ry)` per corner** — the pill/circle
  behavior. A uniform circular radius makes all four equal ⇒ **byte-identical to
  the old path** (proven: buiy_core 39 + buiy_verify 22 GPU goldens all
  unchanged). The instance record already CARRIED the per-corner radii; only the
  shader collapsed them.

**Built (framework, `buiy_view` — prime FINAL candidates):**
- **`.radius_corners(tl, tr, br, bl)`** — per-corner circular radii (the thinnest
  signature that expresses the design's wobble: four `f32` px). Overrides
  `.radius`; lowers to a per-corner `Border.radius` (`Corners`). Per-AXIS
  elliptical corners are NOT rendered (the band SDF is circular) — the residual
  below.
- **`.shadow(dx, dy, blur, spread, color)`** — pushes one drop-shadow term
  (chains front-to-back = CSS order), lowering to a `BoxShadow`. The design's
  `--sh-*` ambient + the `0 5px 0 ink` 3D-press underside.
- **`.justify_end()` + `.align_end()`** (`JustifyContent::FlexEnd` /
  `AlignItems::FlexEnd`) — the bottom-right floating theme toggle. Trivial siblings
  of the existing justify/align set.

**Built (app, `examples/dooduel`):**
- `theme.rs` — a `Palette` struct (the theme-VARYING tokens) with `LIGHT` / `DARK`
  const ladders (the protokit `:root` ↔ `SURFACES_DARK`), a `ThemePref` enum
  (model-owned), `WOBBLE_CARD`/`WOBBLE_PANEL` radius tokens, the `--sh-*` shadow
  colors, and `sync_theme_resource` (a model-observing side effect that swaps the
  `Theme` RESOURCE light↔dark + pins the protokit accent). Theme-invariant colors
  (white, the always-dark top bar, the scrim) stay module consts.
- `lib.rs` — threaded `Palette` through the whole view (~40 helpers), applied the
  sketchy pass (`sketchy_card` / `sketchy_panel` decorators = wobble + ink outline
  + `--sh-*`; the 3D-press on the primary pills; wobble on the logo blob +
  pedestals), added `theme: ThemePref` + `Msg::SetTheme` + `Msg::Restore`, and the
  fixed bottom-right `theme_toggle`.
- `storage.rs` — native persistence: a JSON blob under the config dir
  (`DOODUEL_STATE_DIR`-overridable for tests), the custom avatar 220×220
  **PNG-encoded + base64'd** (`image` + `base64`). `load_at_boot` restores pixels
  onto the paint surface + enqueues `Msg::Restore`; `persist_on_change` writes only
  when the persisted subset (theme/name/avatar-kind/**saved-pixel-version**)
  changes. Wasm = a stub (W7). A `saved_version` counter on `PaintCanvases` was
  added so persist writes AFTER the scratch→saved copy lands, not the frame before.
- `capture_w6.rs` — Home light, Home DARK (toggled through the funnel), in-game
  sketchy, + a persist→reload round-trip in a second GPU-free app.

**Ran the artifact** (`target/dooduel-captures/w6_*.png`, 1000×680, RX 6700 XT,
eyeballed + pixel-asserted): **Home light** — the wobble card + ink outlines + a
clean 3D-press purple **pill** (the lens is gone; no ears); **Home dark** — the
whole surface swapped (light-canvas px `401594 → 3734`, dark-canvas `391272`),
dark cards with **light** ink outlines (design-faithful), the lighter dark-accent
`#A78BFA`, and the widget-owned name input re-themed too; **in-game** — wobbled
ink-outlined panels + the timer ring + the painted canvas, all in dark. The
persistence round-trip **theme=dark, name=Mara, custom=true, avatar face px=1874
survived the PNG round-trip.** All asserts pass.

**Findings (classified):**
- **[framework-bug — FIXED, headline #1; rebless list EMPTY] The ears are gone.**
  Rounding the fill to the band inner radius closed the W3/W4 residual. Scope was
  tiny (reuse `inner_radius`, take the min). **Rebless list: empty** — every
  existing GPU golden is byte-identical because the framework's goldens never had
  a bordered+rounded fixture (the app had *avoided* the combination precisely
  because of this bug). So the fix is proven ONLY by the Dooduel captures (the
  cards/pills/panels now outline cleanly), not by a golden diff. The FINAL should
  ADD a bordered-rounded golden fixture so the fix has a regression guard.
- **[framework-bug — FIXED, headline #2] The radius-LENS is gone.** `min(rx, ry)`
  per corner in the band makes a wide bordered pill actually pill. This is what let
  me finally put an ink outline on the primary buttons (the design's whole button
  look) — before W6 they were borderless-rounded to dodge BOTH the lens (border)
  and the ears (fill). Byte-identical for uniform radii, so no golden moved.
- **[framework — per-corner RESIDUAL, journaled precisely] Two half-measures, both
  fine in practice.** (a) The FILL still carries ONE uniform radius (the
  `PackedInstance.radius` slot is shared with the shadow-sigma + text paths, so
  widening it to per-corner is a stride change that touches the shadow pipeline —
  too deep for a prototype wave). So under a *wobble* border the fill rounds to the
  MIN inner corner; at a much-larger wobble corner a faint fill "ear" can remain.
  It is invisible here because the wobble amplitude is small (±3px) AND the fill/
  canvas contrast is low (white-on-light, dark-on-dark) — the W4 "ears near-
  invisible on the light canvas" observation generalizes. (b) The band is per-
  corner but **circular** (`min(rx,ry)`), so the design's per-AXIS ellipticity
  (`26px/30px`) renders as a circular `26`. Neither is visible at Dooduel's scale;
  the FINAL wants per-corner fill radii (a wider `PackedInstance` or a dedicated
  rounded-fill instance) + an elliptical-corner SDF if per-axis is ever wanted.
- **[framework-bug — CONFIRMED, `LineStyle::Dashed` renders SOLID] Dashed verdict.**
  Two-layer gap: (1) `buiy_view::.border()` hard-codes `LineStyle::Solid`, so a
  dashed border is not even *requestable* from the view; (2) `band.wgsl` has no
  dash/dot pattern — `resolve_border` only skips `LineStyle::None`, every other
  style paints identically solid. So the design's dashed room-code box + join input
  are **approximated with a solid ink border** (accepted). A shader dash pattern is
  a real feature (screen-space arc-length stipple), too deep for a prototype; the
  FINAL should decide dashed/dotted borders (+ a `LineStyle` arg on `.border()`).
- **[missing-feature — press-state hook DOES NOT EXIST; 3D-press ships resting-
  only] Verdict.** A Buiy `view` is a pure `fn(&Model) -> Element`; a transient
  `:active`/pressed state is interaction state the runtime owns, NOT a field the
  view can read, so there is **no styling hook** for the design's press-down
  (`translateY + shadow-collapse`) transition. The 3D-press ships **resting-state
  only** (the chunky offset shadow always present). This is correct for MVU (a
  pressed *style* would be ephemeral non-model state); the FINAL path is either a
  controlled `:active` pseudo-state surfaced INTO the view (like `ControlledLeaf`),
  or an animation/transition layer that owns hover/press styling outside the model.
- **[framework — shadows don't follow the corner radius] The 3D-press is softened.**
  A Buiy shadow is a (blur-rounded) RECT — `resolve_shadows`/`shadow.wgsl` carry no
  corner radius (the `PackedInstance.radius` slot is the blur sigma). A hard
  (`blur 0`) `0 5px 0 ink` behind a Full-radius **pill** therefore shows square ink
  nubs at the pill's bottom-left/right (the pill curves in ~12px over the bottom
  5px). I softened the 3D-press to `0 5px 2px ink` (a 2px blur rounds the nubs
  away) — it reads as a chunky-but-soft underside rather than the design's crisp
  zero-blur "sticker" edge. Card `--sh-md` hard terms are fine (12% alpha, small).
  FINAL: give the shadow a corner radius (needs a wider instance) so a hard offset
  shadow follows the shape.
- **[finding — dark-palette VISUAL VERDICT, the 2nd typed-theme consumer, strong]
  The `Theme`-resource swap re-themes EVERYTHING, including widgets the app doesn't
  pin.** The cleanest theming outcome: the app pins its own ladder via a threaded
  `Palette` (surfaces/ink/tints), AND `sync_theme_resource` swaps the whole `Theme`
  resource `default_light_theme()`↔`default_dark_theme()` so the **widget-owned**
  surfaces the app can't reach (the `text_input`'s fill/text) re-theme for free.
  Verified in the dark capture — the name field went dark without app code. The
  design's `--ink` becoming light in dark mode (light outlines on dark cards)
  "just worked" because `INK` is a `Palette` field. VERDICT: typed themes scale;
  the two-layer split (app `Palette` for the design ladder + `Theme` resource for
  widget defaults) is the right seam. Residual: the app STILL hand-pins ~10 tokens
  the `Color` facade can't name (canvas/surface-2/ink-2/tints) — the facade is
  still too small (the recurring W3 finding).
- **[game-seam — theme as model-owned, replayable] `SetTheme` folds through the
  funnel; the toggle is a message.** The theme is a model field, so a theme swap is
  in the replay log like any nav; `sync_theme_resource` is the projection
  model→`Theme` (same shape as the paint/confetti syncs). Clean.
- **[DX — persistence, positive with one real gotcha] The write-timing bug.**
  serde_json + base64 + the `image` PNG round-trip were trivial. The one real
  gotcha: `persist_on_change` first fired the frame the avatar KIND flips to
  `Custom` — one frame BEFORE `sync_tools_to_canvases` copies scratch→saved — so it
  persisted a BLANK avatar and never re-fired (key unchanged). The capture caught
  it (face px = 0 on reload). Fix: a `saved_version` counter bumped on the copy,
  folded into the persist key, so the write re-fires once the pixels land. LESSON
  (recurring): a side-surface (the pixel buffer) that lags the model by a frame
  needs a VERSION the persist/observe key includes — the same "monotonic seq
  counter" shape the tool clear/undo already use.
- **[DX — positive] The render fixes were mechanical + surgical.** Both were "the
  data is already there, the consumer just collapsed it": fix 1 reused the band's
  `inner_radius`, fix 2 read the per-corner radii the instance already packed. Zero
  new fields, zero stride change, zero golden churn. The `AGENTS.md`-level
  factoring held up again.
- **[app-bug — mine, found by RUNNING] The lens.** The FIRST light capture rendered
  the Play button as a pointed EYE — the moment adding an ink border to a Full-
  radius pill exposed the latent band-lens. A headless probe would never have caught
  it; the recurring **RUN the artifact** lesson. Fixed in the band shader (min),
  re-captured → clean pill.

**Gate results:** `cargo test -p dooduel` **35/35**; `buiy_view` all green;
`buiy_verify` headless **206** snapshots + **22** GPU goldens byte-identical;
`buiy_core` headless (416/203/169/314/201/154 …) + GPU (**39** goldens) byte-
identical; `buiy` + `buiy_widgets` green; `cargo clippy -D warnings` clean on
buiy_core/buiy_view/dooduel; `cargo fmt` clean. **GPU rebless list: EMPTY** (both
render fixes are byte-identical for every existing fixture). capture_w6 renders +
asserts all three surfaces + the persistence round-trip.

**Surprised by / friction:** that the two render fixes touched ZERO goldens — I'd
braced for a rebless list and a careful quad-only-vs-bordered audit, but the
framework simply had no bordered-rounded fixture (the app had engineered AROUND
the bug). The friction was all app-side: the band lens (only the live render
showed it) and the persist write-timing (only the round-trip assert showed it).
The pleasant surprise was the `Theme`-resource swap re-theming the `text_input`
for free — I expected to have to reach into the widget.

**If we did this again:** add an ink border to ONE pill FIRST (the lens would show
instantly), and reach for the `min(rx,ry)` band + inner-radius fill together (they
are the same "the pill/panel finally outlines cleanly" change). And bump the
`saved_version` from the start — the frame-lag between a model flag and its side-
surface is a known shape now.

**Handed to W7 (web + mobile) / the FINAL:**
- **W7 (persistence):** `storage.rs` native is done + round-trip-proven; the wasm
  branch is a **stub** — W7 wires `localStorage` under the design keys
  (`dooduel-proto-theme` / `dooduel-proto-avatar`) via `web-sys`. The blob shape
  (serde `PersistedState`, base64 PNG) is web-ready; W7 just splits the two keys.
  The theme toggle + the `SetTheme` funnel path are wasm-safe (pure model + a
  resource swap).
- **W7 (touch/mobile):** the theme toggle is a pressable `button` — touch
  activation is the same gap as every other button (the [[buiy-wasm-support]]
  touch-activation follow-up). The wobble/shadow render on the web backends
  unchanged (band + shadow are existing pipelines; only `band.wgsl` changed, and it
  stayed within the WebGL2 16-attribute fold — no new vertex attributes).
- **FINAL re-decisions surfaced (W6-new):** (1) **per-corner FILL radii** (widen
  `PackedInstance` or a dedicated rounded-fill instance — retires the wobble-fill
  residual); (2) an **elliptical-corner SDF** if per-axis wobble is ever wanted;
  (3) **dashed/dotted borders** (a `LineStyle` arg on `.border()` + a band stipple);
  (4) a **shadow corner radius** (hard offset shadows that follow the shape — the
  crisp 3D-press); (5) a **controlled `:active`/pressed style hook** surfaced into
  the view (the press-down transition); (6) grow the `Color` facade to the theme's
  token vocabulary (the recurring "5 roles vs a dozen"); (7) a bordered-rounded
  GPU golden fixture so the ears/lens fixes get a regression guard.

### 2026-07-03 — W7 web + mobile

The wave that ships Dooduel to the browser (dual-backend wasm) + a phone layout,
and — the headline — is the FIRST time the whole app was ever driven by REAL
POINTER INPUT. That single fact surfaced a click-swallowing bug every prior wave
missed, and answered the campaign's biggest web-mobile unknown (does touch draw?)
decisively. The `buiy_view` `.ignore_picking()` addition is prime FINAL-candidate;
everything else is app/harness/prototype. Verified by driving BOTH backends in
headless Chromium (WebGPU on the real AMD adapter, WebGL2 on SwiftShader) with
mouse AND touch, 21 screenshots in `target/dooduel-captures/web_*.png`.

**Built (framework, `buiy_view` — prime FINAL candidate):**
- **`.ignore_picking()` (`Pickable::IGNORE`) — the interactivity fix.** A new
  `Element` modifier (field + builder + a drift-only `apply_ignore_picking`
  reconciler arm inserting/removing `Pickable::IGNORE`) makes a node transparent to
  picking — neither a hit-target nor an occluder — while its interactive CHILDREN
  stay pickable (the `pointer-events:none` on a container / `auto` on children
  pattern). The picking backend ALREADY had the pass-through machinery (and a
  comment describing this exact "topmost transparent box swallows every click"
  hazard); the view simply had no way to express it. Non-regressive: opt-in,
  default no-op, every `buiy_verify` snapshot byte-identical.

**Built (app + web crate):**
- **`examples/dooduel_web`** — the wasm entry mirroring `gallery_web`: dual-backend
  `webgpu|webgl2` features, a canvas-bound window (`fit_canvas_to_parent`), wired
  into `tools/build-web.sh` (two artifacts + the `navigator.gpu` loader). A shared
  **`dooduel::install_runtime`** (theme + clock + viewport + canvas + confetti +
  storage) now backs BOTH the native `main.rs` and the web bin, so the plugin set
  never drifts. Workspace member; native `--workspace` builds it as an ordinary
  windowed binary (canvas hint is a no-op off-web).
- **wasm-safe `dooduel` lib** — `x11`/`wayland` target-gated to non-wasm (unix winit
  deps never reach the web build); `web-sys` (`Window`/`Storage`) added for wasm.
- **`storage.rs` wasm branch** — refactored the persistence backend to a typed
  `load_persisted`/`save_persisted` seam (each target owns its encoding): native =
  the JSON file (unchanged); wasm = `localStorage` under the design's keys
  (`dooduel-proto-theme` string, `dooduel-proto-avatar` JSON — with the design's
  `Default = key-absence`/`removeItem` semantics — plus a `dooduel-proto-name` the
  design doesn't persist but the campaign wants).
- **Responsive shell** — `viewport_w/h` on the model, `Msg::SetViewport` folded
  `set_if_neq`-clean, a `ViewportPlugin` enqueuing on window-size change (the Tick
  pattern reused), and `is_mobile()` (~900px). Mobile screen variants: `in_game`
  single-column (top bar → header card → horizontal scoreboard strip → canvas →
  toolbar → chat, the design's phone collapse), pre-game card-width clamps, overlay
  `max_w` clamps, a mobile-trimmed top bar.

**Ran the artifact (both backends, mouse + touch):**
- **WebGPU desktop full drive** — boot → Home (full parity: logo blob, Caveat
  wordmark, doodle avatar + pencil badge, Play pill, Create/Join, opponents, theme
  toggle) → click Play → the in-game 3-pane with the "Pick a word!" scrim
  (WINDMILL/RAINBOW/UMBRELLA) → pick a word → **mouse-draw a stroke → black ink
  landed on the canvas**, word revealed "RAINBOW", "You're drawing".
- **WebGPU mobile (touch emulation)** — touch-tap Play → the single-column mobile
  in-game → tap a word → **touch-DRAG on the canvas → ink landed**. The whole
  phone flow is playable by touch.
- **WebGL2 (SwiftShader)** — the loader falls back (no WebGPU adapter) → boots +
  paints the full in-game screen + a Play click advances it (interactive). Faint
  diagonal streaks are a SwiftShader software-render artifact, not an app bug.
- **Persistence** — theme→**dark**, name→**"Zoe"** (typed), and a preset avatar all
  **round-trip a page reload** (verified via the rendered screenshots + the
  `localStorage` keys).

**Findings (classified):**
- **[framework-bug — THE headline, FIXED] A transparent `.fill().top_layer()`
  container swallowed EVERY click across the whole app.** The floating theme toggle
  is a full-viewport `.fixed().fill().top_layer()` column holding a bottom-right
  pill; the transparent box sits topmost in the pick order and occludes everything
  beneath (the picking backend truncates at the first occluder). It is present on
  every screen, so NOTHING was clickable. **Why no prior wave caught it:** the app
  was only ever exercised by the a11y `Action::Click` probe (which actions an
  entity by role+label, bypassing pick occlusion) + GPU screenshots — the web run
  is the FIRST real-pointer test. Root-caused by layer-by-layer instrumentation
  (temp global `Pointer<Press/Release/Click>` observers + a `Changed<Model>`
  logger): cursor → press → release → click ALL reached the button hierarchy, yet
  no message folded → an invisible occluder. Fixed with the new `.ignore_picking()`
  on the toggle container → interactivity restored app-wide (the click now
  penetrates the full button hierarchy and `Msg::Play` folds). **This is
  target-agnostic — it would fail on a native mouse too; only a11y-clicks hid it.**
- **[game-seam / framework — TOUCH-DRAWING VERDICT, the campaign's biggest
  web-mobile unknown, POSITIVE] Touch drawing reaches the paint observers with ZERO
  framework work.** A touch drag → a bevy touch pointer → `Pointer<Press/Drag/
  Release>` → the canvas' existing observers → `PaintSurface` → ink. `bevy_picking`
  derives touch pointers; the observers are pointer-id-agnostic; the drag's
  `PointerButton::Primary` gate holds for touch. The feared layer-by-layer touch
  debugging never happened (once the occluder was cleared). Buiy's pointer-driven
  raster canvas is touch-ready by construction — the single most important thing
  W7 proved for the FINAL's multi-agent playtest on phones.
- **[framework — touch-tap button activation, POSITIVE] A touch tap activates a
  Buiy button.** Tapping Play / a word choice advanced the model on the web build —
  the touch-activation path (a wasm-campaign follow-up) works for the buttons this
  app uses.
- **[framework-bug — HiDPI web scaling, NEW, needs a FINAL investigation] At
  `devicePixelRatio > 1` (every real phone / retina) the fresh-load UI renders
  scaled ~dpr× and overflows.** Reproduced dpr-proportionally: correct at dsf=1,
  ~2× too large at dsf=2, ~3× at dsf=3 (card + content spill past the viewport,
  top/right clipped). The responsive LOGIC is correct — `is_mobile`/`card_w` read
  the logical `window.width()` (390/412) — so the bug is in the RENDER/layout
  scale-factor handling on the wasm path, not the shell. This WOULD affect a real
  phone, so the mobile layout is only verified clean at dsf=1. (Related: a DYNAMIC
  window resize also mis-sizes the surface — the render confines to a sub-region —
  so verification used a fresh browser context per size; likely the same
  scale-factor / surface-reconfigure seam.) **FINAL: a focused buiy_core HiDPI/web
  scale-factor investigation.**
- **[DX — wasm storage, POSITIVE] The typed `load_persisted`/`save_persisted` seam
  split cleanly per target.** `web-sys` `Window`/`Storage`, best-effort (a private-
  mode / missing window is a silent no-op, never a crash). The design's two-key
  split + `Default = removeItem` matched exactly. `base64` + the `image` PNG codec +
  `serde_json` are all pure-Rust, so they compiled + ran on wasm untouched (the
  custom-avatar PNG round-trip is the same code as native, verified there). One
  gotcha: a native-only `use std::path::PathBuf` needed `#[cfg(not(wasm))]`.
- **[DX — responsive shell, POSITIVE, low cost] The viewport→model→mobile seam is
  the Tick pattern again.** A `ViewportPlugin` (kept out of `install`, like the
  clock) enqueues `SetViewport` only on a real size change; `set_if_neq` absorbs
  steady frames; `is_mobile()` derives the breakpoint; the view branches. No new
  framework primitive was needed — W3a's sizing modifiers (`.width/.fill/.grow/
  .justify_*`) sufficed; the cost was the mobile layout builders + width math (the
  card must fit INSIDE `screen_root`'s `Space::Lg` padding, else it overflows —
  found by RUNNING at 412px).
- **[missing-feature — mobile parity gaps, worked around] Three view-container gaps
  the phone layout hits.** (1) No view **scroll container** → the single column can
  exceed a short phone's height with no scroll (canvas-first ordering keeps the
  canvas + toolbar on-screen; the chat can clip). (2) No **horizontal scroll** → the
  scoreboard strip even-splits the ≤4 seats via `.grow()` instead of the design's
  overflow-x strip. (3) No **bottom-sheet modal** → the pick/reveal overlays stay
  centered scrims (clamped to the viewport width so they don't overflow a phone).
  All acceptable for the prototype; all real FINAL candidates for phone parity.
- **[DX — IME/soft-keyboard, POSITIVE with emulation limits] Typing works via the
  shipped `WebImePlugin`.** Focusing the name field + typing "Zoe" folded through
  the hidden-`<input>` bridge (synthesized `KeyboardInput`) and persisted — focus
  did not break the app. LIMIT: headless emulation cannot raise a real on-screen
  keyboard, nor exercise `ime_position` tracking or the touch-only policy (the known
  `WebImePlugin` follow-ups) — those need a real device.
- **[DX — fonts on wasm, POSITIVE] The `.ttf` embed "just works".** Caveat/Shantell
  are `include_bytes!`-embedded (not asset-server-loaded), so they render on both
  backends with no fetch — the task's font question answered.
- **[finding — dual-backend loader] The `build-web.sh` `navigator.gpu` loader is
  correct.** It picks WebGPU on the real adapter, falls back to WebGL2 (SwiftShader,
  `adapter:false`) with relocatable relative paths; both artifacts boot + paint +
  are interactive.
- **[HARNESS lessons — NOT app bugs; logged so W8 doesn't re-hit them] Four
  browser-driving traps.** (a) **A canvas is single-context** — probing `#buiy` with
  `getContext('webgl2')` permanently binds a WebGL2 context and POISONS bevy's
  WebGPU surface (`create_surface` panic). Never call `getContext` on the app
  canvas. (b) trunk emits **absolute** asset paths → loading a per-backend subdir
  directly 404s; drive the `build-web.sh` ROOT loader (relative), or rebuild with
  `--public-url`. (c) A **stale root loader** referencing an overwritten wasm hash
  silently loads nothing (blank) — rebuild BOTH artifacts via `build-web.sh` to keep
  the loader consistent. (d) **Cold synthetic click misses** — move → settle ~120ms
  → down → settle → up; a bundled `mouse.click` is too fast for the hover hit-test
  (the one-frame hover-lag lesson, confirmed).

**Gate results:** `cargo test -p dooduel` **35/35**; `buiy_view` + `buiy_verify`
all green (the `ignore_picking` addition is byte-identical for every existing
snapshot — opt-in default no-op); `cargo clippy -D warnings` + `cargo fmt` clean on
`buiy_view` / `dooduel` / `dooduel_web`; **both wasm backends build clean** via
`tools/build-web.sh examples/dooduel_web` (webgpu + webgl2 artifacts + loader);
native `--workspace` still builds `dooduel_web` as a windowed binary. `Cargo.lock`
gained `web-sys` (a deliberate wasm dep — committed). No native GPU suite run
(unchanged this wave).

**Surprised by / friction:** the whole app reading as non-interactive on the first
web run — a heart-stopping moment. Layer-by-layer instrumentation traced it NOT to
input delivery (cursor/press/release/click all landed on the button) but to a
transparent full-screen overlay swallowing clicks, invisible to every prior test
because they used a11y-clicks. Then touch drawing Just Worked on the first try. The
HiDPI dpr-proportional zoom was the other surprise. Also friction: the trunk
absolute-path / stale-loader / canvas-single-context harness traps cost several
cycles before the DIAG build loaded correctly.

**If we did this again:** RUN a real POINTER click on the NATIVE windowed app in an
early wave — the occluder bug is target-agnostic and was hidden only because
nothing ever pointer-clicked the app (the a11y probe + GPU capture both bypass pick
occlusion). And test HiDPI (dsf=2/3) from the first mobile boot, not last.

**Handed to W8 (multi-agent playtest) / the FINAL:**
- **W8:** the web build is drivable headlessly (playwright, both backends); MOUSE
  and TOUCH both reach the canvas + buttons; the DIAG pattern (global `Pointer`
  observers + a `Changed<Model>` logger, temporarily in the web bin) is the way to
  confirm folds when a drive "does nothing". The reusable flexible driver + the
  four harness lessons above are the playtest substrate; a per-agent-player drive is
  the same shape as these single-agent drives.
- **FINAL re-decisions surfaced (W7-new):** (1) **promote `.ignore_picking()`**
  (a real `buiy_view` gap — prime candidate); consider AUTO-`Pickable::IGNORE` for
  any transparent (`Color::NONE`) top-layer container so authors don't have to
  remember. (2) **the HiDPI web scale-factor bug** (blocks real phones — the top
  mobile FINAL item). (3) a view **scroll container** + **horizontal scroll** +
  **bottom-sheet modal** for true phone parity. (4) the `WebImePlugin`
  `ime_position` + touch-only-policy follow-ups (need a real device). (5) fold
  `install_runtime` / the `ViewportPlugin` shape into the FINAL's app scaffolding.

### 2026-07-03 — W8 playtest infrastructure

The LAST Phase-A wave: the machinery for the campaign's headline acceptance test —
**a live match with four separate LLM agents, one per seat.** Three deliverables:
the framework's missing **stroke/drag helper** (the never-proven gap the
re-baseline flagged; prime FINAL candidate), a long-running **playtest host** that
exposes a match over a file protocol, and a **scripted rehearsal** that plays a
whole turn through that protocol so the live agents won't hit infra bugs. All
three built + run + green.

**Built (framework, `buiy_verify` — the prime FINAL candidate):**
- **`PointerHarness::stroke(path)` / `drag(from,to,steps)` + a reusable
  `drive_stroke(app, window, pointer, path)` free fn.** Presses primary at
  `path[0]`, writes a `PointerAction::Move{delta}` per subsequent point (updating
  `PointerLocation` coherently via `PointerInput::receive`), releases at the last —
  so `bevy_picking`'s `pointer_events` derives `DragStart → Drag → DragEnd` on the
  PRESS target. This is the FIRST headless driver of bevy's drag machine (every
  prior harness test used `move_to`, which by design emits NO `Move` and never
  drags). A new `CapturedDrag` resource records the payload (per-move `delta`,
  start→end `distance`, position). [missing-feature — delivered]

**Built (app, `examples/dooduel`):**
- **`game.rs` Config knobs** — `pick_seconds`/`reveal_seconds` join the existing
  `draw_seconds` as Config fields (phase durations are now configurable, not hard
  consts — slow file-protocol agents need wide windows), and `bots_enabled` gates
  `due_bot_guesses` so all four seats are agent-driven. The consts stay as the
  defaults, so every existing test is byte-identical (35/35 unchanged). [decision]
- **`src/bin/playtest_host.rs`** — a native-only, GPU-free long-running host: the
  probe preset + a REAL-TIME `GameClockPlugin` + `CanvasPlugin`. It reads the model
  and writes, under `--dir`: **`seat_<i>_view.md`** (an honest per-seat report),
  **`state.json`** (phase/drawer/round/countdown/tick/canvas_ink/scores),
  **`canvas.png`** (the CPU paint buffer encoded with `image`, ~2 Hz on-change via
  a content hash), and **`host.log`**; and it polls **`commands.jsonl`**
  (start/pick/guess/stroke/clear/status/quit). Guesses + picks route through the
  REAL funnel (`Msg::Guess`/`Msg::ChooseWord`); strokes apply the proven
  `stroke_segment` path directly. Malformed lines are logged + skipped (never
  panics); every file write is atomic (temp + rename).
- **`scripts/playtest_rehearsal.sh`** — drives one full turn through the file
  protocol ONLY and asserts 21 checks.

**Ran the artifacts:**
- **Drag helper (`cargo test -p buiy_verify` pointer_drag_c7, 3/3):** a 4-step
  drag emits exactly 1 `DragStart` + 4 `Pointer<Drag>` (each `delta ≈ (40,0)`) +
  1 `DragEnd` (`distance = (160,0)`), all on the press target — headless, no
  window, no GPU. The re-baseline recipe worked EXACTLY as predicted.
- **Canvas end-to-end (`examples/dooduel/tests/canvas_e2e.rs`, 1/1):** the unified
  driver boots Dooduel + the canvas, drags a real synthetic pointer across the
  laid-out canvas rect, and the app's OWN paint observers land INK in the
  `PaintCanvases` buffer at the stroked pixels — the full input → funnel → canvas
  path proven with no GPU.
- **Rehearsal (`bash …/playtest_rehearsal.sh`, 21/21):** `start → picking`
  (drawer 0) → drawer picks `bicycle` → `drawing` → a rectangle stroke
  (`canvas_ink=13468`, `canvas.png` written) → a non-drawer stroke REJECTED → three
  seats guess `bicycle` (scored 478/392/321, drawer 100) → reveal → next turn
  (drawer 1). Transcript excerpt: `pick: seat 0 chose word[0] "bicycle" (funnel)`
  / `stroke REJECTED (seat 1 is not the drawer)` / `guess: seat 1 "bicycle"`.

**Findings (classified):**
- **[framework — THE drag-helper verdict, POSITIVE; re-baseline CONFIRMED] The
  Press → N×`Move{delta}` → Release recipe is exactly right.** The re-baseline's
  reading of `bevy_picking` was correct to the letter: `PointerInput::receive`
  (scheduled by the core `PickingPlugin`, which the harness has) updates
  `PointerLocation` from each `Move` message's location, and `pointer_events`
  derives the drag from `location.position - drag.latest_pos` on the entity captured
  at PRESS — so the drag fires on the press target even if the pointer leaves it,
  and a zero-delta step is skipped. `move_to` (a direct location write, no `Move`)
  provably does NOT drag (a dedicated test asserts it), which is exactly why a real
  stroke helper was the missing piece: a naive "sequence of `move_to`s" would have
  silently drawn nothing. NO framework bug — the helper is pure additive surface.
- **[game-seam / framework — THE unified-driver verdict] ONE App runs the GPU-free
  probe preset AND real picking; the recipe has exactly ONE non-obvious piece.**
  `BuiyProbePlugin` (the MVU funnel + reconciler + a11y/text/widgets, no render, no
  picking) composes cleanly with the picking stack — because the probe preset
  simply *omits* picking, adding it back conflicts with nothing. The full recipe:
  `MinimalPlugins + TransformPlugin + AssetPlugin + InputPlugin +
  bevy::picking::PickingPlugin + BuiyProbePlugin + buiy_core::picking::PickingPlugin
  + BuiyPickingBackendPlugin`, a `Camera2d` + `PrimaryWindow` + a `PointerId::Mouse`
  pointer, then `dooduel::install` + `CanvasPlugin`. The ONE gotcha, found by a
  "Resource does not exist" panic and bisected: **`app.init_asset::<Image>()`** —
  `CanvasPlugin` creates + mirrors a bevy `Image`, but the probe preset never adds
  `ImagePlugin`/`RenderPlugin` (which normally register `Assets<Image>`), so a
  headless canvas host must register the asset itself. Scroll/animation are NOT
  needed (confirmed by removing them). This IS the "unified headless driver"
  answer, and the FINAL's playtest scaffolding should bake in the `init_asset` line.
- **[game-seam — the honest per-seat view design] Project the game onto a clone at
  `viewing_as = i` and reuse `word_display()` — never a hand-rolled per-seat
  redaction.** The one correctness risk in a per-seat report is leaking the secret
  word to a guesser. Rather than re-deriving "who may see the word," the host clones
  the `Game`, sets `viewing_as = seat`, and calls the SAME `word_display()` the UI
  uses — so the redaction logic (drawer / already-guessed / reveal see the letters;
  everyone else sees blanks + hint-revealed positions) has exactly one home and
  cannot drift between the UI and the host. Verified live: with the secret
  `scarecrow`, the drawer's view shows `S C A R E C R O W` while seat 1's shows
  `_ _ _ _ _ _ _ _ _`. The chat tail is safe to show every seat (the secret only
  enters chat at reveal, as `The word was "…"`, which is public then).
- **[game-seam — the funnel-vs-harness seam, journaled] Gameplay through the
  funnel; lifecycle direct.** Guesses + picks are enqueued as real `Msg`s
  (`apply_guess`/`choose_word` do the authoritative validation — the drawer can't
  guess, no repeats, wrong-phase rejected), so the host exercises the identical
  pipeline a human or bot does. The match START is a host-harness action (direct
  `game.start_match(name, config)` on the model) so the host can inject its custom
  Config (bots-off, wide durations) without adding a config-carrying `Msg` to the
  shared reducer. Strokes apply the real `stroke_segment` path directly (input
  fidelity is already proven by the D1 drag test + W7's browser drives), which keeps
  the host free of the picking stack entirely.
- **[game-seam — the real-time clock knob] `Time` is wall-clock, so phase durations
  are real seconds regardless of loop pacing.** The host runs `GameClockPlugin`
  (the `Res<Time>` → `Msg::Tick` driver) and paces its own `app.update()` loop with
  a small sleep. Because `MinimalPlugins`' `TimePlugin` advances the generic `Time`
  from the real clock, `Game::tick`'s `now − anchor` arithmetic yields real
  wall-clock countdowns even if the loop stalls on file I/O — so `DOODUEL_DRAW_SECS`
  et al. are honest seconds. The phase anchors on the first tick AFTER `start`, so
  idle time before `start` never eats the draw window. Defaults chosen for slow
  agents: **draw 120 s, pick 45 s, reveal 12 s** (a file-poll + think cycle easily
  exceeds the interactive 10 s pick window, so the const durations had to become
  Config knobs — otherwise the drawer would always auto-pick before an agent
  responded).
- **[DX — positive] Atomic file writes + a byte-cursor command log = a robust
  protocol.** Every host→agent file is written temp-then-`rename` (a polling agent
  never sees a half-written `state.json`), and `commands.jsonl` is consumed by a
  byte offset that only advances past `\n`-terminated lines (a partial trailing line
  is left for the next poll) — so an agent appending a line mid-poll is safe.
  Malformed JSON is logged + skipped. This made the rehearsal deterministic on the
  first real run.
- **[DX — the rustc stack-overflow trap, logged for the FINAL] The big bevy bins
  SIGSEGV the COMPILER under the default stack.** Building `dooduel`'s capture bins
  (and the new host) intermittently crashed `rustc` with a SIGSEGV during
  monomorphization — a compiler stack overflow, not a code bug. `RUST_MIN_STACK=33554432`
  (32 MiB) fixes it. Worth pinning in the FINAL's build docs / CI env for the
  example bins.

**Surprised by / friction:** the drag recipe needed ZERO iteration — it worked the
first run exactly as the re-baseline predicted, which is a strong signal the
re-baseline's `bevy_picking` reading was precise. The one real snag was the
unified-driver's `Assets<Image>` panic (opaque "Resource does not exist" with the
system name stripped by `debug=0`), which a 4-step plugin bisect pinned to the
missing `init_asset::<Image>()`. The pleasant surprise was that `ScrollInputPlugin`
turned out NOT to be needed (I'd added it defensively mirroring `PointerHarness`);
removing it kept the recipe minimal + honest.

**If we did this again:** reach for `init_asset::<Image>()` immediately when putting
`CanvasPlugin` on any non-render preset (it's the render plugin's job normally), and
bisect a "Resource does not exist" by plugin from the start rather than reading
backtraces (they're symbol-stripped under `debug=0`). Otherwise the same shape — the
`drive_stroke` free fn (shared by the harness + the foreign-app canvas test) and the
clone-and-project honest view are the load-bearing pieces.

**READY for the live multi-agent playtest.** The infra is proven end-to-end by the
21-check rehearsal. Launch: `cargo build -p dooduel --bin playtest_host` (with
`RUST_MIN_STACK=33554432`), then
`target/debug/playtest_host --dir <shared-dir>` (env knobs to taste); point four
agents at `<shared-dir>/seat_{0..3}_view.md` + `state.json` + `canvas.png` (read)
and `<shared-dir>/commands.jsonl` (append). The drawer agent picks + strokes; the
others guess; the host advances the match in real time.

**Handed to the FINAL:**
- **Promote the stroke/drag helper** — `PointerHarness::stroke`/`drag` +
  `drive_stroke` are prime FINAL-candidate `buiy_verify` surface (the only new
  primitive this wave; kept clean). Add a golden/reftest fixture that DRAWS via the
  helper if a rendered drag is ever wanted.
- **Bake `init_asset::<Image>()` into the headless canvas recipe** (or have
  `CanvasPlugin` register it defensively via `is_plugin_added`-style guard) so a
  no-render host/test never hits the panic.
- **The file-protocol host is a FINAL-shippable playtest harness** (per-seat honest
  views + funnel-routed guesses + real-time clock). Open items if it graduates from
  prototype: a richer `state.json` (word length, hint count, per-seat "can I act
  now" flags), an in-protocol `continue`/skip-reveal, and streaming the chat as a
  separate file so agents needn't diff the whole view.

### 2026-07-03 — W8-live: THE MULTI-AGENT PLAYTEST (acceptance bar — MET)

Four independent LLM agents each played a different seat of a real match through
`playtest_host`'s file protocol (evidence preserved:
`2026-07-02-dooduel-PROTO1-playtest/` — the full `commands.jsonl` transcript,
`host.log`, final `state.json` + `canvas.png`, a sample honest seat view).
**The match completed end-to-end**: 1 round × 4 turns, each agent drew once
(rainbow / castle / saxophone / guitar — authored as stroke polylines, recognized
by the other agents from the rendered `canvas.png` alone), every word guessed by
every guesser, final podium Priya 1052 / Theo 980 / Alex 816 / Sam 686. All
commands routed through the real MVU funnel; the honest per-seat views held
(drawer saw the word, guessers saw blanks). All four agents independently rated
it "genuinely fun" and praised the canvas ("crisp, correctly-colored, instantly
recognizable — the drawing primitives are the star").

**Consolidated findings** (converging across the 4 independent reports):
- [app-bug] Round counter overflows at match end — the podium reads "Round 2/1"
  (`round=2, total_rounds=1`); `total_rounds` also reads 2 pre-start vs 1
  in-match (stale default before host config applies).
- [app-bug] Stale drawer at podium — `drawer`/`drawer_name` never cleared; the
  final screen says "Drawing: Alex" and tags him "(drawing)".
- [app-bug] **The drawer plays blind** — "X guessed the word!" chat lines only
  became visible to the drawer AFTER the turn flipped; Theo wasted a clear+redraw
  on a turn that was already fully guessed. Wants a live guessed-count + in-phase
  chat delivery.
- [app-bug/product] No feedback for wrong guesses — no echo of your own guess,
  nothing on a miss; the close-guess ("X is close!") path was NEVER exercised in
  live play (all guesses exact) and no agent saw a mechanism for it — verify it
  works + echo guesses back to the guesser's chat.
- [needs-verify] Hint letters NEVER revealed across all 4 turns (fast guesses may
  have preempted the schedule — or the reveal path is broken; targeted test).
- [host-bug] Countdown labeled "Ns left" decrements at a non-wall-clock rate (two
  agents measured ~1.3/s and ~4–5/s) — ticks-vs-seconds mismatch in the host
  view; and there is no signal that a turn is about to end early on all-guessed.
- [product] The canvas appears ALL-AT-ONCE to guessers (agents batch strokes +
  the host flushes ~2 Hz) — the partial-art race that makes skribbl tense is
  absent from the playtest transport. (The real UI streams strokes live — W7's
  browser drives prove it — so this is playtest-host + agent behavior; a
  per-stroke PNG flush would restore the dynamic for agent play.)
- [product/design-question] Drawer payout is a flat +100 regardless of how
  many/fast guessers succeed → your drawing turn is a guaranteed score deficit
  (Sam guessed everything first-try and still placed last). Check the design's
  drawer formula (skribbl scales by guesser success). Also: no "turn X of N"
  indicator; the previous drawing lingers through the next Picking phase.

**Verdict**: the acceptance bar — "playtested by multiple agents, each playing a
different player" — is MET, on the first live run, with real gameplay bugs found
that no prior gate (probe tests, GPU captures, browser drives) could see. The
playtest-as-a-gate thesis is validated.
