**Date:** 2026-06-14
**Status:** active
**Subject:** `matchesGoldenFile` and the `GoldenFileComparator` backend — Flutter's golden matcher and why its local default flakes across hosts

# matchesGoldenFile and the comparator backend

## The matcher

`matchesGoldenFile` is `flutter_test`'s golden-file matcher. The signature is `AsyncMatcher matchesGoldenFile(Object key, {int? version})` ([matchesGoldenFile API](https://api.flutter.dev/flutter/flutter_test/matchesGoldenFile.html)):

- **`key`** — a `Uri`/`String` URL identifying the golden image. Any other type throws an `ArgumentError`.
- **`version`** — an optional `int` "to differentiate historical golden files."

Because it is asynchronous, it must be driven via `await expectLater(...)`. It accepts:

- a **`Finder`** — which must match exactly one widget; it then captures the rendered image of that widget's **first `RepaintBoundary` ancestor**,
- a **`Future<ui.Image>`**, or
- a **`ui.Image`**.

Golden images are written or refreshed with `flutter test --update-goldens`.

The matcher itself **does no comparison**. It delegates to the top-level ambient `goldenFileComparator`, "which acts as the backend for this matcher" ([matchesGoldenFile API](https://api.flutter.dev/flutter/flutter_test/matchesGoldenFile.html)). That indirection is the entire extension story — swapping the comparator is how Flutter switches between local pixel-diffing and the Skia-Gold cloud backend.

## The comparator extension seam

`GoldenFileComparator` is an abstract class with two methods ([GoldenFileComparator API](https://api.flutter.dev/flutter/flutter_test/GoldenFileComparator-class.html)):

- **`compare`** — "Compares the pixels of decoded png `imageBytes` against the golden file identified by `golden`."
- **`update`** — "Updates the golden file identified by `golden` with `imageBytes`."

Comparators run "in the `TestWidgetsFlutterBinding.runAsync` zone and are thus not subject to the fake async constraints." Being abstract is the seam: subclasses implement local pixel-diffing, Skia-Gold upload, or skip-on-unsupported-environment behavior. The framework picks among them at test bootstrap (see [flutter-gold-infra.md](flutter-gold-infra.md)).

## The default: `LocalFileComparator` is brutally strict

The default backend for `flutter test` is `LocalFileComparator`. It ([LocalFileComparator API](https://api.flutter.dev/flutter/flutter_test/LocalFileComparator-class.html)):

- "loads golden files from the local file system, treating the golden key as a relative path from the test file's directory," and
- "performs a pixel-for-pixel comparison of the decoded PNGs, returning true only if there's an exact match."

**Zero tolerance.** No AA-exclusion, no fuzzy budget, no per-channel delta. That exactness is exactly what makes it flake across hosts.

(The no-op default in raw `flutter_test` — before `flutter_goldens` wires anything — is `TrivialComparator`; see [flutter-gold-infra.md](flutter-gold-infra.md).)

## Why the local default flakes across hosts

The API docs warn directly: "Custom fonts may render differently across different platforms, or between different versions of Flutter. For example, a golden file generated on Windows with fonts will likely differ from the one produced by another operating system" ([GoldenFileComparator API](https://api.flutter.dev/flutter/flutter_test/GoldenFileComparator-class.html), [LocalFileComparator API](https://api.flutter.dev/flutter/flutter_test/LocalFileComparator-class.html)).

The root cause: different OSes scale and rasterize fonts with **different font engines**, so a pixel-exact PNG baked on macOS fails a byte-comparison on a Linux CI runner. Subpixel font smoothing and anti-aliasing differences a human eye cannot see, but a `==` on decoded bytes catches every time.

The standard practitioner mitigations all exist because `LocalFileComparator` itself offers no tolerance:

- bundle fonts; never depend on system fonts,
- pin device-pixel-ratio,
- run goldens on one canonical OS.

**Device-pixel-ratio and surface size are their own flake axis.** `matchesGoldenFile` captures whatever physical-pixel surface the test renders, so the device-pixel-ratio (DPR) and logical surface size must be pinned, not just the OS and font. The same widget at DPR 1.0 vs 2.0 produces different physical pixel counts and different sub-pixel snap positions; an unpinned DPR makes a golden non-reproducible even on one host. Flutter's widget-test binding fixes a default test surface and DPR for exactly this reason, and the higher-level packages let you fan out a fixed device list (golden_toolkit's `multiScreenGolden`, see [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md)) so each DPR/size is its own deterministic golden rather than a flake source. **Implication for Buiy:** because Buiy targets wgpu/HiDPI, the golden harness must pin both the logical surface size and the device-pixel-ratio per golden (and treat logical-vs-physical scaling as an explicit, asserted axis), not assume a 1:1 mapping.

Flutter's deeper answers — collapsing the font axis with an obscure box-glyph test font ([obscure-text-font.md](obscure-text-font.md)) and moving the source of truth to a multi-positive server ([flutter-gold-infra.md](flutter-gold-infra.md)) — are both responses to this same zero-tolerance pixel-exactness.

## Implications for Buiy

The load-bearing fact for Buiy: a pixel-exact *local* comparator flakes across hosts, so any Buiy text/pixel golden tier needs **either** a tolerance knob **or** a server-side multi-master backend (which Buiy, being local-first, declines — see [lessons.md](lessons.md) `## Avoid`). The cleaner answer Buiy reaches for first is *upstream determinism*: collapse the font axis so the local comparator has nothing host-dependent to disagree about.

When a tolerance knob *is* needed (the narrow real-font residue tier), the shape to copy is Flutter's Impeller golden harness, which uses **two** parameters rather than one scalar: `maxDiffPixelsPercent` (the fraction of pixels allowed to differ) **and** `pixelColorDelta` (the max per-channel color delta a differing pixel may have) — passing when "less than 1% of pixels are different by less than 4 color component deltas" ([engine PR #40824](https://github.com/flutter/engine/pull/40824)). That two-axis budget (a count *and* a per-pixel magnitude) is what an AA-aware comparator needs; Buiy's current naive L1/RMSE metrics collapse both axes into one and lack such a budget (strategy report §4). Build the fuzzy backend to take both knobs — see [lessons.md](lessons.md) `## Borrow`.

The comparator-as-swappable-seam pattern itself is worth borrowing — Buiy's `GoldenConfig` is the analogous backend-selection point.

## Sources

- `matchesGoldenFile` API: https://api.flutter.dev/flutter/flutter_test/matchesGoldenFile.html
- `GoldenFileComparator` API: https://api.flutter.dev/flutter/flutter_test/GoldenFileComparator-class.html
- `LocalFileComparator` API: https://api.flutter.dev/flutter/flutter_test/LocalFileComparator-class.html
- flutter/engine#40824 (`maxDiffPixelsPercent` + `pixelColorDelta` golden threshold): https://github.com/flutter/engine/pull/40824
- Sibling files: [flutter-gold-infra.md](flutter-gold-infra.md), [obscure-text-font.md](obscure-text-font.md), [lessons.md](lessons.md)
