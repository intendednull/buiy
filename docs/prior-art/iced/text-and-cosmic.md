**Date:** 2026-05-22
**Status:** active
**Subject:** Iced — text shaping and rendering history; cosmic-text adoption; NO Parley migration as of 0.14

# Iced text and cosmic-text

Iced is the **largest non-Bevy production consumer of cosmic-text** as of 2026-05-22. This makes Iced's text-substrate trajectory load-bearing for Buiy's text commitment ([text.md](../../specs/2026-05-07-buiy-foundation/text.md)): if Iced moves off cosmic-text, the upstream maintenance pressure on cosmic-text drops significantly.

Sibling files: [`architecture.md`](architecture.md), [`widgets-and-styling.md`](widgets-and-styling.md). Cross-cuts heavily with [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) and [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Avoid → text-shaper migration.

## Brief correction

The orchestrator brief asserted Iced migrated to Parley + harfrust. **This is wrong as of Iced 0.14.0.** Iced 0.14 (released 2025-12-07) pins `cosmic-text = "0.15"` in its `Cargo.toml` ([source](https://github.com/iced-rs/iced/blob/0.14/Cargo.toml)). There is no `parley` dependency, no `swash` dependency (direct), no Iced commit mentioning a Parley migration. The 0.14 changelog tracks **cosmic-text version bumps** (`0.13 → 0.14 → 0.15`) and that is the substrate's full evolution this release.

The Parley narrative is **bevy-specific**: Bevy issue [#21765](https://github.com/bevyengine/bevy/issues/21765) (2025-11-06) tracks Bevy's `bevy_text` migration from cosmic-text to Parley + swash. That migration has not propagated to Iced.

Practical impact for Buiy: Iced + Buiy converge on cosmic-text; bevy_ui post-0.19 diverges. The convergence-on-Parley framing applies only to two-of-three (Bevy + Floem use Parley; Iced + Buiy do not).

## Iced's text-substrate timeline

| Iced version | Released | Text substrate |
|---|---|---|
| 0.1 – 0.11 | 2020-04 – 2023-09 | `glyph_brush` + `ab_glyph` (no BiDi, no fallback) |
| 0.12 | 2024-02-15 | First cosmic-text adoption (via PR #1697 "Text shaping, font fallback, and `iced_wgpu` overhaul" — landed in 0.12 development) |
| 0.13 | 2024-09-18 | cosmic-text `0.10` |
| 0.14 | 2025-12-07 | cosmic-text bumped through `0.13` → `0.14` → `0.15` during the 0.14 dev cycle |

(The 0.12 release was when complex-script shaping, BiDi, and font fallback became real; 0.11 and earlier could only render Latin / basic-script text and had no fallback chain.)

cosmic-text's own substrate evolved underneath Iced through the same period:

- cosmic-text `0.10` (2023-Q4 era) — `rustybuzz` shaper.
- cosmic-text `0.15.0` (2025-09-09, PR [#417](https://github.com/pop-os/cosmic-text/pull/417)) — switched shaper from `rustybuzz` to `harfrust` (HarfBuzz Rust port). Iced 0.14 inherits this transitively.

So Iced 0.14 is, *transitively*, on `harfrust` — but via cosmic-text, not directly. Iced does NOT have `harfrust` in its own `Cargo.toml`; the dependency is cosmic-text's.

## Why Iced has not migrated to Parley

There is no public design rationale in Iced for migrating to Parley. As of 2026-05-22:

- No open issue tracks the migration on `iced-rs/iced`.
- cosmic-text 0.15 (current Iced dep) is recent (2025-09) and active.
- COSMIC desktop (libcosmic, on top of Iced) depends on cosmic-text directly for its own purposes — switching Iced's text engine would force libcosmic to maintain dual stacks.

Iced's text needs are well-served by cosmic-text (BiDi, font fallback, color emoji via swash COLRv0+CBDT+sbix, complex-script shaping via harfrust). The features Parley adds over cosmic-text (vector-path glyph output for vello-style renderers, slightly different shaping API surface, Linebender-stewarded) don't map to Iced's renderer architecture (which uses raster atlases via swash). Iced's renderer is wgpu-quad-and-atlas; cosmic-text's `SwashImage` output is exactly what that renderer wants.

This is why the Parley story is bevy-specific: bevy_text shipped a *vello*-aligned rendering path (issue #21765 is in part about converging on Linebender's stack). Iced doesn't share that motivation.

**Implication for Buiy:** Iced is the strongest "cosmic-text will survive even if Bevy leaves" signal available. Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Validates → "System76 stewardship via COSMIC desktop dogfooding."

## Text rendering pipeline in Iced

Per-frame text handling:

1. `widget::Text` (or `RichText`, `Markdown`, etc.) builds a `cosmic_text::Buffer` from the text + attrs.
2. `iced_graphics` / `iced_wgpu` glyph cache holds `cosmic_text::SwashCache` keyed by `(font_id, glyph_id, font_size_bits, subpixel_x_bin, subpixel_y_bin, weight, flags)`.
3. On layout, the buffer's `BufferLine` and `LayoutLine` are computed (cached lazily in the buffer).
4. On render, the glyph cache rasterizes any missing glyphs into a CPU `SwashImage`; the renderer uploads them into a wgpu texture atlas.
5. Quads are emitted per glyph referencing the atlas region.

For the software renderer (`iced_tiny_skia`), the same `SwashImage` is blitted via `tiny-skia` into the backbuffer instead of into a texture atlas.

This is the standard cosmic-text embedding pattern, and it matches what Buiy commits to ([architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md) "cosmic-text → glyph atlas → render pass, owned end-to-end").

## IME support

**0.14 added IME support** ([release notes](https://github.com/iced-rs/iced/releases/tag/0.14.0): "Input method support with IME capabilities"). The implementation:

- winit IME events (`Preedit`, `Commit`, `Enabled`, `Disabled`) plumb into the widget tree.
- `TextInput::Id::focus()` triggers IME enable on the focused widget.
- `Commit` text inserts via the standard editor `Action::Insert` path.
- `Preedit` rendering: Iced renders the preedit as a decoration above the buffer text; the buffer is not mutated until commit.

This mirrors the IME-boundary pattern recommended in [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Borrow #9 ("The IME boundary contract"). Buiy's text-editing-design sub-spec can study Iced's implementation directly.

**Status note:** IME in Iced 0.14 is brand-new (~6 months in production at this writing). Real-world coverage of CJK / Indic IMEs is in early days — Halloy uses it; comprehensive IME test coverage in CI is not (yet) published.

## BiDi support

BiDi via cosmic-text's `unicode-bidi` integration — UAX #9 segmentation, per-run shaping via harfrust. Iced exposes:

- `text::Shaping::Auto` (added 0.14) — auto-detect script and pick shaping strategy per-run.
- `text::Shaping::Basic` — GSUB-skipping fast path (Latin-only, fast).
- `text::Shaping::Advanced` — full GSUB + GPOS + per-script shaping.

`Shaping::Advanced` is the recommended setting for any RTL or complex-script content. `Shaping::Basic` is a performance opt-out for pure-Latin text.

Per-paragraph direction override (RTL paragraph in an LTR document, equivalent of CSS `dir="rtl"`) requires inserting Unicode formatting characters (`U+2066`–`U+2069`) into the text — Iced does not expose a direct API. Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Avoid → "Assuming Buffer-level paragraph-direction override works per-line."

## Vertical writing modes

**Not supported.** cosmic-text does not implement vertical writing modes ([`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Avoid). Iced inherits the gap. `writing-mode: vertical-rl / vertical-lr` are not expressible in Iced. Apps that need vertical text (Japanese book layout, some traditional Chinese layouts) cannot use Iced.

## Color emoji

Via swash through cosmic-text: COLRv0/CPAL, CBDT/CBLC, sbix all work. **COLRv1 does not** (swash gap, Fedora 43+ Noto Color Emoji ships as COLRv1, will not render). Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Avoid → COLRv1.

## Text editing widgets

- `TextInput` — single-line. Receives `Action`-style commands (cursor motion, insert, delete). Cursor / selection state held in the widget-state `Tree`. Selection rectangles painted by Iced's renderer using `Editor::with_selection_bounds`-style geometry from cosmic-text.
- `TextEditor` — multi-line. Added in 0.12, mature in 0.13. Wraps cosmic-text's `Editor` (not `ViEditor`). No vi-mode, no syntax highlighting by default (apps can layer `iced_highlighter` via the `highlighter` feature, which wraps `syntect`).
- `Markdown` — rendering-only widget; not editable.

Editor extras Iced does NOT ship:

- No undo/redo stack (apps roll their own on top of cosmic-text's `Change` events — same shape Buiy would use, see [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Avoid → "Assuming Editor has undo/redo built in").
- No multi-cursor.
- No find-and-replace UI.
- No collaborative-editing primitives.

## Implications for Buiy

1. **Iced is the most important non-Bevy cosmic-text consumer; its continued use is load-bearing for cosmic-text's long-term health.** Iced's ~1.9M downloads + COSMIC desktop dogfooding give cosmic-text a non-Bevy survival path. Buiy's cosmic-text bet ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) is meaningfully de-risked by Iced. Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Validates.
2. **The Parley convergence narrative is Bevy-side only.** Don't assume Iced + Bevy will both end up on Parley; assume Iced stays on cosmic-text for the foreseeable future. Buiy's parallel-to-bevy text stance ([`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) Top of File #2) is sound on Iced grounds, not just Buiy-internal grounds.
3. **Iced's IME implementation is recent and worth studying for the Buiy IME contract.** Buiy's `buiy-text-editing-design` sub-spec should compare its IME state machine against Iced 0.14's implementation directly — both are embedders translating winit IME events to cosmic-text editor actions. Same shape, same gotchas (composition spans, undo grouping, cursor disambiguation at preedit boundaries).
4. **`Shaping::Auto` (0.14) is a useful pattern.** Auto-detecting shaping mode per run avoids the "always-Advanced is slow on pure-Latin paragraphs" problem. Buiy can adopt the same pattern in its text component — or commit to always-Advanced and benchmark to confirm the overhead is tolerable. Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Avoid → "Assuming `Shaping::Basic` is fine."
5. **Vertical writing modes are not in Iced and not in cosmic-text.** Buiy's commitment to vertical writing modes ([text.md § Bidirectional text, Tier E](../../specs/2026-05-07-buiy-foundation/text.md)) requires Buiy to layer a per-block transform above cosmic-text — neither Iced's experience nor cosmic-text upstream gives Buiy a model to copy.

## Sources

- Iced 0.14 Cargo.toml — https://github.com/iced-rs/iced/blob/0.14/Cargo.toml
- Iced 0.14.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.14.0
- Iced 0.13.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.13.0
- Iced 0.12.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.12.0
- PR #1697 (text shaping + font fallback + iced_wgpu overhaul) — https://github.com/iced-rs/iced/pull/1697
- cosmic-text repo — https://github.com/pop-os/cosmic-text
- cosmic-text PR #417 (rustybuzz → harfrust, 2025-09-09) — https://github.com/pop-os/cosmic-text/pull/417
- Bevy issue #21765 (bevy_text → Parley) — https://github.com/bevyengine/bevy/issues/21765
- Buiy foundation text — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- cosmic-text prior-art folder — [`../cosmic-text/`](../cosmic-text/)
- bevy-ui prior-art folder — [`../bevy-ui/`](../bevy-ui/)
