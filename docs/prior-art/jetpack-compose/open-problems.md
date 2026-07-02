**Date:** 2026-06-26
**Status:** active
**Subject:** Jetpack Compose — Android/Kotlin declarative UI as a COMPOSITION model (@Composable, Modifier chain, state hoisting, slot APIs, CompositionLocal)

# Open problems — what Compose structurally does NOT solve

Honest accounting of the gaps the *composition model* leaves open — the things
Buiy must solve itself because Compose offers no reusable mechanism, only (in
some cases) a negative result. Decisions and patterns are in
[lessons.md](lessons.md); the runtime split is in
[architecture.md](architecture.md). The semantics/automation lens develops the
a11y-tree gaps further at
[`../retained-mode-semantics-automation/open-problems.md`](../retained-mode-semantics-automation/open-problems.md).

## 1. It does not unify app state with the a11y/semantic tree — by design

App state lives in *hoisted, caller-owned* state (`mutableStateOf`/`ViewModel`);
the semantics tree is a *separately declared* layer (`Modifier.semantics`,
covered in the sibling). Compose therefore offers Buiy **no precedent for "app
state IS the AccessKit tree"** (**F1**) — its working answer is the opposite (a
distinct state/domain layer, with semantics *derived from* the widgets, never
the canonical store). Useful as evidence *for* separation, but it leaves Buiy's
*unification* question unanswered.

## 2. No framework-level typed change bus / introspectable binding

`value`/`onValueChange` is one-way and *function-local*; there is no queryable
registry of "widget X's value changed." An out-of-process agent (Buiy's target
consumer) cannot observe a widget's typed value change unless the app
explicitly wires it. The semantics-action channel is itself fire-and-forget
(see the sibling's open-problems §7). So **F2**'s typed binding is solved *at
the call site*, **not** as an introspectable event stream — the part Buiy most
needs (an agent-observable typed change signal) Compose does not provide.

## 3. Recomposition correctness is never statically guaranteed

Skipping depends on **stability**, which is *inferred* or *asserted* via
`@Stable`/`@Immutable` — annotations the compiler **trusts without verifying**.
`List`/`Set`/`Map` and any type from a module the Compose compiler didn't run
on are treated unstable → unnecessary re-runs. *Strong skipping mode* and the
compiler stability reports **mitigate** the silent-wrong cliff but do not
eliminate it. The "invisible until production" failure mode (**F3**) is a
permanent property of the model, not a bug. (This is precisely the tax Buiy is
right to avoid by staying retained + explicit `Changed<T>` — see
[lessons.md](lessons.md) A1/A2.)

## 4. `CompositionLocal` has no call-site guarantee a value was provided

For non-static locals there is no type-checked guarantee at the read site that
a provider exists upstream; the *workaround* is "always supply a default." The
implicit dependency is not checked where it is read — Google's own docs warn it
makes behavior "harder to reason about" and debugging requires walking the
composition to find the provider (**F6** guardrail; verbatim in
[lessons.md](lessons.md) A3).

## 5. Composition is code, not data — no serialized scene

A `@Composable` slot is a *closure*; there is **no document/data representation
of a composition**. Compose sidesteps **F7**'s "dynamic content in a data
scene" by making the entire UI *code*. A *data-driven* retained system (Buiy's
`bsn!` scenes) still has to solve dynamic/conditional content itself — Compose
contributes no reusable mechanism here, only the negative result that
closure-slots are the easy path and data-scenes are not. Buiy's analog (named
child-entity slots + ECS reconciliation) is strictly weaker than a live lambda,
and that gap is real.

## 6. Boilerplate is relocated, not removed

Compose trades retained-mode marker/tree-walk boilerplate (**F7**) for
**recomposition-management boilerplate**: `remember`, `derivedStateOf`, `key`,
`@Stable`/`@Immutable`, `movableContentOf`, stability-config files,
immutable-collection wrappers. Net verbosity (**F8**) is lower than imperative
View code but is **not** zero — and the new boilerplate is *correctness-critical*
(get a stability annotation wrong and updates silently break), not merely
verbose. Buiy should not assume "declarative" implies "less to manage"; it
implies *different* things to manage.

## 7. A guideline bounds drift but does not eliminate it

Even with a published, RFC2119-graded, enforced API style guide plus API-lint,
Compose still grew the `TextField` **two-state-model** double spelling
(`value`/`onValueChange` vs `state: TextFieldState`) once the controlled-value
convention hit the limits of complex editing state. So the **F5** cure (one
canonical spelling) is *bounded*, not absolute — pair any Buiy guideline with
lint AND a decided deprecation policy up front, and expect the hardest widgets
(text editing) to strain a single-spelling rule.

## 8. The compiler-coupling problem it *did* solve is one Buiy never has

A non-transfer, treated in full in [architecture.md](architecture.md) (the
Distribution lesson) and [lessons.md](lessons.md) A4: Compose's hardest
historical packaging problem was a runtime/compiler↔language version triangle,
resolved by merging the compiler into Kotlin 2.0. `bsn!` is **Bevy's** macro,
versioned with Bevy, so that problem — and its fix — simply do not apply here.

## Sources

- https://developer.android.com/develop/ui/compose/state-hoisting
- https://developer.android.com/develop/ui/compose/compositionlocal
- https://developer.android.com/develop/ui/compose/performance/stability
- https://developer.android.com/develop/ui/compose/mental-model
- https://developer.android.com/develop/ui/compose/text/migrate-state-based
- https://github.com/androidx/androidx/blob/androidx-main/compose/docs/compose-component-api-guidelines.md (read 2026-06-26)
- Cross-link (not duplicated): `../retained-mode-semantics-automation/open-problems.md`
