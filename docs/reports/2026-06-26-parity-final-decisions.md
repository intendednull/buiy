# Widget Catalog Parity — FINAL DECISIONS (the reviewer's guide)

**Status:** `[active]` — the human-review gate's decision record for the
`parity-final` branch. Written at Wave 6 (the final prep wave), HEAD on
`parity-final`, off `main` @ `fdb8dda`.
**Reads with:** the binding spec `docs/specs/2026-06-26-widget-catalog-parity-final-design.md`,
the plan `docs/plans/2026-06-26-widget-catalog-parity-final.md`, the re-decided
architecture `docs/reports/2026-06-26-parity-final-research-decisions.md`, the
exact-values table `docs/specs/2026-06-25-widget-catalog-values.md`, and the
prototype journal (in the `parity-prototype` worktree,
`docs/reports/2026-06-25-parity-prototype-journal.md`).

This document is the single place a reviewer can read **what shipped, what
changed from the prototype and why, and the verification evidence** without
replaying the whole campaign. The PR body summarizes; this is the long form.

---

## 1. The prototype → final narrative (one paragraph)

The campaign ran in three user-directed phases: (1) land the 5-screen
widget-catalog gallery (PR #80 → `main` @ `fdb8dda`); (2) a **throwaway
PROTOTYPE** (`parity-prototype`, never merged) that proved exact visual parity
with the Claude `Widget Catalog.dc.html` design is achievable end-to-end —
building every framework capability the design needs (dark theme + accent ramp,
animation/tweens, Geist fonts + letter-spacing, gradients, vector icons,
backdrop-blur, the unified shell + router) and reaching **1774 headless + 91 GPU
green** with all 5 screens screenshot-verified; (3) **this FINAL** — a fresh
worktree that **re-decided every prototype choice with the full picture**
(`research-decisions.md`, a 7-track fleet) and then **ported the GPU-proven work
+ landed the re-decided refinements** as a reviewable, merge-gated branch. The
final is a HYBRID: re-deriving sound, GPU-proven code from scratch is not best
practice, but re-*deciding* it is — so the validated prototype commits were
cherry-picked for the KEEP work and the REFINE/REDESIGN items landed as
deliberate phase-3 changes. Both branches share the base `fdb8dda`, which made
the cherry-picks clean. The result: the same exact-parity gallery, on
production-intent architecture, gate-green and human-review-gated.

---

## 2. The re-decided architecture — what shipped (KEEP / REFINE / REDESIGN)

The binding set is spec § 2; the per-subsystem rationale is in
`research-decisions.md`. Below is **what the reviewer will actually find in the
code**, with the verdict it realizes.

### KEEP — ported the validated prototype work as-is

| Decision | What shipped | Where |
|---|---|---|
| **`BackgroundLayers` sibling component** (not `Background.layers`) | `BackgroundLayers(Vec<BackgroundLayer>)` decomposed sibling — source-compatible with the 103 `Background { color }` callers + the 22 `bsn!` patch sites; a node with no gradient simply carries no `BackgroundLayers`. | `crates/buiy_core/src/render/components.rs` |
| **Distinct 104 B `GradientInstance`** (not inlined into `PackedInstance`) | The band/shadow precedent — its own record + `gradient.wgsl` + `RawBufferVec` + draw call, drawn in the quad paint slot. `PackedInstance` stays byte-stable (R2 compositor re-tint indexes its frozen offsets). 2 inlined stops (the design only ever uses 2). | `render/components.rs`, `render/gradient*.rs`, `gradient.wgsl` |
| **lyon → R8 glyph-alpha vector icons** | `Icon { path_d, stroke_width, size_px, fill, color }` → lyon stroke-tessellate → R8 coverage → the EXISTING glyph atlas → a `GlyphAlphaInstance` tinted per-instance (re-tints live on accent swap). | `render/icon_producer.rs`, `render/icon_raster.rs` |
| **`cross_root_rank` stacking** (M1 + M6 fix) | Threaded through `StackingContext` + `layout/systems.rs` + `render/extract.rs` + `text/extract.rs` + `picking/depth.rs`. Top-layer (modal) + anchor-override (menu) descendant fills + text now rasterize. | 5 files; see § 5 |
| **`SkipReason::DisplayNone` paint-skip** (M5 + header artifact) | `write_paint_skip` roots a subtree skip on a runtime `Display::None` flip, mirroring `write_clip_rects`'s existing pruning. | `render/visibility.rs` |
| **~33-token dark palette + accent ramp + `SetAccent`** | `default_dark_theme()` + `derive_accent_ramp()` (exact JS lighten math) + the `SetAccent(Color)` message → `theme.is_changed()` re-extract. **Token-only — no `ColorToken::Literal`** (gate-#11 forced-colors discipline). | `crates/buiy_core/src/theme.rs` |
| **`Tween<T>` + `Easing` + `Repeat`** animation | Per-property tween components (Translate/Rotate/Scale/Opacity/BackgroundColor), `Easing::DESIGN`, `Repeat::{Once,Loop,PingPong}` (infinite blink), reduced-motion = snap-to-rest. (Reject `bevy_animation`, confirmed.) | `crates/buiy_core/src/animation/` |
| **Geist + Geist Mono variable faces** | Embedded (OFL); monospace generic re-pinned to Geist Mono; entity-level decorations. | `crates/buiy_core/src/text/` |
| **`ScreenRouter` via `Display::None` toggle** | All 5 screens spawned once; the router toggles `Display::None` + `A11yHidden` (zero hidden-screen layout cost — Taffy prunes the subtree; state-isolation free). | `examples/buiy_gallery/src/shell.rs` |
| **Paint-skip virtualization** | All 1000 scroll rows spawned + `ContentVisibility::Auto`; inspector "mounted" = the windowed visible count (matches the footer); "nodes" = the filtered total. | `examples/buiy_gallery/src/` |

### REFINE — production cleanups landed as deliberate phase-3 changes

- **`AnimatedBackgroundColor` → auto-composite.** The prototype left this an
  opt-in widget special-case ("Wave D decides"). The final promotes it into the
  render extract's `resolve_background_color`: when a node carries
  `AnimatedBackgroundColor` (a live `BackgroundColorTween` writes it) the
  extract paints that color over the static token, for *every* node, falling
  back to the token `bg` at rest. One check, zero per-widget duplication.
  (`render/extract.rs:resolve_background_color` + the `NodePaintQuery.animated_bg`
  field.)
- **`buiy::prelude` + promotions.** A `prelude` module re-exports the common
  surface. The everyday authoring primitives the gallery proved are promoted to
  the `buiy` crate root: `LetterSpacing`, `SetAccent`, `Tween`/`Easing`/`Repeat`
  (+ the per-property tween components), and — **closed in Wave 6** (see § 3) —
  the gradient/icon fan `BackgroundLayers`/`BackgroundLayer`/`LinearGradient`/
  `RadialGradient`/`ColorStop`/`Icon`. (`crates/buiy/src/lib.rs`.)
- **Composites → promote the genuinely-general ones to `buiy_widgets`.**
  `meter`, `table_row`/`table_header`, `search_input`, `kbd`/`kbd_content`,
  `status_dot`, `pulse_blink` (+ `MeterFill`/`RowSelBar`/`TableRow`/`TableRowData`
  and the `set_*` mutators) now live in `buiy_widgets::composites` (font-neutral —
  each text-bearing builder takes a `FontFamily`); screen-specific compositions
  stay gallery-local. (`crates/buiy_widgets/src/composites.rs`.)

### REDESIGN — done the production way, not the prototype's mid-flight patch

- **Extract-query arity → a named `NodePaintQuery` projection (the
  spec-premise correction).** The prototype's flat `extract_buiy_nodes` query
  hit Bevy's 15-term `QueryData` tuple ceiling and was patched mid-flight by
  nesting two terms in a sub-tuple (always flagged as a stopgap). The spec § 2
  REDESIGN proposed splitting extract into **separate producer systems**
  (`extract_buiy_base/colors/effects/gradients/icons`, each mutating a shared
  `ExtractedNodes` map). **The final corrected that premise:** a multi-system
  split would desync the single retain-damage gate against the GPU-proven
  paint-order walk (architecture.md § 3.1 / the R5 retain-damage design). So the
  partition was done at the **data-projection layer** instead — one
  `#[derive(QueryData)] struct NodePaintQuery` whose fields are grouped by the
  same logical sub-systems (base / colors / effects / gradients), with **no
  arity ceiling** (a derive expands fields without the tuple cap). Extract stays
  ONE system behind ONE damage gate; a future paint input is a new field, never
  another sub-tuple. This mirrors the established `a11y::A11yNodeQuery` fix for
  the identical ceiling. The FILTER-side damage probe got the same treatment: a
  nested `Or<(EffectGroup, Opacity, BackdropFilter)>` sub-union keeps the outer
  filter tuple under the 15-arity `QueryFilter` ceiling **and** adds a principled
  re-extract term for a runtime backdrop-blur-radius edit (the one paint input
  the old gate would have missed). (`render/extract.rs:1015-1205`.)
- **Headless harness → a public `BuiyHeadlessPlugin`.** The prototype
  hand-rolled a `capture_shell` plugin list that drifted from `BuiyPlugin`. The
  final ships `buiy::BuiyHeadlessPlugin` — the Buiy data-side subset
  (theme → layout → core → text → focus → a11y → widgets → render; no
  winit/picking/scroll/animation) as ONE plugin. Both capture bins
  (`capture_shell`, `capture_composites`) now compose it + the bevy headless
  primitives, so the offscreen path no longer maintains a parallel plugin list.
  (`crates/buiy/src/lib.rs:BuiyHeadlessPlugin`.)
- **Verification → dual-path (headless always + relative-property GPU on
  lavapipe).** See § 4 for the strategy and why exact-pixel screen goldens are a
  documented CI-time follow-up rather than landed here.

### Resolved inter-track disagreements (spec § 2 is binding — note where it
overrode a research track)

1. **LetterSpacing contract = PX, not em.** The text-fonts research track
   recommended switching to an em-direct authoring contract (delete the
   `px / font_size` division). The binding spec § 2 resolution 1 **overrode**
   that: keep the prototype's logical-px contract (`spaced()` lowers
   `px / font_size` so the on-screen advance is exactly the authored px at any
   size). Rationale: px is concrete + intuitive for authors, the values table
   already gives px-at-size, and the prototype's authoring sites are already px
   (no re-audit). **Shipped: PX.** (`text/sync.rs:spaced()`.)
2. **Dark-theme default = framework stays LIGHT; the gallery opts in
   explicitly.** The api-prelude research track recommended making dark the
   framework default. The binding spec § 2 resolution 2 **overrode** that for
   framework neutrality: a general UI framework must not force dark on all apps.
   **Shipped:** `default_light_theme()` is the framework default; the gallery's
   `main.rs` calls `insert_resource(default_dark_theme())` explicitly at boot —
   no env-var magic, no hidden default. (`examples/buiy_gallery/src/main.rs:40`.)

---

## 3. What changed from the prototype (and why) — the deliberate diffs

Everything in § 2 REFINE/REDESIGN is a prototype → final change. The
reviewer-relevant ones, condensed:

| Change | From (prototype) | To (final) | Why |
|---|---|---|---|
| Extract partition | flat query + a mid-flight nested sub-tuple stopgap | `NodePaintQuery` named projection, one system, one damage gate | no arity ceiling; the multi-system split the spec proposed would desync the retain-damage gate (spec-premise correction) |
| `AnimatedBackgroundColor` | opt-in widget special-case | auto-composite in `resolve_background_color` | one check, zero per-widget duplication |
| Headless capture | hand-rolled `capture_shell` plugin list | `BuiyHeadlessPlugin` (first-class subset) | kills the drift from `BuiyPlugin` |
| Composites | gallery-local | the general ones promoted to `buiy_widgets::composites` | reusable, font-neutral |
| Prelude | gradient/icon types reached via `buiy_core::render::components` | promoted to `buiy::prelude` | everyday authoring primitives; discoverability |
| Backdrop damage gate | (the value edge was implicit) | nested `Or<(EffectGroup, Opacity, BackdropFilter)>` term | a runtime blur-radius edit now has a principled re-extract term + the filter tuple stays under the 15-arity ceiling |
| Dark default | framework light + gallery opt-in (ambiguous re: the plan) | framework light + gallery **explicit** opt-in (decided, documented) | framework neutrality; explicit > implicit |
| LetterSpacing | px (shipped, with an em-confusion noted) | px (re-confirmed as the binding contract) | concrete for authors; overrides the research's em proposal |

**Wave-6-specific change (this prep wave).** The audit found the prelude
gradient/icon promotion (spec § 2 REFINE) had been *specified* but not *landed*
in Waves 2/5 — the gallery still imported `BackgroundLayers`/`BackgroundLayer`/
`LinearGradient`/`RadialGradient`/`ColorStop`/`Icon` directly from
`buiy_core::render::components`. Wave 6 closed that gap: the six types are now
re-exported at the `buiy` crate root (→ `buiy::prelude`), and the two gallery
consumers (`shell.rs`, `composites.rs`) were repointed to reach them through the
prelude — exercising the promotion. Pure import-path change; the full gate
re-ran green (§ 4). This is the only code change Wave 6 made; it makes the public
API match its own binding spec.

---

## 4. Verification evidence (the merge gate's proof)

### 4.1 Full workspace gate (CI-equivalent), re-run at the Wave-6 HEAD

| Leg | Command | Result |
|---|---|---|
| Format | `cargo fmt --all -- --check` | clean |
| Lints | `cargo clippy --workspace --all-targets --locked -- -D warnings` | clean (0 warnings) |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` | clean |
| Type-check | `cargo check --workspace --locked` | clean |
| **Headless tests** | `cargo test --workspace --locked` | **1791 passed, 0 failed, 106 ignored** |
| **`buiy_core` GPU lane** | `cargo test -p buiy_core --locked -- --ignored --test-threads=1` | **69 passed, 0 failed** |
| **`buiy_verify` GPU lane** | `cargo test -p buiy_verify --locked -- --ignored --test-threads=1` | **22 passed, 0 failed** |
| Supply-chain | `cargo deny check` | advisories / bans / licenses / sources **ok** |

Total: **1791 headless + 91 GPU = 1882 tests green.** The GPU lanes ran on this
host's RX 6700 XT (real adapter, no display server needed for offscreen
render-to-texture). `cargo deny` emits 3 pre-existing `wildcard` path-dependency
warnings (`buiy`/`buiy_verify`/`buiy_widgets` internal path deps) — documented
harmless (they only matter for crates.io publication, which these workspace
crates are not). The 106 ignored headless tests are precisely the GPU `#[ignore]`
lane that the two GPU commands above run.

### 4.2 Run-the-GUI screenshots (design fidelity)

All 5 screens + a live accent swap were re-captured offscreen at the design
preview size (1280 × 800) on the RX 6700 XT via the `capture_shell` release
binary and compared to the design + the values table:

`docs/reports/parity-final-assets/`: `c-todo.png`, `c-scroll.png`, `c-menu.png`,
`c-modal.png`, `c-showcase.png`, `c-accent-green.png`.

| Screen | Verdict | Confirmed against values.md |
|---|---|---|
| **TodoMVC** (`c-todo`) | design-faithful | gradient logo + wordmark + badge + mono chip + theme chip + github; rail (active accent bar + icon + name + sub); "todos" H1 with tight `-0.025em` tracking + "2 left" badge; round checkboxes (one checked accent + SVG check + strikethrough `#3a4049`); All/Active/Done filter pills; caption; full inspector (Composed-of chips, live state total 3 / remaining 2 / completed 1 / filter all, 4 accent swatches); status bar. Dotted viewport bg present. |
| **Virtual List** (`c-scroll`) | design-faithful | "Entity tree" H1 + "1,000 nodes · windowed" + 240px "Filter nodes…"; sticky table header (INDEX/NODE/FRAME/STATE uppercase mono dim); zero-padded idx, depth indent, type-colored 7×7 dot (per §1.1 palette), ms warn-colored when >1.4ms (row_0004 = 1.50 orange), state OK/WARN; footer "rows 1–11 mounted"; inspector "mounted 11" matches the footer window (the resolved nuance). |
| **Overlay Menu** (`c-menu`) | design-faithful (one cosmetic note) | 420px file card (accent folder tile + "primary_button.bsn" / "crates/buiy_widgets · 1.2 KB" + ⋮); the dropdown OPEN paints all 5 items with icons + labels + the red Delete (the M1 fix — anchored-popover descendant bg + text rasterize); footer blink dot + "last action —"; reworded caption; inspector. Cosmetic note in § 6. |
| **Modal Dialog** (`c-modal`) | design-faithful | scrim dims the shell with `blur(2px)` (top-layer + backdrop-blur); centered 440px create dialog — NAME field, KIND segmented (Button selected = accent fill, the M6 descendant-fill fix), Register switch (ON, accent track), footer Esc kbd + Cancel + Create (accent confirm). The two trigger buttons are pruned behind the scrim. |
| **Controls** (`c-showcase`) | design-faithful | 2-col grid; switch card (3 switches, Wireframe ON accent); slider+radius (14px accent label, 88×88 gradient preview square at radius 14, slider track/fill/thumb); segmented (Compact selected); meter+build (64% accent-gradient fill + Run build); stepper (− / 03 / +); disclosure accordion (Layout&flex expanded with body + props tag, Theme tokens, Accessibility); inspector live state (wireframe on / radius 14px / density compact / count 3 / build 64%). |
| **Accent swap** (`c-accent-green`) | **live re-theme verified** | the Controls screen with `SetAccent(green)` — the logo gradient, rail active bar, switch track, slider label + preview-square gradient + fill + thumb, segmented selection, meter gradient fill, inspector accent values, AND the selected-swatch ring all re-themed to green (`#45c07d → #6ece9a`). Proves `SetAccent → theme.is_changed() → re-extract` re-resolves every accent-bearing paint (gradients, fills, text colors, selection bars). |

Confirmed live by the screenshots: **M1/M6** (menu + modal top-layer descendant
fills paint), **backdrop-blur** (modal scrim), **dotted bg**, **gradients** (logo,
slider preview, meter), **vector icons** (every stroke icon + the SVG check),
**letter-spacing tracking** (tight H1s, spaced section labels), and the **live
accent-swap**.

### 4.3 GPU-golden verification strategy (the decision)

**Decision: the existing dual-path verification is sufficient evidence for the
reviewer + CI. New exact-pixel lavapipe-blessed full-screen goldens are a
documented CI-time follow-up, not landed here.**

The dual path that exists:

1. **Headless layout / display-list snapshots (Tiers 0–2, CI always, no
   adapter).** `shell_skeleton.snap`, the 5 per-screen layout snapshots, the
   router + behavior tests, the content-paints invariants. These pin all
   geometry exactly and are CPU-deterministic — they run in CI on every push
   without a GPU.
2. **Relative-property GPU lanes (Tiers 4–5, lavapipe-portable, CI's GPU
   lane).** `render_gradient_gpu` / `render_icon_gpu` / `render_backdrop_blur_gpu`
   / `render_top_layer_paint_gpu` (+ the text golden suite, caret/decoration
   goldens) assert **adapter-tolerant relative properties**, not exact pixels:
   the gradient corners satisfy `br_red > tl_red + 10` (the ramp runs
   `--ac → --ac2`); backdrop-blur reduces the under-element local variance below
   the sharp-stripe variance but keeps it non-zero (real blur, not a flat fill),
   while a band away keeps full variance (the blur is local); a top-layer
   descendant accent fill satisfies its channel band. Their own doc-comments
   state they pass on **both** the RX 6700 XT and CI's lavapipe.

Why exact-pixel screen goldens are **not** added now:

- The relative-property lanes already prove each capability (gradient / icon /
  blur / top-layer paint) *rasterizes correctly* and **already run on lavapipe in
  CI** — they assert the invariant that matters without coupling to one adapter's
  exact pixels.
- Full-screen 1280×800 goldens are brittle: the journal documents repeated
  golden churn whenever the font-db / atlas / AA changes (the disclosure-caret
  and underline-residue re-blessings). A whole-screen golden re-blesses on any
  such change, for marginal coverage over the targeted relative-property tests.
- They are host-pinned: a blessed pixel image must be blessed on the **exact**
  pinned lavapipe (the RX host here ≠ CI's lavapipe), which needs a lavapipe
  reconstruction. The campaign's reconstruction technique exists (CLAUDE.md GPU
  lane; the widget-catalog campaign used it) if a reviewer wants them, but
  sinking it into this prep wave trades a brittle maintenance burden for little
  net assurance.

So: **the headless layout snapshots + the relative-property GPU lanes (both in
CI) + the 6 audit screenshots are the verification evidence.** Exact-pixel
lavapipe-blessed screen goldens are logged as a CI-time follow-up in § 6.

---

## 5. Resolved framework bugs (the prototype's highest-value output, carried in)

All found by **running the GUI** — headless gates never exercised them — and
root-caused (no hacks, no disabled tests):

1. **LetterSpacing** mis-lowered as em (cosmic) not px → `px / font_size` lowering
   in `text/sync.rs:spaced()` (the on-screen advance is exactly the authored px
   at any size). Shipped PX per the binding spec (§ 2).
2. **M1 + M6** — top-layer (modal) + anchor-override (menu) descendant
   `Background` fills (+ text glyphs for the override case) did not rasterize.
   Root cause: `StackingContext` had no cross-root rank for repositioned /
   top-layer subtrees. Fix: `cross_root_rank` threaded through layout + render +
   text + picking. Verified live (the menu dropdown + the modal create-confirm /
   selected-segment / switch all paint — see § 4.2 `c-menu` / `c-modal`).
3. **M5 + the viewport-header "WARN" artifact** — `Display::None` /
   zero-size nodes still painted glyphs (a runtime-flip `Display::None` subtree
   kept a collapsed `ResolvedLayout`, and the flat GPU extract painted it; a
   zero-area `Icon` still drew its native-size glyph). Fix:
   `SkipReason::DisplayNone` in `write_paint_skip` (subtree-roots the skip) +
   `icon_paints` skips a zero-area box.
4. **`shadow.card` / `shadow.menu` / `shadow.modal` / `shadow.danger`** tokens
   undefined (magenta sentinel) → added to `default_dark_theme()` per values.md
   § 2.
5. **Checkbox `✓` tofu** (the embedded faces lack U+2713) → the round checkbox
   uses an SVG-check `Icon` whose opacity toggles (no `CheckboxMark` child, so no
   `.notdef` glyph). **`⌘` tofu** (U+2318) → rendered as the Lucide `command`
   vector `Icon` via `kbd_content` (the registered-only font system carries no
   U+2318); `⋮` / `∅` in captions reworded to glyphs the faces carry.

---

## 6. Residual gaps + follow-ups (for the reviewer)

None of these block the parity verdict; they are the honest remaining edges.

- **Menu open-state focus ring (cosmetic, widget-owned).** The `c-menu` capture
  shows the WCAG `:focus-visible` ring around the *whole dropdown panel*. Root
  cause (not a parity regression): `buiy_widgets::menu::sync_menu_open`
  implements the `aria-activedescendant` pattern — on open it moves DOM focus to
  the **menu container** (not the first item) and sets `focus_visible = true`, so
  the ring draws on the container. The design instead highlights the *active
  item*. This is correct, accessible, **pre-existing `buiy_widgets` behavior**
  (the landed widget-catalog campaign owns it), surfaced by the forced-open
  capture. Follow-up (buiy_widgets, not parity): ring the active descendant
  rather than the container, or suppress the container ring under pointer-origin
  opens.
- **Exact-pixel lavapipe screen goldens (CI-time follow-up).** Per § 4.3 — the
  relative-property GPU lanes + headless snapshots are the chosen evidence; a
  small set of blessed full-screen goldens (logo gradient, dotted bg, icons,
  blur, a caret) can be added later via the campaign's lavapipe-reconstruction
  technique if a reviewer wants pixel-exact screen regression coverage.
- **Headless test count 1791 vs the spec's "≥1800" soft target.** The suite is
  comprehensive and fully green; the difference is test organization /
  consolidation, not missing coverage (the prototype's 1774 grew to 1791 plus the
  91 GPU lane). Reported honestly rather than padded.
- **Carried-over prototype v1 ceilings** (unchanged, documented, accepted for
  this phase): paint-skip virtualization is not DOM-style row recycling (all 1000
  rows resident; "mounted" reports the windowed count to match the design + the
  footer); `PopoverAlign::End` is unimplemented (the dropdown left-aligns to the
  ⋮ button, which sits at the card right, so the visual is correct); the showcase
  grid is ~752 px at the 1280 viewport (the design's 880 is a responsive nominal
  target — spec § 2 KEEP); entrance-animation durations (menu/modal/toast) are
  the documented invented values (values.md § 5 has none inline); animations are
  single-frame-uncapturable (verified by tween unit tests, not goldens).
- **Spec prose vs the runtime `Display::None` edge.** The render-pipeline spec
  `paint-order-and-top-layer.md` § 5.1 still phrases `Display::None` as "never
  reaches extract" in the absolute; that holds for born-`None` entities, while
  the runtime-flip edge is now handled in `write_paint_skip` (the in-code
  `SkipReason` doc is updated). A spec-prose reconciliation is a render-subsystem
  doc follow-up, not a parity blocker.

---

## 7. The human-review gate

This branch is **merge-gated on human review** (spec § 1, § 4 non-goals — no
self-merge). The team-lead opens the PR; this document + the PR body + the 6
screenshots + the gate results in § 4 are the reviewer's evidence package. The
suggested spot-checks: the `NodePaintQuery` projection + its spec-premise
correction (§ 2 REDESIGN), the prelude surface (§ 3), the two binding overrides
(LetterSpacing px + light-default, § 2), and the 6 screenshots against
values.md. On approval: squash-merge to `main`.
