**Date:** 2026-06-14
**Status:** active
**Subject:** What the visual-golden storage/triage ecosystem structurally does NOT solve

# Open problems

What Gold, reg-suit, and the SaaS triad **structurally do not solve**, no matter how well-operated. These are the limits Buiy should not expect a Tier-5 golden harness to overcome.

## 1. The oracle problem is not solved — only deferred to a human

None of these tools know what *correct* looks like. A golden/expectation only encodes "this matched a human-approved image at some point." The first time a test runs, every image is `untriaged` and someone must *decide*. Gold's triage UI, reg-suit's HTML report, and the SaaS review screens all make the human decision faster, but none remove it. A regression that a human approves by mistake becomes the new truth. This is why Buiy's strategy pushes detection *down* to Tiers 1–4 (structured snapshots, metamorphic invariants, reftests), which assert *relations* and need no per-image human oracle. Gold is the tier where the oracle problem is irreducible.

## 2. Stale positives accumulate silently

Gold's multi-positive model (a test passes if it matches *any* approved digest) has no garbage collection: an approved digest that no live config produces anymore stays approved forever. Over time the positive set drifts from a curated "these are the acceptable renders" into "everything anyone ever blessed." A real regression that happens to match an old, now-wrong positive passes silently. Nothing in the documented Gold workflow prunes this; it is operational debt the gardener must manage by hand.

## 3. Flake is mitigated, not eliminated — and the mitigations require a standing human role

Gold's flaky answer is **inexact matching (Fuzzy/Sobel) + manual time-boxed ignores**, both owned by a gardener. There is **no documented automatic flaky-auto-quarantine in Gold** (unlike Argos's "minimum occurrences in 7 days → auto-ignore"). Inexact thresholds must be tuned per test (Gold ships `determine_gold_inexact_parameters.py` precisely because hand-picking fails), and ignores expire on an hours scale, so a chronically-flaky config needs repeated human attention. The flake never goes away; it is held at bay by ongoing labor. *Unverified: exact post-expiry re-activation semantics of time-boxed ignores are not specified in primary docs.*

## 4. Cross-machine pixel reproducibility is out of scope

Gold's entire params/traces design is an admission that **the same scene rasterizes differently across OS / GPU / driver / AA setting**, and it copes by treating each config as a separate trace plus allowing multiple positives. It does not make the pixels reproducible — it *catalogs* the irreproducibility. For Buiy, whose least-deterministic artifact is the pixel (FP non-associativity, FMA contraction, `fwidth` derivatives, sRGB encode on GPU write — see [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md)), this means a Gold-style harness inherits a baseline-per-backend×dpr explosion that no amount of triage tooling collapses.

## 5. The cost/ops floor is high and not amortizable away

Gold is a GCS bucket + a GCE/k8s frontend + per-corpus config + a human triage queue — heavy standing infrastructure justified only at the scale of "each commit creates >500k images" ([golden README](https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md)). The self-hosted cost/ops figure for a small project is **unverified** (the Skia/Chromium backends are Google-operated). The SaaS alternatives convert capital cost into per-snapshot billing that "scales significantly with volume" (Percy ≈ $5,000/mo for a 100k-screenshot team per a secondary, **unverified** comparison). Either way the golden tier carries a cost floor that the cheaper pyramid tiers do not.

## 6. Commit-key resolution is the part that breaks in the OSS reference design

reg-suit's `reg-keygen-git-hash-plugin` walks the branch graph to find "the parent's commit which is the source of the topic branch" and special-cases merge commits, but the README is thin on rebased branches, squash-merges, and multi-parent histories. When the keygen picks the wrong parent, the "expected" baseline is wrong and every comparison is noise. There is no durable accept ledger to fall back on (acceptance is implicit in git history), so a mis-resolved key has no second source of truth. This is the concrete fragility Buiy inherits if it copies the commit-keyed-store pattern naively.

## 7. Structural diff still misses semantic intent

Even reg-suit's structural x-img-diff (matched/changed/unmatched keypoints) and Argos's clustering only describe *where pixels moved*, not *whether the change was intended*. "The button is now blue" and "the button regressed to blue" produce identical diffs. The tools surface the change; only the pyramid's higher tiers (token-set snapshots, reftests asserting `==`/`!=` relations) can encode *intent* without a human in the loop.

## Implications for Buiy

These seven limits are the case *for* the strategy report's pyramid: every problem here is irreducible at the golden tier and tractable at a cheaper one. Buiy should (a) keep Tier 5 a minimal residue, (b) never expect a golden harness to solve the oracle/flake/reproducibility problems, and (c) if it does build the residue, copy the *concepts* (params/traces, multi-positive, tunable inexact match, expiring ignores) without the standing service. See [lessons.md](lessons.md).

## Sources

- skia-buildbot golden README: https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md
- Skia Gold docs: https://skia.org/docs/dev/testing/skiagold/
- Chromium GPU Pixel Testing With Gold: https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md
- reg-keygen-git-hash-plugin: https://github.com/reg-viz/reg-suit/blob/master/packages/reg-keygen-git-hash-plugin/README.md
- vizzly comparison (secondary, unverified $): https://vizzly.dev/visual-testing-tools-comparison/
- Buiy visual-bug-detection strategy: [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md)
