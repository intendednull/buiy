**Date:** 2026-06-18
**Status:** active
**Subject:** Retained-mode semantics automation (Flutter + Jetpack Compose) — folder index, key facts, and reading order

# Retained-mode semantics automation — Flutter & Jetpack Compose

This folder collects the external prior art on **retained-mode UI frameworks that expose a single framework-owned semantic tree and reuse it for many consumers** — OS accessibility, test automation, design-time tooling, and (increasingly) LLM agents. The two systems documented in depth are **Flutter** and **Jetpack Compose**: both keep a retained widget/composition tree, own their own renderer, and project a *separate* semantics tree out of it. That semantics tree is the load-bearing object — it is simultaneously the a11y surface a screen reader walks, the surface an automated test drives, and the surface a debugger/inspector visualizes. This folder exists to answer one question for Buiy: *what does the field already know about treating the semantic tree as a unified inspection/automation/agent surface, and how did two mature frameworks structure the action vocabulary, the finders, the test harness, and the out-of-process transport that hang off it?*

Evidence files stay factual and version-pinned. The Buiy-specific verdicts live only in [`lessons.md`](lessons.md). This README is an index, not a deep dive.

_(See also [`../flutter-golden-testing/`](../flutter-golden-testing/) for the Flutter testing-**determinism** angle — goldens, the Ahem font, `debugDisableShadows`. That folder and this one are different subjects: golden-image determinism there, semantic-tree automation here.)_

## Key facts

| System | Semantic model | Action primitive | Test surface | Out-of-process transport | Maintainer / license |
|---|---|---|---|---|---|
| **Flutter** (a11y/test) | `SemanticsNode` tree built from `SemanticsProperties` / `SemanticsConfiguration`; rendered from the retained widget→element→render tree | `SemanticsAction` enum (`dart:ui`) — `tap`, `scrollUp/Down/Left/Right`, `increase`/`decrease`, cursor + `setSelection`/`setText`, `copy`/`cut`/`paste`, `focus`, `dismiss`, `collapse`/`expand`, `scrollToOffset`, `customAction`, … | `flutter_test` `SemanticsController`: `simulatedAccessibilityTraversal` (walk visible tree as AT would) + `performAction` (fire a `SemanticsAction` on a `Finder`-located node). `integration_test` (in-process) reuses widget-test finders + `WidgetController` | **VM Service Protocol** = JSON-RPC 2.0 over WebSocket; `ext.flutter.inspector.*` service extensions back DevTools / Widget Inspector | Google · **BSD-3-Clause** ([`flutter/flutter`](https://github.com/flutter/flutter)) |
| **Dart & Flutter MCP server** | (consumes Flutter's running widget tree) | inherits Flutter's runtime introspection | n/a (agent layer) | MCP, wrapping VM-Service introspection of the *running* app's widget tree + runtime errors | Google (Dart/Flutter team) · BSD-3-Clause-family **(unverified** exact SPDX of `dart-lang/ai`**)** — status **EXPERIMENTAL** |
| **Jetpack Compose** | `SemanticsNode` tree from `SemanticsProperties` + `SemanticsActions`, set via `Modifier.semantics`; **merged** tree by default, `useUnmergedTree=true` opts into unmerged | `SemanticsActions` (e.g. `OnClick`, `SetText`, `ScrollBy`, `OnImeAction`) fired via `performSemanticsAction` | `ComposeTestRule` + `SemanticsNodeInteraction` finders (`onNodeWithText` / `…ContentDescription` / `…Tag`) + ops (`performClick`, `performTextInput`, `performScrollTo`, `assert*`) | **`Modifier.testTag` + `testTagsAsResourceId`** surfaces test tags as Android resource IDs into `AccessibilityNodeInfo` → reachable by UI Automator / Espresso out-of-process | Google (AndroidX) · **Apache-2.0** ([`androidx/androidx`](https://github.com/androidx/androidx)) |

Version pins (as of **2026-06-18**; patch numbers are the highest-risk figures — re-verify before citing):

- **Flutter** stable **3.44.0**, released **2026-05-18** ([release notes](https://docs.flutter.dev/release/release-notes/release-notes-3.44.0); date corroborated by the [flutter-announce post](https://groups.google.com/g/flutter-announce/c/9KeO2B20yuU) and [release issue #186410](https://github.com/flutter/flutter/issues/186410)). The exhaustive `SemanticsAction` constant list reflects this version; an older Flutter pin must re-check which constants existed then (`collapse`/`expand`/`focus`/`scrollToOffset` are newer additions).
- **`androidx.compose.ui` / `ui-test`** stable **1.11.x** (1.11.0 stable **2026-04-22**, April '26 BOM `2026.04.01`; latest patch ~**1.11.2**). **In 1.11.0 the v2 testing APIs became the default and the v1 testing APIs were deprecated** — matchers/finders are unchanged, but the v2 `create*ComposeRule` APIs switch the default test dispatcher to `StandardTestDispatcher` (queued coroutines vs. the v1 `UnconfinedTestDispatcher`). See [`compose-semantics.md`](compose-semantics.md).
- **Dart & Flutter MCP server** — repo [`dart-lang/ai` `pkgs/dart_mcp_server`](https://github.com/dart-lang/ai/tree/main/pkgs/dart_mcp_server), [docs](https://docs.flutter.dev/ai/mcp-server). Docs state verbatim it "is experimental and likely to evolve quickly." Requires **Dart 3.9+** (a Flutter-version requirement was also cited but is lower-confidence). Whether it has graduated past "experimental" since the docs snapshot is **(unverified)**. Detail in [`transports.md`](transports.md).

## How to use this folder

_Framing disclosure (added at finalize): this folder was assembled as prior-art research for Buiy's agent-interface design, written from Buiy's AccessKit-semantic-tree-first agent-surface stance. It is a learn-from artifact, not a neutral catalog: the selection of what to document and the "Implications for Buiy" framings are oriented around the thesis that Buiy's already-authored AccessKit tree is the right bidirectional LLM-agent perception+control surface. Buiy-specific judgments are isolated in [`lessons.md`](lessons.md) as validates/borrow/avoid; Buiy design decisions are NOT baked into the evidence files._

Start at this README, then follow the reading order below. Use the [glossary](glossary.md) for unfamiliar terms. When consulting this folder during spec/plan/review work, treat each evidence file as a launchpad for fresh online research rather than a frozen snapshot — re-verify any version number before citing it, since this area (especially the MCP layer) moves fast.

## Table of contents

- [`README.md`](README.md) — this index.
- [`flutter-semantics.md`](flutter-semantics.md) — Flutter's `SemanticsNode`/`SemanticsConfiguration` model, the full `SemanticsAction` vocabulary, and the `flutter_test` `SemanticsController` (`simulatedAccessibilityTraversal` + `performAction`) plus `integration_test` vs. legacy `flutter_driver`.
- [`compose-semantics.md`](compose-semantics.md) — Jetpack Compose's `SemanticsProperties`/`SemanticsActions`, the merged-vs-unmerged tree, `ComposeTestRule` finders/ops, `testTag`/`testTagsAsResourceId`, and the 1.11 v1→v2 testing-API shift.
- [`transports.md`](transports.md) — the out-of-process layer: Flutter's VM Service Protocol (JSON-RPC over WebSocket, `ext.flutter.inspector.*`), the Dart & Flutter MCP server (experimental widget-tree introspection), and Compose's `AccessibilityNodeInfo`/UI-Automator bridge via `testTagsAsResourceId`.
- [`one-tree-n-consumers.md`](one-tree-n-consumers.md) — the architectural thesis: one framework-owned semantic tree consumed by OS a11y, test automation, tooling, and now LLM agents — distilled from how both frameworks structure it.
- [`open-problems.md`](open-problems.md) — unsettled questions both frameworks expose: merged/unmerged ambiguity, tree-staleness during async settle (pump-and-settle / await-idle), action acknowledgement / result channel, in-process vs. out-of-process trade-offs, agent-surface security in always-live builds.
- [`lessons.md`](lessons.md) — **Buiy-facing** verdicts: validates / borrow / avoid.
- [`glossary.md`](glossary.md) — terms used across the folder.

## Glossary

Term definitions (`SemanticsNode`, `SemanticsAction`/`SemanticsActions`, `SemanticsConfiguration`, merged vs. unmerged tree, `Finder` / `SemanticsNodeInteraction`, `testTag` / `testTagsAsResourceId`, VM Service Protocol, `simulatedAccessibilityTraversal`, pump-and-settle / await-idle, in-process vs. out-of-process driver, `AccessibilityNodeInfo`) live in [`glossary.md`](glossary.md).

## Canonical reading order

1. [`flutter-semantics.md`](flutter-semantics.md) — the most fully developed example of a semantic-action vocabulary plus a test harness built directly on the a11y tree.
2. [`compose-semantics.md`](compose-semantics.md) — the same pattern in a different ecosystem; introduces merged/unmerged and the resource-id bridge.
3. [`transports.md`](transports.md) — how each framework exposes the tree *out of process* (and how the MCP agent layer is bolted on).
4. [`one-tree-n-consumers.md`](one-tree-n-consumers.md) — pull the pattern together: one tree, N consumers.
5. [`open-problems.md`](open-problems.md) — the hard parts both frameworks leave unsolved.
6. [`lessons.md`](lessons.md) — translate it for Buiy.

## Why this matters for Buiy

Flutter and Compose are the **closest analogs to Buiy that exist**: a retained widget tree, a framework-owned renderer, and a *separate* semantic tree projected from it — exactly Buiy's shape. They are the field's strongest proof of the **one-tree-N-consumers** pattern: the *same* semantics tree is the a11y surface a screen reader walks, the test surface an automated driver fires actions against, and the tooling surface an inspector visualizes — and now, via the Dart & Flutter MCP server's running-widget-tree introspection, the **agent surface** too. Crucially, Buiy's [AccessKit](../accesskit/) tree is the **Rust-native sibling** of `SemanticsNode`/`SemanticsProperties`: same role+name+state+actions model, same projected-from-retained-tree architecture. What Flutter proves with `SemanticsController.performAction` and Compose with `performSemanticsAction` — that firing a semantic action by node reference *is* a complete control surface — is precisely the bidirectional half Buiy's currently output-only AccessKit tree is missing. Whether and how Buiy adopts this is decided in [`lessons.md`](lessons.md), not here.

## Sources

- https://github.com/flutter/flutter
- https://github.com/flutter/flutter/blob/master/LICENSE
- https://docs.flutter.dev/release/release-notes/release-notes-3.44.0
- https://groups.google.com/g/flutter-announce/c/9KeO2B20yuU
- https://github.com/flutter/flutter/issues/186410
- https://api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html
- https://api.flutter.dev/flutter/flutter_test/SemanticsController-class.html
- https://api.flutter.dev/flutter/flutter_test/SemanticsController/simulatedAccessibilityTraversal.html
- https://docs.flutter.dev/ai/mcp-server
- https://github.com/dart-lang/ai/tree/main/pkgs/dart_mcp_server
- https://developer.android.com/jetpack/androidx/releases/compose-ui
- https://developer.android.com/blog/posts/whats-new-in-the-jetpack-compose-april-26-release
- https://developer.android.com/reference/kotlin/androidx/compose/ui/test/package-summary
- https://developer.android.com/develop/ui/compose/testing/apis
