# Widget Catalog — Exact-Values Reference

**Status:** Source of truth for the pixel-parity reimplementation.
**Source:** `docs/reference-designs/widget-catalog/Widget Catalog.dc.html`
**Date:** 2026-06-25

This document is the **single source of truth** for the Buiy reimplementation of
the Claude "Widget Catalog" design. Implementers author against the values here
and **never re-derive from the HTML**. Every distinct value in the design appears
below.

The source has **two** value sources, both mined here:

- **Inline-style HTML** — the static shell, screen layouts, header/rail/inspector/
  status chrome, and the screen scaffolds.
- **`<script type="text/x-dc">` JS** (`renderVals()`) — builds per-element style
  objects at runtime: todo-row styles, filter/segmented/kind buttons, scroll-row
  styles, menu-item styles, switch track/thumb, slider, disclosure chevrons,
  accent swatches, confirm buttons, the accent ramp (`applyAccent`), etc.

Values sourced from the JS are tagged **(JS)**; values from inline HTML are tagged
**(HTML)**. Line references are to the source file.

> **Accent is runtime-themeable.** Four accent options exist; the *default* is
> `#5b86f5` (blue). The JS sets four CSS custom properties from the chosen accent
> (`--ac`, `--ac2`, `--acsoft`, `--acglow`) via `applyAccent()`. Wherever a value
> below reads `--ac`/`--ac2`/`--acsoft`/`--acglow`, it resolves from the **current
> accent** (see §1 ramp). The inline fallbacks in the HTML (e.g.
> `var(--ac,#5b86f5)`) are the **blue-default** values.

---

## 1. Color tokens

### 1.1 Token taxonomy — every distinct literal color

Every distinct hex / rgba in the design, mapped to a proposed semantic token.

#### `color.surface.*` — backgrounds / fills

| Token | Value | Use site (examples) |
|---|---|---|
| `color.surface.app` | `#0b0c0e` | body bg; root flex container; viewport `<main>` bg (HTML 17, 31, 85) |
| `color.surface.chrome` | `#0d0e11` | header, rail `<nav>`, inspector `<aside>`, status `<footer>`, viewport-header bg (HTML 34, 57, 402, 449) |
| `color.surface.chrome-translucent` | `#0d0e11cc` | viewport header bg (80% alpha over `backdrop-filter:blur(6px)`) (HTML 87) |
| `color.surface.card` | `#16181c` | todo card, scroll panel, menu card, modal card, showcase cards, search field, swatch default border-fill, chip bg (HTML 104, 149, 154, 188, 248, 309…; JS rows tinted from this) |
| `color.surface.inset` | `#121417` | header chips, kbd, todo footer/header strips, segmented/kind track, scroll table header+footer strips, stepper/build buttons, modal inputs/footer, swatch nested (HTML 40, 43, 110, 126…) |
| `color.surface.raised` | `#1a1d22` | menu dropdown card; toast (HTML 203, 461) |
| `color.surface.raised-alt` | `#1e2127` | menu header icon tile; menu button (open state); slider/progress track; segmented hover-ish (HTML 190, 333, 370, 709 JS) |
| `color.surface.danger` | `#391b1a` | modal delete warning-icon tile (HTML 286) |
| `color.surface.danger-soft` | `#1a1213` | "Delete" trigger button bg (modal screen) (HTML 237) |
| `color.surface.transparent` | `transparent` | inactive nav button bg, inactive filter/seg/kind bg, default rows (JS 560, 586, 603…) |

#### `color.border.*`

| Token | Value | Use site (examples) |
|---|---|---|
| `color.border.subtle` | `#1c1f24` | all 1px chrome dividers: header bottom, rail right, viewport-header bottom, inspector left, status top, card inner dividers, todo row borders (HTML 34, 57, 87, 105, 402, 449; JS 575) |
| `color.border.subtle-2` | `#14161a` | scroll-row bottom border (JS 603) |
| `color.border.default` | `#262a31` | header-chip / kbd / badge / card / search / segmented / swatch-default / stepper-base borders (HTML 40, 43, 102, 104, 149, 188…) |
| `color.border.strong` | `#2c313a` | menu dropdown border, modal card border, modal inputs, stepper/build/cancel buttons, toast border (HTML 203, 248, 254, 263…) |
| `color.border.strong-2` | `#3a4150` | menu button border (open state) (JS 709) |
| `color.border.muted` | `#39404a` | unchecked todo checkbox border; scrollbar thumb hover fill (JS 578; HTML 22) |
| `color.border.danger` | `#3a2422` | "Delete" trigger button border (HTML 237) |

#### `color.text.*` — ink ladder

| Token | Value | Use site (examples) |
|---|---|---|
| `color.text.primary` | `#f1f3f6` | brand "buiy", screen H1/H2, input text, todo draft, stepper count, modal labels, switch-on labels (HTML 17, 39, 101…) |
| `color.text.bright` | `#e7eaef` | active todo item text; menu-item label (JS 580, 618) |
| `color.text.secondary` | `#c2c8d2` | github icon, rail name (inactive), stat values, scroll type cell, cancel button, modal body (HTML 48, 78, 168, 295; JS 564) |
| `color.text.muted` | `#868d99` | header chips, stat keys, search placeholder hint, viewport size badge, filter-off, modal sub, scroll selected label, inspector uppercase headings (HTML 43, 77, 88, 91…) |
| `color.text.faint` | `#6f7783` | rail desc, scroll name cell, scroll ms (normal), inspector desc, switch desc, menu file-path sub (HTML 67, 169, 195; JS 600) |
| `color.text.dim` | `#555c67` | rail section labels (uppercase), idx-off, kbd glyph, todo-empty, scroll idx/state-label/window-footer, faint captions, disclosure tag (HTML 58, 63, 110, 124…) |
| `color.text.dimmer` | `#3a4049` | todo caption ("double-click semantics…"), `clear done` disabled text, status-bar separators `\|`, todo line-through decoration color (HTML 136; JS 580, 693) |
| `color.text.prompt` | `$ ` prefix `#555c67` | header `$ ` shell prompt prefix (HTML 43) — same as `color.text.dim` |
| `color.text.danger` | `#f0655b` | "Delete" button text, delete-icon tile, menu Delete item, confirm-delete error states (HTML 237, 286; JS 618) |
| `color.text.danger-dim` | `#7a3a36` | Delete menu-item `kbd` color (danger kbd) (JS 616) |

#### `color.text.on-accent` (special)

| Token | Value | Use site |
|---|---|---|
| `color.text.on-accent` | `#07101f` | text/icon on filled-accent surfaces: logo bar stroke, checked-checkbox check, active filter/seg/kind label, "New widget" button text, create-confirm text (HTML 37, 233; JS 579, 586, 624, 723) |

#### `color.accent.*` — four selectable options (default = blue)

| Token | Value | Name | Notes |
|---|---|---|---|
| `color.accent.blue` | `#5b86f5` | Blue | **default** `--ac`; inline fallback everywhere |
| `color.accent.green` | `#45c07d` | Green | option 2 |
| `color.accent.violet` | `#b98aff` | Violet | option 3 |
| `color.accent.coral` | `#f0655b` | Coral | option 4 (== `color.status.error`) |

Derived per-accent values: see §1.2.

#### `color.status.*`

| Token | Value | Use site |
|---|---|---|
| `color.status.ok` | `#45c07d` | status-bar "ready" dot+text, OK scroll state, `Checkbox`/`Button` type dots, completed counts, switch-on inspector colors (HTML 450; JS 598, 660…) — same hex as `accent.green` |
| `color.status.warn` | `#d7a23f` | WARN scroll state, ms > 1.4ms color, `Input`/`Stepper` type dots (JS 474, 598, 601) |
| `color.status.error` | `#f0655b` | ERR scroll state, `Scroll` type dot (JS 475, 598) — same hex as `accent.coral` |

#### Type-dot palette (JS `TYPES`, line 473–476) — entity-tree node colors

These reuse accent/status/text tokens; listed for completeness (scroll-list dots
+ inspector "Composed of" chip dots):

| Node type | Dot color | Token |
|---|---|---|
| Stack, Row, Grid | `#5b86f5` | `color.accent.blue` |
| Text | `#868d99` | `color.text.muted` |
| Button, Icon | `#45c07d` | `color.status.ok` |
| Image | `#b98aff` | `color.accent.violet` |
| Input | `#d7a23f` | `color.status.warn` |
| Scroll | `#f0655b` | `color.status.error` |
| Spacer | `#555c67` | `color.text.dim` |
| TextInput (inspector) | `#d7a23f` | `color.status.warn` |
| Checkbox (inspector) | `#45c07d` | `color.status.ok` |

#### `color.misc.*` — pure white / dotted bg

| Token | Value | Use site |
|---|---|---|
| `color.misc.white` | `#fff` (`#ffffff`) | switch thumb fill (JS 627, 638) — **NB:** the slider thumb uses `#f1f3f6`, *not* white (HTML 335) |
| `color.misc.dot-bg` | `#16181c` | viewport dotted radial-gradient dot color (== `surface.card`) (HTML 85) |

#### Specials (rgba, scrim, selection, scrollbar)

| Token | Value | Use site |
|---|---|---|
| `color.scrim` | `rgba(4,5,7,.66)` | modal backdrop dim (HTML 247) |
| `color.selection.bg` | `rgba(91,134,245,.32)` | `::selection` highlight (HTML 18) — **fixed blue**, not accent-derived |
| `color.scrollbar.track` | `transparent` | webkit scrollbar track (HTML 20) |
| `color.scrollbar.thumb` | `#262a31` | scrollbar thumb (== `border.default`), w/ 3px transparent border + `background-clip:content-box` (HTML 21) |
| `color.scrollbar.thumb-hover` | `#39404a` | scrollbar thumb hover (== `border.muted`) (HTML 22) |
| `color.accent-bar.logo-ring` | `rgba(91,134,245,.35)` | logo `0 0 0 1px` inset ring (HTML 36) — **fixed blue**, see §2 |

> **Ambiguity flag:** `::selection` (`rgba(91,134,245,.32)`) and the logo's
> `0 0 0 1px rgba(91,134,245,.35)` ring are hard-coded **blue** in the HTML even
> though the accent is themeable. Parity decision needed: keep them fixed blue
> (matches source literally) or re-derive from `--ac` (matches design *intent*).
> The source renders them fixed blue; recommend matching the source.

### 1.2 Accent ramp — derived values

The JS `applyAccent(hex)` (lines 515–524) computes the ramp from the chosen
accent and writes four CSS vars:

```
r,g,b   = parse hex channels
lighten(v) = min(255, round(v + (255-v)*0.22))      // +22% toward white
--ac    = hex
--ac2   = '#' + hex(lighten(r),lighten(g),lighten(b))
--acsoft= rgba(r,g,b,.16)
--acglow= rgba(r,g,b,.55)
```

Computed values per accent option:

| Accent | `--ac` | `--ac2` (computed, authoritative) | `--acsoft` | `--acglow` |
|---|---|---|---|---|
| **Blue** (default) | `#5b86f5` | `#7fa1f7` | `rgba(91,134,245,.16)` | `rgba(91,134,245,.55)` |
| **Green** | `#45c07d` | `#6ece9a` | `rgba(69,192,125,.16)` | `rgba(69,192,125,.55)` |
| **Violet** | `#b98aff` | `#c8a4ff` | `rgba(185,138,255,.16)` | `rgba(185,138,255,.55)` |
| **Coral** | `#f0655b` | `#f3877f` | `rgba(240,101,91,.16)` | `rgba(240,101,91,.55)` |

> The **inline HTML fallback** for `--ac2` is hard-coded `#6f96ff` (e.g. HTML
> 36, 331) — a *different, hand-picked* value that only shows if the JS never
> runs. The **JS-computed** `--ac2` (above) is what renders at runtime. Use the
> JS-computed values for parity; the inline `#6f96ff` is a dead fallback.

Authoritative JS-computed `--ac2` (full channel math, for verification):

| Accent | channels | `--ac2` computed |
|---|---|---|
| Blue `#5b86f5` | (91,134,245) → (127,161,247) | **`#7fa1f7`** |
| Green `#45c07d` | (69,192,125) → (110,206,154) | **`#6ece9a`** |
| Violet `#b98aff` | (185,138,255) → (200,164,255) | **`#c8a4ff`** |
| Coral `#f0655b` | (240,101,91) → (243,135,127) | **`#f3877f`** |

> Detailed channel math (verified against the JS `lighten` + `Math.round`):
> - Blue: r 91→`91+164·.22=127.08`→127=`7f`; g 134→`134+121·.22=160.62`→161=`a1`; b 245→`245+10·.22=247.2`→247=`f7`; → `#7fa1f7`
> - Green: r 69→`69+186·.22=109.92`→110=`6e`; g 192→`192+63·.22=205.86`→206=`ce`; b 125→`125+130·.22=153.6`→154=`9a`; → `#6ece9a`
> - Violet: r 185→`185+70·.22=200.4`→200=`c8`; g 138→`138+117·.22=163.74`→164=`a4`; b 255→255=`ff`; → `#c8a4ff`
> - Coral: r 240→`240+15·.22=243.3`→243=`f3`; g 101→`101+154·.22=134.88`→135=`87`; b 91→`91+164·.22=127.08`→127=`7f`; → `#f3877f`
>
> **This block is the authoritative recomputation** — use `#7fa1f7`, `#6ece9a`,
> `#c8a4ff`, `#f3877f`. (Note 127 = `0x7f`, not `0x7c`.)

---

## 2. Box-shadow catalog

Every unique `box-shadow` in the design. Format: `offset-x offset-y blur spread
color`.

| Name | Value | Use site |
|---|---|---|
| `shadow.logo` | `0 0 0 1px rgba(91,134,245,.35), 0 6px 18px -6px var(--acglow)` | header logo tile (two layers: 1px blue inset-like ring + accent glow) (HTML 36) |
| `shadow.card` | `0 12px 32px -16px rgba(0,0,0,.7)` | todo card, scroll panel, menu card (HTML 104, 154, 188) |
| `shadow.modal` | `0 30px 70px -20px rgba(0,0,0,.85)` | modal dialog card (HTML 248) |
| `shadow.menu` | `0 16px 40px -12px rgba(0,0,0,.8)` | menu dropdown, toast (HTML 203, 461) |
| `shadow.accent-button` | `0 8px 20px -8px var(--acglow)` | "New widget" button, create-confirm button (HTML 233; JS 723) — uses `--acglow` (`rgba(ac,.6)` per inline fallback; ramp writes `.55`) |
| `shadow.danger-button` | `0 8px 20px -8px rgba(207,58,54,.5)` | delete-confirm button (JS 723) |
| `shadow.slider-preview` | `0 10px 26px -10px var(--acglow)` | 88×88 slider preview square (HTML 331); inline fallback `rgba(91,134,245,.6)` |
| `shadow.slider-thumb` | `0 2px 6px rgba(0,0,0,.5)` | slider thumb (15×15 dot) (HTML 335) |
| `shadow.switch-thumb` | `0 1px 3px rgba(0,0,0,.4)` | switch thumb (modal + showcase) (JS 627, 638) |
| `shadow.blink-dot` | `0 0 0 4px var(--acsoft)` | menu "last action" blink dot (8×8) (HTML 216); `--acsoft`=`rgba(ac,.16)` |
| `shadow.ready-dot` | `0 0 6px #45c07d` | status-bar "ready" dot (7×7) (HTML 450) — fixed green glow |
| `shadow.swatch-selected` | `0 0 0 3px rgba(0,0,0,.4), 0 4px 12px -4px ${hex}` | selected accent swatch — two layers: 3px black ring + colored drop using that swatch's own hex (JS 668) |
| `shadow.swatch-default` | `none` | unselected accent swatch (JS 668) |

> **Ambiguity flag:** the inline `--acglow` fallbacks read `rgba(...,.6)` in
> several sites (HTML 233, 331; JS 723) but the JS ramp writes `--acglow` =
> `rgba(ac,.55)`. At runtime the **`.55` ramp value wins** wherever `--acglow`
> resolves. For blue default that's `rgba(91,134,245,.55)`. Treat `.55` as
> authoritative; the `.6` literals are dead fallbacks.

---

## 3. Border-radius

Radius scale in the design: **2 / 5 / 6 / 7 / 8 / 9 / 10 / 11 / 12 / 14 / 99px**
(99px = pill). Plus asymmetric corners.

| Radius | Token suggestion | Element(s) |
|---|---|---|
| `2px` | `radius.xs` | scroll-row type dot (7×7), inspector chip dot (6×6) (HTML 167, 418) |
| `5px` | `radius.sm` | header "widget catalog" badge, dark chip, viewport size badge, todo `kbd` `↵`, header `$` code chip (HTML 40, 91, 110, 43) |
| `6px` | `radius.md` | logo tile, github button, header chips, kbd chips, todo delete-btn hover area, filter buttons, segmented/kind buttons, menu cancel `kbd` (HTML 36, 43, 48, 118; JS 586, 624) |
| `7px` | `radius.md-2` | menu-item buttons, modal close button (×) (HTML 254; JS 618) |
| `8px` | `radius.lg` | nav screen buttons, search field, menu header icon tile, modal inputs, stepper buttons, build button, cancel/confirm buttons, accent swatches, menu ⋮ button (HTML 149, 190, 263…; JS 560, 668, 709) |
| `9px` | `radius.lg-2` | "New widget"/"Delete" trigger buttons, segmented/kind track, modal warning-icon tile (HTML 233, 237, 267, 286) |
| `10px` | `radius.xl` | menu dropdown card, toast (HTML 203, 461) |
| `11px` | `radius.xl-2` | (only as bottom corners — see asymmetric) |
| `12px` | `radius.2xl` | todo card, scroll panel, menu card, showcase cards (HTML 104, 154, 188, 309) |
| `14px` | `radius.3xl` | modal dialog card (HTML 248) |
| `99px` | `radius.pill` | nav active accent left-bar, todo "remaining" badge, todo checkbox (20×20), switch track (40×23) + thumb (17×17), slider track/fill/thumb, progress track/fill, blink dot, ready dot (HTML 102, 333; JS 561, 578, 626…) |

**Asymmetric:**

| Value | Element |
|---|---|
| `0 0 11px 11px` | menu screen footer strip (bottom corners only — top is flush against the divider) (HTML 215) |

> Note: `radius.pill` (99px) is the design's "fully rounded" idiom; in Buiy this
> maps to a max-radius / circle constraint, not a literal 99px on small elements.

---

## 4. Typography

**Families:** `Geist` (UI sans) and `Geist Mono` (monospace). Body fallback stack
`'Geist','Helvetica Neue',Arial,sans-serif` (HTML 17). Weights used: **400, 450,
500, 600, 700** (700 declared in the font import but unused in the rendered tree).

**Letter-spacing → px conversion** (em·size, for parity since Buiy takes px). The
design uses these em values: `-.025em`, `-.01em`, `.02em`, `.04em`, `.08em`,
`.1em`, `.12em`, `.14em`. px is computed at each element's font-size:

| Element | Family | Size (px) | Weight | Letter-spacing | LS px @ size | Color token | Transform | Decoration |
|---|---|---|---|---|---|---|---|---|
| **Header — brand "buiy"** | Geist Mono | 15 | 600 | -.01em | -0.15 | `text.primary` `#f1f3f6` | — | — |
| **Header — "widget catalog" badge** | Geist Mono | 11 | 500 | .02em | 0.22 | `--ac` (accent) | — | — |
| **Header — `$ cargo run…` chip** | Geist Mono | 11.5 | 500 | — | — | `text.muted` `#868d99` (prefix `$ ` = `#555c67`) | — | — |
| **Header — "dark" theme chip** | Geist Mono | 11 | 500 | — | — | `text.muted` `#868d99` | — | — |
| **Rail — "Screens"/"Stats" section labels** | Geist Mono | 10 | 500 | .14em | 1.40 | `text.dim` `#555c67` | uppercase | — |
| **Rail — nav index "01"…** | Geist Mono | 10.5 | 500 | — | — | active `--ac` / off `#555c67` | — | — |
| **Rail — nav name** | Geist | 13 | 500 | — | — | active `#f1f3f6` / off `#c2c8d2` | — | — |
| **Rail — nav desc** | Geist | 11 | 400 | — | — | `text.faint` `#6f7783` | — | — |
| **Rail — stat key** | Geist | 11.5 | 400 | — | — | `text.muted` `#868d99` | — | — |
| **Rail — stat value** | Geist Mono | 11.5 | 500 | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Viewport header — screen name** | Geist Mono | 12.5 | 500 | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Viewport header — screen path** | Geist Mono | 11 | 400 | — | — | `text.dim` `#555c67` | — | — |
| **Viewport header — size badge** | Geist Mono | 11 | 500 | — | — | `text.muted` `#868d99` | — | — |
| **Todo — H1 "todos"** | Geist | 30 | 600 | -.025em | -0.75 | `text.primary` `#f1f3f6` | — | — |
| **Todo — "N left" badge** | Geist Mono | 12 | 500 | — | — | `text.muted` `#868d99` | — | — |
| **Todo — draft input** | Geist | 15 | 450 | — | — | `text.primary` `#f1f3f6` | — | — (placeholder "What needs doing?") |
| **Todo — `↵` kbd** | Geist Mono | 10 | 500 | — | — | `text.dim` `#555c67` | — | — |
| **Todo — item text (active)** | Geist | 14.5 | 450 | — | — | `text.bright` `#e7eaef` | — | none |
| **Todo — item text (done)** | Geist | 14.5 | 450 | — | — | `text.dim` `#555c67` | — | **line-through**, decoration color `#3a4049` |
| **Todo — "N items left"** | Geist Mono | 11.5 | 500 | — | — | `text.muted` `#868d99` | — | — |
| **Todo — filter buttons (All/Active/Done)** | Geist | 11.5 | 500 | — | — | on `#07101f` / off `#868d99` | — | — |
| **Todo — "Clear done"** | Geist | 11.5 | 500 | — | — | enabled `#868d99` / disabled `#3a4049` | — | — |
| **Todo — empty label** | Geist | 13 | 450 | — | — | `text.dim` `#555c67` | — | — |
| **Todo — caption ("double-click semantics…")** | Geist Mono | 11 | 400 | — | — | `text.dimmer` `#3a4049` | — | — |
| **Scroll — H1 "Entity tree"** | Geist | 18 | 600 | -.01em | -0.18 | `text.primary` `#f1f3f6` | — | — |
| **Scroll — total label** | Geist Mono | 11 | 500 | — | — | `text.muted` `#868d99` | — | — |
| **Scroll — search input** | Geist | 13 | 450 | — | — | `text.primary` `#f1f3f6` | — | (placeholder "Filter nodes…") |
| **Scroll — table header (Index/Node/Frame/State)** | Geist Mono | 10 | 500 | .1em | 1.00 | `text.dim` `#555c67` | uppercase | — |
| **Scroll — row idx** | Geist Mono | 11 | 500 | — | — | `text.dim` `#555c67` | — | — |
| **Scroll — row type** | Geist Mono | 12.5 | 500 | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Scroll — row name** | Geist Mono | 12.5 | 400 | — | — | `text.faint` `#6f7783` | — | — |
| **Scroll — row ms** | Geist Mono | 11.5 | 500 | — | — | normal `#6f7783` / >1.4ms `#d7a23f` | — | — |
| **Scroll — row state (OK/WARN/ERR)** | Geist Mono | 10 | 500 | .04em | 0.40 | OK `#45c07d` / WARN `#d7a23f` / ERR `#f0655b` | — | — |
| **Scroll — footer labels** | Geist Mono | 11 | 500 | — | — | `text.dim` `#555c67` (right label `#868d99`) | — | — |
| **Menu — file name "primary_button.bsn"** | Geist | 14 | 500 | — | — | `text.primary` `#f1f3f6` | — | — |
| **Menu — file sub "crates/… · 1.2 KB"** | Geist Mono | 11.5 | 400 | — | — | `text.faint` `#6f7783` | — | — |
| **Menu — item label** | Geist | 13 | 450 | — | — | `text.bright` `#e7eaef` (danger `#f0655b`) | — | — |
| **Menu — item kbd** | Geist Mono | 10 | 500 | — | — | `#555c67` (danger `#7a3a36`) | — | — |
| **Menu — "last action" label** | Geist Mono | 11.5 | 500 | — | — | `text.muted` `#868d99` | — | — |
| **Menu — last-action value** | Geist Mono | 12 | 500 | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Menu — body paragraph** | Geist | 12 | 400 | — | — | `text.dim` `#555c67`, line-height 1.6 | — | — |
| **Modal — trigger "New widget"/"Delete"** | Geist | 13 | 600 | — | — | New `#07101f` / Delete `#f0655b` | — | — |
| **Modal — hint paragraph** | Geist | 12 | 400 | — | — | `text.dim` `#555c67`, line-height 1.6 | — | — |
| **Modal — title (H2)** | Geist | 16 | 600 | -.01em | -0.16 | `text.primary` `#f1f3f6` | — | — |
| **Modal — subtitle** | Geist | 12.5 | 400 | — | — | `text.muted` `#868d99` | — | — |
| **Modal — field labels (NAME/KIND)** | Geist Mono | 11 | 500 | .08em | 0.88 | `text.muted` `#868d99` | uppercase | — |
| **Modal — name input** | Geist Mono | 13.5 | 450 | — | — | `text.primary` `#f1f3f6` | — | (placeholder "my_widget") |
| **Modal — kind buttons** | Geist | 12 | 500 | — | — | on `#07101f` / off `#868d99` | — | — |
| **Modal — "Register globally" title** | Geist | 13 | 500 | — | — | `text.primary` `#f1f3f6` | — | — |
| **Modal — register sub** | Geist | 11.5 | 400 | — | — | `text.faint` `#6f7783` | — | — |
| **Modal — delete body** | Geist | 13.5 | 450 | — | — | `text.secondary` `#c2c8d2`, line-height 1.55 (code inline `#f1f3f6`) | — | — |
| **Modal — "Esc to close" kbd** | Geist Mono | 10 | 500 | — | — | `text.dim` `#555c67` | — | — |
| **Modal — Cancel button** | Geist | 12.5 | 600 | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Modal — Create/Delete confirm** | Geist | 12.5 | 600 | — | — | create `#07101f` / delete `#fff` | — | — |
| **Showcase — card section labels** | Geist Mono | 10 | 500 | .12em | 1.20 | `text.dim` `#555c67` | uppercase | — |
| **Showcase — slider radius value** | Geist Mono | 12 | 500 | — | — | `--ac` (accent) | — | — |
| **Showcase — segmented buttons** | Geist | 12 | 500 | — | — | on `#07101f` / off `#868d99` | — | — |
| **Showcase — stepper count** | Geist Mono | 20 | 600 | — | — | `text.primary` `#f1f3f6` | — | — |
| **Showcase — stepper +/− buttons** | (icon) | font-size 18 | — | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Showcase — meter progress label** | Geist Mono | 12 | 500 | — | — | `text.muted` `#868d99` | — | — |
| **Showcase — "Run build" button** | Geist | 12 | 600 | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Showcase — disclosure title** | Geist | 13.5 | 500 | — | — | `text.primary` `#f1f3f6` | — | — |
| **Showcase — disclosure tag** | Geist Mono | 10.5 | 500 | — | — | `text.dim` `#555c67` | — | — |
| **Showcase — disclosure body** | Geist | 12.5 | 400 | — | — | `text.muted` `#868d99`, line-height 1.6 | — | — |
| **Inspector — "Inspector" label** | Geist Mono | 10 | 500 | .14em | 1.40 | `text.muted` `#868d99` | uppercase | — |
| **Inspector — widget name** | Geist | 14 | 600 | — | — | `text.primary` `#f1f3f6` | — | — |
| **Inspector — widget desc** | Geist Mono | 11.5 | 400 | — | — | `text.faint` `#6f7783`, line-height 1.5 | — | — |
| **Inspector — section labels** | Geist Mono | 10 | 500 | .12em | 1.20 | `text.dim` `#555c67` | uppercase | — |
| **Inspector — "Composed of" chips** | Geist Mono | 11 | 500 | — | — | `text.secondary` `#c2c8d2` | — | — |
| **Inspector — live-state key** | Geist | 12 | 400 | — | — | `text.muted` `#868d99` | — | — |
| **Inspector — live-state value** | Geist Mono | 12 | 500 | — | — | per-row (accent/status/secondary/dim) | — | — |
| **Status bar — all text** | Geist Mono | 11 | 500 | — | — | `text.dim` `#555c67` base; "ready" `#45c07d`; path `#868d99`; separators `#3a4049`; "buiy 0.3.0" `#868d99` | — | — |
| **Toast — message** | Geist | 12.5 | 500 | — | — | `text.primary` `#f1f3f6` | — | — |

> **Inline `<code>` spans** inside paragraphs use `font-family:'Geist Mono'` at
> the paragraph's size, color `#868d99` (or `#f1f3f6` for the delete-body code
> token) (HTML 221, 242, 289).

> **Line-height:** the design only sets explicit `line-height` on body
> paragraphs (`1.5`, `1.55`, `1.6`); all other text uses the browser default
> (`normal` ≈ 1.2). For parity, single-line UI labels should not add extra
> leading; multi-line bodies use the values noted per row.

---

## 5. Transition & animation catalog

### 5.1 Transitions

| Component | Property | Duration | Easing | Source |
|---|---|---|---|---|
| Nav screen button | `background` | .12s | (default `ease`) | JS 560 |
| Todo checkbox | `all` | .12s | (default `ease`) | JS 578 |
| Switch **track** (modal + showcase) | `background` | .15s | (default `ease`) | JS 626, 637 |
| Switch **thumb** (modal + showcase) | `left` | .15s | `cubic-bezier(.2,.8,.2,1)` | JS 627, 638 |
| Progress/meter fill | `width` | .3s | `cubic-bezier(.2,.8,.2,1)` | HTML 371 |
| Disclosure chevron | `transform` | .15s | (default `ease`) | JS 655 |

> No other elements declare transitions. The generic "`transition: all .12s`"
> pattern the brief mentions appears specifically as the todo-checkbox `all .12s`
> (JS 578); the nav button is `background .12s` (JS 560). There is **no** global
> `* { transition: all .12s }`.

### 5.2 Keyframe animations

| Name | Keyframes | Duration | Timing | Iteration | Applied to | Source |
|---|---|---|---|---|---|---|
| `spin` | `to { transform: rotate(360deg) }` | **not specified inline** | — | — | (declared but **no element uses it** in the rendered tree — there is no spinner element; `building` state shows text "Building…" not a spinner) | HTML 23 |
| `blink` | `0%,55% {opacity:1} 56%,100% {opacity:.25}` | **1.6s** | (default `ease`) | infinite | menu "last action" blink dot (HTML 216) | HTML 28 |
| `ov-in` | `from {opacity:0} to {opacity:1}` | **not inline** | — | — | (overlay fade-in; declared, used via class not present inline — see note) | HTML 24 |
| `menu-in` | `from {opacity:0; transform:translateY(-6px) scale(.98)} to {opacity:1; transform:none}` | **not inline** | — | — | menu dropdown entrance | HTML 25 |
| `modal-in` | `from {opacity:0; transform:translateY(8px) scale(.985)} to {opacity:1; transform:none}` | **not inline** | — | — | modal card entrance | HTML 26 |
| `toast-in` | `from {opacity:0; transform:translateY(8px)} to {opacity:1; transform:none}` | **not inline** | — | — | toast entrance | HTML 27 |

> **Ambiguity flags (durations):**
> - **`spin`** has **no duration anywhere** — neither in the `@keyframes` nor on
>   any element (and no element references `spin`). If a build-spinner is added in
>   the reimplementation, pick a duration (common: `0.8s`–`1s` linear infinite);
>   the source provides none. **Flag: invent a value, document it.**
> - **`menu-in` / `modal-in` / `toast-in` / `ov-in`** declare *keyframes* but the
>   inline elements **do not** set an `animation:` shorthand with a duration
>   (only `blink` is wired up inline, at `1.6s`). In the live React-ish runtime
>   these are presumably applied via class with a CSS default; **CSS
>   `animation-duration` defaults to `0s`** (i.e. no visible animation) unless a
>   stylesheet rule supplies one. The keyframes encode the *intended* from→to.
>   **Recommended entrance durations for parity** (not in source — flag as
>   chosen): `menu-in` ~120ms, `modal-in` ~160ms, `toast-in` ~180ms, `ov-in`
>   ~120ms, all `ease-out`. Document whatever is chosen.

> **Implementation note (parity-prototype Wave D) — the `blink` pulse is now
> LIVE.** Wave C deferred `blink` (the menu "last action" dot + the showcase
> status dot rendered statically at full opacity) because `buiy_core::animation`'s
> `Tween` ran once and removed itself — no infinite loop. Wave D added a `Repeat`
> mode to `Tween` (`Once` / `Loop { count }` / `PingPong { count }`, `count: None`
> = infinite) and the `composites::pulse_blink` helper wires both dots to an
> **infinite `PingPong` `OpacityTween`** opacity `1`→`.25`→`1`, `BLINK_HALF_SECS`
> = 0.8 s each pass (= the source's `1.6s` full cycle). `Opacity < 1` auto-forms an
> effect group (render `effect.rs`), so the dot composites + dims with no manual
> group authoring. Under reduced motion the tween snaps to the steady **bright**
> rest state (`from` = opacity 1.0) and removes itself — a pulse never oscillates
> (§ 5.3). (Single-frame captures don't show the pulse; the live `cargo run` does.)

### 5.3 Reduced motion

The design exposes a "Reduced motion / Disable transitions" switch (`sw.motion`,
showcase) but **does not** wire a `prefers-reduced-motion` media query or any
behavioral hook — it is a *visual demo toggle only* (JS 634, 664). For the
reimplementation, the documented intent is **jump-to-end** (no
transition/animation, final state shown immediately) when reduced-motion is
active. **Flag: behavior is design-intent, not implemented in source.**

---

## 6. Icon catalog

All icons are inline `<svg>` on a **24×24 viewBox** unless noted, `fill="none"`
with stroked paths (except the GitHub icon, which is `fill`). Stroke styling is
per-icon. "Render size" = the `width`/`height` attributes (px).

| # | Icon | viewBox | Render px | Stroke-w | Fill/stroke | Color (token) | Path `d` | Source |
|---|---|---|---|---|---|---|---|---|
| 1 | **Logo bars** | 24×24 | 13×13 | 2.4 | stroke, `linejoin:round` | `#07101f` (on-accent) | `M5 4h7a4 4 0 0 1 0 8H5zM5 12h8a4 4 0 0 1 0 8H5z` | HTML 37 |
| 2 | **Theme moon** | 24×24 | 13×13 | 1.7 | stroke, `linecap+linejoin:round` | `#868d99` | `M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z` | HTML 45 |
| 3 | **GitHub** | 24×24 | 15×15 | — | **fill** (`currentColor`) | `#c2c8d2` (link color) | long mark path (`M12 2C6.48 2 2 6.58 2 12.25c0 4.53…Z`) — GitHub octocat | HTML 49 |
| 4 | **Rail icon — TodoMVC** (check-in-circle) | 24×24 | 17×17 | 1.7 | stroke, round | active `#f1f3f6` / off `#868d99` | `M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5` | JS 550 |
| 5 | **Rail icon — Virtual List** (lines) | 24×24 | 17×17 | 1.7 | stroke, round | active/off as #4 | `M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01` | JS 551 |
| 6 | **Rail icon — Overlay Menu** (vert dots) | 24×24 | 17×17 | 1.7 | stroke, round | active/off as #4 | `M12 6h.01M12 12h.01M12 18h.01` | JS 552 |
| 7 | **Rail icon — Modal Dialog** (window) | 24×24 | 17×17 | 1.7 | stroke, round | active/off as #4 | `M4 5h16v14H4zM4 9h16` | JS 553 |
| 8 | **Rail icon — Controls** (sliders) | 24×24 | 17×17 | 1.7 | stroke, round | active/off as #4 | `M4 8h10M18 8h2M4 16h2M10 16h10M14 5v6M8 13v6` | JS 554 |
| 9 | **Search** (magnifier) | 24×24 | 15×15 | 1.7 | stroke, round | `#555c67` | `M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M20 20l-4-4` | HTML 150 |
| 10 | **Todo — toggle-all chevron** | 24×24 | 16×16 | 2 | stroke, round | `toggleAllColor` (`--ac` when all done & non-empty, else `#555c67`) | `M6 9l6 6 6-6` | HTML 107 |
| 11 | **Todo — checkmark** | 24×24 | 13×13 | 2.4 | stroke, round | `#07101f`, `opacity` 1 if done else 0 | `M4 12.5 9 17.5 20 6.5` | HTML 115 |
| 12 | **Close ×** (todo delete, modal close) | 24×24 | 14×14 | 1.7 | stroke, `linecap:round` | todo-delete `#555c67`; modal-close `#868d99` | `M6 6l12 12M18 6 6 18` | HTML 119, 255 |
| 13 | **Menu ⋮** (vert dots, thick) | 24×24 | **17×17** | 2.4 | stroke, `linecap:round` | open `#f1f3f6` / closed `#868d99` | `M12 6h.01M12 12h.01M12 18h.01` | HTML 200 |
| 14 | **Menu folder** (header tile) | 24×24 | 17×17 | 1.7 | stroke, `linejoin:round` | `--ac` | `M3 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z` | HTML 191 |
| 15 | **Menu item — Open** (external) | 24×24 | 15×15 | 1.7 | stroke, round | inherits item color | `M14 4h6v6M20 4l-9 9M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6` | JS 608 |
| 16 | **Menu item — Rename** (pencil) | 24×24 | 15×15 | 1.7 | stroke, round | item color | `M4 20h4L19 9a2 2 0 0 0-3-3L5 17zM14 7l3 3` | JS 609 |
| 17 | **Menu item — Duplicate** (copy) | 24×24 | 15×15 | 1.7 | stroke, round | item color | `M9 9h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V10a1 1 0 0 1 1-1M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1` | JS 610 |
| 18 | **Menu item — Copy link** | 24×24 | 15×15 | 1.7 | stroke, round | item color | `M9 15l6-6M10.5 6.5 12 5a4 4 0 0 1 6 6l-1.5 1.5M13.5 17.5 12 19a4 4 0 0 1-6-6l1.5-1.5` | JS 611 |
| 19 | **Menu item — Delete / trash** (also delete trigger) | 24×24 | 15×15 (menu) / 16×16 (trigger) | 1.7 | stroke, round | `#f0655b` (menu danger) / trigger inherits | `M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13h10l1-13` | JS 612; HTML 238 |
| 20 | **Modal — plus / "New widget"** (also stepper +) | 24×24 | 16×16 (modal) / 15×15 (stepper) | 2.2 (modal) / 2 (stepper) | stroke, `linecap:round` | inherits | `M12 5v14M5 12h14` | HTML 234, 357 |
| 21 | **Modal — warning triangle** | 24×24 | 19×19 | 1.8 | stroke, round | `#f0655b` | `M12 3 2 20h20zM12 10v4M12 17h.01` | HTML 287 |
| 22 | **Slider preview / inspector gear** | 24×24 | 15×15 | 1.5 | stroke, round | `--ac` | `M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7M19.4 12a7.6 7.6 0 0 0-.1-1.3l2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2.2-1.3l-.4-2.5H10l-.4 2.5a7.6 7.6 0 0 0-2.2 1.3l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0 0 2.6l-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2.2 1.3l.4 2.5h4l.4-2.5a7.6 7.6 0 0 0 2.2-1.3l2.4 1 2-3.4-2-1.6c.07-.43.1-.86.1-1.3` (gear) | HTML 404 |
| 23 | **Stepper −** (minus) | 24×24 | 15×15 | 2 | stroke, `linecap:round` | `#c2c8d2` | `M5 12h14` | HTML 353 |
| 24 | **Disclosure chevron** (right) | 24×24 | 16×16 | 1.9 | stroke, round | open `--ac` / closed `#868d99`; `rotate(90deg)` when open | `M9 5l7 7-7 7` | HTML 382 |
| 25 | **Toast — check-in-circle** | 24×24 | 16×16 | 1.8 | stroke, round | `--ac` | `M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5` | HTML 462 |
| 26 | **⌘ Command glyph** (kbd shortcut, parity-prototype Wave D) | 24×24 | 11×11 | 1.5 | stroke, round | inherits kbd color | `M18 3a3 3 0 0 0-3 3v12a3 3 0 0 0 3 3 3 3 0 0 0 3-3 3 3 0 0 0-3-3H6a3 3 0 0 0-3 3 3 3 0 0 0 3 3 3 3 0 0 0 3-3V6a3 3 0 0 0-3-3 3 3 0 0 0-3 3 3 3 0 0 0 3 3h12a3 3 0 0 0 3-3 3 3 0 0 0-3-3` (Lucide `command`) | JS 610–611 (`⌘D`/`⌘L`) |

> Same path appears in #4 (rail TodoMVC) and #25 (toast) — the check-in-circle
> glyph — and #6/#13 (vertical dots) differ only in render size + stroke-width.
>
> **Icon #26 — the ⌘ Command glyph (parity-prototype Wave D, finding M4).** The
> design authors the literal `⌘` (U+2318) in a Geist Mono `<kbd>` for the `⌘D`
> Duplicate / `⌘L` Copy-link menu shortcuts (JS 610–611) and the `⌘K` showcase kbd,
> relying on the **browser's system-font fallback** to supply the glyph. Buiy's
> font system is **registered-only** (deterministic — Geist / Geist Mono / Fira,
> NONE of which carry U+2318), so the literal **tofus**. The exact-parity-best fix:
> render ⌘ as a real `Icon` (the Wave B3 vector path) — a faithful, crisp Command
> shape that matches the design's other stroke icons with no font dependency. The
> `composites::kbd_content` helper splits a `⌘`-prefixed shortcut into
> `[⌘ icon][letter]`; non-`⌘` kbds (`↵`, `F2`, `⌫` — all in Geist Mono) stay plain
> mono text.

**Non-SVG "icons" (CSS dots — NOT SVG):**

| Dot | Geometry | Fill | Extra | Source |
|---|---|---|---|---|
| **Menu blink dot** ("last action") | 8×8, radius 99px | `--ac` | `box-shadow:0 0 0 4px --acsoft`, `animation:blink 1.6s infinite` | HTML 216 |
| **Status "ready" dot** | 7×7, radius 99px | `#45c07d` | `box-shadow:0 0 6px #45c07d` (no animation) | HTML 450 |
| **Scroll-row type dot** | 7×7, radius 2px | per type (§1.1) | — | HTML 167 |
| **Inspector chip dot** | 6×6, radius 2px | per type (§1.1) | — | HTML 418 |

---

## 7. Per-element layout

### 7.1 Shell chrome (fixed)

| Region | Dimension | Padding / detail | Source |
|---|---|---|---|
| Root container | `100vh × 100%`, column, `overflow:hidden`, bg `#0b0c0e` | — | HTML 31 |
| **Top chrome (header)** | height **52px**, `flex:none` | `padding:0 16px 0 18px`; `gap:18px`; border-bottom 1px `#1c1f24`; bg `#0d0e11`; `align-items:center` | HTML 34 |
| — logo cluster | — | `gap:10px`; logo tile 24×24, radius 6 | HTML 35–38 |
| — header chips | — | each `padding:5px 9px`, gap 6, 1px border `#262a31`, radius 6, bg `#121417` | HTML 43–47 |
| — github button | 30×30 | 1px `#262a31`, radius 6, bg `#121417` | HTML 48 |
| **Screen rail (nav)** | width **248px**, `flex:none`, column | border-right 1px `#1c1f24`; bg `#0d0e11` | HTML 57 |
| — "Screens" label | — | `padding:16px 14px 8px` | HTML 58 |
| — nav list | — | `padding:0 8px`; `gap:2px` | HTML 59 |
| — nav button | full-width | `padding:9px 10px 9px 12px`; `gap:10px`; radius 8 (JS 560) | JS 560 |
| — active accent bar | width **2.5px** | abs `left:0; top:8px; bottom:8px`; radius 99 (JS 561) | JS 561 |
| — nav idx span | width 16px | `flex:none; text-align:left` | HTML 63 |
| — stats block | — | `padding:12px 14px`; border-top 1px; `gap:9px` | HTML 73 |
| **Viewport (main)** | `flex:1`, column | bg `#0b0c0e` + dotted bg (see §7.3) | HTML 85 |
| — viewport header | height **42px**, `flex:none` | `padding:0 16px`; `gap:12px`; border-bottom 1px; bg `#0d0e11cc` + `backdrop-filter:blur(6px)` | HTML 87 |
| — viewport canvas | `flex:1`, `overflow:auto`, `position:relative` | — | HTML 95 |
| **Inspector (aside)** | width **280px**, `flex:none`, column, `overflow-y:auto` | border-left 1px `#1c1f24`; bg `#0d0e11`; conditional on `inspector` prop (default true) | HTML 401–402 |
| — "Inspector" header | — | `padding:16px 16px 10px`; `gap:8px` | HTML 403 |
| — name block | — | `padding:8px 16px 14px`; border-bottom 1px | HTML 408 |
| — "Composed of" / "Live state" / "Accent" sections | — | each `padding:14px 16px`; section-label margin-bottom 11px; border-bottom 1px (Accent has none) | HTML 413, 424, 436 |
| — composed chips | — | `flex-wrap:wrap; gap:6px`; chip `padding:4px 9px` | HTML 415–417 |
| — live-state rows | — | `gap:9px`; baseline space-between | HTML 426–428 |
| — accent swatches | 30×30 each | `gap:8px`; radius 8 | HTML 438; JS 668 |
| **Status bar (footer)** | height **28px**, `flex:none` | `padding:0 14px`; `gap:16px`; border-top 1px; bg `#0d0e11` | HTML 449 |

### 7.2 Per-screen layout

#### Todo (HTML 98–138)

| Element | Value |
|---|---|
| Outer wrap | `max-width:560px; margin:0 auto; padding:48px 24px 64px` |
| Header row | `align-items:flex-end; space-between; margin-bottom:18px` |
| Card | 1px `#262a31`, radius 12, bg `#16181c`, `overflow:hidden`, `shadow.card` |
| Draft row | `gap:12px; padding:14px 16px`; border-bottom 1px `#1c1f24` |
| Toggle-all button | 22×22, `flex:none` |
| Todo row (JS 575) | `gap:12px; padding:12px 14px 12px 16px`; border-bottom 1px `#1c1f24` |
| Checkbox (JS 578) | 20×20, radius 99; border `1.5px` (`#39404a` unchecked / `transparent` checked); bg `--ac` checked / transparent |
| Delete button | 26×26, radius 6 |
| Empty label | `padding:36px 16px; text-align:center` |
| Footer strip | `gap:12px; padding:11px 14px`; border-top 1px; bg `#121417` |
| Filter button (JS 586) | `padding:5px 11px`; radius 6 |
| Caption | `margin-top:14px; text-align:center` |

#### Scroll / Virtual List (HTML 141–182)

| Element | Value |
|---|---|
| Outer wrap | `height:100%`, column, `padding:18px 22px`, `min-height:0` |
| Header row | `gap:12px; margin-bottom:14px` |
| Search field | width **240px**, height **34px**, `padding:0 11px`, `gap:8px`, 1px `#262a31`, radius 8, bg `#16181c` |
| Panel | `flex:1`, 1px `#262a31`, radius 12, bg `#16181c`, column, `shadow.card` |
| Table header | `gap:12px; padding:8px 14px`; border-bottom 1px; bg `#121417`; cols Index `46px`, Node `flex:1`, Frame `66px` (right), State `42px` (right) |
| Scroll body | `flex:1; overflow-y:auto; position:relative` |
| Inner spacer | `position:relative; height:{total·34}px` (virtual) |
| **Row height** | **34px** (`ROWH`, JS 505) |
| **Viewport est.** | **360px** (`VPORT`, JS 505) — used for windowing math |
| Row (JS 603) | abs `top:idx·34px`, `height:34px`, `gap:8px`, `padding:0 12px`; border-bottom 1px `#14161a`; selected bg `--acsoft` + `inset 2.5px 0 0 --ac` |
| Row cols | idx `46px`, indent `depth·13px` (cap depth 3), dot 7×7 r2, type, name `flex:1`, ms `66px` (right), state `42px` (right) |
| Footer | `padding:7px 14px`; border-top 1px; bg `#121417`; space-between |
| **Windowing** | `start=max(0, floor(scrollTop/34)−6)`; `visN=ceil(360/34)+12 = 23`; `end=min(total, start+23)` (JS 593–595) |

#### Menu (HTML 185–227)

| Element | Value |
|---|---|
| Centering wrap | `min-height:100%; center; padding:40px` |
| Card wrap | width **420px**, max 100% |
| Card | 1px `#262a31`, radius 12, bg `#16181c`, `shadow.card` |
| Header | `gap:12px; padding:14px 16px`; border-bottom 1px |
| File icon tile | 34×34, radius 8, bg `#1e2127` |
| Menu ⋮ button (JS 709) | 32×32, radius 8 |
| **Dropdown** | abs `top:38px; right:0`; width **218px**; `z-index:30`; `padding:5px`; 1px `#2c313a`; radius 10; bg `#1a1d22`; `shadow.menu` |
| Menu item (JS 618) | `gap:10px; padding:8px 9px`; radius 7 |
| Footer | `padding:14px 16px; gap:10px`; bg `#121417`; radius `0 0 11px 11px` |
| Blink dot | 8×8, radius 99 (see §6) |
| Scrim (outside-click) | abs `inset:0; z-index:20` (when open) |

#### Modal (HTML 229–301)

| Element | Value |
|---|---|
| Trigger wrap | `min-height:100%; center; padding:40px; column; gap:20px` |
| Trigger buttons | height **40px**, `padding:0 16px`, `gap:8px`, radius 9 |
| Overlay | abs `inset:0; z-index:40; center; padding:24px` |
| Backdrop | abs `inset:0`, bg `rgba(4,5,7,.66)`, `backdrop-filter:blur(2px)` |
| Dialog card | width **440px**, max 100%, 1px `#2c313a`, radius 14, bg `#16181c`, `shadow.modal`, `overflow:hidden`, `z-index:1` |
| Header | `padding:18px 20px 14px`; border-bottom 1px; `gap:12px; align-items:flex-start` |
| Close button | 28×28, radius 7, 1px `#262a31`, bg `#121417` |
| Create body | `padding:18px 20px; column; gap:16px` |
| Field label group | `gap:7px` |
| Name input | height **38px**, `padding:0 12px`, 1px `#2c313a`, radius 8, bg `#121417` |
| Kind track | `gap:4px; padding:3px`; 1px `#262a31`; radius 9; bg `#121417` |
| Kind button (JS 624) | `flex:1; padding:7px 0`; radius 6 |
| Register row | `padding:12px 14px`; 1px `#262a31`; radius 9; bg `#121417` |
| Switch track (JS 626) | 40×23, radius 99 |
| Switch thumb (JS 627) | 17×17, radius 99, `top:3px`, `left` 3px↔20px |
| Delete body | `padding:20px; gap:14px; align-items:flex-start` |
| Delete icon tile | 38×38, radius 9, bg `#391b1a` |
| Footer | `padding:13px 20px; gap:10px`; border-top 1px; bg `#121417`; `justify-content:flex-end` |
| "Esc to close" kbd | `padding:4px 7px`; 1px `#262a31`; radius 6; `margin-right:auto` |
| Cancel button | height **36px**, `padding:0 14px`, 1px `#2c313a`, radius 8, bg `#16181c` |
| Confirm button (JS 723) | height **36px**, `padding:0 16px`, radius 8; bg `--ac` (create) / `#cf3a36` (delete); `shadow.accent-button`/`shadow.danger-button` |

#### Showcase / Controls (HTML 304–395)

| Element | Value |
|---|---|
| Outer wrap | `max-width:880px; margin:0 auto; padding:28px 24px 56px` |
| Grid | `grid-template-columns:1fr 1fr; gap:16px` |
| Card (each) | 1px `#262a31`, radius 12, bg `#16181c`, `padding:16px` |
| Section label margin-bottom | 14px (switch/slider), 12px (segmented/stepper/meter) |
| Switch list | `gap:13px` |
| Switch track (JS 637) | 40×23, radius 99; bg `--ac` on / `#2a2f37` off |
| Switch thumb (JS 638) | 17×17, radius 99, `top:3px`, `left` 3px↔20px, bg `#fff` |
| Slider preview square | 88×88, gradient, radius `{radius}px` (0–40), `shadow.slider-preview` |
| Slider track | height **6px**, radius 99, bg `#1e2127` |
| Slider fill | height 6px, radius 99, bg `--ac`, width `{radiusPct}` |
| Slider thumb | 15×15, radius 99, bg `#f1f3f6`, `shadow.slider-thumb`, `translate(-50%,-50%)` |
| Segmented track | `gap:4px; padding:3px`; 1px `#262a31`; radius 9; bg `#121417` |
| Segmented button (JS 645) | `flex:1; padding:7px 0`; radius 6 |
| Stepper buttons | 34×34, 1px `#2c313a`, radius 8, bg `#121417` |
| Stepper count | `min-width:44px; text-align:center` |
| Meter track | height **8px**, radius 99, bg `#1e2127`, `overflow:hidden` |
| Meter fill | height 100%, radius 99, gradient (90deg), `transition:width .3s …` |
| "Run build" button | `margin-top:12px; width:100%; height:34px`; 1px `#2c313a`; radius 8; bg `#121417` |
| Disclosure card | `grid-column:1 / -1`; `padding:6px 16px` |
| Disclosure header button | `gap:10px; width:100%; padding:13px 0`; left-aligned |
| Disclosure body | `padding:0 0 14px 26px` |
| Disclosure divider (JS 654) | border-bottom 1px `#1c1f24` between items (none on last) |

### 7.3 Dotted background + scrollbar

| Detail | Value | Source |
|---|---|---|
| **Dotted viewport bg** | `background-image:radial-gradient(#16181c 1px, transparent 1px)`; `background-size:22px 22px` | HTML 85 |
| **Scrollbar size** | width/height **10px** | HTML 19 |
| **Scrollbar track** | `transparent` | HTML 20 |
| **Scrollbar thumb** | bg `#262a31`; radius 99; `border:3px solid transparent`; `background-clip:content-box` (→ visible thumb ≈ 4px wide) | HTML 21 |
| **Scrollbar thumb hover** | bg `#39404a`; same 3px border + content-box clip | HTML 22 |

---

## 8. Gradients

Four gradient instances. All stops are **opaque** (`--ac` and `--ac2` are solid
hex, not rgba).

| Name | Definition | Use site | Source |
|---|---|---|---|
| `gradient.accent-150` | `linear-gradient(150deg, var(--ac), var(--ac2))` | header logo tile (24×24); slider preview square (88×88); "New widget" trigger button | HTML 36, 331, 233 |
| `gradient.accent-90` | `linear-gradient(90deg, var(--ac), var(--ac2))` | meter / progress fill | HTML 371 |

Per-accent resolved stops (using authoritative `--ac2` from §1.2):

| Accent | `gradient.accent-*` stops |
|---|---|
| Blue | `#5b86f5 → #7fa1f7` |
| Green | `#45c07d → #6ece9a` |
| Violet | `#b98aff → #c8a4ff` |
| Coral | `#f0655b → #f3877f` |

> The "New widget" / create-confirm buttons use a **flat** `--ac` background
> (JS 723; HTML 233 fill is solid `var(--ac,#5b86f5)`), **not** the gradient —
> only the logo, slider preview, and the two fill bars (meter, and the logo tile)
> use gradients. The accent-button "gradient" the brief lists refers to the
> *glow shadow* (`shadow.accent-button`), the button face itself is flat accent.

---

## 9. Component default state (JS initial `state`, lines 485–503)

For wiring parity (initial render before any interaction):

| Field | Default | Field | Default |
|---|---|---|---|
| `screen` | `'todo'` | `radius` | `14` (→ 35% slider) |
| `todos[1,2]` | done | `seg` | `'compact'` |
| `todos[3,4,5]` | active | `count` | `3` (→ "03") |
| `filter` | `'all'` | `progress` | `64` (→ "64%") |
| `draft` | `''` | `building` | `false` |
| `q` (search) | `''` | `disc` | `{a:true, b:false, c:false}` |
| `selRow` | `-1` (none) | `accent` | `'#5b86f5'` (blue) |
| `menuOpen` | `false` | `sw.wireframe` | `true` |
| `lastAction` | `'—'` | `sw.snap` | `false` |
| `modalOpen` | `false` | `sw.motion` | `false` |
| `modalMode` | `'create'` | `mPublic` | `true` |
| `mName` | `''` | `mKind` | `'button'` |
| `inspector` prop | `true` | preview | 1280×800 |

Entity-tree generator (1000 nodes, JS 478–484): `type=TYPES[(i·7+3)%10]`,
`depth=i==0?0:(i·13)%5`, `ms=((i·37)%180)/100+0.02`,
`st = i%53==0?'WARN':(i%131==0?'ERR':'OK')`,
`name=names[(i·11)%20]+'_'+pad4(i)`.

---

## Open ambiguities (consolidated)

1. **`spin` keyframe has no duration** and no element uses it. If a build
   spinner is added, the duration is invented (recommend ~0.8–1s linear
   infinite). Document the chosen value.
2. **Entrance animations** (`menu-in`, `modal-in`, `toast-in`, `ov-in`) declare
   keyframes but **no inline `animation-duration`** — only `blink` (1.6s) is
   wired. CSS default is `0s` (no anim). Recommended durations: menu/ov ~120ms,
   modal ~160ms, toast ~180ms (ease-out) — **invented, flag in impl.**
3. **`--ac2` inline fallback (`#6f96ff`) ≠ JS-computed (`#7fa1f7` for blue).**
   The JS value renders at runtime; use it. Inline `#6f96ff` is dead fallback.
4. **`--acglow` `.6` inline literals ≠ ramp `.55`.** Ramp wins at runtime; use
   `.55`.
5. **`::selection` (`rgba(91,134,245,.32)`) and logo ring
   (`rgba(91,134,245,.35)`)** are hard-coded **blue**, not accent-derived.
   Recommend matching source (fixed blue).
6. **`prefers-reduced-motion`** is not implemented — the "Reduced motion" switch
   is a visual demo toggle only. Intended behavior is jump-to-end; **flag as
   design intent, not in source.**
7. **`VPORT=360`** (JS) is a windowing constant, not the actual rendered panel
   height (which is `flex:1`); it only affects how many rows mount. Parity of the
   *visual* result depends on the real panel height, but the *windowing math*
   must use 360 to match mount counts.
