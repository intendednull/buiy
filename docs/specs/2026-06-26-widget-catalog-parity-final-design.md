# Widget Catalog — Exact Visual Parity, FINAL (Phase 3) Design

**Status:** Phase 3 target-state. The **final, mergeable** version. Built on
`parity-final` (off `main` @ `fdb8dda`). **Merge-gated on HUMAN REVIEW.**
**Inputs:** the prototype (phase 2, `parity-prototype`, never merged) + its
journal/retrospective; the re-decided architecture in
`docs/reports/2026-06-26-parity-final-research-decisions.md` (7-track fleet); the
design `docs/reference-designs/widget-catalog/Widget Catalog.dc.html` + values
table `docs/specs/2026-06-25-widget-catalog-values.md`.

## 1. Goal & strategy

Ship the exact-parity Widget Catalog as a **production, reviewed** change: the
framework capabilities + the unified gallery shell + all 5 design-faithful
screens + live accent theming, at mergeable quality.

**Strategy — HYBRID (cherry-pick the validated, re-architect the pressure
points).** The prototype proved exact parity is achievable end-to-end (1774
headless + 91 GPU green, all 5 screens screenshot-verified) and the phase-3
research re-evaluated **every** decision with the full picture. Verdict: the core
technical choices are sound (keep), a few are production cleanups (refine), and
three are genuine redesigns. Re-deriving sound, GPU-proven code from scratch is
*not* best practice; re-deciding it **is**. Since `parity-final` and
`parity-prototype` share the base `fdb8dda`, the validated prototype commits are
cherry-pickable; we cherry-pick the KEEP work and land the REFINE/REDESIGN work
as deliberate phase-3 changes. The PR narrates "ported the validated prototype +
these re-decided refinements" for review.

## 2. Architecture decisions (KEEP / REFINE / REDESIGN)

Full rationale per subsystem in the research-decisions report; the binding set:

### KEEP (port the validated prototype work as-is)
- **Render data model:** `BackgroundLayers` *sibling* component (source-compat w/
  103 callers + bsn!); distinct 104 B `GradientInstance` (keeps `PackedInstance`
  byte-stable); lyon→R8 glyph-alpha **vector icons** (per-instance accent tint).
- **Render correctness fixes:** `cross_root_rank` stacking (M1+M6 top-layer/
  anchor-override descendant paint); `SkipReason::DisplayNone` paint-skip (M5 +
  the header artifact).
- **Theme:** the ~33-token dark palette + shadow tokens + `derive_accent_ramp` +
  the `SetAccent` runtime swap (mutate `Theme` → `is_changed()` re-extract).
  **Token-only — NO `ColorToken::Literal`** (gate-#11 forced-colors discipline).
- **Animation:** the `Tween<T>` + `Easing::CubicBezier`/`DESIGN` + `Repeat`
  (PingPong/Loop) per-property tween model; reduced-motion = snap-to-rest.
  (Reject `bevy_animation`, confirmed.)
- **Text/fonts:** Geist + Geist Mono **variable** faces; monospace re-pinned to
  Geist Mono; entity-level decorations (the design has no inline-emphasis).
- **Shell:** `ScreenRouter` via **`Display::None` toggle** of all-5-spawned (zero
  hidden-screen layout cost; state-isolation free); **global resources + per-screen
  markers** (no per-screen scoping needed). Imperative `spawn_*_screen` builders.
- **Virtualization:** paint-skip (`ContentVisibility::Auto`, all 1000 spawned) —
  not DOM windowing. The inspector "mounted = resident entities" vs the footer
  "visible window" are *both correct* (standardize the wording, don't change the
  model).
- **Viewport widths:** rail 248 / inspector 280; the showcase grid is responsive
  (~752 px at 1280) — the design's 880 is a nominal target, not a hard width.

### REDESIGN (do it the production way, not the prototype's mid-flight patch)
- **Extract-query arity → nested logical sub-systems FROM THE START.** Partition
  `extract_buiy_nodes` into `extract_buiy_base (Node, ResolvedLayout, transform,
  clip)` + `…_colors (Background/Border/TextColor)` + `…_effects (BackdropFilter/
  EffectGroup)` + `…_gradients (BackgroundLayers)` + `…_icons (Icon)`, each a
  producer mutating the shared `ExtractedNodes` map. Never bump the 15-tuple
  `QueryData` bound again. (Mirrors the existing glyph/shadow producer pattern.)
- **Headless harness → a public `BuiyPlugin::headless()` / `BuiyHeadlessPlugin`**
  (theme/layout/core/text/focus/a11y/widgets/render; no picking/winit) replacing
  `capture_shell`'s hand-rolled, drift-prone plugin list.
- **Verification → CI goldens blessed on pinned LAVAPIPE** (not the RX dev host).
  Dual-path: Tier 0-2 headless layout/display-list snapshots run in CI *always*;
  a small set of GPU goldens (logo gradient, dotted bg, icons, blur, a caret)
  blessed on the reconstructed pinned lavapipe (per the campaign's technique +
  CLAUDE.md GPU lane). Institutionalize **run-the-GUI** pre-commit.

### REFINE (production cleanups)
- **`AnimatedBackgroundColor` → auto-composite** in the render extract's
  `resolve_background_color` (a one-line check), not an opt-in widget special-case.
- **`buiy::prelude` + promotions:** add a `prelude` module; promote
  `BackgroundLayers/BackgroundLayer/LinearGradient/RadialGradient/ColorStop/Icon/
  LetterSpacing/SetAccent/Tween/Easing/Repeat` (everyday authoring primitives).
- **Composites → promote the genuinely-general ones to `buiy_widgets`** (`meter`,
  `table_row`/`table_header`, `search_input`, `kbd`, `status_dot`, `pulse_blink`);
  keep screen-specific compositions gallery-local.

### Resolved inter-track disagreements (this spec is binding)
1. **LetterSpacing contract = PX (keep the prototype's fixed `px/font_size`
   lowering).** Rejecting the research's em-direct alternative: px is concrete +
   intuitive for authors, the values table already gives px-at-size, and the
   prototype's authoring sites are px (no re-audit). The one division is worth the
   DX. Document the contract clearly.
2. **Dark-theme default = framework stays LIGHT; the gallery inserts
   `default_dark_theme()` explicitly at boot.** A general UI framework must not
   force dark on all apps; explicit > implicit. (= the prototype's approach; the
   research's "dark framework default" is rejected for framework neutrality.)

## 3. Verification (the merge gate's evidence)
- Workspace gate (CLAUDE.md): `fmt --check` · `clippy --workspace --all-targets
  -D warnings` · `doc -D warnings` · `test --workspace` (+ the lavapipe GPU lanes).
  Target: ≥1800 headless + the buiy_core + buiy_verify GPU lanes green.
- **CI goldens on lavapipe** (blessed locally on the reconstructed pinned adapter).
- **Run `cargo run -p buiy_gallery`** + screenshot all 5 screens + the accent-swap;
  compare to the design/values table.
- The PR carries: the prototype→final narrative, the resolved-decisions doc, the
  5 screenshots, and the full gate results — for the human reviewer.

## 4. Non-goals (this phase)
Self-merge (human-review-gated); true DOM windowing; a typed token-scale system
(C-tier); per-keyframe easing; a CSS cascade. Animated-gradient stops.

## 5. Build sequence → see the plan
`docs/plans/2026-06-26-widget-catalog-parity-final.md` — 6 waves: (1) API/prelude
+ extract-query partition design; (2) port A (theme+anim+fonts) + refinements;
(3) port B (gradients/icons/blur/cross_root_rank) + the extract refactor; (4) port
C1 shell + 5 screens; (5) port C4 inspector + promote composites + headless
plugin; (6) parity audit + lavapipe goldens + full gate + human-review prep.
