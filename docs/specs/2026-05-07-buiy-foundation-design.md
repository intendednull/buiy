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

7. **Verifiable.** Every claim Buiy makes (every widget behavior, every APG keyboard contract, every WCAG SC, every theme variant, every layout primitive, every animation curve) is covered by automated tests that run in CI without a human in the loop. "It works" is an output of the test pipeline, not an assertion.

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
- **BSN** (Bevy 0.18+): `bsn! { Button [ Text("Save") ] }` or hot-reloadable `.bsn` files. BSN spawns any components, including Buiy's — no special integration needed.

The BSN-friendliness constraint on every Buiy component is **not optional**: small, public-fielded, observable, decomposed by concern. Megacomponents are forbidden.

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

### 2.7 Reactivity

Observers + change detection only. No signal/computed/effect layer in this spec.

### 2.8 Module organization

Buiy ships as a workspace of focused crates. Final crate split is an open question (Section 5); the principle is committed:

- `buiy` — meta-crate, re-exports common API, ships top-level `BuiyPlugin`.
- `buiy_core` — components, render pipeline, layout integration, focus model, theme tokens, a11y primitives, plugin scaffolding.
- `buiy_text` — rich text + IME-correct text editing on cosmic-text.
- `buiy_widgets` — APG widget catalog.
- `buiy_animation` — transitions, keyframes, layout transitions, reduced-motion gating.
- `buiy_forms` — form state machine, validation, constraint pseudo-classes.
- `buiy_devtools` — inspector, contrast linter, focus visualizer, AccessKit tree viewer.
- `buiy_3d` — 3D-anchored / diegetic UI.
- `buiy_bsn` — BSN authoring helpers when on Bevy 0.18+.
- `buiy_verify` — verification harness; consumed as `dev-dependency` by every other crate; usable by downstream Buiy users.

### 2.9 Compatibility & policy

- **Rolling latest-stable Bevy.** Each Bevy minor release is a migration event for underlying primitive APIs (wgpu, AccessKit, render graph).
- **MSRV** tracks Bevy's MSRV.
- **`std` only.** AccessKit requires it.
- **Platform support** matches Bevy + AccessKit: Windows (UIA), macOS (NSAccessibility), Linux (AT-SPI), Android (TalkBack), iOS (in progress upstream), web (limited until AccessKit web adapter ships).
- **Coexistence with bevy_ui:** Buiy and bevy_ui can both run in the same app. They render in separate passes, manage focus separately, and have separate AccessKit trees that AccessKit composes per window.

## 3. Feature inventory

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason).

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
- Angles, time, frequency, resolution — as needed.

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
- System color keywords (`Canvas`, `CanvasText`, `LinkText`, `ButtonText`, `ButtonBorder`, `GrayText`, `Highlight`, `HighlightText`, `Field`, `FieldText`) for forced-colors. **F**

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
- `text-decoration-line` / `-style` (incl. `wavy`) / `-color` / `-thickness` / `text-underline-offset` / `-position` / `text-decoration-skip-ink`. **F/C**
- `text-emphasis-*` (CJK). **E**
- `text-transform`. **C**

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

**Pseudo-elements for text**
- First-letter / first-line equivalent (drop caps). **E**
- Selection styling (`::selection` analogue). **F**
- Marker styling (list bullets). **C**
- Placeholder styling. **C**
- Spelling/grammar error decorations. **E**

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
- Special: `color` (color picker), `file` (file picker), `hidden`. **C**
- Button-like: `submit`, `reset`, `button`, `image`. **F**
- Selection: `checkbox` (incl. tri-state via indeterminate), `radio`. **F**

**Other form controls**
- Select (single + multi). **F**
- Combobox (textbox + popup). **F**
- Textarea. **F**
- Button. **F**
- Output (computed-result element). **C**
- Progress, meter. **C**
- Fieldset, legend. **C**
- Label (with-for or wrapping; ACCNAME 1.2 source). **F**
- Form (in-process submit/reset semantics; not HTTP). **C**

**Constraint validation**
- Attributes: `required`, `pattern`, `min`, `max`, `step`, `minlength`, `maxlength`, `multiple`. **F**
- ValidityState analogue: `valueMissing`, `typeMismatch`, `patternMismatch`, `tooLong`, `tooShort`, `rangeUnderflow`, `rangeOverflow`, `stepMismatch`, `badInput`, `customError`. **F**
- `setCustomValidity` / `reportValidity` / `checkValidity`. **F**
- Pseudo-class state: `:required`, `:optional`, `:valid`, `:invalid`, `:user-valid`, `:user-invalid`, `:in-range`, `:out-of-range`, `:placeholder-shown`, `:read-only`, `:read-write`, `:default`, `:checked`, `:indeterminate`, `:disabled`, `:enabled`. **F**
- Form-associated custom components (analogue of `ElementInternals`). **C**

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
- Keyboard shortcut binding (`aria-keyshortcuts` analogue). **C**
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

**Pseudo-class state surface**
- `:hover`, `:active`, `:focus`, `:focus-visible`, `:focus-within`, `:target`. **F**
- `:has()` (dependent-state selector). **C**
- `:is()`, `:where()`, `:not()`. **C**

**Out:** deprecated DOM mutation events, trusted-vs-synthetic distinction, hashchange / popstate. **O**

### 3.8 Animation and motion

**Property transitions** (CSS Transitions analogue)
- Transition any animatable property on state change. **F**
- `transition-property` / `-duration` / `-timing-function` / `-delay` / `-behavior` (allow-discrete). **F**
- Timing functions: `linear()` (multi-stop), `ease`, `ease-in/out/in-out`, `cubic-bezier()`, `steps()`, `step-start/end`. **C**
- Discrete property transitions (e.g., display) via `@starting-style` analogue. **C**
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
- Loading hints (lazy, eager). **C**
- `object-fit` (`contain` / `cover` / `fill` / `none` / `scale-down`), `object-position`. **F**
- `image-rendering` hints (`auto` / `smooth` / `high-quality` / `crisp-edges` / `pixelated`). **C** — `pixelated` critical for game pixel art.
- Format support driven by Bevy asset pipeline (PNG, JPEG, WebP, AVIF, KTX2, etc.). **F**

**Video / audio**
- Video element (controls, autoplay, muted, loop, poster, preload, playsinline). **E**
- Audio element. **E**
- Multiple sources / format negotiation. **E**
- Captions / subtitles via VTT (track element analogue). **C** — captioning is an a11y requirement (WCAG 1.2.2).
- Picture-in-picture, fullscreen control. **E**

**Programmatic rendering surfaces**
- Render-to-texture surfaces (Buiy nodes drawn on a Bevy texture; usable for in-world UI, mini-maps, custom drawing surfaces). **C**
- Direct integration with Bevy's render targets. **C**

**Vector graphics**
- Vector image rendering (SVG-equivalent) — likely via lyon or comparable. **C**
- Inline vector primitives (rect, circle, ellipse, line, polyline, polygon, path, group). **C**
- Vector filters (feGaussianBlur, feColorMatrix, feMorphology, feTurbulence, feDisplacementMap). **E**
- External SVG via image asset (rasterized). **C**

**Out:** iframe equivalent (no nested document model), MathML, embedded objects/applets. **O**

### 3.10 Widget catalog (APG patterns)

Every widget below ships, by default, with: APG keyboard contract, AccessKit role + name source + states, theme-token consumption, `:focus-visible` behavior, forced-colors fallback (no shadow-only affordances), reduced-motion fallback, RTL mirroring, ≥24×24 hit target (WCAG 2.5.8), per-widget verification suite (Section 3.15).

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
- Breadcrumb. **C**
- Tree. **C**
- Treegrid. **E**

**Containers & overlays**
- Dialog (modal + non-modal). **F**
- Alert Dialog. **F**
- Popover (light dismiss). **F**
- Tooltip. **F**
- Disclosure (button + content). **F**
- Accordion (incl. exclusive accordion via `name` attribute analogue). **F**
- Window splitter. **C**

**Display & feedback**
- Progressbar (determinate + indeterminate). **F**
- Meter. **C**
- Alert (live region, `role=alert`). **F**
- Status (live region, `role=status`). **F**
- Toast / Snackbar (live region with auto-dismiss + WCAG 2.2.3 compliance). **F**
- Carousel. **E**
- Feed (live-loading list). **E**

**Tabular data**
- Table (semantic). **C**
- Grid (data grid with cell navigation). **C**
- Sortable / filterable extensions. **C**

Each widget's APG keyboard contract is enumerated in the per-widget sub-spec (Section 4 — `buiy-widget-catalog-design`).

### 3.11 Accessibility (ARIA + WCAG 2.2)

**ARIA roles taxonomy** — full enumeration, mapped to AccessKit `Role`.

- **Landmarks (8):** banner, complementary, contentinfo, form, main, navigation, region, search. **F**
- **Document structure (~38):** article, blockquote, caption, cell, columnheader, definition, deletion, document, emphasis, feed, figure, generic, group, heading, img, insertion, list, listitem, mark, math, meter, none/presentation, note, paragraph, row, rowgroup, rowheader, separator (non-focusable), strong, subscript, superscript, suggestion, table, term, time, toolbar, tooltip. **F**
- **Standalone widgets (20):** button, checkbox, gridcell, link, menuitem, menuitemcheckbox, menuitemradio, option, progressbar, radio, scrollbar, searchbox, separator (focusable), slider, spinbutton, switch, tab, tabpanel, textbox, treeitem. **F**
- **Composite widgets (9):** combobox, grid, listbox, menu, menubar, radiogroup, tablist, tree, treegrid. **F**
- **Live region (5):** alert, log, marquee, status, timer. **F** (marquee at **E**)
- **Window (2):** alertdialog, dialog. **F**

**ARIA states & properties**

- **Widget states:** `aria-busy`, `aria-checked`, `aria-disabled`, `aria-expanded`, `aria-hidden`, `aria-invalid`, `aria-pressed`, `aria-selected`. **F**
- **Widget properties:** `aria-autocomplete`, `aria-haspopup`, `aria-label`, `aria-level`, `aria-modal`, `aria-multiline`, `aria-multiselectable`, `aria-orientation`, `aria-placeholder`, `aria-readonly`, `aria-required`, `aria-sort`, `aria-valuemax`, `aria-valuemin`, `aria-valuenow`, `aria-valuetext`. **F**
- **Live region:** `aria-live`, `aria-atomic`, `aria-relevant`, `aria-busy`. **F**
- **Drag/drop:** deprecated in ARIA 1.2 — not implemented. **O**
- **Relationships:** `aria-activedescendant`, `aria-colcount`, `aria-colindex`, `aria-colindextext`, `aria-colspan`, `aria-controls`, `aria-describedby`, `aria-description`, `aria-details`, `aria-errormessage`, `aria-flowto`, `aria-labelledby`, `aria-owns`, `aria-posinset`, `aria-rowcount`, `aria-rowindex`, `aria-rowindextext`, `aria-rowspan`, `aria-setsize`. **F**
- **Global:** `aria-current`, `aria-keyshortcuts`, `aria-roledescription`, `aria-braillelabel`, `aria-brailleroledescription`. **C**

**Accessible Name and Description Computation (ACCNAME 1.2)**
- Full algorithm implemented in `buiy_core`. **F**
- Name from `aria-labelledby` > `aria-label` > host-language label > content > `title`. **F**
- Description from `aria-describedby` > `aria-description` > host-language > `title`. **F**
- Hidden subtree exclusion rules. **F**

**Live regions and announcements**
- Politeness levels (off / polite / assertive). **F**
- `aria-atomic`, `aria-busy`, `aria-relevant`. **F**
- `role=status` / `role=alert` / `role=log` / `role=timer`. **F** (`role=marquee` **E**)
- Global announcer service for ad-hoc announcements. **F**

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

**WCAG 2.2 Success Criteria — full A/AA enumeration committed as floor.** Each SC is mapped to either an automated CI check (Section 3.15), a runtime-honored constraint (e.g., reduced-motion), or a documented design constraint. AAA SCs are aspirational. The WCAG-SC mapping table is owned by the verification sub-spec (`buiy-verification-design`) since each SC's enforcement strategy varies.

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
- Signal / computed / effect layer — deferred sub-spec. **E**
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

The verification subsystem makes "every claim Buiy makes is automated" a first-class commitment. Detail belongs in `buiy-verification-design`; the inventory below enumerates the floor.

**Test categories**

1. **Unit tests** — every component, every layout calculation, every event handler, every state machine. Standard Rust `cargo test`. **F**
2. **Visual regression tests** — golden image diff per widget × state × theme variant × viewport size. Bevy screenshot system + image diff (`image-compare` or similar). **F**
3. **AccessKit tree snapshot tests** — golden JSON diff per widget × state. Catches role, name, description, states, relationships regressions. **F**
4. **Layout snapshot tests** — golden Taffy output per layout fixture. **F**
5. **Synthesized input replay** — keyboard, pointer, touch, gamepad, IME composition events injected as Bevy events; assert resulting state. **F**
6. **APG keyboard-contract conformance suite** — every APG pattern, every documented key, every state transition asserted. **F**
7. **WCAG 2.2 testable-SC suite** — programmatic checks for each machine-testable SC (1.4.3 contrast, 1.4.10 reflow, 1.4.11 non-text contrast, 1.4.12 text spacing, 2.4.7 focus visible, 2.5.8 target size, 4.1.2 name/role/value, etc.). **F**
8. **Contrast linter** — every theme, every token combination, both WCAG 2 and APCA, run at theme load and in CI. **F**
9. **Hit-target linter** — every interactive widget rendered with hit area ≥24×24 across all viewport sizes. **F**
10. **Property tests / fuzzing** — layout, text shaping, focus traversal, BiDi caret traversal. Invariants like "focus tree is reachable from any starting node," "AccessKit tree has no orphans," "every focusable node has an accessible name." **F**
11. **Hot-reload validation** — modify a `.bsn` file or theme asset, assert live tree updates correctly without leaks. **F**
12. **Performance regression** — frame-time budgets per scene, layout time, render time, AccessKit tree update time. CI alerts on regressions. **F**
13. **Memory leak tests** — long-running scenarios verify atlas reuse, entity cleanup, theme asset release. **F**
14. **Cross-platform CI matrix** — Windows / macOS / Linux for AT-SPI; Android (where AccessKit's adapter allows headless testing); iOS coverage tracks AccessKit upstream. **F**
15. **AccessKit consumer-driven simulation** — `accesskit_consumer` simulates an AT against the tree without a real screen reader. CI verifies that the tree shape produces correct AT-side observations. **F**

**What's verified per widget**
- Visual: rendered output matches golden in every theme variant, multiple viewport sizes.
- AccessKit: tree shape, role, name, description, states, relationships match golden.
- Keyboard: every APG-mandated key produces correct state transition.
- Pointer: hover/active/click/focus states correct.
- Focus: tab order through component correct; focus ring rendered correctly.
- Forced-colors: visual fallback uses system colors; no shadow-only affordances.
- Reduced-motion: animations short-circuit.
- RTL: layout mirrored.
- Hit target: ≥24×24 at all viewport sizes.
- Disabled state: not focusable, not announced as interactive.
- Inert subtrees: removed from tab order + AccessKit.

**What's verified at the system level**
- Theme contrast linter passes.
- Forced-colors compatibility scan.
- Memory budgets (no atlas/entity leaks over a long-running fixture).
- Performance budgets (frame time, layout time, render time, AccessKit update time).
- Hot-reload preserves state.

**CI policy**
- Runs on every PR.
- No human approval gate beyond standard PR review.
- Failure blocks merge.
- Cross-platform matrix runs in parallel.

**Tooling**
- `accesskit_consumer` — simulates an AT consumer for headless screen-reader-equivalent testing.
- Bevy's screenshot system + an image-diff crate — visual regression.
- `proptest` — property-based testing.
- `buiy_verify` — Buiy's own test harness crate; consumed by every other Buiy crate as `dev-dependency`; usable by downstream Buiy users to test their own widgets.

**Explicitly out of scope (still recommended manually but NOT a CI gate)**
- Real screen-reader output verification (NVDA, JAWS, VoiceOver, etc. produce SR-specific utterances). The pipeline verifies the AccessKit tree is correct; correct tree → correct SR output, modulo SR-specific bugs that are upstream from us. Real SR testing remains a release-time sanity check, not a CI gate.
- Subjective visual quality. Designers verify, machines do not.

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

- Buiy and bevy_ui can coexist in one app — separate trees, separate render passes, separate focus, separate AccessKit trees that AccessKit composes per window. **F**
- Within one tree, you pick Buiy or bevy_ui. No mixing. **F**
- Buiy components do not extend `bevy_ui::Node`. **F**
- Migration from bevy_ui → Buiy is by replacement, not extension. **F**

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
| `buiy-render-pipeline-design` | Render passes, top-layer compositing, clipping, filters, blend modes, atlasing, color management. |
| `buiy-layout-design` | Taffy integration, anchor positioning, container queries, writing-mode integration. |
| `buiy-text-rendering-design` | cosmic-text integration, atlas management, font registration, fallback. |
| `buiy-text-editing-design` | IME composition, BiDi caret, undo/redo, multi-line, rich-text edit surface. |
| `buiy-focus-model-design` | Focus tree, `:focus-visible`, traps, restoration, roving tabindex, gamepad spatial nav. |
| `buiy-accessibility-design` | AccessKit tree construction, decomposed components, ACCNAME 1.2, live regions. |
| `buiy-theme-tokens-design` | Semantic tokens, theme assets, variants, OS-pref binding, contrast linter. |
| `buiy-widget-catalog-design` | APG patterns shared infrastructure; per-widget specs nest as multi-file children. |
| `buiy-animation-design` | Transitions, keyframes, layout transitions, springs, reduced-motion gating. |
| `buiy-forms-design` | Form state machine, constraint validation, validation pseudo-classes. |
| `buiy-input-events-design` | Pointer, keyboard, touch, gamepad, IME, drag-and-drop. |
| `buiy-i18n-design` | BiDi, vertical writing, ICU, locale-aware formatters, calendar/numbering systems. |
| `buiy-3d-anchored-ui-design` | Billboards, worldspace UI, render-to-texture, hit-testing. |
| `buiy-verification-design` | Automated pipeline, harness API, WCAG-SC mapping table, CI matrix. |
| `buiy-devtools-design` | Inspector, overlays, contrast checker, focus visualizer, theme editor. |
| `buiy-bsn-integration-design` | BSN authoring helpers, decomposed-component conventions, hot-reload semantics. |

Each sub-spec gets one or more plans (`docs/plans/`) for implementation.

## 5. Open questions

- **Final crate split.** Single crate vs multi-crate workspace; if multi-crate, the exact partition. The spec commits to modular subsystems with clean boundaries; the partition can change.
- **Reactivity layer.** Observers + change detection only in v1. Whether to add a signal/computed/effect primitive in a follow-up sub-spec is open.
- **CSS-flavored stylesheet.** Never, or as a future layer above tokens? bevy_flair sets one precedent; the right answer depends on user demand.
- **Date/time pickers — Buiy-owned vs OS-delegated.** Buiy-owned per APG gives consistent visuals; OS-delegated is lighter. Spec defaults to Buiy-owned (consistency), but this is reversible.
- **WCAG 2.2 SC enforcement strategy.** Per-SC: automated CI check, runtime-honored constraint, or documented design constraint. The mapping table is owned by `buiy-verification-design`.
- **3D-anchored UI prioritization.** The renderer is ours and `Transform` works, so it's unblocked. Whether `buiy_3d` is concurrent with foundation work or strictly deferred is a planning choice.
- **Coexistence policy with `bevy_feathers` / `bevy_ui_widgets`.** Coexistence at the app level is committed; whether Buiy ships migration adapters from bevy_ui widgets is open.
- **Performance budgets.** What frame-time budget Buiy is allowed for layout + render + AccessKit update is open and lives in `buiy-verification-design`.
- **Platform support staging.** All platforms (Windows / macOS / Linux / Android / iOS / web) at v1, or staged?
- **Hot-reload of components (not just themes).** In scope as part of `buiy-bsn-integration-design`?
- **Render-to-texture surface API contract.** Feeds `buiy_3d`; the boundary is open.
- **Animation library substrate.** Roll our own springs, depend on `bevy_animation`, or wrap an existing crate?
- **OS spellchecker integration.** Where the OS exposes a spellchecker, Buiy uses it; where not, software fallback. The fallback library choice is open.
- **Real screen-reader testing in CI.** Currently out of CI (manual at release). If this becomes feasible (e.g., headless NVDA via vmnv tools), it becomes a CI gate.

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
