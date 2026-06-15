**Date:** 2026-06-14
**Status:** active
**Subject:** The Ahem → FlutterTest obscure-text font — why box glyphs and a power-of-2 units-per-em make layout/text golden output font-engine-agnostic

# The obscure-text determinism font (Ahem → FlutterTest)

To stop *text* from being the flake source, `flutter test` substitutes a single obscure test font for all unspecified text: "if fontFamily isn't specified or the specified font families are not available, the default test font FlutterTest will be used" ([Flutter-Test-Fonts.md](https://github.com/flutter/flutter/blob/master/docs/contributing/testing/Flutter-Test-Fonts.md)). The rationale, verbatim: "Rectangles are used for text to avoid curves that might cause irrelevant test failure when comparing pixels." This is the single cheapest determinism lever in the whole golden stack.

## Two failing modes the font removes

The lineage is two distinct fixes layered:

1. **Curve-rasterization variance.** Real glyph outlines are curves; different platform font engines anti-alias those curves differently, so the same glyph rasterizes to slightly different pixels per OS. A glyph that is a **solid box filling the em square** has no curves — nothing for the rasterizer to disagree about.
2. **Metric rounding variance.** Even with box glyphs, the *metrics* (advance width, ascent, descent, baseline) are computed by the platform font engine and can round differently. This is where the **units-per-em** choice matters.

## Ahem vs FlutterTest — the verified metrics

| Font | Ascent | Descent | Units-per-em | Line-gap |
|---|---|---|---|---|
| **FlutterTest** (current default) | 768 (0.75 em) | 256 (0.25 em) | **1024** | 0 |
| **Ahem** (legacy default) | 800 (0.8 em) | 200 (0.2 em) | **1000** | 0 |

Historically the default was **Ahem**, "designed to show black spaces for every character and icon" — solid boxes filling the em square. Flutter now defaults to **FlutterTest**.

## The load-bearing detail: a power-of-2 units-per-em

The decisive difference is not the glyph shape (both are boxes) — it is the em size. FlutterTest's "`1024 units-per-em` is a power of 2, making it less likely to introduce precision loss in metrics calculations, when used as a divisor… FlutterTest generally provides more precise and font-engine-agnostic font/glyph metrics than `Ahem`" ([Flutter-Test-Fonts.md](https://github.com/flutter/flutter/blob/master/docs/contributing/testing/Flutter-Test-Fonts.md)).

The doc names Ahem's exact failing directly: "with the `Ahem` font you would get slightly different metrics on different platforms, since they use different font engines to scale the font." Ahem's UPM of **1000** is not a power of 2, so dividing by it in metric scaling introduces floating-point precision loss that diverges per engine. The power-of-2 em makes the divisions exact (or at least bit-identical across engines), which is what makes the metrics font-engine-agnostic.

**The takeaway is not "boxes instead of curves" — it is "boxes AND a power-of-2 UPM with pinned ascent/descent."** Boxes kill the curve-rasterization axis; the power-of-2 em kills the metric-rounding axis. You need both for integer-exact, engine-independent layout numbers.

## Shaped variants

FlutterTest also ships shaped variants for exercising specific layout cases: **Square**, **"Ascent Flushed,"** **"Descent Flushed,"** and varying x-advance glyphs (**Full**, **1/2**, **1/3**) — all with "no outlines in the glyph." These let tests assert advance-width and baseline behavior with predictable, integer-clean metrics.

## Implications for Buiy

This is the determinism knob Buiy should mirror most directly. Ship a `BUIY_TEST_FONT` — an Ahem-style box-glyph font — and make it the default for the cheap, broad tiers (layout-number snapshots through reftests). When choosing/building it:

- **Pick a power-of-2 units-per-em** (1024, like FlutterTest) and **pin ascent/descent** so glyph metrics are integer-exact. This is the *actual* determinism win — it makes cosmic-text/harfrust shaping + Taffy line-breaking produce byte-identical layout numbers regardless of the host's FreeType/HarfBuzz build, collapsing the font axis for the bulk of text-bearing goldens.
- **Box glyphs are still real coverage.** The glyph atlas still exercises its rasterize/pack/upload path; the *only* variable left is Buiy's own code, not the system font stack.
- **License caveat (unverified).** The original Ahem font ships with WebKit/Blink/Flutter as a permissively-licensed test asset, but its exact redistribution terms were not confirmed against a primary license file in this pass. Confirm before bundling, or generate a clean-room box font.

Buiy keeps a separate narrow real-font fidelity tier for the cases a box font cannot test (kerning, fallback, emoji) — see [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md) and [lessons.md](lessons.md).

## Sources

- Flutter-Test-Fonts.md: https://github.com/flutter/flutter/blob/master/docs/contributing/testing/Flutter-Test-Fonts.md
- Flutter rendering breaking-changes (Ahem → FlutterTest default swap): https://docs.flutter.dev/release/breaking-changes/rendering-changes
- Sibling files: [matches-golden.md](matches-golden.md), [determinism-knobs.md](determinism-knobs.md), [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md), [lessons.md](lessons.md)
