**Date:** 2026-06-14
**Status:** active
**Subject:** Reftests — the consult-this-when-designing decision file: Validates / Avoid / Borrow for Buiy's Tier-4 reftest harness

# Lessons for Buiy

This is the decision file. The other files in this folder are evidence; this one is decisions. Reftests are the **Tier-4 headline mechanism** of Buiy's reftests-first visual-bug-detection strategy and "the single highest-leverage absence" in the current tree — there are no reftests anywhere yet. The lessons here are written for the author of the Buiy reftest harness (`reftest!(match/mismatch, …)` on the offscreen `wgpu` capture path).

## Top of file: the single most important finding

**A reftest asserts a relationship between two renderings of *different source files* by the *same engine in the same run* — and that one mechanic gives Buiy a rendering oracle that survives platform noise with zero stored baselines.** Both halves share the identical `wgpu::Device`, driver, glyph rasterizer, AA, DPI, and clock, so every platform-variance term cancels in the diff. WPT proves this at scale: thousands of CSS reftests, zero stored screenshots. The reference is an *independent oracle* reached "by a different route"; the test passes only if the engine produced the *right* pixels two different ways — not merely that two runs of the same buggy code agree.

The whole bet rests on one discipline: **the reference must not use the feature under test.** Get that right and reftests carry **lower maintenance than per-platform goldens** — every new CSS-subset feature ships one reference pairing whose only upkeep is "keep two equivalent scenes equivalent": no per-platform golden, no rebaseline on theme tweaks, no binary blobs, no eyeball review. (The strategy report argues this is *sub-linear* growth as the feature surface grows; that is Buiy's design rationale, not a measured browser figure.) Get it wrong and the test passes vacuously. That discipline is Open Question #1 and the load-bearing risk.

## Why it ports to Buiy cleanly

Nothing about reftests is HTML-specific. Buiy already renders to an offscreen `wgpu` texture, so it gets the methodology's core guarantees with two *structural advantages* over the browsers:

1. **One app run cancels platform variance — for free.** Buiy renders both `test_scene` and `ref_scene` to texture in a single process against the same `wgpu::Device`, so the same-engine-cancels-variance guarantee from the "Top of file" finding holds by construction. Driver-dependent SDF rounding, glyph-atlas AA, and subpixel coverage appear *identically* in both images, so `==` can often be **exact**, not fuzzy — a stronger guarantee than golden screenshots, which must survive driver upgrades.
2. **Typed scenes, not HTML strings.** `test_scene` / `ref_scene` are programmatic widget trees (or BSN assets). The reference's disjoint code path is trivial to construct because Buiy has a **primitive layer** (literal-positioned boxes) that bypasses Taffy and the CSS-subset entirely. The manifest is ordinary Rust — `reftest!(match, "flex_justify_end", test_scene, ref_scene)` — not a `reftest.list` text file.

## Validates

Buiy design choices the reftest experience confirms:

- **Reftests-first / goldens-last is industry rule, not novelty.** Blink states it outright — "You should only write a pixel test if you cannot use a reference test" — for the identical reason Buiy's pyramid puts Tier-4 above Tier-5: pixel rendering depends on the GPU/driver/text-system. Cite Blink's rule when defending the ordering. The quote and its sourcing live in *this* folder's [consumers.md](consumers.md) (taken from the Chromium "Writing Web Tests" doc); the sibling `../blink/` folder does **not** yet carry a reftest/pixel-test facet (the strategy report flags adding one), so do not chase the rule there.
- **No stored baseline is the right v1 stance.** WPT runs thousands of CSS reftests with zero screenshots; the oracle is a live reference, not a frozen artifact that reds the suite on every legitimate restyle. Buiy's Tier-4 storing **zero bytes** is the same architecture, validated at browser scale. See [wpt.md](wpt.md).
- **The two-axis fuzzy metric is the correct tolerance model.** Gecko and WPT independently converged on `(maxDifference, totalPixels)` — separating "how wrong per pixel" from "how many pixels wrong" because a single scalar cannot tell benign AA smoothing from a real one-box-misplaced regression. Buiy's planned `(max_pixel_delta, max_diff_pixels)` gate is the same model. Do not reinvent it. See [fuzzy-matching.md](fuzzy-matching.md).
- **`!=` (mismatch) for proving suppression is the right tool.** Asserting `content-visibility: hidden` `!=` the visible render guards against a silent no-op, where a `==` would pass vacuously on blank-vs-blank. Buiy's `!=` anti-tests for cull/skip behavior are the canonical use. See [methodology.md](methodology.md).
- **Reference independence as a first-class concern.** Both the browsers and the strategy report treat "the reference must use a disjoint code path" as load-bearing, not incidental. Buiy elevating it to an Open Question (who reviews it; can it be lint-enforced) matches the browsers' hard-won caution. See [open-problems.md](open-problems.md).

## Avoid

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Writing a reference with the feature under test.** A flex reference using flex, an `@container` reference using `@container`: a shared bug renders both identically wrong and the test passes vacuously — the symmetric twin of the golden weakness. | [open-problems.md](open-problems.md), CSSWG wiki | Route references through the **primitive/absolute layer** or a second independent style mechanism. Where one disjoint reference is impossible, support **multiple references** (≥1 `==` must match) so two techniques must agree. Lint-enforce where possible (Open Question #1). |
| **A `fuzzy` range that includes 0 when a difference is expected.** It silently swallows the signal that the bug was fixed (no unexpected-pass report → the stale budget never gets retired) and re-admits a regression that lands back in the window. | [fuzzy-matching.md](fuzzy-matching.md) | Pin **both ends** (`fuzzy(1-1,8-8)`, never `0-…`) for deterministic cases; widen to `lo-hi` only as far as measured run-to-run variance demands. Accept the wart that intermittent tests must drop to 0-inclusive — and treat that as pressure to engineer determinism at the source. |
| **Reaching for goldens before the reftest is exhausted.** Goldens cost a stored corpus, per-config baselines, flake budget, and human triage; reftests need none of that. | [consumers.md](consumers.md), Blink rule | Demote goldens to the **irreducible residue** (no feature-free reference exists): shadow falloff, glyph fidelity, color-emoji, blend math, gamma. Everything relational stays in Tier-4. |
| **Trying to reftest the unreftestable.** Underline position/thickness, dotted/dashed/ridge/groove/double borders, focus-ring geometry, font-metric-dependent rendering — no feature-free reference can reproduce them. | [open-problems.md](open-problems.md), CSSWG wiki, WPT #7676 | Route font-metric/UA-defined effects to **Tiers 1–3** (shaping snapshots, structured display-list snapshots, property invariants — assertions on structured data, not pixels) and the genuine rasterization residue to **Tier-5**. The pyramid is the answer; do not force a reftest. |
| **Cross-backend reftest pairings.** A test on Vulkan vs a reference on Metal reintroduces exactly the variance reftests cancel. | [open-problems.md](open-problems.md) | Keep both captures on the **same `wgpu` backend in the same process**. Get cross-platform confidence by running the whole suite on each *pinned* backend independently, not by cross-backend `==`. Verify the per-CI-lane backend assumption before claiming exact-match CI. |
| **Capturing before the scene has settled.** WPT waits for `reftest-wait` to clear (load + fonts + pending paints) before the screenshot; capturing early diffs a half-rendered frame. | [wpt.md](wpt.md) | Gate the texture readback on Buiy's deterministic settle condition — 0 pending assets, glyph atlas warmed, clock advanced (the `GoldenConfig::deterministic()` triad already built) — the analogue of `reftest-wait`. |
| **Assuming chained/multiple references are universally available.** Blink supports *neither* multiple nor chained references; only Gecko/WPT do. | [consumers.md](consumers.md) | If Buiy adopts multiple references as the independence mitigation, build that capability into the `reftest!` harness deliberately — it is not free, and one major engine omits it. |

## Borrow

Concrete reftest primitives worth studying before building the Buiy analogue:

1. **The `==` / `!=` operator pair and its boolean aggregation.** `==`/`rel=match` (pass if SAME), `!=`/`rel=mismatch` (pass if DIFFERENT); with multiple refs, ≥1 `==` must match (OR), *all* `!=` must mismatch (AND). Buiy's `reftest!(match/mismatch, …)` is the typed analogue; the aggregation rules are the spec for multi-reference independence. See [methodology.md](methodology.md), [wpt.md](wpt.md).

2. **The two-axis fuzzy budget, exact.** `fuzzy(minDiff-maxDiff, minPixelCount-maxPixelCount)` (Gecko) / `<meta name=fuzzy content="maxDifference=10-15;totalPixels=200-300">` (WPT), ranges inclusive, **per-reference**. Copy the metric and the per-pairing scoping onto the `RefCase`; for `!=` the minimum bounds must be 0. This is the same metric the strategy report's unified perceptual gate targets. See [fuzzy-matching.md](fuzzy-matching.md).

3. **The pin-both-ends calibration discipline.** Measure the *actual* `(max difference, different pixels)` from a real failing run (the harness prints it), set the range tight (`n-n` when deterministic), never include 0 when a difference is expected. The payoff: a fixed bug surfaces as an *unexpected pass* that retires the budget, restoring exact-match coverage. Tooling like Gankra's `live-reftest-analyzer` makes the triage visual — Buiy's failing-run diff PNG is the analogue. See [fuzzy-matching.md](fuzzy-matching.md).

4. **The `reftest-wait` settle handshake.** Capture only after load + font loading + pending paints, signalled by removing `class="reftest-wait"`. Buiy's analogue is an asserted "scene settled" gate (0 pending assets, atlas warmed, clock at an explicit virtual timestamp) before texture readback — and it doubles as the animation-snapshot mechanism (capture at stepped clock times). See [wpt.md](wpt.md).

5. **The out-of-band per-engine expectation model (only if importing external corpora).** Servo stores pass/fail expectations as `.ini` metadata in a `meta` folder, treating the shared corpus as an external fixture it never edits. Buiy authors its own reftests in Rust (expectation co-located with the `#[test]`), so it needs this *only* if it imports an external corpus — e.g. Taffy's WPT-derived layout fixtures. Then store expectations out-of-band, do not mutate imported fixtures. See [consumers.md](consumers.md).

6. **Authoring patterns mapped to Buiy's CSS-subset surface** (the test/reference pairs to write):
   - *Flex/grid → literal offsets.* Test: `justify_content: SpaceBetween` row of three 40px boxes in a 200px container. Reference: three boxes at absolute x = 0, 80, 160 via the primitive layer. `==` proves the Taffy integration *and* box-generation math with a reference that never touches the flex solver.
   - *`@container` → hand-authored equivalent.* Test: a widget whose style resolves via a container query at a given container size. Reference: the same tree with the *resolved* branch inlined as a plain style, no `@container` rule. `==` proves the query engine selected and applied the right rule.
   - *`content-visibility: hidden` → mismatch.* Test: a subtree with `content-visibility: hidden`. Reference: the identical tree visible. Assert `!=` — the hidden subtree must *not* paint.
   - *Logical → physical mirror.* Logical-property layout `==` its physical-property mirror, proving writing-mode/direction resolution.
   - *Transform → translated coordinates.* `translate(50,50)` `==` an element authored at the translated coordinates.

7. **The CPU-vs-GPU cross-check as a Tier-4.5 oracle** (Buiy-specific, Vello-pattern). Where no feature-free reference exists for a *rasterization* property (SDF corner AA), Buiy's existing CPU SDF port is an independent rasterization oracle: render the same primitive on GPU and CPU in one run, diff with the two-axis metric. Stores zero bytes, needs no second authoring path (the CPU port *is* the independent implementation), and catches AA bugs no markup-style reftest can. Build it before broad goldens. See [open-problems.md](open-problems.md).

## How to use this file

- **Buiy reftest-harness author:** read the "Top of file" finding and Borrow items 1–4 and 7, then the Avoid rows on reference-with-feature-under-test, 0-inclusive fuzz, cross-backend pairing, and capture-before-settle. Each maps to a concrete `reftest!` / `RefCase` / capture-gate decision. The two open questions the evidence forces the plan to close: (a) **who reviews / how to lint reference independence** (Open Question #1) and (b) **whether Buiy pins both ends of every fuzz budget** (Open Question #2).
- **Anyone defending the reftests-first ordering:** the Validates list is the set of Buiy choices the browsers confirm — cite Blink's "only write a pixel test if you cannot use a reference test" and WPT's zero-baseline scale.
- **Anyone scoping a feature for visual testing:** the Avoid row "trying to reftest the unreftestable" + [open-problems.md](open-problems.md) tell you when an effect *cannot* be a reftest and which tier owns it instead.
- The other files are evidence; do not re-derive their detail here — follow the cross-links.

## Sources

- Sibling files: [README.md](README.md), [methodology.md](methodology.md), [gecko-reftests.md](gecko-reftests.md), [fuzzy-matching.md](fuzzy-matching.md), [wpt.md](wpt.md), [consumers.md](consumers.md), [open-problems.md](open-problems.md), [glossary.md](glossary.md)
- CSSWG wiki, test/reftest (reference-must-differ; vacuous-pass; impossible-to-reftest) — https://wiki.csswg.org/test/reftest
- Firefox Source Docs, Reftest (operators, fuzzy, pin-both-ends) — https://firefox-source-docs.mozilla.org/layout/Reftest.html
- web-platform-tests, writing reftests (rel=match/mismatch, fuzzy, reftest-wait, underline wart) — https://web-platform-tests.org/writing-tests/reftests.html
- Blink — Writing Web Tests ("only write a pixel test if you cannot use a reference test") — https://chromium.googlesource.com/chromium/src/+/main/docs/testing/writing_web_tests.md
- Servo Book — Testing (out-of-band `.ini` expectations) — https://book.servo.org/contributing/testing.html
- Sibling prior art: [../blink/lessons.md](../blink/lessons.md), [../servo-stylo/](../servo-stylo/), [../taffy/lessons.md](../taffy/lessons.md), [../xilem-masonry/masonry-toolkit.md](../xilem-masonry/masonry-toolkit.md)
- Buiy strategy report (Tier-4 reftests, Open Questions #1/#2, CPU-vs-GPU cross-check) — [../../reports/2026-06-14-visual-bug-detection-strategy.md](../../reports/2026-06-14-visual-bug-detection-strategy.md)
