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
        },
        A11yNodeView {
            entity: Entity::from_raw_u32(2).unwrap(),
            role: A11yRole::Generic,
            name: "Container".into(),
            description: String::new(),
            focusable: false,
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
    };
    let node = to_accesskit_node(&focusable);
    assert!(node.supports_action(accesskit::Action::Focus));

    let non_focusable = A11yNodeView {
        entity: Entity::from_raw_u32(2).unwrap(),
        role: A11yRole::Generic,
        name: "Container".into(),
        description: String::new(),
        focusable: false,
    };
    let node = to_accesskit_node(&non_focusable);
    assert!(!node.supports_action(accesskit::Action::Focus));
}
