**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui_widgets — Bevy's official headless widget primitives crate

# bevy_ui_widgets

`bevy_ui_widgets` ships Bevy's official **unstyled, behavior-only widget primitives**: ECS components + observer-driven event plumbing that implement the *interaction logic* of common widgets (button, checkbox, slider, etc.) without rendering them. The companion `bevy_feathers` crate styles these primitives for the in-engine editor; downstream apps may consume the headless primitives bare and supply their own visuals. The crate is sibling to `bevy_ui` (the layout/render substrate) and `bevy_feathers` (the styled-widget kit). Introduced in Bevy 0.17, still labeled "Experimental" in the source even after the `experimental` cargo-feature designation was lifted in 0.18.

## Key facts

| | |
|---|---|
| Crate | `bevy_ui_widgets` (in the [bevyengine/bevy](https://github.com/bevyengine/bevy) monorepo at `crates/bevy_ui_widgets/`) |
| Latest stable | **0.18.1** (published 2026-03-04 — *not 2026-05-13 as the brief stated; that's the 0.19.0-rc.1 publish date*) |
| Pre-release | **0.19.0-rc.2** (published 2026-05-22, today) |
| First release | **0.17.0** (2025-09-30, Bevy 0.17 ship date) |
| Original name | `bevy_core_widgets` → renamed to `bevy_ui_widgets` in [PR #20944](https://github.com/bevyengine/bevy/pull/20944) (viridia, 2025-09-10), pre-0.17 |
| Description | "Unstyled common widgets for Bevy Engine" |
| License | MIT OR Apache-2.0 |
| Total downloads (crates.io) | 201,008 |
| Recent downloads (90d) | 177,568 |
| Source size (0.19.0-rc.2) | 2,652 code lines, 125 comment lines, 10 Rust files |
| Crate owners (publish rights) | mockersf, cart, bevyengine:publish team |
| Plugin group | `UiWidgetsPlugins` (plural — note that, despite the brief, the type is `UiWidgetsPlugins` not `UiWidgetsPlugin`) |
| Stability | `lib.rs` carries `## Warning: Experimental`; "API is likely to change substantially: be prepared to migrate your code." The cargo-feature `experimental` *gate* was removed in [PR #22934](https://github.com/bevyengine/bevy/pull/22934) (alice-i-cecile, 2026-02-18) for 0.18, but the source-level warning remains. |
| State-management model | **External**. Widgets do not maintain their own value/checked/etc. state internally; they emit `EntityEvent`s (`Activate`, `ValueChange<T>`) and the app updates Bevy components in response. Explicit non-goal: two-way data binding. |

## What ships (per `crates/bevy_ui_widgets/src/lib.rs` on `main` @ 2026-05-22)

| Widget | Module | Since | APG pattern | Source size |
|---|---|---|---|---|
| Button | `button` | 0.17 | [button](https://www.w3.org/WAI/ARIA/apg/patterns/button/) | ~140 lines |
| Checkbox | `checkbox` | 0.17 | [checkbox](https://www.w3.org/WAI/ARIA/apg/patterns/checkbox/) | ~280 lines |
| RadioGroup + RadioButton | `radio` | 0.17 | [radio](https://www.w3.org/WAI/ARIA/apg/patterns/radio/) | ~210 lines |
| Slider (+ SliderThumb, SliderValue, SliderRange, SliderStep, SliderPrecision, SliderDragState, TrackClick, SliderOrientation) | `slider` | 0.17 (vertical added 0.18 via [#21827](https://github.com/bevyengine/bevy/pull/21827)) | [slider](https://www.w3.org/WAI/ARIA/apg/patterns/slider/) | 754 lines |
| Scrollbar (+ ScrollbarThumb, ScrollbarDragState, ControlOrientation) | `scrollbar` | 0.17 | scrollbar (ARIA role; no APG pattern) | ~520 lines |
| MenuPopup + MenuItem + MenuButton (+ MenuEvent, MenuAction, MenuFocusState, MenuLayout) | `menu` | 0.18 | [menu / menubar](https://www.w3.org/WAI/ARIA/apg/patterns/menubar/) | 474 lines |
| Popover (positioning primitive, NOT a widget per se) | `popover` (the only `pub mod`) | 0.18 | n/a — positioning substrate | 326 lines |
| EditableText input handler (drives `bevy_text::EditableText`) | `text_input` | 0.19 | [textbox](https://www.w3.org/WAI/ARIA/apg/patterns/textbox/) (single-line and `combobox` see notes) | 529 lines |
| `observe(...)` helper bundle | `observe` | 0.17 | infrastructure | ~100 lines |

**Notably NOT shipped:** Toggle (use Checkbox + AccessKit Switch role override), Tabs, Accordion, Disclosure, Tooltip, Combobox, Listbox, Dialog, AlertDialog, ProgressBar, Tree, Treegrid, Toolbar, Breadcrumb, DatePicker / TimePicker / ColorPicker (Feathers ships its own ColorPlane), Spinbutton, Carousel, Card, Rating, Meter, FilePicker, Searchbox, virtual-keyboard primitive (the example `virtual_keyboard.rs` is hand-rolled). See [`apg-coverage.md`](apg-coverage.md) for the gap map.

## How to use this folder

Canonical reading order:

1. [`README.md`](README.md) — this file.
2. [`architecture.md`](architecture.md) — the headless-widget primitive pattern: components-for-state, observers-for-events, no rendering. Plugin shape. Relationship to `bevy_ui` and `bevy_feathers`.
3. [`widgets.md`](widgets.md) — per-widget enumeration: components, events, APG keyboard contract, state requirements.
4. [`api.md`](api.md) — component / event / observer API conventions. How an app wires custom styling over the headless components.
5. [`apg-coverage.md`](apg-coverage.md) — WAI-ARIA APG conformance map; coverage gaps; comparison to Buiy's planned APG breadth.
6. [`integration.md`](integration.md) — `app.add_plugins(UiWidgetsPlugins)` setup; cargo features; coexistence patterns.
7. [`history.md`](history.md) — pre-headless landscape, discussion [#16900](https://github.com/bevyengine/bevy/discussions/16900), 0.17 → 0.18 → 0.19 evolution.
8. [`distribution.md`](distribution.md) — license, MSRV, release cadence (tied to Bevy), area SMEs, governance.
9. [`open-problems.md`](open-problems.md) — critiques (scope, BSN-friendliness, discoverability) + open problems (APG coverage, gamepad/touch maturity, themability beyond Feathers, perf at scale, BiDi, WASM).
10. [`ecosystem.md`](ecosystem.md) — comparisons to `bevy_feathers`, third-party kits, Buiy's own catalog; production-usage observations (nascent).
11. [`lessons.md`](lessons.md) — **the consult-this-when-designing file.** validates / avoid / borrow for Buiy.
12. [`glossary.md`](glossary.md) — terms.

## Glossary stub

- **Headless widget** — interaction logic (state components + event observers) with no rendering/styling. The downstream consumer composes a renderer.
- **External state management** — widget does not own state; emits events; app updates ECS components in response. Avoids two-way data binding.
- **Observer** — Bevy ECS callback that runs in response to an `EntityEvent`; the primary plumbing primitive for bevy_ui_widgets.
- **`UiWidgetsPlugins`** — the `PluginGroup` that registers all widget plugins. Individual `*Plugin`s can be added singly.
- **`Activate` / `ValueChange<T>`** — the two crate-level `EntityEvent` types.
- **APG** — [WAI-ARIA Authoring Practices Guide](https://www.w3.org/WAI/ARIA/apg/). The contract source for widget behavior. Full glossary in [`glossary.md`](glossary.md).

## Framing disclosure

These docs are written from a **parallel-stack** stance — Buiy ships its own widget catalog under its own component model, *not* a layer over `bevy_ui_widgets`. The "Implications for Buiy" sub-sections frame `bevy_ui_widgets`'s choices through that lens. Coexistence is per-window (per [foundation/cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)), not a layering. Future readers auditing whether parallel-stack is itself the right primitive should weigh the corpus accordingly: it's a learn-from-bevy_ui_widgets-into-Buiy-parallel-stack artifact, not a neutral catalog.

## Sources

- Cargo.toml on `main` — https://raw.githubusercontent.com/bevyengine/bevy/main/crates/bevy_ui_widgets/Cargo.toml
- `lib.rs` on `main` — https://raw.githubusercontent.com/bevyengine/bevy/main/crates/bevy_ui_widgets/src/lib.rs
- crates.io API metadata — https://crates.io/api/v1/crates/bevy_ui_widgets (fetched 2026-05-22)
- Bevy 0.17 release announcement — https://bevy.org/news/bevy-0-17/
- Bevy 0.18 release announcement — https://bevy.org/news/bevy-0-18/
- Discussion #16900 "Standard Headless Widgets" — https://github.com/bevyengine/bevy/discussions/16900
- PR #20944 (rename `bevy_core_widgets` → `bevy_ui_widgets`) — https://github.com/bevyengine/bevy/pull/20944
- PR #22934 (remove `experimental` feature flag) — https://github.com/bevyengine/bevy/pull/22934
- PR #21827 (vertical slider) — https://github.com/bevyengine/bevy/pull/21827
- PR #21835 (scrollbar fix) — https://github.com/bevyengine/bevy/pull/21835
- PR #23924 (`FromTemplate` derive) — https://github.com/bevyengine/bevy/pull/23924
- Buiy foundation spec: media & widgets — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy foundation spec: interaction — [`../../specs/2026-05-07-buiy-foundation/interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)
- Buiy foundation spec: architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling prior-art: [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../bevy-feathers/`](../bevy-feathers/), [`../bevy-a11y/`](../bevy-a11y/), [`../bevy-picking/`](../bevy-picking/), [`../accesskit/`](../accesskit/)
