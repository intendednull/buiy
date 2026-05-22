**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — what the schema can express (Role / state / relation / live-region / text), and the explicit gap list of what it deliberately doesn't

## What AccessKit's tree CAN express

AccessKit's `accesskit::Node` schema is described in the upstream README as "based largely on Chromium's cross-platform accessibility abstraction" ([README](https://github.com/AccessKit/accesskit/blob/main/README.md)). That lineage is load-bearing: Chromium's `ui::AXNode` already had to map ARIA → UIA + NSAccessibility + AT-SPI, and AccessKit inherits both the shape and the mapping table. The result is a tree model that is *WAI-ARIA-aligned* without being a one-to-one ARIA implementation — most ARIA concepts have a representation, but the spelling is AccessKit's, not ARIA's.

## Role mapping (ARIA → AccessKit `Role`)

The `Role` enum at 0.24.0 has **182 variants**, declared `#[repr(u8)]` and ordered by expected frequency rather than alphabetically (the ordering is a serialization optimisation — variable-length encoding hits the common variants in the low byte) ([Role docs](https://docs.rs/accesskit/latest/accesskit/enum.Role.html)). The docstring states: "The majority of these roles come from the ARIA specification. Reference the latest draft for proper usage."

Sample mapping (the full table is in [`tree-model.md`](tree-model.md)):

| ARIA role | AccessKit `Role` | Notes |
|---|---|---|
| `button` | `Role::Button` | `DefaultButton` distinguishes the default-action button in a form. |
| `link` | `Role::Link` | |
| `combobox` | `Role::ComboBox` (read-only) / `Role::EditableComboBox` | AccessKit splits the two ARIA 1.2 combobox flavours into distinct variants. |
| `grid` | `Role::Grid` | `GridCell`, `Row`, `RowHeader`, `ColumnHeader` companion roles. |
| `tree` | (no exact match — use `Role::ListBox` + `Role::TreeItem` children, or `Role::TreeGrid` for tree-grid) | The 182-variant Role enum has `TreeItem` and `TreeGrid` but the "naked tree" container role is **not in the verified slice from docs.rs**; verify against the next release. |
| `tabpanel` | `Role::TabPanel` | |
| `menu` / `menubar` | `Role::Menu` / `Role::MenuBar` | |
| `tablist` | `Role::TabList` | |
| `radiogroup` | `Role::RadioGroup` | |
| `textbox` (single-line) | `Role::TextInput` | Specialised: `EmailInput`, `NumberInput`, `PasswordInput`, `PhoneNumberInput`, `SearchInput`, `UrlInput`, `DateInput`, `ColorWell`. |
| `textbox` (multi-line) | `Role::MultilineTextInput` | |

## ARIA states → AccessKit boolean / tri-state setters

From [`Node` docs](https://docs.rs/accesskit/latest/accesskit/struct.Node.html):

| ARIA state / property | AccessKit setter | Type |
|---|---|---|
| `aria-disabled` | `set_disabled()` | flag (clear via `clear_disabled()`) |
| `aria-hidden` | `set_hidden()` | flag |
| `aria-busy` | `set_busy()` | flag |
| `aria-modal` | `set_modal()` | flag |
| `aria-required` | `set_required()` | flag |
| `aria-readonly` | `set_read_only()` | flag |
| `aria-multiline` | `set_multiline()` | flag |
| `aria-multiselectable` | `set_multiselectable()` | flag |
| `aria-checked` (`true`/`false`/`mixed`) | `set_toggled(Toggled)` | `Toggled` enum: `True`, `False`, `Mixed`. |
| `aria-pressed` (`true`/`false`/`mixed`) | `set_toggled(Toggled)` | same enum (AccessKit unifies checked/pressed). |
| `aria-expanded` | `set_expanded()` / `set_collapsed()` | (verify exact spelling against current API). |
| `aria-selected` | `set_selected()` | flag. |
| `aria-orientation` (`horizontal`/`vertical`) | `set_orientation(Orientation)` | enum. |
| `aria-haspopup` (`menu`/`listbox`/`tree`/`grid`/`dialog`) | `set_has_popup(HasPopup)` | enum. |
| `aria-autocomplete` | `set_auto_complete(AutoComplete)` | enum. |
| `aria-invalid` (`grammar`/`spelling`/`true`/`false`) | `set_invalid(Invalid)` | enum. |
| `aria-current` | `set_aria_current(AriaCurrent)` | enum. |
| `aria-live` (`polite`/`assertive`) + politeness | `set_live(Live)` | enum. |

The unification of `aria-checked` and `aria-pressed` into a single `Toggled` enum is one of AccessKit's deliberate consolidations away from ARIA's surface, on the basis that the underlying platform APIs (UIA `TogglePattern`, NSAccessibility `AXValue`) already model both as one concept.

## ARIA relations → AccessKit Node relations

| ARIA relation | AccessKit setter |
|---|---|
| `aria-labelledby` | `set_labelled_by([NodeId])` |
| `aria-describedby` | `set_described_by([NodeId])` |
| `aria-controls` | `set_controls([NodeId])` |
| `aria-activedescendant` | `set_active_descendant(NodeId)` |
| `aria-flowto` | `set_flow_to([NodeId])` |
| `aria-details` | `set_details([NodeId])` |
| `aria-errormessage` | `set_error_message(NodeId)` |
| `aria-owns` | `set_owns([NodeId])` |
| (popup target) | `set_popup_for(NodeId)` |
| (radio group membership) | `set_radio_group([NodeId])` |
| (label container) | `set_label([str])` direct, vs `set_labelled_by` for indirect. |

The `set_labelled_by` / `set_described_by` distinction matters for the [ACCNAME 1.2](https://www.w3.org/TR/accname-1.2/) algorithm — AccessKit holds the *references*; the consuming toolkit (Buiy in `buiy_core` per [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)) walks the references to resolve the final name.

## Live regions

AccessKit's `set_live(Live)` covers ARIA's `aria-live` (`off` / `polite` / `assertive`). `set_busy()` plus a politeness setting also covers `aria-busy`. The flags for `aria-atomic` and `aria-relevant` exist as additional Node setters; the *announcement orchestration* (timing, queueing, replacement vs append) is host-side, not AccessKit-side. Buiy ships a "global announcer service" resource (per [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)) that materialises ad-hoc announcements as transient live-region nodes pushed via `TreeUpdate`.

## Text content for screen readers

AccessKit has three text-content axes on a `Node`:

- `set_label([str])` — the **accessible name** (short, primary identifier; equivalent to `aria-label`).
- `set_description([str])` — supplementary description (equivalent to `aria-description`).
- `set_value([str])` — current value of a control (input contents, slider position as text, etc.).
- `set_placeholder([str])` — text-input placeholder.

Rich text and hypertext are **explicitly out of scope** at the current release: the README states "[the released adapters] don't yet support rich text or hypertext." For Buiy, this means rich text content (cosmic-text-shaped multi-run paragraphs) is exposed to AccessKit as a flattened string label on the containing node, with structure conveyed only through the role taxonomy (`Paragraph`, `Heading`, `Emphasis`, `Strong`, etc.) and parent/child nesting — not via inline runs.

## What AccessKit deliberately CANNOT express

### Keyboard contracts

The WAI-ARIA APG (Authoring Practices Guide) keyboard contracts — "tab focuses combobox; arrow-down opens listbox; type-ahead first-letter narrows options" — are **not in AccessKit**. AccessKit only models the tree (role, state, relations) and the actions an AT may request (`Click`, `Focus`, `SetValue`, `ScrollIntoView`, etc., 22 variants in [`Action`](https://docs.rs/accesskit/latest/accesskit/enum.Action.html)). Implementing the APG keyboard contract is the consumer's job. Buiy spec'd this explicitly in [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) — "Per-widget contracts enumerated in `buiy-widget-catalog-design`."

### Custom widget patterns outside the 182 roles

The `Role` enum is closed (it's `#[repr(u8)]`). Apps cannot register custom roles. If a UI invents a widget pattern that doesn't fit one of the 182 variants, the integration option is `Role::Generic` plus `set_role_description([str])` to give the AT a string to read. The Buiy approach should be: every widget in the catalog picks an existing Role; bespoke widgets fall back to `Generic` + `role_description`.

### Forced-colors mode rendering hints

AccessKit does not dictate visual rendering. Forced-colors mode (`forced-colors: active`) is a CSS-level / consumer-level concern. The Buiy spec covers this in [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) under "User preferences" — forced-colors is honored at the Buiy theme / token layer, not via AccessKit.

### Reduced motion

Same shape as forced-colors. `prefers-reduced-motion` is honored by the Buiy animation pipeline; AccessKit has no role here. (`Action::ShowTooltip` and the auto-show/auto-hide timing is consumer-controlled; AccessKit just relays the request.)

### Inert subtrees

AccessKit's `set_hidden()` flag is the closest analogue to ARIA's `aria-hidden` and HTML's `inert`. It removes a subtree from AT exposure. However, *programmatic focus exclusion* (the other half of `inert` — focus traversal must skip the subtree even when the user Tab-cycles) is **not** AccessKit's job. The host has to enforce the focus-skipping side; AccessKit only handles the tree-exposure side. Buiy enforces both: `A11yStates::Hidden` triggers `set_hidden(true)` for the AT side, and the Buiy focus model excludes the same subtree from Tab traversal.

### Sequential-focus-navigation-starting-point

AccessKit has `Action::SetSequentialFocusNavigationStartingPoint` for the *incoming side* (an AT can request the host set the starting point). The actual implementation of "Tab from here" is host-side. Buiy implements this in its focus model, not in the A11y subsystem; the AccessKit action is just a route into Buiy's focus API.

### APCA / WCAG contrast

Out of scope by design. AccessKit models the tree, not the rendered pixels. Buiy's contrast verification runs separately as a CI linter against the theme tokens (see [`accessibility.md § "Visual a11y"`](../../specs/2026-05-07-buiy-foundation/accessibility.md)).

## Where each gap is addressed in Buiy

- Keyboard contracts → `buiy-widget-catalog-design` per-widget keyboard contracts.
- Custom roles / role_description fallback → Buiy widget catalog assigns Role per widget; bespoke widgets use `Generic` + `role_description`.
- Forced-colors / reduced-motion → Buiy theme tokens + animation gating (see [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md), [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)).
- Inert subtrees → Buiy focus model + `A11yStates::Hidden` (see [`accessibility.md § "Focus management"`](../../specs/2026-05-07-buiy-foundation/accessibility.md)).
- Sequential-focus-navigation-starting-point → Buiy focus model (see [`accessibility.md § "Focus management"`](../../specs/2026-05-07-buiy-foundation/accessibility.md)).
- Contrast / target size → Buiy verification harness (see [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)).

## Cross-links

- Role enum full list / Node payload shape: [`tree-model.md`](tree-model.md).
- Adapter API surface: [`api.md`](api.md).
- Platform-side mapping (UIA / NSAccessibility / AT-SPI): [`platform-adapters.md`](platform-adapters.md).
- Open questions about role coverage and AT-SPI quirks: [`critiques.md`](critiques.md).

## Sources

- https://github.com/AccessKit/accesskit/blob/main/README.md
- https://docs.rs/accesskit/latest/accesskit/enum.Role.html
- https://docs.rs/accesskit/latest/accesskit/enum.Action.html
- https://docs.rs/accesskit/latest/accesskit/struct.Node.html
- https://www.w3.org/TR/accname-1.2/
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/accessibility.md
