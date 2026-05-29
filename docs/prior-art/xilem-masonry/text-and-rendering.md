**Date:** 2026-05-22
**Status:** active
**Subject:** Parley + Skrifa + Fontique + Vello — Linebender's text & rendering stack; comparison to cosmic-text (Buiy's choice)

# Text & rendering: the Linebender side of the fork

This file is the focused comparison of **Linebender's text+render stack** (Parley + harfrust + Skrifa + Fontique + Vello) versus **Buiy's choice** (cosmic-text + harfrust + swash + own-wgpu-pipeline). The fork is significant because Bevy 0.19-dev migrated to Parley (issue [#21765](https://github.com/bevyengine/bevy/issues/21765)); from Bevy 0.19 onward, **Buiy is the only Rust UI choosing cosmic-text where the largest game engine in the ecosystem chose Parley**.

## The two stacks side by side

| Layer | Linebender (Xilem/Masonry) | Buiy |
|---|---|---|
| Font data | Skrifa / read-fonts | (cosmic-text's internal; fontdb) |
| Font enumeration & fallback | Fontique | (cosmic-text's internal; fontdb) |
| Shaping | harfrust (HarfBuzz Rust port) | harfrust (same library) — or swash, depending on cosmic-text branch |
| Layout (line break, BiDi, paragraphs) | Parley | cosmic-text (`Buffer`) |
| Rasterization | Vello (compute-shader-based) | Buiy's own wgpu pipeline, glyph atlas from swash/zeno |
| Composition | Vello scene | Buiy render-graph node |
| Editing primitives | (Parley exposes layout; editing is up to consumer) | cosmic-text `Buffer` (designed for editing) + buiy_text on top |

**Key non-divergence:** Both use **harfrust** for shaping. The shaper is the same. What differs is the layout API, the editing model, and the rasterization path.

## What Parley does well

- **Modern paragraph layout** with rich-text styling. The `RangedBuilder` API takes a string and a sequence of style ranges; Parley produces laid-out lines.
- **BiDi correct** (UAX #9) via integration with harfrust + ICU-derived data.
- **AccessKit-integrated** — Parley has an `accesskit` feature that emits text-run accessibility info usable in `Node::set_text` and related setters. This is a meaningful integration point Buiy's text-rendering subspec should study (see [`accessibility.md`](accessibility.md)).
- **Variable-font support** via Skrifa.
- **Color emoji** via harfrust + COLR/CPAL table support (Skrifa parses; Parley uses).

## What cosmic-text does that Parley doesn't (yet)

- **Editor-shaped API.** cosmic-text's `Buffer` is *designed* for text editing: cursor management, line wrapping, swappable backing strings, IME composition support, multi-cursor primitives. Parley's API is layout-first; editing is the consumer's problem.
- **Editor IME hooks.** cosmic-text exposes IME composition state directly. Parley requires the consuming framework to manage IME state and re-shape on each composition update.
- **Production use in editors.** cosmic-text is the text engine of COSMIC Desktop, Zed, Lapce. Production editor use battle-tests the editing path in ways Parley hasn't yet seen.

This is the core reason Buiy chose cosmic-text: the editing primitives matter, and Buiy's `buiy_text` (foundation [`architecture.md § 2.8`](../../specs/2026-05-07-buiy-foundation/architecture.md)) needs IME-correct text editing as a load-bearing feature.

## What Parley does that cosmic-text doesn't (yet)

- **Richer style-range model.** Parley's `RangedBuilder` takes typed property ranges (color, weight, italic, etc.); cosmic-text's per-span styling is more limited.
- **Better COLR-emoji and OpenType-features integration.** Parley exposes per-script features; cosmic-text's coverage has historically lagged.
- **First-class AccessKit integration.** Parley's `accesskit` feature is a published integration; cosmic-text leaves a11y to the consumer.

## The harfrust common substrate

Both stacks shape with harfrust. This means:

- Shaping bugs in one are likely shaping bugs in the other (file upstream against harfrust).
- The complex-script coverage (Indic, Arabic, Hebrew, CJK, Korean Hangul) is consistent between Parley and cosmic-text.
- Variable-font support is consistent.

This is the *substrate-convergence* observation from [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md) #4: the substrate set is converging across Rust UI stacks, so the substrate's maintenance benefits everyone regardless of which layout/editor API is consumed above it.

## The Bevy 0.19-dev migration

bevy_text migrated from cosmic-text 0.16 (Bevy 0.15 through 0.18) to **parley 0.9.0 + swash 0.2.6** in Bevy 0.19-dev (issue [#21765](https://github.com/bevyengine/bevy/issues/21765), opened 2025-11-06, labeled `Ready-For-Implementation` / `Blessed`). The rationale Bevy contributors gave:

1. cosmic-text's API is editor-shaped; Bevy's text is mostly *display* text, where the editor APIs are overkill.
2. Parley has tighter AccessKit integration.
3. Parley + swash (instead of cosmic-text's preferred raster path) gives more control over glyph rendering on the Bevy side.

This decision affects Bevy directly; Buiy is parallel-to-bevy_ui and **deliberately stays on cosmic-text** (foundation [`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md)). The implication: post-Bevy-0.19, Buiy is the only major Bevy-ecosystem UI library on cosmic-text. cosmic-text's other adopters (COSMIC, Zed, Lapce, Iced) remain, so the upstream substrate isn't going away — but it's a notable ecosystem signal.

## Vello vs custom render passes

Linebender renders text by emitting glyph quads (or path-fills for vector fonts) into a `vello::Scene`. Vello rasterizes via compute shaders.

Buiy's foundation commits to a **custom wgpu render pipeline** integrated into Bevy's render graph. The custom pipeline must:

- Rasterize glyphs into atlas textures (likely via swash, as bevy_text does pre-0.19, or via cosmic-text's preferred raster path).
- Manage atlas pages (LRU, eviction, scale-tier).
- Composite atlas-sampled quads into the UI layer.

This is *not Vello*, but the *capability set Vello demonstrates* (anti-aliased path fill, gradient interpolation in arbitrary color spaces, blur, blend, clip-path arbitrary shape) is the Buiy render-pipeline target. Vello is the **feasibility witness** that these capabilities can ship in pure-wgpu, not a dependency.

## What Buiy can study from this stack

- **Parley's `accesskit` feature** as the model for how a text layout engine surfaces text-run accessibility info. When `buiy_text` builds AccessKit `Node`s for text widgets, the per-run boundaries should map onto something like Parley's text-run model.
- **Vello's compute-shader-based anti-aliased fill** as the model for how Buiy renders rounded clip-paths and complex shapes without a triangulator. Vello's source is GPL-clear-by-Apache-2.0; the algorithms are documented in Raph's blog posts.
- **Linebender Color's color-space-aware interpolation** as the model for gradient sampling in CSS-spec-compliant color spaces (OKLab, Display-P3, etc.).

## What Buiy is *not* studying

- **Parley's editing API** — Buiy's `buiy_text` uses cosmic-text's editor primitives directly.
- **Fontique's font-enumeration API** — cosmic-text's enumeration is fine for v1; revisit if cosmic-text's coverage thins.
- **Vello as a render dependency** — Buiy ships its own wgpu render passes integrated into Bevy's render graph.

## Cross-references

- [`../cosmic-text/`](../cosmic-text/) — the Buiy text-shaper folder. Reading order: `lessons.md` first, then `editing.md` for the IME story.
- [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) Top-of-file finding #2 — the Bevy 0.19 migration framing.
- [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md) Validates row "Parley" — woodpecker_ui's Parley use confirms the upstream is production-viable for game-UI workloads.

## Sources

- Parley repo: https://github.com/linebender/parley
- Parley docs.rs: https://docs.rs/parley/latest/parley/
- Skrifa repo: https://github.com/googlefonts/fontations
- Fontique: part of Parley workspace
- Vello repo: https://github.com/linebender/vello
- harfrust: https://github.com/harfbuzz/harfrust (used by both stacks)
- swash: https://github.com/dfrg/swash
- cosmic-text: https://github.com/pop-os/cosmic-text
- Bevy issue #21765 (Parley migration): https://github.com/bevyengine/bevy/issues/21765
