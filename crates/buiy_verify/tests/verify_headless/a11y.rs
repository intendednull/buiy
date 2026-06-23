use bevy::prelude::*;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{A11yLabel, A11yNodeView, A11yPlugin, A11yRole};
use buiy_core::{CorePlugin, focus::Focusable};
use buiy_verify::a11y::{
    TreeView, consume, diff_snapshots, node_for, semantic_tree, snapshot_tree,
};

// Bevy 0.18 removed `Entity::from_raw`; `from_raw_u32` returns `Option<Entity>`.
fn entity(index: u32) -> Entity {
    Entity::from_raw_u32(index).expect("valid entity index")
}

#[test]
fn snapshot_tree_serializes_to_stable_json() {
    let nodes = vec![
        A11yNodeView {
            entity: entity(1),
            role: A11yRole::Button,
            name: "Save".into(),
            description: "".into(),
            focusable: true,
            ..Default::default()
        },
        A11yNodeView {
            entity: entity(2),
            role: A11yRole::Text,
            name: "Hello".into(),
            description: "".into(),
            focusable: false,
            ..Default::default()
        },
    ];
    let json = snapshot_tree(&nodes);
    assert!(json.contains("\"role\":\"Button\""));
    assert!(json.contains("\"name\":\"Save\""));
    assert!(json.contains("\"focusable\":true"));
}

#[test]
fn diff_returns_none_for_identical_snapshots() {
    let nodes = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Button,
        name: "Save".into(),
        description: "".into(),
        focusable: true,
        ..Default::default()
    }];
    let snap = snapshot_tree(&nodes);
    assert!(diff_snapshots(&snap, &snap).is_none());
}

#[test]
fn diff_returns_some_for_different_snapshots() {
    let result = diff_snapshots("a", "b");
    assert!(result.is_some());
    let text = result.unwrap();
    assert!(text.contains("LEFT") && text.contains("RIGHT"));
}

// ---------------------------------------------------------------------------
// gate-#3 in-process `accesskit_consumer` read tier (P1a Phase 0).
// The lowest verification rung: build the real TreeUpdate via the production
// fold, hand it to accesskit_consumer, and read the node back the way an AT
// does. Every later P1a state fixture stands on this rung.
// ---------------------------------------------------------------------------

#[test]
fn consumer_reads_back_a_button_label() {
    // Synthetic-view path: the fold → consumer round-trip in isolation, no App.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Button,
        name: "Save".into(),
        description: String::new(),
        focusable: true,
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node =
        node_for(&tree, node_id_for(entity(1))).expect("button node present in the consumer tree");
    // `accesskit_consumer::Node::label()` returns `Option<String>` in 0.36.
    assert_eq!(node.label().as_deref(), Some("Save"));
    assert_eq!(node.role(), accesskit::Role::Button);
}

#[test]
fn semantic_tree_round_trips_role_and_name_through_a_running_app() {
    // End-to-end App path: spawn a minimal a11y entity, drive a frame so the
    // production `build_tree` system fills `A11yTreeBuilder`, then snapshot
    // THROUGH the consumer tier and assert role + name survived the round-trip.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(A11yPlugin);
    app.world_mut().spawn((
        Focusable::default(),
        A11yRole::Button,
        A11yLabel("Save".to_string()),
    ));

    app.update();

    let snapshot = semantic_tree(&mut app, TreeView::Unmerged);
    assert_eq!(
        snapshot, "Button  Save",
        "role + name must round-trip producer → consumer; got: {snapshot:?}",
    );
}

// ---------------------------------------------------------------------------
// P1a first-batch decomposed-state fixtures (gate-#3 consumer tier).
//
// Each builds an `A11yNodeView` carrying one projected state and asserts the
// setter's observable output through the in-process `accesskit_consumer::Tree`
// — the same way an AT reads it. `A11yExpanded` is asserted at the PRODUCER tier
// (in crosscut/a11y_translate.rs) because accesskit_consumer 0.36 exposes no
// public `is_expanded()` getter (semantic-tree.md §0 gap). `A11yHidden` is
// carried-only in P1a (no setter), also covered producer-side.
// ---------------------------------------------------------------------------

#[test]
fn consumer_reads_toggled_tri_state() {
    // `Toggled::Mixed` must survive the round-trip uncollapsed.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Checkbox,
        name: "Bold".into(),
        toggled: Some(accesskit::Toggled::Mixed),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("checkbox node present");
    assert_eq!(node.toggled(), Some(accesskit::Toggled::Mixed));

    // And a plain `True` case.
    let views = vec![A11yNodeView {
        entity: entity(2),
        role: A11yRole::Switch,
        name: "Wifi".into(),
        toggled: Some(accesskit::Toggled::True),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(2))).expect("switch node present");
    assert_eq!(node.toggled(), Some(accesskit::Toggled::True));
}

#[test]
fn consumer_reads_selected() {
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Button,
        name: "Tab One".into(),
        selected: Some(true),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("selected node present");
    assert_eq!(node.is_selected(), Some(true));
}

#[test]
fn consumer_reads_disabled_marker() {
    // Disabled marker present ⇒ the flag is set.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Button,
        name: "Save".into(),
        disabled: true,
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("disabled node present");
    assert!(node.is_disabled());

    // Absent ⇒ not disabled (the fold omits the marker).
    let views = vec![A11yNodeView {
        entity: entity(2),
        role: A11yRole::Button,
        name: "Cancel".into(),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(2))).expect("enabled node present");
    assert!(!node.is_disabled());
}

#[test]
fn consumer_reads_modal_marker() {
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Dialog,
        name: "Confirm".into(),
        modal: true,
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("modal dialog node present");
    assert!(node.is_modal());
}

#[test]
fn snapshot_entity_field_is_the_canonical_node_id() {
    let e = entity(1);
    let nodes = vec![A11yNodeView {
        entity: e,
        role: A11yRole::Button,
        name: "Save".into(),
        description: "".into(),
        focusable: true,
        ..Default::default()
    }];
    let json = snapshot_tree(&nodes);
    let expected = buiy_core::a11y::translate::node_id_for(e).0;
    assert!(
        json.contains(&format!("\"entity\":{expected}")),
        "snapshot must emit the canonical NodeId ref (to_bits()+1), got: {json}",
    );
}
