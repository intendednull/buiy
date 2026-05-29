**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Chronological history and design lineage

# History

## Pre-headless landscape (≤ Bevy 0.16, before September 2025)

Bevy shipped a `bevy_ui` crate from 0.1 (August 2020) onward, but **never an official widget kit**. `bevy_ui::widget::Button` existed as a tiny marker component for interaction state, but no headless widget catalog, no styled-widget kit, and no APG-conformant keyboard contracts.

The result: every third-party Rust GUI experiment in the Bevy ecosystem invented its own widget catalog. Major efforts (each with their own re-invention of button, slider, checkbox, etc.):

- **`bevy_egui`** (Lain-dono → Vladyslav Batyrenko, since 2021) — wraps `egui`, brings egui's widget catalog. Doesn't compose with `bevy_ui`.
- **`sickle_ui`** (UmbraLuminosa, since 2024) — opinionated widget kit on top of `bevy_ui`. Pre-dates bevy_ui_widgets; ships its own buttons, sliders, scrollbars, color picker, dropdown menu, etc.
- **`woodpecker_ui`** (StarArawn, since 2024) — React-style widget framework on top of `bevy_ui`.
- **`bevy_lunex`** (IDEDARY, since 2023) — 2D/3D UI kit with its own layout engine and widget set.
- **`iyes_ui_navigation`** — focus + navigation library; widget kit-adjacent.
- Many smaller crates (`belly`, `kayak_ui`, `bevy_ui_dsl`, etc.) — most archived or stagnant by 2025.

The 2023 [10 Challenges for Bevy UI Frameworks](https://github.com/bevyengine/bevy/issues/11100) (TimJentzsch) was the closest the community had to a benchmark; **no Rust UI library cleanly passed all 10 by late 2023**, including bevy_ui itself.

## Discussion #16900: "Standard Headless Widgets" (2024-12-19)

[Discussion #16900](https://github.com/bevyengine/bevy/discussions/16900), opened by **viridia** (Talin / Mike Schlossman, the most active widget-area Bevy collaborator), proposed:

- A curated collection of **foundational, unstyled** UI widgets for Bevy.
- Components emit events for state changes rather than managing state internally (the "controlled component" pattern).
- Framework-agnostic — vanilla Bevy components + observers + bubbling events.
- AccessKit integration first — screen reader support automatically.
- Minimal tier: atomic, low-level components (buttons, toggles, sliders); not complex assemblies (data tables, file pickers, virtual scrollers).
- Platform abstraction — unified interface across desktop, console, mobile via event-based architecture.

viridia's framing: *"headless UI component libraries provide high-quality widget implementations with no built-in styling."* — a direct reference to JS/TS prior art (Headless UI, Radix Primitives, ReaKit, React Aria).

**Major participants:** viridia (proposer), alice-i-cecile (Bevy maintainer; extensive technical feedback), Tsudico, muhammad-shozab.

The discussion produced the design rationale for both `bevy_core_widgets` (later renamed) and `bevy_feathers`.

## Bevy 0.17 (2025-09-30): first release

**Crate first published**: `bevy_ui_widgets 0.17.0-rc.1` on 2025-09-12, `0.17.0` on 2025-09-30, both by Mockersf / cart.

The crate was originally named **`bevy_core_widgets`**. [PR #20944](https://github.com/bevyengine/bevy/pull/20944) (viridia, merged ~2025-09-10) renamed it to `bevy_ui_widgets` to clarify the `bevy_ui` lineage. [PR #20972](https://github.com/bevyengine/bevy/pull/20972) (alice-i-cecile) marked the crate experimental immediately afterward.

**Widgets shipped in 0.17.0:** Button, Slider, Scrollbar, Checkbox, RadioButton + RadioGroup. Plus the `observe(...)` helper. Total: 5 widgets, 7 source files, 1,148 code lines.

**Companion crate `bevy_feathers 0.17.0` released the same day** — the styled-widget kit aimed at the in-engine editor.

**Bevy 0.17 release announcement** credits:
- bevy_ui_widgets: @viridia, @ickshonpe, @alice-i-cecile.
- bevy_feathers: @viridia, @Atlas16A, @ickshonpe, @amedoeyes.

The framing in the announcement was *"Headless Bevy UI Widgets (Experimental)"* — clearly tagged as in-motion.

## Bevy 0.17.x patch releases

- **0.17.1** (2025-10-01) — bug fixes.
- **0.17.2** (2025-10-04) — bug fixes.
- **0.17.3** (2025-11-17) — includes [PR #21835](https://github.com/bevyengine/bevy/pull/21835) (PPakalns, merged 2025-11-17) "Fix bevy_ui_widgets scrollbar bug where scrollbar_size wasn't taken into account."

## Bevy 0.18 (2026-01-13): Menu + Popover added; experimental flag removed

**0.18.0** published 2026-01-13 by cart. Source grew from 7 to 9 files (1,148 → 1,729 code lines). Two major additions:

- **`Popover`** positioning primitive — automatic placement-finding inspired by [Floating UI](https://floating-ui.com/). Credit: @viridia, @PPakalns.
- **`MenuPopup` + `MenuItem` + `MenuButton`** — popup menu widget, built on Popover, with keyboard nav and focus-state integration. Credit: @viridia, @PPakalns.
- **`RadioButton` / `RadioGroup` improvements** — better event propagation, keyboard activation. Credit: @viridia, @PPakalns.

[PR #21827](https://github.com/bevyengine/bevy/pull/21827) (DuckyBlender, merged 2025-12-09) added vertical-slider support to `Slider` via the new `SliderOrientation` enum.

[PR #22934](https://github.com/bevyengine/bevy/pull/22934) (alice-i-cecile, merged 2026-02-18) removed the `experimental` cargo-feature designation — bevy_ui_widgets is now compiled into `bevy::ui_widgets` by default. The `## Warning: Experimental` doc-comment in `lib.rs` remains; the crate is "experimental in API but compiled in by default."

**0.18.1** (2026-03-04) — patch release. Source: 1,732 code lines, 9 files. **This is the current stable as of 2026-05-22.**

Note: the brief stated "Latest stable: 0.18.1 (2026-05-13)." Verification: **2026-05-13 is the publish date of `0.19.0-rc.1`**, not 0.18.1. 0.18.1 was published 2026-03-04. Corrected.

## Bevy 0.19 (in flight): text input added

**0.19.0-rc.1** published 2026-05-13. **0.19.0-rc.2** published 2026-05-22 (today, by mockersf).

The major 0.19 addition is **`text_input.rs`** (529 lines) — the input handler that drives `bevy_text::EditableText`. Provides full keyboard editing (caret motion, selection, word-level nav, clipboard via Cmd-C/X/V, platform-aware modifiers), IME composition, and double/triple-click selection. Pairs with `bevy_text::EditableText` (which carries the data model + edit log) and `bevy_ui::widget::{scroll_editable_text, update_editable_text_layout, TextScroll}` (which handle visual layout + scroll).

[PR #23924](https://github.com/bevyengine/bevy/pull/23924) (fallible-algebra, merged 2026-04-22) added `FromTemplate` to most `bevy_ui_widgets` components — improving BSN-template compatibility ahead of BSN landing.

Total 0.19.0-rc.2: **2,652 code lines, 10 Rust files** — over 2× the 0.17.0 size in two releases.

## Cumulative widget timeline

| Bevy | Date | Added |
|---|---|---|
| 0.17.0 | 2025-09-30 | Button, Checkbox, RadioGroup/Button, Slider, Scrollbar, `observe()` |
| 0.17.1–0.17.3 | 2025-10 → 2025-11 | bug fixes (scrollbar size) |
| 0.18.0 | 2026-01-13 | + Popover, + MenuPopup/MenuItem/MenuButton, + vertical-slider, radio improvements |
| 0.18.0 | 2026-02-18 | `experimental` cargo feature removed (PR #22934) |
| 0.18.1 | 2026-03-04 | patch — current stable |
| 0.19.0-rc.1 | 2026-05-13 | + text_input (EditableText input handler), + `FromTemplate` derives |
| 0.19.0-rc.2 | 2026-05-22 | RC iteration |

## Implications for Buiy

- **The crate is ~8 months old, ships ~5 widgets at v1.x level of polish.** Buiy's foundation spec commits to ~50 widgets at F+C tier. The contrast is the calibration anchor: don't expect Buiy to ship its catalog in one release. The Bevy precedent is "5 widgets per release, ~3 releases to add 8 widgets." Buiy's plans should respect that cadence or articulate why a faster cadence is achievable (verification harness, BSN authoring, parallel agent dispatch).
- **The "experimental" warning has stuck for ~8 months even after the cargo-feature gate was removed.** The API is genuinely still in motion — `FromTemplate` derives were added in 0.19. Buiy locking against any specific API surface from bevy_ui_widgets would be paying churn cost; the per-window parallel-stack choice avoids it.
- **The 0.17 → 0.19 trajectory shows the "core widget set" wasn't enough.** Menu, Popover, text input each got added one release at a time. Buiy's foundation spec ships them all in v1 by construction — this is consistent with the parallel-stack rationale (no upstream constraint on cadence).

## Sources

- crates.io `bevy_ui_widgets` version history — https://crates.io/crates/bevy_ui_widgets (fetched 2026-05-22)
- Discussion #16900 — https://github.com/bevyengine/bevy/discussions/16900
- Bevy 0.17 announcement — https://bevy.org/news/bevy-0-17/
- Bevy 0.18 announcement — https://bevy.org/news/bevy-0-18/
- PR #20944 (rename `bevy_core_widgets` → `bevy_ui_widgets`) — https://github.com/bevyengine/bevy/pull/20944
- PR #20972 (mark experimental) — https://github.com/bevyengine/bevy/pull/20972
- PR #21827 (vertical slider) — https://github.com/bevyengine/bevy/pull/21827
- PR #21835 (scrollbar fix) — https://github.com/bevyengine/bevy/pull/21835
- PR #22934 (remove experimental flag) — https://github.com/bevyengine/bevy/pull/22934
- PR #23924 (FromTemplate derives) — https://github.com/bevyengine/bevy/pull/23924
- Issue #11100 (10 Challenges) — https://github.com/bevyengine/bevy/issues/11100
- Sibling: [`distribution.md`](distribution.md), [`../bevy-ui/history.md`](../bevy-ui/history.md), [`../bevy-feathers/`](../bevy-feathers/)
