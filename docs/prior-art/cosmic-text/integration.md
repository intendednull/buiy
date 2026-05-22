**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — how embedders integrate it (Iced, Bevy, COSMIC desktop, Buiy)

# Integration

cosmic-text is a **library**, not a runtime. It does not own the window, the GPU, the event loop, the clipboard, or the IME. Embedders provide all of that and call into cosmic-text for shaping, layout, hit-testing, edit-state mutation, and glyph rasterization. This file documents the integration shape that's emerged across the half-dozen production embedders, and where Buiy diverges.

Cross-links: [architecture.md](architecture.md) (types and their ownership), [editing.md](editing.md) (the `Editor` boundary), [shaping.md](shaping.md) (the shaper output the embedder consumes), [capabilities.md](capabilities.md).

## The canonical integration shape

Every embedder ends up with roughly the same five-piece structure:

1. **One `FontSystem` per process (effectively a singleton).** Owns the `fontdb::Database`, the locale, and the shape-result cache. Cloning is expensive; sharing is the rule. Implements neither `Send` nor `Sync` ergonomically in practice — embedders typically wrap it in `Arc<Mutex<FontSystem>>` or pin it to the UI thread.
2. **One `SwashCache` per process (effectively a singleton).** Owns the rasterized glyph image cache. Held alongside `FontSystem`. The embedder reads `SwashImage`s out of it and uploads them to its own GPU atlas.
3. **One `Buffer` per text node.** Holds the source text, the attrs, the layout result, the cursor scroll position. Re-laid-out whenever text, attrs, or width change. The embedder is responsible for change detection — cosmic-text doesn't track "is this Buffer dirty" itself.
4. **One `Editor<'buffer>` per editable text node.** Wraps a mutable borrow of the Buffer and tracks `Cursor` + `Selection` + in-progress `Change`. Created on-demand in the input-handling path.
5. **A glyph atlas owned by the embedder.** cosmic-text gives you `SwashImage` (alpha or RGBA pixel buffer); you decide how to pack and upload. Most embedders use a growing rectangle-packer (`etagere` is common) on a single 2D texture array.

The Buiy spec ([text.md](../../specs/2026-05-07-buiy-foundation/text.md)) commits to owning steps 3, 4, and 5 end-to-end as part of Buiy's render pipeline. cosmic-text supplies steps 1, 2, and the shaping/layout that produces the `LayoutRun`s feeding step 5.

## How Iced integrates

Iced adopted cosmic-text in **0.10.0** (released 2023-07-28) — changelog entry verbatim: *"Text shaping, font fallback, and `iced_wgpu` overhaul. [#1697]"*. The integration lives in the `iced_graphics` crate (cosmic-text in tree) plus `glyphon` (`grovesNL/glyphon`, first published 2022-05-10) as the wgpu glyph-atlas renderer. Iced 0.13.0 (2024-09-18) bumped to cosmic-text 0.10; subsequent point releases tracked through 0.15 in Iced 0.14.0 (2025-12-07).

The Iced shape:
- `iced_graphics` holds the `FontSystem` + `SwashCache` singletons inside its renderer.
- Each `Text` widget owns a `Buffer` keyed by `(content, font, size, line_height, bounds)`.
- `glyphon` (or its fork `iced_glyphon`) owns the GPU atlas. It rasterizes via `SwashCache` and uploads on demand.
- Iced does not expose `Editor` directly; the `TextInput` widget builds its own cursor model that internally calls into cosmic-text's measurement APIs but uses Iced's own input event flow.

This last point matters: **Iced did not adopt `cosmic-text::Editor` for its text-input widget**. It uses cosmic-text for shape + layout + glyph raster but reimplements selection state at the Iced layer. Embedders evaluating cosmic-text should know that even Iced — the largest non-COSMIC consumer — partially routes around the editor module.

## How Bevy UI integrates

Bevy switched from `ab_glyph` (not glyph_brush as sometimes claimed; glyph_brush had been a transitive dep earlier) to cosmic-text in **PR #10193**, merged **2024-07-04** during the 0.14 development cycle, shipped in Bevy **0.15** (2024-11-29). The migration introduced:

- A required `CosmicBuffer` component holding the per-text-node `Buffer`.
- Font sizes recalibrated by approximately 1.2× (the cosmic-text metrics interpretation differs from `ab_glyph`'s).
- `Text2dBounds` replaced with `TextBounds` accepting `Option<f32>` (because cosmic-text's wrap width is `Option`, not `f32::INFINITY`).
- Removal of the old `TextSettings` struct + `subpixel_alignment` feature.

Bevy 0.15 also introduced system font support as the headline benefit — cosmic-text via fontdb iterates `/usr/share/fonts`, `~/Library/Fonts`, `%WINDIR%\Fonts`, etc.

The Bevy shape:
- A single `Res<CosmicFontSystem>` resource wraps `FontSystem` (which is non-Sync; the resource pins it to a thread).
- Each `Text` entity gets a `CosmicBuffer` component holding the per-text `Buffer`.
- Glyph rasterization happens in a render-world system that consumes `LayoutRun`s and writes into Bevy's existing 2D text atlas (not a cosmic-text-specific one).
- `bevy_cosmic_edit` (a separate crate by `Dimchikkk`, **archived 2025-03-21**) added an editing surface on top, but is no longer maintained.

Buiy specifically diverges from Bevy UI's atlas strategy: per the spec, Buiy owns the atlas end-to-end without leaking per-span fonts.

## How COSMIC desktop integrates (dogfood)

cosmic-text is built by System76 for the COSMIC desktop (`pop-os/cosmic`), and the COSMIC text editor (`pop-os/cosmic-edit`) is the largest in-repo consumer. The dogfooding pattern is what shaped the API:

- COSMIC apps use `cosmic` (the libcosmic crate), which uses `iced` (a COSMIC fork), which uses cosmic-text. Three levels of indirection.
- The COSMIC text editor uses `ViEditor` (the `vi`-feature variant) for its undo stack and syntax highlighting via `syntect`.
- `cosmic-files`, `cosmic-settings`, `cosmic-comp` (the compositor's title-bar text), `cosmic-osd`, and the panel/dock all consume cosmic-text indirectly through libcosmic.
- The Universal Declaration of Human Rights stress-test (~500 languages, 8 MB, 106,746 lines) ships in the repo as the canonical correctness fixture.

The COSMIC stack is where cosmic-text's BiDi-aware caret traversal, font fallback ordering, and the `ellipsize` API got hardened — features land in cosmic-text the same week they're needed by a COSMIC app.

## How Freya and Floem integrate (they don't)

**Freya** (`marc2332/freya`) uses **Skia** for text rendering via `freya-skia-safe`, not cosmic-text. The Skia text layout engine handles its own shaping (HarfBuzz inside Skia) and rendering. Freya is sometimes listed as a cosmic-text consumer; that's wrong.

**Floem** (`lapce/floem`) uses **Parley** (the Linebender text layout crate), not cosmic-text. Floem's Cargo.toml lists `parley = "0.7.0"` as the text dependency. Lapce-the-editor itself uses cosmic-text-via-glyphon, but Floem-the-toolkit does not.

This narrows the actual cosmic-text downstream list to: **COSMIC desktop, Iced, Bevy UI (post-0.14), bevy_cosmic_edit (archived), Zed/GPUI (historically; the active main-branch Cargo.toml shows no cosmic-text reference, suggesting a migration we couldn't fully verify), plus a long tail of small editors and graphics crates (femtovg, basalt, uiua)**.

## How Buiy plans to integrate

The Buiy spec ([text.md](../../specs/2026-05-07-buiy-foundation/text.md)) sub-spec `buiy-text-rendering-design` will own the integration in detail. The architectural commitments from the foundation spec:

- **Own the text pipeline end-to-end.** Buiy's render pipeline drives cosmic-text directly — no `iced` / `libcosmic` layer in between.
- **Own the atlas.** A single Buiy-managed glyph atlas, no per-text-node leakage, no per-span font handle leaking. This is a deliberate divergence from Bevy UI's 0.14+ approach.
- **No `bevy_text` re-export.** Buiy is parallel to bevy_ui, so the existing Bevy text plumbing is not consumed; Buiy talks to cosmic-text crate directly.
- **IME via Bevy's winit plumbing.** winit `Ime::*` events → Buiy's input layer → cosmic-text `Action::Insert` for committed text; preedit lives in a Buiy-managed parallel span (see [editing.md § IME composition](editing.md#ime-composition-the-embedder-boundary)).
- **Font registration via Buiy's asset pipeline** (`@font-face` analogue per the foundation spec), feeding into fontdb's runtime `load_font_source` API rather than fontdb's filesystem scan.

The Buiy sub-spec `buiy-asset-pipeline-design` will own the atlas-warmup strategy, asset GC, and per-context atlas sharing question.

## IME boundary in detail

The lifecycle of a CJK composition:

1. winit emits `Ime::Enabled` when the IME activates on a focused text input. Embedder records "IME is composing" state.
2. winit emits `Ime::Preedit(text, cursor_range)` whenever the composition text changes. The embedder stores `text` in a separate "preedit segment" parallel to the cosmic-text `Buffer` — *it does not mutate the Buffer*. The preedit is rendered as a decoration span (typically underlined) by the embedder's own draw code, positioned at the current `Cursor` from the `Editor`.
3. winit emits `Ime::Commit(committed_text)` when the user accepts the composition. The embedder clears its preedit segment and calls `Editor::action(font_system, Action::Insert(c))` for each char of `committed_text`, aggregating the resulting `Change`s into a single undo group.
4. winit emits `Ime::Disabled` when the IME deactivates.

Cursor positioning during preedit: cosmic-text's `Editor::cursor()` returns the position where the commit will land; the embedder uses that to set the winit IME composition window position via `Window::set_ime_cursor_area(position, size)`. Without that call, OS IME popups appear at screen-origin or at the window's last cursor position, which is the visible bug in many naive integrations.

## Atlas / glyph-cache strategy

cosmic-text owns `SwashCache`, which is a `HashMap<CacheKey, SwashImage>` keyed by glyph-id + size + font + subpixel-offset bucket. The embedder reads `SwashImage` (CPU-side pixel data) and uploads to its own GPU texture atlas.

The shared-vs-per-context question: most embedders use a single process-wide atlas (Iced, Bevy 0.14+, COSMIC). Per-window or per-render-target atlases create cross-atlas glyph duplication, which wastes VRAM but eliminates contention. The Buiy spec leaves the choice to the `buiy-render-pipeline-design` sub-spec; the foundation spec only commits to "no atlas leaks" (i.e. no glyph staying resident after its text node disappears for many frames).

## GPU rendering

cosmic-text is CPU-only. The shaping engine (HarfRust since 0.15, see [shaping.md](shaping.md)) and rasterizer (`swash`) both run on the CPU. The output of cosmic-text's render path is:

- **For layout queries:** `Buffer::layout_runs()` yields `LayoutRun { glyphs: Vec<LayoutGlyph>, ... }` with per-glyph `x`, `y`, `font_id`, `cache_key`. Logical-order metadata + visual-order coordinates.
- **For rasterization:** `SwashCache::get_image(font_system, cache_key)` returns `Option<&SwashImage>` — `Some(image)` with `Placement { left, top, width, height }` and `Content::Mask | Content::Color | Content::SubpixelMask`. The embedder uploads pixels and emits a quad in its own render pass.

Buiy's render pipeline (`buiy-render-pipeline-design` sub-spec, not yet written) will define the wgpu-side: glyph-quad batching, signed-distance-field upgrade path (if any — cosmic-text's swash output is bitmap-only), color-emoji compositing, and integration with the top-layer / clip / blend model the foundation spec commits to.

## Sources

- Iced changelog — https://github.com/iced-rs/iced/blob/master/CHANGELOG.md (entry for 0.10.0 / #1697)
- Bevy PR #10193 (cosmic-text migration) — https://github.com/bevyengine/bevy/pull/10193
- `bevy_cosmic_edit` (archived 2025-03-21) — https://github.com/StaffEngineer/bevy_cosmic_edit
- COSMIC text editor — https://github.com/pop-os/cosmic-edit
- libcosmic — https://github.com/pop-os/libcosmic
- glyphon — https://github.com/grovesNL/glyphon
- Floem Cargo.toml — https://github.com/lapce/floem/blob/main/Cargo.toml (uses Parley, not cosmic-text)
- Freya Cargo.toml — https://github.com/marc2332/freya/blob/main/Cargo.toml (uses freya-skia-safe)
- Buiy foundation spec text.md — `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/text.md`
- winit `Ime` event — https://docs.rs/winit/latest/winit/event/enum.Ime.html
