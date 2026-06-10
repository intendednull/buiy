//! The embedded deterministic default font artifact (font-assets § 4).
//!
//! These tests read the committed artifact directly (no cargo feature
//! involved): it must parse via fontdb, register exactly one face under the
//! family name the OFL name records carry, and resolve through a
//! generic-family pin inside a registered-only `FontSystem`.

use std::sync::Arc;

use cosmic_text::FontSystem;
use cosmic_text::fontdb::{Database, Family, Query, Source};

static EMBEDDED: &[u8] = include_bytes!("../assets/fonts/FiraSans-Regular-latin.ttf");

#[test]
fn artifact_parses_as_a_single_fira_sans_face() {
    let mut db = Database::new();
    let ids = db.load_font_source(Source::Binary(Arc::new(EMBEDDED)));
    assert_eq!(ids.len(), 1, "the subset is a single-face ttf");
    let face = db.face(ids[0]).expect("face is registered");
    assert!(
        face.families.iter().any(|(name, _)| name == "Fira Sans"),
        "subset retains the family name (OFL name records); got {:?}",
        face.families
    );
}

#[test]
fn font_system_with_artifact_resolves_a_pinned_generic_family() {
    let mut db = Database::new();
    db.load_font_source(Source::Binary(Arc::new(EMBEDDED)));
    db.set_sans_serif_family("Fira Sans");
    let font_system = FontSystem::new_with_locale_and_db(String::from("en-US"), db);
    let resolved = font_system.db().query(&Query {
        families: &[Family::SansSerif],
        ..Query::default()
    });
    assert!(
        resolved.is_some(),
        "Family::SansSerif resolves through the pin to the embedded face"
    );
}
