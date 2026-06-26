//! Headless **router behavior** gate (parity Wave C1). Proves the screen-switch
//! mechanism (Candidate A: spawn-all-once + `Display::None`/`A11yHidden` toggle)
//! actually works: after mounting the shell, exactly one screen root is laid out
//! (the default Todo) and the other four are `Display::None`; writing a
//! `SwitchScreen` + running the applier flips which root is `Display::None` and
//! updates the `ScreenRouter` resource. This is the functional guard the layout
//! snapshot (a single static frame) cannot express.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::scene::ScenePlugin;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::a11y::A11yHidden;
use buiy_core::layout::Display;
use buiy_core::theme::default_dark_theme;
use buiy_gallery::shell::{
    Screen, ScreenRoot, ScreenRouter, ScreenRouterPlugin, SwitchScreen, build_shell,
    mount_screens_with,
};

/// A shell app wired with the full [`ScreenRouterPlugin`] (so the applier runs),
/// the dark theme, and a headless window — but with the shell built imperatively
/// (the plugin's `setup_shell` Startup also builds it, so build it ONLY via the
/// plugin to avoid a double tree). Seeds a small scroll set.
fn router_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);
    app.insert_resource(default_dark_theme());
    app.init_resource::<ScreenRouter>();
    app.add_message::<SwitchScreen>();

    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1280, 800),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    // Build the shell + mount the screens directly (this test drives the applier
    // by hand via `apply_screen_router`-equivalent message flow; it does NOT add
    // `ScreenRouterPlugin`'s Startup, to keep the tree single + deterministic).
    let world = app.world_mut();
    build_shell(world);
    mount_screens_with(world, 4);
    app
}

/// Collect each screen root's `(Screen, is_display_none, is_a11y_hidden)`.
fn screen_states(app: &mut App) -> Vec<(Screen, bool, bool)> {
    let mut q = app
        .world_mut()
        .query::<(Entity, &ScreenRoot, Option<&Display>, Option<&A11yHidden>)>();
    let world = app.world();
    let mut v: Vec<(Screen, bool, bool)> = q
        .iter(world)
        .map(|(_, r, d, h)| (r.0, matches!(d, Some(Display::None)), h.is_some()))
        .collect();
    v.sort_by_key(|(s, _, _)| format!("{s:?}"));
    v
}

#[test]
fn initial_state_shows_only_the_default_screen() {
    let mut app = router_app();
    for (screen, is_none, is_hidden) in screen_states(&mut app) {
        if screen == Screen::default() {
            assert!(
                !is_none && !is_hidden,
                "the default screen {screen:?} must be visible (not Display::None / A11yHidden)"
            );
        } else {
            assert!(
                is_none && is_hidden,
                "inactive screen {screen:?} must be Display::None + A11yHidden at boot"
            );
        }
    }
}

#[test]
fn switching_flips_which_screen_is_displayed() {
    let mut app = router_app();

    // Switch to Scroll by writing a SwitchScreen + driving the applier directly
    // (the applier is `buiy_gallery::shell::apply_screen_router`, an exclusive
    // system; call it via a one-shot to avoid scheduling the whole plugin).
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<SwitchScreen>>()
        .write(SwitchScreen(Screen::Scroll));
    let id = app
        .world_mut()
        .register_system(buiy_gallery::shell::apply_screen_router);
    app.world_mut().run_system(id).expect("run applier");

    // The router now points at Scroll.
    assert_eq!(app.world().resource::<ScreenRouter>().0, Screen::Scroll);

    // Exactly Scroll is shown; the others (incl. the former Todo) are hidden.
    for (screen, is_none, is_hidden) in screen_states(&mut app) {
        if screen == Screen::Scroll {
            assert!(
                !is_none && !is_hidden,
                "after switch, Scroll must be visible, got Display::None={is_none} hidden={is_hidden}"
            );
        } else {
            assert!(
                is_none && is_hidden,
                "after switch, {screen:?} must be Display::None + A11yHidden"
            );
        }
    }
}

/// A no-op switch (to the already-active screen) leaves the router + states
/// unchanged — the applier early-returns on `target == current` (idempotent).
#[test]
fn switching_to_the_active_screen_is_a_noop() {
    let mut app = router_app();
    let before = screen_states(&mut app);

    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<SwitchScreen>>()
        .write(SwitchScreen(Screen::default()));
    let id = app
        .world_mut()
        .register_system(buiy_gallery::shell::apply_screen_router);
    app.world_mut().run_system(id).expect("run applier");

    assert_eq!(app.world().resource::<ScreenRouter>().0, Screen::default());
    assert_eq!(
        before,
        screen_states(&mut app),
        "no-op switch changed state"
    );
}

// Reference the plugin so the import is used (it is the production wiring the
// behavior tests validate piecemeal; the binary adds it whole).
#[allow(dead_code)]
fn _plugin_is_the_production_wiring() -> ScreenRouterPlugin {
    ScreenRouterPlugin
}
