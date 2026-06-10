//! Opt-in background system-font discovery (font-assets § 5) and the
//! rebuild swap (font-assets § 3.1) that merges it in.

use std::mem;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use cosmic_text::{FontSystem, fontdb};

use super::font_system::{BuiyFallback, FontsGeneration, SharedFontSystem, registered_fonts_db};

/// The in-flight system-font scan, if any. Inserted by
/// [`spawn_system_font_scan`] (the `BuiyTextPlugin { system_fonts: true }`
/// startup path) and consumed by [`apply_system_font_scan`]. Public so tests
/// (and advanced apps) can inject a prebuilt database task; absent in the
/// steady state — the poll system is inert without it.
#[derive(Resource)]
pub struct PendingSystemFontScan(pub Option<Task<fontdb::Database>>);

/// Spawn the scan on `AsyncComputeTaskPool`: a FRESH database built from the
/// registered baseline (the embedded default font + family pins; T5's
/// `FontRegistry` adds every registered `Source::Binary`) plus
/// `load_system_fonts()`. First paint never waits on this — the issue-#505
/// cost (~1.3 s release) stays off the startup path.
pub fn spawn_system_font_scan(mut commands: Commands) {
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut db = registered_fonts_db();
        db.load_system_fonts();
        db
    });
    commands.insert_resource(PendingSystemFontScan(Some(task)));
}

/// Poll the pending scan; on completion, swap the rebuilt database in and
/// bump [`FontsGeneration`] EXACTLY once (architecture § 2.2 — the reshape
/// trigger `TextSync` consumes from T2). Runs before `BuiySet::Layout` so a
/// completed swap is visible to the same frame's layout pass.
pub fn apply_system_font_scan(
    pending: Option<ResMut<PendingSystemFontScan>>,
    fonts: Res<SharedFontSystem>,
    mut generation: ResMut<FontsGeneration>,
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

    swap_font_db(&fonts, db);
    generation.0 += 1; // exactly one bump per completed scan
}

/// The font-assets § 3.1 rebuild pattern, under ONE lock hold so no other
/// world ever observes the placeholder: carry the locale through
/// `into_locale_and_db`, rebuild over the new database with the same
/// deterministic [`BuiyFallback`]. `new_with_locale_and_db*` does no
/// filesystem scan, so the swap itself is cheap. Every fontdb ID is
/// reissued by a rebuild — `AtlasKey`s are never persisted across one
/// (font-assets § 3.2; enforced by the T4 producer, not here).
pub fn swap_font_db(fonts: &SharedFontSystem, db: fontdb::Database) {
    let mut guard = fonts.lock();
    let old = mem::replace(&mut *guard, placeholder_font_system());
    let (locale, _discarded_db) = old.into_locale_and_db();
    *guard = FontSystem::new_with_locale_and_db_and_fallback(locale, db, BuiyFallback);
}

/// Briefly parked in the mutex during [`swap_font_db`]'s `mem::replace`;
/// never observable (the swap completes under the same lock hold).
fn placeholder_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db(String::from("en-US"), fontdb::Database::new())
}
