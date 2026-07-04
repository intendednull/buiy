# Dooduel — Prototype 1 retrospective: the Phase A learning gate

> **The prototype's PRIMARY deliverable.** Synthesizes the
> [journal](2026-07-02-dooduel-PROTO1-journal.md) (W0 skeleton → W8-live playtest)
> and the [playtest evidence](2026-07-02-dooduel-PROTO1-playtest/) into
> keep / refine / redesign for the FINAL. The prototype code (branch
> `worktree-dooduel-proto1`, off `origin/main @ a969cbf`) is an **unmerged
> reference — DO NOT MERGE.** The FINAL re-decides every choice with this in hand;
> framework features ship as their own reviewed, **general-purpose** PRs dogfooded
> by Dooduel, not as Dooduel-local code.

---

## 1. Verdict

**Exact design parity + the multi-agent playtest bar are BOTH achievable, and
BOTH were achieved this prototype — to strong parity with a bounded, entirely
character-layer residual-gap list, and to a full 4-agent playtest that completed
on the first live run.** The path the charter bet on is proven end-to-end: a
whole skribbl.io-clone game — native desktop + web-desktop + web-mobile, drawing
canvas, real-time game loop, theming, persistence — builds on Buiy today, and the
framework gaps it forces are all additive surface, not architectural walls.

**Parity fidelity, honestly.** Structure, layout, spacing, the protokit color
ladder, the Caveat/Shantell type pairing, circular doodle avatars, rounded
pills/cards, the sketchy ink outlines + wobble, the 3D-press resting state, the
dark theme, the timer ring, the painted canvas, and the confetti burst all match
the design (RUN-verified by GPU capture, eyeballed against the design HTML, every
wave). The residual deltas are all **character-polish, not capability**: color
emoji render as tofu (dropped/substituted), the 3D-press ships resting-only (no
press-down animation), dashed borders render solid, the timer ring steps
once/second instead of sweeping smoothly, per-axis elliptical wobble corners
render circular, radial-gradient backgrounds are flattened, hard drop-shadows are
softened, and HiDPI phones mis-scale. None is a wall; each is a scoped follow-on
enumerated in §6.

**Wave scoreboard** (each RUN + verified — GPU capture and/or probe test — before
the wave was journalled):

| Wave | Outcome | One-liner |
|---|---|---|
| **W0 skeleton** | ✅ | `examples/dooduel` boots off AGENTS.md-only, **compiled + probe-tested first try**; surfaced the root-can't-fill/center gap. |
| **W1 canvas** | ✅ keystone | `RasterImage` primitive + CPU paint buffer; **byte-exact GPU readback** of brush/fill/eraser. Decided CPU-buffer over accumulation-RT. |
| **W2 game core** | ✅ | Pure `Game` machine + Tick-fold clock + seeded bots; virtual-clock tests. **Re-characterized G3** (see surprises). |
| **W3a view layout** | ✅ | Sizing/justify/grow surface + `raster()` element + paint-order fix; **centered Home** proven. |
| **W3 screens** | ✅ strong | Home/Join/Lobby to protokit parity; **borderless-rounded-fill** render fix; found the light-palette stub + emoji tofu + radius lens. |
| **W4 in-game** | ✅ strongest | 3-pane screen to parity; pressable icon, timer ring, model-owned toolbar; found no-flex-wrap, no-scroll-container, bordered "ears". |
| **W5 podium/avatar** | ✅ **zero fw edits** | Podium + confetti + 2nd canvas — the W3a/W4 surface sufficed; found per-quad-alpha, raster-in-overlay-hidden, raster-no-rounded-clip. |
| **W6 sketchy/theming** | ✅ | Bordered-rounded fill + per-corner band + lens render fixes; light/dark toggle + persistence; **dashed=solid confirmed**, press-state hook absent. |
| **W7 web/mobile** | ✅ **touch draws** | Dual-backend wasm; **touch drawing works with zero fw work**; found the click-swallowing occluder + the HiDPI web scale bug. |
| **W8 infra** | ✅ | Stroke/drag helper (**first-try**, recipe exactly as re-baseline predicted) + unified headless driver + file-protocol playtest host. |
| **W8-live** | ✅ **bar MET** | 4 independent agents, one per seat, played a full match; 8 real gameplay bugs found that no prior gate could see. |

**Re-baseline build-list: prediction vs. outcome.** The audit's build list
(`../reports/2026-07-02-dooduel-rebaseline-audit.md` §"The
re-baselined build list") was accurate about *what was buildable*; the value came
from where the outcome **diverged** from it:

| # | Prediction | Outcome | Score |
|---|---|---|---|
| 1 | Canvas: textured-node + CPU buffer + **dirty-region upload**; strategy **accumulation-RT-leaning** | Built as CPU buffer sampled by a distinct `RasterImage` pipeline. **Accumulation-RT rejected** with evidence; **dirty-region deferred** (full re-upload, fine at 720×450). Both surfaces built. | HIT, **strategy reversed** |
| 2 | Recurring tick / Subscription — "**the framework feature Dooduel most clearly forces**" | **NOT a capability gap.** Hand-rolled in 6 lines; the prototype proved the *wished-for* edge-triggered `Cmd::interval` is the WRONG shape. Ask re-cast as a boilerplate-removal ergonomic. | **RE-CHARACTERIZED** (biggest divergence) |
| 3 | Unified headless driver; multi-App **or** shared-world topology proven | Built (stroke/drag + probe + one `init_asset` gotcha). Topology = **single-model file-protocol host**, not multi-App. Multi-App concurrency **risk unretired**. | HIT, topology narrowed |
| 4 | View-surface growth "**as consumed**": sizing/justify, image/canvas kind, richer Color | Built, **and grown far past the prediction** (11+ modifiers) — and grown *ad-hoc, wave by wave* (a DX friction, §5). Richer Color = a `Custom` escape, not a grown facade. | HIT, larger + ad-hoc |
| 5 | Theme toggle + persistence (light palette "complete" per Track B) | Built (Theme-resource swap; native JSON + wasm localStorage; avatar PNG). **Light palette found to be a family-derived STUB**, not the protokit ladder — Dooduel was its first visual consumer. | HIT, **audit correction** |
| 6 | Dialog (G5), Toast (G6), timer-ring (G4), confetti — "small, templated" | Timer-ring (per-second arc-icon) + confetti built + verdict-positive. **G5 Dialog + G6 Toast never needed** (overlays = `.top_layer()` scrims; chat replaces toasts) → no data. | HIT for what was used |

**Net:** the audit correctly scoped the *shape* of the work. Six things it did
NOT predict came only from building + RUNNING: the canvas strategy reversal, G3
being ergonomic-not-capability, the light-palette stub, a whole long tail of
view-surface gaps (flex-wrap/scroll/text-align/inset), the click-swallowing
occluder, and the bordered-rounded-fill class of render bugs the framework had
never had a fixture for. Those six are the prototype's real yield.

**Recommendation for the FINAL:** ship the canvas + the view surface + the render
fixes as **general-purpose framework PRs** (all cleanly separable — §7), but
**re-decide three things the prototype learned to do differently**: (a) the
paint-order **per-raster-anchor interleave** instead of the top-layer-suffix split;
(b) the G3 clock as a **poll-clock-as-Msg `ClockPlugin<M>`**, explicitly NOT an
edge-triggered timer; (c) a **designed, coherent view sizing/layout surface** in
one pass instead of the wave-by-wave accretion. And **bake the recurring-bug
guards in**: a native-pointer interaction test tier (the occluder), a
bordered-rounded golden fixture (the ears/lens), and a side-surface version
counter (the persist race).

---

## 2. Validated — KEEP (port to the FINAL; re-derive the rationale)

Decision-by-decision. "KEEP" = the pattern is right, port it. "KEEP-pattern,
REDESIGN-mechanism" = the shape is right but the FINAL should ship a better
implementation (called out in §3).

| Decision | Verdict | Evidence (RUN-verified) | Why |
|---|---|---|---|
| **`RasterImage` = a distinct pipeline keyed by record, NOT a new `BuiyPrimitiveKind`** (W1) | KEEP | Byte-exact GPU readback (red `[220,40,40]`, filled green, erased→paper); every existing golden byte-identical | Mirrors the band/gradient precedent exactly; the closed `BuiyPrimitiveKind` enum + the byte-stable quad path stay untouched; the reserved `Path` slot is left for real vector art |
| **CPU-authoritative paint buffer** (vs GPU-RT, vs `Path`) (W1) | KEEP | Flood fill = ~40-line bounded scanline; eraser = bg stamp; brush = circle-stamp + interpolation — all pure, unit-tested | Flood fill *needs to read pixels* (a GPU-RT makes it a readback); undo/serialize/persist all want the buffer on the CPU anyway. The RT only saves the re-upload — a perf refinement, not a capability. `Path` is semantically wrong for freehand+fill+erase raster. Re-argue and keep. |
| **Keyed `PaintCanvases` resource map** (`HashMap<CanvasKind, PaintSurface>`) (W5) | KEEP | Game canvas byte-identical across the generalization; avatar (220×220) composed by construction | The reconciler despawns a `raster` node on screen-swap and respawns on re-entry; a **component** on the node would lose the pixels, a **resource** survives. The map advertises N-canvas scaling. W1's "two canvases compose" prediction confirmed. |
| **View sizing / justify / shrink / align surface** (`.width/.height/.fill/.grow/.shrink/.justify_*/.align_*`) (W3a/W4) | KEEP-surface, **REDESIGN-shape** | Centered Home (title center_x 512≈480); 3-pane in-game to parity; every layout snapshot byte-identical | The *altitude* is right (expose intents — fill/grow/center — the reconciler lowers `Sizing`/`FlexParams`). But it **accreted ad-hoc across 4 waves** and still misses flex-wrap/scroll/text-align/inset/per-side-padding. FINAL: design the whole surface once (§3, §5). |
| **`raster()` element + identity patching** (W3a) | KEEP | Reconciler test: canvas entity + handle survive an unrelated model patch; no re-upload on unrelated fold | The element beat the mount-under-ECS-shell hatch decisively: one `Kind` + one reconciler arm; the canvas gets keyed/when/lifecycle for free; the handle rides the replayable model. Handle patched **by identity** (`RasterImage` isn't `PartialEq`) so a fold never re-uploads the texture. |
| **Paint-order top-layer split** (in-flow quads → raster → top-layer quads → glyphs) (W3a) | **KEEP-pattern, REDESIGN-mechanism** | Overlay-over-canvas byte-verified; 6 interleave unit tests + 5 GPU goldens; byte-identical for every non-raster view | The *insight* (splice a `Rasters` step into the existing gradient interleave at a boundary) is right and made the fix byte-identical. But the boundary assumes top-layer forms a **contiguous quad suffix** — exact only for one `ui()` root, and a non-top-layer overlay still draws UNDER the canvas. FINAL: ship the **general per-raster-anchor interleave** (§3.render). |
| **Pressable `Icon`** (role-keyed press route) (W4) | KEEP | Clicking the seat-1 avatar chip hops the seat, end-to-end through pointer + probe + a11y | The a11y **Button contract is role-keyed** — `contract_for(Button).honor(Click)` emits `OnPress` with no `Button` component. Both a real pointer click and a probe `Action::Click` hit the same `PressAction` → funnel. Clean; extend it (§3) to containers/rasters. |
| **`.ignore_picking()`** (`Pickable::IGNORE`) (W7) | KEEP (+ consider auto) | Restored app-wide interactivity; every `buiy_verify` snapshot byte-identical (opt-in no-op) | The picking backend already had pass-through machinery + a comment describing this exact hazard; the view just couldn't express it. The `pointer-events:none`-on-container / `auto`-on-children pattern. FINAL: consider auto-`IGNORE` for transparent (`Color::NONE`) top-layer containers so authors can't forget (§4). |
| **Bordered-rounded fill ("ears") fix + per-corner band + lens fix** (W6) | KEEP | buiy_core 39 + buiy_verify 22 GPU goldens byte-identical; the live pill/panel now outlines cleanly | Both were "the data is already there, the consumer collapsed it": fill radius = band `inner_radius` (min); band selects per-corner radius by quadrant with `min(rx,ry)`. Zero new fields, zero stride change. **The framework never had a bordered-rounded fixture** → add one (§4). |
| **`.shadow(dx,dy,blur,spread,color)`** (W6) | KEEP-pattern, REFINE | Ambient card `--sh-*` + the 3D-press underside render; within the WebGL2 16-attr fold | Chains front-to-back (CSS order), lowers to `BoxShadow`. But a Buiy shadow is a blur-rounded **rect** — no corner radius — so a hard offset shadow behind a pill shows square nubs (softened with a 2px blur). FINAL: give the shadow a corner radius (§3.render). |
| **Stroke/drag helper** (`PointerHarness::stroke`/`drag` + `drive_stroke`) (W8) | KEEP | `pointer_drag_c7` 3/3: a 4-step drag emits 1 `DragStart` + 4 `Pointer<Drag>` + 1 `DragEnd`, all on the press target; canvas e2e lands ink headless | The re-baseline's `bevy_picking` reading was exact: Press@p0 → N×`Move{delta}` → Release; `move_to` (no `Move`) provably never drags. Pure additive `buiy_verify` surface, no framework bug. Prime FINAL candidate. |
| **Unified headless driver recipe** (probe preset + picking, one App) (W8) | KEEP (+ bake `init_asset`) | Canvas e2e + rehearsal green; recipe minimal (scroll/animation NOT needed) | `BuiyProbePlugin` composes with the picking stack because the preset simply omits picking. **The one non-obvious piece: `app.init_asset::<Image>()`** — the probe preset never adds `ImagePlugin`/`RenderPlugin`, so a headless canvas host must register the asset (opaque "Resource does not exist" panic under `debug=0`). Bake it in. |
| **Tick-fold game clock** (`Res<Time>` → `enqueue(Msg::Tick(now))`, reducer derives all) (W2) | **KEEP-pattern, REDESIGN as `ClockPlugin<M>`** | Live match: score 500 to the guesser, chat line, redacted word all correct; steady frame folds byte-identical (`set_if_neq`), no rebuild | The game logic is a *pure function of the Tick stream* — countdowns, hint reveals, phase timeouts, bot fires all derive from `now − anchor`. Only DERIVED values are stored (never raw `now`) → perf-free by construction. "Re-anchor on the next tick" makes `Game::tick` the sole clock owner (plain `Msg` and timeout share one path). FINAL: lift the 6-line driver into a reusable `ClockPlugin<M>` (§3.mvu). |
| **Model-owned tool state, projected model→canvas each frame** (W4) | KEEP | Tool changes replay; `sync_tools_to_canvas` one-way projection; imperative ops (clear/undo/auto-clear) ride monotonic `*_seq` counters drained once | The clean boundary between MVU-governed UI state and a side render surface the framework can't fold. Same shape reused for the canvas handle, theme swap, confetti, persistence. Zero friction. |
| **Honest per-seat playtest views** (clone `Game`, set `viewing_as=i`, reuse `word_display()`) (W8) | KEEP | Live: drawer sees `S C A R E C R O W`, seat 1 sees `_ _ _ _ _ _ _ _ _` | The redaction logic has exactly ONE home (the same accessor the UI uses) so it cannot drift between UI and host — the only real correctness risk (leaking the word to a guesser) is closed by construction, not by a hand-rolled per-seat redaction. |
| **Storage blob shape** (serde `PersistedState`; avatar PNG-encoded + base64; two-key web split) (W6/W7) | KEEP | Native round-trip: theme=dark, name=Mara, custom=true, avatar face px=1874 survived; web round-trip verified | Pure-Rust `image`+`base64`+`serde_json` compiled + ran unchanged on wasm. The design's `Default = key-absence`/`removeItem` semantics matched exactly. Typed `load_persisted`/`save_persisted` seam split cleanly per target. |

Also KEEP (no framework surface, but load-bearing patterns): the **pure `Game`
core** with zero framework coupling (every rule a `&mut self` method, unit-testable
with no ECS/GPU/clock); **bots-as-emits** (a bot guess is a real `Msg` run to
completion in the same drain a human hits — idempotency free via `turn_guesses`);
the **rising-edge side-effect observer** shape (confetti, per-turn canvas clear,
theme sync, persist — all READ `Dooduel.screen`/model via a `Local` latch, never
touch the funnel).

---

## 3. REFINE / REDESIGN — what the FINAL does differently

Every "FINAL re-decision" the journal accumulated, organized by subsystem.

### Render (`buiy_core`)

1. **Per-raster-anchor paint-order interleave** (retires the top-layer-suffix
   split, §2). Interleave each raster into the flat quad draw by its OWN
   `node_quad_anchor` — the *exact* gradient-bleed precedent — so a raster paints
   at its true stacking position with no top-layer special case and no
   contiguous-suffix assumption. This is the general fix W3a/W5 both pointed at;
   it retires TWO limits at once: (a) a non-top-layer overlay drawing under a
   canvas, and (b) **a raster inside a top-layer modal being hidden** (W5's avatar
   editor had to become a full in-flow screen because a modal panel painted over
   the raster it contained). Until it lands, the rule is "a modal may contain text
   + quads, but a modal containing a raster must be a full screen."
2. **Per-corner FILL radii** (retires the wobble-fill residual). The fill still
   carries ONE uniform radius (the `PackedInstance.radius` slot is shared with the
   shadow-sigma + text paths — widening it is a **stride change touching the shadow
   pipeline**, too deep for a prototype wave). Under a *wobble* border the fill
   rounds to the min inner corner; at a large wobble corner a faint fill ear can
   remain (invisible at Dooduel's ±3px amplitude + low contrast). FINAL: widen the
   instance or add a dedicated rounded-fill instance.
3. **Per-axis elliptical corners.** The band is per-corner but **circular**
   (`min(rx,ry)`), so the design's `26px/30px` renders circular `26`. Needs an
   elliptical-corner SDF if per-axis wobble is ever wanted (invisible at this scale).
4. **Dashed/dotted borders.** `LineStyle::Dashed` **CONFIRMED renders solid**
   (two-layer gap: `.border()` hard-codes `Solid`, and `band.wgsl` has no
   dash/dot pattern — `resolve_border` only skips `None`). Needs a screen-space
   arc-length stipple + a `LineStyle` arg on `.border()`. The design's dashed
   room-code box is approximated solid.
5. **Rounded clip for `raster`.** The raster shader clips to a rectangular AABB
   only, so custom-drawn avatars render as square stickers while stock doodles are
   circular. Needs a corner-radius clip (or an avatar circle-mask).
6. **Per-quad particle alpha** (a fade that does NOT promote to an `EffectGroup`).
   A Buiy `Opacity < 1.0` forms an EffectGroup (off-screen composite boundary), so
   a per-particle `OpacityTween` on ~110 confetti pieces would spin up ~110
   off-screen targets. Confetti dropped the fade (hard despawn). `Translate`/`Rotate`
   only form a *cheap* stacking context — safe. Any particle system on Buiy needs a
   cheap per-quad alpha.
7. **Shadow corner radius** (§2) — for the crisp zero-blur 3D-press "sticker" edge.
8. **Bordered-rounded golden fixture is MISSING** — the ears/lens fixes are proven
   ONLY by Dooduel captures because the framework has no such fixture (the app had
   unknowingly engineered *around* the bug). Add one as a regression guard (§4).

### View surface (`buiy_view`)

The surface grew ad-hoc; the FINAL designs it coherently. Consumed-and-kept:
`.width/.height/.fill/.grow/.shrink/.justify_*/.align_*/.top_layer/.fixed/
.ignore_picking/.color/.font/.border/.shadow/.radius/.radius_corners`, the
`raster()`/`icon()` elements, styleable buttons (gated on "explicitly styled" to
keep shared-widget goldens byte-identical), `Color::Custom`/`Color::rgb`. **Still
missing, each worked around app-side:**

- **flex-wrap** (`.wrap()`) — the 16-swatch toolbar overflowed into the chat pane
  (found by RUNNING; split into two explicit rows).
- **per-side padding** — the fixed-60px top bar fought uniform padding (used
  content-height + `Md`).
- **text-align** — centered copy renders left-aligned; word-slot underlines are a
  child 4px bar, not `border-bottom`; system chat lines centered by an
  `align_center` wrapper.
- **inset / absolute-with-inset** — no way to place the per-seat drawing/guessed
  corner badges or pixel-perfectly center a modal. `.fixed()` resolves against the
  **root content box, not the viewport** (a scrim lands at the padded root origin,
  not `(0,0)`).
- **scroll_column** (`Overflow::Scroll` + auto-max `ScrollOffset` on append) — no
  view-level scroll container; chat capped to the last 12 messages.
- **horizontal scroll** — the mobile scoreboard strip even-splits seats via
  `.grow()` instead of the design's overflow-x.
- **rotation** — pick tiles not tilted ±3°; the "free & open source" ribbon dropped.
- **font-weight** — variable fonts load but `.font()` has no weight arg (all text
  is the default instance).
- **richer Color facade** — 5 semantic roles vs ~a dozen protokit tokens; the app
  pins ~10 exact colors via `Custom`. FINAL: grow the facade OR keep the documented
  `Custom` escape (the two-layer split — app `Palette` + `Theme` resource — worked).
- **press-state / `:active` hook** — a view is a pure `fn(&Model)->Element`; a
  transient pressed style is runtime-owned state the view can't read, so the
  3D-press ships **resting-only**. FINAL: a controlled `:active` pseudo-state
  surfaced INTO the view (the `ControlledLeaf` shape) or an animation/transition
  layer that owns hover/press styling outside the model. (Correct for MVU — a
  pressed *style* is ephemeral non-model state.)
- **container / raster press route** — only a leaf `icon`/`button` wires the press
  route; a clickable container (children intercept the hit + carry no role) or a
  pressable `raster` (own seat chip) silently ignores `.on_press`.
- **`.top_layer()` modal ergonomics** — should `.top_layer()` imply `.fixed()` +
  auto-centering (+ the auto-`ignore_picking` for a transparent scrim, §4)?

### MVU / framework

- **G3 — `ClockPlugin<M>` / `Cmd::tick_every`, re-characterized.** Ship a
  **runtime-provided poll clock source as a Msg** — `M::tick(now)` enqueued every
  frame — **NOT** an edge-triggered `Cmd::interval`/`Cmd::timeout`. The prototype
  proved the edge-triggered timer is the *wrong* shape: a fired-once timer is hard
  to replay and hard to keep `set_if_neq`-clean, whereas the poll-from-`now` fold
  is level-triggered (replayable, idempotent, perf-free). The gap is purely
  ergonomic — every app re-hand-rolls the 6-line driver + the anchor arithmetic.
- **`on_submit_with(fn(String)->Msg)`.** `on_submit` takes a *static* `Msg`, so the
  typed guess round-trips through a model field (`on_input → SetChatInput` →
  `on_submit → SubmitGuess`). Matches the editor's command-sourced model, but the
  `_with` variant deletes the two-message dance (mirrors `on_input_with`).
- **Headless virtual-clock advance** (`Time::advance_by`) for animation captures.
  `BuiyHeadlessPlugin` omits `AnimationPlugin` (probes don't animate) and a tight
  `app.update()` loop has ~µs `delta`, so the confetti capture had to `sleep 16ms`
  between frames. A FINAL headless-animation harness wants a virtual advance, not
  real sleeps.

### Platform (wasm / mobile)

- **HiDPI web scale bug — the TOP mobile item.** At `devicePixelRatio > 1` (every
  real phone / retina) the UI renders scaled ~dpr× and overflows
  (dpr-proportional: correct at dsf=1, ~2× at dsf=2, ~3× at dsf=3). The responsive
  *logic* is correct (reads logical `window.width()`); the bug is in the
  render/layout scale-factor handling on the wasm path. A dynamic window resize
  also mis-sizes the surface (render confines to a sub-region) — likely the same
  scale-factor/surface-reconfigure seam. **Needs a focused `buiy_core` HiDPI/web
  investigation.** The mobile layout is only verified clean at dsf=1.
- **Emoji / COLR.** `🎨🎉🥇✏️✅👑…` render as `.notdef` tofu (bundled faces are
  Latin-only; the coverage rasterizer is monochrome R8 — no COLR/CBDT/sbix).
  Geometric glyphs (`▶ ‹`, punctuation < U+2300) DO render. FINAL: pick an emoji
  strategy (an outline emoji font through the existing coverage path, or a
  color-emoji pipeline).
- **Native pointer-click test tier — the W7 lesson.** The occluder bug (§4) is
  target-agnostic and was hidden only because nothing ever *pointer-clicked* the
  app (a11y-click probe + GPU capture both bypass pick occlusion). RUN a real
  pointer click on the native windowed app in an EARLY wave.

### App (playtest-found gameplay bugs) — see §4.

---

## 4. Framework / system BUGS the prototype surfaced (found by RUNNING)

The honor roll, with root causes. Every one required *running* the artifact — a
headless snapshot could not see any of them.

1. **The invisible click-swallowing occluder (THE headline; the recurring class).**
   A transparent `.fixed().fill().top_layer()` container (the floating theme
   toggle) sits topmost in the pick order and occludes everything beneath — the
   picking backend truncates at the first occluder — so on **every screen NOTHING
   was clickable.** Hidden from every prior gate because the app was only ever
   driven by the a11y `Action::Click` probe (which actions by role+label, bypassing
   pick occlusion) + GPU screenshots; W7's web run was the FIRST real-pointer test.
   Root-caused by layer-by-layer instrumentation (temp global `Pointer<Press/
   Release/Click>` observers + a `Changed<Model>` logger): cursor → press → release
   → click all reached the button, yet no message folded → an invisible occluder.
   Fixed with `.ignore_picking()`.
   **This is at least the third appearance of "an invisible pickable box swallows
   clicks" in project history** (the parity campaign's detached modal absorbing
   clicks; the wasm campaign's cold-synthetic-click artifact; now this).
   **Durable prevention:** a **native-pointer live-interaction test tier** that
   drives real synthetic pointer clicks against the running app on the DEFAULT
   (headless) lane — not GPU-lane-only, and not a11y-clicks (which bypass the exact
   occlusion this class of bug lives in). The parity campaign built a
   live-interaction tier; the lesson is it must be the default gate for any
   `.top_layer()`/transparent-container change, plus the auto-`Pickable::IGNORE`
   for transparent top-layer containers so the bug is unwritable.

2. **Paint order is by primitive TIER, globally, not per-stacking-context** (W1).
   The raster draws in the fill tier; ALL glyphs in a later tier for the WHOLE
   view — so cross-root/cross-context ordering was inexpressible. Fixed for
   Dooduel by the top-layer split (§2); FINAL ships the general per-anchor
   interleave (§3).

3. **Background fills never rounded** (W3). `pack_extracted` hard-coded
   `radius: 0.0`; widgets only *looked* rounded because their border BAND masked
   the square fill corners. Glaring on Dooduel's large colored pills. Fixed by
   completing the stubbed `PackedInstance.radius` path (borderless-rounded, then
   bordered-rounded in W6).

4. **The bordered-rounded "ears" + the radius LENS** (W3/W6). A bordered rounded
   fill showed square-corner ears (band rounds, fill doesn't); a wide
   `border-radius:9999px` box drew a pointed **lens** (the band SDF clamped rx/ry
   independently and used a circular radius). Both latent because the framework
   had **no bordered-rounded fixture** — the app had unknowingly engineered around
   them. Fixed in W6 (fill→inner-radius; per-corner `min(rx,ry)` band). **No golden
   moved** → add the missing fixture as a guard (§3.render).

5. **The light-palette stub discovery** (W3). `default_light_theme()`'s non-accent
   tokens are family-derived STUBS, not the protokit ladder (`text_primary` 0.10
   gray ≈ ink but not `#14161b`; no distinct canvas/surface, no status/on-accent
   tier). The re-baseline's "Track B shipped a complete light palette" is
   field-coverage-true but design-fidelity-false. Dooduel (its first visual
   consumer) pinned the exact ladder via `Color::Custom`.

6. **The persist one-frame race** (W6). `persist_on_change` fired the frame the
   avatar KIND flipped to `Custom` — one frame BEFORE `sync_tools_to_canvases`
   copied scratch→saved — so it persisted a BLANK avatar and never re-fired (key
   unchanged). Caught by the reload assert (face px = 0). Fixed with a
   `saved_version` counter folded into the persist key. **General lesson:** a
   side-surface that lags the model by a frame needs a VERSION the observe/persist
   key includes (the same monotonic-seq shape the tool clear/undo use).

7. **Canvas squish under flex-shrink** (W3a). A `.fill()` root flex-shrank the
   fixed-450px canvas to 134px. Fixed with `.shrink(false)` (and `raster()`
   defaults to it).

8. **Toolbar overflow** (W4). One wrapping design row of 16 swatches overflowed
   horizontally into the chat pane (no flex-wrap). Caught the instant the first
   in-game capture rendered. Split into two rows.

9. **Per-turn-clear vs the harness** (W8). The const phase durations (10s pick)
   auto-picked before a slow file-protocol agent could respond; had to become
   `Config` knobs (draw 120s / pick 45s / reveal 12s) with the consts as defaults
   (every existing test byte-identical).

10. **rustc stack overflow on the bevy bins** (W8). Building the capture/host bins
    intermittently SIGSEGV'd the *compiler* during monomorphization.
    `RUST_MIN_STACK=33554432` (32 MiB) fixes it. Pin in the FINAL's build docs / CI env.

11. **Ticks-vs-seconds in the host view** (W8-live). The `state.json` countdown
    decrements at a non-wall-clock rate (agents measured ~1.3/s and ~4–5/s) — a
    ticks-vs-seconds mismatch in the host report (the game clock itself is correct;
    the host's rendered "Ns left" is not).

12. **The drawer plays blind** (W8-live). "X guessed the word!" chat lines only
    became visible to the drawer AFTER the turn flipped, so one agent wasted a
    clear+redraw on an already-fully-guessed turn. Wants a live guessed-count +
    in-phase chat delivery.

13. **Round-counter overflow** (W8-live). The podium reads "Round 2/1" (`round=2,
    total_rounds=1`); `total_rounds` also reads 2 pre-start vs 1 in-match (stale
    default before host config applies). Confirmed in `state.json` + `seat_1_view.md`.

14. **Stale drawer at podium** (W8-live). `drawer`/`drawer_name` never cleared —
    the final screen says "Drawing: Alex" and tags him "(drawing)".

15. **[NEEDS-VERIFY] Hint letters never revealed** across all 4 live turns. Either
    fast guesses preempted the schedule OR the reveal path is broken — the
    hint-reveal path was NEVER exercised in live play (a targeted test is owed).
    UNVERIFIED either way.

16. **[NEEDS-VERIFY] No wrong-guess feedback.** No echo of your own guess, nothing
    on a miss; the close-guess ("X is close!") path was NEVER exercised live (all
    guesses exact) — no agent saw a mechanism for it. Verify it works + echo guesses
    back. UNVERIFIED.

Not framework bugs but journalled honestly: the **pre-existing GPU-lane
system-COUNT meta-tests** (`render_smoke`/`render_prepare`/`render_compositor`
asserted `==5` extract systems; the raster pipeline added a 6th) — bumped to 6 in a
hygiene commit; the 36 golden IMAGES were always byte-identical. And the stale
`capture_w3a` Home assertion after the W3 rebuild.

---

## 5. DX report — the third product (building a GAME on Buiy today)

This campaign is explicitly dev-experience feedback. The verdict: **Buiy is a
pleasant, honest surface to build a real game on — the friction is a coherent
sizing/layout surface and the absence of scroll, not anything architectural.**

**Positives (ranked by how much they mattered):**

1. **AGENTS.md-only, first-try compile (W0).** `examples/dooduel` compiled + its
   probe test passed off nothing but the front-door docs — a strong signal the
   documented patterns are accurate. This recurred: the render spine, the font
   pipeline, the reconciler, and the pressable-icon route all behaved exactly as
   documented, first try.
2. **view + probe + capture compose with zero fighting.** The three test/build
   surfaces (MVU view, a11y probe snapshot, offscreen GPU capture) stack cleanly;
   `text!`/`on_press`/kind-swap all behave as documented.
3. **The render spine is beautifully factored for extension.** Adding a whole new
   sampled primitive (`RasterImage`) was mechanical: mirror band/gradient (instance
   record + `SpecializedRenderPipeline` + specialize + a draw section). The two W6
   render fixes were surgical (reuse data already carried; zero stride change; zero
   golden churn).
4. **Keyed lists (`keyed_column`) solved scoreboard/chat/word-picker for free** —
   identity + widget-state preserved, replay reconstructs from the model.
5. **The funnel discipline scaled to a whole game.** The single-`Msg`-enum +
   nested-`Game` + thin-`update` split held with no strain at ~10+ reducer arms;
   bots, clock, nav, and in-turn all fold through one drain.
6. **Two canvases by construction** (the keyed resource) and **touch drawing free**
   (a touch drag reaches the paint observers with zero framework work — the
   campaign's biggest web-mobile unknown, retired positive).
7. **Typed theming scales** — the two-layer split (app `Palette` for the design
   ladder + `Theme` resource for widget defaults) re-themes even widgets the app
   can't reach (the `text_input` went dark for free).

**Frictions (ranked by cost):**

1. **No scroll container** — the single most-wanted missing piece. Forced the chat
   cap-to-12 workaround and constrains the mobile single-column. A `scroll_column`
   (+ auto-scroll-to-bottom) is the #1 view FINAL candidate.
2. **The sizing surface grew ad-hoc, wave by wave.** `.width` → `.fill` → `.grow` →
   `.shrink` → `.justify_*` → `.align_*` → `.top_layer` → `.fixed` → `.radius_corners`
   → `.shadow` → `.ignore_picking` each landed when a screen demanded it. The
   *altitude* is right (intents, not raw `Sizing`), but the FINAL should design the
   coherent surface in ONE pass — including flex-wrap, per-side padding, text-align,
   and inset/absolute from the start (all four were worked around, all four are real).
3. **The single-model / single-`ui()` shape** is right for Dooduel but pushes the
   whole app into one 3082-line `lib.rs` view file. Not a bug — but the FINAL's app
   should split per-screen modules, and the framework could document the pattern.
4. **Emoji tofu** — a persistent, cosmetic-but-visible gap across every wave;
   every emoji is dropped or substituted.
5. **Stale rust-analyzer noise** — flagged the new crate "unlinked" (cargo is the
   truth); a recurring low-grade distraction.
6. **`RUST_MIN_STACK`** — the bevy example bins need a 32 MiB compiler stack or
   rustc SIGSEGVs; undocumented until hit.

**The game-on-a-game-engine seam verdict — POSITIVE.** `bevy_state` is not
compiled into the workspace at all, so the phase machine (pick→draw→reveal) went
into **MVU** (the Menu-machine precedent) — and it worked cleanly: the machine is a
pure `Game` + a thin reducer, testable without the funnel. The **Tick-fold clock
was pleasant** (not a workaround — the *right* design; §2). The **rising-edge
side-effect observer** pattern (confetti, per-turn canvas clear, theme sync,
persist all READ the model via a `Local` latch, never enqueue) is the cleanest
demonstration of "a game-state edge drives a decoupled ECS overlay without touching
the funnel." Bevy tweens (`Translate`/`Rotate` on hand-spawned roots) drove
confetti through the transform bridge with zero friction. The seam between Bevy
game mechanics and the Buiy MVU UI is a genuine strength, not a tax.

---

## 6. Residual gaps for the FINAL to close

**Exact-parity deltas still open** (all character-layer; RUN-verified as present):

- Smooth timer ring (currently steps once/second — verified readable, but not the
  design's smooth `dashoffset`; needs a sub-second animate hook or a ring primitive
  + a faint background-ring second layer).
- Dashed / dotted borders (verified renders solid).
- Color emoji (verified tofu).
- The 3D-press *press-down animation* (ships resting-only; the `:active` hook is
  absent).
- Per-axis elliptical wobble corners (render circular).
- Radial-gradient backgrounds (flattened), the rotated ribbon (dropped).
- HiDPI phones (verified mis-scaled at dpr > 1) — the top platform item.

**Playtest fidelity gaps** (for agent play specifically):

- **Progressive stroke streaming.** The real UI streams strokes live (W7 browser
  drives prove it), but the playtest *transport* batches strokes + flushes
  `canvas.png` at ~2 Hz, so the canvas appears all-at-once to guessers — the
  partial-art tension that makes skribbl fun is absent from agent play. A
  per-stroke PNG flush would restore it. (This is a playtest-host + agent behavior
  gap, NOT a UI gap.)
- **Real-device IME / soft keyboard.** Typing works via the shipped `WebImePlugin`
  hidden-input bridge (verified in emulation), but headless emulation cannot raise
  a real on-screen keyboard nor exercise `ime_position` tracking / the touch-only
  policy — UNVERIFIED on a real device.
- **Multi-App concurrency is UNRETIRED.** The playtest used a single-model
  file-protocol host, not N concurrent Apps in one process (the audit-flagged
  "prototype risk to retire early"). If the FINAL wants true client-server or
  multi-instance topology, that risk is still open.

**Design-questions to put to the human** (gameplay decisions the playtest exposed,
not framework work):

- **Drawer payout formula.** Flat +100 regardless of how many/how fast guessers
  succeed → your drawing turn is a guaranteed score deficit (Sam guessed everything
  first-try and still placed last; final Priya 1052 / Theo 980 / Alex 816 / Sam
  686). skribbl scales the drawer's points by guesser success — check the design's
  intended formula.
- **Hint-reveal schedule** — verify the reveal path works at all (never fired live)
  and confirm the seconds-left thresholds.
- **Turn indicator** — no "turn X of N"; the previous drawing lingers through the
  next Picking phase; wrong-guess feedback + own-guess echo are missing.

---

## 7. Build strategy for the FINAL

### The framework work ships as general-purpose PRs (dogfooded by Dooduel)

Per the charter, framework features ship as their own reviewed, general-purpose
PRs — NOT Dooduel-local code. The prototype's framework commits are cleanly
separable (they touch `crates/` only, distinct from the `examples/dooduel` app
commits). Cherry-pick candidates, in dependency order, by commit:

| Prototype commit(s) | Crate(s) | FINAL disposition |
|---|---|---|
| `646aabc` + `22c3a91` | buiy_core | **PR-F1** re-implement clean: `raster`/image-node primitive (distinct pipeline). Bump the render system-count constants. Ship WITH the missing bordered-rounded golden fixture. |
| `6ef6df5` | buiy_view + buiy | **PR-F2** the *coherent* view sizing/layout surface — but **redesigned in one pass**: fill/grow/shrink/justify/align **plus** flex-wrap, per-side padding, text-align, inset/absolute (the prototype proved all are needed). Includes the `raster()` element + identity patch. |
| `2854643` | buiy_core | **PR-F4a** — **SUPERSEDE** the top-layer split with the general per-raster-anchor interleave. |
| `fc2a72b` (fw parts) | buiy_view + buiy_core | **PR-F3** styling surface (`Color::Custom`/`rgb`, `.color/.font/.border` with a `LineStyle` arg, `icon()` + a viewbox arg, styleable buttons) **+ PR-F4b** borderless-rounded fill (render). |
| `f4357af` (fw parts) | buiy_view | **PR-F5** press routing: pressable icon (KEEP) + a **container/raster press route** (the residual) + a controlled `:active` hook. Plus `.shrink`/`.align_start` (fold into PR-F2). |
| `fb4cc18` (fw parts) | buiy_core + buiy_view | **PR-F4c** render: bordered-rounded fill (decide the wider instance for per-corner), per-corner band + `min(rx,ry)` lens fix, elliptical SDF, dashed stipple, raster rounded clip, per-quad particle alpha, shadow corner radius. `.radius_corners/.shadow/.justify_end/.align_end` → PR-F2/F3. **The biggest render PR — may split.** |
| `9237822` (fw part) | buiy_view | **PR-F6** `.ignore_picking()` + auto-`IGNORE` for transparent top-layer containers + the **native-pointer live-interaction test tier** (the occluder guard). |
| `f92cb04` | buiy_verify | **PR-F8** stroke/drag helper + `drive_stroke` + the unified headless driver recipe (bake `init_asset::<Image>()`). |
| *(net-new, not in the prototype)* | buiy_core mvu | **PR-F7** the G3 ergonomic: `ClockPlugin<M>` / `Cmd::tick_every` (poll-clock-as-Msg, NOT edge triggers) + `on_submit_with` + a headless `Time::advance_by`. |
| *(investigation)* | buiy_core | **PR-F9** the HiDPI/web scale-factor bug — blocks real phones. |

Suggested sequencing: **F1 → F2 → F3 → F4 → F5 → F6 → F7 → F8**, with F9 (HiDPI)
in parallel as an investigation. F1/F2 are foundational (nothing renders without
the primitive + the layout surface); F4 (render fixes) is the largest and gates
visual parity; F7 (clock) is small and unblocks the game loop; F6/F8 are the
playtest + interactivity guards.

### Re-implemented cleanly (not cherry-picked)

The paint-order fix (redesign to per-anchor, F4a) and the sizing surface (redesign
coherently, F2) are re-implementations, not ports — the prototype's shapes are
right but the FINAL does them better. The per-corner FILL radius (F4c) needs the
`PackedInstance` stride decision the prototype deferred.

### The app (`examples/dooduel`)

**Restructure, don't keep-shape.** The game core (`game.rs` — pure, zero framework
coupling, unit-testable) ports **nearly verbatim** — it is the highest-quality,
most-reusable app code. `paint.rs` (keyed `PaintCanvases` + the model→canvas sync)
and `storage.rs` (the typed per-target persistence seam) port cleanly. But
`lib.rs` (3082 lines, the whole view + ~40 helpers in one file) should **split into
per-screen modules** (home/join/lobby/in_game/podium/avatar_editor). Keep the
single-model/single-`ui()` shape (correct for Dooduel) but fix the gameplay bugs
§4.11–16 (round-counter overflow, stale drawer, drawer-blind, wrong-guess feedback,
hint-reveal — write the targeted tests the playtest showed were missing). The
`dooduel_web` crate + `install_runtime` (the shared native/web plugin set) + the
`ViewportPlugin` responsive shell port as-is. The **file-protocol `playtest_host`
is a FINAL-shippable playtest harness** — graduate it with a richer `state.json`
(word length, hint count, per-seat "can I act now" flags), an in-protocol
`continue`/skip-reveal, and per-stroke streaming (§6).

### Docs debt to main (before worktree cleanup)

The `prototype-first-development` skill mandates committing the learning docs to
the durable `docs/` system before the throwaway worktree is removed. What must
reach `main` via a docs PR:

- This retrospective + the [journal](2026-07-02-dooduel-PROTO1-journal.md) +
  the [playtest evidence](2026-07-02-dooduel-PROTO1-playtest/) (transcript,
  host.log, state.json, canvas.png, sample seat view) — all currently on
  `worktree-dooduel-proto1`, unpushed.
- The campaign docs currently on `worktree-scribbl-campaign` (also unpushed): the
  charter, the re-baseline audit, and the `reference-designs/dooduel/` bundle +
  `REQUIREMENTS-DELTA.md`.
- Update `docs/README.md` (the master index) and the
  `buiy-scribbl-campaign` memory to point Phase A → RETROSPECTIVE-DONE, FINAL-next.

Only the throwaway *code* stays unmerged.

---

## 8. Provenance

- Journal: `docs/prototypes/2026-07-02-dooduel-PROTO1-journal.md` (W0–W8-live, each
  RUN + verified by GPU capture and/or probe test before journalling).
- Playtest evidence: `docs/prototypes/2026-07-02-dooduel-PROTO1-playtest/`
  (`commands.jsonl`, `host.log`, `state.json`, `canvas.png`, `seat_1_view.md`).
- Campaign context (sibling worktree `worktree-scribbl-campaign`):
  `docs/prototypes/2026-07-01-scribbl-campaign-charter.md` (+ the 2026-07-02
  amendment) and `docs/reports/2026-07-02-dooduel-rebaseline-audit.md`.
- Prototype framework code (unmerged, `worktree-dooduel-proto1` off `a969cbf`):
  `crates/buiy_core/src/render/{raster.rs,raster.wgsl,extract.rs,buckets.rs,
  node.rs,band.wgsl,…}`, `crates/buiy_view/src/{element.rs,reconcile.rs,tokens.rs}`,
  `crates/buiy_verify/src/pointer.rs`, and the app in `examples/dooduel` +
  `examples/dooduel_web`.
- House-style reference: `2026-06-26-mvu-as-core-PROTO3-RETROSPECTIVE.md`
  (sibling doc in this directory).
