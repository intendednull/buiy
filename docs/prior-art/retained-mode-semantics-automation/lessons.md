**Date:** 2026-06-18
**Status:** active
**Subject:** Retained-mode semantics trees (Flutter + Jetpack Compose) — the decision file: what their precedent validates, what to avoid, what to borrow for Buiy

# Lessons for Buiy

The decision file for this folder. Evidence lives in the siblings; the
load-bearing claim is in [`one-tree-n-consumers.md`](one-tree-n-consumers.md):
Flutter and Jetpack Compose each ship **one framework-owned semantic tree**
consumed by accessibility, test automation, tooling, and (now) LLM agents. Buiy
already owns that tree (the AccessKit tree from
[`../accesskit/`](../accesskit/README.md)) — these lessons are about wiring the
remaining consumers onto it instead of building parallel stacks.

Each item is 1–3 lines and cites the evidence file. `(unverified)` flags any
claim not sourced to a primary doc.

---

## Validates — Buiy's existing bets, confirmed by two independent precedents

- **The AccessKit-tree-as-agent-surface bet.** Both Flutter and Compose expose
  their semantic tree to AT *and* test automation *and* tooling from one
  authored source; Flutter's MCP server now adds LLM agents as a fourth consumer
  over that same widget/semantics tree. The one-tree/N-consumers pattern is
  proven twice, not a Buiy gamble. See
  [`one-tree-n-consumers.md`](one-tree-n-consumers.md),
  [`flutter-semantics.md`](flutter-semantics.md),
  [`compose-semantics.md`](compose-semantics.md).

- **Semantic actions over pixels.** Flutter's `SemanticsController.performAction`
  and Compose's `performSemanticsAction` both drive the app through *named
  semantic actions on tree nodes*, not synthesized coordinate taps. The semantic
  layer is the contract; pixels are downstream. See
  [`flutter-semantics.md`](flutter-semantics.md),
  [`compose-semantics.md`](compose-semantics.md).

- **AccessKit `Action` as the `SemanticsAction`/`SemanticsActions` analog.**
  Flutter's `SemanticsAction` (≥26 constants: `tap`, `setText`, `copy`,
  `increase`, `setSelection`, `dismiss`, `focus`, …) and Compose's
  `SemanticsActions` are exactly the role AccessKit's `Action` enum already
  plays in Buiy (`Click`, `Focus`, `Increment`, `SetValue`, `SetTextSelection`,
  `CustomAction`, …; the 22-variant set per docs.rs/latest = accesskit 0.24.1 —
  **verify the count against the accesskit version Buiy actually compiles
  against**, since the in-tree Cargo.lock pin can lag docs.rs latest; see
  [`../accesskit/`](../accesskit/lessons.md) item 5). Buiy does not need a new
  action vocabulary — it needs to *consume* the one it already advertises. See
  [`flutter-semantics.md`](flutter-semantics.md).

- **A two-tier transport (in-process API + out-of-process RPC).** Flutter ships
  both `integration_test` (in-process, reuses widget-test finders) and the VM
  Service Protocol (JSON-RPC over WebSocket, out-of-process, backing DevTools +
  MCP). Compose mirrors it: `ComposeTestRule` in-process, UI Automator /
  Layout Inspector out-of-process. The same tree, two transports — exactly the
  shape Buiy wants (in-process for `buiy_verify`, AccessKit `ActionRequest` over
  the `bevy_winit` channel for external agents). See
  [`transports.md`](transports.md).

---

## Borrow — concrete patterns to lift

- **The generic `perform_action(node, Action, data)` primitive + ergonomic
  wrappers.** Both frameworks expose one general action dispatcher and layer
  named shortcuts on top: Flutter's `performAction(finder, action, args)` plus
  `tap()`/`copy()`/`setText()`/`scrollUp()`; Compose's `performSemanticsAction`
  plus `performClick()`/`performTextInput()`. Buiy should route every
  `ActionRequest { action, target, data }` through one primitive, then add
  typed wrappers. See [`flutter-semantics.md`](flutter-semantics.md),
  [`compose-semantics.md`](compose-semantics.md).

- **An explicit settle/idle signal between act and perceive.** Flutter's
  `pumpAndSettle` and Compose's `waitForIdle`/`awaitIdle` (sharper under the v2
  `StandardTestDispatcher` default) exist so a consumer never reads the tree
  between an action and its settled effect. A Buiy agent loop
  (perceive→act→perceive) needs the analog — a "tree is current as of frame X /
  effects applied" signal — or it will perceive a stale tree. See
  [`open-problems.md`](open-problems.md) §6.

- **Stable author-set locators decoupled from i18n.** Compose's
  `Modifier.testTag` (surfaced to UI Automator/Espresso via
  `testTagsAsResourceId`, available since ~Compose 1.2.0-alpha08 *(high-risk
  version, third-party source)*) and Flutter's `ValueKey`/`byValueKey` give
  automation a stable handle that survives label translation and copy edits.
  Buiy should expose a test-tag/key on nodes distinct from the a11y `name`;
  whether its existing `NodeId`/`Entity` identity (per
  [`../accesskit/`](../accesskit/README.md)) already suffices is an
  [`open-problems.md`](open-problems.md) question. See
  [`flutter-semantics.md`](flutter-semantics.md),
  [`compose-semantics.md`](compose-semantics.md).

- **A rich matcher/locator set, not just by-id.** Compose's `SemanticsMatcher`
  composes by text / content-description / tag / role / state with hierarchy
  combinators (`hasParent`, `hasAnyChild`, `onChildren`); Flutter has
  `bySemanticsLabel` / `byValueKey` Finders. Buiy's agent + verify layer wants
  the same: match on role + name + state + tag, combine by tree position. See
  [`compose-semantics.md`](compose-semantics.md),
  [`flutter-semantics.md`](flutter-semantics.md).

- **An in-process test API as `buiy_verify`'s lowest tier.** The
  `SemanticsController` / `ComposeTestRule` shape — locate a node, assert its
  semantics, perform an action, assert again, all in the same process with no
  RPC — is the cheapest tier to add and the one that most directly exercises the
  tree. It slots under Buiy's verification tiers (cf. `using-buiy-verification`,
  layout/display-list snapshots) as a semantics-query tier. See
  [`transports.md`](transports.md),
  [`one-tree-n-consumers.md`](one-tree-n-consumers.md).

---

## Avoid — failure modes and traps the precedents reveal

- **Do NOT gate the agent surface behind a debug-only build.** Some tooling
  channels (Flutter's VM Service / DevTools) are dev-mode affordances. Buiy's
  AccessKit tree is **always live in normal builds** (it is what screen readers
  consume) — the agent surface should be designed as a normal-build, always-on
  surface, not a debug feature behind a flag. **But** an always-on action
  *ingress* in production needs an explicit trust/authorization model that the
  debug-gated frameworks never had to build — pay that cost explicitly. See
  [`transports.md`](transports.md), [`open-problems.md`](open-problems.md) §8,
  [`../accesskit/`](../accesskit/README.md).

- **Do NOT make pixel-coordinate locators the primary path.** Flutter's
  `flutter_driver` (legacy, separate-process, coordinate-leaning) was superseded
  by the in-process semantic-finder model of `integration_test`. The industry
  moved *away* from coordinates toward semantic locators; Buiy should start where
  they ended. (The official migration page frames `flutter_driver` as a migration
  target and **does not use the word "deprecated"**; whether it is formally
  *removed* in 3.44 vs merely legacy is *(unverified)*.) See
  [`transports.md`](transports.md),
  [`flutter-semantics.md`](flutter-semantics.md).

- **Do NOT fragment into parallel automation stacks.** Pick **one** locator +
  action model — the AccessKit tree — and make verify, external agents, and AT
  all consume it. Compose's `useUnmergedTree` toggle and the v1→v2 test-API
  migration (v2 default, v1 deprecated as of `compose.ui` **1.11.0**, 2026-04-22
  *(high-risk patch numbers)*) show the maintenance cost of even *one* tree with
  modal variants; two divergent stacks would be worse. See
  [`compose-semantics.md`](compose-semantics.md),
  [`open-problems.md`](open-problems.md).

- **Do NOT assume an action reports its own result.** Flutter's `performAction`
  is fire-and-forget (`checkForAction` only asserts the action is *advertised*);
  Compose returns a bare handled-`Boolean`. Neither tells you *what changed* —
  the durable answer is re-perceive the tree after it settles. AccessKit's
  `ActionRequest` inherits the fire-and-forget shape, so a Buiy agent learns
  outcomes by re-reading, not from the action. Whether to add an explicit
  acknowledgement/result channel is open. See
  [`open-problems.md`](open-problems.md) §7.

- **Treat the LLM-agent layer as still-young.** Flutter's MCP server is the
  closest precedent for Buiy's exact goal, but its docs state verbatim it "is
  experimental and likely to evolve quickly" (requires Dart 3.9+). Borrow the
  *architecture* (introspect the tree, expose actions), not a frozen API. See
  [`flutter-semantics.md`](flutter-semantics.md),
  [`open-problems.md`](open-problems.md).

---

## How this lands in Buiy

The Buiy-specific design decision — make the already-authored AccessKit tree
**bidirectional** by consuming AccessKit `ActionRequest`s through the existing
`bevy_winit` channel — is recorded in [`../accesskit/`](../accesskit/lessons.md)
(items 5 and 11) and the foundation a11y spec
([`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)),
not invented here. Today Buiy's use of that channel is **outbound only** — it
exports the tree and authors actions on nodes (a11y/translate.rs); the **inbound**
`ActionRequest` consumer is the gap to close. This folder's contribution is the
cross-framework evidence that the pattern is sound and the borrow/avoid list
above. Open questions (merged-vs-unmerged equivalent, the verify-tier boundary,
the settle/freshness signal, action acknowledgement, always-live security, agent-
layer API churn) live in [`open-problems.md`](open-problems.md); terminology is in
[`glossary.md`](glossary.md).

Two sibling prior-art folders referenced loosely in early drafts —
`llm-agent-interface/` and `browser-automation/` — **do not exist** in
`docs/prior-art/` as of this writing, and no research charter for them was found
in `docs/` *(unverified — not yet created)*. They are deliberately **not**
linked from this folder; add the cross-links only once those folders actually
land.

## Sources

- Flutter `SemanticsController` (flutter_test) — methods incl. `performAction(finder, action, args, checkForAction)`, `simulatedAccessibilityTraversal`, and `tap`/`copy`/`paste`/`setText`/`scrollUp` wrappers: https://api.flutter.dev/flutter/flutter_test/SemanticsController-class.html
- Flutter `SemanticsAction` constant list: https://api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html
- Flutter `flutter_driver` migration page (frames as migration target; no "deprecated"): https://docs.flutter.dev/release/breaking-changes/flutter-driver-migration
- Flutter `WidgetTester.pumpAndSettle` (settle primitive): https://api.flutter.dev/flutter/flutter_test/WidgetTester/pumpAndSettle.html
- Flutter 3.44.0 release (stable, 2026-05-18): https://docs.flutter.dev/release/release-notes/release-notes-3.44.0
- Dart and Flutter MCP server (experimental; introspects running widget tree + runtime errors; requires Dart 3.9+): https://docs.flutter.dev/ai/mcp-server
- Jetpack Compose April '26 release — v2 testing APIs default, v1 deprecated (1.11.0): https://developer.android.com/blog/posts/whats-new-in-the-jetpack-compose-april-26-release
- Compose `SemanticsActions` (Boolean-returning callbacks; PerformImeAction→OnImeAction, OnAutofillText→OnFillData): https://composables.com/jetpack-compose/androidx.compose.ui/ui/objects/SemanticsActions/api
- Compose UI testing APIs (ComposeTestRule, SemanticsMatcher, testTag, useUnmergedTree, awaitIdle): https://developer.android.com/develop/ui/compose/testing/apis
- AccessKit `Role`/`Action` reference (Rust, docs.rs/latest = 0.24.1): https://docs.rs/accesskit/latest/accesskit/enum.Action.html
