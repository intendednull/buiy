//! Pure translation from Buiy's frame-built `A11yNodeView` snapshot into the
//! AccessKit data model. Keeping this module winit-free means we can
//! unit-test it without provisioning a real window.

use crate::a11y::{A11yLive, A11yNodeView, A11yRole};
use accesskit::{Live, Node, NodeId, Role, Tree, TreeUpdate};
use bevy::prelude::Entity;

/// Stable AccessKit root node id. Every adapter pushes the same root so the
/// AT sees one tree per Buiy window. v0.x may key this off the window entity
/// when multi-window-aware ATs become a target.
pub const ROOT_NODE_ID: NodeId = NodeId(0);

/// Convert a Bevy [`Entity`] into a stable [`NodeId`]. Entity::to_bits is
/// deterministic within a session, which is sufficient for AT consumption
/// (the AT doesn't compare across sessions).
pub fn node_id_for(entity: Entity) -> NodeId {
    // Avoid 0 (reserved for ROOT_NODE_ID). +1 is safe because Bevy never
    // produces an `Entity` whose bits are `u64::MAX`.
    NodeId(entity.to_bits().saturating_add(1))
}

/// Inverse of [`node_id_for`]: recover the [`Entity`] an inbound [`NodeId`]
/// addresses. Total and panic-free — built to resolve ids that arrive from
/// outside (AccessKit action callbacks, agents), which may not be valid:
/// `NodeId(0)` / [`ROOT_NODE_ID`] (the synthetic root) and any id whose
/// `id.0 - 1` is not a valid [`Entity::to_bits`] encoding both map to `None`.
/// A well-formed id produced by [`node_id_for`] (`entity.to_bits() + 1`) round-trips.
pub fn entity_for_node_id(id: NodeId) -> Option<Entity> {
    if id == ROOT_NODE_ID {
        return None;
    }
    Entity::try_from_bits(id.0 - 1)
}

/// Translate one [`A11yNodeView`] into an [`accesskit::Node`].
///
/// Note: `Node::set_label` takes `impl Into<Box<str>>` in accesskit 0.21;
/// `String` and `&str` both satisfy that bound.
pub fn to_accesskit_node(view: &A11yNodeView) -> Node {
    let mut node = Node::new(role_to_accesskit(view.role));
    if !view.name.is_empty() {
        node.set_label(view.name.clone());
    }
    if !view.description.is_empty() {
        node.set_description(view.description.clone());
    }
    // Decomposed state fold (P1a, first batch). One ordered arm per component;
    // **absence ⇒ the setter is not called** (semantic-tree.md §§2,5). Every
    // setter signature below is verified against the resolved accesskit 0.24.1
    // (committed Cargo.lock): `set_toggled` takes the `Toggled` enum
    // (`unique_enum_property_methods!`), `set_expanded`/`set_selected` take
    // `bool` (`bool_property_methods!`), and `set_disabled`/`set_modal` take
    // **no argument** (`flag_methods!` markers). This is the single emission
    // point for these setters (standing rule §0.2).
    if let Some(t) = view.toggled {
        node.set_toggled(t);
    }
    if let Some(b) = view.expanded {
        node.set_expanded(b);
    }
    if let Some(b) = view.selected {
        node.set_selected(b);
    }
    if view.disabled {
        node.set_disabled();
    }
    if view.modal {
        node.set_modal();
    }
    // Decomposed state fold (P1a, second batch). Setter signatures verified
    // against the resolved accesskit 0.24.1 (committed Cargo.lock):
    // `set_numeric_value`/`…_min`/`…_max`/`…_step`/`…_jump` take `f64`
    // (`f64_property_methods!`); `set_value`/`set_placeholder` take
    // `impl Into<Box<str>>` (`string_property_methods!`);
    // `set_orientation`/`set_has_popup`/`set_live` take the accesskit enum
    // (`unique_enum_property_methods!`); and `set_live_atomic()` is a
    // **no-argument** marker (`flag_methods!`, lib.rs:1806 — **not** `set_atomic`,
    // which does not exist in 0.24). This is the single emission point (§0.2).
    if let Some(v) = &view.value {
        node.set_numeric_value(v.now);
        node.set_min_numeric_value(v.min);
        node.set_max_numeric_value(v.max);
        if let Some(s) = v.step {
            node.set_numeric_value_step(s);
        }
        if let Some(j) = v.jump {
            node.set_numeric_value_jump(j);
        }
        if let Some(t) = &v.text {
            node.set_value(t.clone());
        }
    }
    // `A11yTextValue` after `A11yValue.text` (spec §5 fold order): both call
    // `set_value`, so the last writer wins. A node carrying both is a role-split
    // contract error a widget never authors; the ordering pins deterministic
    // behavior regardless.
    if let Some(s) = &view.text_value {
        node.set_value(s.clone());
    }
    if let Some(p) = &view.placeholder {
        node.set_placeholder(p.clone());
    }
    if let Some(o) = view.orientation {
        node.set_orientation(o);
    }
    if let Some(h) = view.has_popup {
        node.set_has_popup(h);
    }
    // Live region: role-implied policy first (resolve_live), overridden by an
    // explicit `A11yLive`. `set_live_atomic()` takes NO argument — the bool gates
    // *whether* to call it.
    let (politeness, atomic) = resolve_live(view.role, view.live);
    if let Some(p) = politeness {
        node.set_live(p);
        if atomic {
            node.set_live_atomic();
        }
    }
    // Relation fold (P1a, the four WIRED `A11yRelations` fields). The view
    // carries them already resolved to `NodeId` (`build_tree` → `node_id_for`),
    // so no `Entity` reaches this seam. Setter signatures verified against the
    // resolved accesskit 0.24.1 (committed Cargo.lock):
    // `set_labelled_by`/`set_described_by`/`set_controls` take
    // `impl Into<Vec<NodeId>>` (`node_id_vec_property_methods!`, lib.rs:1880);
    // `set_active_descendant` takes a single `NodeId`
    // (`node_id_property_methods!`, lib.rs:1898). The four carried-but-unwired
    // relation fields (`owns`/`flow_to`/`details`/`error_message`) have NO arm
    // here — deliberately deferred (co-drive §3.2). **Empty ⇒ the setter is not
    // called.**
    if !view.labelled_by.is_empty() {
        node.set_labelled_by(view.labelled_by.clone());
    }
    if !view.described_by.is_empty() {
        node.set_described_by(view.described_by.clone());
    }
    if !view.controls.is_empty() {
        node.set_controls(view.controls.clone());
    }
    if let Some(id) = view.active_descendant {
        node.set_active_descendant(id);
    }
    // SC-4 scroll fold (P1a, the schema + single emission point; C5 populates
    // the source in Wave 4). The six scroll setters all take `f64`
    // (`f64_property_methods!`, lib.rs:1971): `offset` → `set_scroll_x`/
    // `set_scroll_y`; the min is always `0.0`; the max is the overflow
    // `content_extent − viewport_extent` clamped to ≥ 0 (a non-scrollable axis
    // reports a `0.0` range). `None` ⇒ not a scroll container ⇒ no setter fires.
    if let Some(s) = &view.scroll {
        node.set_scroll_x(s.offset.x as f64);
        node.set_scroll_x_min(0.0);
        node.set_scroll_x_max((s.content_extent.x - s.viewport_extent.x).max(0.0) as f64);
        node.set_scroll_y(s.offset.y as f64);
        node.set_scroll_y_min(0.0);
        node.set_scroll_y_max((s.content_extent.y - s.viewport_extent.y).max(0.0) as f64);
    }
    // NOTE: `A11yHidden` (`view.hidden`) has **no fold arm** in P1a. The final
    // design (semantic-tree.md §7.4) prunes hidden entities + subtrees from
    // `build_tree` rather than flagging the node; that prune needs the ECS-tree
    // nesting that lands in P1b. The flag is carried on the view so P1b only has
    // to add the prune.
    // Action advertisement (P1c-a, widget-contracts.md §§1,5). Two contributors,
    // ONE outbound source of truth (the lockstep keystone):
    //
    // 1. Every `Focusable` node implicitly advertises `{Focus, Blur}` — it is
    //    addressable (Focus) and clearable (Blur) regardless of role.
    // 2. The role's `A11yContract` (looked up via `contract_for`) adds the
    //    role-specific verbs from `actions()` (Button ⇒ `Click`). A role with
    //    no interactive contract contributes nothing here.
    //
    // The same `actions()` list is re-validated inbound by the router (P1c-b)
    // before `honor` is called — advertise and honor cannot drift. (The old
    // focusable-`Focus`-only hardcode is gone: Blur was missing, and the
    // role-specific verbs were absent.)
    if view.focusable {
        node.add_action(accesskit::Action::Focus);
        node.add_action(accesskit::Action::Blur);
    }
    if let Some(entry) = crate::a11y::contract_for(view.role) {
        for &action in entry.actions {
            node.add_action(action);
        }
    }
    // 3. A **state-keyed** capability layered on the role contract (widget-contracts.md
    //    §5 "Disclosure-trigger"): any node carrying `A11yExpanded` also advertises
    //    `{Expand, Collapse}` — *in addition to* its role's contract verbs. A
    //    Disclosure-trigger is `Role::Button` (so the Button contract still supplies
    //    `Click`), but it is *expandable*, which is modelled as this reusable
    //    state-driven capability rather than a new role. The inbound router honors
    //    these two verbs generically for any `A11yExpanded` entity (action.rs), so
    //    advertise and honor stay in lockstep. Keyed on `view.expanded.is_some()`
    //    (the projection of the `A11yExpanded` component's presence).
    if view.expanded.is_some() {
        node.add_action(accesskit::Action::Expand);
        node.add_action(accesskit::Action::Collapse);
    }
    node
}

fn role_to_accesskit(role: A11yRole) -> Role {
    match role {
        A11yRole::Generic => Role::GenericContainer,
        A11yRole::Button => Role::Button,
        A11yRole::Link => Role::Link,
        A11yRole::Image => Role::Image,
        A11yRole::Text => Role::Label,
        A11yRole::Heading => Role::Heading,
        A11yRole::Dialog => Role::Dialog,
        A11yRole::AlertDialog => Role::AlertDialog,
        A11yRole::Tooltip => Role::Tooltip,
        A11yRole::Checkbox => Role::CheckBox,
        A11yRole::Switch => Role::Switch,
        A11yRole::Slider => Role::Slider,
        A11yRole::TextInput => Role::TextInput,
        A11yRole::MultilineTextInput => Role::MultilineTextInput,
        A11yRole::Region => Role::Region,
        A11yRole::Group => Role::Group,
        A11yRole::Status => Role::Status,
        A11yRole::Alert => Role::Alert,
        A11yRole::Log => Role::Log,
    }
}

/// Resolve a node's live-region policy (semantic-tree.md §5).
///
/// An *explicit* [`A11yLive`] always wins. With none, the policy is **implied by
/// the role**: `Alert` ⇒ Assertive + atomic, `Status` ⇒ Polite + atomic, `Log` ⇒
/// Polite (non-atomic). Any other role implies no live region (`(None, false)`),
/// so the fold emits no `set_live`. Returning `(None, _)` means "no live region";
/// the bool is the atomic flag (only meaningful when politeness is `Some`).
///
/// This is the **single** place the role→policy mapping lives, so an alert that
/// carries `A11yRole::Alert` but no author `A11yLive` still announces correctly
/// (the gate-#4 must-fix; prior-art/wai-aria-apg/live-regions.md).
pub fn resolve_live(role: A11yRole, explicit: Option<A11yLive>) -> (Option<Live>, bool) {
    if let Some(l) = explicit {
        return (Some(l.politeness), l.atomic);
    }
    match role {
        A11yRole::Alert => (Some(Live::Assertive), true),
        A11yRole::Status => (Some(Live::Polite), true),
        A11yRole::Log => (Some(Live::Polite), false),
        _ => (None, false),
    }
}

/// Build a full [`TreeUpdate`] with **real parent→child nesting** over the
/// `A11yNodeView` list (semantic-tree.md §7).
///
/// Each view carries its resolved a11y `parent`/`children` (filled by
/// `build_tree`'s `nearest_a11y_ancestor` collapse). This fn lays those edges
/// out as accesskit `push_child` calls in document order and parents every
/// **top-level** node (`view.parent == None`) under the single synthetic root.
///
/// # Root keying (§7.2)
///
/// `root_entity` keys the synthetic `Role::Window` root off the **window entity**
/// when one exists (the live adapter passes `Some(window)`); headless callers
/// (`MinimalPlugins` tests, the in-process consumer) pass `None`, falling back to
/// the stable [`ROOT_NODE_ID`]. Either way exactly one `Role::Window` node parents
/// the top-level widgets, so the AT sees one tree per Buiy window. Multi-window
/// per-`WindowId` keying is a named Phase-2 follow-up (per-window root ids).
///
/// `entity_for_node_id` maps the window-entity root id back to the window entity;
/// it maps [`ROOT_NODE_ID`] to `None`. Both are non-widget ids the action router
/// already rejects.
pub fn build_tree_update(
    views: &[A11yNodeView],
    focused: Option<NodeId>,
    root_entity: Option<Entity>,
) -> TreeUpdate {
    // Root id: the window entity's id when present (§7.2), else the synthetic
    // constant for the headless / no-window path.
    let root_id = root_entity.map(node_id_for).unwrap_or(ROOT_NODE_ID);

    let mut nodes = Vec::with_capacity(views.len() + 1);

    // One accesskit node per view, with its resolved a11y children pushed in
    // document order (the `parent`/`children` edges `build_tree` computed via the
    // `nearest_a11y_ancestor` wrapper collapse). Top-level nodes are collected to
    // parent under the synthetic root below.
    let mut top_level = Vec::new();
    for view in views {
        let id = node_id_for(view.entity);
        let mut node = to_accesskit_node(view);
        for &child in &view.children {
            node.push_child(node_id_for(child));
        }
        nodes.push((id, node));
        if view.parent.is_none() {
            top_level.push(id);
        }
    }

    // The single synthetic root parents every top-level (parentless) node.
    let mut root = Node::new(Role::Window);
    for id in &top_level {
        root.push_child(*id);
    }
    nodes.insert(0, (root_id, root));

    TreeUpdate {
        nodes,
        tree: Some(Tree::new(root_id)),
        // Required since accesskit 0.23 (multi-tree). `TreeId::ROOT` (the nil
        // UUID) is the single root tree — exactly Buiy's one-tree-per-window
        // model; subtrees (`buiy-accessibility-design`) would key off this.
        tree_id: accesskit::TreeId::ROOT,
        focus: focused.unwrap_or(root_id),
    }
}
