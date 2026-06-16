**Date:** 2026-06-14
**Status:** active
**Subject:** How Servo and Blink consume the reftest corpus — out-of-band expectations, the per-engine pass-state model, and the reftest-first / golden-last ordering

# Consumers: Servo and Blink

The same `==`/`rel=match` methodology is run by multiple engines against their own code. How a *parallel* engine consumes a shared reftest corpus — and how it ranks reftests against golden tests — is the most directly transferable architecture for Buiy, which is itself a from-scratch engine designing its first reftest tier.

## Servo: reftests as the primary CSS signal for a parallel Rust engine

Servo — the closest prior art, a parallel Rust engine — consumes the W3C reftest corpus through its `mach` Python driver. The current Servo Book lists `./mach test-wpt` as the one command for the full WPT suite ("The simplest way to run the Web Platform Tests in Servo is `./mach test-wpt`"), with subset invocations like `./mach test-wpt dom` or `./mach test-wpt tests/wpt/yourtest`. The corpus lives under `tests/wpt`:

- `tests/wpt/tests` — the upstream cross-browser tests (including CSS-WG reftests).
- `tests/wpt/mozilla/tests` — Servo-only tests that depend on Servo features.
- `tests/wpt/webgl` / `tests/wpt/webgpu` — imported suites.

**The key architectural takeaway: expectations are stored out-of-band, not in the test files.** Pass/fail expectations are not asserted in the test files but stored as `.ini` metadata under a `meta` folder, refreshed via `./mach test-wpt --update-expectations path/to/tests/` and `./mach test-wpt --manifest-update`; CI imports are pulled with `./mach update-wpt <actions-run-url>`. A parallel engine treats the shared reftest corpus as an **external fixture** and tracks *its own* per-test pass state separately, rather than forking the tests. Aggregate CSS-vs-WPT health is tracked publicly at servo.org/wpt (scored as "percentages of total **enabled** tests… that pass," subtest tests scored 0–1 by passing fraction), drilling into wpt.fyi/results/?product=servo.

**Historical `test-css` / `test-ref` (currency caveat).** The CSS-specific entry point was historically `./mach test-css`, documented as running "the cross-browser CSS WG reference tests… intended to work across many browsers," alongside `./mach test-ref` for Servo-specific reftests. **Could not fully verify currency:** these appear in older wiki/blog material (2015 era); the present-day Servo Book testing page surfaces only `test-wpt`, `test-unit`, `test-tidy`, `test-devtools`, suggesting CSS reftests were folded into the unified WPT path. Treat `test-css` as historically real but possibly superseded. Servo's reftest manifests use `.list` files with `==` (must-match) and `!=` (must-not-match) operators (e.g. `test/ref/basic.list`).

## Blink: reftests in `web_tests`, with goldens as last resort

Chromium runs the suite via `third_party/blink/tools/run_web_tests.py`; tests live in `third_party/blink/web_tests`. Blink documents three tiers and **explicitly ranks reftests above pixel/golden tests**:

> "Reference tests, also known as reftests, perform a pixel-by-pixel comparison between the rendered image of a test page and the rendered image of a reference page."

And the controlling rule:

> "You should only write a pixel test if you cannot use a reference test."

— because pixel tests are "less robust… because the rendering of a page is influenced by many factors such as the host computer's graphics card and driver, the platform's text rendering system, and various user-configurable operating system settings."

Blink's reference linking: test `foo.html` pairs with `foo-expected.html` via `<link rel="match">`, or `foo-expected-mismatch.html` via `<link rel="mismatch">`. Notably, **"Multiple references and chained references are not supported"** in Blink — a divergence from Gecko/WPT, which do support them. This "reftest-first, golden-only-if-forced" ordering is exactly Buiy's pyramid.

## Implications for Buiy

Two patterns transfer with high confidence:

1. **The reftest-first / golden-last ordering is industry-validated, not novel.** Blink states it as a *rule* ("only write a pixel test if you cannot use a reference test"); Buiy's pyramid places Tier-4 reftests above Tier-5 goldens for the identical reason (rendering depends on GPU/driver/text-system). Cite Blink's rule when defending the ordering.
2. **Out-of-band per-engine pass-state is the right model for a parallel engine — but Buiy mostly sidesteps it.** Servo's `.ini`-in-`meta` expectation model exists because Servo runs *someone else's* tests and must track which it currently fails without editing them. Buiy authors its *own* reftests in Rust, so the test and the expected result co-locate (a `#[test]` either passes or is `#[ignore]`-with-reason). Buiy only needs the Servo model if it ever imports an *external* corpus (e.g. Taffy's WPT-derived layout fixtures, already noted as reusable in the strategy report) — at which point store expectations out-of-band rather than mutating imported fixtures.

The currency caveat on `test-css`/`test-ref` is a reminder that command surfaces drift; cite the *mechanism* (out-of-band expectations, reftest-first ordering), not the exact subcommand.

## Sources

- Servo Book — Testing — https://book.servo.org/contributing/testing.html
- servo/servo `tests/wpt` — https://github.com/servo/servo/tree/main/tests/wpt
- Servo wiki — Testing (historical `test-css`/`test-ref`) — https://github.com/servo/servo/wiki/Testing
- Servo environment blog (2015) — https://servo.org/blog/2015/07/22/environment/
- Servo WPT pass rates — https://servo.org/wpt/ ; https://wpt.fyi/results/?product=servo
- Blink — Writing Web Tests (reftest-first rule, no chained refs) — https://chromium.googlesource.com/chromium/src/+/main/docs/testing/writing_web_tests.md
- Sibling files: [wpt.md](wpt.md), [methodology.md](methodology.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md)
- Sibling prior art: [../servo-stylo/](../servo-stylo/), [../blink/](../blink/)
