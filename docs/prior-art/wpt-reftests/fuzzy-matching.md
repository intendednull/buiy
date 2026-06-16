**Date:** 2026-06-14
**Status:** active
**Subject:** Fuzzy matching — the two-axis tolerance budget, the `fuzzy()` / `fuzzy-if()` / `<meta name=fuzzy>` syntax, and the pin-both-ends discipline

# Fuzzy matching

A reftest asserts that the test page and the reference page render to *identical* pixels (or, with `!=`, to *non*-identical pixels). In practice exact equality is too strict: anti-aliasing, sub-pixel positioning, GPU/driver rounding, and spec-permitted implementation latitude produce tiny, legitimate per-pixel differences that would otherwise flood the suite with false failures. Fuzzy matching lets a reftest tolerate a *bounded* amount of difference while still failing on anything outside the bound. Both Gecko reftest and web-platform-tests use the **same two-axis model**, and an annotation must specify both axes.

## The two axes

1. **Maximum per-channel pixel difference** — the largest allowed difference in any single RGB(A) color channel of any one pixel ("how wrong per pixel").
2. **Number of differing pixels** — the total count of pixels allowed to differ at all ("how many pixels wrong").

The split exists because the two failure modes it discriminates are physically different. A *small* color delta spread over *many* pixels (anti-aliasing along a long edge) is benign; a *large* color delta even on a *few* pixels (a glyph in the wrong color, or a box shifted a pixel) is a real bug. A single "percent different" scalar cannot separate these — it would either accept a catastrophic small-area error or reject benign large-area smoothing. Splitting "how wrong per pixel" from "how many pixels wrong" lets authors accept the first while still catching the second.

## Syntax

**Gecko** (manifest annotation): `fuzzy(minDiff-maxDiff,minPixelCount-maxPixelCount)`, with the conditional form `fuzzy-if(condition,minDiff-maxDiff,minPixelCount-maxPixelCount)` to scope a budget to a platform/config (e.g. `fuzzy-if(cocoaWidget,1-1,8-8)`). Each axis is a **range**, not a single number, and **both ends of both ranges are checked inclusively**: the observed max-channel difference must fall within `[minDiff, maxDiff]` *and* the observed differing-pixel count within `[minPixelCount, maxPixelCount]`, or the test fails.

**WPT** (markup meta tag): `<meta name=fuzzy content="maxDifference=10-15;totalPixels=200-300">`. The argument names are optional — `"15;300"` is equivalent to the named form — and "These range checks are inclusive." When a test has several possible references with different tolerances, "One meta element is required per reference requiring a unique fuzziness value, but any unprefixed value will automatically be applied to any ref that doesn't have a more specific value" (prefix form: `option1-ref.html:10-15;200-300`). For `!=` (mismatch) reftests the **minimum bounds of the ranges must be zero**.

## The discipline: pin both ends, do not include 0

The non-obvious doctrine — and the part most worth importing into Buiy — is that a range is a *two-sided* assertion, and when a difference is *expected* the range should **not** include 0. Gecko's guidance: use the tightest bounds possible; "if the behavior is entirely deterministic this means a range like `fuzzy(1-1,8-8)`, and if at all possible, the ranges should not include 0."

- **Pinning the lower end** (1, not 0) means that if the underlying bug is later *fixed* and the test starts matching exactly, the harness reports an **unexpected pass** — the signal that the `fuzzy()` annotation is now stale and can be removed, restoring exact-match coverage. A range that starts at 0 silently swallows that signal: the test "passes" whether the difference is present or gone, so a fix (or a *further* regression that happens to land back inside the window) goes unnoticed.
- **Pinning the upper end** catches the difference *growing* past the calibrated budget.

So a deterministic case should use `n-n` (e.g. `1-1`, `8-8`), widening to `lo-hi` only as much as genuine run-to-run variance demands — the window being the smallest interval that still passes reliably while leaving regressions outside it.

## The wart: when 0 is unavoidable

The docs concede the limit honestly: "In cases where the test only sometimes fails, this unfortunately requires using 0 in both ranges," and the consequence is stated plainly — "we won't get reports of an unexpected pass if the test regresses further." So intermittently-failing (non-deterministic) tests are forced into the weaker 0-inclusive form and **lose the regression-catching property**; that is the acknowledged cost, not a feature.

> *Verification flag:* the longer Gecko paragraph could not be pulled verbatim through the fetch tool (a content-length guard blocked literal quotation); the substance was cross-confirmed across two independent search retrievals of that same page, but the precise sentence punctuation is reconstructed, not byte-exact. The two-axis definition, inclusive-range semantics, the `fuzzy(1-1,8-8)` example, the "should not include 0" rule, and the intermittent-test caveat are all from the cited primary docs.

## How authors calibrate the budget

The numbers are empirical, measured from a real failing run rather than guessed. The tooling reports the *actual* max-channel difference and differing-pixel count for a comparison — under `wpt run` via logging (e.g. `--log-mach=-`), and Gecko's reftest output prints the detected `image comparison ... max difference / different pixels` for a fuzzy failure. The author reads off the observed pair, then sets the range tight around it. Third-party tooling exists to make this triage visual — e.g. Gankra's `live-reftest-analyzer` for inspecting failed Gecko reftests.

## Implications for Buiy

This is the metric model Buiy's Tier-4 (and the unified perceptual metric) must copy directly, not reinvent. The strategy report (`§ Cross-cutting mechanisms`) already commits Buiy to a two-axis fuzzy/outlier gate — `(max_pixel_delta, max_diff_pixels)`, AA-excluded — replacing the existing naive L1 and RMSE metrics that cannot express it. Adopt per-reference fuzz (WPT prefixes the ref URL; Buiy would carry the budget on the `RefCase`). Adopt the **pin-both-ends, never-include-0** discipline so a fixed bug surfaces as an unexpected pass that retires the budget — this is Open Question #2 in the strategy report ("is Buiy willing to pin both ends?"). And inherit the honest wart: intermittently-failing Buiy reftests would have to drop to 0-inclusive and lose the regression signal — which is itself an argument for engineering determinism at the source (the determinism stack) so the deterministic `n-n` form stays usable.

## Sources

- Firefox Source Docs, Reftest — https://firefox-source-docs.mozilla.org/layout/Reftest.html
- web-platform-tests, writing reftests — https://web-platform-tests.org/writing-tests/reftests.html
- Gankra, `live-reftest-analyzer` — https://github.com/Gankra/live-reftest-analyzer
- Sibling files: [gecko-reftests.md](gecko-reftests.md), [methodology.md](methodology.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md)
- Buiy strategy report (two-axis metric, Open Question #2) — [../../reports/2026-06-14-visual-bug-detection-strategy.md](../../reports/2026-06-14-visual-bug-detection-strategy.md)
