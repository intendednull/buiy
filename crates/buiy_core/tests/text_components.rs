//! The phase-1 text authoring surface (font-assets § 8): the `Text` content
//! component, the font trio, and the plugin-level defaults that cover unset
//! components. Headless — no FontSystem interaction anywhere in this file.

use bevy::prelude::*;
use buiy_core::text::{
    BuiyTextPlugin, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, GenericFamily,
    TextStyleDefaults,
};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// The CSS initial values the unset-component fallbacks reproduce:
/// sans-serif / 16 px (`medium`) / 400 (`normal`).
#[test]
fn component_defaults_are_the_css_initials() {
    assert_eq!(FontSize::default().0, 16.0);
    assert_eq!(FontWeight::default().0, 400);
    assert_eq!(
        FontFamily::default().0,
        FontStack(vec![FamilyEntry::Generic(GenericFamily::SansSerif)])
    );
}

/// font-assets § 8: plugin-level defaults cover unset components. One source
/// of truth — the resource is constructed FROM the component defaults.
#[test]
fn plugin_defaults_mirror_component_defaults() {
    let app = text_app();
    let defaults = app.world().resource::<TextStyleDefaults>();
    assert_eq!(defaults.family, FontFamily::default().0);
    assert_eq!(defaults.size, FontSize::default().0);
    assert_eq!(defaults.weight, FontWeight::default().0);
}

mod text_color {
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::TextColor;

    /// `ColorToken::default()` is `Transparent` — right for `Background`
    /// ("absent == no fill"), INVISIBLE for glyphs. The glyph foreground
    /// defaults to `CurrentColor`, which `resolve_token` already lowers to
    /// the theme default foreground (`CanvasText` under forced-colors, else
    /// `color.text.primary`) — glyph-pipeline § 7.
    #[test]
    fn text_color_defaults_to_current_color_not_transparent() {
        assert_eq!(TextColor::default().0, ColorToken::CurrentColor);
    }
}

/// Author-set components are reflect-registered (BSN / inspectors), the
/// layout-components convention.
#[test]
fn authoring_types_are_registered_for_reflection() {
    let mut app = text_app();
    app.update();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    for name in [
        "buiy_core::text::components::Text",
        "buiy_core::text::components::FontFamily",
        "buiy_core::text::components::FontSize",
        "buiy_core::text::components::FontWeight",
    ] {
        assert!(
            registry.get_with_type_path(name).is_some(),
            "type not registered: {name}",
        );
    }
}
