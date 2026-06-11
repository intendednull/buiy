//! THE fontdb ID-stability pins (font-assets § 3.2, corrected — see the T5
//! plan's Orientation fact 1 and erratum 1): fontdb `ID`s are slotmap keys.
//! Within one `Database` value ("lineage") surviving faces keep their IDs
//! across `remove_face` and dead IDs never alias; across DIFFERENT
//! `Database` instances equal ID values name different faces. These tests
//! are drift tripwires for any fontdb bump — if one fails after an upgrade,
//! the FontKeyInterner lineage mechanics below it are what's at stake.

use std::sync::Arc;

use buiy_core::text::{FontKeyInterner, registered_fonts_db};
use cosmic_text::fontdb;

/// The embedded default font bytes, loaded twice to get two distinct faces
/// in one db (fontdb does NOT dedup sources — verified, Orientation table).
fn two_face_db() -> (fontdb::Database, fontdb::ID, fontdb::ID) {
    let mut db = registered_fonts_db();
    let first = db.faces().next().expect("embedded face").id;
    let bytes: Arc<Vec<u8>> = Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .expect("embedded font artifact exists"),
    );
    let ids = db.load_font_source(fontdb::Source::Binary(bytes));
    assert_eq!(ids.len(), 1);
    (db, first, ids[0])
}

#[test]
fn surviving_ids_are_stable_across_remove_face() {
    // The § 3.1 unregister path: into_locale_and_db carries the SAME
    // Database, so removal of one face must leave every other ID valid.
    let (mut db, keep, remove) = two_face_db();
    assert!(db.face(keep).is_some() && db.face(remove).is_some());
    db.remove_face(remove);
    assert!(db.face(keep).is_some(), "surviving face keeps its ID");
    assert!(db.face(remove).is_none(), "removed ID is dead");
}

#[test]
fn dead_ids_never_alias_within_a_lineage() {
    // Slot reuse bumps the slotmap version: a re-added face gets a NEW id,
    // and the dead id keeps returning None forever — in-lineage interner
    // entries can never serve the wrong face.
    let (mut db, _keep, remove) = two_face_db();
    db.remove_face(remove);
    let bytes: Arc<Vec<u8>> = Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .unwrap(),
    );
    let readded = db.load_font_source(fontdb::Source::Binary(bytes))[0];
    assert_ne!(readded, remove, "re-add issues a fresh ID");
    assert!(db.face(remove).is_none(), "the dead ID stays dead");
    assert!(db.face(readded).is_some());
}

#[test]
fn fresh_databases_reissue_equal_id_values_for_different_faces() {
    // THE aliasing hazard (Orientation fact 1, erratum 1): two fresh
    // databases hand out the same (slot, version) key values in insertion
    // order, so ID equality is meaningless across lineages. This is the
    // fact FontDbLineage + the interner clear exist for.
    let db_a = registered_fonts_db();
    let db_b = registered_fonts_db();
    let a = db_a.faces().next().unwrap().id;
    let b = db_b.faces().next().unwrap().id;
    assert_eq!(a, b, "equal ID values across independent databases");
    // (Same bytes here, but nothing about the VALUE says so — a system
    // scan puts arbitrary faces in these slots.)
}

#[test]
fn interner_clears_per_lineage_but_never_reuses_seats() {
    let (db, first, second) = two_face_db();
    drop(db);
    let mut interner = FontKeyInterner::default();
    assert_eq!(interner.intern(first), 0);
    assert_eq!(interner.intern(second), 1);
    assert_eq!(interner.intern(first), 0, "idempotent within a lineage");

    // Lineage 1 → 2: the map clears (old IDs are meaningless now)…
    assert!(interner.begin_lineage(2));
    assert!(!interner.begin_lineage(2), "same lineage = no-op");
    assert_eq!(interner.len(), 0, "map cleared");
    // …but seats stay monotonic: the same ID VALUE re-interned after the
    // clear gets a FRESH u32 — never seat 0/1, which may still name
    // grace-resident atlas entries of the OLD faces. (The as-built
    // len()-as-next allocation would hand back 0 here — the aliasing bug
    // this test exists to prevent.)
    assert_eq!(interner.intern(first), 2);
    assert_eq!(interner.intern(second), 3);
}

#[test]
fn lineage_resource_defaults_to_zero() {
    use buiy_core::text::FontDbLineage;
    assert_eq!(FontDbLineage::default().0, 0);
}
