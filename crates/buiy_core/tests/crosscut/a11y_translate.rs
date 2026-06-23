//! Unit tests for the pure `A11yNodeView` → AccessKit translation. No winit
//! window is required — this is the test-friendly seam between Buiy's a11y
//! tree and the OS-level adapter.

use bevy::prelude::*;
use buiy_core::a11y::{
    A11yNodeView, A11yRole,
    translate::{build_tree_update, to_accesskit_node},
};

#[test]
fn role_maps_to_accesskit_role() {
    let view = A11yNodeView {
        entity: Entity::PLACEHOLDER,
        role: A11yRole::Button,
        name: "Save".into(),
        description: String::new(),
        focusable: true,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_eq!(node.role(), accesskit::Role::Button);
    assert_eq!(node.label(), Some("Save"));
}

#[test]
fn build_tree_update_emits_root_plus_children() {
    let views = vec![
        A11yNodeView {
            entity: Entity::from_raw_u32(1).unwrap(),
            role: A11yRole::Button,
            name: "Save".into(),
            description: "Saves the document".into(),
            focusable: true,
            ..Default::default()
        },
        A11yNodeView {
            entity: Entity::from_raw_u32(2).unwrap(),
            role: A11yRole::Generic,
            name: "Container".into(),
            description: String::new(),
            focusable: false,
            ..Default::default()
        },
    ];
    let update = build_tree_update(&views, None);
    // Root + 2 child nodes:
    assert_eq!(update.nodes.len(), 3);
    // Tree pointer is set with a stable root id:
    assert!(update.tree.is_some());
}

#[test]
fn focused_node_id_round_trips() {
    let views = vec![A11yNodeView {
        entity: Entity::from_raw_u32(42).unwrap(),
        role: A11yRole::Button,
        name: "Focus me".into(),
        description: String::new(),
        focusable: true,
        ..Default::default()
    }];
    // Caller passes the entity-derived NodeId for the focused node.
    let focused_id = buiy_core::a11y::translate::node_id_for(Entity::from_raw_u32(42).unwrap());
    let update = build_tree_update(&views, Some(focused_id));
    assert_eq!(update.focus, focused_id);
}

#[test]
fn description_round_trips() {
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(1).unwrap(),
        role: A11yRole::Button,
        name: "Save".into(),
        description: "Saves the document".into(),
        focusable: true,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_eq!(node.description(), Some("Saves the document"));
}

#[test]
fn focusable_view_has_focus_action() {
    let focusable = A11yNodeView {
        entity: Entity::from_raw_u32(1).unwrap(),
        role: A11yRole::Button,
        name: "Click".into(),
        description: String::new(),
        focusable: true,
        ..Default::default()
    };
    let node = to_accesskit_node(&focusable);
    assert!(node.supports_action(accesskit::Action::Focus));

    let non_focusable = A11yNodeView {
        entity: Entity::from_raw_u32(2).unwrap(),
        role: A11yRole::Generic,
        name: "Container".into(),
        description: String::new(),
        focusable: false,
        ..Default::default()
    };
    let node = to_accesskit_node(&non_focusable);
    assert!(!node.supports_action(accesskit::Action::Focus));
}

// ---------------------------------------------------------------------------
// P1a first-batch decomposed-state — PRODUCER-tier fixtures.
//
// `A11yExpanded` is asserted here (not in the consumer suite) because
// accesskit_consumer 0.36 exposes no public `is_expanded()` getter, while the
// producer `accesskit::Node` does (semantic-tree.md §0 gap). `A11yHidden` is
// carried-only in P1a (the §7.4 prune is P1b), so it must NOT flag the node — a
// producer-tier assertion pins that contract.
// ---------------------------------------------------------------------------

#[test]
fn expanded_view_sets_expanded() {
    // accesskit 0.24's `bool_property_methods!` getter returns `Option<bool>`:
    // explicit true/false round-trip as `Some(_)`, and absence (no fold arm) is
    // `None` — so the three states are distinguishable at the producer tier.
    let expanded = A11yNodeView {
        role: A11yRole::Button,
        expanded: Some(true),
        ..Default::default()
    };
    assert_eq!(to_accesskit_node(&expanded).is_expanded(), Some(true));

    let collapsed = A11yNodeView {
        role: A11yRole::Button,
        expanded: Some(false),
        ..Default::default()
    };
    assert_eq!(to_accesskit_node(&collapsed).is_expanded(), Some(false));

    // Absence ⇒ the arm is omitted ⇒ the property is unset.
    let unset = A11yNodeView {
        role: A11yRole::Button,
        expanded: None,
        ..Default::default()
    };
    assert_eq!(to_accesskit_node(&unset).is_expanded(), None);
}

#[test]
fn hidden_is_carried_but_not_flagged_in_p1a() {
    // `A11yHidden` carries the flag on the view for P1b's prune, but P1a emits
    // NO fold arm — the node must not be marked hidden. (P1b replaces the
    // carried flag with the §7.4 entity+subtree prune.)
    let view = A11yNodeView {
        role: A11yRole::Button,
        hidden: true,
        ..Default::default()
    };
    assert!(
        !to_accesskit_node(&view).is_hidden(),
        "A11yHidden must not flag the node in P1a (the prune is P1b)"
    );
}

// ---------------------------------------------------------------------------
// P1a second-batch — PRODUCER-tier fixtures.
//
// `A11yPlaceholder` is asserted here because the consumer `placeholder()` getter
// is FILTERED (it only surfaces on a node that is_text_input() && is empty), so
// the producer `Node::placeholder()` is the clean, unambiguous observation.
// `resolve_live` is a pure function asserted directly as a truth table.
// ---------------------------------------------------------------------------

#[test]
fn placeholder_view_sets_placeholder() {
    let view = A11yNodeView {
        role: A11yRole::TextInput,
        placeholder: Some("Search…".into()),
        ..Default::default()
    };
    assert_eq!(to_accesskit_node(&view).placeholder(), Some("Search…"));

    // Absence ⇒ no fold arm ⇒ unset.
    let none = A11yNodeView {
        role: A11yRole::TextInput,
        ..Default::default()
    };
    assert_eq!(to_accesskit_node(&none).placeholder(), None);
}

#[test]
fn resolve_live_role_implied_truth_table() {
    use accesskit::Live;
    use buiy_core::a11y::resolve_live;

    // No explicit A11yLive ⇒ role implies the policy.
    assert_eq!(
        resolve_live(A11yRole::Alert, None),
        (Some(Live::Assertive), true),
        "Alert ⇒ Assertive + atomic",
    );
    assert_eq!(
        resolve_live(A11yRole::Status, None),
        (Some(Live::Polite), true),
        "Status ⇒ Polite + atomic",
    );
    assert_eq!(
        resolve_live(A11yRole::Log, None),
        (Some(Live::Polite), false),
        "Log ⇒ Polite, non-atomic",
    );
    // Any other role ⇒ no live region.
    assert_eq!(
        resolve_live(A11yRole::Button, None),
        (None, false),
        "non-live roles imply no live region",
    );
}

#[test]
fn resolve_live_explicit_overrides_role() {
    use accesskit::Live;
    use buiy_core::a11y::{A11yLive, resolve_live};

    // An explicit component wins over the role-implied default, even when the
    // role would imply something different (Alert would imply Assertive+atomic).
    let explicit = A11yLive {
        politeness: Live::Polite,
        atomic: false,
    };
    assert_eq!(
        resolve_live(A11yRole::Alert, Some(explicit)),
        (Some(Live::Polite), false),
        "explicit A11yLive overrides the Alert role default",
    );

    // Explicit on a non-live role still applies (the role implies nothing).
    let explicit = A11yLive {
        politeness: Live::Assertive,
        atomic: true,
    };
    assert_eq!(
        resolve_live(A11yRole::Button, Some(explicit)),
        (Some(Live::Assertive), true),
    );
}

#[test]
fn entity_for_node_id_inverts_node_id_for() {
    use buiy_core::a11y::translate::{entity_for_node_id, node_id_for};
    let e = Entity::from_raw_u32(42).unwrap();
    assert_eq!(entity_for_node_id(node_id_for(e)), Some(e));
}

#[test]
fn entity_for_node_id_maps_root_to_none() {
    use buiy_core::a11y::translate::{ROOT_NODE_ID, entity_for_node_id};
    assert_eq!(entity_for_node_id(ROOT_NODE_ID), None);
}

#[test]
fn entity_for_node_id_rejects_foreign_id() {
    use buiy_core::a11y::translate::entity_for_node_id;
    // NodeId(1) -> id.0 - 1 == 0, which is not a valid Entity::to_bits encoding.
    assert_eq!(entity_for_node_id(accesskit::NodeId(1)), None);
}
