---
Kind: decision
Status: accepted
Date: 2026-07-02
Area: widgets
Resolves: docs/plans/follow-ups.md — "Widgets — default Switch track never recolors" (N1) + "Widgets — menu items have no active-descendant highlight" (N2)
---

# Default `Switch` track recolor + `Menu` active-item focus ring

## Context

The 2026-07-01 gallery widget-catalog audit logged two framework-level widget
defects as follow-ups (`docs/reports/2026-07-01-gallery-widget-catalog-audit.md`
findings **N1**, **N2**). The prior audit cycles (#112, #115) fixed *gallery-level*
frozen-pixel bugs; these two are **framework-level** (`crates/buiy_widgets`), which
is why they were deferred "decide deliberately". A 2026-07-02 follow-up audit pass
re-confirmed both against current source; two fresh-context spec reviews validated
the mechanics and caught a critical contrast error in the first draft (recorded in
the N2 decision below). This note is the decided fix for both.

Both are genuine web-parity / WCAG gaps in **shipped default widgets**:

- **N1** — toggling a default `Switch` changes only the thumb *position*; the track
  never recolors. A web/native switch recolors its track (grey→accent) as the
  primary on/off affordance. The S4 modal "Register globally" switch is a *default*
  `Switch`, so it exhibits this; the S5 showcase switches escape it only via custom
  track pixels + `drive_showcase_switches`.
- **N2** — a `Menu` supports roving `aria-activedescendant` keyboard nav (arrow keys
  move `MenuModel.active`), but **nothing paints the active item**. A sighted
  keyboard user gets no feedback — a WCAG 2.2 gap (SC 1.4.11 Non-text Contrast: the
  visual indication of a component's state needs ≥ 3:1). The active-descendant *is*
  exposed to AT (`bind_menu_model` writes `A11yRelations.active_descendant`); only
  the visual is missing.

## Current state (source of truth)

- `crates/buiy_widgets/src/switch.rs`
  - `update_switch_visual` (:244) writes only the `SwitchThumb` `Translate` on
    `Changed<A11yToggled>`; it holds no `Background` query.
  - `switch_background()` (:123) → `ColorToken::SurfaceSecondary` (`#16181c`), a
    static track fill; `switch_thumb_background()` (:214) → `ColorToken::SurfacePrimary`
    (`#0b0c0e`, near-black). Both initializers are `pub(crate)` so the `scene::switch()`
    scene-fn spells the same canonical values — changing them updates both the
    `#[require]` and scene-fn paths (no separate scene-fn edit needed).
- `crates/buiy_widgets/src/menu.rs`
  - `bind_menu_model` (:835) reacts to `Changed<MenuModel>` and already resolves the
    active item entity (`nth_menu_item`, :796/:866) purely to set `active_descendant`;
    it never touches item paint.
  - `MenuModel.active` (:596) is an `Option<usize>` index into the `MenuItem` children
    (document order); `MenuMsg::Highlight`/`Open`/`Close` move it via `set_if_neq`
    (:627–653) → `Changed<MenuModel>` fires on every move and on close (`active=None`).
  - The gallery's items are **direct children** of the `Menu` (`examples/buiy_gallery/src/lib.rs:2988`),
    the same set `nth_menu_item` walks; the gallery menu **panel** is
    `ColorToken::SurfaceRaised` (`#1a1d22`, `lib.rs:2972`), not the framework default
    `menu_background()` = `SurfacePrimary`.
- The render layer has a first-class `Outline` component
  (`crates/buiy_core/src/render/components.rs:449`) — painted **outside** the border
  box, **not** part of the layout box (no layout shift), never clipped by the
  element's own clip. The focus system already uses it: `focus_ring_outline()`
  (`crates/buiy_core/src/focus.rs:142`) = `Outline { color: FocusRing, style: Solid,
  width: 2px, offset: 2px }`, and `lower_focus_ring` (:167) inserts/removes it keyed
  on `FocusedEntity`+`FocusVisible`, gated by a `FocusRingMarker` so it **never
  clobbers an author's own `Outline`**. `ColorToken::FocusRing` is contracted to be
  ≥ 3:1 and forced-colors-safe (theme.rs).

## Design intent (reference)

From `docs/reference-designs/widget-catalog/Widget Catalog.dc.html` (mirror
`docs/specs/2026-06-25-widget-catalog-values.md`):

- **Switch** (JS 625–627 modal register; 636–638 showcase):
  `switchTrack.background = swOn ? --ac : '#2a2f37'` with a `.15s` transition;
  `switchThumb.background = '#fff'` (constant), only `left` animates. `--ac` = the
  **live** accent ramp = `ColorToken::Accent` (moves with `SetAccent`), not the fixed
  `AccentBlue` swatch. `#2a2f37` is an unnamed one-off literal.
- **Menu item**: the reference renders items **flat** — there is *no* design token
  for an active/hover item background. An active indicator is an **added a11y
  affordance**, not design parity.

## Decisions

### N1 — recolor the `Switch` track (framework)

`update_switch_visual` gains a `SwitchTrack` `Background` write on the same
`Changed<A11yToggled>` edge that slides the thumb (widen its `tracks` query from
`Query<&Children, With<SwitchTrack>>` to `Query<(&Children, &mut Background), …>` and
write in the existing walk — the track entity is already in hand; no borrow conflict):

- `Toggled::True | Toggled::Mixed` → `ColorToken::Accent`
- `Toggled::False` → `ColorToken::SurfaceRaisedAlt`

`set_if_neq`-guarded (`if bg.color != want { bg.color = want }`).

Two supporting initializer changes so the **resting** (never-yet-toggled) state is
already correct at spawn (the driver only fires on change — and `Added` counts as
`Changed`, so a seeded switch like the S4 register switch, which seeds
`A11yToggled::True`, paints `Accent` on its first update regardless):

- `switch_background()` → `ColorToken::SurfaceRaisedAlt` (was `SurfaceSecondary`) —
  the OFF token, matching what the driver sets, so no first-toggle color jump.
- `switch_thumb_background()` → `ColorToken::White` (was `SurfacePrimary`) — the
  design's constant white thumb. **In scope for N1, not creep:** with an
  `Accent`/`SurfaceRaisedAlt` track a near-black thumb is dark-on-dark in the OFF
  state, undermining the legibility N1 exists to deliver; white is the design value
  and the only thumb legible on *both* track colors. Thumb color stays constant
  (initializer only, no driver logic).

Net: the default `Switch` matches the reference **colors** (ON=`Accent`,
OFF≈`SurfaceRaisedAlt`, thumb=`White`), identical to the showcase switch's custom
look. (Geometry — 40×20 / 16px thumb vs the reference's 40×23 / 17px — is
pre-existing and out of scope.)

**OFF-token note (I1):** the reference OFF literal is `#2a2f37`; `SurfaceRaisedAlt`
is `#1e2127` — a *slightly darker* named approximation, deliberately chosen for (a)
consistency with the already-shipped showcase switch, which makes the identical
`#2a2f37`→`SurfaceRaisedAlt` mapping, and (b) forced-colors safety (a `Custom`
literal is not forced-colors-safe). The `#2a2f37`→`SurfaceRaisedAlt` mapping is
recorded in `values.md` §7.2 so it is not re-litigated.

**Rejected alternative:** give the modal register switch custom track pixels + a
gallery reflect (the showcase pattern). Rejected — a gallery *workaround* that
leaves the framework `Switch` broken for every other consumer; N1's affected control
uses the *default* switch pixels, so the fix belongs in the framework.

### N2 — a focus-ring `Outline` on the active `Menu` item (framework)

Paint the active item's ring **inline inside `bind_menu_model`** (in `MenuSet::Bind`,
which runs after `MenuSet::Drain`, so it sees the same-frame fold), reusing the
`active_entity` that bind already resolves for `active_descendant`. It mirrors
`lower_focus_ring`, keyed on `MenuModel.active`:

- for any `MenuItem` carrying our `MenuActiveRing` marker that is **not** the active
  target → remove `Outline` + `MenuActiveRing`;
- for the active target, if it lacks the marker **and** has no author `Outline` →
  insert `menu_active_ring_outline()` + `MenuActiveRing`.

**Folded into the bind, not a sibling system (revised after CI).** The first
implementation added a separate `paint_menu_active_item` system in `MenuSet::Bind`.
That failed the **multi-threaded-executor** CI lane: adding *any* system to `Update`
perturbs the executor's ordering enough to flip the schedule-order-fragile resolved
position of a **size-0 hidden** tooltip node (the parked `InfoTip` primitive), which
two gallery layout snapshots pin — and the single-threaded and MT executors then
disagreed on the value (0,0 vs 0,6), so no snapshot value passed both lanes. Folding
the paint into the *existing* `bind_menu_model` adds **no** system, so the schedule
is byte-identical to before and the invisible node keeps its base position under both
executors (verified locally under `--features buiy_core/multi_threaded`). The ring
still has its own `MenuActiveRing` marker, so it is independently unit-testable via
`app.update()` (the tests drive `MenuModel` and assert the ring), and the underlying
snapshot fragility is logged as a follow-up.

`menu_active_ring_outline()` = `Outline { color: ColorToken::FocusRing, style: Solid,
width: 2px, offset: 0 }` — the framework focus ring, but **on the border box edge**
(offset 0) rather than the +2px outset `focus_ring_outline()` uses, because menu
items are full-width and tightly stacked; a +2px *outset* ring would overlap
neighbors / clip at the panel edge, while an edge ring reads cleanly. Width/color
match the canonical focus ring (WCAG 2.4.11 ≥ 2px; `FocusRing` ≥ 3:1). The exact
offset is confirmed by the GPU eyeball (below) and tuned to inset if the edge ring
crowds the panel.

**Why an `Outline`, not a background fill (the critical review finding, C1).** A
background *fill* cannot meet SC 1.4.11's 3:1 on Buiy's dark panels: the highlight
paints on `SurfaceRaised` (`#1a1d22`, gallery) or `SurfacePrimary` (`#0b0c0e`,
default), and *no* surface-family token reaches 3:1 against near-black — the first
draft's `SurfaceRaisedAlt` fill was ~1.05:1 on the gallery panel (imperceptible) and
would have failed the eyeball. `AccentSoft` (~1.3:1) also falls short; full `Accent`
would pass but requires recoloring the item's app-authored icon/label/kbd children
(a child-clobber problem the framework can't own). The `FocusRing` `Outline` is the
only option that (a) meets 3:1 by contract, (b) needs **no** `Background` or child
recolor — eliminating the clobber problem entirely — (c) causes **no** layout shift
(overlay, not box), and (d) is *semantically* the right affordance: the
active-descendant item is where roving focus conceptually lives, so the focus ring
is exactly the indicator a keyboard user expects.

This design touches **no** item `Background`, so the first draft's
`menu_item_background()` → `Transparent` change is dropped (unneeded). That function's
doc/code contradiction (doc says "transparent", code returns `SurfacePrimary`;
visually moot since the default panel is also `SurfacePrimary`) is logged as a
separate follow-up, not bundled here.

**Container-ring interaction (eyeball):** when a menu opens, `bind_menu_model` sets
`FocusedEntity = menu container` + `FocusVisible`, so `lower_focus_ring` already
rings the *container*. The bind's ring pass rings a different entity (the active
*item*) with a different marker — no conflict — but the two rings
coexist visually. The GPU eyeball confirms this reads acceptably; if the container
ring competes with the item ring, suppressing the container ring for
active-descendant containers is logged as a follow-up (pre-existing behavior, out of
scope here).

**Rejected alternative (altitude):** a **gallery** `reflect_menu_active_item`
mirroring `reflect_active_filter`/`reflect_menu_button_open_state`. It would fix the
catalog symptom with zero framework blast radius, but leaves the framework `Menu`'s
roving nav visually silent for every other consumer — the actual defect. The clean
framework fix has no golden/layout churn and no clobber, so fixing the widget is
strictly better than fixing its demo.

**Distinct from #115's rejection (S4):** #115 rejected a *framework* `MenuButton`
open-visual driver and kept that in the gallery, because the trigger's open *color*
is gallery styling with a design-parity target. N2 is different in kind: the
active-item indicator is a *widget-level WCAG affordance* with **no** design-parity
target (the reference is flat), owed to every consumer. Fixing it in the framework is
consistent with — not a reversal of — #115.

## Out of scope (logged as follow-ups, not fixed here)

- `menu_item_background()` returns `SurfacePrimary` despite its doc saying
  "transparent" — a latent doc/code contradiction (visually moot). New follow-up.
- No pointer-hover highlight for menu items (this note wires only the keyboard
  roving/active-descendant indicator).
- Disabled `Switch`: `update_switch_visual` is gated purely on `Changed<A11yToggled>`
  with no disabled check; if a disabled `Switch` state lands later it would still
  recolor. No disabled `Switch` exists today — new follow-up if/when it does.
- Container focus-ring vs active-item ring de-duplication (see above), if the eyeball
  shows it is warranted.

## Docs to update on landing

- Flip `docs/plans/follow-ups.md` N1 (~:1892) and N2 (~:1913) to **LANDED** with a
  link to this note (per the follow-ups convention — mark resolved, don't delete);
  add the new follow-ups listed above.
- Add this decision note to the `docs/README.md` master index.
- Cross-note in `docs/reports/2026-07-01-gallery-widget-catalog-audit.md` that N1/N2
  are resolved.
- Record the `#2a2f37`→`SurfaceRaisedAlt` switch-off mapping in `values.md` §7.2.

## Verification

- **New framework unit tests** (nothing asserts these colors/outlines today, so no
  re-blessing — verified by both spec reviews):
  - `switch.rs`: on `Changed<A11yToggled>`, `SwitchTrack.Background.color` flips
    `Accent` (on) ↔ `SurfaceRaisedAlt` (off); thumb stays `White`; a freshly-spawned
    off switch already has `SurfaceRaisedAlt` (resting state, no first-toggle jump).
  - `menu.rs`: after open + arrow-nav, the item at `MenuModel.active` carries an
    `Outline{ FocusRing }` + `MenuActiveRing`, every other item has neither, moving
    the active index moves the ring (old item cleared), and close (`active=None`)
    clears all rings. A menu with an author `Outline` on an item is not clobbered.
- **Gallery driven regression tests** (`examples/buiy_gallery/tests/interaction.rs`):
  the S4 modal register switch's track `Background` flips to `Accent` on toggle-on
  and `SurfaceRaisedAlt` on toggle-off; the S3 menu, opened + arrow-navigated,
  carries the ring on the active item and clears the previous one.
- **Headless gate**: `cargo test -p buiy_widgets` + `cargo test -p buiy_gallery`
  green; `fmt` + `clippy --workspace --all-targets` + `doc --workspace` clean.
- **GPU legs**: both `--ignored` lanes (`buiy_core`, `buiy_verify`) must stay green.
  No Switch/Menu golden or layout snapshot asserts these colors/outlines (a color
  change doesn't churn geometry-only `.snap`s, and an `Outline` adds no geometry), so
  this is a regression check, not a re-bless — but it must be run (rendered pixels
  change). Grep for any default-`Menu` display-list snapshot before landing (none
  expected).
- **GPU eyeball**: re-render the S4 modal (register switch toggled) + the S3 menu
  (open, arrow-navigated) to confirm the accent track and the active-item ring
  actually paint, and to tune the ring offset + judge the container-ring interaction.
