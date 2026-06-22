**Date:** 2026-06-18
**Status:** active
**Subject:** Glossary — Flutter & Jetpack Compose semantics + test/automation terms used across this prior-art folder

# Glossary

Concise definitions of the terms that recur across this folder, grouped by side
(Flutter, Compose, shared/thesis). 1–2 lines each. See [`README.md`](./README.md)
for the folder map; deeper treatments live in
[`flutter-semantics.md`](./flutter-semantics.md),
[`compose-semantics.md`](./compose-semantics.md),
[`transports.md`](./transports.md), and
[`one-tree-n-consumers.md`](./one-tree-n-consumers.md).

---

## Flutter / Dart

**SemanticsNode** — A node in Flutter's framework-owned semantics tree: the
condensed accessibility/automation view of the render tree (role/flags, label,
value, rect, available actions). One node may merge several render objects. This
is the tree assistive tech, tests, and tooling all read.

**SemanticsProperties** — The data object attached via the `Semantics` widget to
describe a subtree's role, label, value, flags, and action callbacks. The
authoring surface that becomes `SemanticsNode` data.

**SemanticsAction** (`dart:ui`) — Enum of actions a node can expose for an
external actor to invoke. Constants as of Flutter 3.44 include `tap`, `longPress`,
`scrollLeft/Right/Up/Down`, `increase`, `decrease`, `showOnScreen`, cursor moves,
`setSelection`, `setText`, `copy`, `cut`, `paste`, focus gain/loss, `customAction`,
`dismiss`, `scrollToOffset`, `collapse`, `expand`, `focus` — not an exhaustive
list pinned to one version; see [`flutter-semantics.md`](./flutter-semantics.md).
Canonical doc URL is the `-class.html` form.

**SemanticsActions** — The per-node record of which `SemanticsAction` values a
given `SemanticsNode` currently supports. (Compose has a same-named concept; see
below.)

**performAction / performSemanticsAction** — Invoke a `SemanticsAction` against a
node. In `flutter_test`, `SemanticsController.performAction(finder, action, {args})`
locates a node by a `Finder` and dispatches the action; this is the inbound /
control direction (the same path assistive tech uses to drive the app). It is
fire-and-forget — `checkForAction` only asserts the action is *advertised*, not
that it succeeded (see acknowledgement, below).

**SemanticsController** (`flutter_test`) — Test-side handle reached via
`tester.semantics`. Exposes `find`, `performAction`, `simulatedAccessibilityTraversal`,
and convenience wrappers (`tap`, `longPress`, `copy`, `paste`, `setText`,
`scrollUp/Down`, `increase`, etc.). Lets a widget test read and drive the
semantics tree directly.

**simulatedAccessibilityTraversal** — `SemanticsController` method that walks the
currently visible semantics tree "as if by assistive technologies," returning an
`Iterable<SemanticsNode>` (optional start/end `Element`/`SemanticsNode` bounds).
The read / perception direction — it yields the screen-reader-order view of the UI.

**pumpAndSettle** — `WidgetTester.pumpAndSettle()` repeatedly pumps frames until
no frames are scheduled (or a timeout). The Flutter "the UI has settled" primitive
a test calls *between* an action and reading its effect, so it never observes a
stale mid-animation tree. Times out on indefinite work (infinite animations). The
Compose analog is `awaitIdle`/`waitForIdle`. See
[`open-problems.md`](./open-problems.md) §6.

**flutter_driver** — Legacy E2E framework. Runs in a **separate process** and
drives the app over the VM service protocol; finders include `byValueKey` and
`bySemanticsLabel`. Superseded by `integration_test`. (The official migration page
frames it as a migration target and **does not use the word "deprecated"**;
sources describe it as legacy/migration-target. A stronger "removed in 3.44"
claim is **(unverified)**.) See [`transports.md`](./transports.md).

**integration_test** — Modern, Google-recommended replacement for `flutter_driver`.
Runs **in-process**, reusing widget-test finders and `WidgetController`
(`tap`, `enterText`, `scrollUntilVisible`, …). See [`transports.md`](./transports.md).

**WidgetController** — Base controller (subclassed by `WidgetTester`) for finding
and interacting with widgets in widget/integration tests — `tap`, `enterText`,
`drag`, `scrollUntilVisible`, etc. (Full signature list beyond these is
**(unverified)** here — cite api.flutter.dev.)

**Finder** — An object that locates widgets/elements in the tree for a test to act
on (`find.text`, `find.byKey`, `find.bySemanticsLabel`, …). The lookup primitive
that `WidgetController` and `SemanticsController` operate against.

**ValueKey** — A `Key` carrying a value used to give a widget a stable identity
across rebuilds; `find.byKey(ValueKey('x'))` (and `flutter_driver`'s `byValueKey`)
target it — the closest Flutter analogue to a test id.

**VM Service Protocol** — The Dart VM's JSON-RPC 2.0 API over a WebSocket. The
out-of-process transport that backs DevTools, the Widget Inspector, and
`flutter_driver`; extensible via service extensions. The endpoint is a
`ws://127.0.0.1:PORT/<token>/ws` URL whose path token is a per-run secret. See
[`transports.md`](./transports.md).

**Service extension** — A method registered on the VM service under a namespace
(e.g. `ext.flutter.inspector.*`) that exposes app/framework introspection and
commands to external tools. Exact current method names should be cited from docs,
not memory.

**Widget Inspector** — The DevTools tooling consumer that visualizes the widget /
render / semantics trees and supports select-on-device; it reads the framework's
trees over the VM service rather than maintaining its own.

**Dart & Flutter MCP server** — Official, **experimental** MCP server
(`dart-lang/ai`, `pkgs/dart_mcp_server`; requires Dart 3.9+). Among code/test/pub
tools it can **introspect and interact with the running app's widget tree** — the
LLM-agent consumer of the same framework trees. License **(unverified)** SPDX
(assumed BSD-3-Clause family). See [`flutter-semantics.md`](./flutter-semantics.md).

---

## Jetpack Compose / Android

**ComposeTestRule** — JUnit test rule (`createComposeRule()` /
`createAndroidComposeRule()`) that hosts Compose content and exposes finders
(`onNode`, `onAllNodes`), the semantics-tree dump (`printToLog`), and frame/clock
control. The entry point for Compose UI tests.

**SemanticsNodeInteraction** — The handle a finder returns, representing one node
in the semantics tree. Chains assertions (`assert*`, `assertIsDisplayed`) and
actions (`performClick`, `performTextInput`, `performScrollTo`,
`performSemanticsAction`).

**SemanticsProperties** (Compose) — Keys for the descriptive semantics a node
carries (text, contentDescription, role, stateDescription, …). Set via
`Modifier.semantics { }`.

**SemanticsActions** (Compose) — Keys for the invocable actions a node exposes
(`OnClick`, `SetText`, `ScrollBy`, …); `performSemanticsAction` dispatches them —
the Compose analogue to Flutter's `SemanticsAction`. Each callback returns a
`Boolean` (handled/success), the one structured action-result signal either
framework offers; see acknowledgement, below.

**awaitIdle / waitForIdle** — `ComposeTestRule.waitForIdle()` / `awaitIdle()`
block until composition, layout, and (by default) the `mainClock` are idle. The
Compose "the UI has settled" primitive — the analog of Flutter `pumpAndSettle`.
Sharper under the v2 `StandardTestDispatcher` default, where queued coroutines
must be advanced/awaited before perceiving. See [`open-problems.md`](./open-problems.md) §6.

**Merged vs. unmerged tree** — Compose maintains two views: the **merged** tree
collapses a `mergeDescendants` subtree (e.g. a button + its text) into one node;
the **unmerged** tree shows every node separately. Matchers operate on the
**merged** tree by default. See [`compose-semantics.md`](./compose-semantics.md).

**useUnmergedTree** — Finder parameter (default `false`) that opts a query into the
unmerged tree, e.g. `onNodeWithText("World", useUnmergedTree = true)` to reach a
child hidden by merging.

**testTag** — `Modifier.testTag("id")` attaches a test-only identifier in the
semantics; `hasTestTag(...)` / `onNodeWithTag(...)` find it. The Compose stable
test id (does not affect accessibility by itself).

**testTagsAsResourceId** — Semantics property that surfaces `testTag` values as
Android view resource IDs, so out-of-process tools (UI Automator, Espresso) can
target Compose nodes via `AccessibilityNodeInfo`. Reportedly available since
Compose **1.2.0-alpha08+** (third-party source — verify against the primary
changelog before stating the exact version).

**AccessibilityNodeInfo** — The Android platform accessibility node object
(`android.view.accessibility`). Compose exports its semantics tree into these so
TalkBack, UI Automator, and Espresso can read/act on Compose UIs through the
standard OS accessibility bridge.

**UI Automator** — Android's cross-app, out-of-process E2E test framework that
drives the device via the accessibility layer (`AccessibilityNodeInfo`); reaches
Compose nodes when `testTagsAsResourceId` is enabled.

**Layout Inspector** — Android Studio tooling that inspects a running app's view /
Composable hierarchy, including Compose semantics — the Compose-side tooling
consumer, parallel to Flutter's Widget Inspector.

---

## Shared / thesis terms

**One tree, N consumers** — Folder thesis: each framework owns a **single**
semantic tree consumed simultaneously by (a) OS accessibility, (b) test
automation, (c) tooling/inspectors, and increasingly (d) LLM agents — rather than
maintaining a separate API per consumer. The load-bearing analogy to Buiy's
AccessKit tree. An architectural observation, not a version claim. See
[`one-tree-n-consumers.md`](./one-tree-n-consumers.md) and the cross-cutting
takeaways in [`lessons.md`](./lessons.md) and [`open-problems.md`](./open-problems.md).

**Action acknowledgement / result channel** — The question of how a consumer
learns whether a fired action *succeeded* and *what changed*. Flutter's
`performAction` is fire-and-forget; Compose actions return a handled-`Boolean`;
neither describes the resulting state change, so the durable answer is
re-perceiving the (settled) tree. See [`open-problems.md`](./open-problems.md) §7.

**Settle / quiescence** — The "when is the tree safe to read after an action"
primitive: Flutter `pumpAndSettle`, Compose `awaitIdle`. A perceive→act→perceive
agent loop needs an equivalent or it reads a stale tree. See
[`open-problems.md`](./open-problems.md) §6.

## Sources

- https://api.flutter.dev/flutter/flutter_test/SemanticsController-class.html
- https://api.flutter.dev/flutter/flutter_test/WidgetTester/pumpAndSettle.html
- https://api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html
- https://docs.flutter.dev/release/breaking-changes/flutter-driver-migration
- https://developer.android.com/develop/ui/compose/testing/apis
- https://developer.android.com/reference/kotlin/androidx/compose/ui/test/package-summary
- https://composables.com/jetpack-compose/androidx.compose.ui/ui/objects/SemanticsActions/api
- https://docs.flutter.dev/ai/mcp-server
- https://github.com/dart-lang/ai/tree/main/pkgs/dart_mcp_server
