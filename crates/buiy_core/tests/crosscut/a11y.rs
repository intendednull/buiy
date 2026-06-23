use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{
        A11yDescription, A11yDisabled, A11yExpanded, A11yHidden, A11yLabel, A11yModal,
        A11yPlaceholder, A11yPlugin, A11yRelations, A11yRole, A11ySelected, A11yTextValue,
        A11yToggled, A11yTreeBuilder,
    },
    components::Node,
    focus::Focusable,
};

#[test]
fn adapter_plugin_loads_without_panic() {
    use bevy::winit::accessibility::ACCESS_KIT_ADAPTERS;
    use buiy_core::a11y::AccessKitAdapterPlugin;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.add_plugins(AccessKitAdapterPlugin);
    // `FocusPlugin::handle_tab` reads `Res<ButtonInput<KeyCode>>`; MinimalPlugins
    // doesn't include `InputPlugin`, so we seed the resource manually —
    // same pattern used in `tests/focus.rs`.
    app.init_resource::<ButtonInput<KeyCode>>();
    // The plugin must install `push_tree_updates` without panicking, even
    // when no winit windows exist. Real adapter creation is exercised by
    // running the `hello_button` example end-to-end.
    app.update();
    // bevy_winit's `ACCESS_KIT_ADAPTERS` thread-local is the source of truth
    // for which windows have AccessKit adapters. Under MinimalPlugins no
    // winit windows are spawned, so the map stays empty.
    let bevy_adapters_empty = ACCESS_KIT_ADAPTERS.with_borrow(|m| m.0.is_empty());
    assert!(
        bevy_adapters_empty,
        "no bevy_winit adapters created under MinimalPlugins"
    );
}

#[test]
fn tree_builder_emits_one_node_per_focusable_with_role_and_label() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let _btn = app
        .world_mut()
        .spawn((
            Focusable::default(),
            A11yRole::Button,
            A11yLabel("Save".to_string()),
        ))
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let snapshot = builder.snapshot();
    let count = snapshot
        .iter()
        .filter(|n| n.role == A11yRole::Button)
        .count();
    assert_eq!(count, 1, "exactly one button node in tree");
    let names: Vec<String> = snapshot.iter().map(|n| n.name.clone()).collect();
    assert!(names.contains(&"Save".to_string()), "Save name present");
}

/// Audit #20 (T2.18): the description-extraction branch (`a11y/mod.rs:110`).
/// `A11yDescription` is spawned in zero existing tests, so its surfacing into
/// the built tree is unexercised. Spawn an entity carrying one and assert the
/// description text appears on its node. A regression that drops the
/// `desc.map(...)` extraction (or extracts the wrong component) reddens this.
#[test]
fn description_component_surfaces_in_tree() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let entity = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel("Save".to_string()),
            A11yDescription("Saves the current document".to_string()),
        ))
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let node = builder
        .snapshot()
        .iter()
        .find(|n| n.entity == entity)
        .expect("the entity with an A11yDescription must appear in the tree");
    assert_eq!(
        node.description, "Saves the current document",
        "A11yDescription text must surface as the node's description"
    );
}

/// P1a (Task 15, ACCNAME): `build_tree` derives `A11yNodeView.name` via
/// `compute_accessible_name`, not a raw `A11yLabel` read. The local precedence is
/// `label > value > placeholder`: a node carrying an `A11yLabel` resolves to its
/// text (the no-name-from-label-regression guarantee), while a node *without* one
/// falls back to its `A11yTextValue` then `A11yPlaceholder`. A regression that
/// re-introduced the raw `label.map(...)` read would drop both fallbacks and
/// redden the value/placeholder cases.
#[test]
fn build_tree_derives_accessible_name_with_local_precedence() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    // Label present ⇒ label wins (preserves the prior name-from-label behavior).
    let labeled = app
        .world_mut()
        .spawn((
            A11yRole::TextInput,
            A11yLabel("Email".to_string()),
            A11yTextValue("typed@example.com".to_string()),
            A11yPlaceholder("you@example.com".to_string()),
        ))
        .id();
    // No label, has value ⇒ value wins.
    let valued = app
        .world_mut()
        .spawn((
            A11yRole::TextInput,
            A11yTextValue("hello".to_string()),
            A11yPlaceholder("Search…".to_string()),
        ))
        .id();
    // No label, no value, has placeholder ⇒ placeholder is the name.
    let prompted = app
        .world_mut()
        .spawn((A11yRole::TextInput, A11yPlaceholder("Search…".to_string())))
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let snapshot = builder.snapshot();
    let name_of = |e: Entity| {
        snapshot
            .iter()
            .find(|n| n.entity == e)
            .map(|n| n.name.as_str())
            .expect("entity must surface in the tree")
    };
    assert_eq!(name_of(labeled), "Email", "label wins (no regression)");
    assert_eq!(name_of(valued), "hello", "no label ⇒ value fallback");
    assert_eq!(name_of(prompted), "Search…", "no label/value ⇒ placeholder");
}

/// P1a: `build_tree` projects each decomposed state component from the real ECS
/// world into the corresponding `A11yNodeView` field. A regression in the query
/// widening or the per-component projection (e.g. forgetting to read a marker, or
/// projecting the wrong inner value) reddens this. Markers project to a presence
/// `bool`; wrappers unwrap to their inner accesskit value.
#[test]
fn build_tree_projects_decomposed_state_components() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let e = app
        .world_mut()
        .spawn((
            A11yRole::Checkbox,
            A11yLabel("Bold".to_string()),
            A11yToggled(accesskit::Toggled::Mixed),
            A11yExpanded(true),
            A11ySelected(true),
            A11yDisabled,
            A11yModal,
            A11yHidden,
        ))
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let node = builder
        .snapshot()
        .iter()
        .find(|n| n.entity == e)
        .expect("the stateful entity must surface in the tree");

    assert_eq!(node.toggled, Some(accesskit::Toggled::Mixed));
    assert_eq!(node.expanded, Some(true));
    assert_eq!(node.selected, Some(true));
    assert!(
        node.disabled,
        "A11yDisabled marker presence ⇒ disabled flag"
    );
    assert!(node.modal, "A11yModal marker presence ⇒ modal flag");
    assert!(
        node.hidden,
        "A11yHidden marker presence ⇒ hidden flag (carried for P1b)"
    );
}

/// P1a (Task 13): `build_tree` resolves the four WIRED `A11yRelations` refs from
/// `Entity` to `NodeId` at build time, so the view stays winit-free and `Entity`
/// never leaks past the seam (semantic-tree.md §3). Spawn an owner referencing a
/// target entity and assert the built view carries the target's `node_id_for`,
/// not the raw entity. A regression that forgets the resolution (or resolves the
/// wrong field) reddens this.
#[test]
fn build_tree_resolves_wired_relations_to_node_ids() {
    use buiy_core::a11y::translate::node_id_for;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let target = app
        .world_mut()
        .spawn((A11yRole::Region, A11yLabel("Panel".to_string())))
        .id();
    let owner = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel("Toggle".to_string()),
            A11yRelations {
                labelled_by: vec![target],
                described_by: vec![target],
                controls: vec![target],
                active_descendant: Some(target),
                // Carried-but-unwired: present on the component but never resolved.
                owns: vec![target],
                ..Default::default()
            },
        ))
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let node = builder
        .snapshot()
        .iter()
        .find(|n| n.entity == owner)
        .expect("the relation owner must surface in the tree");

    let target_id = node_id_for(target);
    assert_eq!(node.labelled_by, vec![target_id], "labelled_by resolved");
    assert_eq!(node.described_by, vec![target_id], "described_by resolved");
    assert_eq!(node.controls, vec![target_id], "controls resolved");
    assert_eq!(
        node.active_descendant,
        Some(target_id),
        "active_descendant resolved",
    );
}

/// P1a (Task 13): an entity carrying ONLY `A11yRelations` (no role/label/state)
/// is a11y content on its own — it points at other nodes — so it must surface,
/// not be skipped by the empty-content branch.
#[test]
fn entity_with_only_relations_surfaces() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let target = app.world_mut().spawn(A11yRole::Region).id();
    let owner = app
        .world_mut()
        .spawn(A11yRelations {
            controls: vec![target],
            ..Default::default()
        })
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    assert!(
        builder.snapshot().iter().any(|n| n.entity == owner),
        "an entity carrying only A11yRelations must not be skipped"
    );
}

/// P1a: a decomposed state component is a11y content on its own — an entity
/// carrying ONLY a state component (no role/label/description/focusable) must
/// still surface as a node, otherwise the state would be silently dropped.
#[test]
fn entity_with_only_a_state_component_surfaces() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    let e = app.world_mut().spawn(A11ySelected(true)).id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    assert!(
        builder.snapshot().iter().any(|n| n.entity == e),
        "an entity carrying only a state component must not be skipped"
    );
}

/// Audit #20 (T2.18): the skip-empty branch (`a11y/mod.rs:103`). An entity with
/// no a11y content at all (no role/label/description/focusable) must be skipped
/// — it never becomes a tree node. Here a plain `Node`-only entity is the
/// non-a11y entity; only the a11y-bearing sibling should surface. Removing the
/// `continue` (so empty entities are pushed) reddens this.
#[test]
fn entity_without_a11y_content_is_skipped() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);

    // Non-a11y entity: carries only a layout `Node`, none of the four a11y
    // components the skip branch checks.
    let plain = app.world_mut().spawn(Node).id();
    // An a11y-bearing entity so the tree is non-empty (proves the system ran
    // and the skip is selective, not a blanket "nothing surfaces").
    let labeled = app
        .world_mut()
        .spawn((A11yRole::Button, A11yLabel("OK".to_string())))
        .id();

    app.update();

    let builder = app.world().resource::<A11yTreeBuilder>();
    let snapshot = builder.snapshot();
    assert!(
        snapshot.iter().any(|n| n.entity == labeled),
        "the a11y-bearing entity must be present"
    );
    assert!(
        !snapshot.iter().any(|n| n.entity == plain),
        "an entity with no a11y content must be skipped from the tree"
    );
}
