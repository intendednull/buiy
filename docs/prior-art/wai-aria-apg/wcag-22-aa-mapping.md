**Date:** 2026-05-22
**Status:** active
**Subject:** WCAG 2.2 (Recommendation 5 October 2023) — the widget-implementable success criteria APG patterns satisfy, with per-SC verification strategy and Buiy's enforcement tier

# WCAG 2.2 AA mapping (widget-implementable success criteria)

WCAG 2.2 was published as a W3C Recommendation on **5 October 2023**. It adds 9 new success criteria over WCAG 2.1 (most notably 2.4.11 Focus Not Obscured, 2.5.7 Dragging Movements, 2.5.8 Target Size Minimum, 3.3.7 Redundant Entry, 3.3.8 Accessible Authentication). The full mapping is at <https://www.w3.org/TR/WCAG22/> and the Understanding-docs at <https://www.w3.org/WAI/WCAG22/Understanding/>.

The Buiy foundation [`accessibility.md § WCAG 2.2 Success Criteria`](../../specs/2026-05-07-buiy-foundation/accessibility.md) lists every Level A and AA SC with one of four enforcement strategies: **CI** (automated in verification pipeline), **RT** (runtime-honoured constraint), **LR** (lint-with-review), **DC** (design constraint — content quality is consuming-app's concern). This file is the per-SC verification-strategy companion for **widget-implementable** criteria — the ones APG patterns satisfy structurally.

Each row below names the SC, its level, the APG-pattern handle that satisfies it, and the Buiy verification approach.

## Perceivable

### 1.3.1 Info and Relationships (A) — CI

**APG.** Every widget pattern specifies the ARIA role, parent / child relationships, and labelling required to expose the widget's information programmatically. The role + relation graph IS the info-and-relationships representation.

**Buiy verification.** Gate 3 (AccessKit tree snapshot) captures role + parent/child + relation set for every focusable node. Snapshots compared against per-widget golden trees.

### 1.3.2 Meaningful Sequence (A) — CI

**APG.** Composite widgets specify reading order through DOM-equivalent ordering and `aria-flowto` for non-linear reading.

**Buiy verification.** AccessKit tree-order snapshot matches visual reading order; per-widget fixtures verify.

### 1.3.4 Orientation (AA) — RT + CI

**APG.** No widget locks orientation; widgets work in both portrait and landscape.

**Buiy verification.** Cross-fixture snapshot in portrait + landscape; widgets render correctly in both.

### 1.3.5 Identify Input Purpose (AA) — CI

**APG.** Combobox and form-input patterns specify `autocomplete` token list per input.

**Buiy verification.** Form linter requires `autocomplete` token on every form field input; CI gate.

### 1.4.11 Non-text Contrast (AA) — CI

**APG.** Focus rings, state-indicator graphics, UI control bounds. APG's "Focus Visible" guidance feeds 2.4.7; 1.4.11 covers state indicators.

**Buiy verification.** Contrast linter validates focus rings and state indicators at 3:1; theme tokens enforce.

### 1.4.13 Content on Hover or Focus (AA) — CI

**APG.** Tooltip pattern explicitly: tooltips are dismissable (Esc), hoverable (move to tooltip without it dismissing), persistent (no auto-timeout). Popover variants must meet the same.

**Buiy verification.** Tooltip + Popover contract assertions; verification gate 7 (keyboard contract).

## Operable

### 2.1.1 Keyboard (A) — CI

**APG.** Every widget specifies a complete keyboard contract; no operation requires pointer / mouse / touch.

**Buiy verification.** Gate 7 — every interactive widget operable from keyboard alone. Fixtures replay synthesised keyboard sequences and assert the widget reaches the same end-state.

### 2.1.2 No Keyboard Trap (A) — CI

**APG.** Composite widgets that trap focus (Dialog, AlertDialog) must have explicit exit (Esc); arrow-key navigation must not trap.

**Buiy verification.** Focus-traversal property test for every widget: from any state, keyboard alone can reach the next widget after the composite.

### 2.1.4 Character Key Shortcuts (A) — CI

**APG.** Widgets with single-char shortcuts emit `aria-keyshortcuts`; users can remap or disable.

**Buiy verification.** `aria-keyshortcuts` registration verified; remap policy verified per widget.

### 2.2.2 Pause, Stop, Hide (A) — CI

**APG.** Carousel auto-rotation has pause / stop / next / prev controls; Feed pauses loading. Toast / Snackbar provide extend / dismiss.

**Buiy verification.** Carousel + Feed + Toast contract assertions: pause control present, dismiss works, extend works.

### 2.3.1 Three Flashes or Below (A) — CI

**APG.** No pattern-specific guidance, but APG patterns don't prescribe flashing animations. Buiy's animation primitives implement a flash detector.

**Buiy verification.** Animation flash detector in CI: max 3 flashes per second across any widget.

### 2.4.1 Bypass Blocks (A) — CI

**APG.** Skip-link primitive + landmark navigation.

**Buiy verification.** Skip-link present in fixture; landmark role roster verified.

### 2.4.3 Focus Order (A) — CI

**APG.** Composite widgets specify roving-tabindex or `aria-activedescendant`; tab order matches visual reading order.

**Buiy verification.** Tab-order snapshot per widget; matches expected order.

### 2.4.7 Focus Visible (AA) — CI

**APG.** Implicit — `:focus-visible` styling expected on every focusable widget.

**Buiy verification.** Focus-ring rendering verified on every focusable widget; contrast linter validates ≥3:1 vs unfocused.

### 2.4.11 Focus Not Obscured (Minimum, AA) — CI

**WCAG 2.2 new.** Focused element is not entirely hidden by author-created content (e.g. sticky toolbar covering it).

**Buiy verification.** Sticky toolbar + modal fixtures; verify focused element is at least partially visible.

### 2.5.3 Label in Name (A) — CI

**APG.** Visible label text is part of accessible name (so voice-control users can speak the label).

**Buiy verification.** Linter: if widget has visible label "Submit", its accessible name must contain "Submit" (case-insensitive, whitespace-normalised).

### 2.5.7 Dragging Movements (AA) — CI

**WCAG 2.2 new.** Every drag-driven widget has a single-pointer or keyboard alternative.

**APG.** Slider (incl. Multi-Thumb) keyboard contract; Window Splitter keyboard contract; Tree drag-to-reorder must have keyboard alternative.

**Buiy verification.** Every drag widget has keyboard alternative; tested. Also AccessKit `Action::Increment` / `Decrement` / custom-action available.

### 2.5.8 Target Size (Minimum, AA) — CI

**WCAG 2.2 new.** Pointer targets ≥24×24 CSS pixels (with documented exceptions for inline targets, etc.).

**Buiy verification.** Hit-target linter enforces ≥24×24 across the catalog. (Note: bevy_feathers's `CHECKBOX_SIZE=18` is a canonical violation; Buiy ships its own widgets.)

## Understandable

### 3.2.1 On Focus (A) — CI

**APG.** Focus events do not trigger context changes (no auto-submit on focus).

**Buiy verification.** Linter: focus event handlers do not call route / navigation / submit. Implicit in widget contract.

### 3.2.2 On Input (A) — CI

**APG.** Input events do not auto-submit / navigate.

**Buiy verification.** Same as 3.2.1, applied to input events.

### 3.3.1 Error Identification (A) — CI

**APG.** Form patterns specify `aria-invalid="true"` + `aria-describedby` to error message; error-message linked to field.

**Buiy verification.** Error-message model per form spec verified per form fixture.

### 3.3.7 Redundant Entry (A) — RT (WCAG 2.2 new)

**APG.** Not directly addressed; form state retention is an app concern.

**Buiy verification.** Form-state machine retains values across navigation steps; verified.

### 3.3.8 Accessible Authentication (Minimum, AA) — CI + DC (WCAG 2.2 new)

**APG.** Not directly addressed; authentication is an app concern.

**Buiy verification.** CI verifies paste-allowed (no `paste` event block) on password / authentication input types and absence of cognitive-puzzle widgets in the default catalog.

## Robust

### 4.1.2 Name, Role, Value (A) — CI — THE CENTRAL SC

**APG.** EVERY pattern specifies the role + name + value emission. This SC is the central widget-implementable criterion; it's why APG exists.

**Buiy verification.** Gate 3 — AccessKit tree snapshot captures role + name + value (via `aria-valuenow` / `aria-valuetext` for range widgets) for every focusable node. Snapshots are the canonical 4.1.2 conformance evidence.

### 4.1.3 Status Messages (AA) — CI

**APG.** Live-region roles (`alert`, `status`, `log`) plus `aria-live` properties. See [`live-regions.md`](live-regions.md).

**Buiy verification.** Gate 4 — announcement output verified per fixture; `role=status` triggers polite announcement.

## SCs widgets enable but don't directly satisfy (Design Constraint, DC)

These SCs are satisfied by the consuming application's content, not by Buiy widgets. Buiy provides the affordance; the app's content must use it correctly.

- 1.1.1 (alt text), 1.2.x (media captions / audio description), 1.3.3 (sensory characteristics), 1.4.1 (use of color), 1.4.2 (audio control), 2.2.1 (timing adjustable), 2.4.5 (multiple ways), 2.5.4 (motion actuation), 3.1.1 (language of page — Buiy plumbs through `lang`), 3.3.2 (labels — Buiy linter advises), 3.3.4 (error prevention — Buiy provides confirmation widgets)

These are not widget-implementable per se; APG patterns don't satisfy them on their own. Buiy's role is to provide the affordance (the `Image.alt` field, the captions slot, the confirmation widget) and let the app's content fill it correctly.

## AAA aspirational

The Buiy foundation positions AAA as aspirational. The relevant AAA SCs widget-implementable in principle:

- 2.4.12 Focus Not Obscured (Enhanced) — focused element fully unobscured. Tier C.
- 2.4.13 Focus Appearance — ≥2 px perimeter, ≥3:1 contrast vs unfocused. Tier C.
- 2.5.5 Target Size (Enhanced) — ≥44×44. Tier C.
- 2.5.6 Concurrent Input Mechanisms — relevant given Buiy's gamepad / keyboard / pointer concurrency goal; aspirational rather than gated.

## How APG patterns satisfy WCAG (table)

| WCAG SC | APG patterns that structurally satisfy it |
|---|---|
| 1.3.1 (info and relationships) | every pattern via role + parent/child + relations |
| 1.3.2 (meaningful sequence) | every pattern via DOM order + `aria-flowto` where needed |
| 1.4.13 (content on hover/focus) | Tooltip, Popover |
| 2.1.1 (keyboard) | every interactive pattern |
| 2.1.2 (no keyboard trap) | Dialog, AlertDialog, Menu |
| 2.1.4 (character key shortcuts) | any pattern emitting `aria-keyshortcuts` |
| 2.2.2 (pause stop hide) | Carousel, Feed, Marquee, Toast |
| 2.4.1 (bypass blocks) | Landmarks, skip-link primitive |
| 2.4.3 (focus order) | every composite widget |
| 2.4.7 (focus visible) | implicit on every focusable widget |
| 2.4.11 (focus not obscured) | Dialog (focus trap), Toolbar |
| 2.5.3 (label in name) | every labeled widget — labelling chain |
| 2.5.7 (dragging movements) | Slider, Multi-Thumb Slider, Window Splitter, Tree-drag-reorder |
| 2.5.8 (target size minimum) | every hit-targeted widget — minimum size contract |
| 3.3.1 (error identification) | form patterns with `aria-invalid` + `aria-describedby` |
| 4.1.2 (name role value) | EVERY pattern |
| 4.1.3 (status messages) | Alert, Status, Log, Timer |

## Sources

- WCAG 2.2 Recommendation (5 October 2023): <https://www.w3.org/TR/WCAG22/>
- WCAG 2.2 Understanding docs: <https://www.w3.org/WAI/WCAG22/Understanding/>
- WCAG 2.2 What's New: <https://www.w3.org/WAI/standards-guidelines/wcag/new-in-22/>
- APG patterns mapping to WCAG: implicit through each pattern's "WAI-ARIA Roles, States, and Properties" + keyboard sections
- Buiy WCAG 2.2 enforcement table: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy verification harness: [`docs/specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
- Sibling files: [`patterns-catalog.md`](patterns-catalog.md), [`focus-management.md`](focus-management.md), [`live-regions.md`](live-regions.md)
