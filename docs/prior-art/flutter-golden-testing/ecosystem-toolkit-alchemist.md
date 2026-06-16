**Date:** 2026-06-14
**Status:** active
**Subject:** Third-party Flutter golden tooling — golden_toolkit (eBay, discontinued) and Alchemist (Betterment, active), and the platform-tests-vs-CI-tests two-tier split

# Third-party ecosystem: golden_toolkit and Alchemist

Two community packages sit on top of `matchesGoldenFile`. The first (`golden_toolkit`) was the de-facto standard and is now discontinued; the second (`Alchemist`) is its active successor and is the **clearest articulation of the rectangle/real-font split** that Buiy should mirror.

## golden_toolkit (eBay) — discontinued

Built directly on `matchesGoldenFile`. Notable APIs:

- **`loadAppFonts()`** — "By default, flutter test only uses a single 'test' font called Ahem… loadAppFonts will automatically load the Roboto font, and any fonts included from packages you depend on." Recommended from a `flutter_test_config.dart` so it runs once for the whole suite ([loadAppFonts docs](https://pub.dev/documentation/golden_toolkit/latest/golden_toolkit/loadAppFonts.html)). This is the *opposite* of the obscure-text approach — it loads **real** fonts so goldens look human-readable, at the cost of host-dependence.

  **The suite-wide hook itself is the reusable mechanism.** `flutter_test_config.dart` is how Flutter makes a font (or any setup) the default across a whole test suite without per-test boilerplate: the framework "scan[s] up the directory hierarchy, starting from the directory in which the test file resides, looking for a file named `flutter_test_config.dart`," and if found expects a top-level `Future<void> testExecutable(FutureOr<void> Function() testMain)` that wraps and invokes the test's own `main()` ([flutter_test library docs](https://api.flutter.dev/flutter/flutter_test/)). The closest such file to the test wins; all others are ignored. Font registration (`loadAppFonts`, or Alchemist's Ahem-forcing) is wired here precisely because it must run once before any test. **Implication for Buiy:** "how do I make the test font the default across a whole suite?" is the first question a Buiy golden-test author hits — Buiy needs an equivalent directory-scoped, run-once setup hook (registering `BUIY_TEST_FONT` and the determinism knobs) so individual tests don't each opt in.
- **`multiScreenGolden()`** — runs a widget across a device list, emitting one PNG per device with the device name appended, auto-sizing the surface to capture scrollables ([golden_toolkit README](https://github.com/eBay/flutter_glove_box/blob/master/packages/golden_toolkit/README.md)).

**Wart (verified):** `golden_toolkit` is **discontinued**. Current version **0.15.0**, publisher **eBay.com** (`ebay.com` on pub.dev), last published **2023-02-21** (per the pub.dev API; ~3 years before this writing). Treat it as historical prior art, not a live dependency. Its abandonment is what pushed the community to Alchemist.

## Alchemist (Betterment + Very Good Ventures) — active

Alchemist institutionalizes the two-tier split. It generates **two** snapshot sets ([Alchemist GitHub](https://github.com/Betterment/alchemist)):

- **Platform tests** — generate "golden files with human readable text," run locally per-OS into `goldens/<platform_name>/`. Host-dependent; **not committed** to source control.
- **CI tests** — identical *except* "the text blocks are replaced with colored squares," stored in `goldens/ci/`. CI tests are "always run using the Ahem font family … to ensure that CI tests are platform agnostic — their output is always consistent regardless of the host platform." Only these are tracked in source control.

The rationale, verbatim: "individual platforms are known to render text differently than others… causing CI systems to fail the test."

**Controls:**

- **`obscureText`** — on for CI, off for platform; toggles whether text "should be obscured by colored rectangles… useful for circumventing issues with Flutter's font rendering between host platforms."
- **`renderShadows`** — replaces shadows with "opaque colors… because shadow rendering can be inconsistent between test runs" (the package-level analog of `debugDisableShadows`, see [determinism-knobs.md](determinism-knobs.md)).
- **`diffThreshold`** — per-config tolerance, set on `PlatformGoldensConfig` / `CiGoldensConfig` (under `AlchemistConfig`).

**Status (verified):** active, MIT, **v0.14.0 (2026-03-13)**, ~298 GitHub stars *(point-in-time figure)*.

## The split, distilled

| | CI / layout tier | Real-font fidelity tier |
|---|---|---|
| Text | obscured → colored rectangles / Ahem box glyphs | real fonts, human-readable |
| Shadows | flat opaque fills | real shadows |
| Where run | any host / CI | locally, one canonical OS |
| Committed? | yes (deterministic, tiny diffs) | no (host-dependent) |
| Catches | layout, composition, stacking, sizing | kerning, fallback, emoji, real shaping |
| Size of suite | the **bulk** | deliberately **narrow** |
| Tolerance | exact / near-exact | threshold-tolerant, flake accepted |

CI/layout goldens obscure text → no glyph rasterization in the comparison → stable across OS / engine / font-revision. A small real-font tier catches genuine text-fidelity regressions, accepting that it is platform-bound.

## Implications for Buiy

Buiy should mirror this two-tier shape directly:

1. **Broad tier** — box-glyph (`BUIY_TEST_FONT`, see [obscure-text-font.md](obscure-text-font.md)) + flat-shadow (`BUIY_DISABLE_SHADOWS`, see [determinism-knobs.md](determinism-knobs.md)) mode for the reftest/structured/golden bulk. Deterministic, committed, tiny diffs.
2. **Narrow tier** — a tiny real-`cosmic-text`/`harfrust` fidelity suite, pinned to one bundled OFL font and a controlled rasterizer, where real shaping fidelity (kerning, fallback, emoji) is asserted and flake is an accepted cost.

**What Buiy should *not* center on:** golden_toolkit's `loadAppFonts()` real-font-everywhere approach — it is exactly the host-dependence the obscure-text font exists to avoid, and the package is discontinued. The lineage's verdict is deterministic-font-first, real-glyph-narrow. Alchemist's `diffThreshold`-per-config is also worth borrowing: the broad tier gets near-zero tolerance, the fidelity tier gets a generous budget. See [lessons.md](lessons.md).

## Sources

- golden_toolkit (pub.dev, discontinued): https://pub.dev/packages/golden_toolkit
- golden_toolkit README: https://github.com/eBay/flutter_glove_box/blob/master/packages/golden_toolkit/README.md
- loadAppFonts docs: https://pub.dev/documentation/golden_toolkit/latest/golden_toolkit/loadAppFonts.html
- `flutter_test_config.dart` / `testExecutable` (suite-wide setup hook): https://api.flutter.dev/flutter/flutter_test/
- Alchemist GitHub: https://github.com/Betterment/alchemist
- alchemist (pub.dev): https://pub.dev/packages/alchemist
- Sibling files: [obscure-text-font.md](obscure-text-font.md), [determinism-knobs.md](determinism-knobs.md), [lessons.md](lessons.md)
