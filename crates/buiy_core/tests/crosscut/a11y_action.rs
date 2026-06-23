//! P1c-b — GATE #6: input replay through the **headless dispatch seam**
//! (`dispatch_action_request`) and the Button keyboard-activation path.
//!
//! These exercise the inbound action router end-to-end without a winit adapter:
//! an `accesskit::ActionRequest` minted in-test resolves to a live entity, runs
//! the liveness + capability + live-state filter (action-router.md §3), and
//! lowers into a real Buiy sink (`OnPress` / `FocusedEntity`). The keyboard path
//! drives the production `keyboard_activation` system (the per-role APG keymap)
//! through the real schedule.

use accesskit::{Action, ActionData, ActionRequest, TreeId};
use bevy::input::ButtonState;
use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::{
    CorePlugin,
    a11y::{
        A11yDisabled, A11yExpanded, A11yPlugin, A11yRole, A11yValue, ActionError,
        NotActionableReason, dispatch_action_request,
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
// schedule (the production `keyboard_activation` system).
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

// ---------------------------------------------------------------------------
// GATE #7 — the APG keyboard ASYMMETRY (Wave-3 slice-1, widget-contracts.md §5).
// The keyboard producer (`keyboard_activation`) writes `OnPress` on the role's
// APG activation keys. A **Checkbox toggles on Space ONLY** (Enter does NOTHING
// — the canonical asymmetry vs Button); a **Switch toggles on BOTH** Space and
// Enter. These tests assert the `OnPress`-emission half of gate #7 at the
// keyboard layer (the `A11yToggled` flip the OnPress drives is asserted in the
// buiy_widgets end-to-end tests, where the toggle consumer lives).
// ---------------------------------------------------------------------------

#[test]
fn space_on_focused_checkbox_fires_on_press() {
    let mut app = setup();
    let cb = app
        .world_mut()
        .spawn((A11yRole::Checkbox, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(cb);

    key_down(&mut app, KeyCode::Space);
    assert_eq!(
        drain_on_press(&mut app),
        vec![cb],
        "Space on a focused Checkbox fires OnPress (APG checkbox)"
    );
}

#[test]
fn enter_on_focused_checkbox_does_nothing() {
    // THE canonical asymmetry: a checkbox does NOT toggle on Enter (Button does).
    let mut app = setup();
    let cb = app
        .world_mut()
        .spawn((A11yRole::Checkbox, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(cb);

    key_down(&mut app, KeyCode::Enter);
    assert!(
        drain_on_press(&mut app).is_empty(),
        "Enter on a focused Checkbox does NOTHING — the canonical APG asymmetry \
         (Space-only; Enter must not toggle a checkbox)"
    );
}

#[test]
fn space_on_focused_switch_fires_on_press() {
    let mut app = setup();
    let sw = app
        .world_mut()
        .spawn((A11yRole::Switch, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(sw);

    key_down(&mut app, KeyCode::Space);
    assert_eq!(
        drain_on_press(&mut app),
        vec![sw],
        "Space on a focused Switch fires OnPress (APG switch)"
    );
}

#[test]
fn enter_on_focused_switch_fires_on_press() {
    // A switch — unlike a checkbox — toggles on Enter too (Space AND Enter).
    let mut app = setup();
    let sw = app
        .world_mut()
        .spawn((A11yRole::Switch, buiy_core::focus::Focusable::default()))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(sw);

    key_down(&mut app, KeyCode::Enter);
    assert_eq!(
        drain_on_press(&mut app),
        vec![sw],
        "Enter on a focused Switch fires OnPress (APG switch: Space AND Enter)"
    );
}

// ---------------------------------------------------------------------------
// The contract dispatch half (gate #6/#7): an inbound `Action::Click` on a
// Checkbox/Switch lowers into the shared `OnPress` sink (the same convergence as
// Button), and an unadvertised verb is rejected.
// ---------------------------------------------------------------------------

#[test]
fn dispatch_click_on_checkbox_fires_on_press() {
    let mut app = setup();
    let cb = app
        .world_mut()
        .spawn((A11yRole::Checkbox, buiy_core::focus::Focusable::default()))
        .id();
    app.update();

    let req = request(node_id_for(cb), Action::Click);
    assert_eq!(
        dispatch_action_request(app.world_mut(), &req),
        Ok(()),
        "Click on a live Checkbox is honored"
    );
    assert_eq!(
        drain_on_press(&mut app),
        vec![cb],
        "Checkbox honor(Click) writes the shared OnPress sink"
    );
}

#[test]
fn dispatch_click_on_switch_fires_on_press() {
    let mut app = setup();
    let sw = app
        .world_mut()
        .spawn((A11yRole::Switch, buiy_core::focus::Focusable::default()))
        .id();
    app.update();

    let req = request(node_id_for(sw), Action::Click);
    assert_eq!(
        dispatch_action_request(app.world_mut(), &req),
        Ok(()),
        "Click on a live Switch is honored"
    );
    assert_eq!(
        drain_on_press(&mut app),
        vec![sw],
        "Switch honor(Click) writes the shared OnPress sink"
    );
}

#[test]
fn dispatch_increment_on_checkbox_is_unsupported() {
    let mut app = setup();
    let cb = app.world_mut().spawn(A11yRole::Checkbox).id();
    app.update();

    let req = request(node_id_for(cb), Action::Increment);
    assert_eq!(
        dispatch_action_request(app.world_mut(), &req),
        Err(ActionError::Unsupported {
            target: node_id_for(cb),
            action: Action::Increment,
        }),
        "a Checkbox advertises only {{Click, Focus, Blur}} — Increment is Unsupported"
    );
}

// ---------------------------------------------------------------------------
// GATE #3 / #7 — the SLIDER value contract + APG slider keyboard (slice-2,
// widget-contracts.md §5). The Slider's verbs change its VALUE (mutate the live
// `A11yValue`), NOT activation (`OnPress`); arrows/Home/End/PageUp/PageDown on a
// focused Slider dispatch those value verbs through the SAME router seam an AT
// drives. These pin both the headless dispatch (Increment/Decrement/SetValue +
// clamping) and the keyboard keymap, and prove the activation keymap stays inert
// for a Slider.
// ---------------------------------------------------------------------------

/// Spawn a focusable Slider over `[min, max]` at `now` stepping by `step`, and
/// focus it. Returns the entity. The bare `A11yRole::Slider` + `A11yValue` +
/// `Focusable` is the contract surface the router/keymap read (the full widget
/// bundle lives in `buiy_widgets`; here we pin the core a11y behavior directly).
fn focused_slider(app: &mut App, now: f64, min: f64, max: f64, step: f64) -> Entity {
    let sl = app
        .world_mut()
        .spawn((
            A11yRole::Slider,
            A11yValue {
                now,
                min,
                max,
                step: Some(step),
                jump: None,
                text: None,
            },
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(sl);
    sl
}

fn now_of(app: &App, sl: Entity) -> f64 {
    app.world().get::<A11yValue>(sl).unwrap().now
}

// --- The headless dispatch seam: value verbs mutate A11yValue (gate #3). ---

#[test]
fn dispatch_increment_on_slider_steps_and_clamps_at_max() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 9.0, 0.0, 10.0, 1.0);

    dispatch_action_request(
        app.world_mut(),
        &request(node_id_for(sl), Action::Increment),
    )
    .unwrap();
    assert_eq!(now_of(&app, sl), 10.0, "Increment steps now 9 → 10");

    // At-max Increment is a clamped no-op (saturated success, not an error).
    dispatch_action_request(
        app.world_mut(),
        &request(node_id_for(sl), Action::Increment),
    )
    .unwrap();
    assert_eq!(
        now_of(&app, sl),
        10.0,
        "Increment at max clamps (no-op success)"
    );
}

#[test]
fn dispatch_decrement_on_slider_steps_and_clamps_at_min() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 1.0, 0.0, 10.0, 1.0);

    dispatch_action_request(
        app.world_mut(),
        &request(node_id_for(sl), Action::Decrement),
    )
    .unwrap();
    assert_eq!(now_of(&app, sl), 0.0, "Decrement steps now 1 → 0");

    dispatch_action_request(
        app.world_mut(),
        &request(node_id_for(sl), Action::Decrement),
    )
    .unwrap();
    assert_eq!(
        now_of(&app, sl),
        0.0,
        "Decrement at min clamps (no-op success)"
    );
}

#[test]
fn dispatch_set_value_on_slider_clamps_out_of_range() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 5.0, 0.0, 10.0, 1.0);

    let req = ActionRequest {
        action: Action::SetValue,
        target_tree: TreeId::ROOT,
        target_node: node_id_for(sl),
        data: Some(ActionData::NumericValue(7.5)),
    };
    dispatch_action_request(app.world_mut(), &req).unwrap();
    assert_eq!(now_of(&app, sl), 7.5, "SetValue sets an in-range value");

    // Out-of-range SetValue saturates at the bound (clamps, never errors).
    let req = ActionRequest {
        action: Action::SetValue,
        target_tree: TreeId::ROOT,
        target_node: node_id_for(sl),
        data: Some(ActionData::NumericValue(999.0)),
    };
    dispatch_action_request(app.world_mut(), &req).unwrap();
    assert_eq!(
        now_of(&app, sl),
        10.0,
        "SetValue out-of-range clamps to max"
    );
}

#[test]
fn dispatch_set_value_on_slider_without_numeric_data_is_bad_data() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 5.0, 0.0, 10.0, 1.0);

    // SetValue with no payload (the wrong/missing variant) is BadData, not a panic.
    let req = request(node_id_for(sl), Action::SetValue);
    assert_eq!(
        dispatch_action_request(app.world_mut(), &req),
        Err(ActionError::BadData {
            target: node_id_for(sl),
            action: Action::SetValue,
        }),
        "SetValue without a NumericValue payload is BadData"
    );
    assert_eq!(now_of(&app, sl), 5.0, "the value is unchanged on BadData");
}

#[test]
fn dispatch_click_on_slider_is_unsupported() {
    // A Slider advertises value verbs only — Click is NOT in its contract.
    let mut app = setup();
    let sl = app.world_mut().spawn(A11yRole::Slider).id();
    app.update();

    assert_eq!(
        dispatch_action_request(app.world_mut(), &request(node_id_for(sl), Action::Click)),
        Err(ActionError::Unsupported {
            target: node_id_for(sl),
            action: Action::Click,
        }),
        "a Slider does not advertise Click — Unsupported"
    );
}

// --- The APG slider keyboard: arrows/Home/End/PageUp/PageDown → value (gate #7). ---

#[test]
fn arrow_up_right_increment_a_focused_slider() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 4.0, 0.0, 10.0, 1.0);

    key_down(&mut app, KeyCode::ArrowUp);
    assert_eq!(now_of(&app, sl), 5.0, "ArrowUp increments by step");
    key_down(&mut app, KeyCode::ArrowRight);
    assert_eq!(now_of(&app, sl), 6.0, "ArrowRight increments by step");
}

#[test]
fn arrow_down_left_decrement_a_focused_slider() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 6.0, 0.0, 10.0, 1.0);

    key_down(&mut app, KeyCode::ArrowDown);
    assert_eq!(now_of(&app, sl), 5.0, "ArrowDown decrements by step");
    key_down(&mut app, KeyCode::ArrowLeft);
    assert_eq!(now_of(&app, sl), 4.0, "ArrowLeft decrements by step");
}

#[test]
fn arrow_keys_clamp_a_focused_slider_at_bounds() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 10.0, 0.0, 10.0, 1.0);
    key_down(&mut app, KeyCode::ArrowUp);
    assert_eq!(now_of(&app, sl), 10.0, "ArrowUp at max clamps");

    app.world_mut().get_mut::<A11yValue>(sl).unwrap().now = 0.0;
    app.update();
    key_down(&mut app, KeyCode::ArrowDown);
    assert_eq!(now_of(&app, sl), 0.0, "ArrowDown at min clamps");
}

#[test]
fn home_end_jump_a_focused_slider_to_min_and_max() {
    let mut app = setup();
    let sl = focused_slider(&mut app, 5.0, 0.0, 10.0, 1.0);

    key_down(&mut app, KeyCode::End);
    assert_eq!(now_of(&app, sl), 10.0, "End jumps to max");
    key_down(&mut app, KeyCode::Home);
    assert_eq!(now_of(&app, sl), 0.0, "Home jumps to min");
}

#[test]
fn page_up_down_use_the_jump_step() {
    let mut app = setup();
    // A slider with an explicit page `jump` of 10 (step 1).
    let sl = app
        .world_mut()
        .spawn((
            A11yRole::Slider,
            A11yValue {
                now: 50.0,
                min: 0.0,
                max: 100.0,
                step: Some(1.0),
                jump: Some(10.0),
                text: None,
            },
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(sl);

    key_down(&mut app, KeyCode::PageUp);
    assert_eq!(now_of(&app, sl), 60.0, "PageUp adds the jump (10)");
    key_down(&mut app, KeyCode::PageDown);
    assert_eq!(now_of(&app, sl), 50.0, "PageDown subtracts the jump (10)");
}

#[test]
fn arrow_on_focused_slider_writes_no_on_press() {
    // A Slider's keys change its VALUE, NOT activation — no `OnPress` is written
    // (the activation keymap is inert for Slider).
    let mut app = setup();
    let _sl = focused_slider(&mut app, 5.0, 0.0, 10.0, 1.0);

    key_down(&mut app, KeyCode::ArrowUp);
    assert!(
        drain_on_press(&mut app).is_empty(),
        "a slider arrow key writes no OnPress (value action, not activation)"
    );
}

// ---------------------------------------------------------------------------
// GATE #3 / #7 — the DISCLOSURE state-keyed Expand/Collapse capability (slice-3,
// widget-contracts.md §5). A disclosure-trigger is `A11yRole::Button` (so its
// `Click` rides the Button contract → OnPress) PLUS `A11yExpanded`, which advertises
// + honors `{Expand, Collapse}` GENERICALLY (the router sets the bool — not the role
// contract). These pin: Expand sets expanded true, Collapse false, both idempotent;
// a disabled disclosure drops them; an Expand on a non-A11yExpanded node is
// Unsupported (the capability is state-keyed, not role-keyed); and Click still
// lowers into OnPress on the Button trigger.
// ---------------------------------------------------------------------------

/// Spawn a disclosure-trigger: `Role::Button` + `A11yExpanded(collapsed)` +
/// `Focusable`. The state-keyed capability is what makes Expand/Collapse honored.
fn disclosure_trigger(app: &mut App, expanded: bool) -> Entity {
    let e = app
        .world_mut()
        .spawn((
            A11yRole::Button,
            A11yExpanded(expanded),
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    app.update();
    e
}

fn expanded_of(app: &App, e: Entity) -> bool {
    app.world().get::<A11yExpanded>(e).unwrap().0
}

#[test]
fn dispatch_expand_on_disclosure_sets_expanded_true() {
    let mut app = setup();
    let d = disclosure_trigger(&mut app, false);

    dispatch_action_request(app.world_mut(), &request(node_id_for(d), Action::Expand)).unwrap();
    assert!(
        expanded_of(&app, d),
        "Expand sets A11yExpanded true (generic honor over the state-keyed capability)"
    );

    // Idempotent: an Expand on an already-expanded disclosure is a no-op success.
    dispatch_action_request(app.world_mut(), &request(node_id_for(d), Action::Expand)).unwrap();
    assert!(
        expanded_of(&app, d),
        "Expand on an expanded disclosure stays true (idempotent)"
    );
}

#[test]
fn dispatch_collapse_on_disclosure_sets_expanded_false() {
    let mut app = setup();
    let d = disclosure_trigger(&mut app, true);

    dispatch_action_request(app.world_mut(), &request(node_id_for(d), Action::Collapse)).unwrap();
    assert!(!expanded_of(&app, d), "Collapse sets A11yExpanded false");

    // Idempotent the other way.
    dispatch_action_request(app.world_mut(), &request(node_id_for(d), Action::Collapse)).unwrap();
    assert!(
        !expanded_of(&app, d),
        "Collapse on a collapsed disclosure stays false (idempotent)"
    );
}

#[test]
fn dispatch_expand_on_disabled_disclosure_is_not_actionable() {
    // Expand/Collapse are ACTIONABLE verbs (unlike Focus/Blur), so the disabled
    // live filter drops them.
    let mut app = setup();
    let d = app
        .world_mut()
        .spawn((A11yRole::Button, A11yExpanded(false), A11yDisabled))
        .id();
    app.update();

    let res = dispatch_action_request(app.world_mut(), &request(node_id_for(d), Action::Expand));
    assert_eq!(
        res,
        Err(ActionError::NotActionable {
            target: node_id_for(d),
            action: Action::Expand,
            reason: NotActionableReason::Disabled,
        }),
        "A11yDisabled drops the actionable Expand verb"
    );
    assert!(!expanded_of(&app, d), "a disabled disclosure never expands");
}

#[test]
fn dispatch_expand_on_non_expandable_is_unsupported() {
    // A plain Button (no A11yExpanded) does NOT advertise Expand — the capability
    // is state-keyed, so without the state it is Unsupported.
    let mut app = setup();
    let btn = app
        .world_mut()
        .spawn((A11yRole::Button, buiy_core::focus::Focusable::default()))
        .id();
    app.update();

    assert_eq!(
        dispatch_action_request(app.world_mut(), &request(node_id_for(btn), Action::Expand)),
        Err(ActionError::Unsupported {
            target: node_id_for(btn),
            action: Action::Expand,
        }),
        "Expand on a node without A11yExpanded is Unsupported (state-keyed capability)"
    );
}

#[test]
fn dispatch_click_on_disclosure_fires_on_press() {
    // The trigger is a Button, so its Click still lowers into the shared OnPress
    // sink (the toggle path the OnPress consumer flips A11yExpanded on).
    let mut app = setup();
    let d = disclosure_trigger(&mut app, false);

    dispatch_action_request(app.world_mut(), &request(node_id_for(d), Action::Click)).unwrap();
    assert_eq!(
        drain_on_press(&mut app),
        vec![d],
        "the disclosure trigger's Click rides the Button contract → OnPress"
    );
}

#[test]
fn space_enter_on_focused_slider_do_nothing() {
    // The activation keys (Space/Enter) are inert for a Slider — it is not in the
    // activation keymap, so neither the value nor `OnPress` changes.
    let mut app = setup();
    let sl = focused_slider(&mut app, 5.0, 0.0, 10.0, 1.0);

    key_down(&mut app, KeyCode::Space);
    key_down(&mut app, KeyCode::Enter);
    assert!(
        drain_on_press(&mut app).is_empty(),
        "Space/Enter on a Slider write no OnPress"
    );
    assert_eq!(
        now_of(&app, sl),
        5.0,
        "Space/Enter leave the slider value unchanged"
    );
}

// ---------------------------------------------------------------------------
// GATE #3 — the TEXT-INPUT value contract (slice-4, widget-contracts.md §5). The
// role split (`TextInput` single-line vs `MultilineTextInput`) shares ONE contract
// surface: `{SetValue}` lowered through the EXISTING `SelectAll`+`Insert` editor
// channel (NO new `EditCommand`). These pin the headless dispatch directly at the
// `buiy_core` layer: `SetValue` replaces the whole editor value (both roles),
// missing/wrong payload is `BadData`, an un-advertised verb is `Unsupported`, and
// a read-only field drops the mutating verb. The editor mechanism needs the
// `SharedFontSystem`, so these use a `BuiyTextPlugin` harness.
// ---------------------------------------------------------------------------

use buiy_core::a11y::A11yReadOnly;
use buiy_core::a11y::contract::{A11yContract, MultilineTextInputContract, TextInputContract};
use buiy_core::text::edit::{SingleLine, TextEditState};

/// A headless app with the a11y + focus surface PLUS `BuiyTextPlugin` (the
/// `SharedFontSystem` the text `honor` locks to apply an edit). The text honor
/// lowers synchronously into `TextEditState`, so no schedule tick is needed to
/// observe the editor value — but the a11y `A11yTextValue` sync lives in
/// `buiy_widgets`, so these core tests assert the EDITOR value (the source of
/// truth); the `A11yTextValue` reflection is asserted at the widget layer.
fn text_setup() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_message::<KeyboardInput>();
    app
}

/// Spawn a single-line text input contract surface: `Role::TextInput` +
/// `TextEditState` + `SingleLine` + `Focusable` (the bundle lives in
/// `buiy_widgets`; here we pin the core contract behavior directly).
fn single_line_input(app: &mut App) -> Entity {
    let e = app
        .world_mut()
        .spawn((
            A11yRole::TextInput,
            TextEditState::for_font_size(16.0),
            SingleLine,
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    app.update();
    e
}

fn editor_value(app: &App, e: Entity) -> String {
    app.world().get::<TextEditState>(e).unwrap().value()
}

/// A `SetValue` carrying a string, addressed at `node`.
fn set_value_request(node: accesskit::NodeId, text: &str) -> ActionRequest {
    ActionRequest {
        action: Action::SetValue,
        target_tree: TreeId::ROOT,
        target_node: node,
        data: Some(ActionData::Value(text.into())),
    }
}

#[test]
fn text_input_contract_advertises_only_set_value() {
    // Both text roles advertise `{SetValue}` beyond the implicit `{Focus, Blur}`;
    // the selection verbs are deferred (co-drive §3.2), so they are NOT advertised.
    assert_eq!(TextInputContract::actions(), &[Action::SetValue]);
    assert_eq!(MultilineTextInputContract::actions(), &[Action::SetValue]);
    assert_eq!(TextInputContract::role(), A11yRole::TextInput);
    assert_eq!(
        MultilineTextInputContract::role(),
        A11yRole::MultilineTextInput
    );
}

#[test]
fn dispatch_set_value_on_text_input_replaces_the_editor_value() {
    let mut app = text_setup();
    let e = single_line_input(&mut app);

    dispatch_action_request(app.world_mut(), &set_value_request(node_id_for(e), "hello")).unwrap();
    assert_eq!(
        editor_value(&app, e),
        "hello",
        "SetValue lowers through SelectAll+Insert → editor value becomes the text"
    );

    // A second SetValue REPLACES (SelectAll selects the prior content first).
    dispatch_action_request(app.world_mut(), &set_value_request(node_id_for(e), "world")).unwrap();
    assert_eq!(
        editor_value(&app, e),
        "world",
        "a second SetValue replaces the whole value (SelectAll first, not append)"
    );

    // An empty target value is a clean clear.
    dispatch_action_request(app.world_mut(), &set_value_request(node_id_for(e), "")).unwrap();
    assert_eq!(editor_value(&app, e), "", "SetValue(\"\") clears the field");
}

#[test]
fn dispatch_set_value_on_multiline_keeps_newlines_single_line_strips() {
    let mut app = text_setup();
    // Single-line: the SingleLine policy strips the embedded newline.
    let single = single_line_input(&mut app);
    dispatch_action_request(
        app.world_mut(),
        &set_value_request(node_id_for(single), "a\nb"),
    )
    .unwrap();
    assert_eq!(
        editor_value(&app, single),
        "ab",
        "a single-line TextInput strips the embedded newline (SingleLine policy)"
    );

    // Multi-line (NO SingleLine marker, Role::MultilineTextInput): newline kept.
    let multi = app
        .world_mut()
        .spawn((
            A11yRole::MultilineTextInput,
            TextEditState::for_font_size(16.0),
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    app.update();
    dispatch_action_request(
        app.world_mut(),
        &set_value_request(node_id_for(multi), "a\nb"),
    )
    .unwrap();
    assert_eq!(
        editor_value(&app, multi),
        "a\nb",
        "a MultilineTextInput keeps the embedded newline (no SingleLine policy)"
    );
}

#[test]
fn dispatch_set_value_on_text_input_without_value_data_is_bad_data() {
    let mut app = text_setup();
    let e = single_line_input(&mut app);

    // SetValue with no payload (or the wrong variant) is BadData, not a panic.
    let req = request(node_id_for(e), Action::SetValue);
    assert_eq!(
        dispatch_action_request(app.world_mut(), &req),
        Err(ActionError::BadData {
            target: node_id_for(e),
            action: Action::SetValue,
        }),
        "SetValue without a Value payload is BadData"
    );
    assert_eq!(
        editor_value(&app, e),
        "",
        "the editor value is unchanged on BadData"
    );
}

#[test]
fn dispatch_click_on_text_input_is_unsupported() {
    // A text input advertises `{SetValue}` only — Click is NOT in its contract.
    let mut app = text_setup();
    let e = single_line_input(&mut app);

    assert_eq!(
        dispatch_action_request(app.world_mut(), &request(node_id_for(e), Action::Click)),
        Err(ActionError::Unsupported {
            target: node_id_for(e),
            action: Action::Click,
        }),
        "a TextInput does not advertise Click — Unsupported"
    );
}

#[test]
fn dispatch_set_value_on_read_only_text_input_is_not_actionable() {
    // `A11yReadOnly` drops the mutating SetValue verb at the §3 live filter, before
    // `honor` is reached — the editor value never changes.
    let mut app = text_setup();
    let e = app
        .world_mut()
        .spawn((
            A11yRole::TextInput,
            TextEditState::for_font_size(16.0),
            SingleLine,
            A11yReadOnly,
            buiy_core::focus::Focusable::default(),
        ))
        .id();
    app.update();

    let res = dispatch_action_request(app.world_mut(), &set_value_request(node_id_for(e), "nope"));
    assert_eq!(
        res,
        Err(ActionError::NotActionable {
            target: node_id_for(e),
            action: Action::SetValue,
            reason: NotActionableReason::ReadOnly,
        }),
        "A11yReadOnly drops the mutating SetValue verb (live filter, before honor)"
    );
    assert_eq!(
        editor_value(&app, e),
        "",
        "a read-only field is never mutated"
    );
}
