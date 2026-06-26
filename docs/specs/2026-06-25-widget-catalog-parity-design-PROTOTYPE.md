# Widget Catalog — Exact Visual Parity (Prototype Spec)

**Status:** `[superseded]` by the Phase 3 FINAL design
`docs/specs/2026-06-26-widget-catalog-parity-final-design.md` (the mergeable,
human-review-gated version that re-decided this prototype's choices from the full
picture). This Phase 2 (PROTOTYPE) target-state design is **exploratory and was
never merged** — it remains here as the reference that informed the final. The
prototype→final decision record is `docs/reports/2026-06-26-parity-final-decisions.md`.
**Not for merge.**
**Research:** `docs/reports/2026-06-25-parity-research-findings.md` (8-track fleet,
full coverage). **Target:** `docs/reference-designs/widget-catalog/Widget Catalog.dc.html`.
**Journal:** `docs/reports/2026-06-25-parity-prototype-journal.md`.

## 1. Goal & scope

Reach **exact visual parity** with the `Widget Catalog.dc.html` Claude design —
the real thing, not an approximation. The design is a dark, Geist-font, 3-pane
IDE-style gallery shell hosting the 5 screens that already exist as isolated
binaries in `buiy_gallery`. Parity requires (a) building the **framework
capabilities** the design leans on but Buiy lacks, and (b) building the
**unified shell** that hosts + switches + themes the screens.

**In scope:** dark token system + runtime accent theming; gradient fills;
transition/animation system; backdrop-blur; vector/SVG icons; Geist fonts +
letter-spacing; the shell (chrome / rail / viewport / inspector / status / toast)
+ `ScreenRouter` + composites; the dotted-grid bg, custom scrollbars, selection,
glow, kbd chips, the active-nav bar.

**Non-goals (this prototype):** merging; a full CSS cascade/inheritance engine
(flat global `Theme` + per-subtree override only); animated gradient stops;
per-keyframe easing; true row-recycling virtualization (paint-skip suffices).

**Prototype discipline:** the value of this phase is *learning what the build
actually takes*. Favor proving each approach end-to-end and journaling the
friction over polishing any single piece. The final phase re-decides.

## 2. Target state (what "done" looks like)

`cargo run -p buiy_gallery` opens the **whole shell** (not one screen): the top
chrome, the left Screens rail (5 nav buttons w/ active accent bar + icon + name +
desc) over a Stats block, the center viewport (header strip + dotted radial-grid
canvas) showing the active screen, the right Inspector (composed-of chips +
live-state + 4 accent swatches that **re-theme the app live**), a status bar, and
a transient toast. Clicking a rail button swaps the viewport screen; clicking a
swatch re-themes; every screen is fully interactive and **matches the design's
exact colors, type, spacing, radii, icons, gradients, and eased motion**.

Verification is the `buiy_verify` harness (layout/display-list snapshots,
reftests, goldens) **plus running the actual GUI** (the widget-catalog campaign's
hard-won lesson: headless-green ≠ works).

## 3. Capability builds

Each references the research report for full detail. Complexity from the fleet.

### 3.1 Theming & token system  *(large)*
- **Build** `default_dark_theme()` with the **~33 named tokens** the design uses
  (surfaces `#0b0c0e/#0d0e11/#16181c/#121417/#1a1d22/#1e2127`; borders
  `#1c1f24/#262a31/#2c313a/#3a4150/#3a2422`; ink
  `#f1f3f6/#e7eaef/#c2c8d2/#868d99/#6f7783/#555c67/#3a4049`; accents
  `#5b86f5/#45c07d/#b98aff/#f0655b`; status `#45c07d/#d7a23f/#f0655b`; surface.danger
  `#391b1a`; text.on-accent `#07101f`) under a `surface/border/text/accent/status`
  taxonomy, plus space (`0/4/8/12/16`) and radius (`2/6/12` + component-local
  8/9/10/12/14) scales.
- **Accent ramp** `--ac2 = lighten(ac, +22%)`, `--acsoft = rgba(ac,.16)`,
  `--acglow = rgba(ac,.55)` — computed **once at the `Theme`-mutation edge** (a
  `SetAccent` message mutates `Res<Theme>`; the existing `theme.is_changed()`
  re-extract re-resolves every paint). Lighten formula verbatim from the design:
  `v + (255-v)*0.22`.
- **Decision: reject `ColorToken::Literal(Color)`** — keeps every paint a token
  reference so the forced-colors gate-#11 analyzer stays enforceable; one-off
  shades become named tokens. *(Flagged for the final phase to re-weigh: the
  gallery has dozens of one-off hexes; token-naming churn is the cost.)*
- **Plugs into:** `theme.rs` (new `default_dark_theme`, accent-ramp fn, swap
  system), `render/color.rs` (resolver unchanged), the `BuiyPlugin` theme insert.

### 3.2 Gradient fills  *(medium)*
- **Build** `Background.layers: Vec<BackgroundLayer>` =
  `Solid(ColorToken) | Linear(LinearGradient) | Radial(RadialGradient)` with
  token-bearing `ColorStop`s; keep `Background.color` as the F-tier solid fast path.
- **GPU:** compute gradients **in the rect SDF fragment shader** (linear =
  projection onto the angle axis; radial = distance-to-center), stop colors
  resolved to concrete `Color` at extract — no atlas bake, no async.
- **Design needs:** `linear-gradient(150deg, --ac, --ac2)` (logo, slider preview,
  accent buttons), `linear-gradient(90deg, --ac, --ac2)` (meter fill). The
  dotted bg (§3.8) is a radial special-case.
- **Plugs into:** `render/components.rs` (the reserved `layers` seam),
  `shader.wgsl`, `extract.rs` (resolve stops), `prepare.rs` (instance data).

### 3.3 Transition / animation system  *(medium)*
- **Build** a lightweight tween registry (reject `bevy_animation` as over-weight):
  `Tween<T>` (from/to/duration/elapsed/easing/on_complete), `Easing::CubicBezier`
  via a 64-pt Newton-Raphson LUT, animatable bindings (Translate / Rotate / Scale
  / Background color / Opacity / Width), an optional `KeyframeTrack` for
  blink/spin/entrance, and tween-update systems in a **new `BuiySet::Animate`**
  (after Input, before Picking). **Reduced-motion gate** jumps to end-state.
- **Design needs:** switch thumb (.15s cubic-bezier(.2,.8,.2,1)), progress width
  (.3s), chevron rotate (.15s), blink (1.6s), spin, menu/modal/toast entrance
  (opacity+translateY+scale). Widgets spawn a tween on their state-change event.
- **Decision:** explicit tween-spawn in widget systems (reject implicit
  auto-detect-on-change as too magical).
- **Plugs into:** new `buiy_core::anim` module, `BuiySet`, the layout transform
  components, render damage gating (`is_changed`).

### 3.4 Backdrop-blur  *(large)*
- **Build** a blur into the existing effect-compositor seam (`BackdropFilter`
  component + `EffectReason::BACKDROP_FILTER` bit already reserved; pooling done):
  a **dual-Kawase** blur (O(log r), 4 passes for blur(6px)) over a parent-region
  copy to an `Rgba16Float` scratch, composited under the element.
- **Design needs:** modal backdrop `rgba(4,5,7,.66)+blur(2px)`; viewport header
  `#0d0e11cc+blur(6px)`. *(Highest-cost item; the nested-parent scratch path is
  the scope risk — the prototype proves it out or documents a fallback.)*
- **Plugs into:** `render/effect.rs`, `compositor.rs` (pass insertion at the group
  subtree boundary), a new blur `.wgsl`.

### 3.5 Vector icons  *(medium)*
- **Build** a vector icon path: an `Icon { path_d, stroke_width, size_px }`
  component; a producer that **tessellates the SVG `d` via lyon**, rasterizes to
  an **R8 coverage bitmap**, and inserts into the **existing glyph-alpha atlas**
  (keyed by `hash(path_d, stroke_width, scale, size)`); emit glyph-alpha
  instances with the resolved accent token as per-instance tint (live recolor,
  no atlas mutation). Reuses the proven atlas (eviction/warmup/tint) and needs
  **no new GPU code**.
- **Decision:** real vector over an icon-font — Lucide bakes stroke-width (≈2.0)
  but the design uses 1.7–2.4, visibly off at 24px. Lucide kept only as a
  **fallback glyph** for any missing icon. (SDF + PNG-sheet rejected: shader cost
  / no live tint.)
- **Design needs:** ~25 stroke icons at 13–24px. The gradient logo is the hard
  case — render its `<path>` as a flat accent glyph, or special-case a small
  gradient quad behind it.
- **Plugs into:** new `Icon` component + `render/icon_producer.rs`, a small
  `buiy_icon_rasterize` helper (lyon + scanline), the atlas + glyph-alpha shader.

### 3.6 Fonts & text  *(medium)*
- **Build** embed **Geist** (400/450/500/600/700) + **Geist Mono** (400/500/600)
  (OFL) under `crates/buiy_core/assets/fonts/`, register both into cosmic-text's
  `FontSystem` at `BuiyTextPlugin` init; expose `GenericFamily::Monospace`
  first-class; add a **`LetterSpacing(f32)`** component wired through
  `cosmic_text::Attrs.letter_spacing` in `text/sync.rs` (design range
  `-.025em … .14em`). Confirm `FontWeight` + `LINE_THROUGH` + `TextAlign` suffice
  (they do).
- **Type scale:** ~14 sizes 10–30px, sans/mono split per element (documented in a
  `TypeScale`-style helper for the gallery to author against).
- **Plugs into:** `text/components.rs` (+`LetterSpacing`), `text/sync.rs`
  (Attrs + triggers + `SyncedText`/`AuthoredStyle`), font-asset loading.

### 3.7 Shell, router & composites  *(large)*
- **Build** `enum Screen` + `ScreenRouter` resource + `SwitchScreen`/`SetAccent`
  messages + an **exclusive applier** (`.after(BuiySet::Input).before(A11yUpdate)`)
  that despawns the `#ScreenContent` subtree and spawns the selected screen, with
  per-screen state isolation (each screen owns its resources/markers).
- **Shell** = a `shell_root` scene: top chrome, left rail (nav + stats), center
  viewport (header + `#Canvas` → `#ScreenContent`), right inspector (chips +
  live-state + swatches), status bar; a viewport-header system reflects
  `ScreenRouter`; a toast resource + `Timer` drives show/fade/despawn in
  `BuiySet::Animate`.
- **Composites** (scene-fns, some promoted to `buiy_widgets`): `stepper`,
  `segmented`, `search_input`, `meter`, `toast`, `badge`, `stat_row`,
  `table_row` (selection + sticky header).
- **Per-screen plugins** generalize the existing intent-collector + exclusive-
  applier pattern (`TodoMvcPlugin` is the template).
- **Plugs into:** `examples/buiy_gallery` (largely new), `buiy_widgets/scene.rs`
  (promoted composites), layout grid/flex/position, `bevy::time::Timer`.

### 3.8 Dotted-grid bg, scrollbars, selection & polish  *(medium)*
- Viewport **dotted radial-grid** (`radial-gradient(#16181c 1px, transparent 1px)`
  22px tile) — a `Pattern` background variant computed in-shader (builds on §3.2).
- **Custom scrollbars** (10px track, `#262a31` thumb, 3px transparent border,
  rounded) on `ScrollArea`.
- **Selection** `rgba(91,134,245,.32)` (token exists); **focus rings** mapped to
  design values; **glow dots** via `BoxShadow` spread (reuse, no new component);
  **kbd chips** (scene-fn); the **active-nav accent left-bar** (2.5px, 99px radius).
- **Plugs into:** `scroll_area.rs`, selection/caret color, `render/components.rs`.

## 4. Integration architecture

```
                 ┌── theme.rs: default_dark_theme + accent ramp + SetAccent swap
  Theme/token ───┤
                 └── color.rs resolver (unchanged) ── re-extract on is_changed
                              │
  Render caps ────┼── Background.layers (Solid/Linear/Radial) → SDF shader      (§3.2,3.8)
  (render/) │     ├── Icon → lyon tessellate → R8 atlas → glyph-alpha shader    (§3.5)
            │     └── BackdropFilter → dual-Kawase scratch pass (compositor)    (§3.4)
            │
  Text ─────┼── Geist/Geist Mono embedded + LetterSpacing → cosmic-text Attrs   (§3.6)
            │
  Anim ─────┼── buiy_core::anim: Tween<T> + Easing LUT in BuiySet::Animate      (§3.3)
            │
  App ──────┴── buiy_gallery: shell_root + ScreenRouter + per-screen plugins
                + composites, authoring against ALL of the above               (§3.7)
```

The render-capability builds (§3.2/3.4/3.5/3.8) **share files** (`components.rs`,
`extract.rs`, `shader.wgsl`, `compositor.rs`) — they must be sequenced or
file-partitioned, not run as naive parallel worktree merges (collision risk).

## 5. Build sequencing (feeds the plan)

- **Wave A — foundations (parallel-safe, disjoint files):**
  A1 fonts + `LetterSpacing`; A2 dark theme + tokens + accent-swap; A3 the
  `anim` tween module (new files + `BuiySet::Animate`). No cross-file collision.
- **Wave B — render capabilities (sequenced; one coherent render-extension
  stream):** B1 gradients (`Background.layers` + shader); B2 dotted-grid pattern
  (radial, on B1); B3 vector icons (atlas + lyon); B4 backdrop-blur (compositor).
  B1→B2 ordered; B3/B4 can interleave but coordinate on shared files.
- **Wave C — shell integration:** C1 `ScreenRouter` + `shell_root` skeleton + nav
  switching; C2 composites; C3 wire the 5 screens with exact content; C4
  inspector (chips/live-state/swatches) + toast + status bar.
- **Wave D — parity pass:** adopt tweens on widgets; exact spacing/radii/selection/
  scrollbars/focus/glow/kbd/nav-bar; **run the GUI**; golden/reftest parity checks;
  journal the deltas.

Each wave is **reviewed by a fresh fleet** (spec-alignment + correctness +
build/test gate) before the next. Capture learnings in the journal continuously.

## 6. Verification

- Headless gate: `cargo fmt`/`clippy`/`doc` + `cargo test --workspace` (the
  CLAUDE.md "run all checks"; `xvfb-run` on Linux).
- GPU lane (`#[ignore]`, lavapipe): render-capability builds add goldens/reftests
  here (gradients, icons, blur, dotted-grid).
- **Run `cargo run -p buiy_gallery`** each wave — the campaign's lesson.
- Parity check: side-by-side the running shell against the design screenshots;
  log pixel/measurement deltas in the journal.

## 7. Open questions & risks (the prototype will resolve these for the final)

1. **`ColorToken::Literal`** — rejected here for gate-#11; but dozens of one-off
   hexes make token-naming heavy. Does a *gated* literal escape-hatch pay off?
2. **Backdrop-blur nested parents** — the scratch-target path is the cost/risk;
   prove it or fall back to a flat semi-opaque scrim and document.
3. **Gradient-in-shader vs atlas LUT** — shader keeps it dynamic; confirm the SDF
   rect shader can carry gradient params without bloating every instance.
4. **Icon gradient logo** — flat-accent glyph vs a gradient quad behind the path.
5. **Render-file collision** — how finely to partition `components.rs`/`extract.rs`/
   `shader.wgsl` so Wave-B agents don't merge-conflict.
6. **Tween ⇄ layout** — animating `Width`/Translate vs Taffy-owned layout each
   frame; does it thrash relayout? Prefer transform-only where possible.
7. **Scope realism** — 8 capabilities incl. 3 "large"; the prototype may land
   some capabilities at "proven, not polished" and say so in the journal.

## 8. Review resolutions & locked decisions (supersede §3–§7 where they conflict)

Fresh-fleet review (feasibility / parity-completeness / sequencing / adversarial).
Corrections and decisions that bind the plan:

**Codebase corrections.**
- `BuiySet::Animate` **already exists** as an enum variant (no systems) — the anim
  work *wires systems into it* + adds a new `buiy_core::animation` module; it does
  not create the set.
- `Background.layers` is **not** a reserved field today — `Background.color` is the
  only fill. The gradient work **adds** the `layers` field (the source comment is
  aspirational).
- `BackdropFilter` component + `EffectReason::BACKDROP_FILTER` bit exist, but the
  parent-region capture seam (`ViewTarget::post_process_write` or equivalent) is
  **unproven on Bevy 0.19** — Wave B4 starts with a spike; **fallback = flat
  semi-opaque scrim** (`color.scrim`) documented as a v1 limitation.
- Only `FiraSans-Regular-latin.ttf` is embedded — Wave A1 must **source Geist +
  Geist Mono (OFL)** and confirm the font-asset load path.

**De-risk-first (each is the FIRST task of its capability, gating the rest):**
1. **Gradient instance stride** — 4-stop ≈ 84 B vs ~68 B budget. Probe on a
   1000-quad scene; if draw-cost >10%, use a **separate stop-color GPU buffer**
   instead of inlining stops in `PackedInstance`.
2. **Backdrop-blur seam** — spike `ViewTarget` access before building the blur.
3. **Icon cold-start** — 25 icons tessellate+rasterize must be <~50 ms or pre-warm
   at init.
4. **Progress Width** — drive via a **post-layout width override** (not per-frame
   Taffy mutation); confirm no relayout thrash.
5. **Accent-swap hitch** — measure frames-to-repaint at 1000 rows on swatch click.

**Locked decisions.**
- **Token-only stays** (reject `ColorToken::Literal`): modal backdrop
  `rgba(4,5,7,.66)` becomes the named token **`color.scrim`**; add **`color.border.danger`**
  (`#3a2422`) and **`color.scrollbar.thumb-hover`** (`#39404a`). One-off shades are
  named tokens.
- **Logo is not an icon-gradient problem**: it is a small rounded **box with a
  `Linear` gradient `Background`** + a **flat icon glyph** (`#07101f`) on top —
  exactly the design's `<div bg=gradient><svg/></div>`.
- **Gradient stops are opaque** `ac`/`ac2`; `acsoft`/`acglow` are for box-shadow
  glow + selection only.
- **Tween authoring rule**: animate **transform + opacity only**; never animate
  Taffy-owned layout per frame. The progress meter animates a post-layout width
  override.
- **Wave B is sequential** on the shared render files (`render/components.rs`,
  `render/extract.rs`, `render/shader.wgsl`, `render/compositor.rs`) in one
  worktree (B1→B2→B3→B4); only genuinely new files
  (`render/gradient_extract.rs`, `render/icon_producer.rs`, `render/blur.wgsl`)
  may be authored in parallel. No naive parallel worktree merges on shared files.
- **ScreenRouter mechanism** is co-designed at the start of Wave C (candidate:
  keep all 5 screen subtrees spawned and toggle `CssVisibility`/`A11yHidden` like
  the todo filter — avoids respawn/refocus/state-loss — vs despawn/respawn with
  explicit per-screen resource cleanup). The plan picks one with a familiar-agent
  co-design pass.

**Exact-values reference.** §3 gives ranges; the **single source of truth for
exact parity** is `docs/specs/2026-06-25-widget-catalog-values.md` (generated from
the design HTML): every color token, box-shadow, border-radius (incl. asymmetric),
font size/weight/family/letter-spacing per element, transition duration+easing
(incl. the `.12s` generic default + `blink`/`spin`/entrance timings), per-icon
stroke-width + size + color-per-state, and per-element border/padding/gap. All
implementers author against that table, not re-derive from the HTML.

**Verification acceptance criteria (Wave D goldens):** gradient stops within ±1 px
at 24 px scale; icon stroke edges visually match (subpixel AA acceptable);
accent-swap re-extract <1 ms (no visible frame drop). Run the GUI each wave.
