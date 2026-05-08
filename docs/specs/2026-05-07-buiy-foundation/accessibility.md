# Feature inventory — accessibility (ARIA + WCAG 2.2)

**Parent:** [README.md](README.md)

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason). See [README.md § Tier legend](README.md#tier-legend).

A small number of WCAG-tied items in this section carry **dual tiers** of the form `F (AA) / C (AAA)`, where the conformance level (AA vs AAA) and the Buiy implementation tier (foundation vs core) differ. This convention applies only here.

## 3.11 Accessibility (ARIA + WCAG 2.2)

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
- Tested against: NVDA, JAWS, Narrator, VoiceOver (mac/iOS), Orca, TalkBack. **F** (via verification harness — see [verification.md](verification.md))
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
| 3.3.1 | Error Identification | A | CI | Error-message model per [interaction.md § 3.6](interaction.md); verified per form fixture. |
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
