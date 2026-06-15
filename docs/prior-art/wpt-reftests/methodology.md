**Date:** 2026-06-14
**Status:** active
**Subject:** Reftests — the core idea: assert a relationship between two same-engine renderings, not a stored baseline

# The reftest methodology

A reftest asserts a *relationship between two renderings produced by the same engine*, not a match against a stored gold image. The harness renders two input files — a **testcase** and a **reference** — captures the resulting bitmaps, and compares them pixel-for-pixel. There are two operators: a *match* assertion that passes if the two renderings are the SAME, and a *mismatch* assertion that passes if they are DIFFERENT.

## Why relationship, not baseline

The canonical motivation (MDN, "Creating reftest-based unit tests"): traditional automated testing "compares output against an invariant 'gold standard,'" but a standards-compliant engine may *legitimately* change a rendering — MDN's example is changing the indentation depth of a `blockquote` — so an invariant baseline produces **false failures and an untrustworthy harness**. Reftests sidestep this. The often-quoted statement of the principle (MDN): "The power of the tool comes from the fact that there is more than one way to achieve any given visual effect in a browser."

The testcase typically exercises the feature under test with complex markup; the reference "uses a different method to produce the same rendering" (CSSWG wiki). Because both files are rendered by the engine under test *in the same run*, anything the engine does consistently — font rendering, antialiasing, subpixel layout, GPU/driver rounding — cancels out of the comparison. You are testing an *internal engine invariant* ("these two equivalent inputs must agree"), not a frozen artifact.

> *Verification flag:* the "blockquote indentation" example and the exact "gold standard" phrasing are quoted from the MDN / devdoc mirror, not re-confirmed verbatim against current firefox-source-docs prose (the page exists but rewording is possible).

## The decisive property: the reference reaches the same pixels by a *different route*

The reference is "a different, usually simpler, file that results in the same rendering as the test. **The reference file must not use the same features that are being tested**" (CSSWG wiki). Gecko states the reference should be created "using a different mechanism than the test." This is the whole game: a reftest validates *what the engine should produce*, not *that two runs of the same buggy code agree*. (The corollary failure mode — a reference that shares the test's buggy code path and so renders identically wrong — is the reference-independence wart; see [open-problems.md](open-problems.md).)

## The two operators

| Operator | Gecko | WPT | Passes when |
|---|---|---|---|
| match | `==` | `<link rel=match>` | the two renderings are **identical** (within fuzz, if any) |
| mismatch | `!=` | `<link rel=mismatch>` | the two renderings are **not identical** |

**`==` (match)** is the workhorse for conformance: render the feature one way, render the same intended pixels a second way that does not use the feature, assert equality.

**`!=` (mismatch)** is the workhorse for catching *regressions to nothing* — asserting that a feature actually produces a visible effect, so a no-op implementation fails. The canonical use: prove suppression. A subtree with `content-visibility: hidden` must `!=` the same subtree rendered visible; if the implementation forgets to suppress paint, the two render identically and the `!=` fails. An exact-match assertion would be vacuous here — a blank-vs-blank bug passes a `==`, so you assert *difference* instead.

## Exact match by default

Comparison is **exact pixel match by default** — every pixel must be identical unless a fuzzy annotation relaxes it (see [fuzzy-matching.md](fuzzy-matching.md)). The capture happens in a fixed viewport — 800×1000 in Gecko (content outside is ignored), 800×600 including scrollbars in WPT — so anything outside that area does not participate in the comparison.

## In-process: both renderings, same engine, same run

The non-negotiable mechanic is that both the testcase and the reference are rendered by the *same engine build on the same machine in the same session*. That is precisely why GPU/driver/font-rasterizer variance is identical on both sides and cancels in the diff. Two different engines with different antialiasing both still pass the same reftest — which is what lets one CSS reftest, authored once, become a conformance assertion every engine checks against *itself*.

## Implications for Buiy

Nothing about reftests is HTML-specific; the methodology is renderer-agnostic. Buiy already renders to an offscreen `wgpu` texture, so it gets the same-engine-cancels-variance guarantee *for free* by rendering both `test_scene` and `ref_scene` to texture in one process against the same `wgpu::Device`: driver-dependent SDF rounding, glyph-atlas antialiasing, and subpixel coverage appear *identically* in both images, so `==` can often be **exact** rather than fuzzy — a stronger guarantee than golden screenshots, which must survive driver upgrades. The Buiy analogue of `<link rel=match>` / `== test ref` is a `reftest!(match, "name", test_scene, ref_scene)` macro: the manifest is ordinary Rust, not a `reftest.list` text file. The full authoring patterns and the load-bearing reference-independence discipline live in [lessons.md](lessons.md).

## Sources

- MDN, "Creating reftest-based unit tests" — https://developer.mozilla.org/en-US/docs/Mozilla/QA/Reftest (mirror: https://devdoc.net/web/developer.mozilla.org/en-US/docs/Creating_reftest-based_unit_tests.html)
- CSSWG wiki, test/reftest — https://wiki.csswg.org/test/reftest
- Firefox Source Docs, Reftest — https://firefox-source-docs.mozilla.org/layout/Reftest.html
- web-platform-tests, writing reftests — https://web-platform-tests.org/writing-tests/reftests.html
- Sibling files: [gecko-reftests.md](gecko-reftests.md), [wpt.md](wpt.md), [fuzzy-matching.md](fuzzy-matching.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md)
