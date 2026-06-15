**Date:** 2026-06-14
**Status:** active
**Subject:** Flutter's golden determinism knobs — `debugDisableShadows`, `obscureText`, and the layered fixed-font + shadow-killswitch + colored-rectangle stack

# Determinism knobs

Flutter's golden-test stack solves text/shadow nondeterminism with three layered knobs: a fixed-metric test font, a shadow killswitch, and (in higher-level packages) "obscure text as colored rectangles." The fixed-metric font has its own file ([obscure-text-font.md](obscure-text-font.md)); this file covers the shadow killswitch and how the knobs layer.

## `debugDisableShadows` — the shadow killswitch

`debugDisableShadows` is a global flag (in the `rendering` library) that "replaces all shadows with solid color blocks… because shadow rendering is not guaranteed to be pixel-for-pixel identical from version to version or even from run to run" ([debugDisableShadows API](https://api.flutter.dev/flutter/rendering/debugDisableShadows.html)).

Key facts:

- It is exposed on the test binding as `disableShadows`, and is **`true` by default** in `AutomatedTestWidgetsFlutterBinding` ([disableShadows API](https://api.flutter.dev/flutter/flutter_test/AutomatedTestWidgetsFlutterBinding/disableShadows.html)). So in the standard widget-test environment, shadows are *off by default* for goldens.
- Mechanically, it forces `BoxShadow.toPaint` to behave as if `blurStyle == BlurStyle.normal`, i.e. it **disables the blur kernel**. The blur math is the non-deterministic part (it varies version-to-version and run-to-run), so removing it removes the flake.
- `BoxDecoration`/`ShapeDecoration` compensate automatically, but **custom painters must account for it** — a painter that draws its own shadow must check the flag.
- It can only be toggled inside a single test case.

The takeaway: the blur/SDF-shadow kernel is a top flake source, so the framework ships a render-time flag that swaps shadows for flat fills in golden mode and turns it on by default.

## `obscureText` — colored-rectangle text (higher-level packages)

Above the framework, packages like Alchemist add an `obscureText` knob that replaces text blocks with colored rectangles for the CI/layout tier (full treatment in [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md)). This is conceptually the same move as the box-glyph font — remove glyph rasterization from the comparison — applied at the widget level rather than the font level. The package docs frame it as "useful for circumventing issues with Flutter's font rendering between host platforms."

## How the knobs layer

The three knobs collapse three independent flake axes:

| Knob | Flake axis removed | Where |
|---|---|---|
| Fixed box-glyph font (FlutterTest/Ahem) | font-engine curve rasterization + metric rounding | framework default ([obscure-text-font.md](obscure-text-font.md)) |
| `debugDisableShadows` (default on) | shadow blur-kernel non-determinism | framework default |
| `obscureText` / colored rectangles | glyph rasterization at the widget level | higher-level packages (CI tier) |

The split worth stealing, stated plainly: **render text as rectangles and shadows as flat fills for the broad layout-golden tier; keep a narrow real-font, real-shadow suite for fidelity** (and accept that the fidelity suite is platform-bound and threshold-tolerant). The broad tier is the bulk of the suite; the fidelity tier is deliberately tiny.

## Implications for Buiy

Buiy's SDF shadow pass is exactly the `debugDisableShadows` kind of risk — blur math plus GPU rounding. Add a `BUIY_DISABLE_SHADOWS` flag that swaps the SDF shadow for a flat fill in the cheap tiers (layout/structured/reftest), leaving real shadow rendering to the host-pinned golden-screenshot tier. There is an open Flutter issue to push this killswitch into the engine as a runtime flag ([flutter/flutter#105475](https://github.com/flutter/flutter/issues/105475)) — Buiy should implement it **engine-side from the start**, not as a debug-build hack, so it is available in release-mode test binaries.

More broadly, Buiy's existing `GoldenConfig` flake-mitigation triad (fixed clock, font-load sync, atlas warmup) is the analog of Flutter's layered knobs; the box-glyph font and shadow killswitch are the two highest-value additions to it. See [lessons.md](lessons.md).

## Sources

- `debugDisableShadows`: https://api.flutter.dev/flutter/rendering/debugDisableShadows.html
- `disableShadows` (default `true` in AutomatedTestWidgetsFlutterBinding): https://api.flutter.dev/flutter/flutter_test/AutomatedTestWidgetsFlutterBinding/disableShadows.html
- flutter/flutter#105475 (push shadow killswitch into the engine as a runtime flag): https://github.com/flutter/flutter/issues/105475
- Alchemist (`obscureText` / `renderShadows`): https://github.com/Betterment/alchemist
- Sibling files: [obscure-text-font.md](obscure-text-font.md), [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md), [lessons.md](lessons.md)
