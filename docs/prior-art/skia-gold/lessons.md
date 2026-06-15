**Date:** 2026-06-14
**Status:** active
**Subject:** Lessons for Buiy — the storage+triage escape hatch: which choices it validates, which traps to avoid, which primitives to borrow

# Lessons for Buiy

This is the consult-this-when-designing file. The other files are evidence; this file is decisions. Scope: the **storage + triage escape hatch** — what to copy from the visual-golden ecosystem when (not before) a Buiy golden set explodes. Comparison set: Skia/Chromium **Gold** (the Go service), **reg-suit** (OSS, commit-keyed object store), and the SaaS triad **Chromatic / Percy / Argos**.

**Bottom line up front.** Buiy should **not** build a Gold-class service. Build a **reg-suit-shaped *local* harness**, and copy four of Gold's *ideas* — params/traces keying, multi-positive baselines, tunable inexact matching, expiring ignores — not its infrastructure. And remember the meta-point from [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md): Tier 5 is a *minimal residue*; if the cheaper tiers (structured snapshots, metamorphic invariants, reftests, CPU-vs-GPU cross-check) do their job, much of this folder is contingency, not roadmap.

## Validates

These Buiy strategy choices are confirmed by the ecosystem's experience:

- **Keep Tier 5 minimal; don't make pixels the default.** Gold's own scale framing ("each commit creates >500k images") and its heavy infrastructure exist *because* Skia/Chromium accepted golden triage as a standing cost. Buiy's pyramid — pushing detection down to deterministic structured snapshots and relational reftests — is validated by every limit in [open-problems.md](open-problems.md): the oracle, flake, and reproducibility problems are all irreducible at the golden tier and tractable at cheaper ones.
- **Store goldens out-of-repo, keyed, not committed as files.** The whole ecosystem (Gold's GCS digests, reg-suit's S3/GCS publisher, Argos/Percy's git-history baselines) rejects committed PNGs. Screenshotbot's verbatim git-LFS critique ([storage-scale.md](storage-scale.md)) names the `O(configs × commits)` pathology Buiy must not recreate. When Buiy graduates past in-repo goldens, the commit-keyed object store is the right first step.
- **A `(widget, state, theme, viewport, backend, dpr)` key schema is the right shape.** It maps directly onto Gold's params-and-traces model — that *is* the trace identity. Validates the strategy report's combinatorial-surface framing.
- **Set-valued baselines per test, not one-baseline.** Gold "supports multiple approved images per test … not uncommon for tests to produce images that are visually indistinguishable, but differ in a handful of pixels." For a GPU library this is essential, and it validates Buiy treating a key as mapping to a *set* of accepted digests.
- **A curated `--accept` / structured-snapshot precedent already exists in-repo.** Buiy's shaping snapshots (`tests/text_shaping_snapshots.rs` with `BUIY_ACCEPT_SHAPING=1`) are an in-repo precedent for both the accept model and structured snapshots — exactly the durable accept ledger reg-suit lacks. Validates extending that pattern rather than importing reg-suit's implicit-in-git-history acceptance.

## Avoid

| Pitfall | Source | Buiy stance |
|---|---|---|
| Standing up a Gold-class service (GCS bucket + GCE/k8s frontend + per-corpus config + triage queue) | [gold-architecture.md](gold-architecture.md) | Gross overkill for Buiy. Gold is right for Chromium/Flutter's thousand-config matrix and "500k images/commit." Do **not** build a database-backed trace store + web frontend until the golden set's scale actually demands it — and the pyramid is designed so it never does. |
| Treating reg-suit's implicit-in-git-history acceptance as sufficient | [ecosystem-tools.md](ecosystem-tools.md) | reg-suit has **no first-class approve command** and **no durable per-image accept ledger** — acceptance is "the next commit's snapshot becomes the baseline." That is the "golden set explodes, triage is manual" failure mode. Buiy must ship an explicit accept command that writes the new digest into a stored baseline set (the shaping-snapshot pattern). |
| Naive commit-key resolution | [ecosystem-tools.md](ecosystem-tools.md), [open-problems.md](open-problems.md) | reg-suit's keygen special-cases merge commits and is thin on rebased / multi-parent / squash histories. Commit-key resolution is the part that breaks. If Buiy keys by commit, design the rebase/squash/merge edge cases up front and keep a durable accept ledger as a second source of truth. |
| Hand-picked global pixel thresholds | [gold-architecture.md](gold-architecture.md) | Gold ships `determine_gold_inexact_parameters.py` (binary-search / local-minima) precisely because global thresholds fail. Buiy should make tolerances **tunable per test/fixture**, not a single global L1/RMSE cutoff. (The strategy report's two-axis fuzzy model is the metric; this is the per-fixture-tolerance discipline.) |
| Expecting multi-positive to be free | [storage-scale.md](storage-scale.md), [open-problems.md](open-problems.md) | Multi-positive baselines accumulate **stale positives silently** — a real regression can match an old wrong positive. If Buiy adopts set-valued baselines, also design pruning / aging, or the set drifts into "everything anyone ever blessed." |
| Assuming a golden harness solves flake | [open-problems.md](open-problems.md) | Gold mitigates flake with inexact matching + manual time-boxed ignores owned by a gardener — it does not eliminate it, and has **no automatic flaky-quarantine** (that's Argos). Buiy must not promise a low-flake golden tier; the flake is held at bay by ongoing labor unless detection moves to deterministic tiers. |
| Coupling the metric to the existing naive L1/RMSE | strategy report §4, [ecosystem-tools.md](ecosystem-tools.md) | Buiy's two naive metrics (`golden.rs` L1, `visual.rs` RMSE) have no AA-exclusion and no two-axis budget — they cannot express Mozilla-style `fuzzy(d_lo-d_hi, p_lo-p_hi)` or Gold's Sobel edge-masking. Fix the metric before building goldens *or* reftests; wrap odiff's YIQ + AA-detection rather than re-deriving a naive engine. |
| Asserting the image digest is MD5 | [storage-scale.md](storage-scale.md), [open-problems.md](open-problems.md) | **Unverified.** Public docs say "hash of pixel content" but do not name the algorithm; the MD5 reference is for the `Expectations` struct, not the image. Don't cite MD5 for image content-addressing in any Buiy doc. |
| Quoting SaaS dollar figures as fact | [ecosystem-tools.md](ecosystem-tools.md) | Chromatic ($149/mo), Percy ($39/mo, ≈$5,000/mo at 100k screenshots) are **vendor-page / secondary, unverified**. For an offline-first MIT/Apache project these tools are disqualified on cost + dependency grounds regardless; don't lean on specific numbers. |

## Borrow

Concepts to copy into a *local* harness (licenses align — reg-suit and Argos are MIT, matching Buiy's dual license):

1. **The commit-keyed object store** (from reg-suit's keygen+publisher split). A content-addressed bucket: local dir → optional S3/GCS, keyed by commit hash, with the baseline fetched as "the parent commit's snapshot." This is the cleanest OSS "storage keyed by commit" pattern. Borrow the *shape*; supply the missing durable accept ledger.

2. **The params/traces key schema** (from Gold). A digest is tagged with key/value params; a trace is the unique param set. Fix Buiy's `(widget, state, theme, viewport, backend, dpr)` schema **before generating any goldens** — retrofitting keys means re-baselining everything. Reserve `backend` to enumerate CPU/Vulkan/GL/Metal and `dpr` to be numeric. Copy the **schema concept**, not a CLI contract — `goldctl`'s exact key-passing flags are not in the public docs.

3. **Set-valued baselines (multi-positive)** (from Gold). A key maps to a *set* of accepted digests, not one — essential for GPU AA nondeterminism. Borrow with stale-positive pruning attached (see Avoid).

4. **Tunable inexact + Sobel-style edge masking** (from Gold). `FuzzyMatchingAlgorithm` (`max_different_pixels`, `pixel_per_channel_delta_threshold`) and `SobelMatchingAlgorithm` (`edge_threshold` to mask anti-aliased edges before comparison). Copy the idea of **tunable per-test tolerances** and edge-masking, optimized rather than hand-picked.

5. **Time-boxed ignore rules keyed by params** (from Gold). Ignores carry an expiry (hours-scale) so flaky configs aren't permanently muted, owned by a gardener role. Borrow the **expiring-ignore primitive**. Note: "flaky-auto-ignore" is a pattern to *design* (anchored on Argos's "minimum occurrences in 7 days → auto-ignore"), not one Gold offers — Gold's answer is inexact match + manual ignores.

6. **The local self-contained HTML diff report** (from reg-cli / x-img-diff-js), as the alternative to a hosted triage UI. Emit one self-contained HTML file per run (expected/actual/diff, ideally with structural overlay: matched/changed/unmatched), openable from CI artifacts. Triage = a human eyeballs it and runs an accept command that writes the new digest into the baseline set. This is the right altitude — never stand up Gold's web frontend.

7. **The odiff diff engine** (from `dmtrKovalenko/odiff`). Production-grade Zig+SIMD YIQ-NTSC + AA-detection; ~6.67×–7.65× faster than pixelmatch/ImageMagick on Cypress images. Wrap it, or re-implement the algorithm in Rust SIMD (a natural fit), rather than shipping the existing naive L1/RMSE.

8. **Argos's flaky-occurrence heuristic** (from Argos). "Minimum occurrences to consider a change flaky (last 7 days)" + pixel clustering is the most concrete prior art for auto-taming flaky-golden noise if Buiy ever needs it.

## SaaS comparison — why not buy

**Argos** is genuinely **MIT** with no hosting restriction — the one self-hostable option if Buiy ever wants a hosted-style UI without vendor lock-in. **Chromatic** and **Percy** are per-snapshot-priced SaaS (figures unverified; Percy "scales significantly with volume"); for an offline-first MIT/Apache project these are disqualifying on cost and dependency grounds. **Net guidance:** copy reg-suit's commit-keyed store + local HTML report; copy Gold's params/traces + multi-positive + tunable inexact-match + expiring ignores as *concepts*; defer (do not build) a Gold service until the golden set's scale genuinely demands a database-backed trace store — which, if the pyramid holds, it won't.

## How to use this file

When designing Buiy's Tier-5 (or Tier-4 reftest) harness, locate the relevant Avoid row to understand the trap, then the matching Borrow row for the primitive to adopt. Promote any decision into the not-yet-written `buiy-verification-design` spec under `docs/specs/` — this file captures what we learn from the golden ecosystem, not Buiy's own commitments. Re-verify versions and any SaaS facts against live sources before lifting concrete details.

## Sources

- Skia Gold docs: https://skia.org/docs/dev/testing/skiagold/
- Chromium GPU Pixel Testing With Gold: https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md
- Flutter Gold help: https://flutter-gold.skia.org/help
- reg-suit: https://github.com/reg-viz/reg-suit · keygen: https://github.com/reg-viz/reg-suit/blob/master/packages/reg-keygen-git-hash-plugin/README.md · reg-cli: https://github.com/reg-viz/reg-cli · x-img-diff-js: https://github.com/reg-viz/x-img-diff-js
- Argos LICENSE (MIT): https://github.com/argos-ci/argos/blob/main/LICENSE
- odiff: https://github.com/dmtrKovalenko/odiff/blob/main/README.md
- Sobel filter rationale (skia bug 9527): https://groups.google.com/a/skia.org/g/bugs/c/uLPDZS_hKYQ/m/_7uliqajCAAJ
- vizzly pricing comparison (secondary, unverified $): https://vizzly.dev/visual-testing-tools-comparison/
- Buiy visual-bug-detection strategy: [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md)
- Sibling files: [gold-architecture.md](gold-architecture.md), [storage-scale.md](storage-scale.md), [ecosystem-tools.md](ecosystem-tools.md), [open-problems.md](open-problems.md), [glossary.md](glossary.md)
