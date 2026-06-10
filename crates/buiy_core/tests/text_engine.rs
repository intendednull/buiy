//! Headless tests for the `buiy_core::text` engine foundation (T1).
//!
//! Spec: docs/specs/2026-06-09-buiy-text-rendering-design/architecture.md
//! §§ 1–2. No wgpu adapter, no RenderApp — T1 is headless-only.

use bevy::app::SubApp;
use bevy::prelude::*;
use buiy_core::text::{BuiySwashCache, BuiyTextPlugin, SharedFontSystem, register_render_world};
use cosmic_text::FontSystem;
use cosmic_text::fontdb::{Database, Family, Query};

/// The dependency smoke: cosmic-text links, and the registered-only
/// constructor (architecture § 2.1) builds without touching system fonts.
#[test]
fn cosmic_text_constructs_a_registered_only_font_system() {
    let font_system = FontSystem::new_with_locale_and_db(String::from("en-US"), Database::new());
    assert_eq!(font_system.locale(), "en-US");
    assert_eq!(
        font_system.db().len(),
        0,
        "new_with_locale_and_db must not scan system fonts"
    );
}

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// architecture § 2.1: SharedFontSystem exists and is lockable, with ONLY
/// registered fonts resident — no system scan at startup.
#[test]
fn plugin_inserts_lockable_registered_only_font_system() {
    let app = text_app();
    let fonts = app.world().resource::<SharedFontSystem>();
    let guard = fonts.lock();
    assert!(
        !guard.locale().is_empty(),
        "locale is pinned at construction"
    );
    #[cfg(feature = "default_font")]
    assert_eq!(
        guard.db().len(),
        1,
        "exactly the embedded default font — a system scan would add hundreds"
    );
}

/// architecture § 1.1: one FontSystem, two worlds — the render-world resource
/// is an Arc clone of the main-world one, never a second engine
/// (fontdb IDs are stable only within one engine).
#[test]
fn render_world_registration_shares_one_engine() {
    let fonts = SharedFontSystem::default();
    let mut render_app = SubApp::new();
    register_render_world(&mut render_app, &fonts);
    let clone = render_app.world().resource::<SharedFontSystem>();
    assert!(
        std::sync::Arc::ptr_eq(&fonts.0, &clone.0),
        "render world must hold a clone of the SAME Arc"
    );
}

/// architecture § 1.3: the swash cache is a plain render-world resource,
/// registered alongside the SharedFontSystem clone. Its consumer (the glyph
/// producer's uncached-only miss path, lock site #3) arrives in T4.
#[test]
fn render_world_registration_inserts_swash_cache() {
    let fonts = SharedFontSystem::default();
    let mut render_app = SubApp::new();
    register_render_world(&mut render_app, &fonts);
    assert!(
        render_app
            .world()
            .get_resource::<BuiySwashCache>()
            .is_some(),
        "BuiySwashCache must be registered with the render-world text half"
    );
}

/// The campaign charter's determinism test: two constructions on a
/// zero-system-font baseline resolve every default family identically.
/// (The construction path never scans, so this holds on ANY host.)
#[cfg(feature = "default_font")]
#[test]
fn two_constructions_resolve_every_default_family_identically() {
    fn resolved_faces(app: &App) -> Vec<Option<String>> {
        let fonts = app.world().resource::<SharedFontSystem>();
        let guard = fonts.lock();
        let db = guard.db();
        [
            Family::SansSerif,
            Family::Serif,
            Family::Monospace,
            Family::Cursive,
            Family::Fantasy,
            Family::Name("Fira Sans"),
        ]
        .iter()
        .map(|family| {
            db.query(&Query {
                families: std::slice::from_ref(family),
                ..Query::default()
            })
            .and_then(|id| db.face(id))
            .map(|face| face.post_script_name.clone())
        })
        .collect()
    }

    let (app_a, app_b) = (text_app(), text_app());
    let (resolved_a, resolved_b) = (resolved_faces(&app_a), resolved_faces(&app_b));
    assert_eq!(resolved_a, resolved_b, "construction must be deterministic");
    for resolution in &resolved_a {
        assert!(
            resolution.is_some(),
            "every pinned generic family + the named family must resolve; got {resolved_a:?}"
        );
    }
}
