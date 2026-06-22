# Semantic Tree — Decomposed Component Model & Real Nesting

**Date:** 2026-06-18
**Status:** draft

The **decomposed semantic-tree substrate**: per-concept state/relation components, their one-to-one accesskit 0.24 setters, the flat derive fold emitting an `accesskit::Node`, accessible-name computation, real ECS-tree nesting with relation overlay/hidden prune, and the merged/unmerged projection. [action-router.md](action-router.md) consumes this state for the live filter, [widget-contracts.md](widget-contracts.md) authors the components, [inprocess-api.md](inprocess-api.md) reads/acts over this tree.

Prior-art: [../../prior-art/bevy-a11y/component-model-incident.md](../../prior-art/bevy-a11y/component-model-incident.md) (the megacomponent anti-pattern #17644 we *invert*), [../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md) (NodeId/TreeUpdate/single-root + the one-canonical-retained-tree model), [../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md) (the bool-property model), [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md) (decomposed-component lessons), [../../prior-art/wai-aria-apg/roles-states-properties.md](../../prior-art/wai-aria-apg/roles-states-properties.md), [../../prior-art/wai-aria-apg/name-computation.md](../../prior-art/wai-aria-apg/name-computation.md), [../../prior-art/wai-aria-apg/live-regions.md](../../prior-art/wai-aria-apg/live-regions.md). See [README.md](README.md) for the index and the transport research-debt note.

> **Version base.** All setters/Actions target **accesskit 0.24 / Bevy 0.19-rc.3**, the current base (the BSN/0.19 bump landed — PR #70, main @ `3b3b0ba`). **Verify each 0.24 signature against the resolved deps (`cargo tree`/`cargo doc`), not docs.rs.**

---

## 1. One tiny component per independently-changing concept

The direct inversion of the [bevy_a11y megacomponent](../../prior-art/bevy-a11y/component-model-incident.md): **every ARIA concept that changes independently is its own small, public-fielded, `Reflect`-derived component.** Flipping `checked` never dirties `expanded`; each is independently `Default`-able, change-detected, and BSN-patchable.

Two surgical exceptions:

- **`A11yValue`** — the five numeric slider fields (`now/min/max/step/jump`) + `text` are one ARIA concept (a valued range); they co-vary.
- **`A11yRelations`** — the eight cross-reference fields, grouped for an **honest reason: translation locality, not co-variance.** All are `Entity`-ref vectors resolved to `NodeId` in the same translate pass. The weaker case (`controls`/`owns`/`flow_to` don't co-vary with `labelled_by`/`described_by`) is acknowledged — not claimed as co-variance. Acceptable because refs-only, translates together as `NodeId` vectors, stays BSN-patchable per-field via `Reflect`. Not a #17644-scale violation.

Net count ~22.

All components: `#[derive(Component, Reflect, FromReflect, Default, Clone, Debug, PartialEq)]` + `#[reflect(Component)]`, `register_type`'d in `A11yPlugin::build`.

File layout in `crates/buiy_core/src/a11y/`: `mod.rs` (existing `A11yRole`/`A11yLabel`/`A11yDescription`, widened `A11yNodeView` — today 5 flat fields `entity`/`role`/`name`/`description`/`focusable` at mod.rs:57-62, `A11yTreeBuilder`, `A11yPlugin`, widened `build_tree`); `states.rs` (NEW); `relations.rs` (NEW); `translate.rs` (`node_id_for` + NEW `entity_for_node_id`, `to_accesskit_node` derive fold, `build_tree_update`, `role_to_accesskit`); `accname.rs` (NEW); `contract.rs`/`action.rs`/`inprocess.rs` (NEW — see siblings); `adapter.rs` (existing outbound push, unchanged).

---

## 2. State components (`states.rs`) → accesskit 0.24 setters

Each maps to exactly one 0.24 setter. **Absence = not-applicable** (`clear_*` implied) — accesskit's bool-property model (`bool_property_methods!`), so `pub bool` is correct and `Option<bool>` is rejected ([../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md)).

| Component | Setter |
|---|---|
| `A11yToggled(pub Toggled)` `{False,True,Mixed}` | `set_toggled` |
| `A11yExpanded(pub bool)` | `set_expanded`; absence ⇒ `clear_expanded` |
| `A11ySelected(pub bool)` | `set_selected`; absence ⇒ `clear_selected` |
| `A11yDisabled` (marker) | `set_disabled()` |
| `A11yReadOnly`/`A11yRequired`/`A11yBusy`/`A11yModal` (markers) | `set_read_only`/`set_required`/`set_busy`/`set_modal` |
| `A11yHidden` (marker) | NOT a node flag — prunes entity+subtree (§7.4) |
| `A11yInvalid(pub Invalid)` `{False,True,Grammar,Spelling}` | `set_invalid` |
| `A11yValue { now,min,max: f64, step,jump: Option<f64>, text: Option<String> }` | `set_numeric_value`·`set_min_numeric_value`·`set_max_numeric_value`·`set_numeric_value_step`·`set_numeric_value_jump`; `text`⇒`set_value` |
| `A11yTextValue(pub String)` | `set_value` (from `TextEditState`; role disambiguates) |
| `A11yPlaceholder(pub String)` | `set_placeholder` |
| `A11yOrientation`/`A11yHasPopup`/`A11yAutoComplete`/`A11yLevel(u32)` | `set_orientation`/`set_has_popup`/`set_auto_complete`/`set_level` |
| `A11yLive { politeness: Live, atomic: bool }` | `set_live` · **`set_live_atomic`** |
| `A11yPosInSet(u32)`/`A11ySetSize(u32)` | `set_position_in_set`/`set_size_of_set` |

`Toggled` is tri-state, unifying aria-checked and aria-pressed through `set_toggled`; `Mixed` is never collapsed.

**`set_live_atomic`, not `set_atomic`** (must-fix): `set_atomic` does not exist in 0.24 (accesskit-0.24.1 lib.rs:1806: `(LiveAtomic, is_live_atomic, set_live_atomic, clear_live_atomic)`).

No `A11yMultiline` — multiline is a **role split** (§4).

---

## 3. Relation component (`relations.rs`)

```rust
#[derive(Component, Reflect, FromReflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct A11yRelations {
    pub labelled_by: Vec<Entity>,  // British double-l, mandatory
    pub described_by: Vec<Entity>,
    pub controls: Vec<Entity>,
    pub owns: Vec<Entity>,
    pub flow_to: Vec<Entity>,
    pub details: Vec<Entity>,
    pub active_descendant: Option<Entity>,
    pub error_message: Option<Entity>,
}
```

Setters: `labelled_by`→`set_labelled_by`, `described_by`→`set_described_by`, `controls`→`set_controls`, `owns`→`set_owns`, `flow_to`→`set_flow_to`, `details`→`set_details`, `active_descendant`→`set_active_descendant`, `error_message`→`set_error_message`.

Storage is `Entity`; resolution to `NodeId` happens at translate time via `node_id_for`, so the view stays winit-free and `Entity` never leaks past the seam. Relations publish **alongside** the flat `set_label` ACCNAME string (redundant by design). Grouping rationale restated honestly: **translation locality, not co-variance** (§1).

---

## 4. `A11yRole` additions — multiline is a role split

`A11yRole` is `#[non_exhaustive]`. Additive: `Checkbox`→`Role::CheckBox`, `Switch`→`Role::Switch`, `Slider`→`Role::Slider`, `TextInput`→`Role::TextInput`, `MultilineTextInput`→`Role::MultilineTextInput`, `Region`→`Role::Region`, `Group`→`Role::Group`.

`TextInput` vs `MultilineTextInput` is a **ROLE SPLIT, not a flag** — 0.24 has no `set_multiline`. Retires the `A11yRole::Text` stopgap (`buiy_widgets/src/text_input.rs:83-88`).

**Two stringifiers update in the SAME change.** Both `translate::role_to_accesskit` AND `buiy_verify::a11y::role_to_str`. `role_to_str` has a `_ => "Unknown"` wildcard that would silently mask a forgotten arm — a half-update shows `"Unknown"` in snapshots. Hard convention, enforced by gate #3 going red on `"Unknown"`.

---

## 5. The derive fold: `to_accesskit_node`

`to_accesskit_node(view: &A11yNodeView) -> accesskit::Node` (translate.rs; today the `if view.focusable { node.add_action(Action::Focus) }` shape) becomes a **flat ordered fold** — one arm per component:

```rust
let mut node = Node::new(role_to_accesskit(view.role));
node.set_label(view.name.clone());                 // ACCNAME (§6)
if let Some(d) = &view.description { node.set_description(d.clone()); }
if let Some(t) = view.toggled  { node.set_toggled(t); }
if let Some(b) = view.expanded { node.set_expanded(b); }
if let Some(b) = view.selected { node.set_selected(b); }
if view.disabled { node.set_disabled(); }
if view.read_only { node.set_read_only(); }
// … required / busy / modal …
if let Some(i) = view.invalid { node.set_invalid(i); }
if let Some(v) = &view.value {
    node.set_numeric_value(v.now);
    node.set_min_numeric_value(v.min);
    node.set_max_numeric_value(v.max);
    if let Some(s) = v.step { node.set_numeric_value_step(s); }
    if let Some(j) = v.jump { node.set_numeric_value_jump(j); }
    if let Some(t) = &v.text { node.set_value(t.clone()); }
}
if let Some(s) = &view.text_value { node.set_value(s.clone()); }
// … placeholder / orientation / has_popup / auto_complete / level …
let (politeness, atomic) = resolve_live(view.role, view.live);  // role-implied first, then override
if let Some(p) = politeness { node.set_live(p); node.set_live_atomic(atomic); }
if let Some(n) = view.pos_in_set { node.set_position_in_set(n); }
if let Some(n) = view.set_size  { node.set_size_of_set(n); }
if !view.labelled_by.is_empty() { node.set_labelled_by(view.labelled_by.clone()); }
// … described_by / controls / owns / flow_to / details …
if let Some(id) = view.active_descendant { node.set_active_descendant(id); }
if let Some(id) = view.error_message     { node.set_error_message(id); }
// Actions from the A11yContract (widget-contracts.md) + Focus/Blur.
```

The single emission point against 0.24 setters. A new ARIA concept = one tiny component + one setter line + one `role_to_str` arm.

### Role-implied live regions (must-fix)

`resolve_live(role, explicit)` derives politeness/atomic from the role when no explicit `A11yLive`, then the explicit component overrides ([../../prior-art/wai-aria-apg/live-regions.md](../../prior-art/wai-aria-apg/live-regions.md)): `Role::Alert`⇒Assertive+atomic, `Role::Status`⇒Polite+atomic, `Role::Log`⇒Polite. Without this, gate #4 is wrong for an alert dialog carrying `A11yRole::Alert` but no author `A11yLive`.

---

## 6. Accessible name — a function, not a component

ACCNAME 1.2 (`labelledby > label > host > content > title`, hidden-subtree exclusion) is a pure `compute_accessible_name(...) -> String` in `accname.rs`, feeding `A11yLabel`'s `set_label` string. Derived every build, never stored. See [../../prior-art/wai-aria-apg/name-computation.md](../../prior-art/wai-aria-apg/name-computation.md).

---

## 7. Real nesting: `build_tree` over the ECS hierarchy

Replaces the synthetic flat single-root (`build_tree_update` today: every node `push_child`'d under one `Role::Window` `ROOT_NODE_ID = NodeId(0)`) with **real parent→children edges + a relation overlay** — the one-canonical-retained-tree model ([../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md)) over Bevy's `ChildOf`/`Children`.

`build_tree` (mod.rs) widens its query tuple to read every `Option<&A11yX>` + `Option<&ChildOf>`/`Option<&Children>`. `A11yNodeView` widens from 5 flat fields (mod.rs:57-62) to a winit-free snapshot: `entity`/`role`/`name`/`description`/`focusable`, all state `Option`s, relations resolved to `Vec<NodeId>`/`Option<NodeId>` (via `node_id_for` at build time — `Entity` never leaks), `parent: Option<Entity>` + ordered `children: Vec<Entity>`.

**7.1 Default nesting — `nearest_a11y_ancestor`.** Default accesskit children = nearest a11y-bearing descendants; a transitive walk skips pure-layout entities so presentational wrappers collapse, no holes.

**7.2 Root.** `node_id_for(window_entity)` when present (retiring hardcoded `ROOT_NODE_ID`-as-constant); the synthetic `Role::Window` root remains only as parent of top-level parentless nodes. **Single-window-scoped** — `ROOT_NODE_ID` has no window discriminator; multi-window per-`WindowId` keying is a named Phase-2 follow-up ([phasing.md](phasing.md)).

**7.3 `owns` re-parent — resolved last, cycle-guarded.** (1) `ChildOf`/`Children` builds child lists. (2) `owns` applied LAST: each owned entity becomes a child of the owner and is removed from its layout-parent's list. (3) Cycle/duplicate-parent **logged once and the edge dropped** — never cyclic, never silent.

**7.4 Hidden/inert prune.** `A11yHidden` + inert entities and descendants emit no node, excluded from parents' lists. Joint focus + AccessKit + picking concern.

**7.5 `build_tree_update`** rewritten so each node `push_child`s resolved a11y children (via `node_id_for`) in document order; relations emit as setters, distinct from structural edges.

**7.6 Falls out free** ([../../prior-art/wai-aria-apg/patterns-catalog.md](../../prior-art/wai-aria-apg/patterns-catalog.md)): accordion (header Button + panel Region subtree), dialog (`labelled_by`/`described_by` + `owns`), multi-thumb slider (one Slider node per thumb under a Group). See [widget-contracts.md](widget-contracts.md).

---

## 8. Merged vs. unmerged — read-time projection over ONE tree

ONE canonical unmerged tree (§7). Merged is a read-time projection, not a second stored tree (rejected in [README.md](README.md#rejected-alternatives)). An `A11yMergeChildren` marker lets the serializer optionally collapse descendants. Snapshot **defaults to `TreeView::Unmerged`** and accepts `TreeView::Merged`; the active view is explicit in the API ([inprocess-api.md](inprocess-api.md)).

---

## 9. Gate #12 by construction

Real-hierarchy build makes [verification.md](verification.md) gate #12 hold by construction: **no orphans** (every node is a `Children` edge, an `owns` child, or parented to the synthetic root); **focus reachable** (a pruned entity can't be focused); **accessible-name-present** (a focusable node without a computed name fails — proptest in `inprocess.rs`). The proptests land in `inprocess.rs`, owned by the a11y subsystem.

---

## Cross-references

- [action-router.md](action-router.md) — live capability filter; inverts `node_id_for` via `entity_for_node_id`.
- [widget-contracts.md](widget-contracts.md) — per-role `A11yContract` impls assembling these components.
- [inprocess-api.md](inprocess-api.md) — snapshot/act over `accesskit_consumer`, `TreeView`, gate #12 proptests.
- [README.md](README.md) — index, locked decisions, rejected alternatives.
- [verification.md](verification.md) — headless gates over this tree.
- [phasing.md](phasing.md) — Phase 0/1a/1b + multi-window follow-up.
