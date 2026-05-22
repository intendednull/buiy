**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — text rendering stack timeline (ab_glyph → cosmic-text → parley/swash), text-edit status, picking integration, focus model, gamepad/spatial nav

## Text rendering: a three-engine timeline

bevy_ui has changed text shapers twice in three releases. Stable today (0.18.1) is cosmic-text 0.16; `main` (0.19-dev) is parley 0.9.0 + swash 0.2.6.

| Release | Window | Stack | Notes |
|---|---|---|---|
| ≤ 0.14 | ≤ 2024-07 | `ab_glyph` | No complex script shaping. No BiDi. No font fallback. |
| 0.15 | 2024-12 | `cosmic-text` (initial adoption) | [PR #10193](https://github.com/bevyengine/bevy/pull/10193) merged 2024-07-04, shipped in 0.15. Adds shaping, BiDi, font fallback, system-font discovery. Breaking changes: `Text2dBounds`→`TextBounds`, font sizes rescale ~1.2× smaller, `subpixel_alignment` flag removed, `CosmicBuffer` component required. |
| 0.16 | 2025-04 | cosmic-text (minor) | Internal refinements. |
| 0.17 | 2025-08 | cosmic-text (minor) | `TextShadow`, `TextBackgroundColor`, `Text2dShadow` shipped. |
| 0.18 | 2025-12 | cosmic-text 0.16 ([PR #22308](https://github.com/bevyengine/bevy/pull/22308), merged 2026-01-01) | Strikethrough/Underline components, font weights via `weight: FontWeight` on `TextFont`, OpenType feature flags, `Strikethrough`/`Underline` components, `FontHinting` component. Text-section-level picking lands. `LineHeight` split out of `TextFont` into its own component. `TextLayoutInfo.section_rects`→`run_geometry`. |
| 0.19-dev (HEAD) | 2026-02+ | **parley 0.9.0 + swash 0.2.6** | Migration tracked in [issue #21765](https://github.com/bevyengine/bevy/issues/21765) (opened 2025-11-06, "`Ready-For-Implementation`" / "`Blessed`"). The 0.18.1 `bevy_text` Cargo.toml shows cosmic-text 0.16; HEAD shows parley + swash. |

**Important brief correction:** the brief asked about cosmic-text adoption "0.13 / 0.14"; the correct version is **Bevy 0.15** (cosmic-text PR merged July 2024, shipped December 2024). Pre-0.15 used `ab_glyph`.

The cosmic-text → parley migration is unusually large for an active stable stack. Both engines share much of the substrate (swash for rasterisation, fontique-style font enumeration, harfbuzz/skrifa lineage), and the choice came down to "parley's shaping/layout performance is reported significantly better, and linebender contributors have worked on Bevy before" — see [issue #21765](https://github.com/bevyengine/bevy/issues/21765) comments. The Buiy foundation spec commits to cosmic-text ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)), so Buiy and post-0.19 bevy_ui will diverge on text shaper.

## What `Text` looks like today

The `Text` component is a `String` newtype since Bevy 0.15 ([Bevy 0.15 notes](https://bevy.org/news/bevy-0-15/)): "`Text (the UI text component) and Text2d (the world-space 2D text component) became literally just a String newtype`."

Rich text is the hierarchical pattern: a parent `Text` entity with `TextSpan` children, each child holding its own `TextFont`, `TextColor`, `LineHeight`, etc. Per-span pickability shipped in 0.18 — "`individual text sections belonging to UI text nodes are now pickable, allowing them to be selected, and can be given observers to respond to user interaction`" ([Bevy 0.18 notes](https://bevy.org/news/bevy-0-18/)). Hyperlink-like and keyword-tooltip patterns become straightforward.

Styling state, as of 0.18 / 0.19-rc.1:

- `TextFont` — family + size + weight + OpenType features + hinting (since 0.18).
- `TextColor` — color of glyphs.
- `LineHeight` — split out from `TextFont` in 0.18.
- `Strikethrough`, `Underline` — 0.18.
- `TextShadow`, `TextBackgroundColor` — 0.17.
- `FontHinting` — 0.18 (cosmic-text 0.16 enabled by `Text` for pixel-aligned text, disabled for `Text2d`).
- `TextLayoutInfo.run_geometry` — replaces `section_rects` in 0.18. Carries span index, bounding rect, underline/strikethrough position + thickness per text run.

## Text editing: what ships, what doesn't

Single-line and multi-line text input arrived as `bevy_ui_widgets::TextInput` and a layout helper module in `bevy_ui::widget::text_input_layout` ([source](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/widget/text_input_layout.rs)). The layout module:

- Defines `TextScroll(Vec2)` for editor viewport scroll position.
- `TextInputMeasure` sizes the input based on visible-lines × line-height and visible-width × character-advance.
- `update_editable_text_content_size`, `update_editable_text_styles`, `update_editable_text_layout`, `scroll_editable_text` are the per-frame editor systems.
- Supports `LineBreak::{AnyCharacter, WordOrCharacter, NoWrap, WordBoundary}` (multi-line is real, not faked).

What is **missing or partial** as of 0.18.1 / 0.19-rc.1:

- **IME composition** — no first-class IME composition surface in `bevy_ui`. Apps needing CJK/Korean input either roll their own via the platform integration or use the third-party `bevy_cosmic_edit`. The text_input_layout file has no IME wiring visible.
- **BiDi caret** — cosmic-text and parley both handle BiDi shaping, but a caret-aware BiDi text-edit (cursor crossing direction boundaries correctly, selection across mixed-direction runs) is not surfaced as a tested widget contract in bevy_ui_widgets. Multi-line BiDi edit remains an open implementation gap.
- **Undo/redo** — no built-in undo stack for text edits. Apps wire their own command history.
- **Rich-text edit** — only plain-text edit. The `Text`/`TextSpan` rich-text model is read-only for users; the editor surface is single-style.
- **Spellcheck** — no bridge to OS spellcheckers (NSSpellChecker on macOS, SpellCheckerFactory on Windows, etc.).
- **Clipboard integration for text fields** — basic copy/paste exists; selection-clipboard (X11/Wayland middle-click primary selection) is not standardised.
- **Composition over IME** — emoji picker / hardware keyboard handling for accented characters relies on the platform; bevy_ui does not implement a Compose-key fallback.

For richer text editing, the de-facto external crate is **`bevy_cosmic_edit`** ([Dimchikkk/bevy_cosmic_edit](https://github.com/Dimchikkk/bevy_cosmic_edit)) — a third-party plugin that wraps cosmic-text's editor with IME-aware widgets. It is not part of bevy_ui itself and a dedicated prior-art folder is in scope as a separate corpus.

## Input: bevy_picking integration

bevy_ui registers with [bevy_picking](https://docs.rs/bevy_picking) as a picking backend ([picking_backend.rs](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/picking_backend.rs)):

- `UiPickingPlugin` is added conditionally behind the `bevy_picking` feature.
- `UiPickingSettings` resource holds runtime config including `require_markers` (whether opt-in `UiPickingCamera` is required).
- `ui_picking` system runs in `PreUpdate` in the `PickingSystems::Backend` set.
- Hit-testing walks the UI stack top-down and respects rectangular clipping via the focus module's `clip_check_recursive` ([focus.rs](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/focus.rs)).

bevy_ui carries its own legacy `Interaction` enum (`Pressed` / `Hovered` / `None`) and `FocusPolicy` (`Block` / `Pass`) updated by `ui_focus_system` in `PreUpdate`. The duality (a bevy_ui-internal focus pass plus the bevy_picking integration) is a partial migration: pre-bevy_picking, bevy_ui owned its own hit-test; the picking backend is a newer layer on top, not a replacement of the internal one.

## Focus model

Two distinct concepts confused under the same word:

- **`Interaction`** (the `bevy_ui::focus` enum) — mouse/touch hit-test state, **not keyboard focus**.
- **Keyboard focus** — lives in the separate `bevy_input_focus` crate. An `InputFocus` resource holds the currently-focused entity. Tab navigation and 2D directional navigation strategies are exposed there ([Bevy 0.16 notes](https://bevy.org/news/bevy-0-16/)).

The focus model is split across crates:

- `bevy_input_focus` — `InputFocus` resource, `Focusable` semantic markers.
- `bevy_ui::focus` — mouse-hover/press state via `Interaction`. **No Tab-key navigation.** ([source](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/focus.rs) confirms: "`No Tab-key navigation logic is present in this file. It focuses exclusively on mouse/touch input handling, not keyboard navigation.`")
- `bevy_ui::auto_directional_navigation` (0.18+) — spatial directional nav. Adds `AutoDirectionalNavigation` marker; `DirectionalNavigationMap` for manual edge overrides; `FocusableArea` for navigable nodes; `CompassOctant`-based search ([Bevy 0.18 notes](https://bevy.org/news/bevy-0-18/), [auto_directional_navigation.rs](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/auto_directional_navigation.rs)).
- `bevy_ui_widgets` / `bevy_feathers` — widget-level focus contracts (button activation on Enter/Space, slider arrow-key handling, etc.). The APG keyboard contracts are widget-local, not a unified focus-tree system.

What is missing relative to a full focus story:

- **`:focus-visible`** semantics — no first-class "focus indicator only when input modality is keyboard/gamepad" rule. Apps add focus rings via the `focus.rs` module in `bevy_feathers`, but the keyboard-vs-mouse modality distinction is not formalised.
- **Focus trap** — no built-in focus-trap primitive for modals.
- **Focus restoration** — closing a popover does not automatically return focus to its trigger.
- **`inert` subtree** — no equivalent to HTML `inert` that disables focus + interaction recursively. Workaround: toggle `FocusPolicy` and `Visibility` manually.
- **Roving tabindex** — no first-class roving-tabindex helper, though composite widgets in `bevy_ui_widgets` (`RadioGroup`, etc.) implement it ad-hoc.
- **`aria-activedescendant`** equivalent — not surfaced (the AccessKit tree side does carry the concept, but no Bevy-side API exposes it consistently).

These are the gaps that Buiy's foundation spec calls out as `F` (foundation) tier ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)): focus tree, `:focus-visible` semantics, traps, restoration, inert subtrees, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point.

## Gamepad / spatial navigation

Spatial gamepad navigation in bevy_ui itself shipped in **0.18** via `AutoDirectionalNavigation` ([Bevy 0.18 notes](https://bevy.org/news/bevy-0-18/), source: [auto_directional_navigation.rs](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/auto_directional_navigation.rs)). The algorithm:

1. Manual edges in `DirectionalNavigationMap` take priority.
2. Fallback: compute the best candidate by screen-position spatial relationship, filtered by `CompassOctant` direction.
3. Visibility, screen-bounds, and target-camera filter entities.

This is a meaningful upgrade — earlier Bevy releases required apps to wire navigation graphs manually, or pull in the external `iyes_ui_navigation` crate. The 0.18 algorithm is comparable to the iyes_ui_navigation approach but ships with the engine.

What spatial nav 0.18 does not yet handle:

- Tab-order navigation (a sequence rather than a 2D field) is in `bevy_input_focus`, not unified with spatial nav.
- Cross-window / cross-camera transitions aren't handled by the spatial algorithm.
- Focus restoration / "remember which item was focused" semantics belong to the widget layer, not the navigator.

For the Buiy lens: the auto-spatial-nav primitive is borrowable directly; Buiy's foundation spec specifies "focus tree + spatial gamepad navigation" as a unified concept, which the bevy_ui split (3 crates) does not yet provide.

## Sources

- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/focus.rs
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/auto_directional_navigation.rs
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/picking_backend.rs
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/widget/text_input_layout.rs
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_text/Cargo.toml
- https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_text/Cargo.toml
- https://github.com/bevyengine/bevy/pull/10193
- https://github.com/bevyengine/bevy/pull/22308
- https://github.com/bevyengine/bevy/issues/7616
- https://github.com/bevyengine/bevy/issues/21765
- https://github.com/Dimchikkk/bevy_cosmic_edit
- https://bevy.org/news/bevy-0-15/
- https://bevy.org/news/bevy-0-16/
- https://bevy.org/news/bevy-0-17/
- https://bevy.org/news/bevy-0-18/
