# Feature inventory — media & widgets

**Parent:** [README.md](README.md)

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason). See [README.md § Tier legend](README.md#tier-legend).

## 3.9 Media and graphics

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

## 3.10 Widget catalog (APG patterns)

Every widget below ships, by default, with: APG keyboard contract, AccessKit role + name source + states, theme-token consumption, `:focus-visible` behavior, forced-colors fallback (no shadow-only affordances), reduced-motion fallback, RTL mirroring, ≥24×24 hit target (WCAG 2.5.8), and coverage by the verification fixture matrix in [verification.md](verification.md) (gates 2 — visual regression, 3 — AccessKit tree snapshot, 4 — announcement output, 7 — APG keyboard contract). Per-widget detail (exact keyboard contract, `aria-haspopup` / `aria-current` value emitted, name source) lives in `buiy-widget-catalog-design`.

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
- Textbox (single-line + multi-line — see [text.md § 3.5](text.md)). **F**
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
- Anchored popover — popover + anchor positioning ([visuals.md § 3.2](visuals.md)). **F**
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

Each widget's APG keyboard contract is enumerated in the per-widget sub-spec (`buiy-widget-catalog-design`).
