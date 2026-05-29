**Date:** 2026-05-22
**Status:** active
**Subject:** Focus in the Bevy stack — what `bevy_a11y` owns (almost nothing), what `bevy_input_focus` owns, what `bevy_feathers` styles, where the Buiy focus model diverges

## What bevy_a11y itself does for focus

**Almost nothing.** Direct verification against `bevy_a11y/src/lib.rs` on main HEAD (0.19.0-dev): no `Focus` resource, no focus-related component, no focus-related system, no focus-related event. The crate's items are `AccessibilityPlugin`, `AccessibilityRequested`, `ManageAccessibilityUpdates`, `AccessibilityNode`, `ActionRequest`, `AccessibilitySystems::Update`. None of these is a focus primitive.

This contradicts a common implication in surrounding documentation that `bevy_a11y` "owns focus tracking." Older Bevy versions may have had a `Focus` resource here (e.g. before the `bevy_input_focus` crate split out, plausibly pre-0.16); on current main and on 0.17.3 stable, there is none.

What `bevy_a11y` *does* contribute to focus is indirect: its `AccessibilityNode` components feed the `TreeUpdate.focus: NodeId` field via `bevy_winit::accessibility::update_adapter`. Each tree update carries one `focus: NodeId`, sourced from `bevy_input_focus`'s `InputFocus` resource (not from `bevy_a11y`). So the focus *signal* flows through bevy_a11y's tree-update path; the focus *state* lives in `bevy_input_focus`.

## Where focus actually lives — `bevy_input_focus`

Verified against [`bevy_input_focus/src/lib.rs`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_input_focus/src/lib.rs) on main HEAD. The crate exposes:

**Resources:**
- `InputFocus` — tracks which `Entity` currently has input focus. Methods: `set(entity)`, `get() -> Option<Entity>`, `clear()`.
- `InputFocusVisible` — boolean resource controlling whether focus indicators should display. This is the `:focus-visible` analogue.

**Components:**
- `AutoFocus` — marker that auto-sets focus to the entity on spawn.

**Events / messages:**
- `FocusedInput<M>` — generic bubbling input event dispatched to the focused entity.
- `AcquireFocus` — bubbling event that sets focus when handled.
- `FocusGained`, `FocusLost` — fired on focus changes.

**Trait:**
- `IsFocused` — methods `is_focused`, `is_focus_within`, `is_focus_visible`, `is_focus_within_visible`. Implemented for `World` and for `IsFocusedHelper` (a `SystemParam`).

**Sub-modules:**
- `autofocus`
- `directional_navigation` — spatial nav (arrow-key / gamepad)
- `gained_and_lost`
- `navigator`
- `tab_navigation` — tab/shift-tab between focusable entities

**Notably absent:**
- No mention of AccessKit or `bevy_a11y`. The crate is a standalone focus-tracking primitive; the bridge to AccessKit happens elsewhere (in `bevy_winit::accessibility::update_adapter`, which reads `InputFocus` and writes `TreeUpdate.focus`).
- No focus-trap / inert subtree / focus-restoration primitive at this layer.

## Tab navigation

In `bevy_input_focus::tab_navigation`. This is not in `bevy_a11y`. The module provides standard Tab / Shift+Tab traversal between focusable entities. The contract is "next focusable entity in document order"; the per-widget focusability is determined by entity components.

Roving tabindex (a single entity in a composite widget owns the tabindex; arrow keys move within) — not directly exposed as a primitive in `bevy_input_focus`. It's the consumer's responsibility (typically a widget library like `bevy_ui_widgets`) to implement the pattern.

`aria-activedescendant` — not modeled in the Bevy focus stack today. The AccessKit `Action::SetSequentialFocusNavigationStartingPoint` exists as a route from AT into the focus model, but Bevy doesn't have a public binding for it yet.

## Spatial / directional navigation

In `bevy_input_focus::directional_navigation`. The algorithm: manual edges in `DirectionalNavigationMap` take priority; fallback computes the best candidate by `CompassOctant` direction filtered by visibility + screen-bounds + target-camera. Comparable to the third-party `iyes_ui_navigation` crate but now ships with the engine (Bevy 0.18+).

This is what gamepad users / arrow-key users get for spatial movement. It runs in parallel to tab navigation (which is sequential). The two are not unified — `bevy_input_focus` has separate sub-modules for them.

## Focus interaction with `bevy_feathers`

`bevy_feathers` is Bevy's first-party styled-widget catalog (post-0.18). Its `:focus-visible` styling reads `InputFocusVisible` (the `bevy_input_focus` resource) and `IsFocused` (the trait) to decide whether to render the focus ring. The styling is per-widget; there is no unified focus-ring primitive.

Per [`/home/user/buiy/docs/prior-art/bevy-feathers/`](../bevy-feathers/) (sibling folder), bevy_feathers' focus visualization is intentionally minimal — a default outline color, no positioning rules, no contrast-checking against background. Per the foundation spec ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)), Buiy commits to ≥2 px perimeter, ≥3:1 contrast vs unfocused (WCAG 2.4.11); bevy_feathers does not.

## What's missing from the Bevy focus stack (relative to web / APG / Buiy needs)

Verified gaps as of main HEAD 2026-05-22:

| Capability | Bevy stack status | Source |
|---|---|---|
| Focus trap for modal dialogs | Not a primitive. Consumers implement. | `bevy_input_focus` source review |
| Focus restoration after dialog close | Not a primitive. | same |
| Inert subtrees (excluded from focus + a11y + hit-testing) | Not a primitive. | same |
| Roving tabindex pattern | Per-widget responsibility. | same |
| `aria-activedescendant` | Not modeled. | same |
| `Sequential focus navigation starting point` | Not implemented; AccessKit action exists. | `accesskit::Action` enum |
| `:focus-visible` semantics (vs `:focus`) | `InputFocusVisible` resource exists, but the "visible vs not visible" heuristic (last input was keyboard?) is consumer-driven. | `bevy_input_focus` source |
| Skip-link primitive | Not a primitive. | source review |
| Focus ring with WCAG 2.4.11 contrast | Not enforced. bevy_feathers ships a default outline; contrast is consumer's problem. | `bevy_feathers` |

Each of these is **foundation-tier in Buiy** ([`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) "Focus management"). The gap list is one of the structural reasons Buiy owns its focus tree end-to-end rather than depending on `bevy_input_focus`.

## Buiy's focus model — what it owns

Per [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.3 and [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) "Focus management":

- **Single focus tree per Buiy window**, derived from the Buiy hierarchy, with `:focus-visible` semantics derived from input-source tracking (last input was keyboard or programmatic → `focus-visible`; last input was pointer → focus but not `focus-visible`).
- **Focus traps** automatic for `Dialog` / `AlertDialog`. Inert subtrees excluded.
- **Focus restoration** on overlay close.
- **Inert subtrees** — excluded from focus + AccessKit + hit-testing in one consistent boundary.
- **Roving tabindex** as a primitive (not per-widget).
- **`aria-activedescendant`** as a primitive.
- **Sequential focus navigation starting point** — Buiy implements `accesskit::Action::SetSequentialFocusNavigationStartingPoint`; the AT relays "Tab from here" requests and Buiy's focus model honors them.
- **Spatial gamepad nav** unified with sequential Tab nav under a single focus model. The algorithm is the same shape as `bevy_input_focus::directional_navigation` (manual edges + CompassOctant fallback) but lives in Buiy's focus crate.
- **Focus ring** ≥2 px perimeter, ≥3:1 contrast vs unfocused (WCAG 2.4.11), with per-theme tokens.
- **Skip-link primitive** as a Buiy widget (visible on focus, jumps to main / a region).

The `TreeUpdate.focus: NodeId` field in AccessKit is set every frame from Buiy's focus state (Buiy's `Entity::to_bits()` is the NodeId). Whether the focus *changed* or not, the field is set on every update — AccessKit treats the focus field as authoritative-per-update, not as a transition signal (see [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — "Treating `is_focused` as a node-state field" pitfall, mitigation: focus is `TreeUpdate.focus`, not a per-node bool).

## Why Buiy can't borrow `bevy_input_focus`

Three reasons, in increasing order of importance:

1. **Component-model mismatch.** Buiy's focus tree is rooted at Buiy's per-window root, not at a global entity. `bevy_input_focus`'s `InputFocus` is a single global `Option<Entity>` — fine for a single bevy_ui app, structurally wrong for a multi-window stack where each window has its own focus tree.

2. **Feature gap.** Most foundation-tier Buiy focus capabilities (traps, restoration, inert, `:focus-visible` source-tracking, focus-ring contrast enforcement, skip-link) are absent from `bevy_input_focus`. Layering a feature crate on top of it would mean reimplementing most of the focus model anyway, with `bevy_input_focus`'s primitives as a substrate for the simple cases. The substrate isn't load-bearing enough to justify the dependency.

3. **AccessKit bridge.** `bevy_input_focus` doesn't push focus to AccessKit; `bevy_winit::accessibility` does, by reading `InputFocus` and writing `TreeUpdate.focus`. On Buiy-owned windows, `bevy_winit::accessibility` isn't running (the adapter is Buiy's, not bevy_winit's). So the bridge would need to be rebuilt anyway.

Net: Buiy owns its focus tree. `bevy_input_focus` continues to exist for bevy_ui-owned windows in mixed-stack apps. The two focus stacks are independent; cross-window focus transitions (Tab from Buiy window to bevy_ui window) are not in the v1 contract.

## Cross-references

- [`architecture.md`](architecture.md) — bevy_a11y's tree-update push that carries `TreeUpdate.focus`
- [`api.md`](api.md) — bevy_a11y has no focus API
- [`coexistence.md`](coexistence.md) — Buiy windows don't use `bevy_input_focus` either
- [`component-model-incident.md`](component-model-incident.md) — separate but related case study (the megacomponent problem)
- [`/home/user/buiy/docs/prior-art/accesskit/tree-model.md`](../accesskit/tree-model.md) — `Tree.focus: NodeId` semantics
- [`/home/user/buiy/docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — focus-as-`TreeUpdate.focus` pitfall row
- [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md` § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md) — Buiy focus model
- [`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) — focus-management feature list

## Sources

- `bevy_a11y/src/lib.rs` (verified no Focus resource on main HEAD): https://github.com/bevyengine/bevy/blob/main/crates/bevy_a11y/src/lib.rs
- `bevy_input_focus/src/lib.rs` (main HEAD): https://github.com/bevyengine/bevy/blob/main/crates/bevy_input_focus/src/lib.rs
- `bevy_input_focus/src/tab_navigation.rs`: https://github.com/bevyengine/bevy/blob/main/crates/bevy_input_focus/src/tab_navigation.rs
- `bevy_input_focus/src/directional_navigation.rs`: https://github.com/bevyengine/bevy/blob/main/crates/bevy_input_focus/src/directional_navigation.rs
- `bevy_winit/src/accessibility.rs` (the bridge that reads `InputFocus` and writes `TreeUpdate.focus`): https://github.com/bevyengine/bevy/blob/main/crates/bevy_winit/src/accessibility.rs
- Buiy foundation — architecture § 2.3: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation — accessibility (Focus management): [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
