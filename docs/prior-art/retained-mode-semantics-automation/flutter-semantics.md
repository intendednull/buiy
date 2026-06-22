**Date:** 2026-06-18
**Status:** active
**Subject:** Flutter — the Semantics tree (SemanticsNode), the SemanticsAction vocabulary, and the test/inspect/agent surfaces that ride it

Flutter ships a single, framework-owned **Semantics tree** that backs accessibility,
widget testing, tooling inspection, and (now) LLM-agent introspection. This is the
clearest production instance of the "one tree, N consumers" thesis these folders
serve — see [one-tree-n-consumers.md](one-tree-n-consumers.md). Compose's parallel is
in [compose-semantics.md](compose-semantics.md); the Buiy analogue (an AccessKit tree
that already exists but is output-only) is drawn out in [lessons.md](lessons.md) and
in [../accesskit/](../accesskit/).

Maintainer: Google. Repo: `flutter/flutter`. License: BSD-3-Clause ("Copyright 2014
The Flutter Authors"). Current stable Flutter at write time: **3.44.0** (released
2026-05-18 per the release notes and the flutter-announce post — treat the exact
patch and date as the highest-risk figures here).

## The Semantics tree

`RenderObject`s contribute semantics via a `SemanticsConfiguration`; the framework
compiles these into a tree of **`SemanticsNode`** objects (one node per semantic
boundary, not one per widget). Each node carries a role/flag set, a
label/value/hint/increased-decreased-value, geometry, and a set of supported
**actions**. This is the same data Flutter serializes across the platform a11y
bridges (Android `AccessibilityNodeInfo` / TalkBack, iOS UIAccessibility / VoiceOver,
web ARIA). The node graph — not the widget tree — is what assistive tech and the test
framework actually traverse, which is exactly why one tree can serve both.

## `SemanticsAction` — the control vocabulary

`SemanticsAction` (in `dart:ui`) is the closed enum of things a consumer can *ask* a
node to do. The full constant set as of 3.44, verified against
`api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html` (note the canonical doc
is the `-class.html` URL; the `SemanticsAction.html` form 404s):

`tap`, `longPress`, `scrollLeft`, `scrollRight`, `scrollUp`, `scrollDown`,
`scrollToOffset`, `increase`, `decrease`, `showOnScreen`,
`moveCursorForwardByCharacter`, `moveCursorBackwardByCharacter`,
`moveCursorForwardByWord`, `moveCursorBackwardByWord`, `setSelection`, `setText`,
`copy`, `cut`, `paste`, `didGainAccessibilityFocus`, `didLoseAccessibilityFocus`,
`customAction`, `dismiss`, `collapse`, `expand`, `focus`.

`collapse`, `expand`, `focus`, and `scrollToOffset` are comparatively recent
additions; the set is not frozen and a project pinning to an older Flutter must
re-check which constants existed then. A node advertises the subset it supports; a
consumer invokes one through **`performAction`** (with optional args, e.g. the
selection range for `setSelection` or the offset for `scrollToOffset`). The same
perform-action path is driven by the OS a11y service *and* by the test framework —
there is no separate "test command" channel.

`customAction` is the escape hatch: an app declares named custom actions (each a
`CustomSemanticsAction` with an id), and a consumer triggers one by id through
`customAction`. This is the analogue of AccessKit's custom-action surface — relevant
to Buiy because an agent vocabulary larger than the standard verbs needs exactly this
kind of open-ended slot.

## `SemanticsController` — traversal + action from a test

`flutter_test` exposes the tree to tests via **`SemanticsController`** (verified at
`api.flutter.dev/flutter/flutter_test/SemanticsController-class.html`). Two methods
matter for the thesis:

- **`simulatedAccessibilityTraversal({start, end, startNode, endNode, view})`** —
  "Simulates a traversal of the currently visible semantics tree as if by assistive
  technologies," returning an `Iterable<SemanticsNode>` **in the order an AT would
  visit them** (optionally bounded by start/end Elements or SemanticsNodes). This is
  the load-bearing primitive: a test reads the world in the *same ordered shape* a
  screen reader would, so a11y order and test order cannot silently diverge.
- **`performAction(finder, action, {args, checkForAction = true})`** — performs a
  `SemanticsAction` on the `SemanticsNode` located by a `Finder`. `checkForAction`
  asserts the node actually advertises the action before firing. Note `performAction`
  is **fire-and-forget**: the assertion is that the action was *advertised*, not that
  it *succeeded* or that any resulting state change has settled. The freshness/result
  question is treated in [open-problems.md](open-problems.md).

On top of these sit convenience wrappers that mirror the action enum: `tap`,
`longPress`, `scrollUp`/`Down`/`Left`/`Right`, `increase`/`decrease`, `setText`,
`setSelection`, `copy`/`cut`/`paste`, `moveCursorForward/BackwardByCharacter/ByWord`,
`didGainAccessibilityFocus`/`didLoseAccessibilityFocus`, `showOnScreen`, `dismiss`,
`customAction`, and `find()` (Element finder → SemanticsNode).

## Two test surfaces: `flutter_driver` vs `integration_test`

Flutter has two automation surfaces, and the architectural difference is the point:

- **`flutter_driver`** — the legacy surface. It runs in a **separate process** and
  drives the app over the service protocol, locating widgets with `Finder`s such as
  `byValueKey` and `bySemanticsLabel`. Google steers new work to `integration_test`;
  the official ["Migrating from flutter_driver"](https://docs.flutter.dev/release/breaking-changes/flutter-driver-migration)
  page is filed under breaking changes and frames it as the thing you migrate *away*
  from. **That page does not use the literal word "deprecated" (or "removed")** — it
  is framed purely as a migration target. Whether `flutter_driver` is formally
  removed vs merely legacy in 3.44 is **(unverified)**; sources describe it as
  legacy/migration-target, not deleted.
- **`integration_test`** — the modern replacement. It runs **in-process**, reusing the
  widget-test finders and `WidgetController` (so `tester.tap`, `tester.enterText`,
  `tester.scrollUntilVisible`, plus the `SemanticsController` above). Because it is
  in-process it can read the live Semantics tree directly rather than marshalling
  commands across a process boundary. (The full `WidgetController` method surface
  beyond `tap`/`enterText`/`scrollUntilVisible` was not exhaustively fetched here —
  cite `api.flutter.dev` for signatures.)

The in-process vs out-of-process split is the same axis explored for Buiy in
[transports.md](transports.md): an agent that lives inside the app's event loop sees a
consistent tree snapshot, where an external driver must serialize across a protocol.

## VM Service Protocol, the Widget Inspector, and DevTools

Underneath the out-of-process tooling is the **VM Service Protocol**: JSON-RPC 2.0
over a WebSocket. Flutter registers *service extensions* under the
`ext.flutter.inspector.*` namespace that back DevTools and the Widget Inspector.
These return widget/element/semantics data as JSON. A verified concrete example is
**`getRootWidgetTree`**, added in [flutter/flutter PR #150010](https://github.com/flutter/flutter/pull/150010)
("Add new WidgetInspector service extension: getRootWidgetTree") as a unifying
replacement for the older `getRootWidgetSummaryTree` /
`getRootWidgetSummaryTreeWithPreviews` extensions (callable with `isSummaryTree` /
`withPreviews` flags); DevTools uses it to fetch the tree iteratively to avoid stack
overflow on large trees. Other extensions in the family include `screenshot` and the
select-widget-on-device flow that powers "click a widget in the running app, jump to
its source." Exact current method names should be cited from the live
`WidgetInspectorService` reference rather than memory; the namespace and
JSON-RPC/WebSocket transport are the stable facts.

The **DevTools Widget Inspector** is the human-facing consumer of these extensions —
it renders the widget/render/semantics trees and the on-device select overlay. Same
backend, different front end: the inspector and an automated driver pull from the
identical service-protocol surface.

## The Dart and Flutter MCP server (the agent consumer)

The newest consumer rides that same VM Service. The **"Dart and Flutter MCP server"**
(maintainer: the Dart/Flutter team at Google; repo `dart-lang/ai`, package
`pkgs/dart_mcp_server`; docs `docs.flutter.dev/ai/mcp-server`) exposes Dart/Flutter
dev actions to MCP-compatible AI clients.

It is explicitly **experimental**. The docs state verbatim: "The Dart and Flutter MCP
server is experimental and likely to evolve quickly." It requires **Dart 3.9 or
later** (one source also cited a Flutter 3.35+ requirement; that figure is
lower-confidence). Whether it has since graduated past "experimental" is
**(unverified)**.

Beyond static tooling (analyze/fix, run tests, `dart format`, pub.dev search +
dependency management, symbol/doc resolution), the agent-surface tie-in is **runtime
app interaction**: the docs describe tools to "introspect and interact with your
running application" — concretely, it "accesses the Flutter widget tree to understand
the layout" and "gets the current runtime errors from the running application" (the
overflow-fix example). So an LLM reaches the *same* framework-owned tree the inspector
and the test framework use, through the *same* VM Service transport. License: inherits
the `dart-lang/ai` repo license (Dart-norm BSD-3-Clause-family; the exact SPDX of
`dart_mcp_server` was not fetched — **(unverified)**).

## Why this matters for Buiy

Flutter validates the core bet: a single framework-owned semantic tree, with a closed
action vocabulary plus a custom-action escape hatch, can serve accessibility, testing,
tooling, *and* an LLM agent without a bespoke per-consumer surface — and the agent is
additive, layered on the existing transport rather than a new tree. The Buiy mapping
(AccessKit tree ↔ Semantics tree; AccessKit ActionRequest ↔ `performAction`;
bevy_winit channel ↔ VM Service) is developed in [lessons.md](lessons.md). Evidence
lives here; design decisions live there.

## Sources

- https://github.com/flutter/flutter
- https://github.com/flutter/flutter/blob/master/LICENSE
- https://docs.flutter.dev/release/release-notes/release-notes-3.44.0
- https://groups.google.com/g/flutter-announce/c/9KeO2B20yuU
- https://api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html
- https://api.flutter.dev/flutter/flutter_test/SemanticsController-class.html
- https://api.flutter.dev/flutter/flutter_test/WidgetController/scrollUntilVisible.html
- https://docs.flutter.dev/release/breaking-changes/flutter-driver-migration
- https://github.com/flutter/flutter/pull/150010
- https://api.flutter.dev/flutter/widgets/WidgetInspectorService-mixin.html
- https://docs.flutter.dev/ai/mcp-server
- https://github.com/dart-lang/ai/tree/main/pkgs/dart_mcp_server
