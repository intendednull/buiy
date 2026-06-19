**Date:** 2026-06-14
**Status:** active
**Subject:** Reference-comparison visual testing (Gecko reftests + web-platform-tests) — folder index and prior-art entry point

# Reftests (Gecko + web-platform-tests)

A reftest ("reference test") asserts a *relationship between two renderings produced by the same engine in the same run* — a **testcase** versus a **reference** that reaches the same pixels by a different route — rather than a match against a stored gold image. It is a *methodology*, not a product: the `==` / `!=` (Gecko) and `rel=match` / `rel=mismatch` (WPT) **operator semantics** are shared across Gecko (Firefox), web-platform-tests, Servo, and Blink (Chromium), each running its own tests against its own engine. The shared corpus is WPT, which Gecko, Servo, and Blink import; the operator semantics are common, but the *reference-linking convention* diverges — Blink discovers references by file name (`foo-expected.html` / `foo-expected-mismatch.html`) and supports neither multiple nor chained references, where Gecko/WPT use explicit `<link>` / manifest URLs and do (see [consumers.md](consumers.md)). Because both halves of every comparison share the identical GPU, driver, font rasterizer, antialiasing, DPI, and clock, all platform-variance terms cancel in the diff — the property that lets thousands of CSS-conformance tests run with **zero stored screenshots**.

## For Buiy

Buiy is designing its visual-bug-detection strategy as a 5-tier pyramid (layout-number snapshots → structured display-list snapshots → metamorphic/property invariants → **reftests** → golden screenshots), and reftests are the **Tier 4** headline mechanism — "the single highest-leverage absence" in the current tree (`docs/reports/2026-06-14-visual-bug-detection-strategy.md` § Tier 4). This folder is the prior-art behind that bet. Reftests port to Buiy cleanly: Buiy already renders to an offscreen `wgpu` texture, so rendering a `test_scene` and a `ref_scene` to texture in a single process against the same `wgpu::Device` reproduces the same-engine-cancels-variance guarantee the browsers engineered — and Buiy's primitive (literal-positioned) layer gives a trivial disjoint code path for authoring references. The decision content lives in [lessons.md](lessons.md).

## Honest assessment

- **The methodology is decades-proven but has a hard structural ceiling.** Reftests cannot cover any effect whose pixels are *intentionally* UA-defined or unspecified — underline position/thickness, `dotted`/`dashed`/`ridge`/`groove`/`double` borders, font-metric-dependent rendering — because no feature-free reference can reproduce them. The CSS-WG enumerates these as "impossible to reftest" verbatim. This is a boundary, not a gap in effort, and it is exactly where Buiy must hand off to Tier 5 goldens. See [open-problems.md](open-problems.md).
- **The reference-independence discipline is load-bearing and easy to get wrong.** If the reference exercises the same code path as the test, a shared bug makes both render identically wrong and the test *passes vacuously*. The browsers mitigate with disjoint techniques and multiple references; Buiy must adopt the same discipline or reftests silently lose their teeth. This is Open Question #1 in the strategy report.
- **Fuzzy matching is a necessary escape hatch and a known wart.** Exact equality is too strict (antialiasing, GPU rounding, spec-permitted latitude), so a bounded two-axis tolerance exists — but over-broad fuzz ranges silently mask real regressions, and intermittently-failing tests are forced into a weaker `0`-inclusive form that loses the regression-catching property. See [fuzzy-matching.md](fuzzy-matching.md).
- **Cross-backend reftests reintroduce variance.** The "one run cancels variance" benefit assumes a fixed `wgpu` backend per CI lane. A Vulkan-vs-Metal reftest would reintroduce the very variance reftests cancel and need fuzz — verify before assuming exact-match CI across platforms.

## Key facts (verified 2026-06-14 against the cited primary sources)

| Fact | Value | Source |
|---|---|---|
| What it is | a test *methodology* — relationship between two renderings of *different source files* by the *same engine* | MDN, CSSWG wiki |
| Operators (Gecko) | `==` (pass if renderings SAME) / `!=` (pass if DIFFERENT) | firefox-source-docs Reftest |
| Operators (WPT) | `<link rel=match>` / `<link rel=mismatch>` | web-platform-tests.org reftests |
| Default comparison | **exact pixel match** unless a fuzzy annotation relaxes it | firefox-source-docs, WPT docs |
| Viewport (Gecko) | **800×1000**; content outside is ignored | firefox-source-docs Reftest |
| Viewport (WPT) | **800×600** including scrollbars if present | web-platform-tests.org reftests |
| Manifest (Gecko) | plain-text `reftest.list`; `#` comments; `include` forms a tree | firefox-source-docs, CSSWG wiki |
| Manifest line (Gecko) | `[ <failure-type> \| <pref> ]* [<http>] <type> <url> <url_ref>` | firefox-source-docs Reftest |
| Manifest (WPT) | `link` element + generated `MANIFEST.json` index | web-platform-tests.org |
| Multiple `==` refs | at least one must match (OR) | CSSWG wiki, WPT |
| Multiple `!=` refs | none may match (AND) | CSSWG wiki, WPT |
| Fuzzy (Gecko) | `fuzzy(minDiff-maxDiff,minPixelCount-maxPixelCount)`; `fuzzy-if(cond,…)` | firefox-source-docs Reftest |
| Fuzzy (WPT) | `<meta name=fuzzy content="maxDifference=10-15;totalPixels=200-300">`; ranges **inclusive** | web-platform-tests.org reftests |
| Reference reuse | sharing references is "strongly encouraged" (legibility + runner optimizations) | web-platform-tests.org |
| Async control | `class="reftest-wait"` on root; capture after `TestRendered` + class removal | web-platform-tests.org |
| Harness (Gecko) | `reftest.sys.mjs` (in-content) + `manifest.sys.mjs` + `runreftest.py`; invoked via `mach reftest` | gecko-dev `layout/tools/reftest` |
| Adopted by | Gecko, web-platform-tests, Servo, Blink | firefox-source-docs, chromium docs, servo docs |
| WPT corpus scale | "over 52000 tests and nearly two million subtests" (Servo, 2023-07-20) — **drifts, cite with date** | servo.org/blog/2023/07/20 |

## Contents

| File | Subject |
|---|---|
| [README.md](README.md) | This index — what a reftest is, honest assessment, key facts, reading order. |
| [lessons.md](lessons.md) | **The decision file.** Validates / Avoid / Borrow for Buiy's Tier-4 reftest harness. Start here when designing. |
| [glossary.md](glossary.md) | Reftest / WPT / Gecko terms, one line each. |
| [methodology.md](methodology.md) | The core idea: relationship-not-baseline, why same-engine cancels variance, the `==` / `!=` semantics, `!=` for proving non-no-op. |
| [gecko-reftests.md](gecko-reftests.md) | Gecko mechanics: the `reftest.list` manifest, operators, reference chains, `fails`/`random`/`skip` annotations, the `.sys.mjs` harness. |
| [fuzzy-matching.md](fuzzy-matching.md) | The two-axis tolerance budget, `fuzzy()` / `fuzzy-if()` / `<meta name=fuzzy>` syntax, the pin-both-ends / never-include-0 discipline, and its acknowledged wart. |
| [wpt.md](wpt.md) | web-platform-tests: the cross-vendor corpus, `rel=match`/`rel=mismatch`, the `wpt` runner + `MANIFEST.json`, scale, and the two-way vendor sync. |
| [consumers.md](consumers.md) | How Servo and Blink consume the corpus — out-of-band `.ini` expectations, reftest-first/golden-last ordering, the per-engine pass-state model. |
| [open-problems.md](open-problems.md) | What reftests structurally cannot do: the "impossible to reftest" category, the reference-independence vacuous-pass failure mode, fuzzy's masking risk, cross-backend variance. |

## Reading order

1. **[methodology.md](methodology.md)** — the one idea everything else elaborates: assert a relationship between two engine renderings, not a baseline.
2. **[gecko-reftests.md](gecko-reftests.md)** and **[wpt.md](wpt.md)** — the two concrete realizations of that idea (manifest-file vs. markup-link), with the same comparison semantics.
3. **[fuzzy-matching.md](fuzzy-matching.md)** — the tolerance model Buiy's Tier-4 metric must copy (it is the same two-axis budget the strategy report's perceptual metric targets).
4. **[consumers.md](consumers.md)** — how a *parallel* engine (Servo, the closest prior art) treats the corpus as an external fixture with its own pass-state — the architectural model for Buiy.
5. **[open-problems.md](open-problems.md)** — the irreducible limit that defines the Tier-4/Tier-5 boundary.
6. **[lessons.md](lessons.md)** — the distilled decisions, written for the author of the Buiy reftest harness.

## Framing disclosure

This folder is written from Buiy's stance: an ECS-native (Bevy 0.18) retained-mode Rust GUI library with a custom `wgpu` pipeline and a typed CSS-subset above Taffy, designing a reftests-first visual-bug-detection pyramid. The "Implications for Buiy" subsections and [lessons.md](lessons.md) read the methodology through that lens — programmatic typed scenes instead of HTML strings, an offscreen `wgpu` texture instead of a browser window, `reftest!(match/mismatch, …)` as the manifest-as-code analogue. The evidence files ([methodology.md](methodology.md), [gecko-reftests.md](gecko-reftests.md), [fuzzy-matching.md](fuzzy-matching.md), [wpt.md](wpt.md), [consumers.md](consumers.md)) describe the systems on their own terms; Buiy implications are confined to clearly-labelled subsections and to [lessons.md](lessons.md).

## Sources

- MDN, "Creating reftest-based unit tests" — https://developer.mozilla.org/en-US/docs/Mozilla/QA/Reftest (mirror: https://devdoc.net/web/developer.mozilla.org/en-US/docs/Creating_reftest-based_unit_tests.html)
- Firefox Source Docs, Reftest — https://firefox-source-docs.mozilla.org/layout/Reftest.html
- CSSWG wiki, test/reftest — https://wiki.csswg.org/test/reftest
- web-platform-tests, writing reftests — https://web-platform-tests.org/writing-tests/reftests.html
- web-platform-tests repo — https://github.com/web-platform-tests/wpt
- gecko-dev `layout/tools/reftest` — https://github.com/mozilla/gecko-dev/tree/master/layout/tools/reftest
- Sibling files: [methodology.md](methodology.md), [gecko-reftests.md](gecko-reftests.md), [fuzzy-matching.md](fuzzy-matching.md), [wpt.md](wpt.md), [consumers.md](consumers.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md), [glossary.md](glossary.md)
- Sibling prior art: [../blink/](../blink/), [../servo-stylo/](../servo-stylo/), [../taffy/](../taffy/), [../xilem-masonry/](../xilem-masonry/)
- Buiy strategy report: [../../reports/2026-06-14-visual-bug-detection-strategy.md](../../reports/2026-06-14-visual-bug-detection-strategy.md)
