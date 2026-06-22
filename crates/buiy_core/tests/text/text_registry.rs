//! FontRegistry (font-assets § 3): the FontFaceSet model — strong handles,
//! explicit register/unregister, in-place add, REBUILD on remove (the
//! unpurgeable font_cache: a fresh FontSystem is the only purge), Modified
//! = remove+re-add. The leak/staleness contract: across N hot-reload
//! cycles, dead IDs never resolve (`get_font` None), the db never grows,
//! and FontsGeneration bumps exactly once per cycle.

use std::sync::Arc;

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::LayoutPlugin;
use buiy_core::text::{
    BuiyFont, BuiyTextPlugin, FontDbLineage, FontFaceDescriptors, FontLoadState, FontRegistry,
    FontsGeneration, SharedFontSystem,
};
use cosmic_text::fontdb;

fn fira_bytes() -> Arc<Vec<u8>> {
    Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .unwrap(),
    )
}

/// MinimalPlugins + text, NO AssetPlugin: the bytes path must work
/// asset-machinery-free (T5 plan decision 13).
fn registry_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn generation(app: &App) -> u64 {
    app.world().resource::<FontsGeneration>().0
}

fn lineage(app: &App) -> u64 {
    app.world().resource::<FontDbLineage>().0
}

fn db_face_count(app: &App) -> usize {
    app.world().resource::<SharedFontSystem>().lock().db().len()
}

#[test]
fn register_bytes_adds_faces_in_place_and_bumps_generation_once() {
    let mut app = registry_app();
    app.update(); // settle plugin init (generation is_added frame)
    let gen0 = generation(&app);
    let faces0 = db_face_count(&app);

    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    app.update();

    assert_eq!(generation(&app), gen0 + 1, "exactly one bump per batch");
    assert_eq!(lineage(&app), 0, "in-place add never bumps lineage");
    assert_eq!(db_face_count(&app), faces0 + 1);
    let registry = app.world().resource::<FontRegistry>();
    assert_eq!(
        registry.load_state("Fira Sans"),
        Some(FontLoadState::Loaded)
    );
    assert_eq!(registry.faces("Fira Sans").len(), 1);
}

#[test]
fn unregister_rebuilds_no_stale_get_font_no_growth() {
    // The § 3.1 contract: after unregister, the dead ID must not resolve
    // (the rebuild swapped in a FRESH FontSystem — fresh font_cache), the
    // db face count returns to baseline, and surviving faces still resolve.
    let mut app = registry_app();
    app.update();
    let faces0 = db_face_count(&app);
    let surviving = app
        .world()
        .resource::<SharedFontSystem>()
        .lock()
        .db()
        .faces()
        .next()
        .unwrap()
        .id;

    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    app.update();
    let dead = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    // Warm the cache so staleness is a real assertion, not vacuous.
    assert!(
        app.world()
            .resource::<SharedFontSystem>()
            .lock()
            .get_font(dead, fontdb::Weight(400))
            .is_some()
    );

    let gen_before = generation(&app);
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .unregister_family("Fira Sans");
    app.update();

    assert_eq!(generation(&app), gen_before + 1);
    assert_eq!(
        lineage(&app),
        0,
        "into_locale_and_db carries the db — in-lineage"
    );
    assert_eq!(db_face_count(&app), faces0, "no growth");
    let fonts = app.world().resource::<SharedFontSystem>();
    let mut guard = fonts.lock();
    assert!(
        guard.get_font(dead, fontdb::Weight(400)).is_none(),
        "no stale get_font hit — the rebuilt FontSystem has a fresh font_cache"
    );
    assert!(
        guard.get_font(surviving, fontdb::Weight(400)).is_some(),
        "surviving IDs stayed valid (Orientation fact 1)"
    );
    drop(guard);
    assert!(
        app.world()
            .resource::<FontRegistry>()
            .load_state("Fira Sans")
            .is_none()
    );
}

#[test]
fn hot_reload_cycles_leak_nothing_and_stay_fresh() {
    // Modified = remove+re-add under ONE lock hold + ONE bump (font-assets
    // § 2). N cycles via the bytes path's re-register (same composed
    // mechanics; the AssetEvent::Modified arm is exercised in the
    // asset-driven test below).
    let mut app = registry_app();
    app.update();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    app.update();
    let faces_registered = db_face_count(&app);

    let mut dead_ids = Vec::new();
    for _cycle in 0..8 {
        let old = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
        let gen_before = generation(&app);
        app.world_mut()
            .resource_mut::<FontRegistry>()
            .reregister_bytes("Fira Sans", fira_bytes());
        app.update();
        assert_eq!(generation(&app), gen_before + 1, "one bump per cycle");
        assert_eq!(
            db_face_count(&app),
            faces_registered,
            "no growth across cycles"
        );
        let new = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
        assert_ne!(new, old, "re-add issues a fresh ID");
        dead_ids.push(old);
    }
    let fonts = app.world().resource::<SharedFontSystem>();
    let mut guard = fonts.lock();
    for dead in dead_ids {
        assert!(
            guard.get_font(dead, fontdb::Weight(400)).is_none(),
            "every prior cycle's ID is dead — no staleness, no leak"
        );
    }
}

#[test]
fn asset_registration_loading_to_loaded_with_strong_pinning() {
    // The asset path: register against a not-yet-loaded handle → Loading;
    // asset arrives → Loaded + one bump. The registry's strong handle pins
    // the asset even when the caller drops theirs.
    use bevy::asset::AssetPlugin;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();

    let handle = app
        .world()
        .resource::<bevy::asset::Assets<BuiyFont>>()
        .reserve_handle();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset("Fira Sans", handle.clone(), FontFaceDescriptors::default());
    app.update();
    assert_eq!(
        app.world()
            .resource::<FontRegistry>()
            .load_state("Fira Sans"),
        Some(FontLoadState::Loading)
    );

    let gen_before = generation(&app);
    let id = handle.id();
    app.world_mut()
        .resource_mut::<bevy::asset::Assets<BuiyFont>>()
        .insert(id, BuiyFont { data: fira_bytes() })
        .unwrap();
    drop(handle); // caller drops; the registry's strong handle must pin
    // Asset events land next frame; settle two.
    app.update();
    app.update();

    let registry = app.world().resource::<FontRegistry>();
    assert_eq!(
        registry.load_state("Fira Sans"),
        Some(FontLoadState::Loaded)
    );
    assert_eq!(generation(&app), gen_before + 1);
    assert!(
        app.world()
            .resource::<bevy::asset::Assets<BuiyFont>>()
            .get(id)
            .is_some(),
        "strong registry handle pins the asset (the weak-registry footgun, § 3)"
    );
}

#[test]
fn one_asset_aliased_under_two_families_completes_both() {
    // One Handle<BuiyFont> registered under TWO declared families (aliasing
    // one font file as e.g. "Body" and "Heading"). The AssetEvent is
    // consumed once, so the applier must match ALL records backed by the
    // asset — completing only one would strand the other in Loading
    // forever (the 3 s Block window then expires into silent fallback).
    use bevy::asset::AssetPlugin;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();

    let handle = app
        .world()
        .resource::<bevy::asset::Assets<BuiyFont>>()
        .reserve_handle();
    {
        let mut registry = app.world_mut().resource_mut::<FontRegistry>();
        registry.register_asset("Body", handle.clone(), FontFaceDescriptors::default());
        registry.register_asset("Heading", handle.clone(), FontFaceDescriptors::default());
    }
    app.update();
    for family in ["Body", "Heading"] {
        assert_eq!(
            app.world().resource::<FontRegistry>().load_state(family),
            Some(FontLoadState::Loading)
        );
    }

    app.world_mut()
        .resource_mut::<bevy::asset::Assets<BuiyFont>>()
        .insert(handle.id(), BuiyFont { data: fira_bytes() })
        .unwrap();
    app.update();
    app.update();

    let registry = app.world().resource::<FontRegistry>();
    for family in ["Body", "Heading"] {
        assert_eq!(
            registry.load_state(family),
            Some(FontLoadState::Loaded),
            "family {family:?} must complete — every record backed by the asset"
        );
        assert!(!registry.faces(family).is_empty());
    }
}

#[test]
fn asset_modified_is_remove_plus_readd() {
    use bevy::asset::AssetPlugin;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(AssetPlugin::default());
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();

    let id = {
        let mut assets = app
            .world_mut()
            .resource_mut::<bevy::asset::Assets<BuiyFont>>();
        assets.add(BuiyFont { data: fira_bytes() }).id()
    };
    let strong = app
        .world_mut()
        .resource_mut::<bevy::asset::Assets<BuiyFont>>()
        .get_strong_handle(id)
        .unwrap();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_asset("Fira Sans", strong, FontFaceDescriptors::default());
    app.update();
    app.update();
    let old = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];

    // Hot-reload: mutate the asset → AssetEvent::Modified.
    let gen_before = generation(&app);
    app.world_mut()
        .resource_mut::<bevy::asset::Assets<BuiyFont>>()
        .get_mut(id)
        .unwrap()
        .data = fira_bytes();
    app.update();
    app.update();

    let new = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    assert_ne!(new, old, "Modified = remove + re-add (fresh ID)");
    assert_eq!(generation(&app), gen_before + 1, "composed under one bump");
    let fonts = app.world().resource::<SharedFontSystem>();
    assert!(fonts.lock().get_font(old, fontdb::Weight(400)).is_none());
}
