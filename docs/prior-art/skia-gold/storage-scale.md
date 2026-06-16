**Date:** 2026-06-14
**Status:** active
**Subject:** How the visual-golden ecosystem dodges golden-storage explosion — content-addressed digests out of repo, per-config params, multi-positive baselines, and the git-LFS pathology

# Storage & scale: dodging golden-storage explosion

The whole ecosystem exists to answer one problem: a naive golden suite stores an `O(configs × commits)` matrix of binary PNGs in version control, where a single font or color tweak rewrites the entire grid. This file documents the techniques that defuse it.

## Gold's core move: stop treating goldens as files

Skia Gold stops treating goldens as *files* and starts treating them as **content-addressed digests stored out-of-repo, with a separate, mutable expectations database**. Per Flutter Gold help, "Images uploaded to Gold are uniquely identified by a hash digest of their pixel content (and potentially other metadata like colorspace)." The bytes live in cloud storage, not git: a Gold instance "consists of two parts: a Google Storage bucket that data is uploaded to and a server running on GCE that ingests the data and provides a way to triage diffs," and `goldctl` "checks whether the hash of the produced image is in the list of approved hashes" — if absent it uploads the image + metadata and exits non-zero ([Chromium Gold doc](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md)). The git repo therefore stores **zero PNGs**; approval state is "managed in Gold outside of Git, but in lockstep with Git commits" ([skia.org skiagold](https://skia.org/docs/dev/testing/skiagold/)). See [gold-architecture.md](gold-architecture.md) for the full data shape.

## Pillar 1: per-config expectations via params/traces

Every digest is tagged with key/value **params** ("These keys … are generally used to label how a digest was produced, for example `OS=Android` or `GPU=Nvidia770GTX`"), and a **trace** is "all digests seen belonging to a unique set of params" ([flutter-gold help](https://flutter-gold.skia.org/help)). This is the structural answer to the OS/GPU matrix: configurations are *dimensions of one logical test*, not N separate committed files. Skia tests "across a range of dimensions, e.g.: OS (Windows, Linux, Mac, Android, iOS), Architectures (Intel, ARM), Backends (CPU, OpenGL, Vulkan etc.)" ([skia.org skiagold](https://skia.org/docs/dev/testing/skiagold/)). Chromium indexes reference data "by version number, OS, GPU vendor, GPU device, and whether or not antialiasing is enabled" (Chromium Gold doc). New params need **no** server-side pre-config; Gold ingests arbitrary keys automatically.

**Implications for Buiy.** Buiy's proposed `(widget, state, theme, viewport, backend, dpr)` key schema maps directly onto Gold's params-and-traces model — that *is* the trace identity. Fix the key schema *before* generating any goldens; retrofitting keys means re-baselining everything. Reserve `backend` to enumerate CPU/Vulkan/GL/Metal and `dpr` to be numeric — the axes a GPU UI library fans out on. (Lesson detail in [lessons.md](lessons.md).)

## Pillar 2: multi-positive baselines

The second pillar keeps legitimate variants from reddening the suite: "A trace (or test) is allowed to have multiple positive digests; in practice this happens due to things like nondeterminism in anti-aliasing algorithms for certain GPUs" (Flutter help). Triage is binary per digest — `positive` = acceptable, `negative` = needs a fix — but a test **passes if its output hash matches *any* approved positive**. Chromium frames this as the explicit reason it left fuzzy/threshold matching: with 2–3 valid variants, "being able to say that any of those images are acceptable is simpler and less error-prone." Triage is also immediate: "new golden images don't need to go through the CQ … Once an image is triaged in Gold, it becomes immediately available for future test runs" — versus a committed-baseline workflow that needs a CL round-trip.

**Cost of the pillar.** Multi-positive means **stale positives accumulate silently** — nothing prunes an approved digest that no config produces anymore. See [open-problems.md](open-problems.md).

## The contrast: committing PNGs / git-LFS

Chromium's predecessor stored approved *hashes* committed to the repo with images in a GS bucket, where "the only thing the user had to go on was a hash"; Gold "moves the images out of the repository, but provides a GUI interface for easily seeing which images are currently approved" (Chromium Gold doc). The git-LFS escape valve does not scale for goldens. Screenshotbot's critique, verbatim ([screenshotbot.io/blog/can-git-lfs-scale](https://screenshotbot.io/blog/can-git-lfs-scale)):

> "If you have 100 commits that change almost all of the screenshots (say a font or color change), you'll soon be using 5GB of storage!"

> "Each CI job needs to fetch all of the current screenshots. This slows down the clone step, which blocks CI for all your developers (whether or not they are making UI changes)."

> "If your screenshots are in Git LFS, the history is going to be slow to fetch, which means developers are unlikely to actually use" bisection.

> "Many teams have dedicated engineers just to manage Git LFS."

The pathology is fundamental: an `O(configs × commits)` matrix of binaries committed to history, where one font tweak rewrites the whole grid.

## Peers converge on out-of-repo + branch-scoped baselines

The whole comparison set rejects committed files (details in [ecosystem-tools.md](ecosystem-tools.md)):

- **reg-suit** "automatically stores snapshot images to external cloud storage (e.g. AWS S3, Google Cloud Storage)" and keys baselines via a git-hash key-generator plugin — images out of repo, key in git ([reg-suit repo](https://github.com/reg-viz/reg-suit)).
- **Chromatic** keeps per-permutation baselines: "One story captured in 3 browsers at 3 viewports equals 9 snapshots … independent baselines and distinct approvals," with TurboSnap copying unchanged baselines forward ([TurboSnap docs](https://www.chromatic.com/docs/turbosnap/)).
- **Argos** and **Percy** resolve baselines by walking git history rather than storing files — "A baseline is not a file—it is a decision"; Percy assigns "each branch … its own branch-level baseline" ([Argos baseline](https://argos-ci.com/docs/baseline-build); [Percy git baselines](https://www.browserstack.com/docs/percy/baseline-management/git)).

## Warts / unverified

- Even exact-hash + multi-positive needs babysitting: Chromium flags tests "prone to noise which causes them to need additional triaging at times" (Chromium Gold doc), motivating fuzzy/Sobel-mask requests ([skia bug 9527](https://groups.google.com/a/skia.org/g/bugs/c/uLPDZS_hKYQ)).
- **Unverified:** the docs say the image digest is a "hash of its pixel content" but do **not** name the algorithm. The "MD5" reference in Gold's Go API applies to the `Expectations` struct hash, **not** the image digest — do not assert MD5 for image content-addressing.
- **Unverified:** the Skia/Chromium backend (GCS + GCE) is Google-operated; a self-host cost/ops figure for Buiy was not found in primary sources.

## Sources

- Skia Gold docs: https://skia.org/docs/dev/testing/skiagold/
- Chromium GPU Pixel Testing With Gold: https://chromium.googlesource.com/chromium/src/+/HEAD/docs/gpu/gpu_pixel_testing_with_gold.md
- Flutter Gold help: https://flutter-gold.skia.org/help
- Screenshotbot — can git LFS scale: https://screenshotbot.io/blog/can-git-lfs-scale
- reg-suit: https://github.com/reg-viz/reg-suit
- Chromatic TurboSnap: https://www.chromatic.com/docs/turbosnap/
- Argos baseline: https://argos-ci.com/docs/baseline-build
- Percy git baseline management: https://www.browserstack.com/docs/percy/baseline-management/git
- Sobel filter rationale (skia bug 9527): https://groups.google.com/a/skia.org/g/bugs/c/uLPDZS_hKYQ
