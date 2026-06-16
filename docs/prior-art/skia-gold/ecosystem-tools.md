**Date:** 2026-06-14
**Status:** active
**Subject:** The visual-testing tool ecosystem (comparison set) — reg-suit, Chromatic, Argos, Percy, BackstopJS, jest-image-snapshot, and the diff-engine layer (pixelmatch / odiff)

# The visual-testing tool ecosystem

This is the OSS+SaaS comparison set Buiy's strategy treats as the "storage + triage escape hatch" precedent. The split is sharp: **OSS tools own the diff engine and leave storage/triage to you; SaaS tools own storage/triage and rent you the diff.** Buiy will likely land in the OSS camp (Rust, no SaaS), so the reg-suit "plugin-the-storage" model and the odiff diff engine are the closest analogs. See [storage-scale.md](storage-scale.md) for why all of them reject committed files, and [lessons.md](lessons.md) for the Buiy decision.

## reg-suit — the self-hostable, commit-hash-keyed reference design

reg-suit (`reg-viz/reg-suit`, npm **0.14.5**, published **2025-08-26**, verified via `npm view`; **MIT**) is the most architecturally relevant: it has **no SaaS backend**. It is a plugin host with three plugin categories ([README](https://github.com/reg-viz/reg-suit/blob/master/README.md)):

- **Key-generator plugins** answer "what commit should I compare to?" `reg-keygen-git-hash-plugin` (v0.14.5) "detects automatically the parent's commit which is the source of the topic branch" by walking the git branch graph, and uses that commit's snapshot as the expected baseline ([keygen README](https://github.com/reg-viz/reg-suit/blob/master/packages/reg-keygen-git-hash-plugin/README.md)). `reg-simple-keygen-plugin` allows arbitrary string keys. **Wart:** the keygen special-cases merge commits ("if your topic branch has the merge commit from the parent branch, this plugin uses this merge commit hash as the expected snapshot key") and the README is thin on rebased-branch / multi-parent edge cases — commit-key resolution is the part that breaks in practice ([open-problems.md](open-problems.md)).
- **Publisher plugins** are the storage layer: `reg-publish-s3-plugin` / `reg-publish-gcs-plugin` fetch the previous (expected) snapshots from object storage, then push current snapshots + the HTML report back, keyed by the generated hash. Config uses runtime placeholder substitution, e.g. `"bucketName": "$S3_BUCKET_NAME"`.
- **Notifier plugins** (GitHub, GitLab, Slack, Chatwork) post commit status / PR comments.

Diff engine is **x-img-diff-js** (structural, not just pixel; OpenCV-via-WebAssembly), gated on `core.ximgdiff`, with `thresholdRate` (0–1 ratio) and `thresholdPixel` (absolute) knobs, plus `matchingThreshold` (YUV distance). The `reg-cli`/reg-suit output is a **static HTML report** (expected/actual/diff) generated locally — no server. ximgdiff mode overlays structural diffs: cyan = matched, red = changed, purple = unmatched keypoints. **Key wart:** the README has no first-class "approve" command — triage is the HTML report plus the next commit's snapshot *becoming* the new baseline. **There is no durable per-image accept ledger; acceptance is implicit in git history.** That is exactly the "golden set explodes, triage is manual" failure mode Buiy's strategy aims to avoid — but the keygen+publisher split is still the cleanest OSS pattern for "object storage keyed by commit."

## Chromatic — baselines multiplied by "modes" (the explosion engine)

Chromatic (SaaS, Storybook-native) stores baselines server-side and compares each build against the last *approved* build. Its defining mechanic is **Story Modes**: combinations of globals (viewport, theme, locale) saved as a named "mode" via the `chromatic.modes` parameter. "These modes are treated separately, with independent baselines and distinct approvals" — two stories × two modes = **four** independently-approved tests ([Modes docs](https://www.chromatic.com/docs/modes/)). This is the literal baseline-multiplication Buiy must budget for. Billing unit is the **snapshot** (one story × one browser × one viewport); a story in 3 browsers × 3 viewports = 9 snapshots. **TurboSnap** uses git + dependency-graph analysis to re-snapshot only changed stories, billing copied snapshots at **1/5 rate** ([TurboSnap docs](https://www.chromatic.com/docs/turbosnap/)). Pricing (free tier 5,000 snapshots/mo, Pro from **$149/mo** for 35,000) is a **vendor-page figure, unverified** — confirm against [chromatic.com/pricing](https://www.chromatic.com/pricing).

## Argos — git-history baselines + flaky auto-ignore

Argos (`argos-ci/argos`, **MIT** — self-hostable, no hosting restriction) picks baselines from git: the most recent candidate build with the same build name, all tests passed, not a *subset*, and "whose commit is an ancestor of the merge base between the triggered build's commit and the baseline branch" ([docs/llms-full.txt](https://argos-ci.com/docs/llms-full.txt)). Standout triage feature is **flaky auto-ignore**: "deterministic pixel diffing" runs multiple diff passes at different thresholds plus **pixel clustering** to separate noise from real change; a project setting — "*Minimum occurrences to consider a change flaky (last 7 days)*" — controls "how many times the same change must appear in the last 7 days before Argos starts ignoring it automatically." GitHub integration sets commit status, posts PR summary comments, runs inside the **merge queue**, and can block merges via branch protection ([GitHub docs](https://argos-ci.com/docs/github)). This "minimum occurrences in 7 days → auto-ignore" is the most concrete prior art for taming flaky-golden noise.

## Percy — branch-scoped "carry-forward" approvals

Percy (BrowserStack) compares against the **last approved build on the same branchline**, not a fixed golden. Approval is at **snapshot** granularity (you cannot approve an individual browser/width screenshot) and approvals are "carried forward" — identical snapshots are approved once per branch lifetime ([approval docs](https://www.browserstack.com/docs/percy/build-results/approval); [baseline docs](https://www.browserstack.com/docs/percy/visual-testing-workflows/baseline-management/overview)). Pricing reportedly "from $39/mo" — **vendor figure, unverified**. Per third-party comparison, Percy's per-screenshot model "scales significantly with volume" (a 10-person / 100k-screenshot team ≈ $5,000/mo) — **secondary source, dollar figure unverified** ([vizzly comparison](https://vizzly.dev/visual-testing-tools-comparison/)).

## OSS leaf tools

- **BackstopJS** (`garris/BackstopJS`): Puppeteer (capture) + **ResembleJS** (diff). Three-verb workflow — `generate` / `test` / `approve`, where `approve` overwrites local reference PNGs; HTML report has a before/after scrubber. Storage = local filesystem ([README](https://github.com/garris/BackstopJS/blob/master/README.md)).
- **jest-image-snapshot** (`americanexpress/jest-image-snapshot`): Jest matcher; baselines in `__image_snapshots__/`, accept via `jest -u`. Default engine **pixelmatch** (per-pixel `threshold` default 0.01); experimental **ssim** structural mode "may become the default" ([npm](https://www.npmjs.com/package/jest-image-snapshot)).
- **pixelmatch / odiff**: the engine layer. **odiff** (`dmtrKovalenko/odiff`, npm `odiff-bin` **4.3.8**) — "originally written in OCaml, currently in **Zig** with SIMD (SSE2/AVX2/AVX512/NEON)," same YIQ-NTSC + antialiasing detection as pixelmatch, CLI + Node binding. Benchmarks (hyperfine, from the README's `relative` column): on Cypress screenshots odiff **1.168s** vs pixelmatch **7.712s** (**6.67×**) and ImageMagick **8.881s** (**7.65×**); on an 8K image odiff **1.951s** vs pixelmatch 10.614s (**5.50×**) and ImageMagick 9.326s (**5.24×**) ([README](https://github.com/dmtrKovalenko/odiff/blob/main/README.md)). odiff is the production-grade engine to wrap if Buiy builds Tier-5 goldens, and a Rust SIMD equivalent is a natural fit. **Note on the headline number:** the 6.67×–7.65× figure is Cypress-specific; on the 8K image the speedup is lower (5.24×–5.50×), so the Cypress range is not odiff's universal speedup. The README's own prose rounds this to "6 times faster" — there is no verified "8× faster" claim.

## Comparison table

| Tool | Storage model | Baseline keying | Accept workflow | Triage UX | Diff engine |
|---|---|---|---|---|---|
| **reg-suit** | Self-host S3/GCS (publisher plugin) | git-graph parent commit (keygen plugin) | Implicit: next commit's snapshot becomes baseline; **no accept command** | Static HTML report; GitHub PR comment | x-img-diff-js (structural) |
| **Chromatic** | SaaS, server-side | Last approved build; per-**mode** baselines | Per-test approve in web app; modes approved separately | Web UI; TurboSnap skips unchanged | Proprietary (standardized browser) |
| **Argos** | SaaS (MIT, self-hostable) | git merge-base ancestor build | Per-build/test approve in UI; flaky auto-ignore | Web UI; auto-ignore + manual "Ignore" | Deterministic pixel diff + clustering |
| **Percy** | SaaS | Last approved on branchline | Snapshot-granular approve; carried forward per branch | Web review UI | Proprietary |
| **BackstopJS** | Local FS | Path/scenario name | `backstop approve` overwrites refs | HTML report w/ scrubber | ResembleJS |
| **jest-image-snapshot** | Local FS (`__image_snapshots__`) | Test/file name | `jest -u` | None (CI fail + diff PNG) | pixelmatch (default) / ssim |
| **odiff / pixelmatch** | n/a (engine only) | n/a | n/a | n/a | YIQ-NTSC + antialiasing |

## Takeaways for Buiy

1. reg-suit's **keygen+publisher plugin split** is the cleanest OSS pattern for "object storage keyed by commit," but its lack of a durable accept ledger is the warning — Buiy needs an explicit accept command that writes the digest into the baseline set.
2. Chromatic "modes" make explicit that **baseline count = stories × viewport × theme × locale × browser**; Buiy's reftest-first strategy is partly a hedge against this multiplication.
3. **odiff** is the engine to wrap (or re-implement in Rust SIMD) if Buiy builds Tier-5 goldens.
4. Argos's **"minimum occurrences in 7 days → auto-ignore"** is the most concrete prior art for taming flaky-golden noise (Gold has no such auto-mechanism).

**Unverified:** all SaaS dollar figures and snapshot quotas (vendor pages / secondary comparisons only). (odiff's speedup figures are verified from the README's hyperfine `relative` column — 6.67×–7.65× on Cypress, 5.24×–5.50× on 8K — and are *not* in the unverified set; a generic "8× faster" claim does not appear in the README.)

## Sources

- reg-suit: https://github.com/reg-viz/reg-suit · keygen: https://github.com/reg-viz/reg-suit/blob/master/packages/reg-keygen-git-hash-plugin/README.md · reg-cli: https://github.com/reg-viz/reg-cli · x-img-diff-js: https://github.com/reg-viz/x-img-diff-js
- Chromatic Modes: https://www.chromatic.com/docs/modes/ · TurboSnap: https://www.chromatic.com/docs/turbosnap/ · pricing (unverified): https://www.chromatic.com/pricing
- Argos: https://argos-ci.com/docs/llms-full.txt · GitHub integration: https://argos-ci.com/docs/github · LICENSE (MIT): https://github.com/argos-ci/argos/blob/main/LICENSE
- Percy approval: https://www.browserstack.com/docs/percy/build-results/approval · baseline overview: https://www.browserstack.com/docs/percy/visual-testing-workflows/baseline-management/overview
- BackstopJS: https://github.com/garris/BackstopJS/blob/master/README.md
- jest-image-snapshot: https://www.npmjs.com/package/jest-image-snapshot
- odiff: https://github.com/dmtrKovalenko/odiff/blob/main/README.md
- vizzly comparison (secondary, unverified $): https://vizzly.dev/visual-testing-tools-comparison/
