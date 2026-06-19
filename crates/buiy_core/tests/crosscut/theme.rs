use buiy_core::theme::{UserPreferences, default_light_theme};

#[test]
fn default_theme_resolves_known_tokens() {
    let theme = default_light_theme();
    let bg = theme
        .color("color.surface.primary")
        .expect("primary surface");
    let fg = theme.color("color.text.primary").expect("primary text");
    assert!(bg != fg, "fg and bg must differ");
    let space_4 = theme.space("space.4").expect("space.4");
    assert!(space_4 > 0.0);
}

#[test]
fn user_preferences_default_to_light_no_reduce_motion() {
    let prefs = UserPreferences::default();
    assert!(!prefs.prefers_dark);
    assert!(!prefs.prefers_reduced_motion);
    assert!(!prefs.forced_colors);
}
