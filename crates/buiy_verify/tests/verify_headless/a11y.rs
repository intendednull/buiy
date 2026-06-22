use bevy::prelude::*;
use buiy_core::a11y::{A11yNodeView, A11yRole};
use buiy_verify::a11y::{diff_snapshots, snapshot_tree};

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
        },
        A11yNodeView {
            entity: entity(2),
            role: A11yRole::Text,
            name: "Hello".into(),
            description: "".into(),
            focusable: false,
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

#[test]
fn snapshot_entity_field_is_the_canonical_node_id() {
    let e = entity(1);
    let nodes = vec![A11yNodeView {
        entity: e,
        role: A11yRole::Button,
        name: "Save".into(),
        description: "".into(),
        focusable: true,
    }];
    let json = snapshot_tree(&nodes);
    let expected = buiy_core::a11y::translate::node_id_for(e).0;
    assert!(
        json.contains(&format!("\"entity\":{expected}")),
        "snapshot must emit the canonical NodeId ref (to_bits()+1), got: {json}",
    );
}
