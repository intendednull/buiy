//! Opt-in background system-font discovery (font-assets § 5) and the
//! rebuild swap (font-assets § 3.1) that merges it in.

use std::mem;
use std::sync::Arc;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use cosmic_text::{FontSystem, fontdb};

use super::font_system::{
    BuiyFallback, FontDbLineage, FontsGeneration, SharedFontSystem, placeholder_font_system,
    registered_fonts_db,
};
use super::match_index::FontMatchIndex;
use super::registry::FontRegistry;

/// The in-flight system-font scan, if any. Inserted by
/// [`spawn_system_font_scan`] (the `BuiyTextPlugin { system_fonts: true }`
/// startup path) and consumed by [`apply_system_font_scan`]. Public so tests
/// (and advanced apps) can inject a prebuilt database task; absent in the
/// steady state — the poll system is inert without it.
#[derive(Resource)]
pub struct PendingSystemFontScan(pub Option<Task<fontdb::Database>>);

/// Spawn the scan on `AsyncComputeTaskPool`: a FRESH database built from the
/// registered baseline (the embedded default font + family pins) plus
/// `load_system_fonts()`. Registered `FontRegistry` sources are re-added at
/// APPLY time, on the main thread, under the swap's own lock hold (T5 plan
/// decision 6: an in-task re-add would lose fonts registered DURING the
/// scan). First paint never waits on this — the issue-#505 cost (~1.3 s
/// release) stays off the startup path.
pub fn spawn_system_font_scan(mut commands: Commands) {
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut db = registered_fonts_db();
        db.load_system_fonts();
        db
    });
    commands.insert_resource(PendingSystemFontScan(Some(task)));
}

/// Poll the pending scan; on completion, swap the rebuilt database in
/// (re-adding registry sources — see [`swap_font_db`]), re-snapshot the
/// [`FontMatchIndex`], and bump [`FontsGeneration`] + [`FontDbLineage`]
/// EXACTLY once each (architecture § 2.2 — the reshape trigger `TextSync`
/// consumes from T2; font-assets § 3.2 — a fresh db reissues every fontdb
/// ID, the producer's lineage probe). Runs before `BuiySet::Layout` so a
/// completed swap is visible to the same frame's layout pass.
pub fn apply_system_font_scan(
    pending: Option<ResMut<PendingSystemFontScan>>,
    fonts: Res<SharedFontSystem>,
    mut registry: ResMut<FontRegistry>,
    mut index: ResMut<FontMatchIndex>,
    mut generation: ResMut<FontsGeneration>,
    mut lineage: ResMut<FontDbLineage>,
    mut commands: Commands,
) {
    let Some(mut pending) = pending else { return };
    let Some(task) = pending.0.as_mut() else {
        return;
    };
    let Some(db) = block_on(future::poll_once(&mut *task)) else {
        return;
    };
    // A finished Task must not be polled again: clear the slot immediately
    // (the remove_resource command applies later, at the next sync point).
    pending.0 = None;
    commands.remove_resource::<PendingSystemFontScan>();

    let snapshot = swap_font_db(&fonts, db, &mut registry);
    index.reset_fresh(snapshot); // fresh db: equal ID values name different faces
    generation.0 += 1; // exactly one bump per completed scan
    lineage.0 += 1; // fresh db: every fontdb ID reissued (§ 3.2)
}

/// The font-assets § 3.1/§ 5 swap, under ONE lock hold so no other world
/// ever observes the placeholder: carry the locale through
/// `into_locale_and_db`, rebuild over the new database with the same
/// deterministic [`BuiyFallback`], then RE-ADD every loaded registry source
/// on this thread and re-record the fresh face IDs (T5 decision 6: an
/// in-task re-add would lose fonts registered DURING the scan).
/// `new_with_locale_and_db*` does no filesystem scan, so the swap itself is
/// cheap. Returns a db clone for the [`FontMatchIndex`] reset. Every fontdb
/// ID is reissued by the FRESH db — the caller must bump [`FontsGeneration`]
/// AND [`FontDbLineage`] together (the producer's two probes; `AtlasKey`s
/// are never persisted across a swap — font-assets § 3.2, enforced by the
/// producer's interner reseat, not here).
pub fn swap_font_db(
    fonts: &SharedFontSystem,
    db: fontdb::Database,
    registry: &mut FontRegistry,
) -> fontdb::Database {
    let mut guard = fonts.lock();
    let old = mem::replace(&mut *guard, placeholder_font_system());
    let (locale, _discarded_db) = old.into_locale_and_db();
    *guard = FontSystem::new_with_locale_and_db_and_fallback(locale, db, BuiyFallback);
    let readds: Vec<(String, Arc<Vec<u8>>)> = registry
        .loaded_sources()
        .map(|(family, bytes)| (family.to_owned(), bytes.clone()))
        .collect();
    for (family, bytes) in readds {
        let ids = guard
            .db_mut()
            .load_font_source(fontdb::Source::Binary(bytes));
        registry.record_faces(&family, ids.to_vec());
    }
    guard.db().clone()
}
