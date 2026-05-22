**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — what plugging it in looked like; Cargo features; coexistence with bevy_ui's native Text and (post-archive) bevy_feathers.

# Integration

**Do not use this crate in new Buiy work.** This page exists so Buiy spec authors evaluating "should we revive bevy_cosmic_edit?" can see what an integration actually entailed and understand the surface area Buiy would inherit if it forked.

## Setup

The minimal integration was three lines:

```rust
use bevy::prelude::*;
use bevy_cosmic_edit::prelude::*;

App::new()
    .add_plugins(DefaultPlugins)
    .add_plugins(CosmicEditPlugin::default())
    .run();
```

`CosmicEditPlugin::default()` registered:

- All component types (`CosmicEditBuffer`, `CosmicEditor`, `CursorColor`, ...).
- The `CosmicFontSystem` resource (singleton).
- The `FocusedWidget` resource.
- Input systems (keyboard, mouse, clipboard, IME pass-through).
- Render-to-texture systems for both `Sprite` and `ImageNode` targets.
- Double-click and triple-click detection.

`CosmicEditPlugin` also accepted a `CosmicFontConfig` for font-loading customization, but no public field-level docs for it survive in the archived repository's docs.rs page (67.11% doc coverage at 0.26.0).

## Cargo features

There was effectively **one** feature:

- `internal-debugging` — enables `bevy/track_change_detection`. Reserved for project maintainers per the Cargo.toml comment.

There was no:

- `default-features = false` knob to drop clipboard (`arboard`) on non-clipboard targets.
- `wasm` opt-in / opt-out (WASM support was always compiled in via `cfg(target_arch = "wasm32")`).
- `headless` mode for testing.
- `no_ime` toggle.

This thin feature surface is a maintenance signal — the maintainer chose to ship one configuration rather than several, reducing test-matrix burden but eliminating opt-out paths consumers might have wanted.

## Dependency pin at 0.26.0

| Dep | Version | Notes |
|---|---|---|
| `bevy` | `^0.15` | Hard pin to Bevy 0.15. No 0.14 fallback path. |
| `cosmic_text` | (implicit) | Pulled in transitively via `bevy_text`. Cosmic-text 0.12.x at this Bevy version. |
| `unicode-segmentation` | `^1.11.0` | Grapheme-cluster boundaries for delete / word-nav. |
| `crossbeam-channel` | `^0.5.8` | Async clipboard fetches on WASM. |
| `image` | `^0.25.1` | The CPU pixel buffer the rasterizer wrote into. |
| `sys-locale` | `^0.3.0` | System locale lookup for cosmic-text default attrs. |
| `document-features` | `^0.2.8` | Doc-comment generation for the (single) feature. |
| `arboard` | `^3.2.0` | Native clipboard (non-WASM). |
| `js-sys` / `wasm-bindgen` / `web-sys` / `wasm-bindgen-futures` | (WASM only) | Browser clipboard, FocusEvent, IME plumbing. |
| `insta` | `^1.29.0` | Dev-only snapshot tests. |

The transitive cosmic-text version is the lurking detail: by riding `bevy_text`'s cosmic-text pull, bevy_cosmic_edit gave up the ability to upgrade cosmic-text independently. When cosmic-text 0.13/0.14 shipped breaking changes (e.g. the rustybuzz → harfrust swap in PR #417, cosmic-text 0.15.0, 2025-09-09 — see [`../cosmic-text/history.md`](../cosmic-text/history.md)), bevy_cosmic_edit had to wait for bevy_text to bump first.

## Coexistence with bevy_ui's `Text`

In the same window, the consumer could have:

- A `Text` node rendered by bevy_text (cosmic-text-via-bevy_text → bevy_text glyph atlas → GPU quads).
- A `CosmicEditBuffer` rendered by bevy_cosmic_edit (cosmic-text → CPU `RgbaImage` → Bevy Image asset → `ImageNode`).

Both pipelines worked, but they were architecturally disjoint:

- **Two separate `FontSystem` instances.** Loading "Roboto.ttf" once for each pipeline. Memory waste; no shared font cache.
- **Two separate glyph caches.** bevy_text had its `bevy_text::GlyphAtlas`; bevy_cosmic_edit had its per-buffer rasterizations.
- **Different update characteristics.** A label re-laid-out on font-size change retriggered bevy_text's atlas eviction; the same change in a bevy_cosmic_edit input re-uploaded the entire CPU image.
- **Layout coherence wasn't enforced.** A `Text` "Username:" label next to a `CosmicEditBuffer` input — they had to be sized + positioned manually; the editor's intrinsic-content-size came from the rasterized image, not from a Taffy `MeasureFunc`.

This is the visible cost of bridge-crate architecture: integration *with the rest of the host ecosystem* was incomplete by construction. Buiy's commitment to own text rendering and editing end-to-end ([`../../specs/2026-05-07-buiy-foundation/architecture.md` § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the design choice that makes coherence possible.

## Coexistence with `bevy_feathers` (post-archive)

`bevy_feathers` is Bevy's experimental first-party widget set, available from Bevy 0.17. It includes a text-input widget. Coexistence with bevy_cosmic_edit was never meaningfully tested because bevy_cosmic_edit was pinned to Bevy 0.15 and never updated to 0.16 or 0.17. **In practice, this combination does not exist.**

If a hypothetical fork updated bevy_cosmic_edit to Bevy 0.17, both crates would want to:

- Claim the `bevy_picking` text-area hover/click events on the same window.
- Manage focus via `bevy_input_focus` (0.16+).
- Register a `MeasureFunc` for their text nodes against Taffy.

Resolution would require explicit "which widget wins on focus competition" logic that neither crate ships. The cleaner answer for any new project is to pick one — and `bevy_feathers` is the one that's still maintained.

See [`../bevy-feathers/README.md`](../bevy-feathers/README.md) for the active alternative.

## Sources

- `Cargo.toml` at final tag — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/Cargo.toml
- README compat table — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/README.md
- cosmic-text release timeline — [`../cosmic-text/history.md`](../cosmic-text/history.md)
- bevy_feathers — [`../bevy-feathers/README.md`](../bevy-feathers/README.md)
- bevy_text → Parley migration — https://github.com/bevyengine/bevy/issues/21765
- Buiy parallel-stack rationale — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
