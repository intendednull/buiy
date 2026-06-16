**Date:** 2026-06-14
**Status:** active
**Subject:** Glossary of Flutter golden-testing terms used across this folder

# Glossary

System-specific terms for the Flutter golden-testing prior-art folder. For Skia Gold's own vocabulary (digest, param, trace, corpus, baseline), see the [`skia-gold`](../skia-gold/README.md) folder's glossary.

- **Golden file** — a reference PNG that a test's rendered output is compared against. "Golden" ≡ the blessed/expected image.

- **`matchesGoldenFile(key, {version})`** — `flutter_test`'s async golden matcher. Captures the first `RepaintBoundary`'s rendered image and delegates comparison to the ambient `goldenFileComparator`. Driven via `await expectLater(...)`. See [matches-golden.md](matches-golden.md).

- **`goldenFileComparator`** — the top-level ambient instance that `matchesGoldenFile` delegates to. Swapping it is how Flutter changes golden backends.

- **`GoldenFileComparator`** — the abstract class (methods `compare` / `update`) that is the extension seam. Subclasses implement local diffing, Gold upload, or skip-on-unsupported.

- **`LocalFileComparator`** — the default backend for `flutter test`. Loads goldens as paths relative to the test file and does a **pixel-for-pixel, zero-tolerance** decoded-PNG comparison. The canonical cross-host flake source.

- **`TrivialComparator`** — the no-op default in raw `flutter_test` before `flutter_goldens` wires a real backend.

- **`--update-goldens`** — the `flutter test` flag that regenerates/refreshes golden files instead of comparing against them.

- **Ahem** — the legacy obscure-text test font: every glyph a solid box filling the em square. Units-per-em **1000** (not a power of 2 → per-platform metric rounding). Designed "to show black spaces for every character and icon."

- **FlutterTest** — the current default test font. Box glyphs like Ahem, but units-per-em **1024** (a power of 2) → more precise, **font-engine-agnostic** metrics. Ascent 0.75 em, descent 0.25 em, line-gap 0. Ships shaped variants (Square, Ascent/Descent Flushed, Full/½/⅓ x-advance).

- **Units-per-em (UPM)** — the font's internal coordinate scale; metrics are expressed in these units and divided by UPM to scale. A **power-of-2 UPM** (1024) makes that division bit-exact across font engines — the load-bearing determinism property. See [obscure-text-font.md](obscure-text-font.md).

- **Obscure text** — rendering text as featureless boxes (box-glyph font) or colored rectangles (`obscureText`) so glyph rasterization is removed from the golden comparison; makes layout deterministic across hosts.

- **`debugDisableShadows` / `disableShadows`** — global/test-binding flag (**default `true`** in tests) that replaces all shadows with solid color blocks, disabling the non-deterministic blur kernel. See [determinism-knobs.md](determinism-knobs.md).

- **`RepaintBoundary`** — the Flutter widget whose rendered image `matchesGoldenFile` captures (the first such ancestor of the matched `Finder`).

- **Flutter Gold** — the Skia Gold instance (`flutter-gold.skia.org`) that the framework uses as its CI golden backend instead of local file compares. A separate engine instance lives at `flutter-engine-gold.skia.org`.

- **`flutter_goldens`** — the in-tree (not-on-pub.dev) package that swaps in a Gold-backed `FlutterGoldenFileComparator` subclass at test bootstrap based on environment.

- **`goldctl`** — Google's CLI client that uploads images + metadata to a Gold instance (`goldctl imgtest add`, `--luci` on CI).

- **`flutter-gold` check** — the per-PR status check that holds pending on any image delta until a `flutter-hackers` human triages (approves) each new image in the Gold dashboard.

- **Triage** — the human act of approving (or rejecting) a new image in Gold. An approved digest auto-passes thereafter (content-addressed).

- **Multiple golden masters / many positives** — Gold's tolerance for several approved images per logical test, absorbing cross-platform rendering differences. The design answer to `LocalFileComparator`'s zero tolerance.

- **`golden_toolkit`** — eBay's community golden helper (`loadAppFonts`, `multiScreenGolden`). **Discontinued** (latest 0.15.0).

- **`loadAppFonts()`** — golden_toolkit helper that loads **real** fonts (Roboto + package fonts) instead of the box-glyph test font — the opposite of the obscure-text approach, trading determinism for human-readable goldens.

- **Alchemist** — Betterment/Very Good Ventures' active golden package. Splits into **platform tests** (real fonts, local, uncommitted) and **CI tests** (Ahem/obscured, committed); controls `obscureText`, `renderShadows`, `diffThreshold`.

- **Platform tests vs CI tests (Alchemist)** — the two-tier golden split: human-readable real-font goldens run per-OS and not committed, vs obscured-text goldens forced to Ahem, committed, platform-agnostic.

## Sources

- All sibling files in this folder.
- Flutter-Test-Fonts.md: https://github.com/flutter/flutter/blob/master/docs/contributing/testing/Flutter-Test-Fonts.md
- `matchesGoldenFile` / `LocalFileComparator` / `debugDisableShadows` API docs (api.flutter.dev)
- Alchemist: https://github.com/Betterment/alchemist
- Skia Gold vocabulary: [`docs/prior-art/skia-gold/`](../skia-gold/README.md)
