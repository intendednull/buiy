**Date:** 2026-06-18
**Status:** active
**Subject:** Open problems and honest gaps in the retained-mode semantics-as-automation-surface model (Flutter + Compose), and which Buiy inherits vs avoids

# Open problems — gaps in the semantics-automation model

The "one tree, N consumers" thesis (see [one-tree-n-consumers.md](one-tree-n-consumers.md))
is attractive but not free. This file collects the honest gaps in the Flutter
and Jetpack Compose implementations of it, marks each as **inherent** (a property
of the model itself, which any AccessKit-first design including Buiy would also
face) or **tooling** (an accident of how Flutter/Compose ship today, fixable
without changing the model), and notes where Buiy's always-live AccessKit tree
sidesteps the problem. Design conclusions live in [lessons.md](lessons.md); this
file is evidence.

---

## 1. Closed action vocabulary vs arbitrary app transitions

**Inherent (partially mitigated).** A semantic tree exposes a *fixed* set of
actions. Flutter's `SemanticsAction` (dart:ui) is a closed enum: `tap`,
`longPress`, the scroll family, `increase`/`decrease`, the cursor/text family,
`copy`/`cut`/`paste`, `dismiss`, `collapse`/`expand`/`focus`, `scrollToOffset`,
and the focus-event actions — verified against
api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html. (The set is larger
than the older enumerations many articles quote; `collapse`, `expand`, `focus`,
and `scrollToOffset` are newer additions.) Compose's `SemanticsActions` is
likewise a fixed property set.

Many real app interactions don't map onto any of these verbs — "swipe this card
to the third snap point," "open the overflow menu's hidden item," "drag this list
row to reorder." The escape hatch is **`customAction`**: an app declares a named
`CustomSemanticsAction` and a screen reader (or an automation driver) invokes it
by identity. This works, but it pushes the contract back into per-app string
conventions — the agent/test must know the custom action's label out of band, and
the action is only as discoverable as the app chose to make it. The closed core
vocabulary is what makes the tree a *stable* automation target; the `customAction`
hatch is what keeps it *complete*, at the cost of a typed, discoverable contract.

**For Buiy:** AccessKit has its own action set (`Action`) plus the same tension.
The lesson is to keep the core verbs typed and add a named-custom-action channel
rather than letting apps invent ad-hoc side channels. See [lessons.md](lessons.md).

---

## 2. Debug-only gating — production apps can't be driven

**Tooling (the sharpest gap, and Buiy's clearest advantage).** Every runtime
introspection/automation transport surveyed in [transports.md](transports.md) is
gated to non-production builds:

- **Flutter VM Service + service extensions.** docs.flutter.dev/testing/build-modes
  states verbatim for **release mode**: "Debugging is disabled." and "Service
  extensions are disabled." A service extension "can only be activated if the
  vm-service is included in the build, which only happens in debug and profile
  mode." The Widget Inspector, DevTools, and the `ext.flutter.inspector.*`
  surface that backs them therefore do not exist in a shipped release binary.
- **Dart-and-Flutter MCP server.** Its runtime app-introspection (read the running
  widget tree, fetch runtime errors) rides the same VM Service, so it inherits
  the same debug/profile gating. It is also documented as **experimental**
  ("likely to evolve quickly") and requires Dart 3.9+ — see
  [transports.md](transports.md).
- **Marionette (Firefox, the analogous remote-control story).** Marionette is
  off by default and historically "only enabled on TBPL debug builds"; a
  `--marionette` CLI flag was added specifically to enable it on other builds,
  and it needs an app-side server binding to be present and switched on
  (`marionette.enabled`). It is not a capability of an ordinary shipped browser.

Net: the *accessibility* consumer (TalkBack/VoiceOver) works in production, but
the *automation/agent* consumers of the very same tree do **not**. An LLM agent
cannot drive a release Flutter app through these channels. This is the practical
refutation of a naive "the agent surface ships for free" claim.

**Why it's tooling, not inherent:** nothing about a semantic tree *requires*
debug-gating. The tree is live in production (screen readers use it). The gating
is a property of how Flutter wires its *transport* (the VM Service) — chosen for
binary size and security, with the tree-shaker stripping extension code from
release builds. A transport that consumes the production accessibility tree
directly would not be debug-gated.

**Buiy's advantage — and its sharper security question (see §7).** Buiy already
authors a live AccessKit tree as part of normal (production) operation, pushed to
platform a11y. Making it bidirectional means consuming AccessKit `ActionRequest`s
through the existing bevy_winit channel — the same path screen readers already
drive. There is no separate debug-only service to gate. The automation surface is
the production a11y surface, so it is live whenever a screen reader would be. That
is the central validates-point in [lessons.md](lessons.md) — but it inverts the
security posture: an always-on action *ingress* now ships in production. See §7.

---

## 3. Merged vs unmerged tree confusion

**Inherent.** Compose maintains *two* views of the tree: a **merged** tree (child
nodes folded into an ancestor when `mergeDescendants` is set — e.g. a button's
icon + label collapse into one focusable node) and an **unmerged** tree (every
node kept intact). Test matchers use the **merged** tree by default;
`useUnmergedTree = true` opts into the raw structure (default `false`).

This is a recurring source of "node not found" failures: a test queries for text
that exists only as a separate node in the *unmerged* tree, but the matcher is
looking at the *merged* tree where that text has been folded into its parent.
The developer must know which tree a given assertion needs. The same duality
exists in Flutter (`SemanticsNode` merge-up via `mergeAllDescendantsIntoThisNode`
/ `MergeSemantics`), and any screen-reader-shaped tree needs *some* merging
(otherwise focus lands on every leaf). So merging is inherent to making the tree
usable for a11y — but the *two-views* split is a real cognitive tax on the
automation consumer, who often wants the structural (unmerged) view while the
a11y consumer wants the merged one.

**For Buiy:** AccessKit has the same node-merging concerns. The takeaway is to be
explicit about which view automation/agents see and to make switching between
them a first-class, discoverable affordance rather than a surprising default.

---

## 4. Custom-painted content is invisible unless semantics are declared by hand

**Inherent.** A semantic tree only contains what the framework or the developer
*declared*. Content drawn directly to a canvas — Flutter's `CustomPainter` /
`CustomPaint`, Compose's `Canvas`/`drawScope` — produces pixels with **no
intrinsic semantics**. A chart, a custom gauge, a hand-rolled diagram is, by
default, a single opaque rectangle (or nothing) in the tree.

Flutter's remedy is `CustomPainter.semanticsBuilder`, which returns a list of
`CustomPainterSemantics` describing parts of the drawing, plus
`shouldRebuildSemantics` to signal when that description changed
(api.flutter.dev/flutter/rendering/CustomPainterSemantics-class.html). This was
added specifically because custom paint was otherwise inaccessible (flutter/flutter
issue #1666, "Make custom paint accessible," and PR #13313). But it is **opt-in
and manual** — the developer must author a parallel semantic description by hand,
and most don't.

The consequence is the same for all three consumers at once: custom-painted
content that lacks a hand-written semantic description is invisible to **screen
readers, to test automation, and to LLM agents** simultaneously. The unification
thesis cuts both ways — the gap is shared, not isolated to one consumer. This is
the strongest argument that "semantics as the universal surface" is only as good
as the discipline of declaring semantics for non-widget content.

**For Buiy:** any Buiy widget that bypasses the standard role/name authoring and
paints directly will be equally invisible to AccessKit consumers. The mitigation
is the same — provide a semantics-builder affordance for custom-drawn widgets and
treat "declared a11y semantics" as a completeness requirement, not an a11y nicety.

---

## 5. Fragmentation across parallel stacks

**Tooling.** The model is reinvented per framework with incompatible APIs:
Flutter's `SemanticsAction`/`SemanticsController`/`flutter_test` vs Compose's
`SemanticsActions`/`ComposeTestRule`/`SemanticsNodeInteraction`, layered over
platform a11y trees (Android `AccessibilityNodeInfo`, iOS UIAccessibility) that
are themselves distinct. Bridges leak: Compose's `testTagsAsResourceId` exists
precisely to surface test tags as Android resource IDs so UI Automator/Espresso
can see them — a seam between the Compose semantic tree and the platform
automation tree. An agent targeting "the semantic tree" must in practice target a
*specific framework's* dialect of it; there is no single cross-stack vocabulary.

**For Buiy:** AccessKit is itself a cross-platform a11y abstraction, which reduces
(not eliminates) this fragmentation — a Buiy agent surface speaks AccessKit's
vocabulary once rather than per-OS. That is a point in AccessKit's favor; see
[lessons.md](lessons.md).

---

## 6. Recomposition / performance and tree-staleness during async settle

**Inherent (cost), tooling (how it's managed).** Keeping a semantic tree
continuously in sync with a reactive UI has a cost: every relevant rebuild may
dirty semantic nodes. Flutter gates rebuilds with `shouldRebuildSemantics`
(custom painters) and batches semantic updates; Compose recomputes semantics as
part of recomposition and folds the merged tree on demand. Two consequences
worth naming honestly:

- A *live* automation/agent surface implies the tree is maintained even when no
  screen reader is attached, or it must be spun up on demand. Flutter historically
  only fully populates semantics when accessibility is active or when a semantics
  listener is registered — so an always-on agent consumer changes the cost model
  from "pay when a11y is on" to "pay always."
- Snapshotting the tree for an agent (vs streaming deltas) trades freshness for
  overhead; a large dynamic UI can produce a large tree per query.

**The settle/idle problem (load-bearing for an agent loop).** The harder, more
practical half is *staleness*: after an agent fires an action, **when is the tree
safe to read?** A reactive UI doesn't update synchronously — an action triggers
recomposition / animation / async work that may not have landed by the next
perception read. Both frameworks expose an explicit *quiescence* primitive for
exactly this:

- **Flutter** — `WidgetTester.pumpAndSettle()` repeatedly pumps frames until no
  frames are scheduled (or a timeout), so a test does not read between an action
  and its settled effect. (It does **not** wait on indefinite work — infinite
  animations or polling timers will time it out; that is a known edge.)
- **Compose** — `ComposeTestRule.waitForIdle()` / `awaitIdle()` block until the
  composition, layout, and (by default) the `mainClock` are idle; tests advance
  the virtual clock (`mainClock.advanceTimeBy`) to settle queued coroutines. The
  v2 `StandardTestDispatcher` default (see [compose-semantics.md](compose-semantics.md))
  makes this *more* explicit: coroutines now queue rather than run eagerly, so a
  test must deliberately advance/await before perceiving.

The lesson: a perceive→act→perceive agent loop needs a **"the UI has settled"
signal** between the action and the next read, or it will perceive a stale tree.
In a test harness this is a blocking call; for an out-of-process agent it must be
an observable idle/quiescence event the transport can surface.

**For Buiy:** Buiy's AccessKit tree is ECS-resident and updated through normal
system scheduling, so the marginal cost of an agent consumer is the cost of
*reading* an already-maintained tree plus feeding `ActionRequest`s back — not
standing up a second tree. But the settle question is the same and arguably
sharper in an ECS frame loop: an `ActionRequest` consumed on frame *N* may not
be reflected in the a11y tree until the a11y-update system runs on frame *N+k*.
Buiy needs an explicit "tree is current as of frame X / the action's effects have
been applied" signal an agent can await before its next perception — the analog
of `pumpAndSettle`/`awaitIdle`. Whether that is a frame-counter watermark, an
idle event, or a synchronous round-trip is a real design decision, flagged for
the spec. The standing question of whether the tree is maintained unconditionally
or only when a consumer is attached is also open.

---

## 7. Action acknowledgement / result channel — "did it work, and what changed?"

**Inherent.** Firing an action is only half a control loop; the agent (or test)
then needs to know **whether the action succeeded** and **what changed**. The two
frameworks answer this differently, and neither answer is complete:

- **Flutter** — `SemanticsController.performAction` is essentially
  **fire-and-forget**. Its `checkForAction` flag only asserts that the target
  node *advertises* the action before dispatching; it does **not** report whether
  the handler ran successfully or what state resulted. The success signal, if any,
  is indirect: the test re-reads the tree (after settling — see §6) and asserts
  the expected change. There is no return value carrying "did it take."
- **Compose** — `SemanticsActions` callbacks are typed `(...) -> Boolean`, so an
  action *does* return a success/handled boolean (verified against the
  `SemanticsActions` reference; e.g. an `OnClick` returns whether the click was
  handled). `performSemanticsAction` surfaces a richer-than-Flutter signal — but
  a boolean "handled" is still not a description of *what changed*; the test must
  re-query the tree for that.

So across both, the durable answer to "what changed" is **re-perceive the tree**,
not a structured action-result payload. There is no standard "action result diff"
channel; the agent's model of the world is refreshed by reading, not by the
action telling it what it did. For a slow out-of-process agent, that means a full
(or delta) tree re-read per action — coupling this problem tightly to §6's
freshness/cost question.

**For Buiy:** AccessKit's `ActionRequest` is itself fire-and-forget — there is no
built-in result return on the inbound channel (it mirrors how a screen reader's
action is delivered). So Buiy inherits Flutter's shape by default: an agent fires
`Action::Click` and learns the outcome by re-reading the tree once it has settled
(§6). Whether Buiy should add an explicit acknowledgement/result signal — a
handled-boolean à la Compose, an applied-on-frame-X watermark, or a changed-nodes
delta — rather than forcing a full re-read is an open design decision, flagged for
the spec. The cheap default (re-perceive) works; a result channel is an
optimization/UX question, not a correctness blocker.

---

## 8. Agent-surface security in an always-live build

**Inherent for Buiy's stance; sidestepped by Flutter's debug-gating.** §2 frames
debug-only gating as a *limitation* of Flutter and an *advantage* of Buiy — but
that advantage carries an obligation. Flutter's VM-Service automation is
incidentally protected: it doesn't exist in release builds, and even in
debug/profile the endpoint is a `ws://127.0.0.1:PORT/<token>/ws` URL whose path
token is a **per-run secret** (see [transports.md](transports.md)) — an attacker
can't connect without it. The automation ingress is both *absent in production*
and *capability-gated by a token* in dev.

Buiy's thesis deliberately removes the first protection: the automation surface
**is** the production a11y surface, always live. An inbound `ActionRequest`
consumer that is on in shipped builds is, by construction, a way for *something*
to drive the app programmatically in production. That sharpens, rather than
softens, several questions the debug-gated frameworks never had to answer:

- **Who is allowed to inject actions?** On the platform a11y channel, the OS
  accessibility service is the trusted injector. If Buiy also accepts actions from
  an in-process agent or an out-of-process transport, what authenticates that
  source, and is it the same trust level as the OS AT?
- **Is the agent ingress separable from the AT ingress?** A screen reader driving
  the app is expected and desirable; an arbitrary local process driving a shipped
  app may not be. The two arrive (or would arrive) through the same
  `ActionRequest` path — whether they *should* share a path, or whether the
  out-of-process/agent transport needs its own opt-in + auth (a token analog to
  Flutter's per-run secret), is a real decision.
- **Capability scope.** A closed action vocabulary bounds what an injected action
  can do (you can't `Click` your way to arbitrary code), which helps — but
  `SetValue` / `ReplaceSelectedText` on the wrong node is still a meaningful
  attack surface in a production app.

This is **not** a reason to abandon the always-live stance — it is the cost of
that stance, to be paid explicitly. The lesson for [lessons.md](lessons.md): an
always-on action ingress must ship with an explicit trust/authorization model for
non-AT injectors (likely: AT actions trusted via the OS path; agent/transport
actions gated behind an opt-in capability), not treated as free because the tree
was already live. Flagged for the spec.

---

## Summary table

| Gap | Inherent / Tooling | Buiy posture |
|---|---|---|
| Closed action vocabulary | Inherent (mitigated by `customAction`) | Shared; keep core verbs typed + named-custom channel |
| Debug-only gating, no prod driving | **Tooling** | **Avoided** — AccessKit tree is live in production (but see security, below) |
| Merged/unmerged confusion | Inherent | Shared; make the active view explicit |
| Custom-paint invisible w/o manual semantics | Inherent | Shared; require semantics for custom-drawn widgets |
| Per-stack fragmentation | Tooling | Reduced — single AccessKit vocabulary |
| Semantics-tree perf + async-settle staleness | Inherent cost | Cheaper read (tree already maintained); needs an explicit settle/freshness signal — open |
| Action acknowledgement / result | Inherent | Default = re-perceive after settle; explicit result channel is open |
| Agent-surface security (always-live ingress) | Inherent to Buiy's stance | Sharper, not softer; needs an explicit trust/auth model — open |

See also: [one-tree-n-consumers.md](one-tree-n-consumers.md) for the model these
gaps qualify, [transports.md](transports.md) for the debug-gating and per-run-
token detail, [compose-semantics.md](compose-semantics.md) for the
`StandardTestDispatcher` settle mechanics, and [lessons.md](lessons.md) for the
validates/avoid/borrow conclusions.

## Sources

- https://api.flutter.dev/flutter/dart-ui/SemanticsAction-class.html
- https://api.flutter.dev/flutter/flutter_test/WidgetTester/pumpAndSettle.html
- https://developer.android.com/reference/kotlin/androidx/compose/ui/test/ComposeUiTest
- https://composables.com/jetpack-compose/androidx.compose.ui/ui/objects/SemanticsActions/api
- https://docs.flutter.dev/testing/build-modes
- https://api.flutter.dev/flutter/foundation/BindingBase/registerServiceExtension.html
- https://api.flutter.dev/flutter/rendering/CustomPainterSemantics-class.html
- https://api.flutter.dev/flutter/rendering/CustomPainter-class.html
- https://github.com/flutter/flutter/issues/1666
- https://github.com/flutter/flutter/pull/13313/files
- https://developer.android.com/develop/ui/compose/accessibility/semantics
- https://developer.android.com/develop/ui/compose/testing/apis
- https://docs.flutter.dev/ai/mcp-server
- https://firefox-source-docs.mozilla.org/testing/marionette/Intro.html
- https://firefox-source-docs.mozilla.org/testing/marionette/Prefs.html
- https://bugzilla.mozilla.org/show_bug.cgi?id=870445
