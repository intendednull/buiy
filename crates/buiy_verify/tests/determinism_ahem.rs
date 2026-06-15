//! `FontMode::Ahem` sole-family resolution (Phase 3.2, verification-design
//! `determinism.md` § "Ahem font mode"). Pure-CPU, headless — resolution runs
//! on the lock-free `FontMatchIndex` substrate, no rasterizer, no adapter.
//!
//! The determinism contract these tests pin: under Ahem mode, fixture text that
//! names `font-family: Ahem` resolves to the bundled em-box face REGARDLESS of
//! host fonts — system fonts are off and Ahem (registered through the
//! production bytes path) is the only family the resolver can reach. That is
//! the host-stability the box-font substitution buys; the pixel-level twin runs
//! `#[ignore]` in `determinism_capture.rs`.

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::LayoutPlugin;
use buiy_core::text::{
    BuiyTextPlugin, FamilyEntry, FontMatchIndex, FontRegistry, FontStack, ResolvedFamily,
    resolve_spans,
};
use buiy_verify::determinism::{AHEM_FAMILY, register_ahem};

/// MinimalPlugins + text, system fonts OFF (the `BuiyTextPlugin::default()`
/// headless capture shape) — no AssetPlugin, no adapter. The resolver
/// substrate works asset-machinery-free.
fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// Lift the resolver substrate (`FontMatchIndex` + `FontRegistry`) out of a
/// settled app, exactly as `buiy_core`'s `text_resolver.rs` does — built
/// entirely through the production App path, no test-only constructors.
fn substrate(app: &mut App) -> (FontMatchIndex, FontRegistry) {
    let index = app
        .world_mut()
        .remove_resource::<FontMatchIndex>()
        .expect("BuiyTextPlugin inserts the FontMatchIndex");
    let registry = app
        .world_mut()
        .remove_resource::<FontRegistry>()
        .expect("BuiyTextPlugin inits the FontRegistry");
    (index, registry)
}

#[test]
fn ahem_is_sole_family_under_ahem_mode() {
    // Register the box-font through the production bytes path + settle.
    let mut app = text_app();
    app.update();
    register_ahem(&mut app);
    let (mut index, registry) = substrate(&mut app);

    // A fixture string under `font-family: Ahem` (the only authored family).
    let stack = FontStack(vec![FamilyEntry::Named(String::from(AHEM_FAMILY))]);
    let resolution = resolve_spans("Hello box", &stack, 400, &registry, &mut index, 0.0);

    // Every span resolves to Ahem — the box-font covers ASCII, so the walk
    // never falls through to a host font (there is none) or the generic.
    assert!(
        !resolution.blocked,
        "Ahem registers synchronously (bytes path)"
    );
    assert!(
        !resolution.spans.is_empty(),
        "non-empty text yields at least one span"
    );
    for span in &resolution.spans {
        assert_eq!(
            span.family,
            ResolvedFamily::Named(String::from(AHEM_FAMILY)),
            "span {:?} resolved to {:?}, not the sole Ahem family — fallback \
             leaked a non-Ahem face",
            span.range,
            span.family,
        );
    }
}

#[test]
fn ahem_resolution_is_host_font_independent() {
    // The determinism claim stated directly: resolution under Ahem mode does
    // NOT depend on what fonts the host has. We cannot install host fonts in a
    // unit test, but we CAN prove the resolved family is fixed to Ahem and
    // never the embedded default ("Fira Sans") even when the stack would
    // otherwise let a covered ASCII char match another registered family.
    let mut app = text_app();
    app.update();
    register_ahem(&mut app);
    let (mut index, registry) = substrate(&mut app);

    // Stack names ONLY Ahem; "Fira Sans" is embedded and also covers ASCII,
    // but it is not in the stack, so it can never win. The result is Ahem,
    // identical to what any other host would resolve (bundled-only).
    let stack = FontStack(vec![FamilyEntry::Named(String::from(AHEM_FAMILY))]);
    let resolution = resolve_spans("ABCabc123", &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(
        resolution.spans.len(),
        1,
        "all-ASCII covered by Ahem ⇒ one span"
    );
    assert_eq!(
        resolution.spans[0].family,
        ResolvedFamily::Named(String::from(AHEM_FAMILY)),
    );
}
