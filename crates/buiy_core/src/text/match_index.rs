//! The resolver's lock-free substrate (T5 plan decision 2): a
//! `fontdb::Database` CLONE snapshot + lazily extracted per-face coverage.
//! Same-lineage slotmap keys are identical across clones, so snapshot IDs
//! are valid against the live engine until the next index reset (every
//! engine mutation site resets the snapshot under its own lock hold:
//! `apply_font_registry`, `apply_system_font_scan`).
//!
//! Coverage comes from the face's cmap via the RE-EXPORTED skrifa
//! (`cosmic_text::skrifa`) through `Database::with_face_data` (`&self` —
//! handles Binary/SharedFile/File sources): `Font::unicode_codepoints()`
//! is gated behind the non-default `monospace_fallback` feature (returns
//! `&[]` under Buiy's default-features pin — T5 erratum 5), and
//! `Font::new` rejects `Source::File`. No new dependency.

use std::collections::HashMap;

use bevy::prelude::*;
use cosmic_text::fontdb;
use cosmic_text::skrifa::{prelude::*, raw::FontRef};

/// Sorted, deduped codepoint set — binary-search membership.
struct CoverageSet(Vec<u32>);

impl CoverageSet {
    fn contains(&self, c: char) -> bool {
        self.0.binary_search(&(c as u32)).is_ok()
    }
}

/// The main-world match substrate the `FontStack` resolver (Task 6) probes
/// from `TextSync` — zero locks, zero `FontSystem`.
#[derive(Resource)]
pub struct FontMatchIndex {
    db: fontdb::Database,
    coverage: HashMap<fontdb::ID, CoverageSet>,
}

impl FontMatchIndex {
    pub fn new(db: fontdb::Database) -> Self {
        Self {
            db,
            coverage: HashMap::new(),
        }
    }

    /// Re-snapshot after an IN-LINEAGE engine mutation (`apply_font_registry`
    /// — the db is carried, never replaced): swap the db clone in and prune
    /// coverage of dead IDs. Survivors keep their sets: within one lineage an
    /// ID names one face forever and dead IDs never alias (pinned in
    /// `text_fontdb_semantics`).
    pub fn reset_in_lineage(&mut self, db: fontdb::Database) {
        self.coverage.retain(|id, _| db.face(*id).is_some());
        self.db = db;
    }

    /// Re-snapshot after a FRESH-LINEAGE swap (`apply_system_font_scan` —
    /// `FontDbLineage` advances): drop the coverage cache entirely, mirroring
    /// `FontKeyInterner::begin_lineage`. A fresh database reissues EQUAL ID
    /// values for DIFFERENT faces in insertion order (Orientation fact 1,
    /// pinned in `text_fontdb_semantics`), so the in-lineage liveness prune
    /// would RETAIN old faces' sets — and dead-ID probes' cached empty sets —
    /// under IDs that now name other faces, and `covers` would answer from
    /// the wrong cmap forever.
    pub fn reset_fresh(&mut self, db: fontdb::Database) {
        self.coverage.clear();
        self.db = db;
    }

    /// fontdb's real CSS matcher, on the snapshot.
    pub fn query(&self, query: &fontdb::Query) -> Option<fontdb::ID> {
        self.db.query(query)
    }

    /// Does `id` cover `c`? Extracts the face's cmap on first probe
    /// (`with_face_data` + skrifa charmap), cached for the face's lifetime
    /// in this index. A face that fails to parse covers nothing.
    pub fn covers(&mut self, id: fontdb::ID, c: char) -> bool {
        if !self.coverage.contains_key(&id) {
            let set = self
                .db
                .with_face_data(id, |data, face_index| {
                    let font = FontRef::from_index(data, face_index).ok()?;
                    let mut cps: Vec<u32> = font.charmap().mappings().map(|(cp, _)| cp).collect();
                    cps.sort_unstable();
                    cps.dedup();
                    Some(cps)
                })
                .flatten()
                .unwrap_or_default();
            self.coverage.insert(id, CoverageSet(set));
        }
        self.coverage[&id].contains(c)
    }
}
