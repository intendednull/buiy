//! P1c-b — GATE #6: input replay through the **headless dispatch seam**
//! (`dispatch_action_request`) and the Button keyboard-activation path.
//!
//! These exercise the inbound action router end-to-end without a winit adapter:
//! an `accesskit::ActionRequest` minted in-test resolves to a live entity, runs
//! the liveness + capability + live-state filter (action-router.md §3), and
//! lowers into a real Buiy sink (`OnPress` / `FocusedEntity`). The keyboard path
//! drives the production `button_keyboard_activation` system through the real
//! schedule.

use accesskit::{Action, ActionRequest, TreeId};
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::{
    CorePlugin,
    a11y::{
        A11yDisabled, A11yPlugin, A11yRole, ActionError, NotActionableReason,
        dispatch_action_request,
    },
    focus::{FocusPlugin, FocusVisible, FocusedEntity},
    interaction::OnPress,
};

/// A minimal headless app with the a11y + focus + interaction surface (the
/// `OnPress` sink rides `CorePlugin`/`InteractionPlugin`).
fn setup() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    // The router's keyboard sibling reads `KeyboardInput` (a Message);
    // MinimalPlugins doesn't register it, so do it manually for the replay.
    app.add_message::<KeyboardInput>();
    app
}

/// Mint an inbound request targeting `node` with `action` and no data.
fn request(node: accesskit::NodeId, action: Action) -> ActionRequest {
    ActionRequest {
        action,
        target_tree: TreeId::ROOT,
        target_node: node,
        data: None,
    }
}

/// Drain `Messages<OnPress>` into a Vec of activated entities (advances the
/// internal cursor via a one-shot reader system).
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
// Headless seam: dispatch_action_request
// ---------------------------------------------------------------------------

#[test]
fn dispatch_click_on_button_fires_on_press() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    app.update();

    let req = request(node_id_for(btn), Action::Click);
    let res = dispatch_action_request(app.world_mut(), &req);
    assert_eq!(res, Ok(()), "Click on a live Button must be honored");

    let fired = drain_on_press(&mut app);
    assert_eq!(
        fired,
        vec![btn],
        "honor(Click) writes the shared OnPress sink for the button"
    );
}

#[test]
fn dispatch_click_on_disabled_button_is_not_actionable() {
    let mut app = setup();
    let btn = app.world_mut().spawn((A11yRole::Button, A11yDisabled)).id();
    app.update();

    let req = request(node_id_for(btn), Action::Click);
    let res = dispatch_action_request(app.world_mut(), &req);
    assert_eq!(
        res,
        Err(ActionError::NotActionable {
            target: node_id_for(btn),
            action: Action::Click,
            reason: NotActionableReason::Disabled,
        }),
        "A11yDisabled drops the actionable Click verb"
    );

    let fired = drain_on_press(&mut app);
    assert!(fired.is_empty(), "a disabled button never fires OnPress");
}

#[test]
fn dispatch_unadvertised_action_is_unsupported() {
    let mut app = setup();
    // A Button advertises {Click, Focus, Blur} only — Increment is NOT in its
    // contract, so the router rejects it at the capability gate.
    let btn = app.world_mut().spawn(A11yRole::Button).id();
    app.update();

    let req = request(node_id_for(btn), Action::Increment);
    let res = dispatch_action_request(app.world_mut(), &req);
    assert_eq!(
        res,
        Err(ActionError::Unsupported {
            target: node_id_for(btn),
            action: Action::Increment,
        }),
        "an unadvertised verb is Unsupported"
    );
}

#[test]
fn dispatch_to_dead_node_is_not_found() {
    let mut app = setup();
    // A NodeId that resolves to a never-spawned entity.
    let ghost = node_id_for(Entity::from_raw_u32(9999).unwrap());
    let req = request(ghost, Action::Click);
    let res = dispatch_action_request(app.world_mut(), &req);
    assert_eq!(
        res,
        Err(ActionError::NotFound { target: ghost }),
        "a NodeId with no live entity is NotFound (stale ref)"
    );
}

#[test]
fn dispatch_focus_sets_focused_entity() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    app.update();

    let req = request(node_id_for(btn), Action::Focus);
    let res = dispatch_action_request(app.world_mut(), &req);
    assert_eq!(res, Ok(()), "Focus is honored generically on any live node");
    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(btn),
        "Action::Focus sets FocusedEntity"
    );
    assert!(
        app.world().resource::<FocusVisible>().0,
        "programmatic Focus is treated as focus-visible (keyboard convention)"
    );
}

#[test]
fn dispatch_blur_clears_focused_entity_only_when_focused() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    let other = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(btn);

    // Blurring a NON-focused node must not steal focus from `btn`.
    dispatch_action_request(app.world_mut(), &request(node_id_for(other), Action::Blur)).unwrap();
    assert_eq!(app.world().resource::<FocusedEntity>().0, Some(btn));

    // Blurring the focused node clears it.
    dispatch_action_request(app.world_mut(), &request(node_id_for(btn), Action::Blur)).unwrap();
    assert_eq!(app.world().resource::<FocusedEntity>().0, None);
}

// ---------------------------------------------------------------------------
// Button keyboard activation (Enter + Space → OnPress), through the real
// schedule (the production `button_keyboard_activation` system).
// ---------------------------------------------------------------------------

/// Send one KeyDown of `key` and run the schedule.
fn key_down(app: &mut App, key: KeyCode) {
    app.world_mut().write_message(KeyboardInput {
        key_code: key,
        logical_key: bevy::input::keyboard::Key::Character("x".into()),
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    app.update();
}

#[test]
fn enter_on_focused_button_fires_on_press() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(btn);

    key_down(&mut app, KeyCode::Enter);
    assert_eq!(
        drain_on_press(&mut app),
        vec![btn],
        "Enter on a focused Button fires OnPress (APG)"
    );
}

#[test]
fn space_on_focused_button_fires_on_press() {
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(btn);

    key_down(&mut app, KeyCode::Space);
    assert_eq!(
        drain_on_press(&mut app),
        vec![btn],
        "Space on a focused Button fires OnPress (APG)"
    );
}

#[test]
fn key_on_focused_non_button_does_not_fire_on_press() {
    let mut app = setup();
    // A focused non-Button (Generic) must NOT activate on Enter/Space.
    let generic = app
        .world_mut()
        .spawn((A11yRole::Generic, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(generic);

    key_down(&mut app, KeyCode::Enter);
    assert!(
        drain_on_press(&mut app).is_empty(),
        "a key on a focused non-Button writes no OnPress"
    );
}

// ---------------------------------------------------------------------------
// The router SYSTEM drains the existing ActionRequestWrapper channel (no
// competing AccessKit handler) and dispatches through the same seam.
// ---------------------------------------------------------------------------

#[test]
fn route_action_requests_drains_the_wrapper_channel() {
    use bevy::a11y::ActionRequest as ActionRequestWrapper;

    let mut app = setup();
    // bevy_winit's `AccessibilityPlugin` registers this Message in a real app;
    // MinimalPlugins doesn't, so register it manually to exercise the drain.
    app.add_message::<ActionRequestWrapper>();
    let btn = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    app.update();

    // Mint the SAME wrapper bevy_winit's `poll_receivers` writes onto the
    // channel; the production `route_action_requests` system (scheduled in
    // BuiySet::Input by A11yPlugin) drains it on the next update.
    app.world_mut().write_message(ActionRequestWrapper(request(
        node_id_for(btn),
        Action::Click,
    )));
    app.update();

    assert_eq!(
        drain_on_press(&mut app),
        vec![btn],
        "route_action_requests drains the channel and lowers Click → OnPress"
    );
}

#[test]
fn enter_with_nothing_focused_does_not_fire_on_press() {
    let mut app = setup();
    app.world_mut().spawn(A11yRole::Button);
    app.update();
    // No FocusedEntity set.
    key_down(&mut app, KeyCode::Enter);
    assert!(
        drain_on_press(&mut app).is_empty(),
        "no focus ⇒ no keyboard activation"
    );
}
