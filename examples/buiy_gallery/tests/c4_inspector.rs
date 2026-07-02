//! Headless **C4 inspector + theming** behavior gate. Proves the three new C4
//! behaviors the layout snapshot (a single static frame) cannot express:
//!
//! 1. **Rail active-state follows the router.** After a `SwitchScreen` +
//!    `reflect_rail_active_state`, exactly the active nav button shows the
//!    `surface.card` bg + the accent bar/idx; the rest are transparent.
//! 2. **A `SetAccent` swatch swap re-themes the resolved accent.** Driving a
//!    swatch's `OnPress` through `route_accent_press` + `apply_set_accent` mutates
//!    `Theme`'s `color.accent` to the pressed swatch's color.
//! 3. **The inspector live-state reflects a screen's state.** After a switch +
//!    `update_inspector_live_state`, the live-state rows carry the active screen's
//!    keys, and toggling a todo updates the `remaining`/`completed` values.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::scene::ScenePlugin;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::ColorToken;
use buiy_core::a11y::{A11yToggled, Toggled};
use buiy_core::render::color::ThemeContract;
use buiy_core::interaction::OnPress;
use buiy_core::render::components::{Background, TextColor};
use buiy_core::text::Text;
use buiy_core::theme::{SetAccent, Theme, default_dark_theme};
use buiy_gallery::inspector::{
    AccentSwatch, InspectorSlot, LiveStateValue, build_inspector_content,
    rebuild_inspector_on_switch, route_accent_press, update_inspector_live_state,
};
use buiy_gallery::shell::{
    NavPart, Screen, ScreenNav, ScreenRouter, SwitchScreen, apply_screen_router, build_shell,
    mount_screens_with, reflect_rail_active_state,
};
use buiy_gallery::{Filter, RowCheckbox, TodoMvcPlugin};

/// A shell app wired with the dark theme, a headless window, the shell tree +
/// inspector content, and the per-screen plugins the live-state reads. The C4
/// systems are driven by hand (one-shot `register_system`) so each test exercises
/// exactly one behavior without the whole `Update` schedule.
fn inspector_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin)
        // `TodoMvcPlugin` inits the `Filter` resource the todo live-state reads +
        // owns the todo row toggle the live-state test drives.
        .add_plugins(TodoMvcPlugin);
    app.insert_resource(default_dark_theme());
    app.init_resource::<ScreenRouter>();
    app.add_message::<SwitchScreen>();
    app.add_message::<SetAccent>();
    // `apply_set_accent` is `ThemePlugin`'s; add it directly (we don't add the
    // whole plugin, which would re-insert the LIGHT theme over our dark one).
    app.add_systems(bevy::app::Update, buiy_core::theme::apply_set_accent);

    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1280, 800),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    let world = app.world_mut();
    build_shell(world);
    mount_screens_with(world, 4);
    build_inspector_content(world);
    app
}

/// [`inspector_app`] with the S2 scroll-row count parameterized — the
/// `scroll_mounted_*` test needs more rows than fit the visible window (11) so
/// the windowed "mounted" count diverges from the filtered "nodes" total.
fn inspector_app_with_rows(scroll_rows: usize) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin)
        .add_plugins(TodoMvcPlugin);
    app.insert_resource(default_dark_theme());
    app.init_resource::<ScreenRouter>();
    app.add_message::<SwitchScreen>();
    app.add_message::<SetAccent>();
    app.add_systems(bevy::app::Update, buiy_core::theme::apply_set_accent);
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1280, 800),
            ..Default::default()
        },
        PrimaryWindow,
    ));
    let world = app.world_mut();
    build_shell(world);
    mount_screens_with(world, scroll_rows);
    build_inspector_content(world);
    app
}

/// Run an exclusive `&mut World` system once via a one-shot registration.
fn run<M>(app: &mut App, system: impl bevy::ecs::system::IntoSystem<(), (), M> + 'static) {
    let id = app.world_mut().register_system(system);
    app.world_mut().run_system(id).expect("run one-shot system");
}

/// Switch the active screen: write `SwitchScreen` + run the applier (which sets
/// the router) + the rail reflect + the live-state refresh.
fn switch_to(app: &mut App, screen: Screen) {
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<SwitchScreen>>()
        .write(SwitchScreen(screen));
    run(app, apply_screen_router);
    run(app, reflect_rail_active_state);
    // The switch-rebuild swaps the per-screen content (chips + live-state row
    // skeleton); `update_inspector_live_state` then fills the fresh rows' values.
    run(app, rebuild_inspector_on_switch);
    run(app, update_inspector_live_state);
}

// ---------------------------------------------------------------------------
// 1. Rail active-state follows the router.
// ---------------------------------------------------------------------------

/// The `(screen, is_active_card_bg)` of each nav button — active = the button's
/// `Background` is `surface.card`.
fn nav_active_states(app: &mut App) -> Vec<(Screen, bool)> {
    let mut q = app.world_mut().query::<(&ScreenNav, &Background)>();
    let world = app.world();
    let mut v: Vec<(Screen, bool)> = q
        .iter(world)
        .map(|(nav, bg)| (nav.0, bg.color == ColorToken::SurfaceCard))
        .collect();
    v.sort_by_key(|(s, _)| format!("{s:?}"));
    v
}

#[test]
fn rail_active_state_follows_the_router() {
    let mut app = inspector_app();

    // At boot, the default Todo button is active; the rest are not. (Built by
    // `build_nav_button(active = screen == default)`.)
    for (screen, active) in nav_active_states(&mut app) {
        assert_eq!(
            active,
            screen == Screen::default(),
            "at boot, only {:?} should be active; {screen:?} active={active}",
            Screen::default()
        );
    }

    // Switch to Menu: exactly Menu is active now.
    switch_to(&mut app, Screen::Menu);
    for (screen, active) in nav_active_states(&mut app) {
        assert_eq!(
            active,
            screen == Screen::Menu,
            "after switch to Menu, {screen:?} active={active} (expected only Menu active)"
        );
    }

    // The Menu nav button's accent BAR is now accent-colored; a non-active one is
    // transparent.
    let bars = nav_bar_colors(&mut app);
    for (screen, is_accent) in bars {
        assert_eq!(
            is_accent,
            screen == Screen::Menu,
            "after switch to Menu, {screen:?} bar accent={is_accent} (expected only Menu)"
        );
    }
}

/// The `(screen, bar_is_accent)` of each nav button's accent left-bar.
fn nav_bar_colors(app: &mut App) -> Vec<(Screen, bool)> {
    // Walk each ScreenNav button → its NavPart::Bar child → its Background.
    let buttons: Vec<(Screen, Vec<Entity>)> = {
        let mut q = app
            .world_mut()
            .query::<(&ScreenNav, &bevy::prelude::Children)>();
        let world = app.world();
        q.iter(world)
            .map(|(nav, ch)| (nav.0, ch.iter().copied().collect()))
            .collect()
    };
    let mut v = Vec::new();
    for (screen, children) in buttons {
        let mut is_accent = false;
        for child in children {
            let world = app.world();
            if matches!(world.get::<NavPart>(child), Some(NavPart::Bar))
                && let Some(bg) = world.get::<Background>(child)
            {
                is_accent = bg.color == ColorToken::Accent;
            }
        }
        v.push((screen, is_accent));
    }
    v.sort_by_key(|(s, _)| format!("{s:?}"));
    v
}

// ---------------------------------------------------------------------------
// 2. A SetAccent swatch press re-themes the resolved accent.
// ---------------------------------------------------------------------------

#[test]
fn accent_swatch_press_retheme_resolves_new_accent() {
    let mut app = inspector_app();

    // The boot accent is the design blue.
    let blue = bevy::prelude::Color::srgb_u8(0x5b, 0x86, 0xf5);
    let green = bevy::prelude::Color::srgb_u8(0x45, 0xc0, 0x7d);
    assert!(
        colors_match(theme_accent(&app), blue),
        "boot accent should be blue"
    );

    // Find the Green swatch + fire its OnPress, then run the collector + applier.
    let green_swatch = find_swatch(&mut app, green).expect("a Green accent swatch exists");
    app.world_mut()
        .resource_mut::<bevy::ecs::message::Messages<OnPress>>()
        .write(OnPress(green_swatch));
    run(&mut app, route_accent_press);
    run(&mut app, buiy_core::theme::apply_set_accent);

    // The resolved `color.accent` is now green (the whole-app re-theme source).
    assert!(
        colors_match(theme_accent(&app), green),
        "after pressing the Green swatch, the theme accent should be green, got {:?}",
        theme_accent(&app)
    );
}

/// The theme's resolved `color.accent`.
fn theme_accent(app: &App) -> bevy::prelude::Color {
    app.world().resource::<Theme>().resolve(ColorToken::Accent)
}

/// The swatch button whose `AccentSwatch` color matches `color`.
fn find_swatch(app: &mut App, color: bevy::prelude::Color) -> Option<Entity> {
    let mut q = app.world_mut().query::<(Entity, &AccentSwatch)>();
    let world = app.world();
    q.iter(world)
        .find(|(_, s)| colors_match(s.0, color))
        .map(|(e, _)| e)
}

/// Two colors match on their srgb u8 channels.
fn colors_match(a: bevy::prelude::Color, b: bevy::prelude::Color) -> bool {
    let to_u8 = |c: bevy::prelude::Color| {
        let s = bevy::prelude::Srgba::from(c);
        let q = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
        (q(s.red), q(s.green), q(s.blue))
    };
    to_u8(a) == to_u8(b)
}

// ---------------------------------------------------------------------------
// 3. The inspector live-state reflects a screen's state.
// ---------------------------------------------------------------------------

#[test]
fn live_state_reflects_the_active_screen() {
    let mut app = inspector_app();

    // Todo is the boot screen: its live-state rows carry the todo keys.
    let keys = live_state_keys(&mut app);
    assert_eq!(
        keys,
        vec!["total", "remaining", "completed", "filter"],
        "Todo live-state keys"
    );

    // The seeded demo rows make `total` non-zero (the `DEMO_SEEDS` are 3 rows).
    let total = live_value(&mut app, "total");
    assert_eq!(total, "3", "Todo total should reflect the 3 demo rows");

    // Switching to the Showcase swaps the live-state row set (its different keys).
    switch_to(&mut app, Screen::Showcase);
    let keys = live_state_keys(&mut app);
    assert_eq!(
        keys,
        vec!["wireframe", "radius", "density", "count", "build"],
        "Showcase live-state keys after switch"
    );
}

/// REGRESSION: the Scroll inspector's `mounted` cell must report the WINDOWED
/// visible-row count (the design's `visRows.length`, what the footer's
/// "rows X–Y mounted" reports), NOT the filtered node total. Both were wired to
/// the filtered total, so `mounted` read 1000 while the footer read 11. With
/// more rows than the visible window (11), `mounted` (= the window) must be
/// strictly less than `nodes` (= the filtered total).
#[test]
fn scroll_mounted_reports_windowed_count_not_filtered_total() {
    let mut app = inspector_app_with_rows(60);
    switch_to(&mut app, Screen::Scroll);

    let keys = live_state_keys(&mut app);
    assert_eq!(
        keys,
        vec!["nodes", "mounted", "window", "selected"],
        "Scroll live-state keys"
    );

    let nodes: usize = live_value(&mut app, "nodes")
        .replace(',', "")
        .parse()
        .unwrap();
    let mounted: usize = live_value(&mut app, "mounted").parse().unwrap();
    assert_eq!(
        nodes, 60,
        "`nodes` is the filtered total (all 60 rows mount)"
    );
    // The visible window over a 360px viewport of 34px rows is 11 rows.
    assert_eq!(
        mounted, 11,
        "`mounted` must be the windowed visible-row count (11), not the total"
    );
    assert!(
        mounted < nodes,
        "with 60 rows the windowed `mounted` ({mounted}) must be below `nodes` ({nodes}) \
         — equality here is the original bug (mounted wired to the filtered total)"
    );
}

/// REGRESSION: the Showcase inspector's `density` cell must read the SHOWCASE
/// segmented (Cozy/Compact/Dense), not the modal screen's KIND segmented
/// (Button/Layout/Input) — both share the `SegmentedOption` marker, so the old
/// unscoped query could return the modal's selected "Button". The density must
/// be the showcase default "compact" and never "button".
#[test]
fn showcase_density_reads_showcase_segmented_not_modal_kind() {
    let mut app = inspector_app();
    switch_to(&mut app, Screen::Showcase);

    let density = live_value(&mut app, "density");
    assert_eq!(
        density, "compact",
        "density must be the showcase segmented's selected option (compact), \
         not the modal KIND segmented's 'button' (the unscoped-query bug)"
    );
    assert_ne!(
        density, "button",
        "'button' here means the inspector read the modal's KIND segmented"
    );
}

/// REGRESSION (live): selecting a different density segment must move the
/// inspector's `density` cell (it follows the showcase segmented selection), and
/// stay scoped to the showcase track even after the click. Drives the real
/// `apply_showcase_intents` restyle path (the `set_segmented` accent re-fill the
/// inspector reads), then refreshes the live-state.
#[test]
fn showcase_density_follows_a_segment_selection() {
    use buiy_gallery::composites::SegmentedOption;
    use buiy_gallery::{ShowcaseDensitySegmented, ShowcaseIntents, apply_showcase_intents};

    let mut app = inspector_app();
    switch_to(&mut app, Screen::Showcase);
    app.init_resource::<ShowcaseIntents>();

    // The showcase density track's "Dense" option (index 2 — Cozy/Compact/Dense).
    let dense = {
        let track_children: Vec<Entity> = {
            let mut q = app
                .world_mut()
                .query_filtered::<&bevy::prelude::Children, bevy::prelude::With<ShowcaseDensitySegmented>>();
            let world = app.world();
            q.iter(world)
                .next()
                .map(|c| c.iter().copied().collect())
                .unwrap_or_default()
        };
        let world = app.world();
        track_children
            .into_iter()
            .find(|&c| world.get::<SegmentedOption>(c).map(|o| o.0) == Some(2))
            .expect("the showcase density track has a Dense (index 2) option")
    };

    // Stage + apply the selection (the production restyle path), then refresh.
    app.world_mut()
        .resource_mut::<ShowcaseIntents>()
        .select_segment = Some(dense);
    run(&mut app, apply_showcase_intents);
    run(&mut app, update_inspector_live_state);

    let density = live_value(&mut app, "density");
    assert_eq!(
        density, "dense",
        "selecting the Dense segment must move the inspector's density to 'dense'"
    );
}

#[test]
fn live_state_updates_when_a_todo_toggles() {
    let mut app = inspector_app();

    // The Todo screen's initial remaining = total − completed (the demo seeds have
    // a mix). Capture the initial `remaining` value, toggle a row, and confirm the
    // live-state updater rewrote it.
    let remaining_before: i32 = live_value(&mut app, "remaining").parse().unwrap_or(-1);
    let completed_before: i32 = live_value(&mut app, "completed").parse().unwrap_or(-1);
    assert!(
        remaining_before >= 0 && completed_before >= 0,
        "the todo remaining/completed values should be numeric"
    );

    // Toggle the first NOT-done row's checkbox to done (mutate `A11yToggled`).
    let toggled = toggle_first_active_row(&mut app);
    assert!(toggled, "there should be at least one active row to toggle");

    // Re-run the live-state refresh; remaining drops by 1, completed rises by 1.
    run(&mut app, update_inspector_live_state);
    let remaining_after: i32 = live_value(&mut app, "remaining").parse().unwrap_or(-1);
    let completed_after: i32 = live_value(&mut app, "completed").parse().unwrap_or(-1);
    assert_eq!(
        remaining_after,
        remaining_before - 1,
        "remaining should drop by 1 after toggling an active row done"
    );
    assert_eq!(
        completed_after,
        completed_before + 1,
        "completed should rise by 1 after toggling an active row done"
    );
    // The `remaining` row's color is accent while > 0, ok-green at 0.
    let want = if remaining_after > 0 {
        ColorToken::Accent
    } else {
        ColorToken::StatusOk
    };
    assert!(
        live_value_color_is(&mut app, "remaining", want),
        "remaining color should be {want:?} at value {remaining_after}"
    );
    // Reference `Filter` so the import is exercised (the live-state reads it).
    let _ = app.world().get_resource::<Filter>();
}

/// The ordered live-state keys currently present in the inspector (read from the
/// `LiveStateValue`-tagged leaves, in the live-state column's child order).
fn live_state_keys(app: &mut App) -> Vec<String> {
    // Find the live-state column, then read its row children's value-leaf keys in
    // order.
    let column = find_slot(app, InspectorSlot::LiveState).expect("the live-state slot exists");
    let rows: Vec<Entity> = {
        let world = app.world();
        world
            .get::<bevy::prelude::Children>(column)
            .into_iter()
            .flat_map(|c| c.iter().copied().collect::<Vec<_>>())
            .collect()
    };
    let mut keys = Vec::new();
    for row in rows {
        // The row's value leaf carries `LiveStateValue(key)`.
        if let Some(key) = descendant_live_key(app, row) {
            keys.push(key);
        }
    }
    keys
}

/// The `LiveStateValue` key of `row`'s tagged value descendant.
fn descendant_live_key(app: &mut App, row: Entity) -> Option<String> {
    let world = app.world();
    let children = world.get::<bevy::prelude::Children>(row)?;
    for child in children.iter().copied() {
        if let Some(k) = world.get::<LiveStateValue>(child) {
            return Some(k.0.clone());
        }
    }
    None
}

/// The inspector slot entity matching `slot`.
fn find_slot(app: &mut App, slot: InspectorSlot) -> Option<Entity> {
    let mut q = app.world_mut().query::<(Entity, &InspectorSlot)>();
    let world = app.world();
    q.iter(world).find(|(_, s)| **s == slot).map(|(e, _)| e)
}

/// The current value string of the live-state row keyed `key`.
fn live_value(app: &mut App, key: &str) -> String {
    let leaf = find_live_leaf(app, key).expect("the live-state key exists");
    app.world()
        .get::<Text>(leaf)
        .map(|t| t.0.clone())
        .unwrap_or_default()
}

/// Whether the live-state row keyed `key` has `TextColor` of the given token.
fn live_value_color_is(app: &mut App, key: &str, want: ColorToken) -> bool {
    let Some(leaf) = find_live_leaf(app, key) else {
        return false;
    };
    app.world()
        .get::<TextColor>(leaf)
        .is_some_and(|c| c.0 == want)
}

/// The `LiveStateValue`-tagged leaf for `key`.
fn find_live_leaf(app: &mut App, key: &str) -> Option<Entity> {
    let mut q = app.world_mut().query::<(Entity, &LiveStateValue)>();
    let world = app.world();
    q.iter(world).find(|(_, k)| k.0 == key).map(|(e, _)| e)
}

/// Toggle the first NOT-done todo row checkbox to done (set its `A11yToggled` to
/// `True`). Returns whether an active row was found.
fn toggle_first_active_row(app: &mut App) -> bool {
    let target: Option<Entity> = {
        let mut q = app
            .world_mut()
            .query_filtered::<(Entity, &A11yToggled), bevy::prelude::With<RowCheckbox>>();
        let world = app.world();
        q.iter(world)
            .find(|(_, t)| t.0 != Toggled::True)
            .map(|(e, _)| e)
    };
    if let Some(e) = target {
        if let Some(mut t) = app.world_mut().get_mut::<A11yToggled>(e) {
            t.0 = Toggled::True;
        }
        true
    } else {
        false
    }
}
