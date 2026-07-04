# apps/dooduel — the Dooduel game ported onto the landed F1–F9 framework surface (App-1)

The flagship App-1 wave: the audited prototype's **DOODUEL** (a skribbl.io-style
draw-and-guess game) graduated out of `examples/` into the first `apps/` workspace
member, as production code on the now-complete framework surface. Native desktop +
web (dual-backend) + the multi-agent playtest host.

**Scope guard:** this PR is the *faithful* port (App-1). The six playtest-found
gameplay bugs are still present here — fixing them (each with a regression test) is
App-2's wave. See the App-2 hand-off list below.

## Module map (`apps/dooduel/src/`)

- `game.rs` — the PURE game core, ported **verbatim** (phase machine, scoring,
  hints, guess normalize/levenshtein/is_close, seeded splitmix64 bots, the honest
  `word_display()`/`word_slots()` redaction). Zero framework coupling; unit-tested.
- `paint.rs` — the keyed `PaintCanvases` resource (`HashMap<CanvasKind, PaintSurface>`)
  + the model→canvas tool projection + Press/Drag/Release observers. **New:** a
  round-clip `Border` stamped onto the custom-avatar display rasters (F4b raster
  rounded clip; the view `raster()` element does not lower `.radius()`).
- `storage.rs` — the typed per-target persistence seam (native JSON file /
  `DOODUEL_STATE_DIR` override / wasm localStorage) + avatar PNG-base64 + the
  `saved_version` one-frame-race guard. Ported verbatim.
- `theme.rs` — the two-layer theming the prototype validated: the app `Palette`
  (LIGHT/DARK protokit ladders, exact sRGB) + the `Theme`-resource sync (accent +
  base ladder swap on `SetTheme`). Ported verbatim on the F3 `Color` surface.
- `avatar.rs` — the 22 `DoodleAvatar` doodles + `hash_str` + the badge builder.
  **Re-decided against F3:** native 40-box coordinates + the `icon()` viewbox arg
  (drops the prototype's 0.6 pre-scale) + the design's `1.5px rgba(0,0,0,.22)` ring
  (F4b bordered-rounded fill makes the ring + tinted fill round cleanly).
- `confetti.rs` — the decoupled podium `ConfettiPlugin` (a rising-edge model side
  effect). **Re-decided against F4b:** the end-of-life fade now uses `QuadAlphaTween`
  (composite-free per-quad alpha) instead of the dropped `OpacityTween`.
- `lib.rs` — the MVU shell: the `Dooduel` model + `Msg` + the thin reducer + `ui()`
  install + the app-authored `install`/`install_runtime` plugin sets. **F7:**
  `ClockPlugin::<Dooduel>::new(Msg::Tick)` replaces the hand-rolled `GameClockPlugin`.
- `view/` — the per-screen split of the prototype's 3082-line `lib.rs`:
  `mod.rs` (the `Screen` router) + `widgets.rs` (shared helpers) + `home.rs`,
  `join.rs`, `lobby.rs`, `in_game.rs`, `podium.rs`, `avatar_editor.rs`.
- `src/main.rs` (windowed native), `src/bin/capture.rs` (per-screen offscreen
  render-to-texture + readback), `src/bin/playtest_host.rs` (the file-protocol
  multi-agent host), `dooduel_web/` (the dual-backend wasm crate).

## The seam-by-seam re-decision ledger (prototype pattern → landed surface)

| Prototype (hand-rolled) | FINAL (landed surface) | What changed |
|---|---|---|
| `GameClockPlugin` + `drive_tick` (6-line `Res<Time>`→`Msg::Tick`) | `ClockPlugin::<Dooduel>::new(Msg::Tick)` (F7) | The poll-clock is now a reusable plugin; suppressed during replay by construction. |
| `icon(d, size, stroke)` + a hand-rolled 0.6 coord/stroke pre-scale (24-box) | `icon(d, size, stroke, viewbox=40)` (F3) | The viewbox arg carries the coordinate space; `avatar.rs` drops `scale_path`/`VIEWBOX_SCALE` and passes native 40-box design coords. |
| `.border(w, color)` (Solid hard-coded) | `.border(w, color, LineStyle)` (F3) | Every border is 3-arg; the room-code box + join input are `LineStyle::Dashed` (F4b renders them — the prototype's dashed rendered solid). |
| avatar badge ring omitted (no border surface) | `.border(1.5, rgba(0,0,0,.22), Solid)` on the badge (F4b bordered-rounded fill) | The design's dark ring is back — the "ears" fix keeps the tinted fill round under the ring. |
| custom-avatar raster renders SQUARE | round clip via an `Added<RasterImage>` `Border` stamp (F4b raster rounded clip) | Round custom avatars — see the DX finding (the view `raster()` doesn't lower `.radius()`). |
| confetti `OpacityTween` dropped (would form ~110 effect groups) | `QuadAlphaTween` (F4b per-quad alpha) | The end-of-life fade is back, composite-free. |
| chat log capped to the last 12 (no scroll) | `keyed_column(...).stick_to_bottom()` (F2 scroll) | The design's auto-scroll-to-bottom; the cap is gone. |
| 16 swatches split into two explicit rows (no flex-wrap) | one `Element::row(...).wrap()` (F2) | One code path; wraps as the width shrinks. |
| mobile scoreboard even-split via `.grow()` | `.scroll_x()` (F2) | The design's overflow-x strip. |
| theme toggle needs explicit `.ignore_picking()` (the occluder fix) | F6 auto-`IGNORE` for transparent top-layer containers | Kept explicit as belt-and-suspenders; the bug is now unwritable. |
| avatar editor = a full in-flow screen (raster-under-modal hidden) | **kept a full in-flow screen** — the F4a "modal" re-decision is BLOCKED (see finding) | RUNNING the modal showed base text bleeding through. |

## Capture evidence (offscreen GPU render-to-texture, eyeballed vs the design bundle)

`apps/dooduel/target/dooduel-captures/`: `home.png`, `home_dark.png`, `lobby.png`,
`join.png`, `in_game_drawer.png`, `in_game_picking.png`, `podium.png`,
`avatar_gallery.png`, `avatar_draw.png`. All at strong design parity — Caveat/Shantell
type, doodle avatars with the ring, the crisp 3D-press shadow (F4b rounded-shadow
instance), dashed room-code box, the framed canvas raster, the scrim overlays, the
winner-center-tallest podium.

## DX findings (the campaign's third product — building a real game on Buiy)

1. **Top-layer modals cannot occlude base-layer text (blocks the F4a modal
   re-decision).** Glyphs draw in one global tier after all quads
   (`node.rs`: "shadow < quad < gradient < glyph", no per-top-layer glyph partition),
   so a top-layer modal's opaque background can never hide base-layer text behind it.
   F4a fixed *raster* interleave, not glyph-over-quad occlusion. The avatar editor
   over Home (all text) bled; the in-game pick/reveal overlays look clean only because
   a blank canvas sits behind them. Reverted the avatar editor to a full in-flow
   screen. **Framework follow-up:** the glyph tier must respect stacking layers (or
   `top_layer` must composite its subtree as a group).
2. **`on_submit_with` is unsafe under a ticking model.** The reconciler
   unconditionally re-patches a matched `text_input` to the view's value
   (`set_editor_value`), so an uncontrolled `text_input("").on_submit_with(...)` gets
   its in-progress text clobbered to `""` on any model change — and Dooduel's clock
   ticks every second. The in-game guess input **must** use the controlled
   `on_input` + `on_submit` pattern. `on_submit_with` is only safe when the model is
   static between keystroke and submit. **Framework follow-up:** skip
   `set_editor_value` for a focused/uncontrolled editor, or gate it on a real change.
3. **The view `raster()` element does not lower `.radius()`/`.border()`** (the
   `Kind::Raster` reconcile arm skips `apply_border`, unlike `Kind::Icon`). Rounding a
   raster (F4b raster clip) needs a `Border` on the `RasterImage` entity, which the
   app attaches via an `Added<RasterImage>` observer. **Framework follow-up:** lower
   raster styling like the icon arm does.
4. **Spacing/gap are token-only (`Space` enum: 4/8/16/24/32).** A design with
   arbitrary px paddings rounds to the nearest token. Not a blocker, but a parity
   ceiling for pixel-exact spacing.
5. **`PressEffect` depth is fixed at 2px with no view-level setter.** The design's
   3D-press is a 5px dip + shadow-collapse; the F5 press dip is automatic but not
   tunable from the view (no `.press_depth()`), and the shadow doesn't collapse.

**Wins:** the render spine took a whole new sampled primitive mechanically; F2's
coherent surface (`.wrap()`, `.scroll_x()`, `.stick_to_bottom()`, per-side padding)
removed three prototype workarounds in one pass; F4b's dashed borders + crisp
rounded shadow + composite-free particle alpha closed three character-layer residuals;
`keyed_column` + the funnel + the rising-edge side-effect observer all behaved
exactly as documented; the GPU capture harness composed cleanly for per-screen
verification.

## Deviations (documented)

- **Podium: winner center + tallest** — the design code's rank-indexed `PODIUM_H`
  under `PODIUM_ORDER = [1,0,2]` literally renders 2nd-place tallest (an apparent
  indexing quirk). The FINAL renders winner-center-tallest (spec §5.g, grounded by
  the real skribbl.io podium; pending designer ratification). A two-constant revert.
- **Avatar editor: a full in-flow screen, not the design's scrim modal** — forced by
  DX finding #1 (the same deviation the prototype accepted).
- **Color emoji stripped from chat + copy** — the bundled Latin fonts have no
  color-emoji glyphs; deferred to its own campaign (spec §5.f).

## App-2 hand-off (bugs observed, NOT fixed here — App-1 is the faithful port)

1. Round-counter overflow at the podium (`total_rounds` stale default vs config).
2. Stale drawer at the podium (`drawer`/"(drawing)" tag never cleared on `Final`).
3. The drawer plays blind (no live `guessed_count`/in-phase "guessed" chat).
4. No wrong/near-miss guess feedback surfaced (the design-exact three-way).
5. Hint reveal — schedule is ported (`game.rs`) but never fired live + unverified
   render; needs the threshold-crossing test.
6. Countdown units in the host report (ticks-vs-seconds). *(App-1 uses `ClockPlugin`,
   which drives `Msg::Tick(Time::elapsed())` at wall-clock; App-2 owns the fix + test.)*

Plus the graduated `playtest_host` (richer state.json, per-stroke streaming, chat as
a separate file) — App-2's §3.4 scope. App-1 ships the prototype's proven host verbatim
(with the F7 clock swap).

## Gate

_(filled after the run — SG headless + game.rs unit tests + the F8 canvas e2e +
per-screen captures + the dual-backend wasm build-check.)_

🤖 Generated with [Claude Code](https://claude.com/claude-code)
