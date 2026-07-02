# Gallery active/open-state reflect — design

**Kind:** decision
**Status:** accepted
**Date:** 2026-07-01
**Area:** examples/buiy_gallery
**Supersedes / relates:** follow-up to the widget-catalog + parity campaigns (the gallery is exact-parity with `docs/reference-designs/widget-catalog/`).

## Problem

A widget-catalog audit (render every widget state to GPU + a driven behavioral
sweep) found two gallery controls whose **active/open visual is applied only at
spawn, with no runtime system to re-apply it on state change** — the same class as
the just-fixed stepper no-op (state changes, display doesn't). Both are confirmed
by driven interaction tests that fail on current code, and both contradict the
parity design, which specifies the state-dependent look.

1. **Todo filter pills (All / Active / Done — `FilterButton`).** `filter_pill_colors`
   (active = `color.accent` bg + `color.text.on-accent` label; inactive =
   `color.surface.transparent` + `color.text.muted`) is called **only** in
   `build_filter_pill`, seeding "All" active. Clicking a pill sets the `Filter`
   resource and `apply_filter` hides rows, but nothing re-tints the pills, so the
   accent highlight stays frozen on "All". (A code comment at the pill-row builder
   falsely claims `apply_filter` "restyles them on a filter change" — it does not.)

2. **Overlay-menu ⋮ trigger (`MenuButton` `#MenuTrigger`).** `build_menu_button`
   hard-codes the **closed** look (bg `color.surface.inset`, border
   `color.border.default`, ⋮-icon `color.text.muted`). The design's `menuBtnStyle`
   recolors the trigger when the menu opens (bg `#1e2127` = `color.surface.raised-alt`,
   border `#3a4150` = `color.border.strong-2`, icon `#f1f3f6` = `color.text.primary`).
   `bind_menu_model` flips the button's `A11yExpanded` on open but never touches its
   paint, and no gallery system does either — so opening/closing the menu leaves the
   trigger visually identical.

Both are gallery-local (the specific colors are gallery styling choices, not
framework defaults), so the fix belongs in the gallery screen plugins.

## Target state

Add two **change-detection reflect systems**, each mirroring the gallery's existing
`reflect_rail_active_state` (nav rail) — the established pattern for "re-apply a
selection/active visual when its source-of-truth changes":

- **`reflect_active_filter`** (owned by `TodoMvcPlugin`): gated on
  `is_resource_changed::<Filter>`, re-applies `filter_pill_colors(mode == filter.0)`
  to every `FilterButton`'s `Background` + its `#FilterLabel` child `TextColor`.
- **`reflect_menu_button_open_state`** (owned by `OverlayMenuPlugin`): on the
  `MenuButton`'s `Changed<A11yExpanded>`, sets its `Background` + border + the
  `#MenuDotsIcon` descendant `Icon.color` to the open or closed token triplet.

Both use `set_if_neq`-style guards (write only when the value differs) to stay
extract-cheap and idempotent — the same discipline `reflect_rail_active_state` and
`update_count` already follow.

Each fix ships with a **driven interaction test** (real shell + real picking +
synthetic click) asserting the *rendered* component changes, not just state:
- `todo_filter_pill_click_moves_the_active_highlight` — clicking "Active" moves the
  accent `Background` to it and drops "All" to the inactive fill. (Already written;
  currently failing → the regression guard.)
- a menu-trigger analog — opening the menu recolors `#MenuTrigger`'s `Background`;
  closing restores it.

The stale/false comment on the filter-pill row builder is corrected to describe the
real mechanism.

## Rejected alternatives

- **Re-tint inside the press handler** (`collect_button_press` for pills;
  `route_menu_press` for the trigger). Rejected: couples the visual to one input
  path (a programmatic `Filter` change or a keyboard/AT-driven menu open wouldn't
  re-tint), doesn't match the change-detection reflect pattern used everywhere else
  in the gallery (`reflect_rail_active_state`, `reflect_accent_selection`,
  `update_count`), and would bloat the collector with pill/child queries. A
  change-detection system keyed on the source-of-truth is the single-source-of-truth
  choice.
- **Add a framework-level `MenuButton` open-visual driver in `buiy_widgets`.**
  Rejected for this cycle: the specific open/closed colors are *gallery* styling (the
  gallery overrides the widget defaults at spawn), so the reflect belongs with the
  gallery, consistent with how the gallery already owns its custom widget visuals.

## Out of scope — surfaced follow-ups (not fixed here)

The audit surfaced two lower-severity, **framework-level** gaps with wider blast
radius; logged for a separate decision rather than widened into this gallery fix:

- **Default `Switch` track never recolors.** `update_switch_visual` slides only the
  thumb; the track fill is a static `color.surface.secondary`. The modal register
  switch (a default `Switch`) therefore doesn't turn its track accent-on as the
  design shows (the showcase switches use a custom track + `drive_showcase_switches`
  that does recolor). Root cause is the framework widget default → framework change.
- **Menu items have no active-descendant highlight.** Arrow-key roving moves
  `MenuModel.active` with no visible feedback (`menu.rs`: "item highlight is a C6
  paint concern, not built here"). Design-faithful today (the reference renders items
  flat), but Buiy's roving focus has no paint — an expected-but-unwired highlight.
