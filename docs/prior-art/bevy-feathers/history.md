**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — chronological history (pre-history → Bevy 0.19-rc)

# History

`bevy_feathers` is the youngest crate in the Bevy UI family. It debuted in **Bevy 0.17 (2025-09-30)** as the upstream answer to a years-long gap: Bevy had no official widget kit. The crate is opinionated, narrow-scope, and explicitly framed in its own description as targeting "editors and utilities" — not games (per the Bevy 0.17 release notes: "Feathers _can_ be used in games, but that is not its motivating use case"). This file traces the pre-history, the introduction, and the per-release additions through 0.19-rc.

## Pre-feathers landscape (2020–2025)

Bevy shipped `bevy_ui` with the engine starting in **0.4 (2020-12-19)** — see [`../bevy-ui/history.md`](../bevy-ui/history.md). But `bevy_ui` exposed primitive widgets only: `Button`, `Image`, `Text`, `Label`, raw `Node`. There was no listbox, combobox, slider, checkbox, radio group, tab, dialog, popover, or menu. The community filled the gap with third-party kits, each making a different paradigm bet:

| Kit | First release | Paradigm | Notes |
|---|---|---|---|
| `kayak_ui` | 2022-04 | Retained widget tree, custom DSL | **Archived 2024** — see [`comparisons.md`](comparisons.md). |
| `bevy_egui` | 2021-01 | Immediate-mode wrapper around `egui` | Different paradigm — not built on `bevy_ui`. |
| `bevy_lunex` | 2023 | Parallel UI, `Transform`-based | Not built on `bevy_ui`. |
| `sickle_ui` | 2024 | Themed widgets on top of `bevy_ui` | Closest direct ancestor in design space. |
| `woodpecker_ui` | 2024 | Custom declarative API | Successor to `kayak_ui` ideas. |

By **late 2023** the gap was canonized as Bevy's "**10 Challenges for Bevy UI Frameworks**" (TimJentzsch, discussion [#11100](https://github.com/bevyengine/bevy/issues/11100), open since 2023-12) — explicit acknowledgement that no kit, official or third-party, cleanly demonstrated all ten. The challenge list is the closest the Bevy ecosystem has to a UI capability benchmark and remains open (see [`open-problems.md`](open-problems.md)).

## The Standard Headless Widgets initiative (2024–2025)

Discussion [#16900](https://github.com/bevyengine/bevy/discussions/16900) — "Standard Headless Widgets" — opened **2024-12-19** by **@viridia**. The proposal: a set of unstyled, accessible primitives (button, toggle, slider, text input) built on `bevy_a11y` + AccessKit, supporting desktop / console / mobile, with controlled-not-uncontrolled state via events. The "headless" framing (behavior + accessibility but no opinion on visuals) became the architectural seed for `bevy_ui_widgets`.

Through Q1 / Q2 2025, viridia shipped a prototype implementation and the design discussion converged on:

- Controlled components emit events; do not own internal state.
- State changes for accessibility require explicit propagation.
- Radio-group mutual exclusion lives at the group, not the button.
- Integration is with `bevy_input_focus` (focus) and `bevy_picking` (hit testing).

The prototype shipped to crates.io as `bevy_ui_widgets 0.17.0-rc.1` on **2025-09-12** — the first public release.

## PR #19730 — feathers lands in Bevy 0.17 (2025-09-30)

PR [#19730](https://github.com/bevyengine/bevy/pull/19730) — "Bevy Feathers: an opinionated widget toolkit for building Bevy tooling" — was opened by **@viridia** (primary author, verified against the GitHub PR API; pre-amble's "ickshonpe" attribution is incorrect, as is a similar attribution in `prior-art/bevy-ui/history.md`). Merged **2025-06-28** by **@alice-i-cecile** with merge comment "I'm happy with this as a base...let's get this merged and let people start experimenting :)". Co-authors per the Bevy 0.17 release notes: **viridia, Atlas16A, ickshonpe, amedoeyes**.

The PR landed:

- Widget controls: button, checkbox, slider, radio.
- Theming framework with design tokens and a dark theme in **OKLCH** color space.
- Predefined palette including axis-specific X/Y/Z indicator colors (editor-specific).
- Inheritable font + text-color styling (`font_styles.rs`).
- An end-to-end example demonstrating the toolkit.

`bevy_ui_widgets` was merged in parallel and ships in the same Bevy 0.17 release. From the 0.17 release notes: "It builds on top of Bevy's new general-purpose 'headless' widget set: `bevy_ui_widgets`." Co-authors on that crate per the 0.17 notes: **viridia, ickshonpe, alice-i-cecile**.

The "editors and utilities" framing was explicit from day one — both in the crate description (`description = "A collection of UI widgets for building editors and utilities in Bevy"`) and the 0.17 release-notes prose.

## Bevy 0.18 (2026-01-13)

Per the 0.18 release notes:

- **`ColorPlane`** widget — a two-dimensional color picker selecting two channels within a color space. Built on the new `color_plane.rs` / `color_slider.rs` / `color_swatch.rs` modules.
- **`Popover`** (in `bevy_ui_widgets`) — floating-positioning popover inspired by the web `floating-ui` package.
- **`MenuPopup`** — dropdown menu built on `Popover` with keyboard navigation.
- Improved `RadioButton` / `RadioGroup` event propagation and keyboard activation.
- Variable-weight font support, text strikethrough/underline, OpenType font features (all `bevy_text` improvements that feathers inherits).
- `AutoDirectionalNavigation` for arrow-keys / gamepad (in `bevy_ui` but feathers consumes).

The Bevy 0.17 release notes had stated feathers would migrate to BSN in 0.18. **It did not** — BSN (PR [#20158](https://github.com/bevyengine/bevy/pull/20158)) remains in draft as of 2026-05-22. See [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § "BSN has not landed." Feathers consequently still ships with the pre-BSN required-components authoring style.

## Bevy 0.19-rc.1 (2026-05-13) and rc.2 (2026-05-22)

Comparing `v0.18.1` to `main` (which is `0.19.0-dev`), the controls/ directory grew from 9 widgets to **14**:

| Added in 0.19-dev | Notes |
|---|---|
| `disclosure_toggle.rs` | APG `disclosure` pattern. |
| `menu.rs` | Companion to bevy_ui_widgets' menu primitive. |
| `number_input.rs` | Numeric stepper / spinbutton. Issue [#24356](https://github.com/bevyengine/bevy/issues/24356) flags an `Update`-schedule rendering bug. |
| `text_input.rs` | Text-entry widget. Was the "still being developed" widget in 0.17 release notes. |

Containers and display modules also expanded — `containers/` now holds `flex_spacer.rs`, `group.rs`, `pane.rs`, `subpane.rs`; `display/` adds `icon.rs` and `label.rs`. The example set was split from a single `examples/ui/feathers.rs` (0.18.1) into `feathers_counter.rs` + `feathers_gallery.rs` on `main`.

AccessKit dependency bumped: **0.21 → 0.24**. This is a major-version-API change for downstream consumers of feathers (and Buiy — see [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Avoid row "AccessKit version pin drift").

The most visible accessibility delta in this cycle: **PR [#24308](https://github.com/bevyengine/bevy/pull/24308)** by **@viridia** (with co-author Richard Braakman, merged 2026-05-21) introduces the new **`AccessibleLabel`** component, closing issues #17644 and #20524. This is the post-mortem fix for the `bevy_a11y::AccessibilityNode` megacomponent problem and is the canonical BSN-unfriendliness incident — see [`critiques.md`](critiques.md) and [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md). Buiy's "decomposed-from-day-one" stance ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the alternative; PR #24308 is the bevy_ui-side after-the-fact fix.

## Key contributors over time

Reconstructed from PR / commit activity:

- **@viridia** — initiator (discussion #16900), primary author of feathers PR #19730, primary author of issue #17644 + PR #24308. The de-facto feathers lead.
- **@ickshonpe** — co-author on PR #19730, high-volume widget-PR submitter through 0.17 / 0.18 cycles.
- **@Atlas16A** — co-author on PR #19730, theme / token work.
- **@amedoeyes** — co-author on PR #19730, virtual-keyboard work.
- **@alice-i-cecile** — co-author on `bevy_ui_widgets`, merger of #19730, UI-area SME.

## Sources

- PR #19730 (feathers introduction) — `https://github.com/bevyengine/bevy/pull/19730`.
- PR #24308 (AccessibleLabel) — `https://github.com/bevyengine/bevy/pull/24308`.
- PR #20158 (BSN, still draft) — `https://github.com/bevyengine/bevy/pull/20158`.
- Discussion #16900 (Standard Headless Widgets) — `https://github.com/bevyengine/bevy/discussions/16900`.
- Discussion #11100 (10 Challenges for Bevy UI Frameworks) — `https://github.com/bevyengine/bevy/discussions/11100`.
- Issue #17644 (bevy_a11y BSN-unfriendly) — `https://github.com/bevyengine/bevy/issues/17644`.
- Issue #24356 (NumberInput Update-schedule bug) — `https://github.com/bevyengine/bevy/issues/24356`.
- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- Bevy 0.18 release notes — `https://bevy.org/news/bevy-0-18/`.
- `bevy_feathers` source v0.18.1 — `https://github.com/bevyengine/bevy/tree/v0.18.1/crates/bevy_feathers/src`.
- `bevy_feathers` source main — `https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src`.
- crates.io versions — `https://crates.io/api/v1/crates/bevy_feathers`.
