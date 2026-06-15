**Date:** 2026-06-14
**Status:** active
**Subject:** Flutter golden testing — Validates / Avoid / Borrow decision file for Buiy's visual-bug detection (text goldens + determinism knobs)

This is the consult-this-when-designing file. The other files in this folder are evidence; this is synthesis. When designing any Buiy visual-test feature that touches text goldens, determinism knobs, or the Tier-5 golden harness — `buiy-verification-design`, the text-golden suite, the forced-colors/shadow determinism work — start here.

The one-line lesson: Flutter's record is an empirical argument for **reftests-over-pixels and determinism-knobs-over-pixel-plumbing**. Even Google-scale Gold tooling does not fully tame host-rasterization flake (#131559). Buiy borrows the knobs, not the hosted service.

## Validates

These Buiy design choices are confirmed by Flutter's experience:

- **Reftests-first / keep the pixel tier minimal.** Flutter's whole golden apparatus exists to fight glyph-rasterization flake, and even with Gold content-addressing, `goldctl`, per-PR human triage, and Docker pinning, host-OS rasterization still leaks ([#131559](https://github.com/flutter/flutter/issues/131559)). This is direct evidence for the strategy report's pyramid: push detection *down* into deterministic tiers and shrink the flaky golden tier to a residue. See [open-problems.md](open-problems.md).
- **An obscure box-glyph test font as the default determinism mode.** Flutter swaps all unspecified text to a box-glyph font (Ahem → FlutterTest) precisely so layout/golden output is identical across OSes. This validates Buiy's plan for a `BUIY_TEST_FONT`. See [obscure-text-font.md](obscure-text-font.md).
- **A shadow killswitch in golden mode.** `debugDisableShadows` is **default-on** in Flutter's test binding because shadow blur is non-deterministic version-to-version and run-to-run. Buiy's SDF shadow pass is the same risk class; a swap-to-flat-fill knob is validated. See [determinism-knobs.md](determinism-knobs.md).
- **A two-tier split: broad obscure-text/flat-shadow + narrow real-font.** Alchemist institutionalizes exactly this (CI tests forced to Ahem and committed; platform tests with real fonts, run locally, not committed). Buiy's "box-glyph bulk + tiny real-`cosmic-text` fidelity tier" mirrors it. See [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md).
- **A curated-accept workflow with explicit blessing.** Flutter goldens are regenerated with `flutter test --update-goldens` and approved by a human pre-submit. Buiy already has `BUIY_ACCEPT_SHAPING` as a curated-accept gate; this validates generalizing it across tiers (`BUIY_ACCEPT_*`). See [flutter-gold-infra.md](flutter-gold-infra.md).
- **The comparator-as-swappable-backend seam.** `matchesGoldenFile` does no compare itself; an abstract `GoldenFileComparator` is the extension point. Buiy's `GoldenConfig` / backend-selection is the analogous seam. See [matches-golden.md](matches-golden.md).

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **A pixel-exact local comparator as the primary text-golden backend.** `LocalFileComparator` is zero-tolerance and "a golden file generated on Windows … will likely differ from the one produced by another operating system." | [matches-golden.md](matches-golden.md), [LocalFileComparator](https://api.flutter.dev/flutter/flutter_test/LocalFileComparator-class.html) | Collapse the font axis upstream (box-glyph + power-of-2 UPM) so the local comparator has nothing host-dependent to disagree about; reserve real-font goldens for one pinned host with a fuzzy budget. |
| **Treating "boxes instead of curves" as the whole determinism win.** Ahem (UPM 1000) still "yields slightly different metrics on different platforms." | [obscure-text-font.md](obscure-text-font.md) | The win is boxes **AND** a power-of-2 UPM (FlutterTest = 1024) with pinned ascent/descent → integer-exact, font-engine-agnostic metrics. Pick UPM 1024 for `BUIY_TEST_FONT`. |
| **Goldening everything with real fonts (the golden_toolkit `loadAppFonts` shape).** Real fonts everywhere = host-dependence everywhere; culprits are "system fonts, missing glyphs, font fallbacks." | [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md) | Real-font goldens are a deliberately *narrow* tier, pinned to one bundled OFL font + controlled rasterizer. `golden_toolkit` is also **discontinued** — do not center a strategy on it. |
| **A hosted Gold-class triage service.** Flutter Gold is Google-Cloud-operated, has an operational floor, gets stuck pending on force-push, and is a documented flake source. | [flutter-gold-infra.md](flutter-gold-infra.md), [open-problems.md](open-problems.md) | Buiy is local-first: commit small box-font goldens to the repo (tiny diffs), keep a host-pinned local rasterizer for the residue, no SaaS. The [`skia-gold`](../skia-gold/README.md) folder covers the storage tradeoffs. |
| **Skipping flaky goldens as a flake remedy.** "When we skip a test we stop sending to Skia Gold entirely" → "no way to verify flaky golden test fixes." | [open-problems.md](open-problems.md), [#111325](https://github.com/flutter/flutter/issues/111325) | Make the tier deterministic-by-construction (box font, flat shadows, fixed clock, warm atlas) so flake doesn't arise; don't manage it by disabling. |
| **A debug-build-only shadow killswitch.** Flutter's `debugDisableShadows` only toggles inside a single test case; there is an open ask to push it into the engine as a runtime flag. | [determinism-knobs.md](determinism-knobs.md), [#105475](https://github.com/flutter/flutter/issues/105475) | Implement `BUIY_DISABLE_SHADOWS` **engine-side from the start** so it works in release-mode test binaries, not as a `debug_assertions`-gated hack. |
| **Trying to make color emoji deterministic.** Color-emoji rendering is platform-divergent and unstable even within Flutter; a box font cannot collapse it. | [open-problems.md](open-problems.md) | Treat Buiy's color-emoji path as the irreducible real golden: pinned hardware + font, generous diff tolerance. Don't fight it with determinism knobs. |
| **Snapshotting a wrong-but-blessed golden as "correct."** Goldens assert "matches the blessed image," not "is correct"; Gold auto-approves a triaged digest forever. | [open-problems.md](open-problems.md) | Prefer reftests (assert `render-A == / != render-B` — a relational oracle) and property invariants (laws, no oracle) below the golden tier; goldens are last resort. This is the strategy report's thesis. |

## Borrow

Concrete primitives and patterns from Flutter worth adapting into Buiy:

1. **A box-glyph test font with power-of-2 UPM.** Build/ship `BUIY_TEST_FONT` with UPM **1024**, pinned ascent/descent (e.g. 0.75/0.25 em like FlutterTest), line-gap 0, every glyph a solid em-box. Default it for the layout-number → reftest tiers. This is the single cheapest determinism lever. *(Confirm the original Ahem's redistribution license before bundling — unverified — or generate a clean-room font.)* See [obscure-text-font.md](obscure-text-font.md).
2. **A shadow killswitch (`BUIY_DISABLE_SHADOWS`).** Swap the SDF shadow for a flat fill in the cheap tiers; default-on in golden mode, engine-side. See [determinism-knobs.md](determinism-knobs.md).
3. **The two-tier golden shape.** Broad obscure-text + flat-shadow tier (deterministic, committed, near-zero tolerance) + a narrow real-`cosmic-text`/`harfrust` fidelity tier (one pinned font, controlled rasterizer, generous threshold). Borrow Alchemist's `diffThreshold`-per-config idea. See [ecosystem-toolkit-alchemist.md](ecosystem-toolkit-alchemist.md).
4. **Generalized curated-accept (`BUIY_ACCEPT_*`).** Extend the existing `BUIY_ACCEPT_SHAPING` pattern so accepting any golden is an explicit, reviewable, diffable act — not a silent overwrite. Flutter's `--update-goldens` + human pre-submit triage is the precedent. See [flutter-gold-infra.md](flutter-gold-infra.md).
5. **The comparator-as-backend seam.** Keep golden comparison behind a swappable backend (Buiy's `GoldenConfig`), so local pixel-diff, re-capture-determinism checks, and any future host-pinned rasterizer are interchangeable. See [matches-golden.md](matches-golden.md).
6. **A two-parameter fuzzy-comparison budget for the host-pinned residue tier.** Flutter's Impeller golden harness gates on two knobs, not one: `maxDiffPixelsPercent` (what fraction of pixels may differ) **and** `pixelColorDelta` (the max per-channel color delta a differing pixel may have) — "less than 1% of pixels are different by less than 4 color component deltas" ([engine PR #40824](https://github.com/flutter/engine/pull/40824)). This two-axis shape (a count budget *and* a per-pixel magnitude budget) is the concrete primitive Buiy's fuzzy comparator needs for the narrow real-font tier; Buiy's current naive L1/RMSE metrics collapse both axes into one scalar and lack an AA-aware budget (strategy report §4). Build the comparator backend to take both. See [matches-golden.md](matches-golden.md), [flutter-gold-infra.md](flutter-gold-infra.md).
7. **Borrow as *cautionary baseline*, not as a build target: hosted Gold.** Study Gold's many-positives + content-addressing as ideas (already covered in [`skia-gold`](../skia-gold/README.md)/lessons), but do not build the service.

## How to use this file

When designing a Buiy visual-test feature: (1) find the **Avoid** row nearest your design, follow the linked evidence file, apply the mitigation; (2) find the **Borrow** item nearest the primitive you're building, read the evidence for shape, adapt for Buiy. Promote any decision into a spec under `docs/specs/` — this file captures what we learn from Flutter, not Buiy's own decisions.

## Sources

- All sibling files in this folder.
- `LocalFileComparator`: https://api.flutter.dev/flutter/flutter_test/LocalFileComparator-class.html
- `debugDisableShadows`: https://api.flutter.dev/flutter/rendering/debugDisableShadows.html
- Flutter-Test-Fonts.md: https://github.com/flutter/flutter/blob/master/docs/contributing/testing/Flutter-Test-Fonts.md
- flutter/flutter#131559, #111325, #105475
- Alchemist: https://github.com/Betterment/alchemist
- Buiy visual-bug-detection strategy: [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md)
- Skia Gold storage/triage tradeoffs: [`docs/prior-art/skia-gold/`](../skia-gold/README.md)
