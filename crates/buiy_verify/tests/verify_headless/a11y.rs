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

// ---------------------------------------------------------------------------
// P1a second-batch decomposed-state fixtures (gate-#3 consumer tier).
//
// value / text_value / orientation / has_popup / live are all surfaced by
// accesskit_consumer 0.36 getters, so they assert at the consumer tier here.
// `placeholder` is consumer-filtered (it only surfaces on an empty text-input
// node), and the `resolve_live` role→policy mapping is a pure function — both
// are asserted at the PRODUCER tier in crosscut/a11y_translate.rs.
// ---------------------------------------------------------------------------

#[test]
fn consumer_reads_slider_numeric_value() {
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Slider,
        name: "Volume".into(),
        value: Some(buiy_core::a11y::A11yValue {
            now: 0.5,
            min: 0.0,
            max: 1.0,
            step: Some(0.1),
            jump: Some(0.25),
            text: None,
        }),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("slider node present");
    assert_eq!(node.numeric_value(), Some(0.5));
    assert_eq!(node.min_numeric_value(), Some(0.0));
    assert_eq!(node.max_numeric_value(), Some(1.0));
    assert_eq!(node.numeric_value_step(), Some(0.1));
    assert_eq!(node.numeric_value_jump(), Some(0.25));
}

#[test]
fn consumer_reads_value_text_when_present() {
    // `A11yValue.text` is the human-readable rendering → set_value.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Slider,
        name: "Zoom".into(),
        value: Some(buiy_core::a11y::A11yValue {
            now: 0.5,
            min: 0.0,
            max: 1.0,
            step: None,
            jump: None,
            text: Some("50%".into()),
        }),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("slider node present");
    assert_eq!(node.numeric_value(), Some(0.5));
    assert_eq!(node.value().as_deref(), Some("50%"));
}

#[test]
fn consumer_reads_text_value() {
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::TextInput,
        name: "Name".into(),
        text_value: Some("hello".into()),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("text input node present");
    assert_eq!(node.value().as_deref(), Some("hello"));
}

#[test]
fn consumer_reads_orientation() {
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Slider,
        name: "Pan".into(),
        orientation: Some(accesskit::Orientation::Horizontal),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("oriented node present");
    assert_eq!(node.orientation(), Some(accesskit::Orientation::Horizontal));
}

#[test]
fn consumer_reads_has_popup() {
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Button,
        name: "Menu".into(),
        has_popup: Some(accesskit::HasPopup::Menu),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("popup-button node present");
    assert_eq!(node.has_popup(), Some(accesskit::HasPopup::Menu));
}

#[test]
fn consumer_reads_explicit_live_region() {
    // An explicit A11yLive overrides any role-implied policy.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Region,
        name: "Updates".into(),
        live: Some(buiy_core::a11y::A11yLive {
            politeness: accesskit::Live::Assertive,
            atomic: true,
        }),
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("live region node present");
    assert_eq!(node.live(), accesskit::Live::Assertive);
    assert!(node.is_live_atomic());
}

#[test]
fn consumer_reads_role_implied_status_live_region() {
    // A `Status` role with NO explicit A11yLive must still announce Polite+atomic
    // (resolve_live role-implied path; semantic-tree.md §5). Without it, gate #4
    // is wrong for a status node carrying no author A11yLive.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Status,
        name: "Saved".into(),
        live: None,
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("status node present");
    assert_eq!(node.live(), accesskit::Live::Polite);
    assert!(
        node.is_live_atomic(),
        "Status role implies atomic announcements"
    );
}

#[test]
fn consumer_reads_role_implied_alert_live_region() {
    // `Alert` ⇒ Assertive + atomic with no explicit A11yLive.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Alert,
        name: "Error".into(),
        live: None,
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("alert node present");
    assert_eq!(node.live(), accesskit::Live::Assertive);
    assert!(node.is_live_atomic());
}

#[test]
fn consumer_non_live_role_has_no_live_region() {
    // A plain Button implies no live region: resolve_live returns (None, _) so the
    // fold emits no set_live, and the consumer falls back to Live::Off.
    let views = vec![A11yNodeView {
        entity: entity(1),
        role: A11yRole::Button,
        name: "Save".into(),
        live: None,
        ..Default::default()
    }];
    let tree = consume(&views, None);
    let node = node_for(&tree, node_id_for(entity(1))).expect("button node present");
    assert_eq!(node.live(), accesskit::Live::Off);
    assert!(!node.is_live_atomic());
}

// ---------------------------------------------------------------------------
// P1a relations — gate-#3 consumer-tier fixtures.
//
// accesskit_consumer 0.36 surfaces `labelled_by()`, `controls()` (both yield an
// iterator of `Node`) and `active_descendant() -> Option<Node>`, so these three
// WIRED relations are asserted at the consumer tier here. `described_by` has NO
// consumer getter, so it is asserted at the producer tier in
// crosscut/a11y_translate.rs. The relation view fields carry NodeIds already
// resolved by `build_tree`; the fixtures build them directly with `node_id_for`,
// and an integration test (crosscut/a11y.rs) exercises the Entity → NodeId
// resolution end-to-end.
//
// To match a relation target the consumer resolves a producer `NodeId` to its
// internal consumer `NodeId` via `node_for(...).id()`, then compares ids (the
// consumer `Node` is not `PartialEq`, but its `id()` is).
// ---------------------------------------------------------------------------

#[test]
fn consumer_resolves_labelled_by_relation() {
    // A (entity 1) is labelled by B (entity 2). Both must exist in the tree so
    // the consumer can resolve the relation target.
    let a = entity(1);
    let b = entity(2);
    let views = vec![
        A11yNodeView {
            entity: a,
            role: A11yRole::TextInput,
            name: "Field".into(),
            labelled_by: vec![node_id_for(b)],
            ..Default::default()
        },
        A11yNodeView {
            entity: b,
            role: A11yRole::Text,
            name: "Email address".into(),
            ..Default::default()
        },
    ];
    let tree = consume(&views, None);
    let node_a = node_for(&tree, node_id_for(a)).expect("labelled node present");
    let node_b = node_for(&tree, node_id_for(b)).expect("label node present");
    // The explicit non-empty list short-circuits the consumer's role-based
    // descendant-label fallback, so labelled_by() yields exactly B.
    let labels: Vec<_> = node_a.labelled_by().map(|n| n.id()).collect();
    assert_eq!(labels, vec![node_b.id()], "labelled_by must resolve to B");
}

#[test]
fn consumer_resolves_controls_relation() {
    // A (entity 1) controls B (entity 2). The consumer `controls()` resolves
    // each controlled id to its node (and panics if absent), so B must exist.
    let a = entity(1);
    let b = entity(2);
    let views = vec![
        A11yNodeView {
            entity: a,
            role: A11yRole::Button,
            name: "Toggle panel".into(),
            controls: vec![node_id_for(b)],
            ..Default::default()
        },
        A11yNodeView {
            entity: b,
            role: A11yRole::Region,
            name: "Panel".into(),
            ..Default::default()
        },
    ];
    let tree = consume(&views, None);
    let node_a = node_for(&tree, node_id_for(a)).expect("controlling node present");
    let node_b = node_for(&tree, node_id_for(b)).expect("controlled node present");
    let controlled: Vec<_> = node_a.controls().map(|n| n.id()).collect();
    assert_eq!(controlled, vec![node_b.id()], "controls must resolve to B");
}

#[test]
fn consumer_resolves_active_descendant_relation() {
    // A composite (entity 1) has active descendant B (entity 2).
    let a = entity(1);
    let b = entity(2);
    let views = vec![
        A11yNodeView {
            entity: a,
            role: A11yRole::Group,
            name: "Listbox".into(),
            active_descendant: Some(node_id_for(b)),
            ..Default::default()
        },
        A11yNodeView {
            entity: b,
            role: A11yRole::Button,
            name: "Option 2".into(),
            selected: Some(true),
            ..Default::default()
        },
    ];
    let tree = consume(&views, None);
    let node_a = node_for(&tree, node_id_for(a)).expect("composite node present");
    let node_b = node_for(&tree, node_id_for(b)).expect("active descendant present");
    assert_eq!(
        node_a.active_descendant().map(|n| n.id()),
        Some(node_b.id()),
        "active_descendant must resolve to B",
    );
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
