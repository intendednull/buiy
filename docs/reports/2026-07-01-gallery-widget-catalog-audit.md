# Gallery widget-catalog audit — 2026-07-01

**Kind:** report (one-shot audit)
**Trigger:** "test each and every widget in the widget catalogue; make sure there
are no more issues" — a follow-up to the stepper + disclosure-chevron fixes
(PR #112). Baseline: `origin/main` `30489ce`.
**Outcome:** two gallery-local behavioral no-ops found + fixed (filter pills, menu
trigger); two lower-severity framework gaps surfaced as follow-ups; everything else
verified correct.

## Method

The gallery is the widget catalog: 5 screens (Todo / Virtual List / Overlay Menu /
Modal Dialog / Controls) exercising ~11 core widgets (Button, TextInput, Checkbox,
Switch, Slider, Disclosure, Dialog, Tooltip, Popover, Menu, ScrollArea) + ~12
composites (stepper, segmented, search_input, meter, toast, badge, chip, kbd,
status_dot, card, table_row, stat_row).

Two failure modes were targeted, from the PR #112 lessons: **render-given-state**
bugs (the chevron class — pixels wrong for a valid state; headless-invisible) and
**behavioral no-ops** (the stepper class — state changes, display doesn't). Four
passes:

1. **Baseline.** Full headless workspace suite (**1899 / 0**) + both GPU `--ignored`
   legs on an AMD RX 6700 XT (`buiy_core` **73 / 0**, `buiy_verify` **23 / 0**). Green.
2. **Visual render (render-given-state).** A force-state GPU capture bin
   (`examples/buiy_gallery/src/bin/probe_states.rs`) rendered the Controls screen with
   every transform/position-sensitive widget forced NON-resting (switches on, slider
   max, segmented moved, stepper bumped, meter full, all disclosures expanded);
   `capture_shell` rendered all 5 resting screens (incl. menu-open, modal-open) and
   `capture_composites` the composites grid. Every PNG was eyeballed.
   **No render bugs** — every widget renders correctly in resting and forced states,
   incl. the PR #112 disclosure chevron (points down + body visible when expanded).
3. **Behavioral sweep (no-ops).** For each interactive widget, checked whether a
   driven click/key changes the *rendered* output (not just internal state),
   cross-referencing existing test assertions. Widgets whose display-follows-state is
   pushed by a real runtime system (`update_count`, `set_segmented`, `set_stepper`,
   `restyle_completed`, `drive_showcase_*`, `set_table_row_selected`,
   `bind_menu_model`, `reflect_rail_active_state`, `reflect_accent_selection`) are
   correct. Two controls change state with **no runtime reflect** → no-ops.
4. **Adversarial verification.** Fresh-context agents confirmed design intent (the
   parity design specifies the missing visuals) and swept for other instances of the
   same class.

## Findings

### Fixed (this cycle — gallery-local, same class: visual set only at spawn)

- **F1 — Todo filter pills don't reflect the active filter.** Clicking Active/Done
  set the `Filter` resource and `apply_filter` hid rows, but nothing re-ran
  `filter_pill_colors`, so the accent highlight stayed frozen on "All".
  `filter_pill_colors` was called only in `build_filter_pill` (spawn). Confirmed by
  the failing driven test `todo_filter_pill_click_moves_the_active_highlight`. A code
  comment falsely claimed `apply_filter` restyled the pills — corrected.
  **Fix:** `reflect_active_filter` (change-detection on `Filter`, mirrors
  `reflect_rail_active_state`), registered in `TodoMvcPlugin`.

- **F2 — Overlay-menu ⋮ trigger doesn't reflect the open menu.** `build_menu_button`
  hard-codes the closed look; the design's `menuBtnStyle` recolors the trigger on
  open (bg `surface.raised-alt`, border `border.strong-2`, icon `text.primary`).
  `bind_menu_model` flipped the button's `A11yExpanded` but never its paint.
  Opening/closing left the trigger visually identical. Confirmed by the failing
  driven test `menu_button_click_recolors_the_trigger_open_then_closed`.
  **Fix:** `reflect_menu_button_open_state` (on the trigger's `Changed<A11yExpanded>`),
  registered in `OverlayMenuPlugin`; the ⋮ glyph tagged `MenuTriggerIcon`.

Both fixes ship with the driven regression tests above (assert the *rendered*
component, the gap that let the no-ops ship). Design + tokens: see
`docs/specs/2026-07-01-gallery-active-state-reflect-design.md`.

### Surfaced as follow-ups (framework-level; wider blast radius — not fixed here)

- **N1 — default `Switch` track never recolors.** `update_switch_visual` slides only
  the thumb; the track fill is a static `color.surface.secondary`. The modal register
  switch (a default `Switch`) therefore never turns its track accent-on as the design
  shows (the showcase switches use a custom track + `drive_showcase_switches` that
  does recolor). Root cause is the framework widget default → a framework decision.
- **N2 — menu items have no active-descendant highlight.** Arrow-key roving moves
  `MenuModel.active` with no visible feedback (`menu.rs`: "item highlight is a C6
  paint concern, not built here"). Design-faithful today (the reference renders items
  flat), but Buiy's roving focus paints nothing — an expected-but-unwired highlight.

### Verified correct (no action)

Button, TextInput, Checkbox, Switch (thumb), Slider, Disclosure, Dialog, Popover,
Menu (open/dismiss/item-activate), ScrollArea (scroll/select/search), and every
composite render + behave correctly. The S3 tooltip + standalone popover sit in a
`Display::None` holder **by design** (kept for the C8b acceptance drivers; the menu
screen doesn't surface them) — not a bug. The Modal inspector "open" readout staying
`false` under a forced-visibility capture is the pre-existing, already-logged capture
artifact, not a runtime bug.

## Verification

Headless workspace suite green; both GPU legs green; the two new driven tests pass
(21/0 in `tests/interaction.rs`); menu-open re-render eyeballed (trigger recolors).
The audit-tooling bin (`probe_states.rs`) is dev-only (not a CI gate), mirroring the
existing `capture_*` bins.
