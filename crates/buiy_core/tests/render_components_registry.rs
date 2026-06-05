//! Headless (no GPU): asserts every author-set render component is
//! `register_type`'d by `BuiyRenderPlugin::build` (the registration runs in
//! the main world, before the RenderApp branch, so this works under
//! MinimalPlugins with no wgpu adapter). The computed components
//! (`ClipRect`, `AncestorClip`, `EffectGroup`) and the layout-owned
//! `OffscreenAuto` are deliberately NOT registered and are asserted absent.

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::render::BuiyRenderPlugin;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{
    AncestorClip, BackdropFilter, Background, Border, BoxShadow, ClipRadius, ClipRect,
    CssVisibility, EffectGroup, Filter, MixBlendMode, OffscreenAuto, Opacity, Outline,
};

#[test]
fn author_set_render_components_are_registered() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let reg = type_registry.read();

    assert!(
        reg.get(std::any::TypeId::of::<Background>()).is_some(),
        "Background"
    );
    assert!(
        reg.get(std::any::TypeId::of::<Border>()).is_some(),
        "Border"
    );
    assert!(
        reg.get(std::any::TypeId::of::<BoxShadow>()).is_some(),
        "BoxShadow"
    );
    assert!(
        reg.get(std::any::TypeId::of::<Opacity>()).is_some(),
        "Opacity"
    );
    assert!(
        reg.get(std::any::TypeId::of::<Outline>()).is_some(),
        "Outline"
    );
    assert!(
        reg.get(std::any::TypeId::of::<Filter>()).is_some(),
        "Filter"
    );
    assert!(
        reg.get(std::any::TypeId::of::<BackdropFilter>()).is_some(),
        "BackdropFilter"
    );
    assert!(
        reg.get(std::any::TypeId::of::<MixBlendMode>()).is_some(),
        "MixBlendMode"
    );
    assert!(
        reg.get(std::any::TypeId::of::<CssVisibility>()).is_some(),
        "CssVisibility"
    );
    assert!(
        reg.get(std::any::TypeId::of::<ClipRadius>()).is_some(),
        "ClipRadius"
    );
    assert!(
        reg.get(std::any::TypeId::of::<ColorToken>()).is_some(),
        "ColorToken"
    );
}

#[test]
fn computed_and_layout_owned_components_are_not_registered_here() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(BuiyRenderPlugin);

    let type_registry = app.world().resource::<AppTypeRegistry>().clone();
    let reg = type_registry.read();

    // ClipRect / AncestorClip / EffectGroup are computed (no Reflect derive,
    // so they cannot be in the registry); OffscreenAuto is layout-owned.
    assert!(
        reg.get(std::any::TypeId::of::<ClipRect>()).is_none(),
        "ClipRect must not be registered here"
    );
    assert!(
        reg.get(std::any::TypeId::of::<AncestorClip>()).is_none(),
        "AncestorClip must not be registered here"
    );
    assert!(
        reg.get(std::any::TypeId::of::<EffectGroup>()).is_none(),
        "EffectGroup must not be registered here"
    );
    assert!(
        reg.get(std::any::TypeId::of::<OffscreenAuto>()).is_none(),
        "OffscreenAuto is layout-owned"
    );
}
