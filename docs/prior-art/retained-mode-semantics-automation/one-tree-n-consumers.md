**Date:** 2026-06-18
**Status:** active
**Subject:** The central thesis — one framework-owned semantics tree serving N consumers (a11y + test + tooling + agent), with semantic *actions* (not pixel coordinates) as the durable command primitive

# One tree, N consumers

The load-bearing observation across Flutter and Jetpack Compose: each framework
builds **one** framework-owned semantic tree (role + name + state + available
actions per node) and exposes that *same* tree to multiple independent clients.
No client gets its own private tree. The tree is the contract; the clients are
interchangeable readers and drivers of it.

## The four consumers

For both frameworks the consumer set is the same shape:

1. **OS accessibility** — screen readers / assistive tech (TalkBack, VoiceOver,
   Narrator). The original reason the tree exists.
2. **Test automation** — Flutter `integration_test` / Compose `ComposeTestRule`
   drive the visible semantics tree with finders + action calls.
3. **Tooling** — Flutter Widget Inspector + DevTools / Compose Layout Inspector
   read the tree to render an interactive node hierarchy.
4. **LLM agents** — newest consumer. The Dart-and-Flutter MCP server can
   "introspect the running app's Flutter widget tree and fetch runtime errors"
   (docs.flutter.dev/ai/mcp-server, marked **experimental**). It rides the same
   inspection plumbing the other three use rather than inventing a new surface.

This is an architectural observation, not a versioned fact: it is the analogy
Buiy is built on (see "Implications" below and `lessons.md`).

## Semantic actions are the durable command primitive

The second half of the thesis is *how* clients drive the tree. Both frameworks
converge on the same answer: a **closed vocabulary of semantic actions**
dispatched *at a node*, with the framework owning dispatch. There is no
"click at pixel (x, y)" in the durable command path — coordinates are an
implementation detail of one transport (touch), not the command itself.

**Flutter** — `SemanticsAction` (dart:ui) is the closed enum. As of stable 3.44
(released 2026-05-18 per the release notes; treat the exact patch/date as
high-risk) its constants are: `tap`, `longPress`, `scrollLeft`, `scrollRight`,
`scrollUp`, `scrollDown`, `increase`, `decrease`, `showOnScreen`,
`moveCursorForwardByCharacter`, `moveCursorBackwardByCharacter`,
`moveCursorForwardByWord`, `moveCursorBackwardByWord`, `setSelection`, `setText`,
`copy`, `cut`, `paste`, `didGainAccessibilityFocus`,
`didLoseAccessibilityFocus`, `customAction`, `dismiss`, `scrollToOffset`,
`collapse`, `expand`, `focus`. (The canonical doc URL is the `-class.html` form;
`SemanticsAction.html` 404s.) The generic dispatch primitive in tests is
`SemanticsController.performAction(finder, action, {args, checkForAction})`,
verified at api.flutter.dev — it "Performs the given SemanticsAction on the
SemanticsNode found by finder." Ergonomic wrappers (tap, copy, paste, scrollUp,
setText, …) sit on top of that one generic call. `simulatedAccessibilityTraversal`
returns the `Iterable<SemanticsNode>` an AT would walk — the perception half.

**Compose** — `SemanticsActions` is the closed action set; the generic primitive
is `performSemanticsAction`, with ergonomic wrappers `performClick`,
`performTextInput`, `performScrollTo`, etc. on `SemanticsNodeInteraction`. Same
shape: one generic `perform(node, Action, data)` with named conveniences above it.

**AccessKit** (Buiy's substrate) already has the identical primitive on the
*request* side. Its `Action` enum (**22 variants** per docs.rs/accesskit, latest
doc = accesskit 0.24.1) —
`Click`, `Focus`, `Blur`, `Collapse`, `Expand`, `CustomAction`, `Decrement`,
`Increment`, `ShowTooltip`, `HideTooltip`, `ReplaceSelectedText`,
`SetTextSelection`, `SetValue`, `ScrollUp/Down/Left/Right`, `ScrollIntoView`,
`ScrollToPoint`, `SetScrollOffset`,
`SetSequentialFocusNavigationStartingPoint`, `ShowContextMenu` — is the same
closed vocabulary, and `ActionRequest { action, target, data: Option<ActionData> }`
is the same `perform(action, node, data)` triple. The variant names differ
(AccessKit `Click` vs Flutter `tap`; AccessKit `Increment` vs Flutter `increase`)
but the structure is one-to-one: a closed enum, a node target, optional typed
payload (`ActionData::Value`, `SetTextSelection`, `ScrollToPoint`, …). (The exact
22-variant count is pinned to docs.rs/latest = 0.24.1; **verify against the
accesskit version Buiy actually compiles against** — the in-tree Cargo.lock pin
can lag the docs.rs latest, so the variant set may differ from what Buiy builds.
See `../accesskit/`.)

### Why actions, not coordinates, are durable

- **Stable under reflow / theming / DPI.** A `Click` on node *N* survives layout
  changes that move *N*'s pixels; `click(x, y)` does not.
- **Framework owns dispatch.** The toolkit routes the action to the right handler
  (hit-testing, focus, IME) — the client need not reimplement input semantics.
- **Same path as AT.** A test or agent exercises the *exact* code path a screen
  reader uses, so automation coverage doubles as a11y coverage.

## Stable locators, decoupled from i18n text

Addressing a node needs a name that does not break when copy changes language.
Both frameworks supply an author-controlled, text-independent locator:

- **Flutter** — `ValueKey` (and `bySemanticsLabel` only where a stable label is
  intended). Keys are author-supplied identity, independent of visible text.
- **Compose** — `Modifier.testTag` + `onNodeWithTag`. Community guidance is
  explicit: "Use testTag for locators and do not use `onNodeWithText` — text
  changes with i18n" (tomasrepcik.dev). `testTagsAsResourceId` further surfaces
  the tag as an Android resource ID to UI Automator/Espresso (reportedly since
  Compose 1.2.0-alpha08 — **verify the introducing version**, from a third-party
  article).

The recurring rule: **locator = author identity; name = human/AT label.** Never
overload the i18n display string as the automation key. (Whether Buiy's existing
ECS identity model — `NodeId`/`Entity`, documented in `../accesskit/` — already
provides such a stable author key, or needs a new test-tag field, is an
`open-problems.md` item, not decided here.)

## The cautionary tale: don't fragment into parallel stacks

The thesis cuts both ways. Flutter *has* the one-tree architecture, but its
*automation entry points* accreted into parallel stacks rather than one:

- `flutter_driver` — legacy, out-of-process, drives over the service protocol
  (Finder: `byValueKey` / `bySemanticsLabel`). Google superseded it with
  `integration_test`. (The official migration page frames it as a migration
  target and **does not use the word "deprecated"**; whether it is formally
  *removed* in 3.44 vs merely legacy is **(unverified)**.)
- `integration_test` — modern, in-process, reuses widget-test finders +
  `WidgetController`. The official replacement.
- The Dart-and-Flutter **MCP server** — a third, agent-facing entry point
  (still **experimental**).

Three doors into the same building. Each new automation consumer arrived as a
fresh top-level stack instead of another reader of the one tree, costing
migration churn (`flutter_driver` → `integration_test`) and concept duplication.
Compose's v1→v2 testing-API shift (v2 became default and v1 was deprecated in
ui-test 1.11.0, ~April 2026 — **confirm the deprecation wording** against the
compose-ui release notes) is a milder version of the same accretion. The lesson
is in `lessons.md` (avoid): keep *one* command/perception surface and add
consumers as thin adapters over it, not as parallel automation stacks.

## Implications for Buiy

Buiy already authors an AccessKit semantic tree (role + name + state + actions)
but it is currently **output-only** — consumer (1), the screen reader, plus
whatever tooling reads the tree. The thesis says the same tree should serve all
four consumers, and the gap to close is **bidirectionality**: consume
`ActionRequest`s back through the existing `bevy_winit` AccessKit channel, exactly
as a screen reader's action would arrive. Today Buiy uses that channel
**outbound only** (it pushes the tree and authors actions on nodes via
`a11y/translate.rs`); the **inbound** path — receiving an `ActionRequest` and
routing it into the ECS — does not yet exist and is the change that turns the
tree from a one-way export into the perception+control surface for:

- the test harness (drive `Action::Click` at a node, assert resulting state);
- tooling/inspection (already reading the tree);
- LLM agents (perceive via the tree, command via `ActionRequest`).

Two design pulls, recorded as `validates` / `avoid` in `lessons.md`, not decided
here:

- **validates** — Buiy starts where the thesis ends: the closed action vocabulary
  (`accesskit::Action`) and the `perform(action, node, data)` request shape
  already exist and the outbound a11y channel is already wired. No new command
  primitive is needed; the durable-action lesson is satisfied by construction.
  What remains is wiring the *inbound* `ActionRequest` consumer.
- **avoid** — Flutter's stack fragmentation. Buiy should resist adding a separate
  "agent driver" or "test driver" with its own addressing model. One tree, one
  `ActionRequest` ingress, N thin consumers. (See `transports.md` for how an
  MCP/JSON-RPC layer should sit *over* the tree, not beside it; the agent-as-
  fourth-consumer framing for Buiy lives in `lessons.md`.)
- Locator decoupling carries over: AccessKit names are AT labels (i18n text); a
  stable author key (the analog of `testTag` / `ValueKey`) is what an agent or
  test should address by. Whether Buiy already has such a stable node key (its
  `NodeId`/`Entity` identity, per `../accesskit/`), or needs to add one, is an
  `open-problems.md` item.

## Cross-links

- `lessons.md` — validates / avoid / borrow distilled from this thesis.
- `flutter-semantics.md`, `compose-semantics.md` — per-framework evidence.
- `transports.md` — how MCP / VM-Service / JSON-RPC layer *over* the tree.
- `open-problems.md` — stable-locator gap, merged-vs-unmerged tree, action
  acknowledgement, async-settle freshness, action completeness.
- `../accesskit/` — the `Action` / `ActionRequest` substrate Buiy uses, and the
  `NodeId`/`Entity` identity model.

## Sources

- https://api.flutter.dev/flutter/flutter_test/SemanticsController-class.html
- https://api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html
- https://docs.flutter.dev/release/breaking-changes/flutter-driver-migration
- https://docs.flutter.dev/ai/mcp-server
- https://docs.rs/accesskit/latest/accesskit/enum.Action.html
- https://github.com/AccessKit/accesskit
- https://developer.android.com/develop/ui/compose/accessibility/semantics
- https://developer.android.com/develop/ui/compose/testing/interoperability
- https://tomasrepcik.dev/blog/2024/2024-02-13-test-tags-and-sematics/
- https://composables.com/docs/androidx.compose.ui/ui/properties/testTag
