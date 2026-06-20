**Date:** 2026-06-18
**Status:** active
**Subject:** Flutter & Compose semantics automation — the two transport tiers (in-process test API vs out-of-process RPC) Buiy should mirror

# Two transport tiers

Both Flutter and Jetpack Compose ship the *same* semantic tree (see [flutter-semantics.md](./flutter-semantics.md), [compose-semantics.md](./compose-semantics.md)) to two structurally different classes of client over two different transports:

1. **In-process** — test code runs *inside* the app's process/isolate and calls the framework directly. No serialization, no socket; the test holds real references to nodes. Fastest, but only available to code co-resident with the app (i.e. tests).
2. **Out-of-process** — a separate process (DevTools, an IDE, a CI driver, an MCP server, an LLM agent) talks to the *running* app over a structured RPC. Pays a serialization + socket cost, but is the only path for tooling and agents that don't share the app's address space.

The recurring shape: **a stable structured RPC against the running app, plus a thin agent shim that re-exposes it.** This is the architecture Buiy should mirror (see [lessons.md](./lessons.md), [one-tree-n-consumers.md](./one-tree-n-consumers.md)).

## Tier 1 — In-process (tests, same isolate/process)

### Flutter
`flutter_test`'s `WidgetController` / `SemanticsController` are direct in-process APIs over the live tree. `SemanticsController.simulatedAccessibilityTraversal` walks the visible semantics tree (returns `Iterable<SemanticsNode>`); `SemanticsController.performAction` performs a `SemanticsAction` on a node located by a `Finder`. `integration_test` (the modern, in-process replacement for the legacy out-of-process `flutter_driver`) reuses these same widget-test finders + `WidgetController` (tap / enterText / scrollUntilVisible). The official migration page frames `flutter_driver` as a migration target and **does not use the literal word "deprecated"** (it says "migrate from"); whether `flutter_driver` is formally *removed* vs merely legacy in 3.44 is **(unverified)**. Source: [api.flutter.dev SemanticsController](https://api.flutter.dev/flutter/flutter_test/SemanticsController-class.html), [flutter-driver migration page](https://docs.flutter.dev/release/breaking-changes/flutter-driver-migration).

### Compose
`ComposeTestRule` + `SemanticsNodeInteraction` are direct method calls on the semantics tree from the same JVM process: finders (`onNodeWithText` / `onNodeWithContentDescription` / `onNodeWithTag`) and ops (`performClick` / `performTextInput` / `performScrollTo` / `performSemanticsAction` / `assert*`). No IPC. Matchers default to the **merged** semantics tree; `useUnmergedTree = true` opts into the unmerged tree. In `androidx.compose.ui` 1.11.0 (stable 2026-04-22) the v2 testing APIs became the default and the v1 testing APIs were deprecated — verify the exact v1→v2 wording against the compose-ui release notes before quoting. Source: [developer.android.com Compose testing](https://developer.android.com/develop/ui/compose/testing/apis).

In-process is the tier where you'd put **Buiy's own test harness** — a system that walks the AccessKit tree and dispatches `ActionRequest`s directly through the ECS, with no socket in the loop.

## Tier 2 — Out-of-process (tooling + agents + cross-process)

### Flutter — Dart VM Service Protocol
The out-of-process backbone is the **Dart VM Service Protocol**: JSON-RPC 2.0 carried over a **WebSocket**. In debug/profile mode the VM hosts a service web server; the advertised endpoint is a `ws://127.0.0.1:PORT/<token>/ws` URL (the path token is a per-run secret). Results come back as JSON. Framework-specific functionality is layered on as **service extensions** — methods namespaced `ext.flutter.*` (the Widget Inspector group is `ext.flutter.inspector.*`). Service extensions are *not* part of the core VM protocol; they are registered by `dart:ui`/Flutter and invoked by prepending the extension name to the RPC. Exact current inspector method names (e.g. `getRootWidgetTree`, screenshot, select-widget-on-device) are widely cited but should be taken from the primary service-extension reference, not memory.

A wrinkle worth noting for anyone implementing the client side: the **Dart Development Service (DDS)** wraps the raw VM service. Once DDS starts, clients connect to the DDS-advertised URI rather than the original VM service URI (which is hidden to discourage direct connections). DDS also brokers a **Dart Tooling Daemon (DTD)** URI (`ws://127.0.0.1:PORT/<token>`), an additional WebSocket layer that IDEs/tooling use. Sources: [Dart VM Service Protocol (service.md)](https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md), [DDS/DevTools/DTD wiki](https://github.com/flutter/flutter/wiki/Using-the-Dart-Development-Service-(DDS)-and-Flutter-DevTools-with-a-custom-Flutter-Engine-Embedding/e59dedf568832a42480f1bd21cac0154ade5c038).

**One socket, many consumers.** DevTools, the IDE Widget Inspector, and the Dart/Flutter MCP server all connect to this same VM-service/DTD socket against the running app, then re-expose their capabilities to *their* clients. That re-exposure is the thin shim.

### The MCP shim (Flutter)
The **"Dart and Flutter MCP server"** (maintainer: Dart/Flutter team, Google; repo `dart-lang/ai`, path `pkgs/dart_mcp_server`; license **(unverified)** SPDX, assumed BSD-3-Clause family) is the agent-facing shim. Docs state verbatim: *"The Dart and Flutter MCP server is experimental and likely to evolve quickly,"* and *"The following instructions require Dart 3.9 or later."* (A Flutter 3.35+ requirement is lower-confidence / **(unverified)**.)

Its transport is **two-sided**, illustrating the pattern exactly:
- **Downstream to the agent:** stdio. Docs: *"The Dart and Flutter MCP server can work with any MCP client that supports standard I/O (stdio) as the transport medium"* (e.g. `claude mcp add --transport stdio dart -- dart mcp-server`).
- **Upstream to the app:** it connects through the Dart Tooling Daemon to the running app's VM service. The `connect_dart_tooling_daemon` tool *"must be successfully called first before accessing the running app"*; then `get_widget_tree` retrieves *"the Flutter widget tree to understand the layout"* and another tool returns *"the current runtime errors from the running application."* Docs summarize the capability as *"Introspect and interact with your running application."*

So the agent surface is: **structured RPC into the running app (VM service / DTD) ←→ MCP server ←→ stdio to the LLM client.** Whether the MCP server has graduated past "experimental" since the docs snapshot is **(unverified)**. Source: [docs.flutter.dev/ai/mcp-server](https://docs.flutter.dev/ai/mcp-server).

### Compose / Android — out-of-process
Android's out-of-process story is more fragmented than Flutter's single socket:
- **Layout Inspector** (the tooling consumer) connects to *debuggable processes* on a device/emulator over the Android debug bridge (ADB). Docs: *"the Layout Inspector automatically connects to the debuggable processes running in the foreground of the connected device,"* showing a real-time Component Tree (per-composable) and Attributes panel for Compose / View / hybrid layouts. Source: [developer.android.com Layout Inspector](https://developer.android.com/studio/debug/layout-inspector).
- **UI Automator / Appium / Espresso** drive the app cross-process via the platform accessibility layer: the OS exposes each node as an `AccessibilityNodeInfo`, marshaled over Android's accessibility IPC. Compose surfaces its test tags into this channel via `Modifier.testTag` + the `testTagsAsResourceId` semantics property (it maps a test tag to an Android resource ID visible to UI Automator/Espresso; "since 1.2.0-alpha08" is **(unverified)** — from a third-party article).

The load-bearing observation: Android's cross-process automation rides the **same accessibility tree** the OS screen reader (TalkBack) uses — exactly the unification thesis (see [one-tree-n-consumers.md](./one-tree-n-consumers.md)).

## The shape Buiy should copy

| Tier | Flutter | Compose | What Buiy maps to |
|------|---------|---------|-------------------|
| In-process | `integration_test` / `SemanticsController` | `ComposeTestRule` / `SemanticsNodeInteraction` | ECS test system that walks AccessKit tree + dispatches `ActionRequest`s directly |
| Out-of-process RPC | VM Service Protocol (JSON-RPC 2.0 / WebSocket) + `ext.flutter.*` | Layout Inspector over ADB; `AccessibilityNodeInfo` IPC | a stable structured RPC over the running Buiy app's AccessKit tree |
| Agent shim | Dart/Flutter MCP server (stdio ↔ DTD/VM service) | third-party Android MCP servers over ADB/a11y | a thin MCP shim re-exposing the RPC over stdio |

The non-obvious lesson for Buiy (carried into [lessons.md](./lessons.md)): the *in-process* tier and the *agent* tier should bottom out on the **same primitives** — the AccessKit semantic tree for perception, and AccessKit `ActionRequest`s for control. Buiy already *exports* the tree and authors actions on it through the `bevy_winit` AccessKit channel; the **inbound** direction (consuming `ActionRequest`s back through that channel) is the gap to close — see [lessons.md](./lessons.md). Flutter shows the target shape: `SemanticsAction`/`performAction` in-process and `ext.flutter.inspector.*` out-of-process both operate on the one `SemanticsNode` tree. The transport tier should be a thin wrapper, not a second, divergent action model.

## Sources
- [api.flutter.dev — SemanticsController (flutter_test)](https://api.flutter.dev/flutter/flutter_test/SemanticsController-class.html)
- [docs.flutter.dev — Migrating from flutter_driver](https://docs.flutter.dev/release/breaking-changes/flutter-driver-migration)
- [developer.android.com — Test your Compose layout (testing APIs)](https://developer.android.com/develop/ui/compose/testing/apis)
- [Dart VM Service Protocol — service.md (dart-lang/sdk)](https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service.md)
- [Dart VM Service Protocol Extensions — service_extension.md (dart-lang/sdk)](https://github.com/dart-lang/sdk/blob/main/runtime/vm/service/service_extension.md)
- [flutter/flutter wiki — DDS / DevTools / DTD](https://github.com/flutter/flutter/wiki/Using-the-Dart-Development-Service-(DDS)-and-Flutter-DevTools-with-a-custom-Flutter-Engine-Embedding/e59dedf568832a42480f1bd21cac0154ade5c038)
- [docs.flutter.dev — Dart and Flutter MCP server](https://docs.flutter.dev/ai/mcp-server)
- [developer.android.com — Debug your layout with Layout Inspector](https://developer.android.com/studio/debug/layout-inspector)
