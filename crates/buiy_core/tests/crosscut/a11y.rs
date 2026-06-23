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
///
/// `A11yHidden` is **not** in this fixture's component set: in P1b it prunes the
/// entity from the tree entirely (§7.4), which is incompatible with asserting the
/// entity surfaces. Its projection into `A11yNodeView.hidden` is covered by the
/// producer-tier `hidden_is_carried_but_not_flagged_in_p1a` (the flag is carried,
/// not folded) and its prune behavior by `a11y_hidden_prunes_entity_and_subtree`.
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

// ===========================================================================
// P1b — real ECS nesting (semantic-tree.md §7).
//
// `build_tree` reads `ChildOf`/`Children` and resolves each node's a11y
// `parent`/`children` by collapsing presentational wrappers
// (`nearest_a11y_ancestor`) and pruning `A11yHidden` subtrees. These fixtures
// build real ECS hierarchies and assert the resolved nesting + the now-live
// `labelledby`/`contents` ACCNAME arms.
// ===========================================================================

/// Build an a11y app, run a frame, and return the freshly built node list.
fn build(spawn: impl FnOnce(&mut World)) -> Vec<buiy_core::a11y::A11yNodeView> {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    spawn(app.world_mut());
    app.update();
    app.world()
        .resource::<A11yTreeBuilder>()
        .snapshot()
        .to_vec()
}

fn node_of(
    snapshot: &[buiy_core::a11y::A11yNodeView],
    e: Entity,
) -> &buiy_core::a11y::A11yNodeView {
    snapshot
        .iter()
        .find(|n| n.entity == e)
        .unwrap_or_else(|| panic!("entity {e:?} must surface in the tree"))
}

/// §7.1: the a11y parent of a node is its **nearest a11y-bearing ancestor** — a
/// pure-layout wrapper between a container and its child collapses with no hole.
/// Hierarchy: container(a11y) → wrapper(plain Node, no a11y) → button(a11y). The
/// button's a11y parent must be the container, and the wrapper must NOT appear.
#[test]
fn nearest_a11y_ancestor_collapses_presentational_wrappers() {
    let mut container = Entity::PLACEHOLDER;
    let mut wrapper = Entity::PLACEHOLDER;
    let mut button = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        container = world
            .spawn((A11yRole::Group, A11yLabel("Toolbar".to_string())))
            .id();
        // Pure-layout wrapper: a `Node` with no a11y content at all.
        wrapper = world.spawn(Node).id();
        button = world
            .spawn((A11yRole::Button, A11yLabel("Bold".to_string())))
            .id();
        world.entity_mut(wrapper).add_children(&[button]);
        world.entity_mut(container).add_children(&[wrapper]);
    });

    // The wrapper carries no a11y content ⇒ no node.
    assert!(
        !snapshot.iter().any(|n| n.entity == wrapper),
        "a presentational wrapper must not emit a node",
    );
    // The button's a11y parent is the container, NOT the wrapper (collapsed).
    let button_node = node_of(&snapshot, button);
    assert_eq!(
        button_node.parent,
        Some(container),
        "button's a11y parent must collapse past the wrapper to the container",
    );
    // The container lists the button as its a11y child in document order.
    let container_node = node_of(&snapshot, container);
    assert_eq!(
        container_node.children,
        vec![button],
        "container's a11y children must collapse the wrapper, yielding the button",
    );
    // A top-level container has no a11y parent (parents to the synthetic root).
    assert_eq!(container_node.parent, None, "container is top-level");
}

/// §7.1: children are emitted in **document order** across a wrapper collapse.
#[test]
fn collapsed_children_preserve_document_order() {
    let mut container = Entity::PLACEHOLDER;
    let mut first = Entity::PLACEHOLDER;
    let mut second = Entity::PLACEHOLDER;
    let mut third = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        container = world.spawn(A11yRole::Group).id();
        first = world
            .spawn((A11yRole::Button, A11yLabel("One".to_string())))
            .id();
        // A wrapper holding the middle child — its child still slots in order.
        let wrapper = world.spawn(Node).id();
        second = world
            .spawn((A11yRole::Button, A11yLabel("Two".to_string())))
            .id();
        third = world
            .spawn((A11yRole::Button, A11yLabel("Three".to_string())))
            .id();
        world.entity_mut(wrapper).add_children(&[second]);
        world
            .entity_mut(container)
            .add_children(&[first, wrapper, third]);
    });

    let container_node = node_of(&snapshot, container);
    assert_eq!(
        container_node.children,
        vec![first, second, third],
        "collapsed children must follow document order (first, wrapped-second, third)",
    );
}

/// §7.4: an entity with `A11yHidden` — and its **whole subtree** — emits NO node.
#[test]
fn a11y_hidden_prunes_entity_and_subtree() {
    let mut visible = Entity::PLACEHOLDER;
    let mut hidden_root = Entity::PLACEHOLDER;
    let mut hidden_child = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        visible = world
            .spawn((A11yRole::Button, A11yLabel("Visible".to_string())))
            .id();
        hidden_root = world
            .spawn((
                A11yRole::Group,
                A11yLabel("Hidden panel".to_string()),
                A11yHidden,
            ))
            .id();
        // A child of the hidden subtree — pruned even though it is not itself
        // marked hidden.
        hidden_child = world
            .spawn((A11yRole::Button, A11yLabel("Inside".to_string())))
            .id();
        world.entity_mut(hidden_root).add_children(&[hidden_child]);
    });

    assert!(
        snapshot.iter().any(|n| n.entity == visible),
        "a non-hidden node must still surface",
    );
    assert!(
        !snapshot.iter().any(|n| n.entity == hidden_root),
        "an A11yHidden entity must be pruned (no node)",
    );
    assert!(
        !snapshot.iter().any(|n| n.entity == hidden_child),
        "the whole subtree under an A11yHidden entity must be pruned",
    );
}

/// §7.1: a node under a pruned subtree collapses to the nearest *non-pruned* a11y
/// ancestor — a hidden wrapper does not become a hole that re-parents its
/// grandchildren. Hierarchy: container → hidden(group) → button. The button is
/// pruned (it is inside the hidden subtree), so it never re-attaches to container.
#[test]
fn hidden_subtree_does_not_reparent_through_the_hole() {
    let mut container = Entity::PLACEHOLDER;
    let mut hidden = Entity::PLACEHOLDER;
    let mut button = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        container = world
            .spawn((A11yRole::Group, A11yLabel("Box".to_string())))
            .id();
        hidden = world.spawn((A11yRole::Group, A11yHidden)).id();
        button = world
            .spawn((A11yRole::Button, A11yLabel("Deep".to_string())))
            .id();
        world.entity_mut(hidden).add_children(&[button]);
        world.entity_mut(container).add_children(&[hidden]);
    });

    assert!(
        !snapshot.iter().any(|n| n.entity == button),
        "a node inside a hidden subtree is pruned, not re-parented to the container",
    );
    let container_node = node_of(&snapshot, container);
    assert!(
        container_node.children.is_empty(),
        "the container's only child was hidden, so it has no a11y children",
    );
}

/// §7.4: the `A11yHidden` prune climbs through a **non-a11y wrapper** ancestor —
/// a hidden marker on a grandparent prunes a grandchild even when the
/// intervening entity carries no a11y content. Hierarchy: hidden(group,a11y) →
/// wrapper(plain Node) → button(a11y); the button must be pruned.
#[test]
fn a11y_hidden_propagates_through_non_a11y_wrapper() {
    let mut button = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        let hidden = world.spawn((A11yRole::Group, A11yHidden)).id();
        let wrapper = world.spawn(Node).id();
        button = world
            .spawn((A11yRole::Button, A11yLabel("Deep".to_string())))
            .id();
        world.entity_mut(wrapper).add_children(&[button]);
        world.entity_mut(hidden).add_children(&[wrapper]);
    });

    assert!(
        !snapshot.iter().any(|n| n.entity == button),
        "the prune must climb through a non-a11y wrapper to the hidden ancestor",
    );
}

/// §6: the `labelledby` ACCNAME arm is now live — a node carrying
/// `A11yRelations.labelled_by` takes its name from the referenced node's name,
/// overriding its own local label. `build_tree` resolves the target's name.
#[test]
fn accname_labelledby_overrides_local_label() {
    let mut field = Entity::PLACEHOLDER;
    let mut labeler = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        labeler = world
            .spawn((A11yRole::Text, A11yLabel("Email address".to_string())))
            .id();
        field = world
            .spawn((
                A11yRole::TextInput,
                A11yLabel("Local label".to_string()),
                A11yRelations {
                    labelled_by: vec![labeler],
                    ..Default::default()
                },
            ))
            .id();
    });

    assert_eq!(
        node_of(&snapshot, field).name,
        "Email address",
        "labelledby must win over the field's own local label",
    );
}

/// §6: the `contents` ACCNAME arm is now live — a node with no local label/value/
/// placeholder takes its name from its subtree text (its collapsed a11y
/// children's names, joined). A button wrapping a text node names from it.
#[test]
fn accname_contents_names_from_subtree_text() {
    let mut button = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        button = world.spawn(A11yRole::Button).id(); // no local name source
        let icon_wrapper = world.spawn(Node).id();
        let text = world
            .spawn((A11yRole::Text, A11yLabel("Click me".to_string())))
            .id();
        // Wrap the text in a presentational node so the collapse is exercised.
        world.entity_mut(icon_wrapper).add_children(&[text]);
        world.entity_mut(button).add_children(&[icon_wrapper]);
    });

    assert_eq!(
        node_of(&snapshot, button).name,
        "Click me",
        "a button with no local name takes its name from its subtree contents",
    );
}

/// §6 precedence: a local label still beats `contents` (the subtree text is the
/// fallback only when no higher arm contributes).
#[test]
fn accname_local_label_beats_contents() {
    let mut button = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        button = world
            .spawn((A11yRole::Button, A11yLabel("Save".to_string())))
            .id();
        let text = world
            .spawn((A11yRole::Text, A11yLabel("ignored subtree".to_string())))
            .id();
        world.entity_mut(button).add_children(&[text]);
    });

    assert_eq!(
        node_of(&snapshot, button).name,
        "Save",
        "an explicit local label outranks the subtree contents",
    );
}

// ---------------------------------------------------------------------------
// The three NAMED gate-#12 invariants (semantic-tree.md §9, co-drive §3.1).
//
// Targeted assertions over representative nested fixtures — NOT a proptest fuzz
// corpus (the exhaustive generators are deferred, co-drive §3.2). Each fixture
// is a small, hand-built nested tree; the invariant holds by construction over
// the real `build_tree` output.
// ---------------------------------------------------------------------------

/// A representative nested fixture: a container with a presentational wrapper, a
/// label child, and a focusable button — plus a hidden subtree and a focusable
/// leaf. Returns `(world-spawn closure, the focusable button, the focusable
/// leaf)` for the invariants to assert over.
fn representative_fixture(world: &mut World) -> (Entity, Entity) {
    // Container → wrapper(plain) → { label(Text), button(focusable) }.
    let container = world
        .spawn((A11yRole::Group, A11yLabel("Form".to_string())))
        .id();
    let wrapper = world.spawn(Node).id();
    let label = world
        .spawn((A11yRole::Text, A11yLabel("Name".to_string())))
        .id();
    let button = world
        .spawn((
            A11yRole::Button,
            A11yLabel("Submit".to_string()),
            Focusable::default(),
        ))
        .id();
    world.entity_mut(wrapper).add_children(&[label, button]);
    world.entity_mut(container).add_children(&[wrapper]);

    // A hidden subtree (must contribute no nodes, and its focusable is unreachable).
    let hidden = world.spawn((A11yRole::Group, A11yHidden)).id();
    let hidden_focusable = world
        .spawn((
            A11yRole::Button,
            A11yLabel("Secret".to_string()),
            Focusable::default(),
        ))
        .id();
    world.entity_mut(hidden).add_children(&[hidden_focusable]);

    // A top-level focusable leaf.
    let leaf = world
        .spawn((
            A11yRole::Button,
            A11yLabel("Cancel".to_string()),
            Focusable::default(),
        ))
        .id();

    (button, leaf)
}

/// Invariant (a) — **no orphans**: every emitted non-root node has a resolvable
/// parent — either an a11y parent that is itself emitted, or `None` (top-level,
/// parented to the synthetic root). A node pointing at a parent that does not
/// emit would be an orphan.
#[test]
fn invariant_no_orphans() {
    let snapshot = build(|world| {
        representative_fixture(world);
    });
    let emitted: std::collections::HashSet<Entity> = snapshot.iter().map(|n| n.entity).collect();
    for node in &snapshot {
        if let Some(parent) = node.parent {
            assert!(
                emitted.contains(&parent),
                "node {:?} has a11y parent {:?} which is not itself emitted — orphan",
                node.entity,
                parent,
            );
        }
        // Every listed child must also be emitted (no dangling child edge).
        for &child in &node.children {
            assert!(
                emitted.contains(&child),
                "node {:?} lists child {:?} which is not emitted",
                node.entity,
                child,
            );
        }
    }
}

/// Invariant (b) — **focus-reachable**: every `Focusable` entity NOT under
/// `A11yHidden` appears in the tree; a hidden focusable does not.
#[test]
fn invariant_focus_reachable() {
    let mut button = Entity::PLACEHOLDER;
    let mut leaf = Entity::PLACEHOLDER;
    let snapshot = build(|world| {
        let (b, l) = representative_fixture(world);
        button = b;
        leaf = l;
    });
    assert!(
        snapshot.iter().any(|n| n.entity == button),
        "a non-hidden focusable (the Submit button) must be reachable in the tree",
    );
    assert!(
        snapshot.iter().any(|n| n.entity == leaf),
        "a top-level focusable leaf (Cancel) must be reachable in the tree",
    );
    // The hidden focusable must NOT appear (pruned) — assert no focusable node is
    // missing its name, and that exactly the two visible focusables are present.
    let focusable_count = snapshot.iter().filter(|n| n.focusable).count();
    assert_eq!(
        focusable_count, 2,
        "exactly the two non-hidden focusables surface (the hidden one is pruned)",
    );
}

/// Invariant (c) — **every-focusable-named**: every focusable node has a
/// non-empty accessible name (an unnamed focusable is an APG defect).
#[test]
fn invariant_every_focusable_named() {
    let snapshot = build(|world| {
        representative_fixture(world);
    });
    for node in &snapshot {
        if node.focusable {
            assert!(
                !node.name.is_empty(),
                "focusable node {:?} ({:?}) has an empty accessible name",
                node.entity,
                node.role,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// P1c-a — the `A11yContract` registry + Button `honor` lowering.
//
// The inbound ROUTER that *calls* `honor` after the liveness + live-capability
// guard is the next slice (P1c-b). Here we pin the contract surface directly:
// the registry maps `Button → {Click}`, and the Button `honor(Click)` lowers
// into the shared `OnPress` sink (SC-1) — the same message the pointer path
// writes.
// ---------------------------------------------------------------------------

#[test]
fn contract_registry_keys_button_on_click() {
    use accesskit::Action;
    use buiy_core::a11y::contract_for;

    let entry = contract_for(A11yRole::Button).expect("Button has a contract");
    assert_eq!(
        entry.actions,
        &[Action::Click],
        "Button advertises Click beyond the implicit Focus/Blur",
    );

    // A non-interactive container role has no contract (it advertises only the
    // implicit Focus/Blur when focusable, nothing role-specific).
    assert!(
        contract_for(A11yRole::Generic).is_none(),
        "Generic has no interactive contract",
    );
    // Wave-3 slice-1 wired Checkbox + Switch: each advertises {Click} beyond the
    // implicit Focus/Blur (the tri-state checkbox + binary switch contracts).
    let cb = contract_for(A11yRole::Checkbox).expect("Checkbox has a contract");
    assert_eq!(cb.actions, &[Action::Click], "Checkbox advertises Click");
    let sw = contract_for(A11yRole::Switch).expect("Switch has a contract");
    assert_eq!(sw.actions, &[Action::Click], "Switch advertises Click");
    // Slice-2 wired Slider: it advertises the VALUE verbs (the first non-Click
    // contract) beyond the implicit Focus/Blur.
    let slider = contract_for(A11yRole::Slider).expect("Slider has a contract");
    assert_eq!(
        slider.actions,
        &[Action::Increment, Action::Decrement, Action::SetValue],
        "Slider advertises {{Increment, Decrement, SetValue}}",
    );
    // The remaining widget roles are wired in later P1d slices (no contract yet).
    assert!(contract_for(A11yRole::TextInput).is_none());
}

#[test]
fn button_honor_click_emits_on_press() {
    use accesskit::Action;
    use bevy::ecs::message::Messages;
    use buiy_core::a11y::A11yContract;
    use buiy_core::a11y::contract::Button;
    use buiy_core::interaction::OnPress;

    // `InteractionPlugin` (via `CorePlugin`) registers `Messages<OnPress>`, the
    // sink the Button contract writes.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(CorePlugin);
    let entity = app.world_mut().spawn_empty().id();

    // Call the contract `honor` directly (the router is P1c-b). Click ⇒ OnPress.
    let result = Button::honor(app.world_mut(), entity, Action::Click, None);
    assert!(result.is_ok(), "Button honor(Click) succeeds: {result:?}");

    let messages = app.world().resource::<Messages<OnPress>>();
    let mut cursor = messages.get_cursor();
    let fired: Vec<_> = cursor.read(messages).map(|m| m.0).collect();
    assert_eq!(
        fired,
        vec![entity],
        "Button honor(Click) must emit OnPress(entity) into the shared sink (SC-1)",
    );
}

#[test]
fn button_honor_unadvertised_verb_is_unsupported_not_panic() {
    use accesskit::Action;
    use buiy_core::a11y::contract::Button;
    use buiy_core::a11y::translate::node_id_for;
    use buiy_core::a11y::{A11yContract, ActionError};

    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(CorePlugin);
    let entity = app.world_mut().spawn_empty().id();

    // A verb the Button contract does not advertise reaches `honor` only as dead
    // code (the router rejects it at the §3 filter first); honor reports it as a
    // typed error rather than panicking.
    let result = Button::honor(app.world_mut(), entity, Action::Increment, None);
    assert_eq!(
        result,
        Err(ActionError::Unsupported {
            target: node_id_for(entity),
            action: Action::Increment,
        }),
    );
}
