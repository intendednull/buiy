# Buiy — UI library foundation design

**Date:** 2026-05-07
**Status:** draft

## Purpose

Define the target shape of Buiy: a comprehensive UI library for the Bevy game engine, covering the modern web platform's UI feature set with full WCAG 2.2 AA accessibility, for both game and app UIs.

This spec is a **feature inventory and architectural foundation**, not an implementation plan. Each subsystem (layout, text, theming, a11y, widgets, etc.) graduates to its own design spec later. Implementation phasing lives in `docs/plans/`, not here.

The spec was written during a brainstorming session that consumed three deep-research reports (Bevy UI ecosystem, web platform feature inventory, accessibility deep-dive). Those reports are the substrate for the catalog in Section 3 and inform the architectural decisions in Section 2.

## 1. Goals and non-goals

### Buiy's goals (the product)

1. **Comprehensive.** Feature parity with the modern web UI platform: HTML semantics, CSS layout / styling / animation surface area, ARIA roles and states, WAI-ARIA APG behavioral patterns, WCAG 2.2 success criteria, complex text (IME, BiDi, RTL, complex script shaping, emoji), the form-control set, drag-and-drop, clipboard, live regions. The web platform feature catalog produced during research is the master list we cull from, not an aspiration. Future web features (anchor positioning, container queries, view transitions, scroll-driven animations) are absorbable, not blocking.

2. **Accessible.** WCAG 2.2 AA is the floor. Every interactive widget ships with its APG keyboard contract, accessible name/role/value, focus management, AccessKit tree wiring. Forced-colors, reduced-motion, prefers-contrast, prefers-color-scheme are honored automatically from OS preferences.

3. **BSN-native.** Every Buiy component is small, public-fielded, observable, and decomposed by concern. No megacomponents, no private setters. BSN authoring works against Buiy components without adapter layers (the lesson of [bevy issue #17644](https://github.com/bevyengine/bevy/issues/17644)).

4. **Parallel to bevy_ui.** Buiy is a parallel UI stack — it integrates the same underlying primitives that bevy_ui uses (Taffy, cosmic-text, AccessKit, bevy_picking, Bevy's render graph) directly, with its own component model and render pipeline. The decision to go parallel rather than build on top of bevy_ui follows from the comprehensive-feature-parity goal: bevy_ui's renderer caps several capabilities (non-rect clipping, backdrop-filter, mix-blend-mode, isolation, true top layer) that web parity requires.

5. **Tracks Bevy.** Rolling latest-stable. No multi-version compatibility promise. Each Bevy minor release is a migration event for Buiy users.

6. **Game and app, both.** Buiy is the UI layer for anything built on Bevy. Productivity-app concerns (IME, complex text, screen readers, complex forms) and game concerns (gamepad nav, in-world UI anchoring, animation polish) are both in scope.

7. **Verifiable.** Every machine-testable claim Buiy makes (every widget behavior with a defined keyboard contract, every AccessKit tree shape, every theme variant's visual output, every layout primitive's resolved geometry, every machine-testable WCAG SC) is covered by automated tests that run in CI without a human approval gate. Claims that depend on human judgment, real OS subsystems, or physical devices (real-SR utterance verification, full OS-IME conformance, real-device mobile coverage, content-quality SCs, subjective visual quality) are documented as **manual release gates** with explicit owners, cadence, and release-blocking sign-off documents — not CI gates. Section 3.15 enumerates which tests live in CI vs at the manual release gate. Tradeoff acknowledgment: several user-experience claims (what an AT actually says, what real devices do, content quality, polish) sit at the manual tier; "fully automated verification pipeline" describes the CI tier specifically.

### Non-goals

- **Networking, persistence, routing/URL navigation, file system access, service workers, sandboxing.** UI is a presentation layer; data and transport are the consuming app's concern.
- **Game-side accessibility content** — audio description of gameplay, difficulty options, narrative aids, content warnings. Buiy provides the *UI primitives* (live regions, caption containers, settings widgets, remap UI); the game owns the substance.
- **A reactive component model with signals/computed/effects in v1.** Bevy's observers + change detection are the reactivity primitive. A signal-style layer is a follow-up sub-spec, not part of foundation.
- **Compatibility across Bevy minor versions.** Each Bevy minor release is a migration event.
- **Non-Bevy frontends.** No web target via WASM-without-Bevy, no SSR.
- **Replacing bevy_ui upstream.** Buiy stands parallel; bevy_ui and Buiy can both run in the same app (different trees).
- **Mixing Buiy and bevy_ui in the same UI tree.** Within one tree, you pick one or the other.

### What this spec does

- Defines the architectural foundation: parallel to bevy_ui, BSN-friendly components, ECS + BSN authoring, token-based theming, AccessKit-first.
- Catalogs every feature/component by category, each tagged with a tier: **F** = foundation (without it nothing else works), **C** = core (any non-trivial UI needs it), **E** = extended (commonly needed but cuttable for a long time), **O** = out (explicitly excluded, with reason).
- Lists the subsystems that will receive their own design specs (Section 4).
- Records open questions for later resolution (Section 5).

### What this spec does NOT do

- Specify APIs in detail. Per-subsystem specs do that.
- Pick release phases or a timeline. Plans do that.
- Specify a single canonical UI style or design language. The default theme passes WCAG 2.2 AA; visual style is a theme concern.

## 2. Architectural foundation

### 2.1 One-line summary

Buiy is a parallel UI stack to bevy_ui, integrating the same underlying primitives (Taffy, cosmic-text, AccessKit, bevy_picking, Bevy's render graph) directly, with its own component model and its own render pipeline.

### 2.2 Underlying primitives Buiy integrates directly

- **[Taffy](https://github.com/DioxusLabs/taffy)** — Flexbox, CSS Grid, Block layout. We feed it our component data. As Taffy adds subgrid, container queries, etc., we get them.
- **[cosmic-text](https://github.com/pop-os/cosmic-text)** — text shaping, BiDi (UAX #9), font fallback, color emoji, RTL. Used directly for both rendering and editing.
- **[AccessKit](https://accesskit.dev)** — accessibility tree + cross-platform AT bridge. We build trees and push `TreeUpdate`s ourselves with our own decomposed components.
- **[bevy_picking](https://docs.rs/bevy_picking)** — hit-testing primitive. We feed our hierarchies into it.
- **Bevy's render graph + wgpu** — our render passes live in Bevy's render graph. Custom shaders for clipping, gradients, borders, filters, blend modes, top layer.
- **Bevy's ECS, observers, change detection, asset system, input, windowing.** Used throughout. Buiy is a Bevy plugin, not a separate framework.

### 2.3 What Buiy owns

- **Component model** — Buiy components (`buiy::Node`, `buiy::Style`, `buiy::Theme`, focus components, a11y components, animation components). Designed BSN-friendly (small, public-fielded, observable, decomposed). Not derived from `bevy_ui::Node`.
- **Render pipeline** — custom Bevy render passes that walk Buiy hierarchies. Full control over rounded clipping, `clip-path` shapes, mask-image, backdrop-filter, mix-blend-mode, isolation/groups, true top-layer compositing, gradients in any color space, border-image, drop-shadow.
- **Layout integration** — drives Taffy ourselves; extends layout (anchor positioning, container queries) without waiting for upstream.
- **Text pipeline** — cosmic-text → glyph atlas → render pass, owned end-to-end. No per-span fonts, no inheritance leaks, no atlas leaks.
- **Focus model** — focus tree, `:focus-visible` semantics, focus rings, focus traps, focus restoration, inert subtrees, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point, spatial gamepad navigation.
- **A11y integration** — Buiy → AccessKit directly. Decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` components drive `TreeUpdate`s. ACCNAME 1.2 name computation lives in `buiy_core`.
- **Theme system** — token assets, hot-reloadable, OS-pref-driven variant binding.
- **Animation primitives** — property transitions, keyframe timelines, layout transitions, springs, all reduced-motion-gated.
- **Live regions / global announcer** — Buiy resource that renders polite/assertive announcements through AccessKit.
- **Form & validation system** — form state machine, constraint validation, validation pseudo-classes.
- **Devtools** — inspector, layout overlay, AccessKit tree viewer, contrast linter, focus-order visualizer.
- **3D-anchored / diegetic UI** — first-class achievable; Buiy nodes can live in 3D space against `Transform`. Stays its own subsystem spec.
- **Verification harness** — test infrastructure for visual regression, AccessKit tree snapshots, synthesized input replay, APG conformance, WCAG SC verification.

### 2.4 Authoring: ECS-native and BSN, both first-class

- **ECS spawn:** `commands.spawn((buiy::Button, OnPress(submit), children![buiy::Text::new("Save")]))`. Always works.
- **BSN** (Bevy 0.18+): `bsn! { Button [ Text("Save") ] }` or hot-reloadable `.bsn` files.

The BSN-friendliness constraint on every Buiy component is **not optional**:

- Small, public-fielded, observable, decomposed by concern. No megacomponents, no private setters.
- Every component derives `Reflect + FromReflect + Default + Clone + Component`.
- Every component is type-registered via `app.register_type::<T>()` in the owning crate's plugin so `.bsn` asset loading can resolve it.

These constraints follow from BSN's reflection-driven asset format (PR #20158) and from the lesson of bevy issue #17644 (megacomponents are BSN-hostile).

### 2.5 Theming: token-based design system

- Themes are **assets**, hot-reloadable.
- Components consume **semantic tokens** (`color.surface.primary`, `space.4`, `radius.md`, `motion.fast`), never raw values.
- A theme defines a **palette + scales + variant**. Variants: `light`, `dark`, `high-contrast`, plus user-defined.
- OS preferences (`prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency`, `inverted-colors`) surface as a `UserPreferences` resource bound to theme variants automatically.
- Default theme passes WCAG 2.2 AA contrast (4.5:1 / 3:1 / 3:1) by construction. Contrast linter validates custom themes at load and in CI.
- A subtree can override its theme by carrying a `Theme` component.
- **CSS-flavored stylesheet is not in this spec.** Future sub-spec if needed.

### 2.6 Accessibility: AccessKit-first

- The AccessKit tree is the source of truth for accessibility. Built lazily (gated on `AccessibilityRequested`), pushed as `TreeUpdate` diffs.
- Stable `NodeId`s derived from Bevy `Entity`.
- ACCNAME 1.2 name computation lives in `buiy_core`.
- Each widget's APG keyboard contract is part of the widget's contract.

**Adapter ownership.** AccessKit allows exactly one tree per `accesskit_winit::Adapter` per window. Buiy owns the adapter handle on any window where Buiy is present, keyed by winit `WindowId` (not Bevy `Entity`). Buiy does *not* layer over `bevy_a11y` — it replaces `bevy_a11y` for windows it owns. `ActionRequest` events from the adapter are routed to Buiy entities via Buiy's own action plumbing, not bevy_a11y's. See Section 3.18 for coexistence rules with bevy_ui.

### 2.7 Reactivity

Observers + change detection only. No signal/computed/effect layer in this spec.

### 2.8 Module organization

Buiy ships as a workspace of focused crates. The principle is **modular subsystems with clean boundaries, opt-in surface area**. The exact partition below is an indicative starting point; final partition is an open question (Section 5):

- `buiy` — meta-crate, re-exports common API, ships top-level `BuiyPlugin`.
- `buiy_core` — components, render pipeline, layout integration, focus model, theme tokens, a11y primitives, plugin scaffolding (may split further per Section 5 open question).
- `buiy_text` — rich text + IME-correct text editing on cosmic-text.
- `buiy_widgets` — APG widget catalog.
- `buiy_animation` — transitions, keyframes, layout transitions, reduced-motion gating.
- `buiy_forms` — form state machine, validation, constraint pseudo-classes.
- `buiy_devtools` — inspector, contrast linter, focus visualizer, AccessKit tree viewer.
- `buiy_3d` — 3D-anchored / diegetic UI.
- `buiy_bsn` — BSN authoring helpers when on Bevy 0.18+.
- `buiy_verify` — verification harness; consumed as `dev-dependency` by every other crate; usable by downstream Buiy users.

**`BuiyPlugin` sub-plugin order.** The top-level plugin adds sub-plugins in this order so dependents see their dependencies on construction: `core` → `theme` → `a11y` → `focus` → `input` → `text` → `widgets` → `animation` → `forms` → `devtools`. Render registration happens in `Plugin::finish` (after `RenderApp` exists).

**System-set partitioning.** Per-frame Buiy work is partitioned into named `SystemSet`s, ordered:

```
BuiySet::Layout → BuiySet::Style → BuiySet::Input → BuiySet::Animate
              → BuiySet::Picking → BuiySet::A11yUpdate → BuiySet::Render
```

Sub-specs hang their systems off these labels. UI animations advance in the `Update` schedule against `Time<Virtual>` (not `FixedUpdate`, which is for game logic). Render-app data is extracted via Bevy's standard `ExtractSchedule` from the main world after `BuiySet::Render` completes.

### 2.9 Compatibility & policy

- **Rolling latest-stable Bevy.** Bevy minor releases drive migration events for underlying primitives. wgpu is a version-pinned dependency of Bevy (Bevy re-exports many wgpu types but the wgpu crate is owned upstream); we follow Bevy's pin. AccessKit releases on its own cadence and is **the open question** of Section 5: the policy proposed here is "AccessKit major release between Bevy minors triggers a Buiy patch release with a documented migration note," but this is not yet committed. No back-compat across Bevy minors.
- **MSRV** tracks Bevy's MSRV.
- **`std` only.** AccessKit requires it.
- **Platform support — staged.** Desktop (Windows / macOS / Linux) is committed for v1 with full CI coverage. Android (TalkBack), iOS (UIAccessibility — currently in-progress upstream in AccessKit), and web (AccessKit web adapter — not yet shipped) are deferred until each platform's AccessKit adapter exposes a headless harness usable in CI; until then they live as manual-release-gate platforms.
- **Render passes & picking** — Buiy registers its own render-graph node and its own `bevy_picking` backend. Render-graph node ordering and picking-backend priority versus bevy_ui's own passes / backend are defined per-window (see Section 3.18); Buiy's own passes do not contractually cooperate with bevy_ui's.
- **Coexistence with bevy_ui** — see Section 3.18. Coexistence is **per-window**, not per-app-shared-window.

## 3. Feature inventory

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason).

A small number of WCAG-tied items in Section 3.11 carry **dual tiers** of the form `F (AA) / C (AAA)`, where the conformance level (AA vs AAA) and the Buiy implementation tier (foundation vs core) differ. The convention applies only in Section 3.11 for SCs that exist at multiple WCAG conformance levels; everywhere else tiers are single-valued.

### 3.1 Document model and component hierarchy

- Element/node tree (entity hierarchy with parent/child/sibling). **F**
- Document order (canonical traversal; default tab order; AccessKit tree order). **F**
- Range and selection model (text ranges spanning nodes, multi-range, programmatic). **C**
- Mutation primitives (Bevy observers + change detection; widget-level `OnMutate` / `OnInsert`). **C**
- Tree walker / filtered traversal (skip inert / hidden / disabled). **C**
- Semantic role categories: landmarks, document structure, sectioning, phrasing, edit annotations, embedded content, tabular, interactive — every category has a Buiy primitive. **F**
- Replaced elements (image, video, canvas, embedded surfaces) — different sizing rules, intrinsic dimensions, `object-fit`. **C**
- Component encapsulation (subtree boundaries via marker components — Shadow-DOM analogue). **E**
- User-defined components, same primitives as built-ins. **F**
- Slot / projection (Bevy's `children!` macro). **C**
- Focus tree — first-class subsystem; ordered, filterable, partitionable. **F**
- `tabindex` analogue (`Auto`, `Skip`, `Order(i32)`). **F**
- `inert` subtree (removes from focus + AccessKit + hit-testing). **F**
- **Out:** HTML parser, document streaming, `<head>` metadata, document.title beyond window title, `<base>`. **O**

### 3.2 Layout

Powered by Taffy directly. Buiy extends Taffy where Taffy doesn't yet cover something needed.

**Box model**
- Content / padding / border / margin boxes; `box-sizing` analogue. **F**
- Margin collapse (block flow). **C**
- `min/max/width/height` with `min-content` / `max-content` / `fit-content` / `auto` / `stretch`. **C**
- `aspect-ratio`. **C**
- Logical properties (`inline-size`, `block-size`, `padding-inline-*`, `margin-block-*`, `border-inline-*`). **F**

**Display modes**
- Block, inline, inline-block. **F**
- Flex / inline-flex (full Flexbox). **F**
- Grid / inline-grid (full CSS Grid). **F**
- `flow-root`. **C**
- `contents`. **E**
- `none`. **F**
- Table, table-row-group, table-row, table-cell, table-caption (semantic table layout for data). **C**
- `list-item`. **E**
- Ruby (CJK furigana). **E**

**Positioning**
- `static`, `relative`, `absolute`, `fixed`, `sticky`. **F**
- Containing-block resolution. **F**
- `inset` shorthand + logical `inset-block-*` / `inset-inline-*`. **F**
- Anchor positioning (`anchor-name`, `position-anchor`, `anchor()`, `anchor-size()`, `position-try`, `@position-try`). **C**

**Flexbox** — full spec via Taffy. **F**

**Grid** — full CSS Grid via Taffy. **F**
- Subgrid — Taffy roadmap; we ship when Taffy ships. **C**
- Masonry — flux. **E**

**Multi-column** (`column-count`, `column-width`, `column-gap`, `column-rule`, `column-span`, `break-*`). **E**

**Float / clear** — legacy. **O**
- `shape-outside`, `shape-margin`. **O**

**Container queries** (`@container`, `container-type`, `container-name`, `cqw/cqh/cqi/cqb` units). **C**

**Scroll-driven animations** (`animation-timeline`, `scroll-timeline`, `view-timeline`). **E**

**Writing modes & direction**
- `writing-mode`: horizontal-tb, vertical-rl, vertical-lr, sideways-rl, sideways-lr. **C**
- `direction`: ltr / rtl. **F**
- `text-orientation`. **E**
- `unicode-bidi`. **C**

**Overflow & scrolling**
- `overflow: visible | hidden | clip | scroll | auto`, axis variants, logical (`overflow-block` / `overflow-inline`). **F**
- `scroll-behavior: smooth | auto`. **C**
- `overscroll-behavior`. **C**
- Scroll snap (`scroll-snap-type`, `scroll-snap-align`, `scroll-snap-stop`, `scroll-padding`, `scroll-margin`). **C**
- `scrollbar-gutter`, `scrollbar-color`, `scrollbar-width`. **C**

**Stacking & paint**
- Stacking contexts (positioned + z-index, opacity < 1, transform, filter, will-change, isolation, mix-blend-mode). **F**
- `z-index`. **F**
- `isolation`. **C**
- True top layer for modals / popovers / dialogs / fullscreen. **F**

**Units**
- `px`. **F**
- Print units (`cm`, `mm`, `in`, `pt`, `pc`, `Q`). **O**
- Font-relative (`em`, `rem`). **F**; (`ch`, `lh`, `rlh`, `cap`, `ic`, `ex`). **C**
- Viewport (`vw`, `vh`, `vmin`, `vmax`, plus small / large / dynamic variants). **C**
- Container (`cqw`, `cqh`, `cqi`, `cqb`, `cqmin`, `cqmax`). **C**
- Percentages. **F**
- `fr` (grid). **F**
- Angles (`deg`, `rad`, `grad`, `turn`). **C**
- Time (`s`, `ms`). **C**
- Frequency (`Hz`, `kHz`). **E**
- Resolution (`dpi`, `dppx`). **C**

**Transforms & containment**
- `transform`, `transform-origin`, 2D + 3D, `transform-style`, `perspective`, `backface-visibility`. **C**
- Standalone `translate` / `rotate` / `scale`. **C**
- `will-change`. **E**
- `contain` (layout / paint / size / style / inline-size / content / strict). **C**
- `content-visibility` (visible / auto / hidden) for off-screen lazy rendering. **C**

### 3.3 Visual styling and rendering

**Color**
- Named colors, `transparent`, `currentColor`. **F**
- `rgb()` / `rgba()` / `hsl()` / `hsla()` / `hwb()`. **F**
- `lab()`, `lch()`, `oklab()`, `oklch()`. **C**
- `color()` with profiles (sRGB linear, display-p3, rec2020, a98-rgb, prophoto-rgb, xyz). **C**
- `color-mix(in <space>, c1 p1, c2 p2)`. **C**
- Relative color syntax. **E**
- System color keywords (`Canvas`, `CanvasText`, `LinkText`, `ButtonText`, `ButtonBorder`, `GrayText`, `Highlight`, `HighlightText`, `Field`, `FieldText`, `Mark`, `MarkText`, `SelectedItem`, `SelectedItemText`, `AccentColor`, `AccentColorText`) for forced-colors. **F**
- `color-scheme` property — opt a subtree into light / dark / both for native widget rendering hints (distinct from the `prefers-color-scheme` media query). **C**
- `forced-color-adjust` — per-element opt-out of forced-colors mode (`auto` / `none` / `preserve-parent-color`). **C**

**Backgrounds**
- `background-color`. **F**
- `background-image` (URL + gradients). **C**
- Gradients: linear, radial, conic, repeating variants; color stops, hints, color-space interpolation. **C**
- Multiple background layers. **C**
- `background-position` / `-size` / `-repeat` / `-attachment` / `-origin` / `-clip` (incl. `text`) / `-blend-mode`. **C**
- `image-set()`. **E**

**Borders**
- `border-width` / `-style` / `-color`, longhands per side, logical. **F**
- `border-radius` incl. elliptical per-corner. **F**
- `border-image`. **E**

**Shadows, filters, effects** (full set achievable because we own the renderer)
- `box-shadow`, multiple, inset, spread. **F**
- `text-shadow`. **C**
- `filter`: blur, brightness, contrast, drop-shadow, grayscale, hue-rotate, invert, opacity, saturate, sepia. **C**
- `backdrop-filter`. **C**
- `mix-blend-mode` + `background-blend-mode`, full blend mode set. **C**
- `isolation`. **C**
- `clip-path`: `inset()`, `circle()`, `ellipse()`, `polygon()`, `path()`, `xywh()`, `rect()`, geometry-box keywords. **C**
- `mask` family (`mask-image`, `-mode`, `-position`, `-size`, `-repeat`, `-origin`, `-clip`, `-composite`). **C**
- `opacity`. **F**
- `visibility: visible | hidden | collapse`. **F**

**Outline & focus indicators**
- `outline-color` / `-style` / `-width` / `-offset`. **F**
- `accent-color`. **C**
- `caret-color`. **C**

**Cursor & pointer behavior**
- `cursor` — full keyword set. **C**
- Custom cursor (image + hotspot, fallback). **E**
- `pointer-events` (`auto` / `none`). **F**
- `user-select` (auto / none / text / all / contain). **F**
- `touch-action`. **C**

**Custom properties + value functions**
- Buiy tokens are the canonical "CSS variables." Typed and themable. **F**
- `calc()` / `min()` / `max()` / `clamp()` analogues for sizes & numbers. **F**
- Math (`mod()`, `rem()`, `round()`, `abs()`, `sign()`, `pow()`, `sqrt()`, trig). **C**
- `env()` analogue (UA values: safe-area, system-color slots, OS prefs). **C**
- Typed custom props. **E**

**Render pipeline features (Buiy-owned)**
- Per-element clipping with arbitrary shapes. **F**
- Per-element filters and blend modes composed correctly with parent stacking context. **C**
- Backdrop sampling for `backdrop-filter`. **C**
- Top-layer compositing. **F**
- Texture atlases for glyphs, icons, gradients, generated masks. **F**
- Render-to-texture surfaces (UI as a texture for in-world / 3D-anchored use). **C**
- HDR rendering pass when display supports it. **E**
- Color management (linear-light render, sRGB / display-p3 / rec2020 output when display advertises). **C**

### 3.4 Typography

**Font selection**
- `font-family` with stack + fallback. **F**
- Generic families (`serif`, `sans-serif`, `monospace`, `cursive`, `fantasy`, `system-ui`, `ui-serif`, `ui-sans-serif`, `ui-monospace`, `ui-rounded`, `emoji`, `math`). **C**
- `font-size` incl. keyword sizes. **F**
- `font-weight`. **F**
- `font-style: normal | italic | oblique <angle>`. **C**
- `font-stretch` / `font-width`. **C**
- `font-variant-*` (caps, numeric, ligatures, east-asian, alternates, position, emoji). **C**
- `font-feature-settings` (raw OpenType). **C**
- `font-variation-settings` (variable-font axes). **C**
- `font-optical-sizing`. **E**
- `font-kerning`. **C**
- `font-synthesis`. **C**
- `font-language-override`. **E**
- `font-size-adjust`. **E**
- `font-palette` + `@font-palette-values`. **E**

**Variable fonts** — single file, registered axes, smooth interpolation, custom axes. **C**

**Font registration** — Bevy asset-pipeline equivalent of `@font-face`: source, format, unicode-range, font-display strategy. **F**
- Metric overrides (`size-adjust`, `ascent-override`, `descent-override`, `line-gap-override`) for synthetic-fallback metric matching. **C**

**Inline text layout**
- `line-height`. **F**
- `letter-spacing`, `word-spacing`. **C**
- `text-align: start | end | left | right | center | justify | justify-all | match-parent`. **F**
- `text-align-last`. **C**
- `text-justify`. **E**
- `text-indent`. **C**
- `vertical-align`. **C**
- `tab-size`. **C**

**Wrapping & breaking**
- `white-space` (incl. longhand `white-space-collapse` + `text-wrap: wrap | nowrap | balance | pretty | stable`). **F**
- `word-break`, `overflow-wrap`, `hyphens`, `line-break`. **C**
- `hyphenate-character`, `hyphenate-limit-chars`. **E**

**Truncation**
- `text-overflow: clip | ellipsis | <string>`. **C**
- Multi-line clamp (`line-clamp`). **C**

**Decoration**
- `text-decoration-line` / `-color`. **F**
- `text-decoration-style` (incl. `wavy`) / `-thickness` / `text-underline-offset` / `-position` / `text-decoration-skip-ink`. **C**
- `text-emphasis-*` (CJK). **E**
- `text-transform`. **C**
- `hanging-punctuation`. **E**
- `text-box-trim` / `text-box-edge` (leading-trim). **E**
- `text-spacing-trim` (CJK). **E**

**Bidirectional text**
- Unicode BiDi (UAX #9), implicit. **F**
- `dir` analogue per text-bearing component. **F**
- `bdo` / `bdi` analogues, `unicode-bidi`. **C**
- Vertical orientation (`text-orientation: mixed | upright | sideways`). **E**
- Ruby annotation primitives. **E**

**Complex script shaping**
- Arabic joining and cursive forms. **C**
- Indic syllable formation, reordering, ZWJ/ZWNJ. **C**
- Thai / Lao / Khmer line break and shaping. **C**
- CJK punctuation, vertical metrics, full-width/half-width. **C**
- Emoji, ZWJ sequences, variation selectors (UTS #51). **C**

**Pseudo-elements for text** — see canonical enumeration in Section 3.7. Cross-references for text-specific pseudo-elements: `::selection` (**F**), `::placeholder` (**F**), `::marker` (**C**), `::first-letter` / `::first-line` (**E**), `::spelling-error` / `::grammar-error` (**E**).

### 3.5 Text editing

**Editor surface**
- Single-line text input. **F**
- Multi-line text input. **F**
- Rich-text edit surface (mixed runs, inline images/links, animated effects). **E**
- Read-only mode. **F**
- Disabled mode. **F**
- Placeholder text. **F**

**Caret & selection**
- Caret model: logical position + visual position (BiDi-aware). **F**
- BiDi caret traversal per UAX #9. **F**
- Selection ranges (single + multi-range). **F**
- Visual selection rectangles (correct for mixed-direction lines). **F**
- Caret color / style (token-themed; blink respects reduced-motion). **F**
- `caret-color`. **F**
- Auto-scroll-into-view on caret movement / focus. **F**

**IME composition**
- Composition events (`compositionstart` / `compositionupdate` / `compositionend`) via Bevy's winit IME plumbing. **F**
- Preedit rendering (underline / highlight). **F**
- Preedit cursor positioning. **F**
- Composition commit + undo as a unit. **F**
- Composition popup positioning. **F**

**Editing operations**
- Standard editing keys: arrows (with Ctrl for word-nav), Home/End (line + document), PgUp/PgDn, Shift-select, Ctrl-A. **F**
- Word-segmented navigation per locale. **C**
- Grapheme-cluster-correct delete. **F**
- Cut / copy / paste (text + HTML + image MIME). **F**
- Undo / redo with composition-aware grouping. **F**

**OS integration**
- Spellcheck (OS where available, software fallback). **C**
- Autocorrect / autocapitalize. **C**
- `inputmode` analogue (text / numeric / decimal / tel / email / url / search). **C**
- `enterkeyhint` analogue. **C**
- Virtual keyboard show/hide hints. **E**

### 3.6 Forms and input

**Input types**
- Text, search, tel, url, email, password. **F**
- Numeric: `number`, `range` (slider). **F**
- Date / time: `date`, `month`, `week`, `time`, `datetime-local` — Buiy ships full pickers per APG. **C**
- Special: `color` (color picker), `file` (file picker). **C**
- Hidden form-state holder (no UI surface; carries form value only). **F**
- Button-like: `submit`, `reset`, `button`, `image`. **F**
- Selection: `checkbox` (incl. tri-state via indeterminate), `radio`. **F**

**Other form controls**
- Select (single + multi). **F**
- Combobox (textbox + popup). **F**
- Datalist — autocomplete suggestion source for textboxes / comboboxes. **F**
- Textarea. **F**
- Button. **F**
- Output (computed-result element). **C**
- Progress, meter (incl. low / high / optimum). **C**
- Fieldset, legend. **C**
- Label (with-for or wrapping; ACCNAME 1.2 source). **F**
- Form (in-process submit/reset semantics; not HTTP). **C**
- HTML invoker analogue (`command` / `commandfor` attributes — declarative bind from a button to a target's command, e.g. `show-popover`, `close`). **C**
- `field-sizing: content` — auto-size text inputs to content. **E**

**Constraint validation**
- Attributes: `required`, `pattern`, `min`, `max`, `step`, `minlength`, `maxlength`, `multiple`. **F**
- ValidityState analogue: `valueMissing`, `typeMismatch`, `patternMismatch`, `tooLong`, `tooShort`, `rangeUnderflow`, `rangeOverflow`, `stepMismatch`, `badInput`, `customError`. **F**
- `setCustomValidity` / `reportValidity` / `checkValidity`. **F**
- Pseudo-class state: `:required`, `:optional`, `:valid`, `:invalid`, `:user-valid`, `:user-invalid`, `:in-range`, `:out-of-range`, `:placeholder-shown`, `:read-only`, `:read-write`, `:default`, `:checked`, `:indeterminate`, `:disabled`, `:enabled`. **F**
- Form-associated custom components (analogue of `ElementInternals`). **C**

**Error message model** (WCAG 3.3.1 / 3.3.3 / 3.3.4)
- Each form control carries an error-message slot routed via the `aria-errormessage` analogue. **F**
- Error messages are live-region-aware: announced on validation failure via the global announcer. **F**
- Per-form error summary primitive (lists invalid fields, links to each). **C**
- Suggestion / fix proposal (3.3.3) is an authoring concern; Buiy provides the slot + aria wiring. **C**

**WCAG 2.2 form-specific SCs**
- WCAG 3.3.7 Redundant Entry (**Level A**) — form-state machine retains values across navigation; per-field "remember me" hooks. **C**
- WCAG 3.3.8 Accessible Authentication (Minimum, **Level AA**) — Buiy provides paste-friendly password fields, optional copy from clipboard, and avoids cognitive-only verification UIs by default. Paste-not-blocked is a CI gate. **F**
- WCAG 3.3.9 Accessible Authentication (Enhanced, **Level AAA**) — strict no-cognitive-test mode opt-in. **E**
- WCAG 3.2.6 Consistent Help (**Level A**) — apps own help placement; Buiy widget catalog ensures Help / Tooltip / Disclosure widgets render consistently. **C**

**State**
- `disabled`, `readonly`, `autofocus`, `name`, `value`, `placeholder`. **F**
- `autocomplete` token list (WCAG 1.3.5 input purpose). **C**
- Form state machine (pristine / dirty / touched / visited / valid). **F**
- Validation state propagation up forms / fieldsets. **F**

**File handling**
- File picker (single + multiple). **C**
- `accept` filter. **C**
- File drag-and-drop into a target. **C**
- Camera / mic capture. **E**
- Directory upload. **E**

**Out:** HTTP form submission, browser autofill credential store integration. **O**

### 3.7 Events and input handling

**Mouse events**
- `mousedown`, `mouseup`, `click`, `dblclick`, `auxclick`, `contextmenu`, `mouseenter`, `mouseleave`, `mouseover`, `mouseout`, `mousemove`. **F**
- Coordinates: client / page / screen / offset / movement. **F**
- Buttons + modifiers. **F**

**Pointer events** (unified, primary input model)
- `pointerdown` / `pointerup` / `pointermove` / `pointercancel` / `pointerover` / `pointerout` / `pointerenter` / `pointerleave` / `pointerrawupdate`. **F**
- `gotpointercapture` / `lostpointercapture`. **C**
- `pointerType` (mouse / touch / pen). **F**
- `pressure`, `tangentialPressure`, `tiltX/Y`, `twist`, `width`, `height` (pen / stylus fidelity). **C**
- `isPrimary`. **F**
- Pointer capture. **F**

**Touch events**
- `touchstart` / `touchmove` / `touchend` / `touchcancel`. **C**
- Multi-touch via stable identifiers. **C**
- Gesture primitives: pinch, rotate, swipe, long-press, double-tap. **C**

**Keyboard events**
- `keydown` / `keyup` / `beforeinput` / `input`. **F**
- Logical key (`KeyboardEvent.key`), physical code (`code`), repeat, location, modifiers, `isComposing`. **F**
- IME composition events. **F**
- Keyboard shortcut binding (`aria-keyshortcuts` analogue). **F** — every menu / button-with-shortcut widget needs it for APG conformance and WCAG 2.1.4.
- Global keyboard shortcut activation (`accesskey` analogue + window-level chord registration; OS-conflict policy: shortcuts that collide with OS / IME modifiers are rejected at registration time). **F**
- Single-key shortcut remap policy (per WCAG 2.1.4): every single-key shortcut is opt-in or remappable, suppressible while a textbox has focus. **F**
- `InputEvent.inputType` taxonomy (`insertText`, `deleteContentBackward`, `historyUndo`, `formatBold`, etc.) for editing semantics. **C**
- Keyboard layout map (logical-to-physical, locale-aware). **E**

**Gamepad** — first-class
- Standard mappings (DPad / sticks / face buttons / triggers / shoulder / start / select). **F**
- Logical actions (navigate / activate / back / context-menu), remappable. **F**
- Spatial focus navigation (DPad / left stick → geometric focus movement). **F**
- Analog inputs for sliders, scrollers, draggables. **C**

**Wheel / scroll**
- `wheel` event with `deltaX/Y/Z` and `deltaMode`. **F**
- `scroll` event. **F**
- `scrollend` event. **C**
- Smooth scrolling, scroll snap, momentum. **C**

**Drag and drop**
- Lifecycle: `dragstart`, `drag`, `dragend`, `dragenter`, `dragover`, `dragleave`, `drop`. **C**
- DataTransfer analogue. **C**
- OS drag-source / drag-target interop. **C**
- Every drag-driven Buiy widget ships a keyboard alternative (WCAG 2.5.7). **F**

**Focus events**
- `focus` / `blur` (non-bubbling). **F**
- `focusin` / `focusout` (bubbling). **F**
- `:focus-visible` heuristic. **F**
- `relatedTarget`. **C**

**Form events**
- `input`, `change`, `submit`, `reset`, `invalid`, `formdata`, `beforeinput`. **F**
- `selectionchange`, `select`. **C**

**Clipboard**
- `copy`, `cut`, `paste` events. **F**
- Programmatic clipboard read/write (text + HTML + image MIME). **C**
- OS clipboard format negotiation. **C**

**Event flow**
- Capture → target → bubble. **F**
- `stopPropagation`, `stopImmediatePropagation`, `preventDefault`. **F**
- Listener options: `passive`, `once`, `signal`, `capture`. **C**
- Synthetic / dispatched events. **C**

**Pseudo-class state surface (interactive)**
- `:hover`, `:active`, `:focus`, `:focus-visible`, `:focus-within`, `:target`. **F**
- `:has()` (dependent-state selector). **C**
- `:is()`, `:where()`, `:not()`. **C**
- `:dir(ltr | rtl)`, `:lang(<code>)`. **F** — required given RTL is a foundation goal.
- `:state(<custom>)` — Custom State Pseudo-class API for form-associated custom widgets. **C**
- `:fullscreen` — true when subtree is the active fullscreen surface. **C**
- `:modal` — true when subtree is an open modal `Dialog` or `AlertDialog`. **C**
- `:popover-open` — true when popover element is open (auto / manual / hint). **C**

**Pseudo-class state surface (structural)**
- `:nth-child(<an+b>)`, `:nth-of-type(<an+b>)`, `:nth-last-child`, `:nth-last-of-type`. **C**
- `:first-child`, `:last-child`, `:only-child`, `:first-of-type`, `:last-of-type`, `:only-of-type`. **C**
- `:empty`. **C**
- `:root`, `:scope`. **C**

**Pseudo-elements**
- `::before`, `::after` — generated content / decorative inserts. **C**
- `::backdrop` — modal / dialog / fullscreen backdrop styling. **F**
- `::selection`. **F**
- `::placeholder`. **F**
- `::marker` (list bullets). **C**
- `::highlight(<name>)` — Custom Highlight API for collaborative highlights, find-in-page, custom underline ranges. **E**
- `::file-selector-button`. **C**
- `::part(<name>)`, `::slotted(<selector>)` — Shadow-DOM-style component-encapsulation pseudo-elements. **E**
- `::details-content` — disclosure / `<details>` open state content. **C**
- `::view-transition`, `::view-transition-group`, `::view-transition-image-pair`, `::view-transition-old`, `::view-transition-new` — view transition pseudo-elements (paired with Section 3.8). **C**
- `::spelling-error`, `::grammar-error`. **E**
- `::first-letter`, `::first-line`. **E**
- `::target-text` (text fragments). **E**

**Observers** (programmatic observation primitives, analogous to web Observer APIs)
- `IntersectionObserver` analogue — observe when a node enters / leaves viewport or another node. **C** — required for lazy-load, virtualization, scroll-based reveal.
- `ResizeObserver` analogue — observe size changes on a node. **C** — required for container-query authors and responsive components.
- `MutationObserver` analogue — observe subtree mutations beyond Bevy's per-component change-detection. **C**
- `PerformanceObserver` analogue — observe per-frame layout / render / a11y-update timings. **E**

**At-rules / cascade primitives**
- Token cascade is Buiy-native; CSS at-rules (`@media`, `@supports`, `@layer`, `@scope`, `@import`) are not the primary expression. The features they expose are reified differently:
  - User-preference media-query equivalents (`prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency`, `inverted-colors`) live in the `UserPreferences` resource. **F**
  - Capability media-query equivalents (`pointer: none|coarse|fine`, `hover: none|hover`). **C**
  - Display media-query equivalents (`color-gamut: srgb|p3|rec2020`, `dynamic-range: standard|high`). **C**
  - Container-query units (`cqw / cqh / cqi / cqb / cqmin / cqmax`). **C** — see Section 3.2.
  - Feature-detection (`@supports`) reifies as runtime capability resources (e.g., `RenderCapabilities`). **C**
  - Cascade layering (`@layer`) is unnecessary because Buiy doesn't ship a stylesheet language; theme override priority is explicit (subtree `Theme` component). **O**
  - `@scope` is unnecessary for the same reason. **O**
  - CSS nesting (`& selector`) — irrelevant without a stylesheet. **O**

**Out:** deprecated DOM mutation events, web's `Event.isTrusted` security flag (Buiy events are all in-process; the verification harness in Section 3.15 *does* synthesize input events for testing — this is unrelated to the web's trusted-vs-synthetic distinction), hashchange / popstate. **O**

### 3.8 Animation and motion

**Property transitions** (CSS Transitions analogue)
- Transition any animatable property on state change. **F**
- `transition-property` / `-duration` / `-timing-function` / `-delay` / `-behavior` (allow-discrete). **F**
- Timing functions: `linear()` (multi-stop), `ease`, `ease-in/out/in-out`, `cubic-bezier()`, `steps()`, `step-start/end`. **C**
- Discrete property transitions (e.g., display) via `@starting-style` analogue. **C**
- `interpolate-size: allow-keywords` analogue — animate to/from intrinsic-size keywords (`auto`, `min-content`, `max-content`, `fit-content`). **C**
- Transition lifecycle events. **C**

**Keyframe animations** (CSS Animations analogue)
- Keyframes (from / to / percentages, named timelines). **F**
- Animation properties: name, duration, timing, delay, iteration-count, direction, fill-mode, play-state, composition, timeline, range. **C**
- Animation lifecycle events. **C**

**Programmatic animation API** (Web Animations API analogue)
- Per-element programmatic control: play, pause, reverse, finish, cancel, playback rate. **C**
- Composite operations (replace / add / accumulate). **E**

**Layout transitions** (View Transitions analogue)
- Animate layout changes (size, position) automatically when state changes. **C**
- Cross-state snapshots. **C**
- Per-element view-transition names. **C**

**Scroll-driven animations**
- Scroll timeline, view timeline. **E**

**Game-flavored animation**
- Spring physics primitives. **C**
- Timeline composition (sequence, parallel). **C**

**Reduced motion**
- All animations short-circuit under `prefers-reduced-motion: reduce`. **F**
- WCAG 2.3.1 — no flashes >3/sec; flash detection in CI. **F**
- WCAG 2.3.3 — animation from interactions respects reduced-motion. **F**

### 3.9 Media and graphics

**Images**
- Image rendering with DPR-aware variants (srcset / sizes analogues). **C**
- `<picture>` art-direction analogue — multiple sources with media-condition switching (e.g., aspect-ratio-based, color-gamut-based). **C**
- Loading hints (lazy, eager). **C**
- `object-fit` (`contain` / `cover` / `fill` / `none` / `scale-down`), `object-position`. **F**
- `image-rendering` hints (`auto` / `smooth` / `high-quality` / `crisp-edges` / `pixelated`). **C** — `pixelated` critical for game pixel art.
- `image-orientation`. **E**
- Format support driven by Bevy asset pipeline (PNG, JPEG, WebP, AVIF, KTX2, etc.). **F**

**Video / audio** — captioning required (WCAG 1.2.x).
- Video element (controls, autoplay, muted, loop, poster, preload, playsinline). **E**
- Audio element. **E**
- Multiple sources / format negotiation. **E**
- Captions / subtitles via VTT (track element analogue), WCAG 1.2.2 (Captions, prerecorded — AA). **C**
- Audio description track support / WCAG 1.2.3 (audio description, prerecorded — A) — slot for descriptive narration track. **C**
- Live captions / WCAG 1.2.4 — slot for live caption stream. **E**
- Audio description / WCAG 1.2.5 (AA). **E**
- Picture-in-picture, fullscreen control. **E**

**Programmatic rendering surfaces**
- Render-to-texture surfaces (Buiy nodes drawn on a Bevy texture; usable for in-world UI, mini-maps, custom drawing surfaces). **C**
- Direct integration with Bevy's render targets. **C**
- 2D drawing context (Canvas2D analogue) — imperative drawing primitive (paths, strokes, fills, gradients, transforms, compositing, text, images, pixel manipulation, hit regions). For custom widgets that paint procedurally without an entity per shape. **C**

**Vector graphics**
- Vector image rendering (SVG-equivalent) — via lyon or comparable. **C**
- Inline vector primitives (rect, circle, ellipse, line, polyline, polygon, path, group). **C**
- Vector filters (feGaussianBlur, feColorMatrix, feMorphology, feTurbulence, feDisplacementMap). **E**
- External SVG via image asset (rasterized). **C**

**Out:** iframe equivalent (no nested document model), MathML, embedded objects/applets, DRM/EME. **O**

### 3.10 Widget catalog (APG patterns)

Every widget below ships, by default, with: APG keyboard contract, AccessKit role + name source + states, theme-token consumption, `:focus-visible` behavior, forced-colors fallback (no shadow-only affordances), reduced-motion fallback, RTL mirroring, ≥24×24 hit target (WCAG 2.5.8), and coverage by the verification fixture matrix in Section 3.15 (gates 2 — visual regression, 3 — AccessKit tree snapshot, 4 — announcement output, 7 — APG keyboard contract). Per-widget detail (exact keyboard contract, `aria-haspopup` / `aria-current` value emitted, name source) lives in `buiy-widget-catalog-design`.

**Foundational widgets**
- Button (incl. toggle button via `aria-pressed`). **F**
- Link. **F**
- Text. **F**
- Image. **F**
- Heading (with level). **F**
- Label. **F**
- Group / Section / Article / Region (semantic containers). **F**
- Landmarks: banner, navigation, main, complementary, contentinfo, search, region, form. **F**

**Selection & form**
- Checkbox (binary + tri-state via `aria-checked="mixed"`). **F**
- Switch. **F**
- Radio Group. **F**
- Listbox (single + multi-select). **F**
- Combobox (textbox + popup listbox). **F**
- Slider (single + multi-thumb). **F**
- Spinbutton (numeric stepper). **F**
- Textbox (single-line + multi-line — see Section 3.5). **F**
- Searchbox. **F**
- Date picker (calendar grid per APG). **C**
- Time picker. **C**
- Color picker. **C**
- File picker. **C**

**Navigation**
- Menu. **F**
- Menubar. **C**
- Menu Button. **F**
- Tabs (auto-activate + manual-activate variants). **F**
- Toolbar. **C**
- Breadcrumb (uses `aria-current="page"`). **C**
- Tree. **C**
- Treegrid. **C**

**Containers & overlays**
- Dialog (modal + non-modal), with `::backdrop` styling and `closedby` (`any` / `closerequest` / `none`) light-dismiss policy. **F**
- Alert Dialog. **F**
- Popover (light dismiss + manual + hint variants), full HTML popover state machine: `auto` (light dismiss + stack), `manual`, `hint`. Invokers via the `command` / `commandfor` analogue. Lifecycle events: `toggle`, `beforetoggle`. **F**
- Anchored popover — popover + anchor positioning (Section 3.2). **F**
- Tooltip — non-interactive, hover/focus only, dismissable / hoverable / persistent per WCAG 1.4.13. **F**
- Disclosure (button + content), uses `::details-content` / `aria-expanded`. **F**
- Accordion (incl. exclusive accordion via `name` attribute analogue). **F**
- Window splitter (with keyboard-alternative resize per WCAG 2.5.7). **C**
- Fullscreen surface — request / exit fullscreen for a Buiy subtree, with `:fullscreen` pseudo-class state, integrated with the top layer. **C**
- Scrollbar — focusable scrollbar widget per ARIA `scrollbar` role, used standalone (e.g., custom scroll containers); the implicit scrollbars on overflow-scroll are themable but not exposed as widgets. **C**

**Display & feedback**
- Progressbar (determinate + indeterminate). **F**
- Meter (with low / high / optimum thresholds). **C**
- Alert (live region, `role=alert`). **F**
- Status (live region, `role=status`). **F**
- Log (live region, `role=log`). **C**
- Timer (live region, `role=timer`). **C**
- Toast / Snackbar (live region with auto-dismiss + WCAG 2.2.3 compliance — pause / stop / extend). **F**
- Carousel — full APG pattern with auto-rotation controls, pause / stop, manual-advance contract, tied to WCAG 2.2.2. **C**
- Feed (live-loading list, tied to WCAG 2.2.2 pause/stop). **C**
- Card (composite container; if the entire surface is clickable, exposes `role="button"` or `role="link"` per the canonical "card" pattern; otherwise `role="group"` with internal interactive children). **C**
- Rating (reuses the APG `slider` pattern with discrete steps; arrow keys increment / decrement, Home / End set min / max). **C**

**Tabular data**
- Table (semantic). **C**
- Grid (data grid with cell navigation). **C**
- Sortable / filterable extensions. **C**

Each widget's APG keyboard contract is enumerated in the per-widget sub-spec (Section 4 — `buiy-widget-catalog-design`).

### 3.11 Accessibility (ARIA + WCAG 2.2)

**ARIA roles taxonomy** — full enumeration, mapped to AccessKit `Role`.

- **Landmarks (8):** banner, complementary, contentinfo, form, main, navigation, region, search. **F**
- **Document structure (38):** article, blockquote, caption, cell, code, columnheader, definition, deletion, document, emphasis, feed, figure, generic, group, heading, img / image (`img` and `image` are interchangeable tokens in ARIA 1.2), insertion, list, listitem, mark, math, meter, none / presentation, note, paragraph, row, rowgroup, rowheader, separator (non-focusable), strong, subscript, superscript, suggestion, table, term, time, toolbar, tooltip. **F**
- **Deprecated and not implemented:** `directory` (deprecated in ARIA 1.2). **O**
- **Standalone widgets (20):** button, checkbox, gridcell, link, menuitem, menuitemcheckbox, menuitemradio, option, progressbar, radio, scrollbar, searchbox, separator (focusable), slider, spinbutton, switch, tab, tabpanel, textbox, treeitem. **F**
- **Composite widgets (9):** combobox, grid, listbox, menu, menubar, radiogroup, tablist, tree, treegrid. **F**
- **Live region — alert, log, status, timer.** **F**
- **Live region — marquee** (legacy, deprecated-leaning). **E**
- **Window (2):** alertdialog, dialog. **F**

**ARIA states & properties**

- **Widget states:** `aria-busy`, `aria-checked` (`true` / `false` / `mixed`), `aria-disabled`, `aria-expanded`, `aria-hidden`, `aria-invalid` (`true` / `false` / `grammar` / `spelling`), `aria-pressed` (`true` / `false` / `mixed`), `aria-selected`. **F**
- **Widget properties:** `aria-autocomplete` (`none` / `inline` / `list` / `both`), `aria-haspopup` (`false` / `true` / `menu` / `listbox` / `tree` / `grid` / `dialog`), `aria-label`, `aria-level`, `aria-modal`, `aria-multiline`, `aria-multiselectable`, `aria-orientation` (`horizontal` / `vertical`), `aria-placeholder`, `aria-readonly`, `aria-required`, `aria-sort` (`ascending` / `descending` / `none` / `other`), `aria-valuemax`, `aria-valuemin`, `aria-valuenow`, `aria-valuetext`. **F**
- **Live region:** `aria-live` (`off` / `polite` / `assertive`), `aria-atomic`, `aria-relevant` (`additions` / `removals` / `text` / `all`), `aria-busy`. **F**
- **Drag/drop ARIA:** `aria-grabbed`, `aria-dropeffect` deprecated in ARIA 1.2 — **not implemented**. **O** Replacement contract: every drag-driven widget exposes (a) a `Move-to`-style action via AccessKit (`Increment` / `Decrement` for ordered lists, custom action for arbitrary positioning), (b) a keyboard alternative per WCAG 2.5.7, (c) a polite live-region announcement on drag start / drag end / drop / cancel. Spec'd in `buiy-input-events-design`.
- **Relationships:** `aria-activedescendant`, `aria-colcount`, `aria-colindex`, `aria-colindextext`, `aria-colspan`, `aria-controls`, `aria-describedby`, `aria-description`, `aria-details`, `aria-errormessage`, `aria-flowto`, `aria-labelledby`, `aria-owns`, `aria-posinset`, `aria-rowcount`, `aria-rowindex`, `aria-rowindextext`, `aria-rowspan`, `aria-setsize`. **F**
- **Global (foundation):** `aria-current` (`page` / `step` / `location` / `date` / `time` / `true` / `false`), `aria-keyshortcuts` (required for menu / button-with-shortcut widgets and WCAG 2.1.4), `aria-roledescription`. **F**
- **Global (core):** `aria-braillelabel`, `aria-brailleroledescription` (only emitted when AT requests braille; AccessKit-supported). **C**
- **`aria-details` vs `aria-describedby` policy** — `aria-describedby` is for short flat text references (descriptive labels). `aria-details` is for rich / structured supporting content (long descriptions, tables, footnotes). Per-widget contracts in `buiy-widget-catalog-design` specify which to emit.

**Accessible Name and Description Computation (ACCNAME 1.2)**
- Full algorithm implemented in `buiy_core`. **F**
- Name from `aria-labelledby` > `aria-label` > host-language label > content > `title`. **F**
- Description from `aria-describedby` > `aria-description` > host-language > `title`. **F**
- Hidden subtree exclusion rules. **F**

**Live regions and announcements**
- Politeness levels (off / polite / assertive). **F**
- `aria-atomic`, `aria-busy`, `aria-relevant`. **F**
- `role=status`, `role=alert`, `role=log`, `role=timer`. **F**
- `role=marquee`. **E**
- Global announcer service for ad-hoc announcements. **F**
- Locale-aware accessible-name composition (number / date formatting inside names; `lang`-switching mid-string in `aria-labelledby` chains). **C**

**Focus management**
- `:focus-visible` semantics. **F**
- Focus ring: ≥2 px perimeter, ≥3:1 contrast vs unfocused (WCAG 2.4.11). **F**
- Focus-not-obscured (WCAG 2.4.11 AA, 2.4.12 AAA). **F** (AA), **C** (AAA)
- Focus appearance enhanced (WCAG 2.4.13 AAA). **C**
- Focus trap for modal dialogs (auto for `Dialog` / `AlertDialog`). **F**
- Focus restoration on overlay close. **F**
- Inert subtrees (excluded from focus + AccessKit + hit-testing). **F**
- Roving tabindex pattern. **F**
- `aria-activedescendant` strategy. **F**
- Sequential focus navigation starting point. **F**
- Skip-link primitive (visible on focus, jumps to main / a region). **F**

**Keyboard interaction patterns** (per APG)
- Tab / Shift+Tab between widgets. **F**
- Arrow keys within composite widgets. **F**
- Home / End, PgUp / PgDn for long lists / sliders. **F**
- Enter / Space to activate. **F**
- Escape to dismiss / close. **F**
- Type-ahead (first-letter search) in menus / listboxes / comboboxes. **F**
- F2 to enter edit mode (grid). **C**
- Per-widget contracts enumerated in `buiy-widget-catalog-design`.

**Screen reader interop**
- AccessKit drives Windows UIA, macOS NSAccessibility, Linux AT-SPI (X11 + Wayland), Android TalkBack, iOS UIAccessibility (in progress upstream), web ARIA (planned upstream). **F**
- Tested against: NVDA, JAWS, Narrator, VoiceOver (mac/iOS), Orca, TalkBack. **F** (via verification harness — see Section 3.15)
- Braille via OS where AccessKit + OS support. **C**

**User preferences**
- `prefers-color-scheme: light | dark`. **F**
- `prefers-reduced-motion`. **F**
- `prefers-reduced-transparency`. **C**
- `prefers-contrast: no-preference | more | less | custom`. **C**
- `prefers-reduced-data`. **E**
- `forced-colors: active | none` + system color keywords. **F**
- `inverted-colors`. **E**

**Visual a11y**
- Contrast: WCAG 1.4.3 AA (4.5:1 / 3:1 large), 1.4.6 AAA (7:1 / 4.5:1), 1.4.11 non-text 3:1. **F** (AA), **C** (AAA)
- APCA contrast utility alongside WCAG 2 ratios. **C**
- Text resizing 200% (1.4.4). **F**
- Reflow at 320 CSS px (1.4.10). **F**
- Text spacing (1.4.12: line-height ≥1.5×, paragraph spacing ≥2× font, letter-spacing ≥0.12em, word-spacing ≥0.16em). **F**
- Content on hover/focus dismissable (1.4.13). **F**
- Pointer target size 24×24 (2.5.8 AA), 44×44 (2.5.5 AAA). **F** (AA), **C** (AAA)
- WCAG 2.5.7 dragging movements alternative for every drag-driven widget. **F**
- WCAG 2.3.1 three-flashes (max 3 flashes/sec). **F**
- No content reliant on color alone (1.4.1). **F**

**WCAG 2.2 Success Criteria — full Level A and Level AA enumeration**

Each SC is mapped to one of four enforcement strategies. **CI** = automated check in the verification pipeline. **RT** = runtime-honored constraint (e.g., reduced-motion is read each frame). **LR** = lint-with-review (machine-flagged, human-confirmed at release; not a CI gate). **DC** = design constraint Buiy enables but cannot enforce (content-quality SCs the consuming app owns). **OOS** = out of scope, with reason. AAA SCs are aspirational and listed at the end.

| SC | Title | Level | Strategy | Notes |
|---|---|---|---|---|
| 1.1.1 | Non-text Content | A | DC + LR | Buiy requires a non-empty `Image.alt` field unless explicitly marked `Decoration`; lint flags missing alt text. |
| 1.2.1 | Audio-only / Video-only (Prerecorded) | A | DC | Media widget exposes alternative-content slot; quality is app concern. |
| 1.2.2 | Captions (Prerecorded) | A | DC | VTT-track support in media widget. |
| 1.2.3 | Audio Description / Media Alternative (Prerecorded) | A | DC | Description-track slot; app owns content. |
| 1.2.4 | Captions (Live) | AA | DC | Live-caption stream slot; app owns transcription. |
| 1.2.5 | Audio Description (Prerecorded) | AA | DC | Same as 1.2.3 quality. |
| 1.3.1 | Info and Relationships | A | CI | AccessKit tree shape verifies role + parent/child + relationships. |
| 1.3.2 | Meaningful Sequence | A | CI | Tree order matches visual reading order; verified by snapshot. |
| 1.3.3 | Sensory Characteristics | A | DC | Don't rely on shape/color/sound alone — content concern. |
| 1.3.4 | Orientation | AA | RT + CI | No locked orientation; verified across portrait + landscape fixtures. |
| 1.3.5 | Identify Input Purpose | AA | CI | `autocomplete` token list per input; lint enforces presence on form fields. |
| 1.4.1 | Use of Color | A | DC | Don't encode meaning in color alone; default theme + linter advise. |
| 1.4.2 | Audio Control | A | DC | Media widget surfaces controls; app uses them. |
| 1.4.3 | Contrast (Minimum) | AA | CI | Contrast linter validates every theme + token combination at 4.5:1 / 3:1. |
| 1.4.4 | Resize Text | AA | CI | 200% zoom fixture + reflow snapshots. |
| 1.4.5 | Images of Text | AA | LR | Linter advises against image-of-text icons; release review confirms. |
| 1.4.10 | Reflow | AA | CI | 320 CSS-px width fixture. |
| 1.4.11 | Non-text Contrast | AA | CI | Linter validates UI controls + state indicators at 3:1. |
| 1.4.12 | Text Spacing | AA | CI | Forced text-spacing fixture verifies layout doesn't clip. |
| 1.4.13 | Content on Hover or Focus | AA | CI | Tooltip / Popover contracts assert dismissable / hoverable / persistent. |
| 2.1.1 | Keyboard | A | CI | APG keyboard contract suite — every interactive widget operable. |
| 2.1.2 | No Keyboard Trap | A | CI | Focus-traversal property test exits every widget. |
| 2.1.4 | Character Key Shortcuts | A | CI | `aria-keyshortcuts` registration + remap policy verified. |
| 2.2.1 | Timing Adjustable | A | DC | App-level timing; Buiy widgets default to no timeout. |
| 2.2.2 | Pause, Stop, Hide | A | CI | Carousel + Feed + Toast assert pause/stop controls. |
| 2.3.1 | Three Flashes or Below | A | CI | Animation flash detector in CI. |
| 2.4.1 | Bypass Blocks | A | CI | Skip-link primitive + landmark navigation present in fixture. |
| 2.4.2 | Page Titled | A | RT | Window title plumbed through AccessKit. |
| 2.4.3 | Focus Order | A | CI | Tab-order snapshot per widget. |
| 2.4.4 | Link Purpose (In Context) | A | LR | Linter advises on empty / generic link names. |
| 2.4.5 | Multiple Ways | AA | DC | App routing concern. |
| 2.4.6 | Headings and Labels | AA | LR | Linter advises on missing / generic headings + labels. |
| 2.4.7 | Focus Visible | AA | CI | Focus-ring rendering verified on every focusable widget. |
| 2.4.11 | Focus Not Obscured (Minimum) | AA | CI | Sticky toolbar + modal fixtures verify focused element clear. |
| 2.4.12 | Focus Not Obscured (Enhanced) | AAA | CI (aspirational) | Focused element fully unobscured (vs Minimum's "not entirely hidden"). |
| 2.4.13 | Focus Appearance | AAA | CI (aspirational) | ≥2 px perimeter, ≥3:1 contrast vs unfocused. |
| 2.5.1 | Pointer Gestures | A | CI | Multi-pointer / path gestures all have single-pointer fallback. |
| 2.5.2 | Pointer Cancellation | A | CI | Activation on up-event with drag-off-cancel verified. |
| 2.5.3 | Label in Name | A | CI | Visible label text is part of accessible name (linter). |
| 2.5.4 | Motion Actuation | A | DC | Motion-driven actions have alternatives at app level. |
| 2.5.7 | Dragging Movements | AA | CI | Every drag widget exposes a keyboard alternative; tested. |
| 2.5.8 | Target Size (Minimum) | AA | CI | Hit-target linter ≥24×24. |
| 3.1.1 | Language of Page | A | RT | Locale resource published to AccessKit. |
| 3.1.2 | Language of Parts | AA | RT | Per-text-component lang plumbed. |
| 3.2.1 | On Focus | A | CI | Focus events do not trigger context changes (linter). |
| 3.2.2 | On Input | A | CI | Input events do not auto-submit / navigate (linter). |
| 3.2.3 | Consistent Navigation | AA | DC | App owns layout consistency. |
| 3.2.4 | Consistent Identification | AA | DC | Buiy widget catalog provides consistent identifiers. |
| 3.2.6 | Consistent Help | A | DC | App places Help widget; Buiy renders it consistently. |
| 3.3.1 | Error Identification | A | CI | Error-message model per Section 3.6; verified per form fixture. |
| 3.3.2 | Labels or Instructions | A | LR | Linter advises on missing labels. |
| 3.3.3 | Error Suggestion | AA | DC | App provides; Buiy renders via error-message slot. |
| 3.3.4 | Error Prevention (Legal/Financial/Data) | AA | DC | App owns the policy; Buiy provides confirmation widgets. |
| 3.3.7 | Redundant Entry | A | RT | Form state retains values across navigation; verified. |
| 3.3.8 | Accessible Authentication (Minimum) | AA | CI + DC | CI verifies paste-allowed (no `paste` block) on password / authentication input types and absence of cognitive-puzzle widgets in the default catalog; the SC's spirit (avoid forcing memory / transcription / cognitive tests) is also a design constraint on app authoring. |
| 4.1.2 | Name, Role, Value | A | CI | AccessKit tree snapshot — the central SC. |
| 4.1.3 | Status Messages | AA | CI | Live-region announcer + role=status verified. |

**AAA aspirational** — implemented as opt-in or noted as future work: 1.4.6 (7:1 contrast), 1.4.7 (Low or No Background Audio — DC), 1.4.8 (Visual Presentation), 1.4.9 (Images of Text No Exception), 2.1.3 (Keyboard No Exception), 2.2.3 (No Timing), 2.3.3 (Animation from Interactions), 2.4.8 (Location), 2.4.9 (Link Purpose Alone), 2.4.10 (Section Headings), 2.5.5 (Target Size Enhanced — 44×44), 2.5.6 (Concurrent Input Mechanisms — relevant given Buiy's gamepad / keyboard / pointer concurrency goal; aspirational rather than gated), 3.1.3-6 (cognitive content), 3.3.5-6 (help / error prevention all), 3.3.9 (Accessible Authentication Enhanced). 2.4.12 and 2.4.13 are in the main table at AAA tier.

The strategy / coverage details (fixtures, tolerances, runner) live in `buiy-verification-design`. This table is the authoritative SC roster; that sub-spec realizes it.

**Inert / hit testing**
- `inert` attribute analogue. **F**
- `pointer-events: none`. **F**
- `aria-hidden` for decorative subtrees. **F**

### 3.12 Internationalization and locale

- `lang` analogue per Buiy text; plumbed to AccessKit for AT pronunciation. **F**
- `dir` LTR/RTL with full UI mirroring (scrollbars, sliders, progress bars, icons). **F**
- Logical CSS properties (start/end vs left/right). **F**
- BiDi UAX #9, full implementation. **F**
- Vertical writing modes (CJK, Mongolian). **C**
- ICU MessageFormat 2.0 for translation strings (placeholders, plurals, gender, select, ordinals). **C**
- Locale-aware formatters: NumberFormat, DateTimeFormat, RelativeTimeFormat, PluralRules, ListFormat, Collator, Segmenter, DisplayNames, DurationFormat. **C**
- Calendar systems (Gregorian, Hebrew, Islamic, Buddhist, Japanese, Persian). **E**
- Numbering systems (Latin, Arabic-Indic, others). **E**
- Pseudolocale support for QA. **E**

### 3.13 State, data, reactivity

- Two-way binding for input value / checked / selected. **F**
- Form state machine. **F**
- Validation state propagation. **F**
- Selection state (single / multi / range with shift-click, ctrl/cmd-toggle). **F**
- Drag state. **C**
- Pseudo-class state surface (Section 3.7). **F**
- Reactivity primitive: Bevy observers + change detection. **F**
- Signal / computed / effect layer. **O** — explicitly excluded from the foundation. May return as a follow-up sub-spec if usage demands it; the spec does not commit to that.
- **Out:** History API / URL routing, `localStorage` / `sessionStorage` / `IndexedDB`. UI does not own persistence or routing. **O**

### 3.14 Theming and user preferences

(Most of this is restated from Section 2 for catalog completeness.)

- Semantic tokens. **F**
- Theme assets (hot-reloadable). **F**
- Variants (light / dark / high-contrast / custom). **F**
- Scales (color, spacing, typography, motion, radius, elevation). **F**
- Theme inheritance — subtree can carry its own `Theme` component to override. **C**
- All `prefers-*` queries surfaced as `UserPreferences` resource. **F**
- Forced-colors mode → token palette swap with system colors. **F**
- Reduced-motion → animation short-circuit. **F**
- Color-scheme → variant swap. **F**
- Color-gamut detection (`(color-gamut: srgb | p3 | rec2020)`). **C**
- Pointer / hover media features (`(pointer: none|coarse|fine)`, `(hover: none|hover)`). **C**
- Dynamic-range detection (`(dynamic-range: standard | high)`). **E**
- CSS-flavored stylesheet — out of this spec; future sub-spec if needed.

### 3.15 Verification pipeline

The verification subsystem realizes goal #7. The inventory below enumerates the floor; detailed strategy (tolerances, baselines, failure thresholds, flake-mitigation, runner choice) lives in `buiy-verification-design`.

The pipeline has two tiers: **CI gates** (every PR; failure blocks merge; no human approval) and **manual release gates** (every release; explicit owner; documented cadence). Goal #7 covers only the CI tier.

#### CI gates

| # | Category | Tier | What it verifies | Notes / risk |
|---|---|---|---|---|
| 1 | Unit tests | F | Component logic, layout calc, event handlers, state machines. | Standard `cargo test`. |
| 2 | Visual regression | F | Rendered output matches golden per widget × state × theme × viewport. | **Per-platform goldens** on a single canonical CI GPU class; perceptual diff with explicit tolerance budget; flake-mitigation via fixed clock + font-load sync + atlas warmup. Golden updates require a human-curated `--accept` workflow (intentional changes); CI policy "no approval gate" applies to *test outcomes*, not to golden updates. |
| 3 | AccessKit tree snapshots | F | Tree shape, role, name, description, states, relationships per widget × state. JSON diff. | `accesskit_consumer`-driven assertions. |
| 4 | Announcement-output snapshots | C | The string a live region produces; the order of name + role + state utterance for focus changes. Asserted via `accesskit_consumer`'s consumer view. | Independent of tree snapshot — verifies what an AT *would* announce, not the AccessKit tree shape itself. Does not run real NVDA/VoiceOver. |
| 5 | Layout snapshots | F | Resolved Taffy output per layout fixture (positions, sizes). | |
| 6 | Synthesized input replay | F | Keyboard, pointer, touch, gamepad events injected as Bevy events; assert resulting state. | IME composition is verified at the Buiy↔winit boundary only; full OS-IME conformance (IBus, fcitx, TSF, macOS IM) is in the manual release gate. |
| 7 | APG keyboard-contract conformance | F | Every APG pattern, every documented key, every state transition. Forms-mode and browse-mode contracts both exercised via `accesskit_consumer`. | Verifies key-to-state mapping; the *AT utterance* on each transition is in #4. |
| 8 | WCAG 2.2 SC suite (machine-testable) | F | Each Level A / AA SC marked **CI** in the Section 3.11 table. | SCs marked **DC** / **LR** are explicitly NOT CI gates. Content-quality SCs (1.1.1 alt-text quality, 1.4.5, 2.4.4, 2.4.6, 3.3.2) are linter-with-review or design-constraint, not CI. |
| 9 | Contrast linter | F | Every theme × every token combination. WCAG 2 (4.5:1 / 3:1 / 3:1) is the gate; APCA (Lc thresholds: ~Lc 60 minimum, Lc 75 body, Lc 90 preferred) is advisory. | Both algorithms ship; WCAG 2 is the legal-bar gate. APCA upgrade path documented in `buiy-verification-design`. |
| 10 | Hit-target linter | F | Every interactive widget rendered with hit area ≥24×24 at every viewport in the fixture set. | Geometric check on the picking hit-rect at layout time. |
| 11 | Forced-colors compatibility scan | F | Two checks: (a) no widget paints a color outside the system-color token set when `forced-colors: active`; (b) no shadow-only affordance — every focusable / state-bearing widget has a non-shadow visual cue (border, fill, outline). | Token-flow analyzer + golden visual diff under forced-colors. |
| 12 | Property tests / fuzzing | F | Generators for hierarchies (max-depth N, max-breadth M, shrink-to-minimal-failing-tree), input streams, theme-variant matrices. Invariants: "focus tree reachable from any starting node," "AccessKit tree has no orphans," "every focusable node has an accessible name," "BiDi caret round-trip equals identity." | `proptest` with named strategies. |
| 13 | Hot-reload validation | F | Reload `.bsn` file or theme asset; assert live entity diff over stable IDs equals expected diff; no entity / atlas leaks. | Equality predicate is "stable-ID-keyed entity-state diff"; spec'd in `buiy-verification-design`. |
| 14 | Performance regression | F | Per-frame layout time + render time + AccessKit-update time relative to main-branch baseline on a fixed self-hosted runner. | The CI gate's *mechanism* (relative-to-main, fixed runner, ±10% default slack) is committed. The *actual budget numbers* per fixture are an open question (Section 5) owned by `buiy-verification-design`. The gate exists at v1; the numbers calibrate over time. |
| 15 | Memory leak tests | F | RSS slope and atlas-entry count return to baseline after a defined long-running fixture (~10 minutes of scripted activity, then idle). | Threshold: RSS slope < 1 MB / minute after warmup; atlas entries return within ε of baseline. |

#### CI platform matrix

- **Desktop (Windows UIA, macOS NSAccessibility, Linux AT-SPI)** — full CI matrix for v1, all categories above.
- **Android** — deferred until `accesskit_android` exposes a headless harness; until then, manual release-gate platform.
- **iOS** — deferred until `accesskit_ios` ships and a CI strategy (Mac runner + simulator, or device farm) is selected; until then, manual release-gate platform.
- **Web** — deferred until AccessKit web adapter ships; until then, no a11y verification on web target.

Open question on platform staging: Section 5.

#### CI policy

- Runs on every PR. Failure blocks merge.
- "No human approval gate" applies to **test outcomes**: green = mergeable. Golden-image and AccessKit-snapshot updates use a human-reviewed `--accept` workflow as part of standard PR review.
- Cross-platform matrix runs in parallel.

#### Manual release gates (NOT CI gates; required at every release)

These gate releases, not PRs. Each has an owner, a documented cadence, and a release-blocking sign-off mechanism: each gate produces a checked-in sign-off document at `docs/release-notes/<version>/manual-gate-<gate>-signoff.md`. Tagging a release is gated on all four sign-off documents being present and approved on the release branch.

1. **Real-SR output sanity sweep** — run a curated fixture suite under NVDA + Firefox-equivalent host (Windows), VoiceOver (macOS), Orca (Linux GNOME), TalkBack (Android emulator). Verify utterances against expected-output strings. Owner: a11y maintainer. Cadence: every minor release. (May graduate to a CI gate if a headless real-SR harness becomes practical — open question.)
2. **Real-device mobile sweep** — Android + iOS on physical or simulated devices; verify TalkBack / VoiceOver behavior, IME composition with real OS IMEs (IBus, fcitx, macOS Japanese IM, Windows TSF), gesture recognizers under real touch. Owner: platform maintainer. Cadence: every minor release.
3. **Subjective visual review** — design lead reviews default theme(s), widget gallery, animation polish. Cadence: every minor release. Note: WCAG 1.4.3 / 1.4.11 contrast is *not* in this gate (it's CI #9); this gate covers polish and brand alignment.
4. **Content-quality SC review** — alt-text quality, link-purpose, label clarity in shipping examples and docs. WCAG 1.1.1 / 1.4.5 / 2.4.4 / 2.4.6 / 3.3.2. Owner: docs maintainer. Cadence: every minor release.

**Coverage tradeoff acknowledgment (cross-reference Section 1):** these four gates collectively cover real-AT speech, real-device behavior, subjective polish, and content quality. Several user-experience claims sit at the manual tier rather than the CI tier. Goal #7 is honest about this — "every machine-testable claim" — but readers should understand that "Buiy's verification pipeline is fully automated" only refers to the CI tier, not to all things end-users experience.

#### Multi-window verification

All CI gates run per-window where applicable. AccessKit tree snapshots, focus tree state, IME consumer state, and picking results are keyed by `WindowId`. Multi-window fixtures verify per-window stack ownership (Section 3.18).

#### Hot-reload trigger flow

Asset hot-reload tests (gate #13) drive the verification harness through Bevy's standard `AssetEvent::Modified` for `BsnAsset`, `ThemeAsset`, and `FontAsset`. Buiy's reload systems observe these events and apply the diff; the harness then asserts the post-reload entity-state diff equals the expected diff. Asset graph and `AssetServer` integration is owned by `buiy-asset-pipeline-design`.

#### Tooling

- `accesskit_consumer` — simulated AT consumer for tree-snapshot and announcement-snapshot testing.
- Bevy's screenshot system + a perceptual-diff crate — visual regression with tolerance budget.
- `proptest` — property-based testing.
- `buiy_verify` — Buiy's own harness crate; `dev-dependency` for every Buiy crate; usable by downstream Buiy users to test their own widgets.

#### What is *not* a CI gate (and why)

- **Real-SR utterance verification.** `accesskit_consumer` simulates the consumer side; it does not run NVDA / JAWS / VoiceOver. Manual release gate (#1 above). Goal #7 explicitly excludes this from "machine-testable claims."
- **Full OS-IME conformance.** OS IME backends sit upstream of winit; we verify Buiy↔winit at the boundary, real OS IME at release time.
- **WCAG content-quality SCs** (1.1.1, 1.4.5, 2.4.4, 2.4.6, 3.3.2). Linter advises; humans confirm.
- **Subjective visual quality.** Manual release gate.
- **Real-device mobile / web a11y.** Pending platform staging (Section 5 open question).

### 3.16 Devtools / DX

- Inspector (entity / component view of Buiy hierarchy). **C**
- Layout overlay (Flexbox / Grid lines, box-model boundaries). **C**
- AccessKit tree viewer. **C**
- Contrast checker (live, against current theme). **C**
- Focus order visualizer (numbered overlay showing tab order). **C**
- Performance profiler (per-frame layout / render / AccessKit timing). **C**
- Theme editor (live token tweaking). **C**
- BSN hot-reload indicator. **C**
- Verification harness CLI for downstream Buiy users. **C**

### 3.17 3D-anchored UI (deferred subsystem)

- UI panels as billboards in 3D space. **C**
- UI panels on curved or arbitrary surfaces. **E**
- Worldspace UI hit-testing through the 3D scene. **C**
- Diegetic UI (UI that lives "in" the game world — terminals, screens, holograms). **C**
- Render-to-texture for UI applied to 3D meshes. **C**

This subsystem gets its own design spec (`buiy-3d-anchored-ui-design`). No `UiTransform` / `Transform` divergence to bridge — Buiy nodes use Bevy's general `Transform`.

### 3.18 Compatibility and coexistence

Coexistence with bevy_ui is **per-window**, not per-app-shared-window. AccessKit allows exactly one tree per window adapter; one window cannot host both bevy_ui and Buiy a11y trees simultaneously without a coordinator. The supported model is:

- An app may have multiple windows. Each window is **owned by one stack**: either Buiy or bevy_ui.
- On a Buiy-owned window: Buiy owns the `accesskit_winit::Adapter`, the render-graph nodes, the `bevy_picking` backend(s), the focus model, the IME consumer. `bevy_a11y` is suppressed for that window. bevy_ui's own systems do not render or interact on that window.
- On a bevy_ui-owned window: bevy_ui retains its current behavior. Buiy is absent.
- Inside a single Buiy tree, you do not mix raw `bevy_ui::Node` content. The component models are independent.
- Migration from a bevy_ui window to a Buiy window is by replacement of the window's UI tree, not by extending bevy_ui components.

**Why per-window, not coordinator-merged.** A merge coordinator (single AccessKit adapter, both stacks pushing subtrees under one root) is theoretically possible but adds a coordination crate, ID-space rules, focus-arbitration rules, and IME-routing rules — meaningful complexity for a use case (mixing Buiy and bevy_ui in one window) the spec does not need. If demand arises later, that becomes a follow-up sub-spec (`buiy-coexistence-design`).

**Coexistence rules — committed:**
- One stack per window. **F**
- Window stack assignment is fixed at window creation; **no runtime stack switching** for an existing window in v1. **F**
- Buiy components do not extend `bevy_ui::Node`. **F**
- Migration from bevy_ui → Buiy is by per-window replacement. **F**
- Per-window state keyed by winit `WindowId`: AccessKit adapter, IME consumer, focus tree root, render-graph node group, `bevy_picking` backend filter. **F**
- Render-graph node ordering, `bevy_picking` backend priority, IME consumer selection, focus arbitration: per-window stack owns these unilaterally on its own window. **F**
- **`UiPickingPlugin` interaction:** when a Buiy window is present, Buiy's picking backend is filtered to that window via `bevy_picking`'s window-filter. Bevy's default `UiPickingPlugin` (added by `DefaultPlugins`) operates on bevy_ui-owned windows only. If an app is Buiy-only and adds `DefaultPlugins`, `UiPickingPlugin` runs on no windows and is a no-op. **F**

**Excluded entirely**
- Networking, fetch, XHR, WebSocket, WebRTC, WebTransport. **O**
- Cookies, localStorage, IndexedDB, service workers, web app manifest, install. **O**
- File System Access, Web Bluetooth, USB, Serial, MIDI, NFC, geolocation. **O**
- WebAuthn, Payment Request. **O**
- Speech recognition / synthesis (game audio, not UI). **O**
- DRM / EME. **O**
- `data:` / `blob:` URL schemes. **O**
- Native iframe / sandbox / cross-origin. **O**
- Same-origin policy, CORS, CSP. **O**
- HTML parser quirks mode, document.write, mutation events (deprecated). **O**
- Print stylesheet (`@media print`, `@page`). **O** — Bevy is not a print target.
- SMIL animation. **O** — superseded by CSS / WAAPI analogue.
- Legacy presentational HTML attributes. **O**

## 4. Sub-spec roadmap

Each subsystem below graduates to its own design spec at `docs/specs/YYYY-MM-DD-<topic>-design.md` when it's that subsystem's turn to be designed. Each will cite this foundation spec.

| Sub-spec | Scope |
|---|---|
| `buiy-render-pipeline-design` | Render passes, top-layer compositing, clipping, filters, blend modes, atlasing, color management, render-graph node ordering. |
| `buiy-layout-design` | Taffy integration, anchor positioning, container queries, writing-mode integration. |
| `buiy-text-rendering-design` | cosmic-text integration, atlas management, font registration, fallback. |
| `buiy-text-editing-design` | IME composition, BiDi caret, undo/redo, multi-line, rich-text edit surface. |
| `buiy-focus-model-design` | Focus tree, `:focus-visible`, traps, restoration, roving tabindex, gamepad spatial nav. |
| `buiy-accessibility-design` | AccessKit tree construction, decomposed components, ACCNAME 1.2, live regions, adapter ownership. |
| `buiy-theme-tokens-design` | Semantic tokens, theme assets, variants, OS-pref binding, contrast linter, APCA upgrade path. |
| `buiy-widget-catalog-design` | APG patterns shared infrastructure; per-widget specs nest as multi-file children. |
| `buiy-animation-design` | Transitions, keyframes, layout transitions, springs, reduced-motion gating. |
| `buiy-forms-design` | Form state machine, constraint validation, validation pseudo-classes, error-message model. |
| `buiy-input-events-design` | Pointer, keyboard, touch, gamepad, IME, drag-and-drop, `bevy_picking` backend registration + priority, drag a11y replacement contract. |
| `buiy-i18n-design` | BiDi, vertical writing, ICU, locale-aware formatters, calendar/numbering systems. |
| `buiy-3d-anchored-ui-design` | Billboards, worldspace UI, render-to-texture surface API, hit-testing. |
| `buiy-verification-design` | Automated pipeline, harness API, WCAG-SC test fixtures + tolerances, CI matrix, manual release-gate cadences, perf baselines, `--accept` workflow. |
| `buiy-devtools-design` | Inspector, overlays, contrast checker, focus visualizer, theme editor. |
| `buiy-bsn-integration-design` | BSN authoring helpers, decomposed-component conventions, reflection-registration ergonomics, hot-reload semantics including component reload. |
| `buiy-asset-pipeline-design` | Theme assets, font assets, `.bsn` assets, icon atlases, vector assets, hot-reload semantics, asset GC, atlas-warmup strategy. |
| `buiy-coexistence-design` | (Conditional sub-spec — only if same-window coexistence with bevy_ui becomes required.) AccessKit-adapter coordinator, render-pass ordering across stacks, picking-backend priority across stacks, IME ownership, focus arbitration. |
| `buiy-window-and-surface-design` | Multi-window, render targets, render-to-texture contracts, off-screen rendering, fullscreen surface, top-layer per-window. |
| `buiy-clipboard-and-os-integration-design` | Clipboard, drag-drop OS interop, virtual keyboard hints, spellcheck OS bridge, system-color resolution, OS-preference plumbing. |

Each sub-spec gets one or more plans (`docs/plans/`) for implementation.

## 5. Open questions

- **Final crate split.** Single crate vs multi-crate workspace; if multi-crate, the exact partition. The spec commits to modular subsystems with clean boundaries; the partition can change.
- **Reactivity layer.** Observers + change detection only in v1. Whether to add a signal/computed/effect primitive in a follow-up sub-spec is open.
- **CSS-flavored stylesheet.** Never, or as a future layer above tokens? bevy_flair sets one precedent; the right answer depends on user demand.
- **Date/time pickers — Buiy-owned vs OS-delegated.** Buiy-owned per APG gives consistent visuals; OS-delegated is lighter. Spec defaults to Buiy-owned (consistency), but this is reversible.
- **WCAG 2.2 SC enforcement strategy.** Per-SC: automated CI check, runtime-honored constraint, or documented design constraint. The mapping table is owned by `buiy-verification-design`.
- **3D-anchored UI prioritization.** The renderer is ours and `Transform` works, so it's unblocked. Whether `buiy_3d` is concurrent with foundation work or strictly deferred is a planning choice.
- **Coexistence policy with `bevy_feathers` / `bevy_ui_widgets`.** Coexistence at the app level is committed; whether Buiy ships migration adapters from bevy_ui widgets is open.
- **Performance budgets — concrete numbers.** The CI gate (Section 3.15 #14) is committed; the per-fixture *budget numbers* (target frame-time per fixture, allowed regression slack) calibrate over time and live in `buiy-verification-design`.
- **Platform support staging.** All platforms (Windows / macOS / Linux / Android / iOS / web) at v1, or staged?
- **Hot-reload of components (not just themes).** In scope as part of `buiy-bsn-integration-design`?
- **Render-to-texture surface API contract.** Feeds `buiy_3d`; the boundary is open.
- **Animation library substrate.** Roll our own springs, depend on `bevy_animation`, or wrap an existing crate?
- **OS spellchecker integration.** Where the OS exposes a spellchecker, Buiy uses it; where not, software fallback. The fallback library choice is open.
- **Real screen-reader testing in CI.** Currently out of CI (manual at release). If this becomes feasible (e.g., headless NVDA via vmnv tools), it becomes a CI gate.
- **AccessKit-adapter ownership when both stacks coexist same-window.** Currently the spec rules this out (per-window coexistence only). If demand arises, `buiy-coexistence-design` defines the coordinator.
- **AccessKit cadence policy decoupled from Bevy.** Whether AccessKit major releases between Bevy minors trigger a Buiy patch release with explicit semver, or are absorbed silently.
- **Reflection-registration ergonomics for BSN consumers.** Whether `register_type` calls are emitted by a derive macro on Buiy components, by a sub-plugin per crate, or by a single global plugin call.
- **Bevy WASM target policy.** Bevy supports WASM; the spec lists "non-Bevy frontends" as out, but Bevy-on-WASM is an in-scope Bevy target. Web a11y waits for AccessKit's web adapter; visual / input / layout work on WASM today. Whether v1 commits to WASM as a target platform is open.
- **Wayland vs X11 a11y differences.** AT-SPI behavior diverges between session types; whether Buiy ships Wayland-specific code paths or assumes AT-SPI parity is open.
- **APCA gate or advisory.** Currently APCA is advisory; WCAG 2 ratios are the gate. If WCAG 3 (which incorporates APCA) reaches recommendation status, the gate flips.
- **Real-device mobile CI staging.** Section 3.15 punts Android / iOS to manual release gate. Open question: budget and timeline for moving them into CI.
- **Crate-split refinement.** Section 2.8 lists `buiy_core` as containing render + layout + focus + theme + a11y primitives. That may be too coarse; splitting into `buiy_render`, `buiy_a11y`, `buiy_layout`, `buiy_focus`, `buiy_theme` is open.

## References

- Bevy UI ecosystem report (research input, May 2026).
- Web platform UI feature catalog (research input, May 2026).
- Accessibility deep-dive (research input, May 2026).
- WAI-ARIA 1.2 — https://www.w3.org/TR/wai-aria-1.2/
- ARIA Authoring Practices Guide — https://www.w3.org/WAI/ARIA/apg/
- Accessible Name and Description Computation 1.2 — https://www.w3.org/TR/accname-1.2/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- AccessKit — https://accesskit.dev
- Bevy issue #17644 (`bevy_a11y` BSN-incompatibility, lesson source) — https://github.com/bevyengine/bevy/issues/17644
- Bevy discussion #14437 (BSN tracking) — https://github.com/bevyengine/bevy/discussions/14437
- Bevy discussion #16900 (Standard Headless Widgets) — https://github.com/bevyengine/bevy/discussions/16900
- Bevy issue #11100 (10 Challenges for Bevy UI Frameworks) — https://github.com/bevyengine/bevy/discussions/11100
