**Date:** 2026-06-14
**Status:** active
**Subject:** Skia Gold + visual-golden ecosystem — system-specific terminology glossary

# Glossary

System-specific terms used throughout this folder. Gold terms first; then the storage/triage concepts shared across the ecosystem; then per-tool terms. Definitions are scoped to how each term is used in the Gold / visual-golden context, not the broader testing literature.

## Skia Gold core

- **Gold** — Skia's image-diff **service** (Go backend + Polymer frontend, Google Cloud). Compares bot-produced images against approved baselines in an external service, not on the test machine. Code in `github.com/google/skia-buildbot` under `//golden/`.
- **digest** — The content hash of a PNG's pixel content (possibly + colorspace metadata). Used synonymously with "image." Identity is content-addressed: identical pixels → identical digest. *Hash algorithm not named in public docs — see [open-problems.md](open-problems.md); NOT verified to be MD5.*
- **param** — A key/value pair labeling how a digest was produced, e.g. `OS=Android`, `GPU=Nvidia770GTX`. Open-ended; Gold ingests new params automatically with no pre-registration.
- **trace** — All digests seen for a unique set of params. Rendered in the UI as a line of colored dots (color = digest). Belongs to exactly one test in one corpus.
- **test** — A named visual case; the grouping under a corpus. A trace belongs to one test.
- **corpus** — The top-level grouping above tests. A Gold instance config lists corpora explicitly via `grouping_param_keys_by_corpus`.
- **positive** — Triage label: this digest is acceptable. A test passes if its hash matches *any* approved positive.
- **negative** — Triage label: this digest must not recur; requires a fix.
- **untriaged** — Triage label: a not-yet-classified digest; "generally means a test has started producing different output."
- **baseline / baseline set** — The set of approved (positive) digests for a test under a given revision. On trybots it is "the union of the master baselines for the current revision and any baselines unique to the CL."
- **expectations** — Gold's mutable database mapping (test, digest) → triage label, stored out-of-Git but in lockstep with Git commits. (The `Expectations` *struct's* hash uses MD5 — distinct from the image digest.)
- **goldctl** — The Gold client ("gold-control"), BSD-3-Clause, built from source/CIPD (no verifiable tagged semver release). Subcommands: `auth`, `imgtest` (`init`/`add`/`finalize`), `validate`. Checks an image's hash against approved hashes; uploads + exits non-zero on miss.
- **multi-positive** — Gold's model allowing many approved digests per test, to absorb GPU anti-aliasing nondeterminism.
- **matching_algorithm** — A per-`PixelTestPage` setting selecting inexact comparison. See Fuzzy / Sobel.
- **Fuzzy matching** — `FuzzyMatchingAlgorithm`: passes when differences are within `max_different_pixels` and `pixel_per_channel_delta_threshold`.
- **Sobel matching** — `SobelMatchingAlgorithm`: applies a Sobel edge filter with `edge_threshold` to mask anti-aliased edges before comparison (`pixel_delta_threshold`). Rationale: skia bug 9527.
- **time-boxed ignore** — An ignore rule for a config that carries an expiry (hours scale), so flaky configs aren't permanently muted. Gardener-owned. *Post-expiry re-activation semantics unverified.*
- **gardener** — The human role that triages untriaged digests, sets ignores, and tunes inexact thresholds. Gold's flake answer is this role + inexact matching, not automation.

## Shared storage/triage concepts

- **content-addressed** — Identity derived from the bytes (the hash) rather than a filename or path. The basis of Gold's "instant pass with no upload" behavior.
- **out-of-repo storage** — Keeping image bytes in object storage (S3/GCS) instead of committed to git, with only a key/hash referenced from version control. The ecosystem-wide answer to golden-storage explosion.
- **branch-scoped / branchline baseline** — A baseline resolved per branch (Percy) or by walking git ancestry (Argos), rather than a single fixed golden. "A baseline is not a file—it is a decision."
- **carry-forward approval** — Identical snapshots approved once per branch lifetime, then reused (Percy).
- **golden-storage explosion** — The `O(configs × commits)` growth of committed binary baselines, where one font/color change rewrites the whole grid. The problem the whole ecosystem exists to defuse.

## Per-tool terms

- **reg-suit** — OSS, self-hostable, plugin-host visual-regression tool (MIT). No SaaS backend.
- **key-generator plugin** — reg-suit plugin answering "what commit do I compare to?" `reg-keygen-git-hash-plugin` walks the git branch graph to find the topic branch's parent commit.
- **publisher plugin** — reg-suit plugin that is the storage layer: fetches expected snapshots from and pushes current snapshots + HTML report to S3/GCS, keyed by the generated hash.
- **notifier plugin** — reg-suit plugin posting commit status / PR comments (GitHub, GitLab, Slack, Chatwork).
- **x-img-diff-js / ximgdiff** — reg-suit's structural diff engine (OpenCV via WebAssembly): cyan = matched region, red = changed, purple = unmatched keypoint.
- **reg-cli** — reg-suit's CLI that generates the local static HTML report (expected/actual/diff).
- **mode (Story Mode)** — Chromatic combination of globals (viewport/theme/locale) saved via `chromatic.modes`. Each mode gets an **independent baseline and distinct approval** — the baseline-multiplication mechanic.
- **snapshot** — The billing/granularity unit: one story × one browser × one viewport (Chromatic); the approval-granularity unit in Percy.
- **TurboSnap** — Chromatic's git + dependency-graph analysis that re-snapshots only changed stories, billing copied snapshots at 1/5 rate.
- **flaky auto-ignore** — Argos feature: after a change recurs ≥ N times in the last 7 days, Argos auto-ignores it. Paired with pixel clustering. The closest prior art to automatic flake quarantine (Gold has none).
- **pixelmatch** — JS per-pixel diff engine (YIQ-NTSC + AA detection); default for jest-image-snapshot.
- **ssim** — Structural-similarity diff mode (experimental in jest-image-snapshot).
- **odiff** — `dmtrKovalenko/odiff`, Zig+SIMD diff engine (npm `odiff-bin`), same YIQ + AA detection as pixelmatch, ~6.67×–7.65× faster on Cypress images. The production engine to wrap.
- **ResembleJS** — BackstopJS's diff engine.

## Sources

- Skia Gold docs: https://skia.org/docs/dev/testing/skiagold/
- Chromium GPU Pixel Testing With Gold: https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md
- Flutter Gold help: https://flutter-gold.skia.org/help
- skia-buildbot golden README: https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md
- goldctl on pkg.go.dev: https://pkg.go.dev/github.com/google/skia-buildbot/gold-client/cmd/goldctl
- reg-suit: https://github.com/reg-viz/reg-suit
- Chromatic Modes: https://www.chromatic.com/docs/modes/ · TurboSnap: https://www.chromatic.com/docs/turbosnap/
- Argos GitHub docs: https://argos-ci.com/docs/github
- Percy baseline management: https://www.browserstack.com/docs/percy/visual-testing-workflows/baseline-management/overview
- odiff: https://github.com/dmtrKovalenko/odiff
