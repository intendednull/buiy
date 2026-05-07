# Feature inventory — text

**Parent:** [README.md](README.md)

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason). See [README.md § Tier legend](README.md#tier-legend).

## 3.4 Typography

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

**Pseudo-elements for text** — see canonical enumeration in [interaction.md § 3.7 Pseudo-elements](interaction.md#37-events-and-input-handling). Cross-references for text-specific pseudo-elements: `::selection` (**F**), `::placeholder` (**F**), `::marker` (**C**), `::first-letter` / `::first-line` (**E**), `::spelling-error` / `::grammar-error` (**E**).

## 3.5 Text editing

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
