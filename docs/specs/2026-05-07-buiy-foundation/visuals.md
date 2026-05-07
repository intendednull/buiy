# Feature inventory — visuals

**Parent:** [README.md](README.md)

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason). See [README.md § Tier legend](README.md#tier-legend).

## 3.1 Document model and component hierarchy

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

## 3.2 Layout

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

## 3.3 Visual styling and rendering

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
