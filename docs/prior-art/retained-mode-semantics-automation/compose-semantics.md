**Date:** 2026-06-18
**Status:** active
**Subject:** Jetpack Compose — the Semantics tree (`SemanticsProperties`/`SemanticsActions`), merged vs unmerged trees, and the `ComposeTestRule`/`SemanticsNodeInteraction` automation surface that drives it

# Jetpack Compose — Semantics tree as inspection/automation surface

Jetpack Compose builds a framework-owned **Semantics tree** parallel to its layout tree. The same tree feeds (a) OS accessibility (TalkBack), (b) the test framework (`ComposeTestRule`), and (c) tooling (Layout Inspector). This is the Android-side instance of the [one-tree-N-consumers](one-tree-n-consumers.md) thesis, structurally analogous to [Flutter's semantics tree](flutter-semantics.md) and to [AccessKit](../accesskit/).

- **Maintainer:** Google (AndroidX). **License:** Apache-2.0 (AndroidX-wide, including `androidx.compose.ui:ui-test*`). **Repo:** <https://github.com/androidx/androidx>.
- **Current stable `androidx.compose.ui` (incl. `ui-test`):** 1.11.x — 1.11.0 stable 2026-04-22 (April '26 BOM `2026.04.01`), with 1.11.1 and 1.11.2 (~2026-05-19) as later patches. Patch numbers are high-risk; verify at release if precision matters.

## The Semantics tree: properties and actions

Each semantics node carries a `SemanticsConfiguration` — a set of `SemanticsPropertyKey` entries. Two key families:

- **`SemanticsProperties`** — descriptive state: `ContentDescription`, `Text`, `EditableText`, `Role`, `StateDescription`, `ProgressBarRangeInfo`, `Disabled`, `Focused`, `Selected`, `ToggleableState`, `TestTag`, etc. (the *what is this* facts a screen reader reads).
- **`SemanticsActions`** — invocable callbacks: the node's *verbs*. These are the direct analog of Flutter `SemanticsAction` and AccessKit `Action`. Verified against the current `SemanticsActions` object (each is typically an `ActionPropertyKey<(...) -> Boolean>` — note the **`Boolean` return**, the action-result channel discussed in [open-problems.md](open-problems.md)):

  `OnClick`, `OnLongClick`, `ScrollBy`, `ScrollByOffset`, `ScrollToIndex`, `SetProgress`, `SetSelection`, `SetText`, `SetTextSubstitution`, `ShowTextSubstitution`, `ClearTextSubstitution`, `InsertTextAtCursor`, `OnImeAction`, `CopyText`, `CutText`, `PasteText`, `Expand`, `Collapse`, `Dismiss`, `RequestFocus`, `CustomActions`, `PageUp`/`PageDown`/`PageLeft`/`PageRight`, `GetTextLayoutResult`, `GetScrollViewportLength`, `OnFillData`.

  Two deprecations to note: **`PerformImeAction` is deprecated, replaced by `OnImeAction`**, and **`OnAutofillText` is deprecated, replaced by `OnFillData`**. Both deprecations are attested in the `SemanticsActions` API reference, which carries the verbatim replacement notes "Use `SemanticsActions.OnImeAction` instead." and "Use `SemanticsActions.OnFillData` instead." (mirrors the androidx KDoc; <https://composables.com/jetpack-compose/androidx.compose.ui/ui/objects/SemanticsActions/api>). The exact release that introduced each deprecation was not pinned to a primary changelog entry — older Compose versions exposed the deprecated names, so a writer pinning to an older release must check which existed then.

A widget *declares* these via the `semantics { }` modifier or via higher-level modifiers (`clickable` sets `OnClick`, `BasicTextField` sets `SetText`/`SetSelection`/text actions). The accessibility bridge and the test framework then *invoke* them. This declare-then-invoke split is exactly Buiy's opportunity: Buiy already authors AccessKit actions on its nodes (output-only); Compose's design shows the same action callbacks being driven by a second consumer (tests) over the same tree.

## Merged vs unmerged tree (`useUnmergedTree`)

The Semantics tree exists in two forms:

- **Unmerged** — every node intact, one per semantics-bearing modifier.
- **Merged** — descendants are collapsed into an ancestor that set `Modifier.semantics(mergeDescendants = true)` (e.g. a `Button` merges its `Text` + `Icon` into one focusable node). `mergeDescendants` defaults to `false`.

Consumers differ on which they use:

- **Accessibility services** consume the **unmerged** tree and apply their *own* merging algorithm (honoring `mergeDescendants`), because different assistive tech merges differently.
- **The test framework** uses the **merged** tree **by default**. Matchers take `useUnmergedTree: Boolean = false`; pass `true` to opt into the unmerged tree (useful when a child is hidden by a parent's merge). This dual view is a lesson for any single-tree design: one canonical tree, but consumers need both a merged "as a user/AT perceives it" projection and a raw "as authored" projection.

## The automation surface: `ComposeTestRule` + `SemanticsNodeInteraction`

Tests obtain a `ComposeTestRule` (via `createComposeRule()` / `createAndroidComposeRule<A>()`), then locate nodes and act on them. The handle is `SemanticsNodeInteraction`: "a semantics node and the path to fetch it from the semantics tree."

**Finders** (on the rule):
- `onNodeWithText(text)`, `onNodeWithContentDescription(desc)`, `onNodeWithTag(testTag)` — convenience wrappers over `onNode(matcher)`.
- `onNode(SemanticsMatcher)`, `onAllNodes(...)`, `onRoot()`, plus tree navigation (`onChildren`, `onSibling`, `onAncestors`).

**Actions** (on `SemanticsNodeInteraction`):
- High-level: `performClick`, `performTextInput`, `performTextClearance`, `performTextReplacement`, `performTextInputSelection`, `performScrollTo`, `performScrollToIndex`, `performImeAction`, `performKeyPress`/`performKeyInput`, `performTouchInput`, `performMouseInput`.
- **Generic: `performSemanticsAction(SemanticsActions.X)`** — invokes any declared action by key. This is the load-bearing analog of Flutter's `performAction(SemanticsAction)` and an AccessKit `ActionRequest`: a single uniform "dispatch this action against the node located in the tree" entry point, rather than N bespoke methods. The high-level helpers above are thin wrappers over it (e.g. `performTextInput` ultimately drives `SemanticsActions.SetText`/`InsertTextAtCursor`). `performSemanticsAction` returns the `SemanticsNodeInteraction` for chaining.
- Text-input actions assert-fail on read-only `TextField`s — the action set reflects node capability, not a blanket API.

**Assertions:** `assert*` matchers — `assertIsDisplayed`, `assertExists`, `assertTextEquals`, `assertHasClickAction`, `assertIsEnabled`/`assertIsSelected`/`assertIsOn`, `assertContentDescriptionEquals`, etc. — read the same property set the screen reader reads. The test oracle and the accessibility surface are the same data.

### v1 → v2 testing-API shift (1.11.0)

In **1.11.0 the v2 testing APIs became the default and the v1 testing APIs were deprecated** (following an opt-in period in 1.10). The mechanical change is the test dispatcher: v1 used `UnconfinedTestDispatcher` (coroutines ran immediately); v2 uses `StandardTestDispatcher` (coroutines queue and don't run until the virtual clock is advanced). Large UI-test suites need migration. This is a *timing/dispatch* change, not a change to the Semantics-tree model or the finder/action surface above. (The settle/idle primitive this implies — `awaitIdle`, `mainClock` advance — is treated in [open-problems.md](open-problems.md).)

## `testTag` → resource ID → cross-framework reach

`Modifier.testTag("x")` sets `SemanticsProperties.TestTag`, which `onNodeWithTag` matches. By itself the tag is visible only to Compose's own test framework. To surface it to **out-of-process / interop drivers** (UI Automator, Appium/UIAutomator2, Espresso) that read the Android `AccessibilityNodeInfo`, set the **`testTagsAsResourceId`** semantics property on a top-level (or subtree) composable. It then:
- fills `AccessibilityNodeInfo.viewIdResourceName` (the field the View system populates from XML resource IDs), making Compose nodes addressable by "resource id" the way classic Views are; and
- mirrors the tag into `AccessibilityNodeInfo.extras` under key `androidx.compose.ui.semantics.testTag`.

This is the seam where a Compose-internal automation handle becomes legible to the **accessibility-tree-based** out-of-process world. `testTagsAsResourceId` was reportedly introduced around Compose 1.2.0-alpha08 — that exact version is from a third-party article, not the primary changelog **(unverified)**; the *mechanism* (it populates `viewIdResourceName` for AT/automation tools) is documented by Google. The Buiy lesson: a semantic tree is most useful when its identifiers escape the framework's own harness and reach the platform's standard accessibility plumbing — which is precisely what AccessKit gives Buiy by construction.

## Tooling consumer: Layout Inspector

**Layout Inspector** (Android Studio) is the tooling consumer of the same tree. It renders the composition with **semantics layers** (merged and unmerged semantics shown per node) alongside **recomposition counts / skip counts**, so a developer can see exactly which semantics a node exposes and how often it recomposes. Same tree, third consumer — paralleling Flutter's Widget Inspector / DevTools.

## Implications for Buiy (pointer)

Compose validates the core Buiy bet: one framework-owned semantic tree, authored by widgets declaring properties + action callbacks, consumed by accessibility, tests, and tooling — with a single generic `performSemanticsAction(key)` dispatch as the uniform control verb. The merged/unmerged split and `testTagsAsResourceId` are the two refinements worth borrowing. Design takeaways are recorded as validates/borrow/avoid in [lessons.md](lessons.md); this file stays evidence-only.

## Sources

- Compose UI release notes (1.11.0 stable, v2 testing default / v1 deprecated): <https://developer.android.com/jetpack/androidx/releases/compose-ui>
- Jetpack Compose April '26 release (Android Developers Blog): <https://android-developers.googleblog.com/2026/04/jetpack-compose-april-2026-updates.html>
- `androidx.compose.ui.test` package summary: <https://developer.android.com/reference/kotlin/androidx/compose/ui/test/package-summary>
- `SemanticsNodeInteraction` API reference: <https://developer.android.com/reference/kotlin/androidx/compose/ui/test/SemanticsNodeInteraction>
- Testing APIs guide: <https://developer.android.com/develop/ui/compose/testing/apis>
- Semantics guide (merged vs unmerged tree): <https://developer.android.com/develop/ui/compose/accessibility/semantics>
- Merging and clearing: <https://developer.android.com/develop/ui/compose/accessibility/merging-clearing>
- `SemanticsActions` object property list (incl. PerformImeAction→OnImeAction and OnAutofillText→OnFillData deprecation notes): <https://composables.com/jetpack-compose/androidx.compose.ui/ui/objects/SemanticsActions/api>
- Test interoperability (testTag / UI Automator / Espresso): <https://developer.android.com/develop/ui/compose/testing/interoperability>
- `testTagsAsResourceId` reference: <https://composables.com/jetpack-compose/androidx.compose.ui/ui/properties/testTagsAsResourceId/api>
- testTagsAsResourceId helping Appium/UIA2 (third-party, version claim): <https://kazucocoa.wordpress.com/2022/05/17/appiumcompose-testtagsasresourceid-helps-appium-uia2/>
