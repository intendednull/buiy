**Date:** 2026-06-14
**Status:** active
**Subject:** Skia Gold — the service model, digest/params/traces data shape, goldctl flow, triage, ignores, and inexact matching

# Gold architecture

Skia Gold is an image-diff **service**, not a local library. Per Skia's own docs it is "a web application that compares the images produced by our bots against known baseline images," with baselines "managed in Gold outside of Git, but in lockstep with Git commits" ([skia.org/docs/dev/testing/skiagold](https://skia.org/docs/dev/testing/skiagold/)). It is written in Go with a Polymer frontend; code lives in the Skia Infra repo `github.com/google/skia-buildbot` under `//golden/` (service) and `//gold-client/` (client). The one architectural decision that defines it relative to a checked-in golden file: **comparison happens in an external service, not on the test machine** ([Chromium Gold doc](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md)).

## The data model: digest → params → trace → test → corpus

- **Digest.** A *digest* is the content hash of a PNG's pixel content — Flutter's help page: "a hash digest of their pixel content (and potentially other metadata like colorspace)"; "the terms digest and image are used synonymously" ([flutter-gold.skia.org/help](https://flutter-gold.skia.org/help)). Identity is content-addressed: identical pixels → identical digest → instant pass with no upload. *The hash algorithm is not named in the public docs; do not assume MD5 — see [open-problems.md](open-problems.md).*
- **Params.** Each uploaded digest is tagged with *params* — "key/value pairs … generally used to label how a digest was produced, for example `OS=Android` or `GPU=Nvidia770GTX`." Params are open-ended: "Gold will automatically identify and process any new params produced by a test." No server-side pre-registration of keys.
- **Trace.** A *trace* is "all digests seen belonging to a unique set of params," rendered in the UI as "lines of colored dots where a color refers to a specific digest." Each trace belongs to exactly one test in one corpus.
- **Corpus → test.** Data is grouped first by **corpus**, then by **test** (Flutter help; Skia README). The Gold instance config requires "an explicit list of corpora, specified via the `grouping_param_keys_by_corpus` field" ([golden README](https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md)).

This is the structural answer to the configuration matrix: OS/GPU/backend are *dimensions of one logical test*, not separate committed files. See [storage-scale.md](storage-scale.md) for how this dodges golden-storage explosion.

## goldctl — the client flow

The client is `goldctl` ("gold-control"), import path `github.com/google/skia-buildbot/gold-client/cmd/goldctl`, BSD-3-Clause. pkg.go.dev lists only a synthetic `v0.0.0-…` pseudo-version — goldctl is built from source / distributed via CIPD, **not** released as a tagged semver binary (no tagged release verifiable). Subcommands: `auth`, `imgtest` (with `init` / `add` / `finalize`), `validate` ([pkg.go.dev/.../goldctl](https://pkg.go.dev/github.com/google/skia-buildbot/gold-client/cmd/goldctl)).

The flow (Chromium docs): a test "produces an image and passes it to `goldctl`, along with some information about the hardware and software configuration … the test name, etc." goldctl "checks whether the hash of the produced image is in the list of approved hashes"; if matched it passes silently, else "`goldctl` uploads the image and metadata to the storage bucket and exits with a failing return code."

- `imgtest init` sets a work dir + `keys.json` once, so later `add` calls are terse.
- `imgtest add` validates that the test carries every param required by its corpus grouping.
- `imgtest finalize` closes out the run.

**GCS is the source of truth.** "All data uploaded from tests will live here and be interpreted by Gold" (Skia README). The frontend ingests the bucket: "the server sees the new data … and ingests it, showing a new untriaged image in the GUI" (Chromium docs).

## The many-positives model (the key distinction from one-baseline goldens)

"Gold supports multiple approved images per test … any of those images are acceptable." Why: "A trace (or test) is allowed to have multiple positive digests; in practice this happens due to things like nondeterminism in anti-aliasing algorithms for certain GPUs" (Flutter help). Triage classifies a digest:

- **`positive`** — acceptable.
- **`negative`** — must-not-recur; requires a fix.
- **`untriaged`** — "generally means that a test has started producing different output."

A test **passes if its output hash matches *any* approved positive**. On the waterfall there is one baseline set; on trybots the baseline is "the union of the master baselines for the current revision and any baselines that are unique to the CL," so reviewers can triage before landing (Chromium docs). Triage is immediate — once triaged, a digest is available for future runs without a CL round-trip.

## Triage UI

Digests are "automatically compared to another digest from the same test; in fact, the most similar digest," with zoom and a `u` shortcut to jump to the largest pixel difference (Flutter help). The frontend is the thing Buiy would *not* build — see the local-HTML-report alternative in [lessons.md](lessons.md) and [ecosystem-tools.md](ecosystem-tools.md) (reg-cli).

## Ignore rules, including time-boxed

The Ignores view lets you "create a new, short-interval (hours) ignore for the most affected configuration(s)" — ignores carry an expiry so flaky configs don't get permanently muted ([Skia docs](https://skia.org/docs/dev/testing/skiagold/)). *Caveat: the docs describe hours-scale time-boxing but do not specify exact post-expiry re-activation semantics — unverified from primary sources.* Note also: Gold's flaky answer is **inexact matching + manual time-boxed ignores**, NOT an automatic flaky-auto-quarantine mechanism (no such mechanism is documented for Gold — Argos has one; see [ecosystem-tools.md](ecosystem-tools.md)).

## Inexact-matching escape hatch

For noisy tests, admins set a `matching_algorithm` on the `PixelTestPage`:

- **Fuzzy** — `max_different_pixels` + `pixel_per_channel_delta_threshold` (e.g. `=2`): pass on "only minor differences" instead of exact-hash equality.
- **Sobel** — adds `edge_threshold` to mask anti-aliased edges before comparison (`pixel_delta_threshold` e.g. `=30`); rationale in [skia bug 9527](https://groups.google.com/a/skia.org/g/bugs/c/uLPDZS_hKYQ).

Gold ships `determine_gold_inexact_parameters.py` with `binary_search` / `local_minima` optimizers to *tune* these per test — the borrowable idea is **tunable per-test tolerances**, not hand-picked global thresholds. The very existence of this escape hatch acknowledges that pure content-addressing is brittle under GPU nondeterminism.

## Adopters (verified)

Skia, Chromium, PDFium, and the Flutter framework (`flutter-gold.skia.org`) all run Gold instances. Both Chromium and Skia docs note Gold "was originally developed for Skia's usage but has been adopted by other projects such as Chromium and PDFium."

## The wart worth flagging for Buiy

Gold is heavy infrastructure — a GCS bucket + a GCE/k8s frontend + per-corpus config + a human triage queue. It is the escape hatch *when a golden set explodes*, not a cheap default. Its own scale framing ("Each commit creates >500k images," Skia README) shows it is built for an org that has already accepted golden-screenshot triage as a standing cost. Buiy's strategy is the opposite: keep Tier 5 minimal so this machinery is never needed. See [lessons.md](lessons.md) `## Avoid`.

## Sources

- Skia Gold docs: https://skia.org/docs/dev/testing/skiagold/
- Chromium GPU Pixel Testing With Gold: https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md
- Flutter Gold help: https://flutter-gold.skia.org/help
- skia-buildbot golden README: https://github.com/google/skia-buildbot/blob/main/golden/docs/README.md
- goldctl on pkg.go.dev: https://pkg.go.dev/github.com/google/skia-buildbot/gold-client/cmd/goldctl
- Sobel filter rationale (skia bug 9527): https://groups.google.com/a/skia.org/g/bugs/c/uLPDZS_hKYQ
