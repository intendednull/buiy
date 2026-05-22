**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — aggregated keyboard contract conventions across the 32 patterns; cross-cutting key meanings and per-widget overrides Buiy must implement

# Keyboard contracts (cross-cutting)

APG codifies a small number of key-meaning conventions that recur across patterns. This file aggregates them and notes the per-widget exceptions. The per-widget keyboard contract is enumerated in [`patterns-catalog.md`](patterns-catalog.md); this file gives Buiy implementers the cross-cutting vocabulary so per-widget contracts read consistently.

The Buiy foundation [accessibility.md § 3.11](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to all of the below "per APG"; the verification harness gate 7 ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) is the APG keyboard contract suite.

## Tab and Shift+Tab — focus traversal between widgets

| Aspect | Rule |
|---|---|
| Direction | Tab forward (document order or focus-tree order); Shift+Tab reverse |
| Default eligibility | Every interactive widget is in the tab sequence exactly once |
| Composite widget entry | Tab enters the composite at the **active** descendant (the most recently focused or first item), not at every child |
| Composite widget exit | Tab leaves the composite to the next widget after the composite; arrow keys do NOT exit |
| Disabled widgets | `aria-disabled="true"` widgets remain focusable for AT discovery but should be skipped per WCAG 2.4.3 ("Focus Order" is preserved) — Buiy convention: emit `aria-disabled` and still in tab order so screen reader can announce "disabled"; alternative is to set tabindex=-1 |
| Modal containment | When a modal Dialog / AlertDialog is open, Tab must trap inside; APG's dialog example uses focus trap (not `inert` on the rest of the document, though `inert` is the HTML primitive Buiy's per-window equivalent uses) |

**Buiy mapping.** Focus tree per [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md) maintains the tab sequence; `:focus-visible` per [accessibility.md § Focus management](../../specs/2026-05-07-buiy-foundation/accessibility.md). Inert subtrees are first-class — they are excluded from focus, AccessKit, and hit-testing simultaneously.

## Arrow keys — navigation WITHIN composite widgets

Arrow-key semantics depend on the widget family. APG's conventions:

| Widget family | Arrow keys | Notes |
|---|---|---|
| **List-shaped** (Listbox, Menu, Menubar, Tree, Tabs) | Down/Up move focus within the composite | Wraps optional per APG (Buiy: configurable, default no-wrap on listbox; wrap on Menu / Menubar) |
| **Grid-shaped** (Grid, Treegrid, Table-if-interactive) | Right/Left within row, Down/Up within column | Wrapping crosses rows / cols per APG; Buiy default = no wrap |
| **Tabs horizontal** | Right/Left between tabs | If `aria-orientation="vertical"`, Down/Up instead |
| **Tabs vertical** | Down/Up between tabs | If `aria-orientation="horizontal"`, Right/Left instead |
| **Slider horizontal** | Right increment, Left decrement | Plus Up=increment, Down=decrement per APG |
| **Slider vertical** | Up increment, Down decrement | |
| **Spinbutton** | Up increment, Down decrement | |
| **Radio Group** | Down/Right move to next radio AND select it; Up/Left to previous AND select | Auto-select on arrow is APG-required; differs from Listbox where arrow only moves focus |
| **Menubar** | Right/Left between top-level items | Down opens submenu; Up does not in menubar |
| **Combobox (popup closed)** | Down opens popup and focuses first/active option; Up opens popup and focuses last | |
| **Combobox (popup open)** | Down/Up move active option (via `aria-activedescendant`) | Listbox-style navigation inside the open popup |

**Critical APG asymmetry: Radio vs Listbox vs Menu.** Radio's arrow keys *select on move* (selection follows focus). Listbox's arrow keys (by default) *only move focus*; selection is committed by Space or Enter — but some Listbox variants (e.g. single-select with `aria-activedescendant`) commit selection on focus change. Menu's arrow keys *only move focus*; activation is Enter / Space. Buiy widget catalog must pin the exact behaviour per widget — there is no single rule. APG documents each variant explicitly.

## Home / End — first / last item

| Widget family | Home | End |
|---|---|---|
| Listbox, Menu, Tree, Tabs, Toolbar | First item | Last item |
| Grid, Treegrid | First cell in current row | Last cell in current row |
| Slider | Min value | Max value |
| Spinbutton | Min value (if defined) | Max value (if defined) |
| Accordion headers | First header | Last header |

**Ctrl+Home / Ctrl+End** in **Grid** / **Treegrid** / **Feed**: jump to first / last cell or article of the entire structure.

## PageUp / PageDown — chunk navigation

| Widget family | Behaviour |
|---|---|
| Listbox / Menu | Move a screenful (commonly 5–10 items per APG; APG suggests "a number determined by the listbox") |
| Slider | Larger step (commonly 10× the regular step) |
| Spinbutton | Larger step |
| Grid / Treegrid | Move a screenful (one viewport of rows) |
| Feed | Move to next / previous **article**, not by screenful |

## Enter and Space — activation

APG carefully distinguishes Enter and Space; they are **not interchangeable** for every widget. The rules:

| Widget | Enter | Space |
|---|---|---|
| Button | activates | activates |
| Link | activates | does NOT activate (per APG; native `<a>` matches this) |
| Checkbox | does NOT toggle | toggles |
| Switch | toggles (APG) | toggles |
| Radio | does NOT (Tab + arrow handle selection) | activates the focused radio |
| Combobox | selects active option and closes | typed into the input as a character |
| Menu / Menubar / Menu Button | activates the item; opens submenu if `aria-expanded` | activates `menuitemcheckbox` / `menuitemradio`; activates a regular menuitem |
| Tab (manual-activate) | activates focused tab | activates focused tab |
| Tab (auto-activate) | n/a (selection follows focus) | n/a |
| Tree | activates / toggles selection | toggles selection (multi-select) |
| Dialog | depends on focused element | depends on focused element |

**Buiy implementation.** Per-widget keyboard handlers must encode this asymmetry. The verification harness fixture for each widget MUST include both Enter and Space test cases and assert the correct one activates per APG.

## Escape — cancel / dismiss

| Context | Behaviour |
|---|---|
| Dialog, AlertDialog | Dismiss the dialog (unless `closedby="none"` opts out) |
| Menu / Menubar / Menu Button | Close current menu; if in submenu, close submenu and return focus to parent item |
| Combobox (popup open) | Close popup, retain typed text |
| Tooltip | Dismiss tooltip without moving focus from trigger (WCAG 1.4.13) |
| Grid cell in edit mode | Exit edit mode, restore prior value |

Buiy's overlay state machine ([interaction.md](../../specs/2026-05-07-buiy-foundation/interaction.md), [media-and-widgets.md § Popover](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) implements light-dismiss + Escape via the HTML Popover `closedby` analogue.

## Type-ahead (first-letter search)

APG specifies type-ahead in **Menu**, **Menubar**, **Listbox**, **Combobox**, and **Tree**. Conventions:

- Typing a letter moves focus to the next item whose name starts with that letter
- Typing multiple letters within a short window (typically 500 ms) matches a longer prefix
- Wraps from end to beginning of the list
- Case-insensitive

**Locale gotcha.** Type-ahead is case-fold-and-locale-sensitive. Buiy uses ICU case folding via `unicode-case-mapping` (or similar) so that, e.g., Turkish "İ" → "i" matches per locale rules. Pin this in the per-widget contract.

## F2 — enter edit mode (Grid)

In **Grid** and **Treegrid**, **F2** enters edit mode for the focused cell; **Esc** exits edit mode and restores the prior value; **Enter** commits and exits. Buiy ships F2 in tier C (along with the Grid widget); Treegrid editing inherits the same contract.

## Per-widget overrides — quick reference

The places where the cross-cutting conventions break in non-obvious ways:

- **Combobox** Down/Up open the popup if closed (does NOT just move focus on the input). Closed-popup Down/Up has special semantics.
- **Radio Group** Tab enters at the **checked** radio (not the first). Down/Up auto-select.
- **Tabs (auto-activate)** selection follows focus on arrow; **Tabs (manual-activate)** does NOT — needs Enter / Space.
- **Tree** Right on a collapsed item expands it; on an expanded item moves to first child. Left on an expanded item collapses; on a collapsed item moves to parent.
- **Treegrid** Right/Left on a row header act like Tree expand/collapse; on a regular cell act like Grid arrow navigation.
- **Menu** Right on an item with submenu opens it; **Menubar** Right moves to next top-level item (NOT submenu).
- **Slider Multi-Thumb** Tab moves between thumbs; arrows operate within the focused thumb.
- **Window Splitter** Right/Left or Up/Down depend on `aria-orientation`; Home/End jump to min/max position.
- **Listbox vs Menu type-ahead** — Listbox can type past the first letter (multi-char prefix); Menu's APG example shows first-letter only but multi-char is permitted.

## What APG does NOT specify

APG **does not specify**:

- **Gamepad navigation.** No directional-pad, no analog-stick, no button mapping. Buiy must extend the contract for game-engine surfaces — see [`lessons.md § Diverge`](lessons.md).
- **Touch gestures** beyond what HTML / pointer events define. APG's "ensure all interactions have a keyboard alternative" (WCAG 2.5.7) is the only touch contract.
- **Voice control beyond what AT provides.**
- **Spatial navigation** (focus-on-target-in-direction). APG focus order is linear/sequential; spatial nav is an extension (Buiy implements it, partly inspired by `tv:remote` / Smart TV conventions and [bevy_picking](../bevy-picking/)).
- **In-world / diegetic UI navigation** — APG assumes 2D screen layouts.

These divergences are documented in [`evolution-and-gaps.md`](evolution-and-gaps.md) and [`lessons.md`](lessons.md).

## Sources

- APG patterns library: <https://www.w3.org/WAI/ARIA/apg/patterns/>
- Per-pattern keyboard sections: each pattern page under `https://www.w3.org/WAI/ARIA/apg/patterns/<pattern-name>/`
- WAI-ARIA 1.2 § Keyboard Interaction: <https://www.w3.org/TR/wai-aria-1.2/#keyboard>
- WCAG 2.1.1 Keyboard: <https://www.w3.org/WAI/WCAG22/Understanding/keyboard.html>
- WCAG 2.5.7 Dragging Movements: <https://www.w3.org/WAI/WCAG22/Understanding/dragging-movements.html>
- Buiy keyboard interaction patterns: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Sibling files: [`patterns-catalog.md`](patterns-catalog.md), [`focus-management.md`](focus-management.md), [`evolution-and-gaps.md`](evolution-and-gaps.md)
