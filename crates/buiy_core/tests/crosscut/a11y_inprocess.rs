//! P1c-c — the **in-process inspection driver** (`a11y::inprocess`): act-then-
//! observe over the headless dispatch seam, end-to-end with no winit adapter and
//! no GPU (inprocess-api.md §§2–5).
//!
//! These prove the driver works as a real test driver: `get_by_role` resolves a
//! widget by role(+name) under the strict single-match rule; `click` drives the
//! same `OnPress` activation the pointer/keyboard paths write and returns the
//! post-action tree; `focus` shows the node focused in the returned snapshot; an
//! unsupported verb surfaces as a typed `Err` (never a panic); and `wait_for`
//! blocks a real frame-loop on a semantic condition (succeeds when it becomes
//! true, times out as `Err` when it never does).

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use buiy_core::a11y::inprocess::TreeView;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{
    A11yExpanded, A11yLabel, A11yPlugin, A11yRole, A11yValue, ActionError, NotActionableReason,
    SemanticTree, StateQuery, click, focus, get_by_role, increment, perform, snapshot, wait_for,
};
use buiy_core::focus::{FocusPlugin, Focusable};
use buiy_core::{CorePlugin, interaction::OnPress};

/// A minimal headless app with the a11y + focus + interaction surface (the
/// `OnPress` sink rides `CorePlugin`/`InteractionPlugin`). The same harness the
/// dispatch-seam tests use — the driver layers over the exact same seam.
fn setup() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    // The router's keyboard sibling (`keyboard_activation`) reads
    // `KeyboardInput` (a Message). `MinimalPlugins` registers no `InputPlugin`,
    // so register the message manually (mirrors the a11y_action harness) — the
    // system param is optional, but the resource must exist for the schedule to
    // validate cleanly under this partial harness.
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_message::<KeyboardInput>();
    app
}

/// Drain `Messages<OnPress>` into the activated entities (advances the cursor).
fn drain_on_press(app: &mut App) -> Vec<Entity> {
    let sys = app
        .world_mut()
        .register_system(|mut reader: MessageReader<OnPress>| {
            reader.read().map(|p| p.0).collect::<Vec<_>>()
        });
    let out = app.world_mut().run_system(sys).unwrap();
    app.world_mut().unregister_system(sys).ok();
    out
}

// ---------------------------------------------------------------------------
// get_by_role — strict single-match resolver (inprocess-api.md §3.2)
// ---------------------------------------------------------------------------

#[test]
fn get_by_role_finds_button_by_role_and_name() {
    let mut app = setup();
    let save = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel("Save".into()),
            Focusable::default(),
        ))
        .id();
    app.world_mut().spawn((
        A11yRole::Button,
        A11yLabel("Cancel".into()),
        Focusable::default(),
    ));
    app.update();

    // Role alone is ambiguous (two buttons) — must disambiguate by name.
    let by_name = get_by_role(app.world_mut(), A11yRole::Button, Some("Save"), None);
    assert_eq!(
        by_name,
        Ok(node_id_for(save)),
        "role+name resolves the single matching Button"
    );
}

#[test]
fn get_by_role_no_match_is_not_found() {
    let mut app = setup();
    app.world_mut().spawn((
        A11yRole::Button,
        A11yLabel("Save".into()),
        Focusable::default(),
    ));
    app.update();

    // No Checkbox in the tree at all.
    let res = get_by_role(app.world_mut(), A11yRole::Checkbox, None, None);
    assert_eq!(
        res,
        Err(ActionError::NotFound {
            target: buiy_core::a11y::translate::ROOT_NODE_ID,
        }),
        "zero matches is a loud NotFound, not a silent None"
    );
}

#[test]
fn get_by_role_two_matches_is_ambiguous_error() {
    let mut app = setup();
    // Two buttons with the SAME name — role+name still matches both.
    app.world_mut().spawn((
        A11yRole::Button,
        A11yLabel("OK".into()),
        Focusable::default(),
    ));
    app.world_mut().spawn((
        A11yRole::Button,
        A11yLabel("OK".into()),
        Focusable::default(),
    ));
    app.update();

    let res = get_by_role(app.world_mut(), A11yRole::Button, Some("OK"), None);
    assert_eq!(
        res,
        Err(ActionError::NotFound {
            target: buiy_core::a11y::translate::ROOT_NODE_ID,
        }),
        ">1 match is the strict-locator disambiguation error (never first-match)"
    );
}

#[test]
fn get_by_role_disambiguates_on_state() {
    let mut app = setup();
    // Two expandable nodes; only one is expanded. A StateQuery disambiguates.
    app.world_mut().spawn((
        A11yRole::Group,
        A11yLabel("Closed".into()),
        A11yExpanded(false),
    ));
    let open = app
        .world_mut()
        .spawn((
            A11yRole::Group,
            A11yLabel("Open".into()),
            A11yExpanded(true),
        ))
        .id();
    app.update();

    let q = StateQuery {
        expanded: Some(true),
        ..Default::default()
    };
    let res = get_by_role(app.world_mut(), A11yRole::Group, None, Some(&q));
    assert_eq!(
        res,
        Ok(node_id_for(open)),
        "the present-only StateQuery (expanded: true) selects the single open node"
    );
}

// ---------------------------------------------------------------------------
// click — act-then-observe through dispatch_action_request → OnPress
// ---------------------------------------------------------------------------

#[test]
fn click_button_fires_on_press_and_returns_post_action_tree() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel("Go".into()),
            Focusable::default(),
        ))
        .id();
    app.update();

    // Resolve by role+name (the addressing the driver advertises), then click.
    let target = get_by_role(app.world_mut(), A11yRole::Button, Some("Go"), None).unwrap();
    let tree = click(app.world_mut(), target);

    // The act half: a post-action SemanticTree comes back inline (Ok).
    let tree = tree.expect("click on a live Button is honored and returns the tree");
    let node = tree
        .node(target)
        .expect("the clicked button is in the post-action tree");
    assert_eq!(node.role, A11yRole::Button);
    assert_eq!(node.name, "Go");

    // The observe half: the SAME shared OnPress sink the pointer/keyboard paths
    // write fired for this entity (the act went through dispatch_action_request).
    assert_eq!(
        drain_on_press(&mut app),
        vec![btn],
        "click() lowers Action::Click → OnPress via dispatch_action_request"
    );
}

// ---------------------------------------------------------------------------
// focus — the returned tree shows the node focused
// ---------------------------------------------------------------------------

#[test]
fn focus_marks_the_node_focused_in_returned_tree() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel("Focus me".into()),
            Focusable::default(),
        ))
        .id();
    app.update();
    let target = node_id_for(btn);

    let tree = focus(app.world_mut(), target).expect("Focus is honored on any live node");
    let node = tree.node(target).expect("focused node present in tree");
    assert!(
        node.state.focused,
        "the post-focus snapshot reads the node back as focused (through the consumer)"
    );
    // And no OTHER node is reported focused.
    let other_focused = tree.nodes.iter().filter(|n| n.state.focused).count();
    assert_eq!(other_focused, 1, "exactly the focused node carries focus");
}

// ---------------------------------------------------------------------------
// perform — an unadvertised verb is a typed Err, never a panic
// ---------------------------------------------------------------------------

#[test]
fn increment_on_button_is_unsupported_result() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel("Btn".into()),
            Focusable::default(),
        ))
        .id();
    app.update();
    let target = node_id_for(btn);

    // A Button advertises {Click, Focus, Blur} — Increment is NOT in its
    // contract, so the driver surfaces Unsupported as a Result (no panic).
    let res = increment(app.world_mut(), target);
    assert_eq!(
        res,
        Err(ActionError::Unsupported {
            target,
            action: accesskit::Action::Increment,
        }),
        "increment on a Button surfaces Err(Unsupported), never panics"
    );

    // The generic `perform` path produces the same typed error.
    let res2 = perform(app.world_mut(), accesskit::Action::Increment, target, None);
    assert_eq!(res2, res, "perform() and the increment() sugar agree");
}

// ---------------------------------------------------------------------------
// snapshot — actions + present-only state are read back through the consumer
// ---------------------------------------------------------------------------

#[test]
fn snapshot_reports_advertised_actions_and_state() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel("Save".into()),
            Focusable::default(),
        ))
        .id();
    // A slider-like valued node: present-only numeric value surfaces.
    app.world_mut().spawn((
        A11yRole::Slider,
        A11yLabel("Volume".into()),
        A11yValue {
            now: 7.0,
            min: 0.0,
            max: 10.0,
            ..Default::default()
        },
    ));
    app.update();

    let tree = snapshot(app.world_mut(), TreeView::default());
    let button = tree.node(node_id_for(btn)).unwrap();
    assert!(
        button.actions.contains(&accesskit::Action::Click),
        "the Button advertises Click (read back through the consumer)"
    );
    assert!(
        button.actions.contains(&accesskit::Action::Focus),
        "a focusable node advertises Focus"
    );
    // Present-only: a Button is not a valued range.
    assert_eq!(button.state.numeric_value, None);

    let slider = tree.by_role(A11yRole::Slider).next().unwrap();
    assert_eq!(
        slider.state.numeric_value,
        Some(7.0),
        "the Slider's present-only numeric value round-trips through the consumer"
    );
}

// ---------------------------------------------------------------------------
// GATE #3 — Checkbox/Switch advertise role + {Click, Focus, Blur} + their
// A11yToggled state, read back through the in-process driver snapshot (Wave-3
// slice-1). The driver is the C7 a11y consumer tier: it sees exactly what a real
// AT would.
// ---------------------------------------------------------------------------

#[test]
fn checkbox_advertises_role_actions_and_tristate_toggled() {
    use accesskit::Toggled;
    use buiy_core::a11y::A11yToggled;

    let mut app = setup();
    // An indeterminate (Mixed) checkbox — the tri-state value a plain bool can't
    // carry, exercised through the consumer.
    let cb = app
        .world_mut()
        .spawn((
            A11yRole::Checkbox,
            A11yLabel("Subscribe".into()),
            A11yToggled(Toggled::Mixed),
            Focusable::default(),
        ))
        .id();
    app.update();

    let tree = snapshot(app.world_mut(), TreeView::default());
    let node = tree.node(node_id_for(cb)).expect("checkbox in tree");
    assert_eq!(node.role, A11yRole::Checkbox, "role is Checkbox");
    // {Click, Focus, Blur} — Click from the contract, Focus/Blur from Focusable.
    assert!(
        node.actions.contains(&accesskit::Action::Click),
        "Checkbox advertises Click"
    );
    assert!(
        node.actions.contains(&accesskit::Action::Focus),
        "Checkbox advertises Focus"
    );
    assert!(
        node.actions.contains(&accesskit::Action::Blur),
        "Checkbox advertises Blur"
    );
    // The tri-state toggle round-trips through the consumer — Mixed is preserved.
    assert_eq!(
        node.state.toggled,
        Some(Toggled::Mixed),
        "the checkbox's tri-state A11yToggled (Mixed) is read back through the consumer"
    );
}

#[test]
fn switch_advertises_role_actions_and_binary_toggled() {
    use accesskit::Toggled;
    use buiy_core::a11y::A11yToggled;

    let mut app = setup();
    let sw = app
        .world_mut()
        .spawn((
            A11yRole::Switch,
            A11yLabel("Wi-Fi".into()),
            A11yToggled(Toggled::True),
            Focusable::default(),
        ))
        .id();
    app.update();

    let tree = snapshot(app.world_mut(), TreeView::default());
    let node = tree.node(node_id_for(sw)).expect("switch in tree");
    assert_eq!(node.role, A11yRole::Switch, "role is Switch");
    assert!(
        node.actions.contains(&accesskit::Action::Click),
        "Switch advertises Click"
    );
    assert!(
        node.actions.contains(&accesskit::Action::Focus),
        "Switch advertises Focus"
    );
    assert_eq!(
        node.state.toggled,
        Some(Toggled::True),
        "the switch's binary A11yToggled (True) round-trips through the consumer"
    );
}

#[test]
fn slider_advertises_role_value_verbs_and_numeric_value() {
    // GATE #3 (slice-2): a Slider advertises role=Slider + {Increment, Decrement,
    // SetValue, Focus, Blur} + its A11yValue (now/min/max), read back through the
    // in-process driver snapshot.
    let mut app = setup();
    let sl = app
        .world_mut()
        .spawn((
            A11yRole::Slider,
            A11yLabel("Volume".into()),
            A11yValue {
                now: 30.0,
                min: 0.0,
                max: 100.0,
                step: Some(5.0),
                jump: None,
                text: None,
            },
            Focusable::default(),
        ))
        .id();
    app.update();

    let tree = snapshot(app.world_mut(), TreeView::default());
    let node = tree.node(node_id_for(sl)).expect("slider in tree");
    assert_eq!(node.role, A11yRole::Slider, "role is Slider");
    for action in [
        accesskit::Action::Increment,
        accesskit::Action::Decrement,
        accesskit::Action::SetValue,
        accesskit::Action::Focus,
        accesskit::Action::Blur,
    ] {
        assert!(
            node.actions.contains(&action),
            "Slider advertises {action:?}"
        );
    }
    // It does NOT advertise Click (it is value-changing, not activatable).
    assert!(
        !node.actions.contains(&accesskit::Action::Click),
        "Slider does not advertise Click"
    );
    // The current numeric value round-trips through the consumer.
    assert_eq!(
        node.state.numeric_value,
        Some(30.0),
        "the slider's A11yValue.now round-trips through the consumer"
    );
}

// ---------------------------------------------------------------------------
// Inspection-driver acceptance (Wave-3 slice-2): get_by_role(Slider) then driver
// increment raises `now` by step, observed in the SemanticTree after a frame
// settles the `A11yTreeBuilder` (the documented perform-then-update contract: the
// slider contract mutates `A11yValue` synchronously on the live component, and the
// next `app.update()` refreshes the builder the snapshot reads through). perform
// (SetValue, out-of-range) clamps the same way.
// ---------------------------------------------------------------------------

#[test]
fn driver_increment_on_slider_raises_now_by_step() {
    let mut app = setup();
    let sl = app
        .world_mut()
        .spawn((
            A11yRole::Slider,
            A11yLabel("Volume".into()),
            A11yValue {
                now: 30.0,
                min: 0.0,
                max: 100.0,
                step: Some(5.0),
                jump: None,
                text: None,
            },
            Focusable::default(),
        ))
        .id();
    app.update();

    // 1) Resolve by role+name (the strict-locator addressing the driver advertises).
    let target = get_by_role(app.world_mut(), A11yRole::Slider, Some("Volume"), None).unwrap();
    assert_eq!(target, node_id_for(sl));

    // 2) Driver increment: the slider contract mutates A11yValue synchronously on
    //    the live component (assert that directly), and a settled frame surfaces it
    //    in the SemanticTree the snapshot reads through the builder.
    increment(app.world_mut(), target).expect("increment honored");
    assert_eq!(
        app.world().get::<A11yValue>(sl).unwrap().now,
        35.0,
        "the slider contract raised the live A11yValue.now 30 → 35 synchronously"
    );
    app.update();
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        tree.node(target).unwrap().state.numeric_value,
        Some(35.0),
        "driver increment raised now 30 → 35 (by step) in the settled SemanticTree"
    );

    // 3) perform(SetValue, out-of-range) clamps to max.
    perform(
        app.world_mut(),
        accesskit::Action::SetValue,
        target,
        Some(accesskit::ActionData::NumericValue(999.0)),
    )
    .expect("SetValue honored");
    assert_eq!(
        app.world().get::<A11yValue>(sl).unwrap().now,
        100.0,
        "perform(SetValue, out-of-range) clamps the live value to max"
    );
    app.update();
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        tree.node(target).unwrap().state.numeric_value,
        Some(100.0),
        "the clamped value (max) surfaces in the settled SemanticTree"
    );
}

// ---------------------------------------------------------------------------
// Inspection-driver acceptance (Wave-3 slice-1): get_by_role then driver click,
// then a real driven frame, flips A11yToggled in the returned SemanticTree. The
// driver's `click` writes OnPress synchronously (no tick); a subsequent
// `app.update()` runs the `buiy_widgets` toggle consumer. Here we model that
// consumer inline (buiy_core has no widget dep) so the convergence — AT click →
// OnPress → toggle advance — is exercised end-to-end at the core tier.
// ---------------------------------------------------------------------------

#[test]
fn driver_click_then_update_flips_checkbox_toggled() {
    use accesskit::Toggled;
    use buiy_core::a11y::A11yToggled;

    let mut app = setup();
    // Model the single OnPress toggle consumer (the production one lives in
    // buiy_widgets; buiy_core can't depend on it, so reproduce its advance rule
    // here to prove the core convergence path end-to-end).
    fn advance_checkbox_on_press(
        mut reader: MessageReader<OnPress>,
        mut q: Query<&mut A11yToggled, With<CheckboxTag>>,
    ) {
        for OnPress(e) in reader.read() {
            if let Ok(mut t) = q.get_mut(*e) {
                t.advance_checkbox();
            }
        }
    }
    #[derive(Component)]
    struct CheckboxTag;
    app.add_systems(Update, advance_checkbox_on_press);

    let cb = app
        .world_mut()
        .spawn((
            A11yRole::Checkbox,
            A11yLabel("Done".into()),
            A11yToggled(Toggled::False),
            Focusable::default(),
            CheckboxTag,
        ))
        .id();
    app.update();

    // 1) Resolve by role+name (the addressing the driver advertises).
    let target = get_by_role(app.world_mut(), A11yRole::Checkbox, Some("Done"), None).unwrap();
    assert_eq!(target, node_id_for(cb));

    // 2) Driver click: writes OnPress synchronously; the returned tree is the
    //    pre-advance snapshot (the consumer hasn't run yet — documented contract).
    let pre = click(app.world_mut(), target).expect("click honored");
    assert_eq!(
        pre.node(target).unwrap().state.toggled,
        Some(Toggled::False),
        "the click's own snapshot is pre-advance (consumer runs on the next tick)"
    );

    // 3) Drive one frame so the consumer advances the toggle, then re-snapshot.
    app.update();
    let post = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        post.node(target).unwrap().state.toggled,
        Some(Toggled::True),
        "AT click → OnPress → consumer advanced the checkbox False→True"
    );
}

// ---------------------------------------------------------------------------
// GATE #3 / #7 / driver — Disclosure-trigger (Wave-3 slice-3). The trigger is
// `A11yRole::Button` + the state-keyed `A11yExpanded` capability: it advertises
// role=Button + {Click, Expand, Collapse, Focus, Blur}, with controls=[panel] and
// the panel a `Role::Region`. The driver exercises the absolute AT set-verbs
// (Expand → expanded true, Collapse → false, idempotent) and the Click→OnPress→
// toggle convergence, all over the in-process consumer (the C7 a11y tier).
//
// The bare contract surface (role=Button + A11yExpanded + Focusable + the
// controls relation + a Region panel) is spawned directly; the full widget bundle
// + the OnPress→expanded consumer + the C4 visual live in `buiy_widgets`. The
// toggle consumer is modelled inline here (buiy_core has no widget dep) to prove
// the core convergence path end-to-end.
// ---------------------------------------------------------------------------

/// Spawn a disclosure-trigger + its controlled Region panel, returning
/// `(trigger, panel)`. The trigger is `Role::Button` (so its `Click` rides the
/// Button contract) PLUS `A11yExpanded` (the state-keyed Expand/Collapse capability)
/// and `controls = [panel]`.
fn disclosure(app: &mut App, name: &str) -> (Entity, Entity) {
    let panel = app
        .world_mut()
        .spawn((A11yRole::Region, A11yLabel(format!("{name} panel"))))
        .id();
    let trigger = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yLabel(name.into()),
            A11yExpanded(false),
            Focusable::default(),
            buiy_core::a11y::A11yRelations {
                controls: vec![panel],
                ..Default::default()
            },
        ))
        .id();
    app.update();
    (trigger, panel)
}

#[test]
fn disclosure_advertises_button_with_expand_collapse_and_controls() {
    // GATE #3: the trigger node advertises role=Button + {Click, Expand, Collapse,
    // Focus, Blur} + A11yExpanded; controls=[panel]; the panel node is Role::Region.
    let mut app = setup();
    let (trigger, panel) = disclosure(&mut app, "Details");

    let tree = snapshot(app.world_mut(), TreeView::default());
    let node = tree.node(node_id_for(trigger)).expect("trigger in tree");
    assert_eq!(node.role, A11yRole::Button, "the trigger role is Button");
    for action in [
        accesskit::Action::Click,
        accesskit::Action::Expand,
        accesskit::Action::Collapse,
        accesskit::Action::Focus,
        accesskit::Action::Blur,
    ] {
        assert!(
            node.actions.contains(&action),
            "the disclosure trigger advertises {action:?}"
        );
    }
    assert_eq!(
        node.state.expanded,
        Some(false),
        "the trigger carries A11yExpanded (collapsed), read back through the consumer"
    );
    assert_eq!(
        node.controls,
        vec![node_id_for(panel)],
        "the trigger controls the panel"
    );
    // The panel is a Region.
    let panel_node = tree.node(node_id_for(panel)).expect("panel in tree");
    assert_eq!(
        panel_node.role,
        A11yRole::Region,
        "the controlled panel is a Region"
    );
}

#[test]
fn driver_expand_collapse_on_disclosure_sets_expanded_and_is_idempotent() {
    // GATE #7 / driver: get_by_role(Button, name) → perform(Expand) sets expanded
    // true; Collapse sets false; an Expand on an already-expanded disclosure is
    // idempotent (stays true, success). The router's generic Expand/Collapse honor
    // mutates the live `A11yExpanded` synchronously (like the slider's `A11yValue`);
    // the documented perform-then-update contract surfaces it in the SemanticTree
    // after a frame settles the `A11yTreeBuilder` the snapshot reads through.
    let mut app = setup();
    let (trigger, _panel) = disclosure(&mut app, "Details");

    let target = get_by_role(app.world_mut(), A11yRole::Button, Some("Details"), None).unwrap();
    assert_eq!(target, node_id_for(trigger));

    // Expand → expanded true (live component mutated synchronously; settled in the
    // tree after a tick).
    perform(app.world_mut(), accesskit::Action::Expand, target, None)
        .expect("Expand honored on an A11yExpanded node");
    assert!(
        app.world().get::<A11yExpanded>(trigger).unwrap().0,
        "perform(Expand) sets the live A11yExpanded true synchronously"
    );
    app.update();
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        tree.node(target).unwrap().state.expanded,
        Some(true),
        "the expanded state surfaces in the settled SemanticTree"
    );

    // Expand again is idempotent (absolute set-verb): stays true, still a success.
    perform(app.world_mut(), accesskit::Action::Expand, target, None)
        .expect("an idempotent Expand on an already-expanded disclosure succeeds");
    assert!(
        app.world().get::<A11yExpanded>(trigger).unwrap().0,
        "Expand on an already-expanded disclosure is an idempotent no-op (still true)"
    );

    // Collapse → expanded false.
    perform(app.world_mut(), accesskit::Action::Collapse, target, None).expect("Collapse honored");
    assert!(
        !app.world().get::<A11yExpanded>(trigger).unwrap().0,
        "perform(Collapse) sets A11yExpanded false synchronously"
    );
    app.update();
    let tree = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        tree.node(target).unwrap().state.expanded,
        Some(false),
        "the collapsed state surfaces in the settled SemanticTree"
    );
}

#[test]
fn driver_click_then_update_toggles_disclosure_expanded() {
    // The Click→OnPress→toggle convergence (the pointer/keyboard/AT-Click path):
    // an AT `Click` on the trigger lowers into OnPress (the Button contract), and
    // the OnPress consumer flips A11yExpanded. Modelled inline (buiy_core has no
    // widget dep) — the production consumer is `advance_expanded_on_press`.
    let mut app = setup();

    fn toggle_expanded_on_press(
        mut reader: MessageReader<OnPress>,
        mut q: Query<&mut A11yExpanded>,
    ) {
        for OnPress(e) in reader.read() {
            if let Ok(mut x) = q.get_mut(*e) {
                x.0 = !x.0;
            }
        }
    }
    app.add_systems(Update, toggle_expanded_on_press);

    let (trigger, _panel) = disclosure(&mut app, "Details");
    let target = node_id_for(trigger);

    // Click writes OnPress synchronously; the consumer runs on the next tick.
    click(app.world_mut(), target).expect("Click honored on the Button trigger");
    app.update();
    let post = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        post.node(target).unwrap().state.expanded,
        Some(true),
        "AT Click → OnPress → consumer flipped the disclosure collapsed→expanded"
    );

    // A second Click toggles back.
    click(app.world_mut(), target).expect("Click honored");
    app.update();
    let post = snapshot(app.world_mut(), TreeView::default());
    assert_eq!(
        post.node(target).unwrap().state.expanded,
        Some(false),
        "a second AT Click toggles the disclosure expanded→collapsed"
    );
}

// ---------------------------------------------------------------------------
// wait_for — block on a semantic condition (no sleeps)
// ---------------------------------------------------------------------------

/// A widget that flips its label to "Done" `N` frames after it is spawned, so a
/// `wait_for` over the SemanticTree has a condition that becomes true mid-loop.
#[derive(Component)]
struct FlipAfter(u32);

fn flip_label(mut q: Query<(&mut A11yLabel, &mut FlipAfter)>) {
    for (mut label, mut flip) in &mut q {
        if flip.0 == 0 {
            label.0 = "Done".into();
        } else {
            flip.0 -= 1;
        }
    }
}

#[test]
fn wait_for_succeeds_when_condition_becomes_true() {
    let mut app = setup();
    app.add_systems(Update, flip_label);
    app.world_mut()
        .spawn((A11yRole::Text, A11yLabel("Working".into()), FlipAfter(3)));
    app.update();

    // Condition: some node is named "Done". False now, true after 3 frames.
    let has_done = |t: &SemanticTree| t.nodes.iter().any(|n| n.name == "Done");
    let res = wait_for(&mut app, has_done, 10);
    assert!(
        res.is_ok(),
        "wait_for resolves once the label flips to Done within the budget"
    );
    assert!(
        has_done(&res.unwrap()),
        "the returned tree is the one satisfying the condition"
    );
}

#[test]
fn wait_for_times_out_when_condition_never_holds() {
    let mut app = setup();
    app.world_mut()
        .spawn((A11yRole::Text, A11yLabel("Never".into())));
    app.update();

    // A condition that can never be true (no node is ever named "Nope").
    let never = |t: &SemanticTree| t.nodes.iter().any(|n| n.name == "Nope");
    let res = wait_for(&mut app, never, 4);
    assert_eq!(
        res,
        Err(ActionError::NotActionable {
            target: buiy_core::a11y::translate::ROOT_NODE_ID,
            action: accesskit::Action::Focus,
            reason: NotActionableReason::Timeout,
        }),
        "a never-true condition exhausts the frame budget and times out"
    );
}
