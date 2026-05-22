**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — how the bridge worked: plugin shape, Editor/Buffer split, render-to-texture pipeline, input routing.

# Architecture

bevy_cosmic_edit was a **bridge crate**: its job was to take cosmic-text's `Editor` + `Buffer` + `FontSystem` and surface them as Bevy components, then route Bevy's `winit` events into cosmic-text mutations and rasterize cosmic-text's glyph output back into Bevy textures. Everything in the crate served that bridging role.

Source structure at the final tag (0.26.0):

```
src/
├── lib.rs                        — plugin entry-point + prelude re-exports
├── cosmic_edit.rs                — component definitions (CosmicWrap, CosmicTextAlign,
│                                   CosmicBackgroundColor, CursorColor, SelectionColor,
│                                   ReadOnly, MaxLines, MaxChars, DefaultAttrs, …)
├── editor_buffer.rs              — EditorBuffer QueryData; CosmicEditor + CosmicEditBuffer
│                                   marker components live in this module's children
├── editor_buffer/                — buffer set_text / set_rich_text / get_text helpers
├── render.rs                     — glyph → image::RgbaImage → Bevy Image upload
├── render_implementations.rs     — Sprite vs UI rendering paths
├── render_implementations/       — per-target render specialization
├── input.rs                      — keyboard + mouse → cosmic-text Action
├── input/                        — submodules (clipboard, IME, key bindings)
├── focus.rs                      — FocusedWidget singleton + focus-on-click
├── double_click.rs               — double / triple-click word/line selection
├── font/                         — CosmicFontSystem resource + font registration helpers
├── password.rs                   — optional masked-input plugin
├── placeholder.rs                — optional placeholder-text plugin
├── user_select.rs                — selection-disable opt-in
├── primary.rs                    — primary-window glue
├── debug.rs                      — debug-overlay helpers
├── utils.rs                      — misc
└── snapshots/                    — insta test fixtures
```

The `CosmicEditPlugin` is the public entry point; the consumer added `app.add_plugins(CosmicEditPlugin::default())` and got the whole pipeline.

## The Editor / Buffer split — borrowed from cosmic-text

bevy_cosmic_edit mirrored cosmic-text's own `Editor` + `Buffer` separation (see [`cosmic-text/editing.md`](../cosmic-text/editing.md)). At the Bevy layer this surfaced as **two separate marker components** on the same entity:

- `CosmicEditBuffer` — wraps the cosmic-text `Buffer`. Holds text + layout + per-span attrs. Required for any text-bearing entity.
- `CosmicEditor` — wraps the cosmic-text `Editor`. Holds cursor + selection + in-progress `Change`. **Optional**: read-only entities had `CosmicEditBuffer` without `CosmicEditor`.

Consumers queried both via the `EditorBuffer` `QueryData` struct (defined in `editor_buffer.rs`):

```rust
#[derive(QueryData)]
#[query_data(mutable)]
pub struct EditorBuffer {
    editor: Option<&'static mut CosmicEditor>,
    buffer: &'static mut CosmicEditBuffer,
}
```

The `EditorBufferItem` (query result) implemented `Deref<Target = Buffer>` so consumers could treat it as the cosmic-text `Buffer` directly. Helper methods (`set_text`, `set_rich_text`, `with_buffer_mut`, `with_buffer`, `get_text`, `borrow_with`, `width`, `height`, `compute_everything`) re-exposed cosmic-text's surface with Bevy-friendly signatures.

This shape is **borrowable** for Buiy — see [`lessons.md`](lessons.md) — even though the crate itself is archived.

## Decomposed style components

Visual styling decomposed onto separate components, matching the bevy_ui post-decomposition idiom (see [`bevy-ui/component-model.md`](../bevy-ui/component-model.md)):

| Component | Purpose |
|---|---|
| `CursorColor` | Caret color (defaults to black). |
| `SelectionColor` | Selection-rectangle background. |
| `SelectedTextColor` | Foreground color for text inside selection. |
| `CosmicBackgroundColor` | Buffer background fill. |
| `CosmicBackgroundImage` | Optional image handle behind text. |
| `DefaultAttrs` | cosmic-text `AttrsOwned` — default font / weight / style / family. |
| `CosmicWrap` | `InfiniteLine` (no wrap) or `Wrap` (wrap at width). |
| `CosmicTextAlign` | Vertical + horizontal alignment (`VerticalAlign`, `HorizontalAlign`). |
| `MaxLines` | Hard cap on line count. |
| `MaxChars` | Hard cap on character count. |
| `ReadOnly` | Marker disabling all write paths. |
| `ScrollEnabled` | Bool flag for scroll-on-overflow. |

Note: `CosmicEditor` and `CosmicEditBuffer` were marker components in the foundational sense, but their *style siblings* were already decomposed at archive time — bevy_cosmic_edit absorbed the bevy_ui post-#17644 lesson before it landed in bevy_ui itself. See [`bevy-ui/critiques.md`](../bevy-ui/critiques.md) § "The megacomponent problem."

## Render pipeline — render-to-texture, not glyph atlas

This is the most consequential architectural choice and the one that increasingly diverged from bevy_text. bevy_cosmic_edit **did not integrate with bevy_text's glyph atlas**. It rasterized cosmic-text glyphs into a CPU `image::RgbaImage`, uploaded that image as a Bevy `Image` asset every frame the buffer dirty-flagged, and displayed the image via either a `Sprite` (2D world) or a `ui::ImageNode` (bevy_ui).

```
cosmic-text:                    bevy_cosmic_edit:                Bevy:
Buffer ────► layout_runs() ──► render.rs ────► image::RgbaImage ──► Image asset ──► Sprite / ImageNode
            (LayoutGlyph)      SwashCache draws         (CPU pixels)  (uploaded to GPU)
                               into pixel buffer
```

Consequences of this choice (which dominate the [`critiques.md`](critiques.md) and [`why-archived.md`](why-archived.md) files):

- **One texture per text input.** Memory grew per-widget, not per-glyph. A long document = a large texture.
- **No subpixel positioning across editors.** Each editor's texture was self-contained; cosmic-text's `SwashCache` lived per editor.
- **Re-rasterize on dirty.** Every keystroke that mutated visible glyphs invalidated and re-uploaded the entire visible-area texture. The dirty-flag granularity (whole-buffer vs visible-area) tightened across the 0.20-series releases but never reached glyph-level.
- **Did not share a `FontSystem` with bevy_text.** Both crates held their own `CosmicFontSystem` resource. Loading the same font twice was common.
- **Could not coexist cleanly with bevy_text's glyph atlas inside the same window.** A bevy_text label next to a bevy_cosmic_edit input rendered through two separate pipelines; clipping, draw order, and material layering had to be reconciled by the consumer.

`render_implementations.rs` + `render_implementations/` split the rasterization target between `Sprite` and `ImageNode` so the same `EditorBuffer` could drive either; example code in `examples/basic_sprite.rs` and `examples/basic_ui.rs` showed both paths.

## IME handling — passed through to winit

IME composition was **not** handled inside the crate. bevy_cosmic_edit relied on `bevy::winit::WinitWindows::set_ime_allowed` to enable the IME on focus, and forwarded `winit::event::Ime::*` events as best it could into cosmic-text `Action::Insert` calls on commit. Preedit (the in-progress composition string) **was not rendered**; the user saw nothing on screen during composition, then the committed text appeared on `Ime::Commit`.

This was a known gap (no tracking issue but discussed in informal user reports) and is the cleanest example of the bridge-layer limit: the crate could neither extend cosmic-text (upstream's `Editor` had no preedit hook — see [`cosmic-text/editing.md` § IME](../cosmic-text/editing.md#ime-composition-the-embedder-boundary)) nor maintain a parallel preedit span itself (the `CosmicEditBuffer` was the only text-bearing state). The "right" implementation requires the embedder to own a preedit overlay; bevy_cosmic_edit did not.

Buiy's text.md ([§ IME composition](../../specs/2026-05-07-buiy-foundation/text.md#34-typography)) requires preedit rendering, preedit cursor positioning, composition as undo unit, and composition popup positioning — all of which were absent from bevy_cosmic_edit. See [`lessons.md`](lessons.md) "Borrow" for the partial pattern + "Avoid" for the gap.

## Cursor + selection rendering

The crate painted the caret and selection rectangles **into the same CPU image** as the glyphs, before upload. Caret: a 1-or-2-pixel-wide rectangle stamped at the cursor position derived from `Editor::cursor()`. Selection: cosmic-text's `Editor::with_selection_bounds(|rects|)` returned a `Vec<Rect>` per visual run (multi-rect on mixed-direction lines per UAX #9); bevy_cosmic_edit stamped each rect into the image as a filled rectangle in `SelectionColor`, then painted text on top so glyphs inside selection were re-rendered in `SelectedTextColor`.

This means selection and caret were **not** separate render passes. They couldn't blend or composite differently from the text; they couldn't use shader effects; the caret didn't blink (re-rendering on a blink cadence would have been a re-upload-per-blink cost).

## Focus model

`FocusedWidget` was a `Resource` holding `Option<Entity>` — the single currently-focused editor. `focus_on_click()` was a system helper consumers could add to grant focus on `Pointer<Down>`. There was no integration with `bevy_input_focus` (Bevy 0.16+, post-archive); the focus tree was a singleton, not a stack.

## Sources

- `src/` directory listing — https://github.com/Dimchikkk/bevy_cosmic_edit/tree/main/src
- `src/cosmic_edit.rs` — component definitions
- `src/editor_buffer.rs` — `EditorBuffer` QueryData
- Repo archive notice (2025-03-21) — https://github.com/Dimchikkk/bevy_cosmic_edit
- cosmic-text Editor/Buffer split — [`../cosmic-text/architecture.md`](../cosmic-text/architecture.md), [`../cosmic-text/editing.md`](../cosmic-text/editing.md)
- bevy_ui decomposition trend — [`../bevy-ui/component-model.md`](../bevy-ui/component-model.md)
- bevy_ui text-rendering timeline — [`../bevy-ui/text-and-input.md`](../bevy-ui/text-and-input.md)
