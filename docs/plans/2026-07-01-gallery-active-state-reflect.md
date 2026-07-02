# Gallery active/open-state reflect — plan

Implements `docs/specs/2026-07-01-gallery-active-state-reflect-design.md`. Two
gallery-local reflect systems + driven regression tests. Verify headless + GPU +
eyeball, then PR + merge.

## W1 — Filter pill active-highlight reflect (`TodoMvcPlugin`)

1. Add `reflect_active_filter(world: &mut World)` in `examples/buiy_gallery/src/lib.rs`,
   modeled on `shell.rs::reflect_rail_active_state`:
   - early-return unless `world.is_resource_changed::<Filter>()`;
   - read `filter.0`; for each `(Entity, &FilterButton, &Children)`, compute
     `active = fb.0 == filter.0`, `(bg, fg) = filter_pill_colors(active)`;
   - set the pill `Background.color = tok(bg)` (guard: only if changed);
   - set the child `#FilterLabel` `TextColor.0 = tok(fg)` (walk children, guard).
2. Register it in `TodoMvcPlugin`, `.after(collect_button_press)` so a click's
   `Filter` write reflects the same frame.
3. Fix the false comment (the pill-row builder claims `apply_filter` restyles the
   pills) to describe `reflect_active_filter`.
4. Test: `todo_filter_pill_click_moves_the_active_highlight` (already added to
   `tests/interaction.rs`) goes green.

## W2 — Menu trigger open-state reflect (`OverlayMenuPlugin`)

1. Tag the ⋮-icon leaf so the reflect can find it (or reuse `#MenuDotsIcon` via a
   marker) — add a small `MenuTriggerIcon` marker on the icon in `build_menu_button`
   if a Name-only lookup is insufficient; prefer a marker for query precision.
2. Add `reflect_menu_button_open_state` in `lib.rs`:
   - on `MenuButton` with `Changed<A11yExpanded>` (or an exclusive world system
     guarded on the button's expanded state), pick the token triplet:
     - open: bg `color.surface.raised-alt`, border `color.border.strong-2`, icon
       `color.text.primary`;
     - closed: bg `color.surface.inset`, border `color.border.default`, icon
       `color.text.muted`;
   - set the button `Background` + border color + the icon descendant `Icon.color`
     (guards).
3. Register in `OverlayMenuPlugin`.
4. Test: a menu-trigger analog in `tests/interaction.rs` — open the menu (click the
   trigger), assert `#MenuTrigger` `Background` becomes the open token; press Esc /
   outside-click, assert it restores. (First write it → fails → implement → passes.)

## Gate + verify

- Fresh-context review of the diff (logic, pattern-consistency, spec alignment).
- Headless: `cargo test -p buiy_gallery` (interaction + the rest) green;
  `cargo fmt --check` + `clippy -D warnings` + `doc -D warnings` clean; full
  workspace `cargo test` green.
- GPU: re-render the Menu screen (closed vs open) + Todo (after an Active click via a
  probe or by eyeball of the live app) — confirm the trigger recolors on open and the
  Active pill highlights. Both GPU legs (`buiy_core`, `buiy_verify`) `--ignored` still
  green (no golden touched — gallery captures aren't CI goldens).
- Update `docs/plans/follow-ups.md` (log the two surfaced framework gaps) and
  `docs/README.md` (index this spec + plan).

## PR + merge

Open a PR; wait for green CI (3-OS + lavapipe GPU + MSRV + web-smoke + deny); merge
per the user's "merge when ready".
