**Date:** 2026-06-14
**Status:** active
**Subject:** Flutter framework golden infrastructure — how `flutter_goldens` wires `matchesGoldenFile` to Flutter Gold (Skia Gold), the per-PR triage workflow, scale, and the verbatim warts

# Flutter framework golden infrastructure (Flutter Gold)

Flutter's framework goldens are among the most heavily-exercised large-scale golden systems in open-source GUI toolkits. The framework itself does **not** use `LocalFileComparator` in CI — it moves the source of truth to a Skia Gold instance (**Flutter Gold**) so that comparison happens in an external, multi-positive service rather than as a zero-tolerance byte-compare on the test machine. (For the Skia Gold service model in depth, see the sibling [`skia-gold`](../skia-gold/README.md) prior-art folder; this file covers Flutter's *use* of it.)

## Wiring: how the comparator gets swapped

The public matcher `matchesGoldenFile(key, {version})` delegates to the ambient `goldenFileComparator` ([matchesGoldenFile API](https://api.flutter.dev/flutter/flutter_test/matchesGoldenFile.html)); the default for raw `flutter_test` is the no-op `TrivialComparator`, and for `flutter test` it is `LocalFileComparator` (see [matches-golden.md](matches-golden.md)).

The internal `flutter_goldens` package (in-tree at `packages/flutter_goldens/`, **not** on pub.dev) swaps in a Skia-Gold–backed comparator at test bootstrap. `testExecutable()` picks a `FlutterGoldenFileComparator` subclass "based on the current environment" ([flutter_goldens.dart](https://github.com/flutter/flutter/blob/master/packages/flutter_goldens/lib/flutter_goldens.dart)):

- **`FlutterPostSubmitFileComparator`** — uploads images to the Skia Gold dashboard via `goldctl`.
- **`FlutterPreSubmitFileComparator`** — "will always return true since golden file test failures are managed in pre-submit checks by the flutter-gold status check."
- **`FlutterSkippingFileComparator`** — skips on unsupported environments.
- **Local fallback** — requests baselines from Skia Gold for the current device.

The selection sniffs env vars — notably `SWARMING_TASK_ID` and `GOLDCTL` — running through Gold only on CI, excluding tryjob-only contexts, and historically gating on the main branch (surfaced from [PR #33688 "Part 1: Skia Gold Testing"](https://github.com/flutter/flutter/pull/33688)). Under the hood, `SkiaGoldClient` shells out to Google's `goldctl` (`goldctl imgtest add`, authenticated with `--luci` on CI) to push images to the Gold instance ([DeepWiki testing-infra](https://deepwiki.com/flutter/flutter/5.3-engine-versioning-and-artifacts)).

Baselines are **not** checked into the repo as PNGs the way third-party setups do — they live in Gold, mirrored locally under `bin/cache/pkg/skia_goldens/...`.

*Unverified:* the precise current env-var gating predicate (`SWARMING_TASK_ID`/`GOLDCTL`, main-branch-only) is drawn from search summaries of PR #33688 and the in-tree comparator, not a line-by-line read of current `flutter_goldens.dart`. Treat the exact predicate as approximate.

## The per-PR triage workflow

Every framework PR that touches golden tests runs them and diffs against Gold. The `flutter-gold` PR check "is applied to pull requests in flutter/flutter that execute golden file tests and are ready for review." On any image delta it ([Writing-a-golden-file-test wiki](https://github.com/flutter/flutter/blob/master/docs/contributing/testing/Writing-a-golden-file-test-for-package-flutter.md)):

- "will hold a pending state,"
- Gold leaves a PR comment linking the image results in its [ChangeLists dashboard](https://flutter-gold.skia.org/changelists), and
- a human in the `flutter-hackers` group must triage (approve) each new image before the check "go[es] green within five minutes."

Unapproved images that land trigger a post-submit error: *"Skia Gold received an unapproved image in post-submit testing."* Gold's content-addressed model means an already-triaged pixel hash is auto-approved forever, which is what makes per-PR triage tractable at framework scale.

The **engine** has its own separate instance at `flutter-engine-gold.skia.org`, with `dart:ui`-only pixel tests ([issue #76565](https://github.com/flutter/flutter/issues/76565)).

## The many-positives model — the design answer to zero-tolerance

The wiki concedes goldens run for "Linux, Mac, Windows, and Web platforms. It is common for there to be slight differences between them," requiring "multiple golden masters for a given test." Gold's triage UI tolerates *multiple accepted masters per test* to absorb cross-platform rendering differences — this is the direct design answer to `LocalFileComparator`'s zero-tolerance flake: move the source of truth to a server that holds many approved variants, instead of demanding one byte-exact file.

Threshold tolerance is a partial mitigation too: the engine/Impeller pixel harness passes when "less than 1% of pixels are different by less than 4 color component deltas" ([engine PR #40824](https://github.com/flutter/engine/pull/40824)). The two knobs that express this are `maxDiffPixelsPercent` (the fraction of pixels allowed to differ) and `pixelColorDelta` (the max per-channel color delta a differing pixel may have) — a two-axis budget worth borrowing for a fuzzy comparator (see [lessons.md](lessons.md) `## Borrow`). *Note:* that threshold is the engine/Impeller harness, not the framework widget goldens, which remain effectively exact-match modulo Gold triage.

## Determinism: the Ahem font

The load-bearing trick that keeps these goldens stable is the obscure-text **Ahem** font: "the Flutter framework uses a font called 'Ahem' which shows squares instead of characters" — every glyph a solid box filling the em square ("black spaces for every character and icon"). This removes per-platform font-rasterization variance from any golden that isn't specifically testing glyph rendering, making the *layout* deterministic across OSes. Full treatment in [obscure-text-font.md](obscure-text-font.md).

## Verbatim warts

- **Docker doesn't fully save you (#131559, Open, P2).** Even with one *identical Ubuntu Docker image*, generating on a Windows host and verifying on a Mac host yields *"a random smattering of mismatched pixels"* ranging *"from single pixels to 30-90 pixel mismatches"* (matthew-carroll, 2023-07-29). Host-OS rasterization leaks through the container.
- **Flaky goldens get skipped, and skipping blinds you.** "When we skip a test we stop sending to Skia Gold entirely," so there is "no way to verify flaky golden test fixes" short of speculatively un-skipping — "The cost of a mistake is closed tree, P0s, wasted time, and other sadness" (yjbanov, 2022-09-10, [#111325](https://github.com/flutter/flutter/issues/111325)).
- **Force-push leaves the check stuck.** A known Skia Gold issue leaves the `flutter-gold` check stuck pending after `git push -f`; the only remedy is *"Try rebasing again. This side-effect is flaky"* (wiki, verbatim).

*Unverified:* the exact verbatim comment text of #111325 could not be loaded from a primary fetch beyond the quoted fragments; re-check against the live thread before treating as exact.

## Implications for Buiy

Flutter Gold is the cautionary "what Buiy does *not* build." It is a hosted Google-Cloud service; for a local-first library it is both the wrong dependency and a documented flake source (#131559, #111325). The reusable wins are the *determinism knobs and the curated-accept discipline*, not the pixel-diff plumbing. Concretely: commit small box-font goldens to the repo (deterministic → tiny diffs), keep a host-pinned local rasterizer for the irreducible suite, and generalize Buiy's existing `BUIY_ACCEPT_SHAPING` curated-accept gate to all snapshot tiers — Flutter's `--update-goldens` + human pre-submit triage is the precedent for that explicit-accept model. See [lessons.md](lessons.md).

## Sources

- `matchesGoldenFile` API: https://api.flutter.dev/flutter/flutter_test/matchesGoldenFile.html
- flutter_goldens.dart: https://github.com/flutter/flutter/blob/master/packages/flutter_goldens/lib/flutter_goldens.dart
- PR #33688 "Part 1: Skia Gold Testing": https://github.com/flutter/flutter/pull/33688
- DeepWiki Flutter testing-infra: https://deepwiki.com/flutter/flutter/5.3-engine-versioning-and-artifacts
- Writing-a-golden-file-test wiki: https://github.com/flutter/flutter/blob/master/docs/contributing/testing/Writing-a-golden-file-test-for-package-flutter.md
- flutter/flutter#131559 (Docker rasterization leak): https://github.com/flutter/flutter/issues/131559
- flutter/flutter#111325 (no way to verify flaky golden fixes; force-push pending): https://github.com/flutter/flutter/issues/111325
- flutter/flutter#76565 (separate engine Gold instance): https://github.com/flutter/flutter/issues/76565
- flutter/engine#40824 (1% / 4-component-delta threshold): https://github.com/flutter/engine/pull/40824
- Skia Gold service model: [`docs/prior-art/skia-gold/`](../skia-gold/README.md)
