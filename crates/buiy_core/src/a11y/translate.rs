//! Pure translation from Buiy's frame-built `A11yNodeView` snapshot into the
//! AccessKit data model. Keeping this module winit-free means we can
//! unit-test it without provisioning a real window.

use crate::a11y::{A11yNodeView, A11yRole};
use accesskit::{Node, NodeId, Role, Tree, TreeUpdate};
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
    // Phase 0 closeout: focusable widgets get the AccessKit "focusable"
    // semantic. Full keyboard-action contract is widget-specific
    // (`buiy-widget-catalog-design`).
    if view.focusable {
        node.add_action(accesskit::Action::Focus);
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
    }
}

/// Build a full [`TreeUpdate`] containing a synthetic root plus one node
/// per [`A11yNodeView`]. Children are listed under the root in iteration
/// order; nesting is a v0.x topic (`buiy-accessibility-design`).
pub fn build_tree_update(views: &[A11yNodeView], focused: Option<NodeId>) -> TreeUpdate {
    let mut nodes = Vec::with_capacity(views.len() + 1);

    // Children first — we still need to materialize their NodeIds before
    // we can list them under the root.
    let mut child_ids = Vec::with_capacity(views.len());
    for view in views {
        let id = node_id_for(view.entity);
        child_ids.push(id);
        nodes.push((id, to_accesskit_node(view)));
    }

    // Root.
    let mut root = Node::new(Role::Window);
    for cid in &child_ids {
        root.push_child(*cid);
    }
    nodes.insert(0, (ROOT_NODE_ID, root));

    TreeUpdate {
        nodes,
        tree: Some(Tree::new(ROOT_NODE_ID)),
        // Required since accesskit 0.23 (multi-tree). `TreeId::ROOT` (the nil
        // UUID) is the single root tree — exactly Buiy's one-tree-per-window
        // model; subtrees (`buiy-accessibility-design`) would key off this.
        tree_id: accesskit::TreeId::ROOT,
        focus: focused.unwrap_or(ROOT_NODE_ID),
    }
}
