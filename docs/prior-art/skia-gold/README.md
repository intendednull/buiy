**Date:** 2026-06-14
**Status:** active
**Subject:** Skia/Chromium Gold + the visual-golden storage & triage ecosystem — the escape hatch for when a golden set explodes

# Skia Gold & the visual-golden storage/triage ecosystem

**Skia Gold** is an image-diff *service* (Go backend + Polymer frontend, run on Google Cloud) that the Skia team built to compare images produced by their bots against known baselines. Its defining architectural move — versus committing golden PNGs to a repo — is that **comparison happens in an external service, not on the test machine**: a test produces a PNG, hands it to the `goldctl` client with hardware/software metadata, and goldctl checks whether the image's content hash (its *digest*) is in the list of approved hashes. Match → silent pass with no upload; miss → upload image + metadata to a GCS bucket, exit non-zero, and surface an untriaged image in the triage UI. Baselines live "outside of Git, but in lockstep with Git commits," tagged with open-ended key/value **params** (`OS=Android`, `GPU=Nvidia770GTX`) that turn the OS×GPU×backend matrix into *dimensions of one logical test* rather than N committed files. Gold supports **multiple approved images per test** (anti-aliasing nondeterminism makes one-baseline-per-test untenable on GPUs), inexact/fuzzy/Sobel matching for noisy tests, and time-boxed ignore rules for flaky configs.

This folder treats Gold as Buiy's **storage + triage escape hatch** — the precedent to reach for *when (not before)* a Buiy golden set explodes — and surrounds it with the comparison set Buiy will actually choose from: **reg-suit** (the OSS, commit-hash-keyed, self-hostable reference design), the SaaS triad **Chromatic / Percy / Argos**, and the OSS leaf tools and diff engines (BackstopJS, jest-image-snapshot, pixelmatch, odiff). The bottom-line decision lives in [lessons.md](lessons.md): Buiy should **not** build a Gold-class service; it should build a reg-suit-shaped *local* harness and copy four of Gold's *ideas* — params/traces keying, multi-positive baselines, tunable inexact matching, expiring ignores — without its infrastructure.

This is the Tier-5 (golden/screenshot) prior-art for [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md). The strategy's whole point is to keep Tier 5 a *minimal residue* by catching most regressions in Tiers 1–4; this folder documents what to do for the residue that genuinely needs stored rasterized images.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Gold language / repo | Go (+ Polymer frontend); `github.com/google/skia-buildbot` under `//golden/` (service) and `//gold-client/` (client) | [skia.org skiagold](https://skia.org/docs/dev/testing/skiagold/) |
| Architectural choice | Comparison in an **external service**, not on the test machine | [Chromium Gold doc](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md) |
| Client | `goldctl` — `github.com/google/skia-buildbot/gold-client/cmd/goldctl`, BSD-3-Clause; built from source/CIPD, **no verifiable tagged semver release** | [pkg.go.dev goldctl](https://pkg.go.dev/github.com/google/skia-buildbot/gold-client/cmd/goldctl) |
| `goldctl` subcommands | `auth`, `imgtest` (`init` / `add` / `finalize`), `validate` | [pkg.go.dev goldctl](https://pkg.go.dev/github.com/google/skia-buildbot/gold-client/cmd/goldctl) |
| Storage backend | GCS bucket (bytes) + GCE/k8s frontend (ingest + triage); Google-operated | [skia-buildbot golden README](https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md) |
| Digest | Content hash of a PNG's pixel content (+ possibly colorspace metadata); "digest" ≡ "image" | [flutter-gold help](https://flutter-gold.skia.org/help) |
| Image-digest algorithm | **Unverified** — docs say "hash of pixel content" but do not name it; the MD5 reference is for the `Expectations` struct, NOT the image | flagged below |
| Triage labels | `positive` / `negative` / `untriaged` (binary triage; pass if hash matches *any* positive) | [flutter-gold help](https://flutter-gold.skia.org/help) |
| Multi-positive | One trace/test may have many approved digests (GPU AA nondeterminism) | [flutter-gold help](https://flutter-gold.skia.org/help) |
| Inexact matching | Per-test `matching_algorithm` — Fuzzy (`max_different_pixels`, `pixel_per_channel_delta_threshold`) / Sobel (`edge_threshold`) | [Chromium Gold doc](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md) |
| Ignores | Time-boxed (hours-scale) ignore rules keyed by params; gardener-owned | [skia.org skiagold](https://skia.org/docs/dev/testing/skiagold/) |
| Scale framing | "Each commit creates >500k images" | [skia-buildbot golden README](https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md) |
| Verified adopters | Skia, Chromium, PDFium, Flutter framework (`flutter-gold.skia.org`) | [Chromium Gold doc](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md), [skia.org skiagold](https://skia.org/docs/dev/testing/skiagold/) |
| reg-suit | `reg-viz/reg-suit`, npm **0.14.5** (2025-08-26), **MIT**, no SaaS backend | [reg-suit repo](https://github.com/reg-viz/reg-suit) |
| Argos license | **MIT** (self-hostable; no hosting restriction) | [Argos LICENSE](https://github.com/argos-ci/argos/blob/main/LICENSE) |
| odiff | `dmtrKovalenko/odiff`, npm `odiff-bin` **4.3.8**; Zig + SIMD; ~6.67×–7.65× faster than pixelmatch/ImageMagick **on Cypress images** (lower — 5.24×–5.50× — on an 8K image; not a universal speedup) | [odiff README](https://github.com/dmtrKovalenko/odiff/blob/main/README.md) |
| SaaS dollar figures | **All unverified** (vendor pricing pages / secondary comparisons only) | flagged below |

## Contents

Each file is independently skimmable with its own `## Sources`.

**Skia Gold itself**

- [**gold-architecture.md**](gold-architecture.md) — The service model: digests, params/traces, corpus→test grouping, `goldctl` flow, GCS as source of truth, the many-positives model, triage labels + UI, time-boxed ignores, inexact/Sobel matching, adopters, and the "heavy infrastructure" wart.
- [**storage-scale.md**](storage-scale.md) — *How* Gold dodges golden-storage explosion: content-addressed digests out of repo + a mutable expectations DB; per-config params instead of N committed files; multi-positive baselines; the git/git-LFS pathology (Screenshotbot's verbatim critique); how the peer tools (reg-suit, Chromatic, Argos, Percy) all converge on out-of-repo + branch-scoped baselines.

**The comparison set**

- [**ecosystem-tools.md**](ecosystem-tools.md) — The OSS-vs-SaaS split (OSS owns the diff engine, SaaS owns storage/triage); per-tool deep notes on reg-suit, Chromatic (modes = the explosion engine), Argos (flaky auto-ignore), Percy (carry-forward approvals), BackstopJS, jest-image-snapshot, and the engine layer (pixelmatch / odiff); the full comparison table.

**Reference**

- [**open-problems.md**](open-problems.md) — What this ecosystem structurally does *not* solve: the oracle problem, stale-positive accumulation, flake without a manual gardener, cross-machine reproducibility, cost/ops floor, commit-key resolution edge cases.
- [**lessons.md**](lessons.md) — **The consult-this-when-designing decision file.** `## Validates` / `## Avoid` / `## Borrow`. This is where Buiy implications live.
- [**glossary.md**](glossary.md) — System-specific terms: digest, param, trace, corpus, baseline, expectation, positive/negative/untriaged, keygen/publisher plugin, mode, snapshot, carry-forward, fuzzy/Sobel matching, TurboSnap.

## Reading order

1. [lessons.md](lessons.md) — the decisions. Start here if you are designing Buiy's Tier-5 harness.
2. [gold-architecture.md](gold-architecture.md) — what Gold actually is, so the lessons have a referent.
3. [storage-scale.md](storage-scale.md) — the storage-explosion problem the whole ecosystem exists to solve.
4. [ecosystem-tools.md](ecosystem-tools.md) — the menu Buiy chooses from (reg-suit is the closest analog).
5. [open-problems.md](open-problems.md) — the limits, so Buiy doesn't expect the tier to do more than it can.
6. [glossary.md](glossary.md) — reference when a term is unclear.

## How to use

**Framing disclosure.** These docs are written from Buiy's stance — an AccessKit-first, wgpu + Taffy + cosmic-text, parallel-to-bevy_ui retained-mode engine building a reftests-first layered visual-bug-detection strategy. The "Implications for Buiy" / lessons framing reads Skia/Chromium Gold + the visual-golden storage & triage ecosystem through that lens; readers auditing whether that strategy is itself right should weigh the corpus accordingly — it is a learn-from artifact, not a neutral catalog.

**Corpus-specific framing.** This corpus is written from the stance that **Buiy lands in the OSS camp (Rust, offline-first, MIT/Apache, no SaaS), and Tier 5 is a deliberately-minimal residue**. "Implications for Buiy" lines therefore lean toward reg-suit's self-hostable shape and treat the SaaS tools mostly as cautionary baseline-multiplication and cost evidence. A reader evaluating whether Buiy should adopt a *hosted* triage UI at all — or whether the golden tier is worth building before the pyramid's cheaper tiers are exhausted — should weigh the corpus accordingly. The strategy report's own thesis is that Tiers 1–4 shrink this tier to almost nothing; if that holds, much of Gold's machinery is moot for Buiy.

**Why "minimal" matters — an order-of-magnitude on the matrix.** Buiy's own `(widget × state × theme × viewport × backend × dpr)` key schema fans out fast. A rough count — say 40 widgets × 4 states × 2 themes — is already ~320 cells *before* any rendering axis; cross with 3 viewports it is ~1k, and a full fan-out over 4 backends (CPU/Vulkan/GL/Metal) × 2 dpr lands near **~7–8k goldens** for a modest v1 catalog. That is the Chromatic "modes" multiplication (see [ecosystem-tools.md](ecosystem-tools.md)) made concrete for Buiy, and it is the number that makes "keep Tier 5 minimal" a quantitative discipline, not a slogan: every widget/state pair pushed *down* to a deterministic structured snapshot or reftest removes a whole backend×dpr column of stored pixels. (Counts are illustrative, not committed; they exist to size the decision, and the lean cut — one backend pinned, one dpr — is ~10× smaller.)

## Sources

- Skia Gold docs: https://skia.org/docs/dev/testing/skiagold/
- Chromium GPU Pixel Testing With Gold: https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md
- Flutter Gold help: https://flutter-gold.skia.org/help
- skia-buildbot golden README: https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md
- goldctl on pkg.go.dev: https://pkg.go.dev/github.com/google/skia-buildbot/gold-client/cmd/goldctl
- reg-suit: https://github.com/reg-viz/reg-suit
- Argos LICENSE (MIT): https://github.com/argos-ci/argos/blob/main/LICENSE
- odiff: https://github.com/dmtrKovalenko/odiff
- Buiy visual-bug-detection strategy: [`docs/reports/2026-06-14-visual-bug-detection-strategy.md`](../../reports/2026-06-14-visual-bug-detection-strategy.md)
- Per-file `## Sources` sections cite the specific URLs each file relies on.
