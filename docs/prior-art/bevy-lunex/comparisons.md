**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — comparison to bevy_ui, sickle_ui, woodpecker_ui, bevy_egui, kayak_ui, Buiy

# Comparisons

bevy_lunex occupies the **parallel-stack, worldspace-first** slot of the Bevy UI landscape. This file places it row-by-row against its closest peers and against Buiy. Each row covers the elevator pitch plus the **key design difference** — the load-bearing fact a Buiy spec author needs.

## At a glance

| Kit | Paradigm | Built on | Worldspace UI? | Layout engine | Accessibility | Status |
|---|---|---|---|---|---|---|
| `bevy_ui` | Retained, screen-space | Bevy core | No | Taffy (flexbox/grid/block) | `bevy_a11y` + AccessKit (in core) | Official, stable |
| `bevy_feathers` | Themed widget kit | `bevy_ui` + `bevy_ui_widgets` | No | Inherits `bevy_ui` | Inherits `bevy_a11y` | Official, experimental |
| `sickle_ui` | Themed widget builder | `bevy_ui` | No | Inherits `bevy_ui` | Inherits `bevy_a11y` | Third-party, active |
| `woodpecker_ui` | Reactive declarative | Own runtime + vello | No | Custom | None documented | Third-party, active |
| `bevy_egui` | Immediate-mode | Own render path | No | egui's internal | None | Third-party, mature, dominant |
| **`bevy_lunex`** | Retained, `Transform`-based | Own runtime + `bevy_picking` | **Yes (first-class)** | Anchored / percent (custom) | **None** | Third-party, single-maintainer |
| `kayak_ui` | Custom DSL widget tree | Own runtime | No | Custom | None | **Archived 2024** |
| **Buiy** | Parallel-stack, web-platform-parity | Taffy + cosmic-text + AccessKit + bevy_picking + Bevy render graph directly | **Yes (planned)** | Taffy (flexbox/grid/block) | AccessKit-first, per-widget WCAG SC mapping | Pre-foundation (spec phase) |

## vs `bevy_ui` (parallel stacks, transform vs Taffy)

`bevy_ui` is the official Bevy UI engine — Taffy-based screen-space layout, full a11y integration via `bevy_a11y` + AccessKit, full ECS integration, and the slot that `bevy_feathers` / `bevy_ui_widgets` build on. It is the default and the volume leader (4.9M downloads).

**Key design difference.** Layout engine and coordinate system. `bevy_ui` runs Taffy and produces screen-space rectangles via a separate `UiTransform`; bevy_lunex does anchored layout directly against general `Transform`. The consequence is that bevy_lunex can put UI in 3D space (and reuse `bevy_picking`'s mesh backend for hit-testing) but loses flexbox/grid/block. `bevy_ui` is structurally screen-space — adding worldspace UI requires a render-path fork. The two systems are mutually-exclusive within one UI tree but can coexist per-window (different windows, different stacks).

## vs `sickle_ui` (extends bevy_ui vs parallel)

`sickle_ui` is the most-developed third-party widget kit that **extends `bevy_ui`** rather than replacing it. It ships tabs, scrollboxes, color pickers, tree views, dockable panels, a token-based theming system, and a builder API. It is the closest direct ancestor of `bevy_feathers` in design space.

**Key design difference.** Where bevy_lunex bypasses `bevy_ui`, `sickle_ui` sits on top of it. The implications:

- `sickle_ui` inherits `bevy_ui`'s strengths (Taffy, a11y) and weaknesses (no worldspace UI, no `Material` integration).
- `sickle_ui` ships a far broader widget catalog than bevy_lunex (which ships none).
- `sickle_ui` cannot do worldspace UI; bevy_lunex cannot do flexbox.

For an application-shaped UI, `sickle_ui` is the closer match. For an in-game HUD that needs 3D placement, bevy_lunex is the only third-party option.

## vs `woodpecker_ui` (parallel custom declarative)

`woodpecker_ui` is the spiritual successor to `kayak_ui` — a declarative widget framework with its own runtime, vello-based rendering, and JSX-style macro composition. It is parallel-to-`bevy_ui` (same stance as bevy_lunex) but takes a different paradigmatic bet: **reactive/declarative**, not retained-component.

**Key design difference.** Composition model. `woodpecker_ui` re-implements layout, rendering, and event handling inside its own subsystem and renders via vello (a 2D vector renderer); bevy_lunex uses Bevy's `bevy_sprite` render path. `woodpecker_ui` uses a custom DSL via macros; bevy_lunex uses plain ECS. Both are parallel-stack, but woodpecker recreates the rendering wheel for vector graphics flexibility while bevy_lunex recreates the layout wheel for worldspace placement.

For Buiy: woodpecker's vello-renderer choice is one parallel-stack option; bevy_lunex's bevy_sprite-renderer choice is another; Buiy's "own render graph directly" is a third. See `comparisons.md` in Buiy's foundation specs for the architectural decision.

## vs `bevy_egui` (immediate-mode, different paradigm)

`bevy_egui` wraps the `egui` immediate-mode GUI library and renders it into Bevy windows. It is the **most-used UI kit in the Bevy ecosystem by a wide margin** — by far the dominant choice for in-game debug overlays, tool UIs, and developer-facing menus.

**Key design difference.** Immediate-mode vs retained-mode. `bevy_egui` rebuilds the widget tree every frame from imperative calls (`ui.button("Click me")`) and does **not** integrate with `bevy_ui`'s Taffy layout, `bevy_a11y`'s AccessKit tree, or `bevy_picking`. bevy_lunex (and Buiy) are retained, ECS-native, picking-integrated. The paradigms are mutually exclusive in a single UI tree but coexist per-window. `bevy_egui` is the right choice for tool UIs and one-off debug panels; bevy_lunex is the right choice for game-world-embedded UI.

## vs `kayak_ui` (the archived cautionary tale)

`kayak_ui` was the most ambitious third-party Bevy UI kit through 2022–2023 — a retained widget tree with a custom DSL, hot-reload support, and an active community. It was **archived in 2024** when its primary maintainer (StarArawn) stepped back and no successor maintainer emerged. Stars at archive time were comparable to bevy_lunex's current 913.

**Key design difference vs bevy_lunex:** none structurally — both are/were parallel-stack, custom-runtime, single-maintainer projects. The differences are timing and survival. The structural lesson for Buiy: **the kayak_ui shape is a recurring failure mode** in the Bevy UI ecosystem. Single-author projects with ambitious scope and no foundation backing accumulate dependent users, then evaporate. bevy_lunex is currently on the same trajectory; whether it follows kayak_ui's archive fate or finds a co-maintainer is unresolved as of 2026-05-22.

## vs Buiy (parallel-stack, web-platform-parity, AccessKit-first)

bevy_lunex and Buiy share the **parallel-stack** stance and the **worldspace-UI** ambition but diverge on every other design bet:

| Axis | bevy_lunex | Buiy |
|---|---|---|
| **Built on** | Own layout (anchored/percent) + `bevy_picking` + `cosmic-text` + `bevy_rich_text3d` (opt-in) + `bevy_sprite` render path | Taffy + cosmic-text + AccessKit + bevy_picking + Bevy render graph directly |
| **Layout engine** | Anchored / percent (no flexbox, no grid, no block) | Taffy (full flexbox, CSS Grid, block, intrinsic sizing) |
| **Scope** | Worldspace game UI; explicit "poor fit for desktop apps" per book | Game **and** app, comprehensive web-platform-parity catalog per [README goal 6](../../specs/2026-05-07-buiy-foundation/README.md) |
| **Worldspace UI** | First-class — `Transform`-positioned UI nodes, 3D text via `bevy_rich_text3d` | Planned first-class via general `Transform` ([cross-cutting.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) |
| **Coordinate system** | One — UI nodes are normal `Transform` entities | One — same stance |
| **Pixel-perfect 2D** | Awkward — anchored layout in world coords; HiDPI is dev's problem | First-class — Taffy in screen-space mode with scale-factor integration |
| **Accessibility** | **None.** No AccessKit dep, no role/label/state/relation model, no focus ring, no screen-reader path | AccessKit-first from day one. Decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`. Per-widget WCAG 2.2 SC mapping in [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md). |
| **Widget catalog** | None shipped — developer builds from primitives | Full APG catalog (button, link, listbox, combobox, slider, tabs, dialog, popover, menu, tree, grid, etc.) per [media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) |
| **Theming** | None — manual styling per-component | Token system, OS-preference-driven, subtree overrides, hot-reload, contrast linter |
| **Animation** | None — manual `Transform`-tween systems; one `Text animation` component added 2025-05 | Transitions, keyframes, layout transitions, springs, reduced-motion-gated |
| **Picking** | `bevy_picking` integration since 0.3 | `bevy_picking` integration (planned, same stance) |
| **Text** | `cosmic-text` (2D) + `bevy_rich_text3d` (3D, opt-in `text3d` feature) | `cosmic-text` direct, with IME and BiDi commitments per [text.md](../../specs/2026-05-07-buiy-foundation/text.md) |
| **Render path** | Through `bevy_sprite` and `bevy_text` | Buiy's own render-graph nodes, directly integrated |
| **DSL / declarative API** | None — open issue #10 since 2023; maintainer prefers imperative | Open — BSN-friendly required-components stance |
| **Hot reload** | None — open issue #11 since 2023 | Planned (asset-based theming hot-reload at minimum) |
| **Status** | Active third-party, single-maintainer, no funding, 913 stars, 40K downloads | Pre-foundation, single-author (intendednull), spec phase |

The **design bets that diverge most sharply**:

1. **Layout engine.** bevy_lunex bets that anchored-only is enough for game UI and worth the "no flexbox" cost. Buiy bets that Taffy + worldspace mode is achievable and worth the integration cost. The deciding factor: whether dense application UIs are in scope. Buiy says yes; bevy_lunex says no.

2. **Accessibility.** bevy_lunex bets accessibility is out of scope for "game UI." Buiy bets it is non-negotiable from day one. The deciding factor: whether "ship to non-gamers" is in scope. Buiy says yes; bevy_lunex says no.

3. **Widget catalog.** bevy_lunex bets developers will hand-roll widgets per game. Buiy bets a shared catalog is necessary to make the ecosystem productive. The deciding factor: whether Buiy is a foundation for a widget ecosystem (yes per spec) or just a layout/render engine (bevy_lunex's choice).

4. **Render path.** bevy_lunex bets `bevy_sprite` is enough and inherits its capabilities. Buiy bets owning the render path is necessary for web-platform-parity (backdrop-filter, mix-blend-mode, true top layer, non-rect clipping, gradients in any color space). The deciding factor: whether closing the web-platform feature gap is in scope. Buiy says yes; bevy_lunex says no.

The **design bet Buiy adopts from bevy_lunex**: parallel-stack stance, worldspace-UI via general `Transform`, `bevy_picking` integration. These are bevy_lunex's load-bearing wins and Buiy inherits them by alignment.

## Sources

- bevy_lunex repo — `https://github.com/bytestring-net/bevy-lunex`.
- bevy_lunex Cargo.toml — `https://raw.githubusercontent.com/bytestring-net/bevy-lunex/main/crate/Cargo.toml`.
- Bevy Lunex book limitations — `https://bytestring-net.github.io/bevy_lunex/`.
- `bevy_ui` crates.io — `https://crates.io/crates/bevy_ui`.
- `sickle_ui` repo — `https://github.com/UmbraLuminosa/sickle_ui`.
- `woodpecker_ui` repo — `https://github.com/StarArawn/woodpecker_ui`.
- `bevy_egui` repo — `https://github.com/vladbat00/bevy_egui`.
- `kayak_ui` archived repo — `https://github.com/StarArawn/kayak_ui`.
- "How do Nice UI in Bevy?!?" — `https://deadmoney.gg/news/articles/how-do-nice-ui-in-bevy`.
- Buiy specs — `../../specs/2026-05-07-buiy-foundation/`.
