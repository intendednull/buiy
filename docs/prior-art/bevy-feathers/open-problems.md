**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — what the crate structurally does not solve today

# Open problems

This file catalogs what `bevy_feathers` **structurally does not solve** as of 2026-05-22 — concrete gaps that an authoring spec or downstream consumer must work around. These are not bugs: they are scope and design choices (or non-decisions) that have not been closed. For higher-level critique see [`critiques.md`](critiques.md); for chronology see [`history.md`](history.md).

## APG widget coverage gaps

The feathers controls directory on `main` ships 14 widgets (button, checkbox, color plane, color slider, color swatch, disclosure toggle, menu, number input, radio, slider, text input, toggle switch, virtual keyboard) plus 5 containers and 2 display widgets. Against the **WAI-ARIA Authoring Practices Guide** pattern set, the following APG patterns are missing: **combobox, listbox, tree, treegrid, grid (data grid), tabs (auto- vs manual-activate variants), dialog (modal + non-modal), alertdialog, tooltip, toolbar, menubar, breadcrumb, accordion, carousel, date picker, time picker, file picker, color picker (with picker UI; feathers has the plane/slider/swatch primitives but no composed picker), feed, log, status, alert, meter, timer, progressbar, switch (feathers has toggle switch — slot is filled), splitter, table.**

This is roughly **30 APG patterns missing** out of a target of ~45–50 for "complete" web-platform parity. Buiy commits to the full catalog in [media-and-widgets.md § 3.10](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md).

## WCAG 2.2 AA per-widget coverage

No per-widget WCAG SC mapping is published. The specific gaps:

- **WCAG 1.4.3** (Contrast Minimum, 4.5:1 / 3:1 / 3:1 AA) — dark OKLCH palette is plausibly compliant but not validated by any contrast linter or CI gate.
- **WCAG 1.4.11** (Non-text Contrast, 3:1 AA) — focus rings and component-state boundaries not validated.
- **WCAG 2.4.7** (Focus Visible, AA) — focus rings missing per issue [#20047](https://github.com/bevyengine/bevy/issues/20047).
- **WCAG 2.4.11 / 2.4.12** (Focus Not Obscured, AA / AAA) — not validated.
- **WCAG 2.5.8** (Target Size, 24×24 AA) — token-level theme constants do not appear to encode this minimum.
- **WCAG 1.4.13** (Content on Hover or Focus — dismissable / hoverable / persistent) — feathers does not ship a tooltip widget.
- **WCAG 2.2.1 / 2.2.2 / 2.2.3** (timing / pause-stop-hide / interruptions) — feathers ships no live-region / carousel / toast widgets where these would apply.

The Buiy verification harness commits to per-WCAG-SC tests (see [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)); feathers does not.

## Forced-colors / `prefers-*` / reduced-motion handling

No documented response to OS user-preference signals:

- **`forced-colors` mode** — no system-color palette swap. The OKLCH theme is rendered regardless of OS forced-colors state.
- **`prefers-color-scheme`** — feathers ships only the dark variant; there is no automatic switch on OS preference.
- **`prefers-contrast`** — no high-contrast palette.
- **`prefers-reduced-motion`** — feathers has no animation primitives to gate, so this is vacuously honored, but as soon as animation arrives the preference will need plumbing.
- **`prefers-reduced-transparency`** — no policy.
- **`inverted-colors`** — no policy.

Buiy commits to surfacing all `prefers-*` queries as a `UserPreferences` resource bound to theme variants automatically ([architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)).

## Theme hot-reload

The Bevy 0.17 release notes describe feathers' theming as "simple/primitive." As of 0.18.1, theme tokens live in Rust source (`dark_theme.rs`, `tokens.rs`, `palette.rs`) and are compiled in. There is no documented path for hot-reloading a theme without recompiling. Bevy's asset hot-reload pipeline (which the editor uses heavily for textures/meshes) is not wired to theme assets.

## Animation / transition primitives

**There are no animation primitives in feathers.** The Bevy 0.18 release notes added `TryStableInterpolate` for animating `Val` (in `bevy_ui`, not feathers), and `AutoDirectionalNavigation` for arrow-key/gamepad nav (focus, not animation). Feathers itself ships no transition system, no keyframe timeline, no spring helpers, no layout-transition primitive.

Practical consequence: a feathers UI cannot animate widget state changes (open/close, focus/blur, value transitions) without manual frame-by-frame interpolation by the consumer.

## Drag-and-drop widgets

Feathers has **no drag-and-drop primitive widget**. `bevy_ui` provides low-level drag events (per `examples/ui/ui_drag_and_drop.rs`), but feathers does not expose a draggable widget, a drop target, a drag-handle pattern, or the APG-required keyboard-alternative for drag (WCAG 2.5.7 Dragging Movements, AA).

## File picker / dialog handling

No file picker widget. No native-file-dialog integration. No file-drop target. The virtual keyboard widget exists (touchscreen input) but the file-system-facing surface is empty. For an editor toolkit this is a notable gap — editors typically need open-file / save-file dialogs.

## Localization / i18n

`bevy_text` (which feathers transitively depends on) supports cosmic-text shaping, which handles **BiDi (UAX #9)**, RTL, and complex script shaping correctly. So feathers inherits BiDi-correct text rendering.

But feathers itself ships:

- No locale-aware formatters (no `NumberFormat`, `DateTimeFormat`, `RelativeTimeFormat`, etc.).
- No translation-string apparatus (no MessageFormat, no plural/gender/select handling).
- No `dir` analogue for full UI mirroring (icons, sliders, scrollbars).
- No `lang` annotation plumbed to AccessKit.

Buiy commits to ICU MessageFormat 2.0 + the full ECMAScript-spec set of locale-aware formatters ([cross-cutting.md § 3.12](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)).

## WASM / mobile target maturity

Feathers inherits `bevy_ui`'s platform matrix:

- **WASM:** UI renders, but AccessKit web adapter has not shipped — accessibility is degraded. No screen-reader bridge in the browser.
- **Android:** UI renders, but `accesskit_android` is in-progress upstream as of 0.17.
- **iOS:** UI renders, but UIAccessibility bridge is in-progress in AccessKit.

Practically, **feathers is desktop-first.** A studio targeting mobile or web cannot rely on the a11y guarantees the desktop adapters provide.

## Performance at 1000+ widget scale

No published feathers performance benchmarks. The underlying `bevy_ui` has documented "a few hundred microseconds per frame at moderate counts" in the cheat-book but no 1000+ node bench. For an editor showing a large entity tree or a productivity app with a deeply nested list, the perf envelope is unverified. See [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md) § "Performance critiques."

## Game-UI vs editor-UI applicability

The "Feathers _can_ be used in games, but that is not its motivating use case" framing is honest about a structural limitation:

- **Game HUD** — feathers has no built-in worldspace UI, no diegetic UI primitive, no 3D-anchored panel. A game HUD must be screen-space only, and screen-space-only HUDs in `bevy_ui` carry the `bevy_ui` renderer caps.
- **Game menus** — feathers' dark OKLCH palette is editor-branded; a game with its own art direction must rebuild the theme wholesale.
- **Settings menus** — feathers' widgets cover sliders / checkboxes / radios but not the full settings-UI catalog (key-binding remap, accessibility toggles per Game Accessibility Guidelines, etc.).
- **Animated UI** — none.
- **Controller-aware visuals** — partial; `AutoDirectionalNavigation` (Bevy 0.18+) lands in `bevy_ui` but not as a feathers-specific feature.

Buiy commits to game + app explicitly ([README.md goal 6](../../specs/2026-05-07-buiy-foundation/README.md)).

## bevy_feathers vs bevy_ui_widgets vs third-party confusion

For new Bevy users, **"which UI kit do I use?"** has no clean answer today:

- `bevy_feathers` — official, editor-targeted, experimental, dark-only.
- `bevy_ui_widgets` — official, headless, requires building visuals yourself.
- `bevy_egui` — third-party, mature, immediate-mode, paradigm-different.
- `sickle_ui` — third-party, retained-mode, richer widget set, no Foundation guarantee.
- `bevy_lunex` — third-party, parallel-stack, game-UI-targeted.
- `woodpecker_ui` — third-party, custom runtime.

The Bevy team has not published authoritative selection guidance (e.g. a comparison page on bevy.org). Issue [#24112](https://github.com/bevyengine/bevy/issues/24112) ("UI Migration Plan for Bevy Examples") is the closest the project has come — and it itself is open, signalling the confusion lives at the example-migration layer too.

## Sources

- `bevy_feathers` source main — `https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src`.
- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- Bevy 0.18 release notes — `https://bevy.org/news/bevy-0-18/`.
- Issue #20047 (focus rings) — `https://github.com/bevyengine/bevy/issues/20047`.
- Issue #24112 (UI examples migration) — `https://github.com/bevyengine/bevy/issues/24112`.
- Issue #24356 (NumberInput Update-schedule bug) — `https://github.com/bevyengine/bevy/issues/24356`.
- Issue #24369 (bevy_ui_widgets incomplete deps) — `https://github.com/bevyengine/bevy/issues/24369`.
- WAI-ARIA Authoring Practices Guide — `https://www.w3.org/WAI/ARIA/apg/`.
- WCAG 2.2 — `https://www.w3.org/TR/WCAG22/`.
- AccessKit — `https://accesskit.dev`.
