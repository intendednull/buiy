**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — Parley + Swash + Fontique text stack, editor / syntax-editor examples, comparison with the cosmic-text path Buiy chose

## The Floem text stack: confirmed Linebender

From `Cargo.toml` on `main` (verified 2026-05):

- `parley = "0.7.0"` — text shaping + paragraph layout (Linebender)
- `swash = "0.2"` — font rasterization, COLR/CPAL color emoji (Chad Brokaw / Linebender)
- `fontique = "0.7.0"` — font enumeration and fallback (Linebender)
- `peniko = "0.6.0"` — graphics primitives that Parley's outputs flow through

Floem does **not** depend on `cosmic-text`. The pre-amble's "Floem uses Parley, NOT cosmic-text" is confirmed.

This is the same stack Xilem uses, and it's the stack Bevy 0.19-dev is migrating *toward* (Bevy issue [#21765](https://github.com/bevyengine/bevy/issues/21765), 2025-11-06). It is **not** the stack Buiy chose; Buiy stays on cosmic-text. See [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) for the rationale.

## How Floem uses Parley

The data flow inside Floem:

1. A `label` or `text_input` view holds a `String` (or a closure returning one).
2. On state change, Parley builds a `Layout<Brush>` from the string + style attributes (font family, size, weight, etc.).
3. Layout cluster + line iteration produce glyph runs.
4. Swash rasterizes glyphs into the active renderer's atlas (vger / vello / skia / tiny-skia).
5. The view paints rectangle quads textured from the atlas.

Selection rectangles, caret positions, and BiDi cluster boundaries come from Parley's `Layout::cursor_for_offset` and `Layout::cluster_for_byte` style APIs.

## What Parley gives Floem

- **BiDi** via Parley's internal pipeline (Parley wraps `unicode-bidi`).
- **Per-run shaping** via swash (Swash includes its own shaper; Parley drives it). Note: this differs from cosmic-text's harfrust (HarfBuzz port) shaper.
- **Vector path output** (`peniko::Path` runs) as an alternative to rasterized glyph atlases. Vello's compute renderer can consume paths directly.
- **Mature production usage** in Lapce's editor, which uses Floem's text editor (`examples/editor`, `examples/syntax-editor`).

## Editor / syntax-editor examples

The Floem repo's `examples/` folder includes:

- `editor` — single-buffer text editor demo.
- `syntax-editor` — editor with syntax highlighting.

These are not toy demos; they are the substrate Lapce builds on. Lapce's full editor surface (multi-cursor, vim mode, LSP integration, syntax highlighting, file tree, panels) is built in Floem on top of the same primitives the examples demonstrate.

This is a substantive validation of the Floem text stack: a production code editor with the typical editor feature set ships on Parley + Swash + Fontique + Floem.

## What's missing in Floem's text surface (no special-casing — same as Parley's gaps)

Parley itself doesn't ship every text feature. Gaps Floem inherits:

- **Vertical writing modes** — Parley does not implement CSS `writing-mode: vertical-*`. Same gap as cosmic-text (see [`../cosmic-text/critiques.md`](../cosmic-text/critiques.md)). Both pure-Rust stacks have this gap.
- **Hyphenation** — not in Parley. Same gap as cosmic-text.
- **COLRv1 color fonts** — Swash doesn't yet support COLRv1; both Parley and cosmic-text inherit this gap.
- **OS-level text features** — Parley does not bridge to platform shaping (Core Text on macOS, DirectWrite on Windows). Cosmic-text doesn't either.

The net for Buiy: the Parley vs cosmic-text choice does **not** turn on these gaps. Both stacks have them.

## What Parley does *differently* from cosmic-text

Substantive design deltas (not just gaps):

| Dimension | Parley | cosmic-text |
|---|---|---|
| Shaper | Swash's built-in shaper | harfrust (HarfBuzz port; was rustybuzz pre-0.15.0) |
| Output | Vector paths (peniko) OR atlasable glyph runs | Atlasable glyph runs (`SwashImage`) |
| Font discovery | fontique (Linebender) | fontdb (Servo/RazrFalcon) |
| Editor primitives | `Layout` + cursor APIs; embedder builds editor | `Editor` + `Buffer` + `Action` enum + `Change` |
| Stewardship | Linebender (broad coalition) | System76 (single primary maintainer) |
| Vello-aligned | Yes — vector path output composes with Vello | No — Vello consumers must re-rasterize |

A Buiy designer asking "which stack should I learn first?" should learn the one that matches **the path their renderer takes**. Buiy paints rasterized atlases, so cosmic-text composes more cleanly. Vello-rendered Buiy would compose more cleanly with Parley. The architectural commitment in Buiy foundation `architecture.md` §2.3 is rasterized atlas, which locks in cosmic-text.

## The divergence question Buiy faces

If Bevy migrates `bevy_text` to Parley + Swash (issue #21765), and Buiy stays on cosmic-text, Buiy will diverge from upstream Bevy on text substrate. The cosmic-text `lessons.md` already commits to this:

> Buiy commits to cosmic-text **independently** of bevy_ui's text-stack decisions. The parallel-to-bevy_ui foundation stance is unchanged by this divergence; Buiy is parallel on text substrate too.

Floem on Parley is **another data point** in the same divergence. Two Rust UI projects (Bevy 0.19-dev and Floem) are now Parley-aligned. Buiy + the broader cosmic-text ecosystem (Iced, COSMIC desktop) are not. Both ecosystems will exist; the divergence is not catastrophic; but the migration cost for any Buiy user who wants to share text infrastructure with a Bevy-text or Floem app is real and growing.

For Buiy's roadmap: this is **not** a reason to revisit the cosmic-text choice. It is a reason to (a) make Buiy's text abstraction thin enough that a future Parley swap is conceivable, (b) document the trade explicitly (see [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md)), and (c) watch the swash COLRv1 work, since both stacks benefit equally from it.

## Sources

- Floem Cargo.toml workspace deps — https://github.com/lapce/floem/blob/main/Cargo.toml
- Parley — https://github.com/linebender/parley
- Swash — https://github.com/linebender/swash
- Fontique — https://github.com/linebender/fontique
- Bevy text migration issue #21765 — https://github.com/bevyengine/bevy/issues/21765
- Floem editor example — https://github.com/lapce/floem/tree/main/examples/editor
- Floem syntax-editor example — https://github.com/lapce/floem/tree/main/examples/syntax-editor
- Cross-link: [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md)
