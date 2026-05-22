**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — the ARIA 1.2 role / state / property vocabulary as the contract Buiy emits via AccessKit, with mapping to AccessKit's `Role` / state / relation API

# Roles, states, and properties (the ARIA vocabulary)

This file documents the ARIA-1.2 vocabulary as Buiy must emit it through AccessKit. The normative source is the [WAI-ARIA 1.2 Recommendation](https://www.w3.org/TR/wai-aria-1.2/) (6 June 2023); APG is the non-normative usage guide. The Buiy enumeration of which states/properties are foundation-tier lives in [accessibility.md § 3.11](../../specs/2026-05-07-buiy-foundation/accessibility.md).

## Roles — the role taxonomy

WAI-ARIA 1.2 defines a closed role taxonomy. Each role falls into one of six categories:

1. **Abstract roles** — never used directly in markup; structure only (`widget`, `composite`, `landmark`, `structure`, `window`, `range`, `select`, ...). Buiy never emits these.
2. **Widget roles** — interactive controls. Sub-divides into:
   - **Standalone** (20): `button`, `checkbox`, `gridcell`, `link`, `menuitem`, `menuitemcheckbox`, `menuitemradio`, `option`, `progressbar`, `radio`, `scrollbar`, `searchbox`, `separator` (when focusable), `slider`, `spinbutton`, `switch`, `tab`, `tabpanel`, `textbox`, `tooltip`, `treeitem`
   - **Composite** (9): `combobox`, `grid`, `listbox`, `menu`, `menubar`, `radiogroup`, `tablist`, `tree`, `treegrid`
3. **Document structure roles** (38): `article`, `blockquote`, `caption`, `cell`, `code`, `columnheader`, `definition`, `deletion`, `document`, `emphasis`, `feed`, `figure`, `generic`, `group`, `heading`, `img` / `image`, `insertion`, `list`, `listitem`, `mark`, `math`, `meter`, `none` / `presentation`, `note`, `paragraph`, `row`, `rowgroup`, `rowheader`, `separator` (non-focusable), `strong`, `subscript`, `superscript`, `suggestion`, `table`, `term`, `time`, `toolbar`
4. **Landmark roles** (8): `banner`, `complementary`, `contentinfo`, `form`, `main`, `navigation`, `region`, `search`
5. **Live region roles** (5): `alert`, `log`, `marquee`, `status`, `timer` — plus the implicit-live `alert` and `status`
6. **Window roles** (2): `alertdialog`, `dialog`

**Deprecated and not emitted by Buiy:** `directory` (deprecated in ARIA 1.2). The Buiy foundation [`accessibility.md § 3.11`](../../specs/2026-05-07-buiy-foundation/accessibility.md) lists this as out-of-scope.

**AccessKit mapping.** AccessKit's `Role` enum has 182 `#[repr(u8)]` variants covering ARIA roles plus document-structural and platform roles. Most ARIA roles map 1:1 to an AccessKit `Role` variant; deliberate divergences (per [`accesskit/tree-model.md`](../accesskit/tree-model.md)):

- ARIA `combobox` splits into AccessKit `ComboBox` (read-only popup) and `EditableComboBox` (editable text + popup) — AccessKit pre-decides what ARIA infers from `aria-expanded` + presence of `<input>`
- ARIA `checkbox` + `aria-pressed` (toggle button) unifies into AccessKit `CheckBox` + `Toggled` enum
- ARIA `img` and `image` are both spelled `Image` in AccessKit (single role)
- ARIA `none` and `presentation` are both `Role::Generic` or omitted via AccessKit "hidden / virtual" mechanisms
- Composite roles map directly: `listbox`, `combobox`, `menu`, `menubar`, `tree`, `treegrid`, `grid`, `radiogroup`, `tablist`

The role-description fallback (`Node::set_role_description("custom name")` + `Role::Generic`) is the supported escape hatch when no `Role` variant fits.

## States — value can change at runtime

ARIA states are attributes whose value is **expected to change in response to user interaction**. APG uses them to communicate dynamic widget state.

| State | Values | Used on | AccessKit field |
|---|---|---|---|
| `aria-busy` | `true` / `false` | any live region or container loading | `is_busy: bool` |
| `aria-checked` | `true` / `false` / `mixed` | `checkbox`, `radio`, `menuitemcheckbox`, `menuitemradio`, `switch`, `option`, `treeitem` | `Toggled { False, True, Mixed }` |
| `aria-disabled` | `true` / `false` | any focusable | `is_disabled: bool` |
| `aria-expanded` | `true` / `false` / undefined | `button`, `combobox`, `treeitem`, `link`, expandable items | `Expanded(Option<bool>)` |
| `aria-hidden` | `true` / `false` / undefined | any element to hide from AT (rarely; prefer `inert`) | hidden via different mechanism |
| `aria-invalid` | `true` / `false` / `grammar` / `spelling` | form fields | `Invalid { False, True, Grammar, Spelling }` |
| `aria-pressed` | `true` / `false` / `mixed` | toggle `button` | unified with `aria-checked` → `Toggled` |
| `aria-selected` | `true` / `false` / undefined | `option`, `tab`, `gridcell`, `row`, `columnheader`, `rowheader`, `treeitem` | `Selected(Option<bool>)` |

**Tri-state matters.** `Selected`, `Expanded` are tri-state (true / false / not-applicable). Mis-emitting "not applicable" as `false` is wrong — it tells the AT the item is collapsible / selectable when it isn't. See [`accesskit/lessons.md § Avoid`](../accesskit/lessons.md) for the corresponding pitfall.

**`Toggled::Mixed` matters.** `aria-checked="mixed"` is the tri-state checkbox (a parent checkbox over partially-checked children); `aria-pressed="mixed"` is the partially-applied formatting toggle (some of the selected text is bold, some isn't). Both are real production cases; Buiy emits `Toggled::Mixed` per [`accessibility.md § 3.11`](../../specs/2026-05-07-buiy-foundation/accessibility.md).

## Properties — describe relationships, capabilities, or essential attributes

ARIA properties are attributes whose value is **less likely to change**; they describe the widget's role-essential attributes. The full ARIA 1.2 property set:

### Widget properties

| Property | Values / form | Used on | Notes |
|---|---|---|---|
| `aria-autocomplete` | `none` / `inline` / `list` / `both` | `combobox`, `textbox`, `searchbox` | |
| `aria-haspopup` | `false` / `true` / `menu` / `listbox` / `tree` / `grid` / `dialog` | trigger of a popup | `true` defaults to `menu` |
| `aria-label` | string | any widget | direct label |
| `aria-level` | integer | `heading`, `treeitem`, `row` | hierarchical level (1-based) |
| `aria-modal` | `true` / `false` | `dialog`, `alertdialog` | indicates modal |
| `aria-multiline` | `true` / `false` | `textbox` | multi-line input |
| `aria-multiselectable` | `true` / `false` | `listbox`, `tree`, `grid`, `tablist` | |
| `aria-orientation` | `horizontal` / `vertical` / undefined | composite widgets | |
| `aria-placeholder` | string | `textbox`, `searchbox` | placeholder hint |
| `aria-readonly` | `true` / `false` | inputs | |
| `aria-required` | `true` / `false` | form fields | |
| `aria-sort` | `ascending` / `descending` / `none` / `other` | `columnheader`, `rowheader` | |
| `aria-valuemax` | number | `slider`, `spinbutton`, `progressbar`, `meter`, `scrollbar`, `separator` (when focusable) | |
| `aria-valuemin` | number | as above | |
| `aria-valuenow` | number | as above | |
| `aria-valuetext` | string | as above | human-readable value (e.g. "75% — high") |

### Live region properties (see [`live-regions.md`](live-regions.md))

| Property | Values |
|---|---|
| `aria-live` | `off` / `polite` / `assertive` |
| `aria-atomic` | `true` / `false` |
| `aria-relevant` | `additions` / `removals` / `text` / `all` (space-separated tokens) |

### Relationship properties

| Property | Values | Used on | AccessKit relation |
|---|---|---|---|
| `aria-activedescendant` | id-ref | composite widget | `active_descendant: NodeId` |
| `aria-colcount` | integer | `grid`, `table`, `treegrid` | `column_count: usize` |
| `aria-colindex` | integer | `cell`, `gridcell`, `row`, headers | `column_index: usize` |
| `aria-colindextext` | string | `cell` | column index text |
| `aria-colspan` | integer | `cell`, `gridcell` | `column_span: usize` |
| `aria-controls` | id-ref list | any | `controls: Vec<NodeId>` |
| `aria-describedby` | id-ref list | any | `described_by: Vec<NodeId>` |
| `aria-description` | string | any (ARIA 1.3, in ARIA 1.2 as informative) | `description: String` |
| `aria-details` | id-ref list | any | `details: Vec<NodeId>` |
| `aria-errormessage` | id-ref | invalid field | `error_message: NodeId` |
| `aria-flowto` | id-ref list | any | `flow_to: Vec<NodeId>` |
| `aria-labelledby` | id-ref list | any | `labelled_by: Vec<NodeId>` (British spelling on AccessKit side) |
| `aria-owns` | id-ref list | any | `owns: Vec<NodeId>` (parent-override) |
| `aria-posinset` | integer | item in a set | `position_in_set: usize` |
| `aria-rowcount` | integer | `grid`, `table`, `treegrid` | `row_count: usize` |
| `aria-rowindex` | integer | `row`, cells | `row_index: usize` |
| `aria-rowindextext` | string | `row` | |
| `aria-rowspan` | integer | cells | `row_span: usize` |
| `aria-setsize` | integer | item in a set | `size_of_set: usize` |

### Global properties (apply to all roles)

| Property | Values | Notes |
|---|---|---|
| `aria-current` | `page` / `step` / `location` / `date` / `time` / `true` / `false` | for "you are here" markers in navigation |
| `aria-keyshortcuts` | string (space-separated shortcuts) | informs AT of keyboard shortcuts; WCAG 2.1.4 |
| `aria-roledescription` | string | overrides role announcement (use sparingly) |
| `aria-braillelabel` | string | braille-specific label (ARIA 1.2) |
| `aria-brailleroledescription` | string | braille-specific role description (ARIA 1.2) |

## `aria-describedby` vs `aria-details` policy

| Use | When |
|---|---|
| `aria-describedby` | Short, flat string references (typical help text, tooltips, character-counter hints) |
| `aria-details` | Rich, structured supporting content (long descriptions, tables, footnotes, glossary pop-outs) |

ARIA 1.2 introduces this distinction; the Buiy widget catalog ([`media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) specifies which each widget emits.

## Deprecated and out

- `aria-grabbed`, `aria-dropeffect` — deprecated in ARIA 1.2. Buiy does not emit them. The replacement contract: every drag-driven widget exposes a Move-to AccessKit action (`Increment` / `Decrement` for ordered lists; custom action for arbitrary positioning), a keyboard alternative per WCAG 2.5.7, and polite live-region announcements on drag start / drag end / drop / cancel. See [`accessibility.md § 3.11 Drag/drop ARIA`](../../specs/2026-05-07-buiy-foundation/accessibility.md).

## AccessKit gotchas

| Gotcha | Source |
|---|---|
| British spelling: `labelled_by` not `labeled_by` | AccessKit `Node` API. Buiy's `A11yRelations` component must match. |
| `Selected` / `Expanded` as `Option<bool>` not `bool` | Tri-state per ARIA. |
| Use `Toggled` enum, not `bool`, for checked/pressed | `Toggled { False, True, Mixed }` |
| `aria-relevant` not in AccessKit | Buiy implements live-region filtering on its own side. See [`live-regions.md`](live-regions.md). |
| `Role` enum is `#[repr(u8)]` closed — no extension | Fall back to `Role::Generic` + `set_role_description(str)`. |
| Rich text doesn't pass through structured | Multi-run cosmic-text paragraphs flatten to single `value` on the containing node; structure conveyed via tree shape only. |

## Sources

- WAI-ARIA 1.2: <https://www.w3.org/TR/wai-aria-1.2/>
- Role definitions index: <https://www.w3.org/TR/wai-aria-1.2/#role_definitions>
- State and property reference: <https://www.w3.org/TR/wai-aria-1.2/#state_prop_def>
- ARIA in HTML: <https://www.w3.org/TR/html-aria/>
- Buiy foundation accessibility: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- AccessKit Role enum: <https://docs.rs/accesskit/0.24.0/accesskit/enum.Role.html>
- AccessKit tree-model: [`docs/prior-art/accesskit/tree-model.md`](../accesskit/tree-model.md)
- AccessKit lessons (Avoid section): [`docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md)
- Sibling files: [`patterns-catalog.md`](patterns-catalog.md), [`name-computation.md`](name-computation.md), [`live-regions.md`](live-regions.md)
