**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — Accessibility: bevy_a11y integration, per-widget wiring, focus model, WCAG 2.2 gap analysis

# Accessibility

bevy_feathers's a11y story is **partial-by-construction**. Feathers sits on top of `bevy_a11y` (which is BSN-hostile per issue [#17644](https://github.com/bevyengine/bevy/issues/17644), with PR [#24308](https://github.com/bevyengine/bevy/pull/24308) — `AccessibleLabel` — landing for Bevy 0.19 on 2026-05-21 to start the decomposition). The feathers widgets themselves either (a) set `AccessibilityNode(...)` directly on the styled entity, (b) rely on `bevy_ui_widgets` to set the role upstream, or (c) ship without explicit a11y wiring. Empirically the third category is the largest.

Cross-link: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Megacomponents that are BSN-hostile" — the bevy_a11y critique. See also [widgets.md](widgets.md) for per-widget surface-area notes.

## Per-widget a11y wiring — verified

This table is what each widget's own source file does. Items marked "upstream" defer to `bevy_ui_widgets`; items marked "none" set nothing in the styled source.

| Widget               | Role set in feathers source? | States / value set?  | Focus | Notes |
|----------------------|------------------------------|----------------------|-------|-------|
| Button               | none                         | upstream (?)         | `TabIndex(0)` + `FocusIndicator` | Verify `bevy_ui_widgets::Button` sets `Role::Button` upstream. |
| Toggle switch        | **`Role::Switch`** explicit  | `Checked` component  | `TabIndex(0)` + `FocusIndicator` | The only widget that pins its role directly in feathers source. |
| Checkbox             | none                         | `Checked` component  | `TabIndex(0)` + `FocusIndicator` | No tri-state (`aria-checked="mixed"`). |
| Radio                | none                         | `Checked` + `RadioButton` marker | `TabIndex(0)` + `FocusIndicator` | Group semantics depend on `bevy_ui_widgets`. |
| Slider               | none                         | value/min/max in `FeathersSliderProps`; `ValueChange<f32>` event | `TabIndex(0)` + `FocusIndicator` | APG slider requires arrow / page / home / end — no evidence in feathers source. |
| Disclosure toggle    | none (checkbox-based)        | `Checked` (= expanded) | `TabIndex(0)` + `FocusIndicator` | APG disclosure pattern wants `button` + `aria-expanded`, not `checkbox` + `checked`. Semantic mismatch. |
| Menu                 | upstream (`MenuButton` / `MenuItem` markers) | `MenuEvent` | `TabIndex(0)` | Popup positioned via popover system; submenu / menubar absent. |
| Text input           | none                         | edit state via `bevy_ui_widgets::EditableTextFilter` | upstream | Single-line only; no IME wiring evident. |
| Number input         | none                         | typed value (`T`) via `ValueChange<T>` | upstream | Not APG `spinbutton` (no arrow stepping). |
| Color slider / plane | none                         | `ValueChange<f32>` / `ValueChange<Vec2>` | upstream | Specialist controls; no APG pattern matches exactly. |
| Virtual keyboard     | none                         | `VirtualKeyPressed<T>` event | upstream | No `Role::Keyboard` analogue. |

**Conclusion:** bevy_feathers's a11y surface is "whatever `bevy_ui_widgets` provides upstream, plus one explicit `Role::Switch` on the toggle and focus indicators on everything." This is not a WCAG 2.2 AA-conformant kit. Treat any audit claim with skepticism.

## Focus model — `src/focus.rs`

Feathers provides a focus-indicator (outline-rendering) layer on top of `bevy_input_focus`:

- **`FocusIndicator`** — marker component: "show a visible focus outline when either this entity or its ancestor is focused."
- **`FocusWithinIndicator`** — marker for the `:focus-within` analogue: outline when this entity or any descendant is focused.
- **System:** `manage_focus_indicators` runs in `PostUpdate` / `UiSystems::Content`. Outline width 2px, offset 2px, color from `tokens::FOCUS_RING`.
- **`:focus-visible` analogue:** outline is gated on `input_focus_visible.0` — only shown when focus is the result of keyboard navigation (or other focus-visible-triggering interaction), not on every mouse click. Matches the CSS `:focus-visible` semantic.
- **Roving tabindex:** **not provided** by feathers. The `bevy_input_focus::tab_navigation::TabNavigationPlugin` is added by `FeathersPlugins`, giving simple sequential `Tab` cycling through `TabIndex(0)` entities. Composite widgets (listbox, tree, grid) that require roving tabindex are not in feathers's catalog.
- **Focus traps, restoration, inert subtrees, `aria-activedescendant`:** none of these are in feathers source. Editor windows that need modal trapping rely on app-side wiring.
- **Spatial / gamepad navigation:** Bevy 0.18 introduced `AutoDirectionalNavigation`; feathers does not opt into it explicitly, but a host app can.

Buiy's foundation [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to a single focus tree with `:focus-visible` semantics, traps, restoration, inert subtrees, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point, and spatial gamepad navigation — a substantially larger surface than what feathers ships.

## WCAG 2.2 AA — practical gap analysis

What feathers does well:

- **SC 1.4.11 Non-text Contrast** — focus ring (2px outline, themed color) provides a 3:1-against-adjacent contrast affordance, *if* the chosen `FOCUS_RING` token actually meets 3:1 against the surface (the contrast is not audited in source).
- **SC 2.4.7 Focus Visible** — `FocusIndicator` + `:focus-visible` gate provides keyboard-only focus rings.
- **SC 2.4.11 Focus Not Obscured (Minimum)** — outline offset 2px keeps the outline outside the element bounds.
- **SC 2.5.8 Target Size (Minimum)** — verify per widget. `ROW_HEIGHT=24`, `CHECKBOX_SIZE=18` — checkbox at 18×18 is **below the 24×24 minimum** without an enlarged hit target (which feathers does not document).

Where feathers falls short of WCAG 2.2 AA in practice:

- **SC 1.4.3 Contrast (Minimum)** — no contrast audit in source; the dark theme is **not certified** to meet 4.5:1 / 3:1 across all token combinations. A consumer must verify their own deployment.
- **SC 1.4.11 Non-text Contrast** — same: not certified. Token values are hand-picked.
- **SC 1.4.12 Text Spacing** — feathers has hardcoded font sizes (`MEDIUM_FONT=14` etc.) and no scaling response to user OS text-scaling.
- **SC 1.4.13 Content on Hover or Focus** — tooltips do not exist as a widget; the dismissable / hoverable / persistent contract is not provided.
- **SC 2.1.1 Keyboard** — major gaps: slider has no audited arrow-key contract; combobox / listbox / tree / grid don't exist; the disclosure-toggle uses checkbox semantics so screen-reader announcements are wrong.
- **SC 2.4.3 Focus Order** — sequential `Tab` order only; composite widgets that need roving tabindex don't exist.
- **SC 2.4.6 Headings and Labels** — no `Role::Heading` widget; no `aria-labelledby` / `aria-describedby` infrastructure beyond what bevy_a11y / `AccessibleLabel` (post-PR #24308) provides.
- **SC 2.5.7 Dragging Movements** — no keyboard alternative for slider dragging is verifiable.
- **SC 3.2.3 Consistent Navigation** — N/A (no multi-page model).
- **SC 4.1.2 Name, Role, Value** — the load-bearing one. Most widgets do not set Role explicitly in feathers source. Whatever role flows through `bevy_ui_widgets` is the de-facto answer, and the absence of `AccessibilityNode` literals in `button.rs`, `slider.rs`, `checkbox.rs`, etc. is notable.
- **SC 4.1.3 Status Messages** — no `Role::Alert`, `Role::Status`, `Role::Log`, `Role::Timer` widgets; no live-region infrastructure.

The honest summary: **bevy_feathers is not WCAG 2.2 AA-conformant out of the box.** It is a styled widget set for editors and utilities, and the editor-focused framing means productivity-app accessibility tradeoffs (modals, dialogs, listbox, tree, table, live regions) have not been a development priority.

## bevy_a11y BSN-hostility — issue #17644 and PR #24308

The substrate problem: `bevy_a11y::AccessibilityNode` was a megacomponent wrapping `accesskit::Node` with all its properties as private fields. BSN templates couldn't patch it. See [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Megacomponents that are BSN-hostile."

PR #24308 (`AccessibleLabel`, merged 2026-05-21 for Bevy 0.19) introduces a decomposed label component as the first step in breaking up `AccessibilityNode`. As of 2026-05-22, that decomposition is not complete — the role + states + relations are still encoded inside the megacomponent.

Implication for feathers: even if a maintainer set out to wire every feathers widget's a11y role explicitly tomorrow, they'd be wiring it through a component the BSN-driven future of Bevy is in the middle of replacing. The wiring effort is double-discounted: it's not done today, and the API is mid-flight. This is partly why most feathers widgets currently lack explicit role wiring — the substrate's decomposition is the load-bearing prerequisite.

## Comparison to Buiy's AccessKit-first design

Per [foundation architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md):

- **Buiy bypasses `bevy_a11y` entirely on any window where Buiy is present.** Buiy owns the `accesskit_winit::Adapter`, keyed by winit `WindowId`. `ActionRequest` events route through Buiy's own action plumbing, not `bevy_a11y::ActionRequest`.
- **Decomposed components from day one:** `A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations`. Every Buiy widget pins its role explicitly. No megacomponent.
- **ACCNAME 1.2** name computation lives in `buiy_core`. Feathers has no ACCNAME implementation — whatever name surfaces is whatever upstream `AccessibilityNode` carries, and that's typically just the `name` field if it's been set.
- **APG keyboard contracts** are part of each widget's contract, with CI-gated verification (foundation [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) gate 7). Feathers has no verification harness for APG conformance.
- **Live regions** (`role="alert"`, `role="status"`, etc.) ship as foundation-tier widgets in Buiy's catalog. Not in feathers.

## Implications for Buiy

Three concrete takeaways:

1. **Wiring a11y through a megacomponent substrate is a recipe for the wiring not happening.** Feathers is the cautionary tale: most widgets don't set a Role, and the most plausible explanation is that `bevy_a11y`'s ergonomics make pinning a role a chore the widget authors skipped. Buiy's decomposed `A11yRole` component is one observable, public-fielded line per widget — by construction it's harder to omit.
2. **A widget kit that does not own its a11y wiring is a widget kit that ships gaps.** Don't separate "the widget" from "its accessibility" — the role and keyboard contract are *part of* the widget contract.
3. **Per-window coexistence with feathers means the host app's editor pane gets feathers's a11y (partial) and the game window gets Buiy's a11y (complete).** From a user's perspective, mixing produces an inconsistent experience. The Buiy-side documentation should be explicit that the app's a11y story is only as strong as its weakest window.

## Sources

- `focus.rs` — https://github.com/bevyengine/bevy/blob/main/crates/bevy_feathers/src/focus.rs
- Verified absence of `accesskit` imports in `button.rs`, `slider.rs`, `checkbox.rs`, etc. — direct source inspection 2026-05-22.
- Verified presence of `AccessibilityNode(accesskit::Node::new(Role::Switch))` in `toggle_switch.rs` — direct source inspection 2026-05-22.
- Issue #17644 (bevy_a11y BSN-unfriendly) — https://github.com/bevyengine/bevy/issues/17644
- PR #24308 (`AccessibleLabel`, decomposition first step, merged 2026-05-21 for 0.19) — https://github.com/bevyengine/bevy/pull/24308
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- Buiy foundation a11y — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6
- Buiy foundation accessibility detail — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- bevy_ui lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Cross-link: [widgets.md](widgets.md), [architecture.md](architecture.md)
