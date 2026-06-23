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
