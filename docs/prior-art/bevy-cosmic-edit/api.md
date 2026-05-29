**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — public component API, prelude surface, event flow, spawn-a-text-input idiom.

# API

The public API at 0.26.0 — preserved here for shape-comparison purposes only. Do not write Buiy code against this crate.

## Plugin

`CosmicEditPlugin` is the entry point. Adding it registers:

- All component types (see [`architecture.md`](architecture.md)).
- The `CosmicFontSystem` resource (singleton wrapping cosmic-text's `FontSystem`).
- The `FocusedWidget` resource (singleton `Option<Entity>`).
- Input-routing systems (keyboard, mouse, clipboard).
- Render-to-texture systems (one per render target: `Sprite`, `ImageNode`).
- Double-click detection.

```rust
use bevy::prelude::*;
use bevy_cosmic_edit::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(CosmicEditPlugin::default())
        .add_systems(Startup, setup)
        .run();
}
```

Optional sub-plugins (gated behind no feature flags — purely opt-in by adding):

- `placeholder::PlaceholderPlugin` — placeholder-text-when-empty rendering (was added in 0.18 per CHANGELOG).
- `password::PasswordPlugin` — mask-text-with-glyph rendering.
- `user_select::UserSelectPlugin` — disable selection on opted-out entities.

## Components

See [`architecture.md` § Decomposed style components](architecture.md#decomposed-style-components) for the full table. The required-pair to spawn a text input was:

- `CosmicEditBuffer` (text content + layout)
- `CosmicEditor` (cursor + selection — omit for read-only)
- At least one render-target component: `Sprite` (with an image handle) or `ImageNode`.

Optional siblings: `CursorColor`, `SelectionColor`, `SelectedTextColor`, `CosmicBackgroundColor`, `CosmicBackgroundImage`, `DefaultAttrs`, `CosmicWrap`, `CosmicTextAlign`, `MaxLines`, `MaxChars`, `ReadOnly`, `ScrollEnabled`.

## Prelude

The `prelude` re-exported the high-traffic types:

- `CosmicEditPlugin`, `CosmicEditor`, `CosmicEditBuffer`, `CosmicFontSystem`, `CosmicFontConfig`.
- `TextEdit`, `TextEdit2d` (render-implementation traits / markers).
- `EditorBuffer` (the `QueryData` for in-system access).
- `FocusedWidget`.
- Styling: `FontStyle`, `FontWeight`, `CosmicColor` (re-exports of cosmic-text types).
- Helpers: `HoverCursor`, `focus_on_click`, `deselect_editor_on_esc`, `print_editor_text`.

## Spawn-a-text-input idiom

Sourced from `examples/basic_ui.rs` at the final tag (paraphrased to fit, real example is longer):

```rust
fn setup(mut commands: Commands, mut font_system: ResMut<CosmicFontSystem>) {
    let attrs = AttrsOwned::new(Attrs::new()
        .family(Family::SansSerif)
        .color(CosmicColor::rgb(0, 0, 0)));

    commands.spawn((
        // Render target — ImageNode for bevy_ui placement
        ImageNode::default(),
        Node {
            width: Val::Px(320.0),
            height: Val::Px(48.0),
            ..default()
        },

        // The editing pair
        CosmicEditBuffer::new(&mut font_system.0, Metrics::new(20.0, 24.0)),
        CosmicEditor::default(),

        // Style decomposition
        DefaultAttrs(attrs),
        CursorColor(Color::BLACK.into()),
        SelectionColor(Color::srgba(0.3, 0.5, 0.9, 0.4).into()),
        CosmicWrap::Wrap,
        MaxLines::default(),
    ));
}
```

For a 2D sprite editor, swap `ImageNode` + `Node` for a `Sprite` with a handle to an empty `Image`. The render-implementation system populated the image in place.

## Events

bevy_cosmic_edit did **not** publish a rich event API. Consumers polled `EditorBuffer` queries each frame and read the text via `editor_buffer.get_text()`. The closest thing to an event was:

- `CosmicTextChanged(Entity, String)` — a Bevy event fired when the buffer's text differed from the previous frame.
- Bevy's standard `Pointer<Click>`, `Pointer<DragStart>`, etc. via the `bevy_picking` integration (added in PR #167, 2024-12-12).

There was no `CursorMoved`, `SelectionChanged`, `CompositionStart`, `CompositionUpdate`, `CompositionEnd`. The consumer reconstructed those from frame-to-frame query diffs. This is a real gap that any Buiy text-edit surface must close — see [`critiques.md`](critiques.md) "Event API thinness."

## Customization knobs

- **Cargo features:** only `internal-debugging` (reserved for project maintainers; it enables `bevy/track_change_detection`). There was no `headless`, `no_clipboard`, `no_wasm` feature gating. See [`integration.md`](integration.md).
- **Per-widget font:** via `DefaultAttrs` with a chosen `Family`. The `font_per_widget.rs` example demonstrated this.
- **Per-widget colors:** via the four color components (`CursorColor`, `SelectionColor`, `SelectedTextColor`, `CosmicBackgroundColor`).
- **Read-only mode:** add `ReadOnly` component. Skips all write paths in the input system.
- **Scroll:** add `ScrollEnabled(true)`. The buffer scrolled vertically when the cursor moved past the visible area; there was no horizontal-scroll for `CosmicWrap::InfiniteLine` (a known gap in 0.26.0).
- **Limits:** `MaxLines(usize)`, `MaxChars(usize)`. Enforced in the input system before forwarding to cosmic-text.

## What's missing from the API surface

For comparison with Buiy's text-edit spec ([text.md § 3.5](../../specs/2026-05-07-buiy-foundation/text.md#35-text-editing)), the gaps are:

- No `inputmode`-equivalent.
- No `enterkeyhint`-equivalent.
- No virtual-keyboard hint.
- No autocorrect / autocapitalize.
- No spellcheck integration.
- No undo/redo (removed in 0.17 per CHANGELOG, never restored — see [`history.md`](history.md)).
- No IME composition events; preedit was unrendered.
- No multi-cursor.
- No find-replace UI.
- No rich-text styled-span editing (`set_rich_text` set attrs but didn't expose a "toggle bold for selection" API).

## Sources

- `src/lib.rs` prelude — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/src/lib.rs
- `src/cosmic_edit.rs` components — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/src/cosmic_edit.rs
- `src/editor_buffer.rs` — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/src/editor_buffer.rs
- `examples/` directory listing — https://github.com/Dimchikkk/bevy_cosmic_edit/tree/main/examples
- PR #167 (bevy::picking integration) — https://github.com/Dimchikkk/bevy_cosmic_edit/pull/167
- Buiy text.md — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
