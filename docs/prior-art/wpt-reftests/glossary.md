**Date:** 2026-06-14
**Status:** active
**Subject:** Reftest / WPT / Gecko terminology used across this folder

# Glossary

Short definitions for the reftest / web-platform-tests / Gecko terms used across this prior-art folder. Each entry points to the file where the full discussion lives.

## Core methodology

- **Reftest (reference test)** — a test that asserts a *relationship between two renderings produced by the same engine* (a testcase and a reference), not a match against a stored gold image. See [methodology.md](methodology.md).
- **Testcase** — the file that exercises the feature under test, often with complex markup. See [methodology.md](methodology.md).
- **Reference** — a different, usually simpler, file that produces the *same* rendering as the testcase *by a different mechanism*; "must not use the same features that are being tested." The independent oracle. See [methodology.md](methodology.md), [open-problems.md](open-problems.md).
- **`==` (match)** — Gecko operator; passes if the two renderings are the SAME. WPT spelling: `<link rel=match>`. See [gecko-reftests.md](gecko-reftests.md), [wpt.md](wpt.md).
- **`!=` (mismatch)** — Gecko operator; passes if the two renderings are DIFFERENT. WPT spelling: `<link rel=mismatch>`. Used to prove a feature actually changes rendering (catches no-op regressions). See [methodology.md](methodology.md).
- **Vacuous pass** — the failure mode where the reference shares the testcase's buggy code path, so both render identically wrong and the `==` passes despite the bug. The reference-independence wart. See [open-problems.md](open-problems.md).

## Manifest and annotations (Gecko)

- **`reftest.list`** — Gecko's plain-text manifest declaring reftests; `#` comments; `include` forms a manifest tree; lives beside the tests it references. See [gecko-reftests.md](gecko-reftests.md).
- **Manifest line** — `[ <failure-type> | <pref> ]* [<http>] <type> <url> <url_ref>`; `<type>` is `==`/`!=`, `<url>` the testcase, `<url_ref>` the reference; results reported under `<url>`. See [gecko-reftests.md](gecko-reftests.md).
- **Reference chain** — multiple reference files linked in turn so one test transitively requires several relationships. See [gecko-reftests.md](gecko-reftests.md).
- **`fails`** — annotation marking an expected failure; *inverts* the pass condition. See [gecko-reftests.md](gecko-reftests.md).
- **`random`** — annotation marking a nondeterministic result; excluded from output. See [gecko-reftests.md](gecko-reftests.md).
- **`skip`** — annotation; do not run (test crashes/hangs the browser). See [gecko-reftests.md](gecko-reftests.md).
- **`-if(condition,…)`** — conditional form of an annotation (`fails-if`, `fuzzy-if`) scoping it to a platform/config (e.g. `fuzzy-if(cocoaWidget,1-1,8-8)`). See [gecko-reftests.md](gecko-reftests.md), [fuzzy-matching.md](fuzzy-matching.md).

## Fuzzy matching

- **`fuzzy(minDiff-maxDiff,minPixelCount-maxPixelCount)`** — Gecko annotation; passes when the max per-channel difference is in `[minDiff,maxDiff]` *and* the differing-pixel count is in `[minPixelCount,maxPixelCount]`, both inclusive. See [fuzzy-matching.md](fuzzy-matching.md).
- **`<meta name=fuzzy>`** — WPT markup form of the same two-axis budget; `content="maxDifference=10-15;totalPixels=200-300"` (named args optional; per-reference via a `ref.html:lo-hi;lo-hi` prefix). See [fuzzy-matching.md](fuzzy-matching.md).
- **Two-axis budget** — the model splitting *maximum per-channel pixel difference* ("how wrong per pixel") from *number of differing pixels* ("how many pixels wrong"), because one scalar cannot separate benign AA from a real small-area bug. See [fuzzy-matching.md](fuzzy-matching.md).
- **Pin both ends / never include 0** — the discipline of setting tight ranges that exclude 0 when a difference is expected, so a fixed bug surfaces as an *unexpected pass*. See [fuzzy-matching.md](fuzzy-matching.md).

## Harness (Gecko)

- **`reftest.sys.mjs`** — the in-content reftest runner (ES-module form; replaced the old `reftest.jsm`). See [gecko-reftests.md](gecko-reftests.md).
- **`manifest.sys.mjs`** — the manifest parser / sandbox; evaluates platform-conditional annotations. See [gecko-reftests.md](gecko-reftests.md).
- **`runreftest.py` / `reftestcommandline.py`** — the Python drivers. Reftests are invoked via `mach reftest`. See [gecko-reftests.md](gecko-reftests.md).

## web-platform-tests

- **web-platform-tests (WPT)** — a cross-browser test suite (single Git repo) run by Chromium, Gecko, WebKit, and Servo against their own engines; reftests are its rendering-oriented subset. See [wpt.md](wpt.md).
- **`rel=match` / `rel=mismatch`** — WPT's `link`-element spelling of the `==` / `!=` operators. See [wpt.md](wpt.md).
- **`reftest-wait`** — a class on the root element that marks a test async; the harness captures only after the class is removed (post load + fonts + paints). See [wpt.md](wpt.md).
- **`MANIFEST.json`** — WPT's generated index classifying each file (testharness/reftest/wdspec) and recording reftest match/mismatch relationships. Built by `wpt manifest`. See [wpt.md](wpt.md).
- **`wpt` CLI** — drives the suite: `wpt serve`, `wpt run`, `wpt lint`, `wpt manifest`. See [wpt.md](wpt.md).
- **`MANIFEST.json` two-way sync** — the bidirectional import/export automation by which Chromium (`external/wpt`), Gecko (`wpt-sync`), and Servo upstream/downstream tests to/from the shared corpus. See [wpt.md](wpt.md).

## Consumers

- **`mach test-wpt`** — Servo's command to run the full WPT suite; subset/update variants exist (`--update-expectations`, `--manifest-update`). See [consumers.md](consumers.md).
- **`.ini` expectations** — Servo's out-of-band per-test pass/fail metadata stored in a `meta` folder, so a parallel engine tracks *its own* pass state without editing the shared tests. See [consumers.md](consumers.md).
- **`mach test-css` / `test-ref`** — historical Servo commands for CSS-WG reference tests / Servo-specific reftests; possibly superseded by the unified `test-wpt` path (currency unverified). See [consumers.md](consumers.md).
- **`run_web_tests.py`** — Blink's web-test runner; tests live in `third_party/blink/web_tests`; Blink supports neither chained nor multiple references. See [consumers.md](consumers.md).

## Sources

- Firefox Source Docs, Reftest — https://firefox-source-docs.mozilla.org/layout/Reftest.html
- web-platform-tests, writing reftests — https://web-platform-tests.org/writing-tests/reftests.html
- CSSWG wiki, test/reftest — https://wiki.csswg.org/test/reftest
- Servo Book — Testing — https://book.servo.org/contributing/testing.html
- Blink — Writing Web Tests — https://chromium.googlesource.com/chromium/src/+/main/docs/testing/writing_web_tests.md
- Sibling files: [methodology.md](methodology.md), [gecko-reftests.md](gecko-reftests.md), [fuzzy-matching.md](fuzzy-matching.md), [wpt.md](wpt.md), [consumers.md](consumers.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md)
