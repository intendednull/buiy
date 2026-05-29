**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — system-specific terms used in this folder.

# Glossary

- **bridge crate** — A third-party crate whose primary job is adapting one upstream project's API to another upstream project's component model, without owning either side. bevy_cosmic_edit was a bridge crate between `cosmic-text` and `bevy_ui`. The structural fragility of bridge crates is the central lesson in [`lessons.md`](lessons.md) and [`why-archived.md`](why-archived.md).
- **`CosmicEditPlugin`** — The Bevy `Plugin` entry point. The consumer added `.add_plugins(CosmicEditPlugin::default())` to their `App` to register components, resources, and systems.
- **`CosmicEditBuffer`** — Bevy component wrapping cosmic-text's `Buffer` (text + layout + per-span attrs). Required on any text-bearing entity.
- **`CosmicEditor`** — Bevy component wrapping cosmic-text's `Editor` (cursor + selection + in-progress `Change`). Optional; absence = read-only.
- **`EditorBuffer`** — Bevy `QueryData` struct combining `Option<&mut CosmicEditor>` + `&mut CosmicEditBuffer`. Returned items `Deref` to cosmic-text `Buffer`. The primary in-system access pattern.
- **`CosmicFontSystem`** — Bevy `Resource` wrapping cosmic-text's `FontSystem`. Singleton per app; not shared with bevy_text's own `FontSystem`.
- **`FocusedWidget`** — Bevy `Resource<Option<Entity>>`. The single currently-focused editor. Not a focus tree; not a stack. Replaced post-archive by `bevy_input_focus` (Bevy 0.16+) — see [`integration.md`](integration.md).
- **`DefaultAttrs`** — Bevy component wrapping cosmic-text `AttrsOwned`. Default per-buffer family / weight / style / color.
- **`CosmicWrap`** — Bevy component enum (`InfiniteLine | Wrap`). Maps to CSS `white-space: nowrap` vs default wrap.
- **`CosmicTextAlign`** — Bevy component holding `VerticalAlign` + `HorizontalAlign`. Maps to a subset of CSS `text-align` + `vertical-align`.
- **`ReadOnly`** — Marker component. Adding it short-circuits all write paths in the input systems.
- **`MaxLines` / `MaxChars`** — Bevy components capping line / character counts; enforced pre-`Action` in the input system.
- **`SwashCache`** — cosmic-text's glyph cache (`HashMap<CacheKey, SwashImage>`). bevy_cosmic_edit maintained one **per `CosmicEditBuffer`** (not shared across editors or with bevy_text's atlas).
- **render-to-texture** — The architecture choice: rasterize cosmic-text glyphs into a CPU `image::RgbaImage`, upload as a Bevy `Image` asset, display via `Sprite` or `ImageNode`. Contrast with **render-graph-integrated** (Buiy's choice — see [`lessons.md` Avoid](lessons.md#avoid)).
- **`Action`** — cosmic-text's input verb enum (`Motion`, `Escape`, `Insert(char)`, `Enter`, `Backspace`, `Delete`, ...). bevy_cosmic_edit's input system translated `winit::event::KeyboardInput` to `Action` calls on the `Editor`.
- **`Change`** — cosmic-text's edit-record type. Each `Action` that modifies text produces a `Change`. bevy_cosmic_edit *did not* accumulate `Change` values into an undo stack (the 0.17 release explicitly removed undo/redo — see [`history.md`](history.md)).
- **preedit** — The in-progress IME composition string between `Ime::Enabled` and `Ime::Commit`. bevy_cosmic_edit did **not** render preedit; the user saw nothing on screen during composition. See [`critiques.md` § IME](critiques.md#ime-boundary).
- **archive (GitHub)** — A repository state where the codebase becomes read-only: no PRs accepted, no issues openable, no commits possible. Triggered by the owner. Distinguish from "abandoned" (which is uncertain): an archived repo has an explicit owner decision. See [`why-archived.md`](why-archived.md).
- **0.26.0** — The final published version. Released 2024-12-07. Pinned to Bevy 0.15.
- **2025-03-21** — The archive date. ~3.5 months after 0.26.0; ~6 weeks after the last commit. Unannounced.
