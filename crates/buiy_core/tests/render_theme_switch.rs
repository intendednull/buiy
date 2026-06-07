//! A theme switch re-resolves token colors with no cached, theme-stamped
//! buffer — proven at the resolver layer (extract re-reads the live Theme).
//! Pure CPU, no GPU. Spec: color-and-forced-colors.md § 2.3 / § 3.1.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, resolve_token};
use buiy_core::render::forced_colors::{PrePreferenceTheme, apply_forced_colors_theme};
use buiy_core::theme::{Theme, UserPreferences, default_light_theme, forced_colors_theme};
use std::borrow::Cow;

#[test]
fn replacing_theme_reresolves_token_next_read() {
    let token = ColorToken::Token(Cow::Borrowed("color.surface.primary"));

    let mut light = default_light_theme();
    assert_eq!(resolve_token(&token, &light), Color::WHITE);

    // Mutate the same key to a new value (a brand/dark swap).
    light
        .colors
        .insert("color.surface.primary".into(), Color::BLACK);
    assert_eq!(resolve_token(&token, &light), Color::BLACK);
}

/// Records `Theme::is_changed()` *inside* the schedule — the only vantage point
/// where an in-schedule mutation is visible. A read of `is_changed()` from the
/// post-`app.update()` World always sees `false`, because the World's
/// last-checked tick has advanced past the frame's mutation (the same property
/// the Task-4 swap test pins via `record_theme_changed`). The extract reads
/// `Theme` from *inside* the schedule too, so this probe mirrors exactly what
/// extract would observe on the swap frame.
#[derive(Resource, Default)]
struct ThemeChangedProbe(bool);

fn record_theme_changed(theme: Res<Theme>, mut probe: ResMut<ThemeChangedProbe>) {
    probe.0 = theme.is_changed();
}

#[test]
fn theme_swap_marks_resource_changed_for_extract() {
    // The is_changed() edge: a ResMut<Theme> swap marks the resource changed on
    // the swap frame, so the same frame's extract (which reads Theme live, from
    // inside the schedule) re-resolves with no cached, theme-stamped buffer
    // (§ 2.3). The probe runs after the swap, mirroring extract's vantage point.
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(default_light_theme());
    app.insert_resource(UserPreferences::default());
    app.init_resource::<PrePreferenceTheme>();
    app.init_resource::<ThemeChangedProbe>();
    app.add_systems(
        Update,
        (apply_forced_colors_theme, record_theme_changed).chain(),
    );

    app.update(); // baseline
    app.world_mut()
        .resource_mut::<UserPreferences>()
        .forced_colors = true;
    app.update(); // swap frame

    assert!(
        app.world().resource::<ThemeChangedProbe>().0,
        "theme swap must mark Theme changed so extract re-resolves (§ 2.3)"
    );
    // And the new value is the forced palette.
    let forced = forced_colors_theme();
    assert_eq!(
        app.world().resource::<Theme>().color("Canvas"),
        forced.color("Canvas")
    );
}
