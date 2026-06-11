//! The opt-in background system-font scan (font-assets § 5) and the
//! `FontsGeneration` reshape trigger (architecture § 2.2).
//!
//! The swap machinery is tested with an INJECTED completed task carrying a
//! deterministic database — never a real `load_system_fonts()` driven to
//! completion in-test: the real scan's duration is host-font-dependent
//! (issue #505: ~1.3 s release / 10 s+ debug on font-heavy hosts) and golden
//! determinism forbids CI frames depending on host fonts anyway.

use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use buiy_core::text::{
    BuiyTextPlugin, FontsGeneration, PendingSystemFontScan, SharedFontSystem, registered_fonts_db,
};

fn text_app(system_fonts: bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins); // TaskPoolPlugin inits AsyncComputeTaskPool
    app.add_plugins(BuiyTextPlugin { system_fonts });
    app
}

/// Drive updates until the generation bumps (condition-based waiting, bounded).
fn wait_for_generation(app: &mut App, want: u64) -> bool {
    for _ in 0..2000 {
        app.update();
        if app.world().resource::<FontsGeneration>().0 == want {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

/// The charter test: a completed scan bumps `FontsGeneration` EXACTLY once.
#[test]
fn injected_scan_swap_bumps_generation_exactly_once() {
    let mut app = text_app(false);
    assert_eq!(app.world().resource::<FontsGeneration>().0, 0);

    // Deterministic stand-in for the scan result: the registered baseline
    // itself (what a scan on a zero-system-font host would produce).
    let task = AsyncComputeTaskPool::get().spawn(async move { registered_fonts_db() });
    app.world_mut()
        .insert_resource(PendingSystemFontScan(Some(task)));

    assert!(
        wait_for_generation(&mut app, 1),
        "completed scan must bump FontsGeneration"
    );
    assert!(
        app.world()
            .get_resource::<PendingSystemFontScan>()
            .is_none(),
        "the scan slot is consumed by the swap"
    );
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<FontsGeneration>().0,
        1,
        "exactly once — no repeat bumps after the swap"
    );
}

/// The swap installs the SCANNED database (not a rebuilt baseline) and keeps
/// the registered families resolving (font-assets § 5: the fresh db re-adds
/// every registered binary before scanning). The injected db carries one face
/// MORE than the construction baseline, so a broken swap that discards the
/// scan result and rebuilds `registered_fonts_db()` fails the face count.
#[cfg(feature = "default_font")]
#[test]
fn swap_installs_the_scanned_db_and_keeps_the_registered_baseline() {
    use std::sync::Arc;

    use cosmic_text::fontdb::{Family, Query, Source};

    let mut app = text_app(false);
    let faces_before = {
        let fonts = app.world().resource::<SharedFontSystem>();
        let guard = fonts.lock();
        guard.db().faces().count()
    };
    assert_eq!(faces_before, 1, "baseline is exactly the embedded face");

    // Stand-in scan result: the registered baseline PLUS one extra binary
    // face — distinguishable from anything `registered_fonts_db()` rebuilds.
    static EXTRA_FACE: &[u8] = include_bytes!("../assets/fonts/FiraSans-Regular-latin.ttf");
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut db = registered_fonts_db();
        db.load_font_source(Source::Binary(Arc::new(EXTRA_FACE)));
        db
    });
    app.world_mut()
        .insert_resource(PendingSystemFontScan(Some(task)));
    assert!(wait_for_generation(&mut app, 1));

    let fonts = app.world().resource::<SharedFontSystem>();
    let guard = fonts.lock();
    assert_eq!(
        guard.db().faces().count(),
        2,
        "the SCANNED db (baseline + 1 extra face) is live, not a rebuilt baseline"
    );
    let resolved = guard.db().query(&Query {
        families: &[Family::SansSerif],
        ..Query::default()
    });
    assert!(resolved.is_some(), "sans-serif still resolves post-swap");
}

/// font-assets § 5 + T5: the swap re-adds every registered `Source::Binary` —
/// on the MAIN THREAD at apply time (T5 plan decision 6: an in-task re-add
/// would lose fonts registered DURING the scan). Fresh db ⇒ `FontDbLineage`
/// advances together with `FontsGeneration`, and the registry's recorded
/// face IDs are re-recorded against the new db.
#[test]
fn scan_swap_readds_registry_sources_rerecords_ids_and_bumps_lineage() {
    use std::sync::Arc;

    use buiy_core::text::{FontDbLineage, FontFaceDescriptors, FontRegistry};
    use cosmic_text::fontdb::Source;

    let mut app = text_app(false);
    app.update();
    let bytes: Arc<Vec<u8>> = Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .expect("embedded font artifact exists"),
    );
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", bytes.clone(), FontFaceDescriptors::default());
    app.update();
    let old_face = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    let gen_before = app.world().resource::<FontsGeneration>().0;
    let lineage_before = app.world().resource::<FontDbLineage>().0;

    // Injected completed scan (the module-doc discipline): a deterministic
    // fresh db stands in for `load_system_fonts()`. It carries one EXTRA
    // binary face so the two lineages allocate DIFFERENTLY: the re-add lands
    // at a shifted slot, making old/new face IDs unequal by value. (With the
    // identical baseline-only allocation, the old ID would be live-by-value
    // in the fresh db and every assert below would pass even with
    // `record_faces` deleted — Orientation fact 1.)
    let extra = bytes.clone();
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut db = registered_fonts_db();
        db.load_font_source(Source::Binary(extra));
        db
    });
    app.world_mut()
        .insert_resource(PendingSystemFontScan(Some(task)));
    assert!(wait_for_generation(&mut app, gen_before + 1));

    assert_eq!(
        app.world().resource::<FontDbLineage>().0,
        lineage_before + 1,
        "fresh db = lineage advance (every fontdb ID reissued)"
    );
    let new_face = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    // The shifted slot guarantees inequality, so this fails if the registry
    // still holds the OLD lineage's ID (i.e. record_faces never ran):
    assert_ne!(new_face, old_face, "faces re-recorded against the fresh db");
    let fonts = app.world().resource::<SharedFontSystem>();
    assert!(
        fonts.lock().db().face(new_face).is_some(),
        "re-recorded ID is live"
    );
    // And the registered bytes survived INTO the fresh db: the injected
    // scan result is baseline + 1 extra face, so the re-add is the only
    // thing that can grow it to baseline + 2.
    assert_eq!(
        fonts.lock().db().len(),
        registered_fonts_db().len() + 2,
        "registered face re-added into the fresh db (baseline + extra + re-add)"
    );
}

/// `swap_font_db` carries the OLD engine's locale through `into_locale_and_db`
/// (font-assets § 3.1) — pinned to a sentinel no host's sys-locale ever
/// reports, so re-deriving via sys-locale (or keeping the placeholder's
/// "en-US") fails on EVERY host, not just non-en-US ones.
#[test]
fn swap_carries_the_locale_through() {
    use std::sync::{Arc, Mutex};

    use buiy_core::text::{BuiyFallback, FontRegistry, swap_font_db};
    use cosmic_text::FontSystem;

    let fonts = SharedFontSystem(Arc::new(Mutex::new(
        FontSystem::new_with_locale_and_db_and_fallback(
            String::from("zz-ZZ"),
            registered_fonts_db(),
            BuiyFallback,
        ),
    )));

    swap_font_db(&fonts, registered_fonts_db(), &mut FontRegistry::default());

    let guard = fonts.lock();
    assert_eq!(
        guard.locale(),
        "zz-ZZ",
        "the swap carries the pre-swap locale, never re-deriving it"
    );
}

/// OFF by default: no scan slot, no bump, ever (font-assets § 5).
#[test]
fn default_plugin_never_scans_or_bumps() {
    let mut app = text_app(false);
    for _ in 0..5 {
        app.update();
    }
    assert!(
        app.world()
            .get_resource::<PendingSystemFontScan>()
            .is_none()
    );
    assert_eq!(app.world().resource::<FontsGeneration>().0, 0);
}

/// The opt-in flag spawns the background scan. Poll-agnostic on purpose: by
/// the time we look, the task is either still pending or already applied —
/// we never WAIT on the real host scan (see module doc).
#[test]
fn system_fonts_flag_spawns_the_background_scan() {
    let mut app = text_app(true);
    app.update(); // Startup schedule runs the spawn
    let pending = app
        .world()
        .get_resource::<PendingSystemFontScan>()
        .is_some();
    let generation = app.world().resource::<FontsGeneration>().0;
    assert!(
        pending || generation == 1,
        "the flag must kick off the scan (pending={pending}, generation={generation})"
    );
}
