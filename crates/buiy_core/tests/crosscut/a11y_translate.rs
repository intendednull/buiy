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
    let update = build_tree_update(&views, None, None);
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
    let update = build_tree_update(&views, Some(focused_id), None);
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
// P1c-a action advertisement — gate-#3 PRODUCER-tier node-action introspection.
//
// The advertised action set on a node is read directly off the producer
// `accesskit::Node` via `supports_action(action)`, which returns exactly the
// node-LOCAL action bitmask (lib.rs:1756: `actions & action.mask()`). This is
// the unambiguous "what this node advertises" observation — the consumer
// `supports_action` walks a parent filter (a slider parent can contribute
// `Increment` to a child), which would muddy an exact-set assertion. The
// existing focus-action test above is on the same producer tier.
//
// `add_action` now derives from `Focusable ⇒ {Focus, Blur}` PLUS the role's
// `contract_for(role).actions()` (Button ⇒ `Click`), replacing the old
// focusable-`Focus`-only hardcode (widget-contracts.md §§1,5).
// ---------------------------------------------------------------------------

/// The full accesskit 0.24 `Action` set, used to assert an *exact* advertised
/// set (the producer exposes no public action iterator, so we probe every
/// variant: assert each expected one is present and every other absent).
const ALL_ACTIONS: &[accesskit::Action] = {
    use accesskit::Action::*;
    &[
        Click,
        Focus,
        Blur,
        Collapse,
        CustomAction,
        Decrement,
        Increment,
        HideTooltip,
        ShowTooltip,
        ReplaceSelectedText,
        ScrollDown,
        ScrollLeft,
        ScrollRight,
        ScrollUp,
        ScrollIntoView,
        ScrollToPoint,
        SetScrollOffset,
        SetTextSelection,
        SetSequentialFocusNavigationStartingPoint,
        SetValue,
        ShowContextMenu,
        Expand,
    ]
};

/// Assert a node advertises EXACTLY `expected` and no other action.
fn assert_advertises_exactly(node: &accesskit::Node, expected: &[accesskit::Action]) {
    for &a in ALL_ACTIONS {
        let want = expected.contains(&a);
        assert_eq!(
            node.supports_action(a),
            want,
            "action {a:?}: advertised={}, expected={want}",
            node.supports_action(a),
        );
    }
}

#[test]
fn button_node_advertises_click_focus_blur() {
    use accesskit::Action::{Blur, Click, Focus};
    // A focusable Button: Focusable ⇒ {Focus, Blur}; the Button contract ⇒ Click.
    let button = A11yNodeView {
        entity: Entity::from_raw_u32(1).unwrap(),
        role: A11yRole::Button,
        name: "Save".into(),
        focusable: true,
        ..Default::default()
    };
    let node = to_accesskit_node(&button);
    assert_advertises_exactly(&node, &[Click, Focus, Blur]);
}

#[test]
fn bare_focusable_advertises_focus_blur_only() {
    use accesskit::Action::{Blur, Focus};
    // A focusable node with NO interactive widget role (Generic) advertises the
    // implicit {Focus, Blur} and nothing else — no contract contributes verbs.
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(2).unwrap(),
        role: A11yRole::Generic,
        name: "Focusable thing".into(),
        focusable: true,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_advertises_exactly(&node, &[Focus, Blur]);
}

#[test]
fn non_focusable_generic_advertises_no_actions() {
    // A non-focusable Generic node: no Focusable ⇒ no {Focus, Blur}; no
    // interactive contract ⇒ no role verbs. It advertises NOTHING.
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(3).unwrap(),
        role: A11yRole::Generic,
        name: "Container".into(),
        focusable: false,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_advertises_exactly(&node, &[]);
}

#[test]
fn non_focusable_button_still_advertises_click() {
    use accesskit::Action::Click;
    // A Button that is somehow not focusable still advertises its contract verb
    // (Click) but NOT {Focus, Blur} (those ride Focusable). Pins that the two
    // contributors are independent.
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(4).unwrap(),
        role: A11yRole::Button,
        name: "Inert".into(),
        focusable: false,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_advertises_exactly(&node, &[Click]);
}

#[test]
fn disclosure_trigger_advertises_click_expand_collapse_focus_blur() {
    use accesskit::Action::{Blur, Click, Collapse, Expand, Focus};
    // GATE #3 (slice-3): a disclosure-trigger is a focusable Button carrying
    // `A11yExpanded` (the view's `expanded` projection). It advertises the Button
    // contract's `Click`, the Focusable `{Focus, Blur}`, AND — keyed on the
    // `A11yExpanded` state, not the role — `{Expand, Collapse}`. Pins that the
    // state-keyed capability layers ON the role contract (Click survives).
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(5).unwrap(),
        role: A11yRole::Button,
        name: "Details".into(),
        focusable: true,
        expanded: Some(false),
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_advertises_exactly(&node, &[Click, Expand, Collapse, Focus, Blur]);
}

#[test]
fn expand_collapse_are_state_keyed_not_role_keyed() {
    use accesskit::Action::{Blur, Collapse, Expand, Focus};
    // The Expand/Collapse advertisement is keyed on `A11yExpanded` (the view's
    // `expanded`), NOT a role: a non-Button expandable (a Generic/Group carrying
    // the state) advertises them too. Conversely a Button WITHOUT the state does
    // not (asserted by `button_node_advertises_click_focus_blur` above).
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(6).unwrap(),
        role: A11yRole::Group,
        name: "Section".into(),
        focusable: true,
        expanded: Some(true),
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    // Group has no interactive contract, so only the Focusable {Focus, Blur} + the
    // state-keyed {Expand, Collapse} — and NO Click (the state-keyed capability is
    // role-agnostic).
    assert_advertises_exactly(&node, &[Expand, Collapse, Focus, Blur]);
}

#[test]
fn tooltip_trigger_advertises_show_hide_tooltip_focus_blur() {
    use accesskit::Action::{Blur, Focus, HideTooltip, ShowTooltip};
    // GATE #3 (slice-5): a tooltip trigger is a focusable node carrying
    // `A11yTooltipHost` (the view's `tooltip_host` projection) and a NEUTRAL role
    // (Generic, no role contract). It advertises the Focusable `{Focus, Blur}` AND
    // — keyed on the `A11yTooltipHost` state, not the role — `{ShowTooltip,
    // HideTooltip}`, and NO `Click` (the neutral role contributes no activation).
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(7).unwrap(),
        role: A11yRole::Generic,
        name: "Help".into(),
        focusable: true,
        tooltip_host: true,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_advertises_exactly(&node, &[ShowTooltip, HideTooltip, Focus, Blur]);
}

#[test]
fn show_hide_tooltip_are_state_keyed_not_role_keyed() {
    use accesskit::Action::{Click, HideTooltip, ShowTooltip};
    // The ShowTooltip/HideTooltip advertisement is keyed on `A11yTooltipHost` (the
    // view's `tooltip_host`), NOT a role — so a Button that ALSO hosts a tooltip
    // advertises BOTH its Click contract AND the tooltip verbs (the capability
    // layers on the role, like Expand/Collapse). A non-focusable Button is used to
    // isolate the role-contract + state-keyed contributions from the Focusable ones.
    let view = A11yNodeView {
        entity: Entity::from_raw_u32(8).unwrap(),
        role: A11yRole::Button,
        name: "Iconbtn".into(),
        focusable: false,
        tooltip_host: true,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_advertises_exactly(&node, &[Click, ShowTooltip, HideTooltip]);
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

// ---------------------------------------------------------------------------
// P1a relations + SC-4 scroll — PRODUCER-tier fixtures.
//
// `described_by` is asserted here because accesskit_consumer 0.36 exposes NO
// `described_by()` getter (only `labelled_by`/`controls`/`active_descendant`
// surface on the consumer — those are asserted at the consumer tier in
// verify_headless/a11y.rs). The producer `accesskit::Node::describedby()` reads
// it back directly. The four carried-but-unwired relation fields
// (`owns`/`flow_to`/`details`/`error_message`) have NO fold arm, so there is
// nothing to assert for them (co-drive §3.2).
//
// SC-4 scroll is asserted here: the producer `Node` exposes the f64 scroll
// getters (`scroll_x()`/`scroll_y()`/… → `Option<f64>`) directly, which is the
// cleanest observation of the six scroll setters firing with the right values.
// ---------------------------------------------------------------------------

#[test]
fn described_by_view_sets_described_by() {
    use buiy_core::a11y::translate::node_id_for;
    // The view carries relations already resolved to NodeId (build_tree's job).
    let target = node_id_for(Entity::from_raw_u32(2).unwrap());
    let view = A11yNodeView {
        role: A11yRole::TextInput,
        described_by: vec![target],
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    // accesskit 0.24's `node_id_vec_property_methods!` getter returns `&[NodeId]`.
    assert_eq!(node.described_by(), &[target]);

    // Empty ⇒ no fold arm ⇒ the property stays unset (empty slice).
    let none = A11yNodeView {
        role: A11yRole::TextInput,
        ..Default::default()
    };
    assert!(to_accesskit_node(&none).described_by().is_empty());
}

#[test]
fn unwired_relation_fields_have_no_fold_arm() {
    // The four carried-but-unwired relation fields never reach the view (they
    // have no `A11yNodeView` projection), so the only thing to pin is that the
    // wired arms don't accidentally emit when the relation is absent — and that
    // a node with no relations emits none of the relation setters.
    let view = A11yNodeView {
        role: A11yRole::Button,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert!(node.labelled_by().is_empty());
    assert!(node.described_by().is_empty());
    assert!(node.controls().is_empty());
    assert_eq!(node.active_descendant(), None);
    // owns / flow_to / details / error_message are never set in P1a.
    assert!(node.owns().is_empty());
    assert_eq!(node.error_message(), None);
}

#[test]
fn scroll_view_sets_the_six_scroll_setters() {
    use buiy_core::a11y::A11yScrollView;
    // offset (0, 40), content (100, 300), viewport (100, 100):
    //   x: max = (100 - 100).max(0) = 0   (no horizontal overflow)
    //   y: max = (300 - 100).max(0) = 200
    let view = A11yNodeView {
        role: A11yRole::Region,
        scroll: Some(A11yScrollView {
            offset: Vec2::new(0.0, 40.0),
            content_extent: Vec2::new(100.0, 300.0),
            viewport_extent: Vec2::new(100.0, 100.0),
            scrollable: true,
        }),
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    // The producer `Node` exposes the f64 scroll getters directly.
    assert_eq!(node.scroll_x(), Some(0.0));
    assert_eq!(node.scroll_x_min(), Some(0.0));
    assert_eq!(node.scroll_x_max(), Some(0.0));
    assert_eq!(node.scroll_y(), Some(40.0));
    assert_eq!(node.scroll_y_min(), Some(0.0));
    assert_eq!(node.scroll_y_max(), Some(200.0));
}

#[test]
fn no_scroll_view_fires_no_scroll_setter() {
    // `scroll: None` (every non-scroll node) ⇒ not a scroll container ⇒ no
    // scroll setter fires.
    let view = A11yNodeView {
        role: A11yRole::Button,
        ..Default::default()
    };
    let node = to_accesskit_node(&view);
    assert_eq!(node.scroll_x(), None);
    assert_eq!(node.scroll_y(), None);
    assert_eq!(node.scroll_y_max(), None);
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
