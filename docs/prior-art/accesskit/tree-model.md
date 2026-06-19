**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — the `Node` data model: roles, actions, states, relations, text, geometry

The `Node` struct is the unit of producer-side accessibility information. Every interactive surface, every label, every container, every text run maps to one `Node` (or in the immediate-mode case, one ephemeral `Node` per frame). This file enumerates the field surface as of `accesskit` 0.24.0 (published 2026-02-01) and the recent dev cycle (`accesskit_consumer` 0.36.0 published 2026-05-11). The data shape is defined in `common/src/lib.rs`; for setter-method documentation see [api.md](api.md).

## Role

`Role` is a flat `#[repr(u8)]` enum with over 160 variants. The intent is to be a superset of the unioned ARIA 1.2, UIA `ControlType`, NSAccessibility role, AT-SPI role, and Android/iOS trait vocabularies — so a producer can express any platform's notion of "what kind of widget is this" via one enum value, and adapters translate to local idiom.

Representative groups (full list at `common/src/lib.rs`):

- **Generic / unknown:** `Unknown` (default), `Generic`, `Group`, `None`, `Presentation`, `Pane`.
- **Text content:** `TextRun`, `Paragraph`, `Label`, `StaticText`, `LineBreak`, `WordBreak`.
- **Headings & landmarks:** `Heading`, `Banner`, `Complementary`, `ContentInfo`, `Main`, `Navigation`, `Region`, `Search`, `Form`.
- **Structure:** `Article`, `Section`, `List`, `ListItem`, `Definition`, `Term`, `Figure`, `Caption`, `Table`, `Row`, `Cell`, `ColumnHeader`, `RowHeader`, `Code`, `Math`.
- **Standalone widgets:** `Button`, `CheckBox`, `RadioButton`, `Switch`, `Link`, `TextInput`, `SearchBox`, `Slider`, `SpinButton`, `ProgressIndicator`, `ScrollBar`, `MenuItem`, `MenuItemCheckBox`, `MenuItemRadio`, `Tab`, `TabPanel`, `Option` (the listbox-option sense), `TreeItem`, `Toolbar`, `Tooltip`.
- **Composite widgets:** `ComboBox`, `Grid`, `GridCell`, `ListBox`, `Menu`, `MenuBar`, `RadioGroup`, `TabList`, `Tree`, `TreeGrid`.
- **Windows / overlays:** `Window`, `Dialog`, `AlertDialog`, `Alert`.
- **Live regions:** `Log`, `Status`, `Timer`. (Politeness level is separate — see [Live regions](#live-regions) below.)
- **Graphics / images:** `Image`, `Graphic`, `Canvas`, `SvgRoot`.
- **Digital publishing / DPub ARIA:** `Footnote`, `DocBackLink`, `DocPageBreak`, plus the full DPub set.

The enum has no namespacing; producers pick the variant closest to the widget's semantic and trust adapters to do the platform mapping. Buiy's ARIA-1.2 roster ([accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) §3.11) maps cleanly onto the `Role` enum with no gaps for the foundation tier.

## Actions

`Action` is the closed set of operations an AT can request on a node. The adapter delivers these to the producer's `ActionHandler` as `ActionRequest { action, target: NodeId, data: Option<ActionData> }`. From `common/src/lib.rs` (0.24):

```rust
pub enum Action {
    Click,
    Focus,
    Blur,
    Collapse,
    Expand,
    CustomAction,
    Decrement,
    Increment,
    HideTooltip,
    ShowTooltip,
    ReplaceSelectedText,
    ScrollDown,
    ScrollLeft,
    ScrollRight,
    ScrollUp,
    ScrollIntoView,
    ScrollToPoint,
    SetScrollOffset,
    SetTextSelection,
    SetSequentialFocusNavigationStartingPoint,
    SetValue,
    ShowContextMenu,
}
```

For a node to be a candidate for an action, the producer must mark it via `Node::add_action(Action::...)`. The adapter then advertises the node as supporting the corresponding platform-native operation (UIA `Invoke` for `Click`, NSAccessibility `accessibilityPerformAction:` for the matching `kAX...Action`, etc.). The `Action::CustomAction` slot is for producer-defined actions surfaced through AT-SPI / NSAccessibility custom-action lists; `ActionData::CustomActionIndex` identifies which custom action when the request arrives.

`SetValue` carries `ActionData::Value(String)` or `ActionData::NumericValue(f64)`. `SetTextSelection` carries `ActionData::SetTextSelection(TextSelection)`. `ScrollToPoint` carries `ActionData::ScrollToPoint(Point)`. Most others carry no data.

## State flags

States are accessor methods on `Node` (the field-level surface is private; setters/getters do the work — see [api.md](api.md)). Bool-valued states from the 0.24 line:

- **Visibility / interaction:** `is_hidden`, `is_disabled`, `is_busy`, `is_read_only`, `is_touch_transparent`, `is_modal`.
- **Selection / expansion (tri-state via `Option<bool>`):** `is_selected -> Option<bool>`, `is_expanded -> Option<bool>`. `None` means "not applicable"; `Some(true)`/`Some(false)` carry the state.
- **Form state:** `is_required`, `is_multiselectable`.
- **Live-region behavior:** `is_live_atomic` (mirror of `aria-atomic`).
- **Text styling at the run level:** `is_italic` (used on `Role::TextRun` nodes inside paragraphs).
- **Editor annotations:** `is_spelling_error`, `is_grammar_error`, `is_search_match`, `is_suggestion`.
- **Visited link:** `is_visited`.
- **Layout hints:** `clips_children`, `is_line_breaking_object`, `is_page_breaking_object`.

Toggled buttons / checkboxes use the `Toggled` enum (`False`, `True`, `Mixed`) on a dedicated field, not a bool — this matches ARIA's tri-valued `aria-pressed` and `aria-checked` and was a deliberate refactor away from earlier bool-based modeling.

`Invalid` is a separate enum (`False`, `True`, `Grammar`, `Spelling`), matching ARIA's `aria-invalid`.

`is_focused` is **not** a node state. Focus is carried on `TreeUpdate.focus: NodeId` — one focused node per tree, set by the producer on every update. Adapters translate this into per-platform focus events.

## Relations

Relations are `NodeId`-list or `Option<NodeId>` fields on `Node`. The ARIA 1.2 relationship attributes map directly:

- **Multi-target lists:** `children`, `controls`, `details`, `described_by`, `flow_to`, `labelled_by` (note the double-l British spelling — not `labeled_by`), `owns`, `radio_group`.
- **Single targets:** `active_descendant`, `error_message`, `in_page_link_target`, `member_of`, `popup_for`, `next_on_line`, `previous_on_line`.

The `children` relation is the structural tree backbone; everything else is auxiliary. `owns` overrides `children` for AT traversal — used when DOM structure and accessibility structure must diverge (rarely needed in retained-mode toolkits, common in DOM/ARIA contexts). `controls` and `flow_to` express "this widget governs / hands off to that one" for AT navigation hints.

For Buiy's foundation-tier ARIA-relationship roster ([accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) §3.11), every listed `aria-*` attribute corresponds to either a relation field or a state on `Node` — no gaps. (The Buiy spec calls out `aria-grabbed` / `aria-dropeffect` as deprecated and not implemented; AccessKit doesn't expose them either.)

## Live regions

Live-region politeness is the `Live` enum on `Node`:

```rust
pub enum Live { Off, Polite, Assertive }
```

`Live::Off` is the default. `Polite` and `Assertive` map to AT-SPI `LIVE_POLITE` / `LIVE_ASSERTIVE`, ARIA `aria-live=polite` / `aria-live=assertive`, UIA `LiveSetting.Polite` / `LiveSetting.Assertive`, NSAccessibility's announcement-priority APIs, and Android `ACCESSIBILITY_LIVE_REGION_POLITE` / `..._ASSERTIVE`. The `is_live_atomic` bool toggles `aria-atomic` semantics.

`aria-relevant` is *not* directly represented in AccessKit's node model — the live-region change semantics in AccessKit are "the producer pushes a new node value; the adapter announces if politeness allows." Buiy needs to layer its `aria-relevant=additions|removals|text|all` logic on its own side, computing which changes warrant a TreeUpdate-and-announcement vs. silent updates.

## Coordinate spaces

`Node::set_bounds(Rect)` takes a rectangle in **window-relative logical coordinates** (the producer's coordinate space inside the window, in the producer's logical-pixel units). The platform adapter is responsible for translating these to OS-screen coordinates by composing in the window's position and DPI scale at query time.

This means producers should not bake screen position into the tree at push time — that would invalidate the entire tree on every window move. Buiy's per-frame a11y update can read computed layout rects in window-local logical coords directly.

`Affine`, `Point`, `Rect`, `Size`, `Vec2` are the geometry primitives in `accesskit::*` (the same shapes as `kurbo`'s, repurposed for the AccessKit schema).

## The root and tree hierarchy

There is no `is_root` flag on `Node`. The root is identified by `Tree.root: NodeId` in the `Tree` metadata struct. Every other node is reachable from the root via the transitive closure of `children` relations. A `TreeUpdate` is allowed to contain nodes that are not yet in the tree (the producer is mid-build); the consolidated tree after applying the update must be consistent (root reachable, no orphans, no cycles).

`Tree { root: NodeId, toolkit_name: Option<String>, toolkit_version: Option<String> }`. The toolkit name/version is exposed to ATs as a diagnostic — Buiy should set `toolkit_name: Some("Buiy")` and `toolkit_version: Some(env!("CARGO_PKG_VERSION"))` on the initial `TreeUpdate`.

## Text content

Long text runs are represented as a parent node (e.g. `Role::Paragraph` or `Role::TextInput`) with `Role::TextRun` child nodes for each style-uniform span. The parent carries `value`, the runs carry per-run styling (`is_italic`, font, decoration). `TextDecoration` and `TextDecorationStyle` enums cover underline/strikethrough/etc. `TextAlign` and `TextDirection` cover paragraph-level layout.

`TextPosition { node: NodeId, character_index: usize }` identifies a cursor position. `TextSelection { anchor: TextPosition, focus: TextPosition }` carries an editor selection. The `Action::SetTextSelection` request lets ATs drive caret placement programmatically.

The AccessKit README notes a current limitation: "single-line and multi-line text input controls" are supported but "rich text or hypertext" — text with embedded links / inline images that need their own a11y treatment — is not fully modeled. Buiy's rich-text and IME work in `buiy_text` ([text.md](../../specs/2026-05-07-buiy-foundation/text.md)) will need to navigate this gap.

## ACCNAME 1.2 mapping

AccessKit does **not compute accessible names**. The producer pre-computes the name via the ACCNAME 1.2 algorithm and pushes the result on `Node::set_label(...)` (and `Node::set_description(...)` for the description). Buiy's spec confirms this: "Full algorithm implemented in `buiy_core`" — ACCNAME is a `buiy_core` responsibility, not an AccessKit responsibility.

The recently-released `accesskit_consumer` 0.36.0 (2026-05-11) added `LocalNodeId` and `TreeId` lookups on the adapter side, but these are about adapter-side tree introspection, not name computation.

For Buiy specifically:

- Buiy walks the ACCNAME chain (`aria-labelledby` > `aria-label` > host-language label > content > `title`) per WAI-ARIA 1.2.
- Buiy emits the final string into `Node::set_label(...)`.
- For the `aria-labelledby` chain itself, Buiy also publishes the source NodeIds via `Node::set_labelled_by(...)` — ATs that want the structured source can read the relation; ATs that want the flat name read `label`. Both are emitted; redundancy is intentional.

## Sources

- `common/src/lib.rs` at the 0.24 line (Role, Action, Node accessors, Tree, TreeUpdate, NodeId, Live, Toggled, Invalid): https://github.com/AccessKit/accesskit/tree/main/common/src
- `accesskit` 0.24.0 docs.rs top-level item list (structs + enums + traits): https://docs.rs/accesskit/0.24.0/accesskit/
- `accesskit_consumer` 0.36.0 release notes (LocalNodeId / TreeId lookup; iOS adapter support): https://github.com/AccessKit/accesskit/releases
- AccessKit README "single-line and multi-line text input controls … but not rich text or hypertext" limitation: https://github.com/AccessKit/accesskit/blob/main/README.md
- Buiy ARIA-1.2 taxonomy: `docs/specs/2026-05-07-buiy-foundation/accessibility.md`
- Sibling: [architecture.md](architecture.md), [platform-adapters.md](platform-adapters.md), [api.md](api.md)
