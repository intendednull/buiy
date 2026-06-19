//! The phase-1 text authoring surface (font-assets § 8): the `Text` content
//! component, the font trio, and the plugin-level defaults that cover unset
//! components. Headless — no FontSystem interaction anywhere in this file.

use bevy::prelude::*;
use buiy_core::text::{
    BuiyTextPlugin, CollapseMode, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight,
    GenericFamily, LineHeight, TextAlign, TextStyleDefaults, TextWrap, WhiteSpace, resolve_wrap,
};
use cosmic_text::{Align, Wrap};

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

/// The carriers' defaults are the CSS initials (measure §§ 5.1–5.3).
#[test]
fn t3_carrier_defaults_are_the_css_initials() {
    assert_eq!(LineHeight::default(), LineHeight::Normal);
    assert_eq!(WhiteSpace::default(), WhiteSpace::Normal);
    assert_eq!(TextWrap::default(), TextWrap::Wrap);
    assert_eq!(TextAlign::default(), TextAlign::Start);
}

/// measure § 5.2 — the normative white-space value table, both columns.
#[test]
fn white_space_value_table() {
    let rows = [
        (WhiteSpace::Normal, CollapseMode::Collapse, Wrap::Word),
        (WhiteSpace::Nowrap, CollapseMode::Collapse, Wrap::None),
        (WhiteSpace::Pre, CollapseMode::Preserve, Wrap::None),
        (WhiteSpace::PreWrap, CollapseMode::Preserve, Wrap::Word),
        (
            WhiteSpace::PreLine,
            CollapseMode::PreserveBreaks,
            Wrap::Word,
        ),
    ];
    for (ws, mode, wrap) in rows {
        assert_eq!(ws.collapse_mode(), mode, "{ws:?} collapse column");
        assert_eq!(ws.base_wrap(), wrap, "{ws:?} wrap column");
    }
}

/// measure § 5.2 — text-wrap composes over the table: `nowrap` forces
/// `Wrap::None`; `wrap` keeps the table value; balance/pretty/stable
/// degrade to the greedy table value (warn-once, not asserted here).
#[test]
fn text_wrap_composition() {
    assert_eq!(
        resolve_wrap(WhiteSpace::Normal, TextWrap::Nowrap),
        Wrap::None
    );
    assert_eq!(resolve_wrap(WhiteSpace::Pre, TextWrap::Wrap), Wrap::None);
    assert_eq!(resolve_wrap(WhiteSpace::Normal, TextWrap::Wrap), Wrap::Word);
    for degraded in [TextWrap::Balance, TextWrap::Pretty, TextWrap::Stable] {
        assert_eq!(resolve_wrap(WhiteSpace::Normal, degraded), Wrap::Word);
        assert_eq!(resolve_wrap(WhiteSpace::Nowrap, degraded), Wrap::None);
    }
}

/// measure § 5.3 — the normative text-align value table. `start` → None
/// (cosmic's unaligned default follows BiDi direction — exactly CSS
/// `start`); `justify-all` degrades to Justified (warn-once).
#[test]
fn text_align_value_table() {
    assert_eq!(TextAlign::Start.to_cosmic(), None);
    assert_eq!(TextAlign::End.to_cosmic(), Some(Align::End));
    assert_eq!(TextAlign::Left.to_cosmic(), Some(Align::Left));
    assert_eq!(TextAlign::Right.to_cosmic(), Some(Align::Right));
    assert_eq!(TextAlign::Center.to_cosmic(), Some(Align::Center));
    assert_eq!(TextAlign::Justify.to_cosmic(), Some(Align::Justified));
    assert_eq!(TextAlign::JustifyAll.to_cosmic(), Some(Align::Justified));
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
        "buiy_core::text::components::LineHeight",
        "buiy_core::text::components::WhiteSpace",
        "buiy_core::text::components::TextWrap",
        "buiy_core::text::components::TextAlign",
    ] {
        assert!(
            registry.get_with_type_path(name).is_some(),
            "type not registered: {name}",
        );
    }
}
