**Date:** 2026-06-14
**Status:** active
**Subject:** Gecko/Mozilla reftest mechanics — the `reftest.list` manifest, operators, reference chains, failure-type annotations, and the harness

# Gecko reftests

Gecko (Firefox) is where the reftest methodology originated and where its concrete mechanics — the manifest format, the operator/annotation vocabulary, and the runner — are most precisely documented. The methodology and the `==` / `!=` semantics are shared with WPT, Servo, and Blink; this file is the Gecko-specific machinery.

## Pass/fail computation

The harness renders both inputs and compares the resulting bitmaps. A test passes when "the bitmaps resulting from displaying the two files in an 800×1000 window are identical" (MDN). Firefox Source Docs confirms the fixed viewport: "The captures of the tests are taken in a viewport that is 800 pixels wide and 1000 pixels tall, so any content outside that area will be ignored." Comparison is **exact pixel match by default**; every pixel must be identical unless a `fuzzy` annotation relaxes it (see [fuzzy-matching.md](fuzzy-matching.md)).

## Operators

- `==` (match) — passes "if the images of the two renderings are the SAME."
- `!=` (mismatch) — passes "if the images of the two renderings are DIFFERENT."

`!=` is the workhorse for catching *regressions to nothing* — asserting a feature actually produces a visible effect, so a no-op implementation fails. (Full operator semantics and the `!=`-proves-suppression pattern are in [methodology.md](methodology.md).)

## Manifest format

Tests are declared in plain-text manifests conventionally named `reftest.list`. Lines starting with `#` are comments. Each test line has the form (Firefox Source Docs):

```
[ <failure-type> | <pref> ]* [<http>] <type> <url> <url_ref>
```

- `<type>` is `==` or `!=`.
- `<url>` is the testcase; `<url_ref>` is the reference. Results are reported under `<url>` only.
- Manifests may `include` other manifests, forming a tree.

Reference linking is **explicit**: every line names both the testcase URL and the reference URL, and Gecko documents no filename-based auto-discovery (no `-ref.html` convention) — verified against Firefox Source Docs. Blink is the divergence here, pairing `foo.html` with a same-named `foo-expected.html` by convention (see [consumers.md](consumers.md)); a Buiy `reftest!` naming scheme should choose explicit-pairing-vs-filename-convention deliberately.

The in-tree `reftest.list` files live *beside the tests they reference* — the harness source directory `layout/tools/reftest/` itself contains no `reftest.list` (verified against the gecko-dev tree).

## Reference chains and multiple references

A single test line names exactly one reference, but references can be **chained**: "If multiple reference files must be matched, each reference file should, in turn, link to the next reference" (CSSWG wiki) — chaining is expressed via the manifest links between *reference* files, letting one test transitively require several relationships.

For specs permitting multiple conforming renderings, "each possible rendering should have its own reference file linked from the test file." The aggregate semantics (CSSWG wiki / WPT):

- If a test has multiple `==` references then **at least one** of those references must match the test (OR).
- If a test has multiple `!=` references, then **none** of those references may match the test (AND).

> *Verification flag:* the precise chaining link mechanism (whether via a manifest column or an in-reference annotation) is summarized from the CSSWG wiki and not cross-checked against the parser source `manifest.sys.mjs`.

## Failure-type annotations

Prefix tokens on a manifest line (Firefox Source Docs):

| Token | Effect |
|---|---|
| `fails` | expected failure — **inverts** the pass condition (the test is known-broken; a *pass* would be the surprise) |
| `random` | result is nondeterministic; excluded from output |
| `skip` | do not run — used when a test crashes or hangs the browser |
| `fuzzy(minDiff-maxDiff,minPixelCount-maxPixelCount)` | pass when per-pixel value differences fall in `[minDiff,maxDiff]` *and* the count of differing pixels falls in `[minPixelCount,maxPixelCount]`, both inclusive |

`fuzzy` is the documented escape hatch for unavoidable antialiasing/platform noise — and a known wart, since over-broad fuzz ranges silently mask real regressions. It has its own file: [fuzzy-matching.md](fuzzy-matching.md). The conditional forms `fails-if(cond,…)` / `fuzzy-if(cond,…)` scope an annotation to a platform or pref (e.g. `fuzzy-if(cocoaWidget,1-1,8-8)`); platform-conditional `sandbox` annotations are evaluated by the manifest parser's sandbox.

## Harness / runner

The runner historically lived in `reftest.jsm`; that `.jsm` form **no longer exists** — verified against the current gecko-dev tree, which contains:

- `reftest.sys.mjs` — the in-content runner.
- `manifest.sys.mjs` — manifest parser / sandbox + platform-conditional annotation evaluation.
- `runreftest.py` and `reftestcommandline.py` — the Python drivers.

The `.jsm`→ES-module `.sys.mjs` form is part of Gecko's tree-wide module-system migration (no single tracking bug is cited here — see the verification flag); reftest navigation was separately refactored onto `JSWindowActor` (Bug 1648444, resolved Firefox 83). Reftests are invoked via `mach reftest`. The same `==`/`!=` manifest methodology was adopted by web-platform-tests and is shared across Gecko, Servo, and Blink.

> *Verification flag:* file presence/absence in `layout/tools/reftest/` was verified directly against the gecko-dev master tree (`reftest.sys.mjs`, `manifest.sys.mjs`, `runreftest.py`, `reftestcommandline.py` present; no `reftest.jsm`). Bug 1648444 ("Refactor reftest navigation code to use JSWindowActor," resolved Firefox 83) was confirmed on Bugzilla. The broad `.jsm`→`.sys.mjs` migration is real but is not driven by one numbered meta-bug; an earlier draft miscited Bug 1838149 for it — that bug is in fact a narrow WebDriver logging-string fix (`ModuleCache.sys.mjs`), unrelated to reftests, and the claim has been corrected to avoid asserting a bug number that does not back it.

## Implications for Buiy

Gecko's manifest is a text file because its testcases are loose HTML files discovered by path. Buiy's "documents" are typed BSN assets or programmatic widget trees, so the manifest is **ordinary Rust** — a `reftest!(match/mismatch, "name", test_scene, ref_scene)` macro or a data-driven harness over `&[RefCase]`, each pairing a `#[test]` under the existing `xvfb-run -a cargo test` gate. The Gecko annotation vocabulary maps usefully: `fuzzy(…)` → a per-pairing two-axis budget (see [fuzzy-matching.md](fuzzy-matching.md)); `fails` → a `#[ignore]`-with-reason or an expected-failure marker; `skip` → not registering the pairing on a backend that cannot run it. The chained/multiple-reference mechanism is the precedent for Buiy supporting *multiple* references where one disjoint reference is impossible (at least one `==` must match), which is the mitigation for the reference-independence wart in [open-problems.md](open-problems.md).

## Sources

- Firefox Source Docs, Reftest — https://firefox-source-docs.mozilla.org/layout/Reftest.html
- MDN, "Creating reftest-based unit tests" — https://developer.mozilla.org/en-US/docs/Mozilla/QA/Reftest
- CSSWG wiki, test/reftest — https://wiki.csswg.org/test/reftest
- gecko-dev `layout/tools/reftest` tree — https://github.com/mozilla/gecko-dev/tree/master/layout/tools/reftest
- Bug 1648444 (reftest navigation onto `JSWindowActor`, resolved Firefox 83) — https://bugzilla.mozilla.org/show_bug.cgi?id=1648444
- Sibling files: [methodology.md](methodology.md), [fuzzy-matching.md](fuzzy-matching.md), [wpt.md](wpt.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md)
