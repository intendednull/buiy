//! The forced-colors Theme swap is a main-world system (no GPU). When
//! `UserPreferences.forced_colors` flips, the active `Theme` becomes the
//! system-color variant; when it clears, the prior theme is restored.
//! Spec: color-and-forced-colors.md § 3.1.

use bevy::prelude::*;
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::color::{ColorToken, SystemColorKeyword, ThemeContract};
use buiy_core::render::forced_colors::{PrePreferenceTheme, apply_forced_colors_theme};
use buiy_core::theme::{Theme, UserPreferences, default_light_theme};
use buiy_core::{BuiySet, CorePlugin};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(default_light_theme());
    app.insert_resource(UserPreferences::default());
    app.init_resource::<PrePreferenceTheme>();
    app.add_systems(Update, apply_forced_colors_theme);
    app
}

#[test]
fn flipping_forced_colors_swaps_in_system_color_theme() {
    let mut app = app();
    // Sanity: the normal theme resolves surface.primary to the LIGHT value.
    assert_eq!(
        app.world()
            .resource::<Theme>()
            .resolve(ColorToken::SurfacePrimary),
        Color::WHITE,
    );

    app.world_mut()
        .resource_mut::<UserPreferences>()
        .forced_colors = true;
    app.update();

    // After the swap, forced-colors mode maps every semantic token to a system
    // color: surface.* -> Canvas, text.* -> CanvasText.
    let t = app.world().resource::<Theme>();
    assert_eq!(
        t.resolve(ColorToken::SurfacePrimary),
        t.resolve(ColorToken::SystemColor(SystemColorKeyword::Canvas)),
    );
    assert_eq!(
        t.resolve(ColorToken::TextPrimary),
        t.resolve(ColorToken::SystemColor(SystemColorKeyword::CanvasText)),
    );
}

#[test]
fn clearing_forced_colors_restores_prior_theme() {
    let mut app = app();
    app.world_mut()
        .resource_mut::<UserPreferences>()
        .forced_colors = true;
    app.update();
    let t = app.world().resource::<Theme>();
    assert_eq!(
        t.resolve(ColorToken::SurfacePrimary),
        t.resolve(ColorToken::SystemColor(SystemColorKeyword::Canvas)),
    );

    app.world_mut()
        .resource_mut::<UserPreferences>()
        .forced_colors = false;
    app.update();

    // Prior (light) theme is back: surface.primary resolves to the light value.
    assert_eq!(
        app.world()
            .resource::<Theme>()
            .resolve(ColorToken::SurfacePrimary),
        Color::WHITE,
    );
}

/// Records `Theme::is_changed()` as observed *inside* the schedule, the only
/// vantage point where an in-schedule mutation is visible. A read of
/// `is_changed()` from the post-`app.update()` World always sees `false` — the
/// World's last-checked tick has advanced past the frame's mutation — so the
/// flip and no-flip cases are indistinguishable from outside. This probe makes
/// them distinguishable.
#[derive(Resource, Default)]
struct ThemeChangedProbe(bool);

fn record_theme_changed(theme: Res<Theme>, mut probe: ResMut<ThemeChangedProbe>) {
    probe.0 = theme.is_changed();
}

/// Build an app with the swap system and an after-it probe chained in the same
/// schedule, so the probe observes the change tick the swap (or its absence)
/// produces on that very frame.
fn app_with_probe() -> App {
    let mut app = app();
    app.init_resource::<ThemeChangedProbe>();
    app.add_systems(
        Update,
        record_theme_changed.after(apply_forced_colors_theme),
    );
    app
}

#[test]
fn no_flip_leaves_theme_unmarked() {
    // After a steady-state frame with no flip, the swap system must NOT
    // deref-mut Theme (which would spuriously mark it changed every frame and
    // force a full re-resolve — § 2.3 / § 3.1). Observed inside the schedule by
    // the probe; a post-update read can never see this, so the no-flip and flip
    // cases would otherwise be indistinguishable.
    let mut app = app_with_probe();
    app.update(); // baseline frame (Theme freshly inserted -> changed this frame)
    app.update(); // steady-state, no preference change
    assert!(
        !app.world().resource::<ThemeChangedProbe>().0,
        "Theme must not be marked changed on a no-flip frame"
    );
}

#[test]
fn flip_marks_theme_changed() {
    // The companion case that gives the no-flip assertion teeth: on the frame
    // the preference flips, the swap deref-muts Theme, so the probe observes it
    // as changed. Without this, a regression that marks Theme changed every
    // frame would leave both cases reading the same value.
    let mut app = app_with_probe();
    app.update(); // baseline
    app.update(); // steady-state (probe reads false here)
    app.world_mut()
        .resource_mut::<UserPreferences>()
        .forced_colors = true;
    app.update(); // flip frame
    assert!(
        app.world().resource::<ThemeChangedProbe>().0,
        "Theme must be marked changed on the frame forced-colors flips"
    );
}

#[test]
fn swap_system_registered_by_render_plugin_runs_headless() {
    // BuiyRenderPlugin must register the main-world swap system + its resource
    // even with no RenderApp (headless). Spawning the plugin and flipping the
    // preference must swap the theme on the next frame. (CorePlugin configures
    // the BuiySet chain but does not insert Theme — the test supplies it.)
    //
    // This proves the swap is *registered and runs*, not which set it is in —
    // that placement is pinned by `swap_system_is_member_of_style_set` below.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.insert_resource(default_light_theme());
    app.insert_resource(UserPreferences::default());
    app.add_plugins(BuiyRenderPlugin);

    app.world_mut()
        .resource_mut::<UserPreferences>()
        .forced_colors = true;
    app.update();
    let t = app.world().resource::<Theme>();
    assert_eq!(
        t.resolve(ColorToken::SurfacePrimary),
        t.resolve(ColorToken::SystemColor(SystemColorKeyword::Canvas)),
    );
}

/// Count the systems `BuiyRenderPlugin` registers in `BuiySet::Style`, as the
/// delta between an app *with* the plugin and a baseline *without* it. CorePlugin
/// may place its own systems in `Style`; those appear in both counts and cancel,
/// so the delta isolates the plugin's contribution — robust to unrelated growth
/// in CorePlugin's `Style` membership.
///
/// `systems_in_set` yields the `SystemKey`s of a set, but in this Bevy build the
/// system *objects* are moved into the executable and are not retained in
/// `graph.systems` after `app.update()` (see the note in
/// `tests/system_set_order.rs`), so membership can only be counted, not matched
/// back to a named system. The delta-count is the deterministic discriminator
/// available without GPU.
fn render_plugin_style_systems() -> usize {
    let count = |with_render: bool| -> usize {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.add_plugins(CorePlugin);
        app.insert_resource(default_light_theme());
        app.insert_resource(UserPreferences::default());
        if with_render {
            app.add_plugins(BuiyRenderPlugin);
        }
        app.update(); // forces Schedule::initialize so set membership is populated

        let schedules = app.world().resource::<Schedules>();
        let schedule = schedules
            .get(Update)
            .expect("CorePlugin registered systems on Update");
        schedule
            .graph()
            .systems_in_set(BuiySet::Style.intern())
            .map(|keys| keys.len())
            .unwrap_or(0)
    };
    count(true) - count(false)
}

#[test]
fn swap_system_is_member_of_style_set() {
    // The behavioral guard for Task 5: the swap must be a *member of*
    // `BuiySet::Style`, not merely registered somewhere. CorePlugin chains
    // Style -> … -> Render, so Style membership is what makes the swap visible to
    // the same frame's extract (§ 3.1: "selected in the main world, before
    // extract"). The set-chain order itself (Style before Render) is already
    // pinned by tests/system_set_order.rs; this test pins the swap *into* Style.
    //
    // Asserting on the membership delta — not a hardcoded count — fails
    // deterministically if `.in_set(BuiySet::Style)` is dropped from the
    // registration in render/mod.rs: the plugin then adds zero systems to Style
    // and the delta is 0. (Verified: removing that placement makes this assertion
    // fail while a probe-ordering check passes nondeterministically.)
    assert!(
        render_plugin_style_systems() >= 1,
        "BuiyRenderPlugin must place its forced-colors swap system in BuiySet::Style"
    );
}
