# Scribbl app — capability & gap audit

**Status:** `[active]` · **Date:** 2026-07-01 · **Base audited:** `origin/main @ 6c4ff22`

One-line verdict: **Scribbl is buildable on today's Buiy. It needs exactly one net-new
framework *subsystem* — a drawing canvas — plus a cluster of MVU app-ergonomics features and a
few smaller primitives. Everything else is app-level work atop proven patterns.** This report
is the research-stage artifact for the Scribbl campaign; the framework features it names are
meant to be built *alongside* the app (Scribbl is the forcing function / dogfood).

---

## 1. What Scribbl is

Scribbl is a free/open-source **skribbl.io clone** — real-time draw-and-guess pictionary — to
be shipped as a **standalone Buiy app** (desktop + browser). The target is the interactive
prototype in the Claude Design project *"Scribbl.io clone design"*
(`Scribbl - Game Spec.dc.html` + `Scribbl Prototype.dc.html`). The prototype is a single-file
React/HTML mock; this campaign reimplements it in Buiy and develops the framework capabilities
it exposes.

**MVP scope (from the design's §12 + the prototype):** a *solo demo* — no real networking. One
human **seat-hops** between mocked players via a dev switcher; canvas/chat/scores/timers are
global state that keeps running regardless of which seat is "in the chair."

Screens & flow: **Home** (name + avatar) → **Join-code** / **Create-room** → **Lobby** →
**In-game loop** → **Final podium**. Overlays: word-pick Dialog, turn-end reveal Dialog,
"waiting for drawer" canvas overlay, transient toasts, light/dark theme toggle.

In-game surface: a fixed dark **top bar** with the seat-switcher; a header with `Round X/Y`, a
role badge, **letter-slot word hints**, and an **SVG timer ring**; the **freehand drawing
canvas** (brush/eraser, 4 sizes, ~16-color palette, undo, clear); a live **scoreboard**; a
scrollable **chat** that auto-scrolls to bottom. Desktop is a 3-column grid
(scoreboard | canvas | chat); a mobile variant stacks.

Game logic (an MVU-shaped **timed state machine**): phases `pick(10s) → draw(80s) → reveal(6s)`;
the drawer rotates over players each round; a turn ends early once everyone has guessed; hints
flip open at time thresholds; auto-pick / auto-advance on timeout. Guess matching normalizes
(lowercase + strip non-alnum), exact-matches to score+lock, and nudges "so close!" on
Levenshtein ≤ 2. Scoring is time- and order-scaled. Light/dark + accent theming with
persistence.

## 2. Method

Two multi-agent audits (66 + 37 agents), each mapping every Scribbl requirement onto real code
and then running an **adversarial refute pass** on every claimed gap (an agent tries to prove
the capability already exists before it is recorded as missing). Several first-guess gaps were
refuted this way (see §4).

> **Process note (and a caught mistake).** The first audit ran against a **stale local base**
> (`4010753`) that was 10 commits behind `origin/main` and *predated the entire MVU-as-core
> merge*. Two agents correctly reported "MVU state layer not present" — an artifact of the
> stale base, not the truth. The render / layout / input / text / vector / animation findings
> are base-insensitive and stand; the **MVU / theming / overlays / scaffolding** dimensions
> were re-run against `origin/main @ 6c4ff22` (MVU present) on a fresh worktree. This is the
> "branch from latest upstream" guideline earning its keep.

## 3. Verdict: the capability matrix

| Subsystem | For Scribbl | Notes |
|---|---|---|
| **Drawing canvas** | ✗ **blocker** | No texture-presenting node, no paintable surface, no dynamic-vector channel. **The one net-new subsystem.** |
| MVU state core (Model/Msg/reducer, routing, phase machines) | ✓ ready | Strong. `app.mvu_model(update)`; screen-routing, phase-machine, per-seat re-derivation all have prior art in the migrated demos. **Zero substrate bugs** found. |
| MVU app *ergonomics* (view/list-reconcile, press→Msg, timer) | ~ pervasive DX gap | Everything is *expressible*, but as hand-written glue (~60 lines per dynamic list; per-button routing; bespoke `Res<Time>`→`Tick` driver). |
| Layout (3-col grid, fixed bar, scroll, responsive) | ✓ ready | Grid + `Position::Fixed` + `Overflow`/`ScrollArea` + container queries all landed; gallery already dogfoods the bounded-scroll idiom. |
| Text (letter-slots, mono, letter-spacing, big display) | ✓ ready | Monospace + `LetterSpacing` + per-entity color/size + centered + editable input all shipped and exercised by the parity gallery. |
| Pointer input (drawing loop, clicks, keyboard) | ✓ ready | Full `Pointer<E>` taxonomy; Press→Drag→Release + canvas-local coords via `GlobalTransform`; in-window drag "capture" for free. |
| Animation (spinner, fades, progress) | ~ mostly ready | Real `Tween<T>` engine (Opacity/Translate/Rotate/Scale/BackgroundColor, easing, repeat) runs every frame. Spinner = looped `Rotate`; fades = Opacity tween. |
| Vector shapes (avatars, arcs) | ~ partial | `Icon` renders arbitrary **static** SVG paths (lyon→coverage atlas): circles, rings, avatar faces all render. No **dynamic/per-frame** vector; coverage tier can't rotate. |
| Theming (dark + accent) | ~ partial | Dark theme full; runtime accent swap + ramp math shipped and live-re-resolving. **Light theme is a 9-token stub**; no light/dark toggle; no persistence. |
| Overlays (Dialog, top-layer, toast) | ~ partial | `Dialog` widget fully built (focus-trap, Escape, inert bg); top-layer/stacking complete. **No MVU-native controlled dialog; no framework toast/stack.** |
| Widgets (button variants, avatar, badge, segmented…) | ~ compose | Catalog exists; several Scribbl widgets missing but composable from primitives in a view. |
| Browser / wasm | ✓ ready | Dual **WebGL2 + WebGPU** (WebGL2 reach landed #94), embedded fonts, enforced web-smoke CI. Time works (single-threaded). Pre-empt `getrandom` wasm config. |

## 4. Gaps refuted by the adversarial pass (do **not** build these)

- **"No stroke/polyline/path primitive."** *Refuted.* `Icon` takes a raw SVG `d` string,
  tessellates via lyon (stroke or fill), rasterizes to the coverage atlas — arbitrary **static**
  paths render today (`render/icon_raster.rs`). The real limit is dynamic/per-frame geometry.
- **"No drawing input / no canvas-local coordinates."** *Refuted.* Press/Drag/Release stream +
  `event.pointer_location.position − canvas.GlobalTransform.translation().xy()` is the exact
  pattern the text editor already uses; bevy_picking routes Drag to the press target (in-window
  capture).
- **"No continuous-rotation / spinner driver."** *Refuted.* `Rotate` transform + `Repeat::Loop`
  in the Tween engine, default Continuous winit mode.
- **"Web app scaffolding must be hand-assembled."** *Refuted.* `examples/buiy_web` +
  `gallery_web` are reusable templates; dual-backend + `navigator.gpu` loader + CI smoke exist.
- **"MVU state layer not present."** *Stale-base artifact.* MVU is present and app-drivable
  (`crates/buiy_core/src/mvu/`, preluded via `crates/buiy/src/lib.rs`).

## 5. The gaps, tiered

Effort key: S ≈ hours, M ≈ 1–3 days, L ≈ 3–7 days, XL ≈ a wave. "Net-new" = framework
capability; "app-level" = build atop existing APIs.

### Tier 0 — Hard blocker (net-new subsystem; Scribbl cannot exist without it)

**G0. Drawing canvas.** The core mechanic. Buiy is retained-mode over a *closed* GPU primitive
set (quad/shadow/glyph + coverage-atlas icons); there is **no node that presents pixels an app
draws** and no efficient way to accumulate strokes. Confirmed non-viable alternatives: "one
quad node per stroke segment" trips a Full re-extract on every structural add (512 nodes already
= 45 ms/frame → O(N²) over a drawing); "one Icon atlas cell per stroke" churns the atlas. This
decomposes into:

- **G0a — Texture-presenting node (`ImageNode`)** · net-new · **L** · the *keystone*: a node
  that samples & presents an app-owned RGBA GPU texture (DPR-aware) into its layout box. Mirrors
  the existing coverage-atlas sampling path but full-color. **Also unlocks images generally**
  (media is "zero code" today).
- **G0b — The paint mechanism** · net-new · **L–XL** · how strokes reach that texture. Two
  strategies to decide *by building* (→ prototype-first, see §7 / Appendix A): an **accumulation
  render target** (persistent per-canvas RT, stamp only the new segment; natural fit for undo
  snapshots + the prototype's ImageData model) **or** a **dynamic stroke/vector channel** (build
  the reserved `Path` primitive / a per-frame stroke instance buffer).

### Tier 1 — MVU app ergonomics (net-new DX; not blockers, but pervasive hand-glue if skipped)

These are the "develop alongside" features; Scribbl is the dogfood. All confirmed missing.

- **G1. The "V" — declarative view / keyed-list reconcile helper** · net-new · **XL** · the
  single biggest app-author cost. Each dynamic list (scoreboard, auto-to-bottom chat, players,
  letter-slots) is ~60 hand-written reconcile lines today (per `examples/todomvc`). Also: MVU's
  record/replay headline silently won't reconstruct dynamically-spawned rows until this is
  principled.
- **G2. Press→Msg routing helper** · net-new · **M** · every click target is a hand-rolled
  `MessageReader<OnPress>` → marker-match → `enqueue`. Scribbl has 10+ distinct targets (nav,
  seat chips, word choices, theme toggle, send, create/join, play-again).
- **G3. First-class timer / Subscription** · net-new · **L** · the game clock is *expressible*
  (a `Tick(now)`-through-the-funnel reducer is proven in `buiy_bench_support` and blessed by the
  §11 gate) but every timer is bespoke `Res<Time>`→`enqueue(Tick)` arithmetic with **no way to
  push-schedule "in N s, send Msg"**. This is the deferred roadmap §8 (`Cmd::task` + keyed
  `Subscription`). Scribbl (a timed state machine) is the natural forcing function to land it.
  *Framework strength to lean on:* `set_if_neq` absorbs steady sub-threshold Ticks
  (`node_rebuilds == 0` on quiet frames), so a per-frame clock is cheap.
- Supporting DX (each S–M): editor↔MVU write-back for clear-after-submit; a `using-mvu`
  app-author guide/skill; a headless MVU logic-test builder (partially present).

### Tier 2 — Smaller net-new primitives

- **G4. Timer-ring arc / parametric sweep** · net-new · **L** (or **S** workaround) · a
  circular-progress paint. Workaround: regenerate an arc-`Icon` per second (1 atlas cell/s,
  bounded churn) — decide vs a real primitive during the app build.
- **G5. MVU-native controlled Dialog** · net-new · **M** · bind `Model → open` with
  `dismissal → Msg`, resolving the second-writer race against `WidgetsPlugin`'s always-on
  `CssVisibility` writers (the deferred "Dialog machine"). The `Dialog` widget itself is done.
- **G6. Toast + toast stack** · net-new · **M** · no framework toast; only an ad-hoc single
  toast in the gallery.
- **G7. Theming for a themed app** · **M** each · complete `default_light_theme` (currently a
  9-token stub → magenta sentinels for a light app); a `SetThemeMode` light/dark toggle
  primitive; a persistence layer (localStorage on web / file on native, also covers accent).
  Dark theme + accent swap already work.

### Tier 3 — App-level work (no framework gap; just build the app)

Screen routing (NavModel enum + `Display::None` toggle); the Pick/Draw/Reveal phase machine
(MenuModel machine-tier pattern); per-seat "viewing as" re-derivation; scoring + guess-matching
(pure reducer, incl. Levenshtein); the 3-col grid / fixed bar / bounded scroll (scroll-to-bottom
= set `ScrollOffset` to max); monospace letter-slots + mono code input (uppercase app-side);
composed widgets (Button variants **M**, Avatar **M**, Segmented/Badge/Card/Spinner/ThemeToggle
**S** each); fades/spinner via the Tween engine; `getrandom` wasm config (**S**, pre-empt the
build break); the web build from the `buiy_web` template.

## 6. Proposed campaign shape

> **Superseded (2026-07-01, same day) by the campaign charter**
> ([`../prototypes/2026-07-01-scribbl-campaign-charter.md`](../prototypes/2026-07-01-scribbl-campaign-charter.md)):
> the user re-framed the method as **whole-app prototype-first** — the *entire* app build
> (not just Wave A's canvas) is Phase A of `prototype-first-development`, journaled, gated
> by a retrospective at feature parity, then a ground-up final. The wave decomposition
> below survives as the likely internal build order of Phase A.

Mirrors the user's framing ("develop the features alongside… get it all done"). Staged
(research → spec → plan → execute), gated at every wave; **framework features land as their own
reviewed, general-purpose PRs with Scribbl as the dogfood consumer** — they are not
Scribbl-specific.

1. **Wave A — Drawing canvas (Tier 0).** Highest uncertainty → **prototype-first**: build and
   *run* both paint strategies (Appendix A), then re-decide and ship `ImageNode` + the canvas.
   Make-or-break; do it first.
2. **Wave B — MVU app ergonomics (Tier 1).** The view/list-reconcile helper, press→Msg routing,
   and the timer/Subscription (land roadmap §8). Unblocks a tractable app; broadly reusable.
3. **Wave C — Supporting primitives (Tier 2).** Timer-ring, MVU-native Dialog, Toast stack,
   light theme + toggle + persistence.
4. **App build (Tier 3).** Assemble Scribbl screen-by-screen, dogfooding A/B/C, following the
   migrated-demo patterns; verify each screen in the running app (desktop + browser).

Sequencing note: Wave B is a *dependency* of a comfortable app build, but Wave A is the true
risk. A/B can proceed in parallel once A's prototype resolves the paint strategy.

## 7. Open decisions for the human (before the first spec)

1. **Drawing-canvas paint strategy** — accumulation render-target vs dynamic-vector channel vs
   a general custom-shader escape hatch. *Recommendation: resolve by prototype (Wave A), not on
   paper.* (Appendix A.)
2. **MVU-ergonomics scope** — build the full view/routing/timer trio as framework features now
   (recommended, per the user's "alongside" intent), or ship a minimal Scribbl-local helper and
   defer the general version.
3. **Timer-ring** — build the arc primitive (reusable) vs the per-second arc-`Icon` workaround
   (cheap, app-local).
4. **Platform target for v1** — desktop + desktop-browser (recommended; mobile-web touch/soft-
   keyboard are deferred framework waves W2/W3) vs include mobile-web.

## Appendix A — Drawing-canvas strategy options

| Option | Idea | Pros | Cons |
|---|---|---|---|
| **A. Accumulation render target** | Per-canvas persistent RT; `ImageNode` presents it; stamp only the new segment each sample | Matches the prototype (ImageData + undo snapshots); O(1) per sample; smooth | New RT lifecycle + a stamping pass; undo = snapshot ring |
| **B. Dynamic vector/stroke channel** | Build the reserved `Path` primitive or a per-frame stroke instance buffer; strokes are live geometry | No RT; resolution-independent; fits the primitive model | Per-frame tessellation cost; undo = geometry log; new pipeline |
| **C. Custom-shader/material escape hatch** | General "app owns this node's draw" hook | Most general; unlocks future custom viz | Largest surface; easy to under-spec; more than Scribbl needs |

`ImageNode` (G0a) is the shared keystone under A and C and useful regardless. The prototype's
own model (canvas + ImageData undo) argues for **A**; **B** is the more "Buiy-native" bet.
Decide by building both minimally and measuring (perf floor: 60 Hz; weak-machine target).

## Appendix B — Evidence

Full per-dimension findings, per-requirement have/partial/missing tables, and every adversarial
verdict are preserved in
[`2026-07-01-scribbl-app-capability-gap-audit-assets/`](2026-07-01-scribbl-app-capability-gap-audit-assets/)
(raw structured outputs of both workflows, 66 + 37 agents; see its README for provenance and the
stale-base caveat). Key code anchors cited by the agents: `crates/buiy_core/src/render/`
(`buckets.rs`, `primitive.rs`, `icon_raster.rs`, `components.rs`), `crates/buiy_core/src/mvu/`
(`mod.rs`, `leaf.rs`), `crates/buiy_core/src/animation/`, `crates/buiy_widgets/`
(`dialog.rs`, `scroll_area.rs`, `menu.rs`), `crates/buiy/src/lib.rs` (prelude),
`examples/todomvc`, `examples/buiy_gallery`, `examples/buiy_web` / `gallery_web`, and the
migration journal `docs/reports/2026-06-30-demos-mvu-migration-journal.md`.
