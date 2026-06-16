**Date:** 2026-06-14
**Status:** active
**Subject:** web-platform-tests — the cross-vendor reference-comparison corpus, `rel=match`/`rel=mismatch`, the `wpt` runner, and the two-way vendor sync

# web-platform-tests (WPT)

web-platform-tests is "a cross-browser test suite for the Web-platform stack" whose stated purpose is to let browsers "ship software that is compatible with other implementations" (WPT repo). It is a *methodology + shared corpus*, not a product: a single Git repo of tests that Chromium, Gecko, WebKit, and Servo all run against their own engines. Reftests are the rendering-oriented subset and the direct ancestor of Buiy's reftests-first tier.

## The reftest mechanism: `rel=match` / `rel=mismatch`

A reftest is a *pair* of pages — a **test** (uses the feature under test) and a **reference** (renders the same visual result by simpler/already-trusted means). The test links to its reference via a `link` element:

- `<link rel=match href=references/green-box-ref.html>` — passes only if test and reference render **pixel-for-pixel identically** within an "800×600 window *including* scroll-bars if present."
- `<link rel=mismatch href=...>` — passes only if they do **not** render identically (used to prove a property actually *changes* rendering).

**Multiple references give boolean logic:** "If there are any match references, at least one must match" (OR / alternates); "If there are any mismatch references, all must mismatch" (AND). References can be chained and shared across many tests — sharing is "strongly encouraged" because it "makes it easier to tell at a glance whether a test passes and enables some optimizations in automated test runners."

**Reference discovery is explicit, not filename-based.** WPT links a test to its reference only through the in-markup `<link rel=match>` / `rel=mismatch` element (and the generated `MANIFEST.json` index built from it) — verified against the writing-reftests doc, which describes no automatic `*-ref.html` / `-expected` filename convention. Gecko is the same: each `reftest.list` line names both URLs explicitly. The filename convention is a *Blink* divergence — Chromium pairs `foo.html` with `foo-expected.html` (or `foo-expected-mismatch.html`) by name — covered in [consumers.md](consumers.md). A Buiy `reftest!` naming scheme can pick either model deliberately; the explicit-pairing model is what WPT/Gecko use.

## The key property: no stored golden images

A reftest stores *no* baseline screenshot. The oracle is the reference page, rendered live by the same engine on the same machine at test time. This cancels out platform font rendering, antialiasing, GPU, and DPI differences — the exact fragility that plagues golden-screenshot testing — because both halves of the comparison share those conditions. Two engines with different antialiasing both still pass the same reftest. (The shared mechanic with Gecko is the same one described in [methodology.md](methodology.md).)

## Timing and fuzzing controls

- Screenshots are taken after load + font loading + pending paints. A test marks itself async with `class="reftest-wait"` on the root element; the harness fires a `TestRendered` event and waits for the class to be *removed* before capturing — the explicit "the scene is settled, capture now" handshake.
- Bounded tolerance via `<meta name=fuzzy content="maxDifference=15;totalPixels=300">` — a per-channel color delta (0–255) and a count of differing pixels, both range-expressible (`content="10-15;200-300"`) and per-reference. Full semantics in [fuzzy-matching.md](fuzzy-matching.md).

**Documented wart (verbatim):** "There is no way to create a reference for underlining, since the position and thickness of the underline depends on the UA, the font, and/or the platform" — i.e. reftests fail wherever a feature's rendering is *intentionally* UA-defined and so cannot be reproduced by independent markup. See [open-problems.md](open-problems.md) for the full untestable category.

## Runner, manifest, and scale

The `wpt` CLI drives everything (WPT repo): `wpt serve` (HTTP server), `wpt run` (execute in a browser), `wpt lint`, and `wpt manifest` (generates `MANIFEST.json`, the index that classifies each file as testharness/reftest/wdspec and records reftest match/mismatch relationships so the runner knows what to compare). Invocation is `./wpt run [browsername] [tests]`, e.g. `./wpt run chrome dom/historical.html` or `./wpt run --binary ~/local/firefox/firefox firefox ...`; `--test-type=reftest` filters to reftests. Chrome, Edge, and Servo are supported by default per Servo's integration notes.

**Scale (verify-flagged — counts drift; cite with date):** the corpus is "over 52000 tests and nearly two million subtests" per Servo's 2023-07-20 post. A later wpt.fyi figure cites "56,552 tests … 1.8 million subtests (as of December 2024)" — *secondary, from search aggregation; could not confirm against a primary wpt.fyi page; treat as approximate*. Either way: thousands of CSS reftests, **zero stored golden screenshots**.

## How cross-vendor sharing actually works (two-way sync)

The corpus is shared by *bidirectional automation*, not manual copying:

- **Chromium:** maintains "a 2-way import/export process with the upstream web-platform-tests repository, where tests are imported into `web_tests/external/wpt`" (full path `third_party/blink/web_tests/external/wpt`). An LUCI "wpt-importer builder" auto-imports to "track tip-of-tree … as closely as possible"; editing files under `external/wpt` makes the exporter "create a provisional pull request … in the upstream WPT GitHub repository" that auto-merges. Rationale (verbatim): "leveraging and contributing to a shared test suite is one of the most important tools in achieving interoperability."
- **Gecko:** Mozilla's `wpt-sync` service does "Synchronize changes between gecko and web-platform-tests" — two-way sync between the Gecko monorepo and upstream WPT.
- **Servo:** "all changes to Servo's in-tree Web Platform Tests will be upstreamed automatically when your PR is merged"; run `./mach update-manifest` after editing a test/reference.
- **WebKit:** participates via its own `LayoutTests/imported/w3c/web-platform-tests` import path. *Not confirmed against a primary WebKit doc in this pass — flagged.*

**Net effect:** one CSS reftest, authored once, becomes a conformance assertion every engine checks against itself — the engines disagree on antialiasing yet agree on the reftest. That is precisely the leverage Buiy's reftests-first tier buys: a rendering oracle that survives platform noise without a golden-image baseline to maintain.

> *Verification flag:* `rel=match`/`rel=mismatch` syntax, fuzzy/`reftest-wait` semantics, the underline wart, and all sync-mechanism quotes are from primary sources (cited below). Test/subtest *counts* are time-stamped estimates that drift; the Dec-2024 figure is secondary and unconfirmed; WebKit's exact sync path is unconfirmed.

## Implications for Buiy

WPT is the proof at scale that a relational rendering oracle needs **zero stored screenshots** — the v1 case for Buiy's reftests-first bet. Two mechanics port directly: (1) the **multiple-reference boolean logic** (≥1 `==` must match; *all* `!=` must mismatch) is the right shape for Buiy supporting several independent references where one disjoint reference cannot fully isolate a feature; (2) the **`reftest-wait` settle handshake** maps onto Buiy's deterministic "scene settled — 0 pending assets, atlas warmed, clock advanced" capture gate before texture readback. The corpus-sharing/sync machinery is *not* directly relevant (Buiy authors its own reftests in Rust), but the consumer-side model — a parallel engine treating the corpus as an external fixture with its own pass-state — is, and is covered in [consumers.md](consumers.md).

## Sources

- web-platform-tests, writing reftests — https://web-platform-tests.org/writing-tests/reftests.html
- web-platform-tests repo — https://github.com/web-platform-tests/wpt
- WPT running tests from a local system — https://web-platform-tests.org/running-tests/from-local-system.html
- Chromium web_platform_tests docs (2-way import/export) — https://chromium.googlesource.com/chromium/src/+/main/docs/testing/web_platform_tests.md
- mozilla/wpt-sync — https://github.com/mozilla/wpt-sync
- Servo WPT blog (2023-07-20; corpus scale) — https://servo.org/blog/2023/07/20/servo-web-platform-tests/
- Sibling files: [methodology.md](methodology.md), [fuzzy-matching.md](fuzzy-matching.md), [consumers.md](consumers.md), [open-problems.md](open-problems.md), [lessons.md](lessons.md)
