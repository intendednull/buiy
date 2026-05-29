**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — focus management contracts: tab order, `:focus-visible`, focus traps, focus restoration, inert subtrees, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point

# Focus management

APG specifies focus management implicitly through every widget pattern; this file aggregates the cross-cutting focus-management primitives Buiy must implement. The Buiy foundation [`accessibility.md § Focus management`](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to all of the below as foundation-tier.

## Tab order

The fundamental rule: **Tab traverses the focusable element tree in document/visual order**. APG's contract is:

1. Every interactive widget is reachable by Tab exactly once
2. The order matches the visual reading order (WCAG 2.4.3)
3. Composite widgets are entered at their **active** descendant (most recently focused, or first if none); the rest of the composite is reached by arrow keys, NOT Tab
4. Tab leaves the composite at the next-focusable widget after it

**Buiy mapping.** Buiy maintains a focus tree per window. Each `Focusable` entity has a tabindex (≥0 for in-tab-sequence, -1 for programmatically-focusable-but-not-tabbable, omitted for non-focusable). The composite-widget pattern uses **either** roving tabindex **or** `aria-activedescendant` — see below.

## `:focus-visible` semantics

CSS `:focus-visible` (vs `:focus`) distinguishes "focus shown because of keyboard navigation" from "focus shown because of pointer click". APG's focus-ring contract assumes keyboard focus is visible; pointer focus may not show a ring depending on user agent heuristic.

**Buiy implementation.** A `FocusVisible` component flag, set when the focus change was driven by a keyboard event (Tab, arrow, type-ahead), cleared when driven by pointer. Theme tokens reference the focus-visible state for ring rendering. See [`accessibility.md § Focus management`](../../specs/2026-05-07-buiy-foundation/accessibility.md).

## Focus ring

WCAG 2.4.7 (Focus Visible, AA) and WCAG 2.4.13 (Focus Appearance, AAA):

- **AA (2.4.7):** focus indicator must be visible
- **AAA (2.4.13):** ≥2 px perimeter, ≥3:1 contrast vs unfocused

Buiy's default focus ring satisfies AAA (≥2 px, ≥3:1) per [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md).

## Focus traps (modal contexts)

APG's Dialog and AlertDialog patterns specify a **focus trap**: while the dialog is open, Tab cycles within it (Tab from last focusable wraps to first; Shift+Tab from first wraps to last). Focus cannot reach content outside the dialog via keyboard.

**Two implementation approaches:**

1. **Inert outside.** Mark all non-dialog UI as `inert` (or `aria-hidden="true"` + tabindex=-1 throughout); Tab naturally skips. HTML's `inert` attribute does this declaratively. Buiy uses this approach.
2. **Trap in handler.** Intercept Tab events; manually re-route last → first. Pre-`inert` approach.

The advantages of inert-outside: AT also recognises the rest of the page as inactive (correct semantics); pointer events are also excluded (correct interaction); the dialog is the only place focus can be.

**Buiy convention.** Dialog and AlertDialog automatically inert the rest of their window when open. The `closedby` policy (`any` / `closerequest` / `none`) controls dismiss; Esc, scrim click, and the lifecycle events `toggle`/`beforetoggle` per HTML Popover spec.

## Focus restoration

When an overlay (Dialog, Popover, Menu) closes, focus must return to the element that opened it (the invoker). APG specifies this in every overlay-pattern's "Keyboard Interaction" section.

**Buiy implementation.** Each overlay records the focus target at open and restores on close. Pinned in the widget catalog per overlay.

**Edge case.** If the invoker no longer exists (was destroyed while the overlay was open), focus restores to a sensible default — usually the next focusable in tab order from the invoker's last known position.

## Inert subtrees

HTML's `inert` attribute (and ARIA's equivalent semantics):

- Subtree is excluded from focus traversal
- Subtree is excluded from AccessKit (not announced)
- Subtree is excluded from hit-testing (pointer events pass through)
- Subtree is visually present (NOT `display: none`)

Buiy's `inert` analogue is foundation-tier; the modal-dialog pattern uses it; the verification harness gate 3 captures inert subtrees as excluded from the AccessKit tree.

`aria-hidden` is a SUBSET: it excludes from AT but NOT focus or hit-testing. Use `inert` for "fully out of bounds"; `aria-hidden` for "decorative, accessible elsewhere".

## Roving tabindex pattern

A composite widget (Menu, Tabs, Listbox without `activedescendant`, RadioGroup, Tree) uses **roving tabindex**:

- Exactly one descendant has `tabindex="0"` (in-sequence) at any time
- All other descendants have `tabindex="-1"` (focusable but not in tab sequence)
- Arrow keys MOVE the `tabindex="0"` from one descendant to another; the moved-to descendant gains DOM focus
- Tab enters the composite at the `tabindex="0"` descendant; Tab leaves to the next focusable after the composite

**Advantages.** Works with all AT; DOM focus matches AT focus.

**Disadvantages.** Mutates DOM on every arrow key; can be expensive in very large listboxes.

**Buiy implementation.** `RovingTabIndex` component on the composite widget root tracks the active descendant; arrow-key handlers update both DOM focus and the roving index.

## `aria-activedescendant` pattern

An alternative composite-widget focus mechanism:

- DOM focus stays on the composite widget root (e.g. the combobox input)
- `aria-activedescendant="id-of-active-option"` on the root names the "logical focus"
- Arrow keys change the active-descendant value (no DOM focus change)
- The visually-focused descendant is the one referenced by `aria-activedescendant`

**Used by.** Combobox, Listbox (specifically the editable-input + popup variants), grid cells in some grid patterns.

**Advantages.** DOM focus doesn't move (text input keeps focus); fewer events fire.

**Disadvantages.** AT-side support is uneven (especially AT-SPI on Linux — see [`platform-bindings.md`](platform-bindings.md)); requires careful keyboard handling.

**Buiy implementation.** `ActiveDescendant` component on the composite widget root. AccessKit's `active_descendant: NodeId` carries the value to the tree.

## Sequential Focus Navigation Starting Point

WAI-ARIA's `Action::SetSequentialFocusNavigationStartingPoint` (AccessKit `Action::SetSequentialFocusNavigationStartingPoint`) tells the focus model: "Tab from here next time the user presses Tab, regardless of where the actual focus is."

Use cases:
- AT moves the virtual-cursor reading position; user then presses Tab — focus should jump to the nearest tabbable from where the virtual cursor is
- A "skip to main content" link puts a starting point at the main landmark
- A search result highlights a snippet; Tab from the highlight should reach the linked widget, not the previously-focused element

**Buiy implementation.** AccessKit's action plumbing routes the request to Buiy's focus model, which records the requested starting point and uses it for the next Tab.

## Skip-link primitive

WCAG 2.4.1 (Bypass Blocks, A). A "skip to main content" link at the top of the page that is invisible until focused. Buiy ships this as a primitive `SkipLink` widget; uses `Action::SetSequentialFocusNavigationStartingPoint` to jump to the named landmark or region.

## Spatial focus navigation (gamepad / TV remote)

**APG does not cover this.** APG's focus model is sequential / arrow-key-within-widget. For game engines, gamepad D-pad and analog-stick navigation requires **spatial** focus traversal: "what is the focusable element to the right of the current focus?" — not "what's next in tab order".

Buiy's focus model extends to spatial navigation for foundation-tier; the algorithm is closer to Smart TV / console game conventions (cf. [`prior-art/unreal-slate-umg/`](../unreal-slate-umg/) CommonUI cardinal navigation, [`prior-art/rmlui/`](../rmlui/) `nav-up` / `nav-down` / `nav-left` / `nav-right` annotations). The Buiy implementation:

- For each focusable, compute the screen-space bounding box
- D-pad-up / down / left / right finds the nearest focusable in that direction (per a scoring function: distance + angular alignment)
- Override per-element: `SpatialNavOverride { up: Entity, ... }` lets authors pin the navigation graph manually

**APG conformance.** Spatial nav is an ADDITION to APG-compliant Tab + arrow navigation, not a replacement. Tab still works; D-pad is the alternative input.

## Open questions

- **`aria-activedescendant` on Linux/AT-SPI.** Orca historically lagged on this pattern; coverage uneven across AT-SPI versions. Buiy verification gate 3 captures the tree; manual-release-gate verifies actual AT behaviour. See [`platform-bindings.md`](platform-bindings.md) and [`accesskit/lessons.md § Avoid`](../accesskit/lessons.md).
- **Focus tree under inert.** When inert is toggled mid-overlay (e.g. nested modal), focus order recomputation must be correct. Buiy's focus tree recomputes on inert change.
- **Focus on detached entity.** If the focused entity is despawned, focus must move somewhere reasonable (next sibling? parent? landmark fallback?). Buiy convention: parent's next focusable child; fallback to first focusable in window.
- **3D-anchored UI focus.** Buiy's diegetic-UI ([`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)) puts widgets in 3D space against `Transform`. Tab order for non-screen-aligned widgets is undefined by APG; Buiy must define a deterministic order. Decision deferred to `buiy_3d` subspec.

## Sources

- WAI-ARIA 1.2 § 5.2.7 Keyboard Interaction: <https://www.w3.org/TR/wai-aria-1.2/#keyboard>
- HTML Living Standard § inert: <https://html.spec.whatwg.org/multipage/interaction.html#the-inert-attribute>
- WCAG 2.4.3 Focus Order: <https://www.w3.org/WAI/WCAG22/Understanding/focus-order.html>
- WCAG 2.4.7 Focus Visible: <https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html>
- WCAG 2.4.11 Focus Not Obscured (Minimum): <https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum.html>
- WCAG 2.4.13 Focus Appearance: <https://www.w3.org/WAI/WCAG22/Understanding/focus-appearance.html>
- AccessKit `Action` enum (incl. `SetSequentialFocusNavigationStartingPoint`): <https://docs.rs/accesskit/0.24.0/accesskit/enum.Action.html>
- Buiy focus model commitment: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Sibling files: [`patterns-catalog.md`](patterns-catalog.md), [`keyboard-contracts.md`](keyboard-contracts.md), [`platform-bindings.md`](platform-bindings.md)
