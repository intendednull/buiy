# Dooduel re-baseline audit — gap status against main @ `a969cbf` (2026-07-02)

> **Status: research input to Phase A (whole-app prototype).** Re-baselines the
> 2026-07-01 capability gap audit (`2026-07-01-scribbl-app-capability-gap-audit.md`)
> against current `origin/main` (`a969cbf`), which now includes the **view-authoring
> campaign** (buiy_view, PRs #106/#110/#111) and the **LLM-dev-support campaign**
> (Tracks B/A/C/D, PRs #113/#117/#120/#121/#123/#124/#125) — both merged after the
> audit's base (`6c4ff22`). **This report's gap tiers supersede the audit's.**
> Method: four parallel read-only agents (surface map, Tier-1 re-check, Tier-0/2
> re-check, probe/playtest + game seam) on a fresh worktree, file:line-verified.
>
> **Superseded where they overlap (Phase A has since run to completion):** the
> re-baselined build list and scope below are superseded by the
> [PROTO1 retrospective](../prototypes/2026-07-02-dooduel-PROTO1-RETROSPECTIVE.md)
> — whose §1 scores this report's predictions against the prototype's actual
> outcomes — and by the [Dooduel FINAL design](../specs/2026-07-03-dooduel-final-design.md);
> consult those first.

## Headline

Of the audit's Tier-1 gaps, **G1 (declarative view + keyed lists) and G2
(press→Msg routing) are DELIVERED** by buiy_view; **G3 (timer/Subscription)
remains the one Tier-1 framework gap**. Tier-0 (the drawing canvas) is unchanged
as the sole net-new subsystem, but is **materially de-risked on the GPU side**.
The light theme claim in the audit is stale — Track B shipped a complete light
palette. Two web items the campaign carried as deferred (touch activation, web
soft-keyboard/IME) actually **landed**. The multi-agent playtest needs a
**unified headless driver** (probe "eyes" + pointer-harness "hands" + a net-new
stroke helper) — no shipped preset does both.

## Verdicts (per gap)

### Tier 0 — drawing canvas (G0): UNCHANGED-MISSING, GPU-side de-risked

- Still no texture-presenting node anywhere; the GPU primitive set is closed:
  `BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }` (`render/buckets.rs:35-45`).
- **New since audit:** a reserved `Path` primitive slot exists — no producer, no
  dedicated shader (falls back to quad, `render/primitive.rs:240`). The
  architecture has pre-carved where a dynamic-vector stroke channel would land.
- **De-risk:** full render-to-texture machinery already exists — offscreen
  `Image::new_target_texture(Rgba8UnormSrgb)` + `RenderTarget` + readback in the
  test/golden harness (`tests/support/mod.rs:426-466`, `render/golden.rs:260-296`),
  and the render node already does sample→draw-quad→attachment ping-pong
  (`render/node.rs:707-840`). The pipeline binds sampled textures today (R8
  coverage atlas, `render/pipeline.rs:124-136`). An `ImageNode`-style instance
  kind is **app-facing wiring + a paint pass, not net-new GPU capability**.
- Fill surface today is solid + gradients only (`render/components.rs:35-128`) —
  no raster path for app content. The view surface likewise has no image/canvas
  element kind and no pointer events (drag stays raw-ECS, which works:
  Press→Drag→Release + canvas-local coords, the text-editor pattern at
  `text/edit/pointer.rs:146-200`).
- **Flood fill** (design requirement) needs CPU-side pixel state → still weighs
  toward the **accumulation-render-target strategy with a CPU-authoritative
  buffer** (paint into a CPU image, upload dirty regions to the target texture).
  Decide by prototype, as chartered. The paint subsystem serves **two** surfaces
  (game canvas + 220×220 avatar editor).

### Tier 1 — MVU app-ergonomics

- **G1 declarative view + keyed reconcile: DELIVERED.** `ui(init, update, view)`
  installs model+reducer+routers+reconciler in one call (`buiy_view/src/app.rs:83-148`).
  `keyed_column(iter, key_fn, view_fn)` does real keyed reconciliation — identity
  + widget-state preserved across reorder/insert/remove, order pinned via
  `replace_children`, duplicate-key debug panic (`reconcile.rs:293-362`), and
  replay reconstructs keyed rows from the model (closes the audit's replay
  caveat; `tests/todomvc.rs:371-464`). Scoreboard/chat/word-picker lists are
  solved. Auto-scroll-to-bottom stays app-level (`ScrollOffset` on append).
- **G2 press→Msg routing: DELIVERED.** `button("…").on_press(Msg)` is the whole
  thing; `ui()` auto-installs generic press/input/submit routers
  (`router.rs:42-103`). `on_press_maybe`, `on_toggle`, capturing `on_input_with`
  (#111), `on_submit` all present. (The view path bypasses C3's `ValueChange<T>`
  — that is the lower-level widget-observation channel; both exist.)
- **G3 timer/Subscription: PARTIAL — the one remaining Tier-1 framework gap.**
  `Cmd::task` (one-shot async, wasm-clean via BoxedFuture, replay-safe) landed
  (`mvu/mod.rs:235-243, 703-722`). Recurring tick/interval/Subscription is still
  reserved roadmap §8 (`Origin::Subscription` "not emitted in v1",
  `mvu/mod.rs:317-319`). The game clock must be the hand-written per-frame
  `Res<Time>` → `enqueue(Tick(now))` driver; the pattern is proven cheap
  (bench fixture `buiy_bench_support/src/mvu_scenes.rs:175-336`: fold `now` →
  derived phase, `set_if_neq` keeps steady frames at zero rebuilds) but ships as
  a fixture, not a production primitive. The only delay idiom in-tree
  (`examples/scaling_view` thread-sleep on the compute pool) is **not
  wasm-viable** — Dooduel targets web, so the Tick driver is required.
  **This is the framework feature Dooduel most clearly forces** (keyed
  Subscription / `every()`, wasm-safe).

### Tier 2 — smaller primitives

- **G4 timer-ring arc: UNCHANGED-MISSING.** Icon renders static SVG arcs
  (lyon → R8 coverage atlas) but the atlas is content-addressed by geometry
  (`icon_producer.rs:103`) — a smooth per-frame sweep churns a new atlas cell
  every frame. Bounded workaround: regenerate the arc once per second
  (~10–80 content-addressed cells per turn, grace-evicted). A smooth sweep needs
  a real arc primitive or the unbuilt Path channel — prototype decides if
  per-second steps read acceptably against the design.
- **G5 MVU controlled Dialog: PARTIAL (now small).** The machine template
  exists — `MenuModel` + `fold_one_inline` in widgets (`menu.rs:42,155`) and the
  model-agnostic `DismissRegistry` (`dismiss.rs:102-140`) which resolves the
  documented second-writer race vs `CssVisibility`. Dialog itself is still
  imperative (`dialog.rs`); a `DialogModel {open}` clone of the Menu machine is
  a small, well-templated job.
- **G6 Toast + stack: UNCHANGED-MISSING** at framework level; the gallery ships
  a copyable single-toast reference (tween entrance + auto-dismiss,
  `capture_composites.rs:33,107`).
- **G7 theming: PARTIAL — much closer than audited.** The audit's "9-token light
  stub" is **stale**: `default_light_theme()` now sets every Palette field with
  real values (`theme.rs:404-476`, Track B #113) — complete but never visually
  validated (no light-default consumer yet; **Dooduel would be first**). Accent
  swap is live (`SetAccent`, ramp derived at resolve) — purple = one call at
  boot. Still missing: a **light/dark toggle** (`PaletteMode` is
  Normal/ForcedColors, not light/dark; switching = swap the whole `Theme`
  resource — no primitive) and **persistence** (no localStorage-analog/native
  file store; also needed for the custom avatar).

### Dooduel character pass (new demands from the design delta)

- **Wobbly asymmetric borders: DELIVERED.** Per-corner, per-axis elliptical
  radii (`Corners`/`Radius {x,y}`, `render/components.rs:327-396`) map CSS
  `26px 32px 24px 30px / 30px …` directly.
- **Chunky 3D-press shadows: DELIVERED.** `Shadow {offset, blur: 0, spread}`
  supports hard offset shadows (`components.rs:406-424`); press = translate down
  + collapse shadow. Colors are `ColorToken` (use `Custom` for ink).
- **Dashed ink borders: enum exists (`LineStyle::Dashed`,
  `components.rs:308-319`), shader rasterization UNVERIFIED** — flag for an
  early prototype probe (may collapse to solid).
- **Confetti: expressible, no particle system.** Per-piece entity +
  `TranslateTween`/`RotateTween` + despawn-on-complete. One-shot burst of ~100
  nodes = one structural re-extract (fine); the podium's repeated bursts need
  watching. A burst helper is hand-rolled.
- **Fonts (Caveat / Shantell Sans): DELIVERED with a packaging constraint.**
  `FontRegistry` + `FontStack` fallback chains work; the loader is **sfnt-only
  (ttf/otf/ttc) — no woff2** (`text/font_asset.rs:23-24`). Ship/embed the
  `.ttf`s (embed for wasm/offline).

### Playtest + game-engine seam

- **Probe (the "eyes"): DELIVERED.** `snapshot_report` = Playwright-style tree +
  text/layout section with `[ZERO-SIZE]` flags; drivers = `perform` (10
  node-addressed AccessKit verbs), `click`/`focus`/`set_value` (runs the real
  edit pipeline)/`get_by_role`/`wait_for`. **Guesser agents need nothing more.**
  No pixels (GPU-free by design) — drawn-ink verification belongs to the GPU
  `--ignored` render-to-texture lane, not the probe.
- **Drag (the "hands"): NET-NEW helper required.** `BuiyProbePlugin` omits
  picking; `buiy_verify::PointerHarness` drives the production picking pipeline
  headless (press/release/click/move_to/scroll/touch_tap) but **its `move_to`
  never emits a Move action → never trips bevy's drag machine; no test anywhere
  drives a headless `Pointer<Drag>`**. The recipe is confirmed in bevy source
  (Press@p0 → N×`PointerAction::Move{delta}` → Release; drag events fire on the
  press target), and a `write_action(Move)` stroke helper follows the existing
  `scroll` precedent — architecturally trivial, genuinely unproven.
  **Deliverable: a unified headless driver (probe eyes + harness hands + stroke
  helper), proven early in the prototype.**
- **Multi-player topology:** no process-global state blocks N Apps in one
  process (fonts per-App; the NonSend a11y pin is omitted by headless presets) —
  but nothing has ever run multiple concurrent Apps (prototype risk to retire
  early). Options: (1) N Apps round-robin `.update()` on one thread;
  (2) real client-server (fidelity target; networking is app-land, Buiy has
  none); (3) single shared-world sim, each agent driving its own player's
  widgets (simplest; matches the design's solo seat-hop). Drawer =
  picking-harness agent; guessers = probe agents.
- **Game seam:** `buiy::prelude` reaches Time/Timer/TimerMode/FixedUpdate/
  run_if + full MVU (enqueue/fold_one_inline/MvuSet) + picking events.
  **`bevy_state` is not compiled into the workspace at all** → the phase machine
  (pick→draw→reveal) goes in **MVU** (Menu-machine precedent) or plain
  resources — a deliberate seam decision to journal. Funnel rules: systems
  enqueue, reducers are the only Model writers, `set_if_neq` absorbs no-ops,
  AT-seam uses `InlineActionRegistry`+`fold_one_inline` (rules in
  `docs/specs/2026-06-29-mvu-as-core-design.md` + agent-interface design docs).
  Import non-preluded bevy names **by name, never by glob** (Text/Node
  collision).
- **Web/touch: two "deferred" items actually LANDED.** Touch activation
  (first-touch-activates via Buiy's own press/release-target path,
  `picking/mod.rs:80-84`, `activation.rs:64-70`) and web soft-keyboard/IME v1
  (`WebImePlugin`, hidden-input pattern, `text/edit/web_ime.rs`) are on main.
  Real remaining web-input gaps: IME caret position tracking + a touch-only
  policy (currently hijacks desktop-web too) — `#[cfg(wasm32)]` DOM code no
  headless gate covers → **verify on a real mobile browser at prototype
  milestones**.

### View-surface limits to design around (from the surface map)

- 7 element kinds only (column/row/text/button/checkbox/text_input/empty) — no
  image/canvas kind, **no sizing/justify/grow modifiers** (the 3-pane in-game
  layout is not expressible in view today), `Color` facade is 5 closed variants.
- **One `ui()` per app** (fixed LogicalId 0, one root, spawns its own Camera2d;
  no guard against a second call). Dooduel = ONE model + one reducer; screens
  via `match` on a `screen` enum (root kind-swap despawns/respawns), sub-views
  via `Element::map` (fn-pointer lift, e.g. `Msg::Lobby`).
- Open question for an early prototype probe: can a view subtree mount under a
  hand-authored ECS parent, or does the reconciler own the single root?
  (Determines whether the game shell is view-with-new-modifiers or raw-ECS
  shell + embedded view regions.)

## The re-baselined build list (what Phase A must actually build)

Framework-shaped (journal as missing-feature; candidates for general-purpose PRs
at the FINAL):

1. **Canvas subsystem** (Tier 0): textured-node instance kind + CPU-authoritative
   paint buffer + dirty-region upload; brush/eraser strokes; flood fill; serves
   game canvas + avatar editor. Strategy decided by prototype (accumulation-RT
   leaning).
2. **Recurring tick / Subscription** (G3, roadmap §8): the one Tier-1 gap;
   wasm-safe.
3. **Unified headless playtest driver**: probe + pointer-harness + stroke
   helper; multi-App or shared-world topology proven.
4. **View-surface growth as consumed**: sizing/justify modifiers (or a proven
   embed-under-ECS-shell pattern), image/canvas element kind, richer Color
   facade.
5. **Theme toggle + persistence** (G7 residue; persistence also for avatar).
6. **Dialog machine (G5), Toast (G6), timer-ring arc or per-second workaround
   (G4), confetti burst helper** — small, templated.

App-land (Bevy game logic, not framework): phase machine in MVU, scoring,
word lists, mock-player seat-hop, chat, hint reveals, networking optional
(playtest can be shared-world).

Verified-working already (do not rebuild): keyed lists, press/input routing,
Cmd::task, widgets + builders, typed tokens + complete light palette +
SetAccent, per-corner radii, hard shadows, font stacks (as .ttf), touch
activation, web IME v1, probe eyes.

## Corrections to stale records

- Audit §Tier-1 G1/G2: superseded — DELIVERED by buiy_view (see above).
- Audit "light theme = 9-token stub": stale since Track B #113.
- `buiy-wasm-support` memory "deferred: touch activation + web
  soft-keyboard/IME": both landed (D8); corrected 2026-07-02.
