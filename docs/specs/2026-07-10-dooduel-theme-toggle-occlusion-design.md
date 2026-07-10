# Dooduel — the in-game theme toggle occludes the chat "Send" button

- **Date:** 2026-07-10
- **Status:** active
- **Area:** apps/dooduel (view layer)
- **Found by:** the QA seat driver (Track 1 of the playtest cycle, commit `4b421a7`'s gate run)

## The bug

On the in-game screen at 1280×800 (desktop 3-pane), a real pointer click at the chat
**Send** button's center activates the **theme toggle** instead of submitting the guess.
Observed live: the seat flipped Light→Dark and the guess stayed in the field. Measured
semantic rects: toggle @(1172,730) 88×50, Send @(1194,736) 41×56 — Send's center
(1214,764) lies inside the toggle's rect.

## Root cause

The floating theme toggle is stacked at the **view root over every screen**:
`apps/dooduel/src/view/mod.rs:39` — `column![content, widgets::theme_toggle(s)]`. The
toggle (`widgets.rs:213`) is a `.fixed().top_layer()` pill pinned to the bottom-right
corner (`.inset_right(20).inset_bottom(20)`). On every screen except in-game the
content is centered, so the corner is empty and the toggle floats harmlessly. **In-game
is the one screen whose content fills the bottom-right corner**: the chat pane is the
rightmost 300px pane and its input row (`in_game.rs:684-700`) puts **Send** in exactly
that corner. The two independently-authored bottom-right widgets collide, and because
the toggle is in the **top layer** it wins the hit-test at the overlap — the click folds
`SetTheme` rather than `SubmitGuess`. This is a real app-level geometric overlap, not
solely a framework picking quirk.

### Pick vs paint (a documented framework trait, not the defect)

They *disagree* at the overlap, which is why Send looked clickable:
- **Paint:** the toggle's pill *quad* is top-layer, so it paints over the chat quads.
  But all glyphs render in one global tier **after** all quads
  (`view/mod.rs:8-11`), so the "Send" label paints on top of the toggle and reads as
  legible/clickable.
- **Pick:** the top-layer toggle sorts above the normal-flow Send button, so the hit
  resolves to the toggle. (The toggle *container* is `.ignore_picking()`, but its pill
  *child* button is not — the child is what wins.)

So paint says "Send on top", pick says "toggle on top". The glyph-tier property is an
existing, documented framework trait; the fixable defect is the app-level corner
collision. (Logged for the framework track, not fixed here.)

## Design ground truth

The design **keeps** the toggle in-game: it is a global `position:fixed; bottom:20px;
right:20px; z-index:900` element rendered outside the screen switch
(`docs/reference-designs/dooduel/Dooduel Prototype.dc.html:547`), present on every
screen including in-game (desktop `In-game (desktop)` wrapper at line 307). The design
avoids occluding Send only through **precise fixed sizing**: chat pane `height:556px`,
total in-game content ≈787px, so at exactly 800px height the Send button's bottom lands
~1px above the toggle's 20px bottom-margin band. That 1px clearance is fragile and
viewport-height-specific; our layout (taller top bar/header) pushes Send *into* the
band, and our toggle's pick rect is ~16px taller than the design's (button padding)
besides.

## Candidate fixes

- **A — Suppress the toggle on the InGame screen** (`when(s.screen != Screen::InGame,
  theme_toggle(s))`). Height-independent; uniform across the desktop 3-pane **and** the
  mobile single-column (both put the chat/Send bottom-right); one-line change.
- **B — Reposition the toggle in-game** (bottom-left, or move it into the existing dark
  top bar). Keeps it reachable in-game, but diverges from the design's "bottom-right on
  every screen" and adds per-screen inconsistency; the top-bar move is a larger surface
  change.
- **C — Reserve layout space so Send always clears the toggle band** (reproduce the
  design's Send-above-toggle relationship). Most faithful to the design, but reproduces
  its fragile ~1px clearance: it re-breaks at any window height ≠ 800 and in the mobile
  layout, and our padded toggle rect would still clip a design-matched Send.

## Recommendation: A (suppress in-game)

A is the smallest change that fully closes the bug at **all** window sizes and in
**both** layouts. The in-game screen is the only screen whose primary controls occupy
the toggle's corner, and the theme choice **persists** (reducer-owned `SetTheme`), so
the player's menu choice carries into the match — nothing is lost mid-game beyond the
ability to re-toggle.

**Rejected:** C is not robust (fragile 1px clearance, breaks on resize + mobile). B
diverges from the design and adds inconsistency. Protecting the primary control (chat
Send) outweighs keeping a marginal toggle in the one screen where it cannot coexist with
the controls.

**Acknowledged divergence:** the design keeps the toggle in-game; suppressing it there is
deliberate and documented. A future in-game settings affordance (or a top-bar theme
control) could restore in-game theme switching without occluding the chat — logged as a
follow-up, not built here.

## Regression-test strategy

A sibling test `apps/dooduel/tests/in_game_occlusion.rs`, using the `canvas_e2e.rs`
unified headless driver (real `bevy_picking` + a synthetic 1280×800 window), navigates
to the in-game Drawing phase (`StartMatch` → `ChooseWord(0)`) and asserts:
1. a real synthetic pointer click at the **Send** button's resolved center does **not**
   flip the theme (the live symptom), and
2. the theme toggle button is **absent** in-game (`get_by_role(Button, "Light")` errors).

The existing `theme_toggle.rs` tests exercise Home (toggle still present there) and stay
valid unchanged.
