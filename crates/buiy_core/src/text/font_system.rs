//! The shared cosmic-text engine: `SharedFontSystem`, its registered-only
//! construction over the embedded default font, and the deterministic
//! `BuiyFallback`.
//!
//! Spec: architecture.md §§ 1.1–1.2, 2.1; font-assets.md §§ 4–5.

use std::sync::{Arc, Mutex, MutexGuard};

use bevy::prelude::*;
use cosmic_text::{Fallback, FontSystem, fontdb};
use unicode_script::Script;

/// Family name the embedded default font registers under, and the target of
/// the serif/cursive/fantasy generic-family pins (font-assets § 4). The
/// sans-serif and monospace generics are re-pinned to Geist / Geist Mono
/// when those embed (parity-prototype A1).
pub const DEFAULT_FONT_FAMILY: &str = "Fira Sans";

/// The embedded Geist UI sans family name (parity-prototype A1; widget-catalog
/// values § 4). Declared `name` ID 1 of `Geist-Variable.ttf`, matched verbatim
/// by fontdb (`Family::Name("Geist")`). The variable `wght` axis (100–900)
/// covers every authored weight (400/450/500/600/700) from one face —
/// cosmic-text applies `Attrs.weight` as the `wght` coordinate
/// (`font/system.rs` `variable_weight_match`).
pub const GEIST_SANS_FAMILY: &str = "Geist";

/// The embedded Geist Mono family name (parity-prototype A1). Declared
/// `name` ID 1 of `GeistMono-Variable.ttf`; the target of the monospace
/// generic pin so `GenericFamily::Monospace` resolves to a TRUE monospace
/// (it pinned to Fira Sans before Geist Mono embedded).
pub const GEIST_MONO_FAMILY: &str = "Geist Mono";

/// The embedded deterministic default font: Fira Sans Regular, latin subset,
/// OFL-1.1. Generated ONLY by `tools/fonts/subset_default_font.sh` (which
/// pins the upstream artifact, the subset ranges, and the fonttools version);
/// the license ships alongside at `assets/fonts/OFL-FiraSans.txt`.
#[cfg(feature = "default_font")]
static DEFAULT_FONT_BYTES: &[u8] = include_bytes!("../../assets/fonts/FiraSans-Regular-latin.ttf");

/// The embedded Geist + Geist Mono variable faces (parity-prototype A1).
/// OFL-1.1 (`assets/fonts/OFL-Geist.txt`), sourced verbatim from
/// `vercel/geist-font` (`packages/next/dist/fonts/geist-{sans,mono}/*-Variable.ttf`).
/// One variable face per family — the `wght` axis spans every authored weight,
/// so no per-weight static files. The widget-catalog parity gallery authors
/// runs as `FontFamily(["Geist"])` / `FontFamily(["Geist Mono"])` or via the
/// monospace generic.
#[cfg(feature = "default_font")]
static GEIST_SANS_BYTES: &[u8] = include_bytes!("../../assets/fonts/Geist-Variable.ttf");
#[cfg(feature = "default_font")]
static GEIST_MONO_BYTES: &[u8] = include_bytes!("../../assets/fonts/GeistMono-Variable.ttf");

/// The one cosmic-text `FontSystem`, shared across the main and render worlds
/// (architecture § 1.1). `FontSystem` is verified `Send + Sync` in 0.19, so
/// `Arc<Mutex<_>>` is sound — no `NonSend` pinning. Exactly three sites may
/// lock it (architecture § 1.2): the Taffy measure closure (T3), `TextCommit`
/// shaping (T3), and the glyph producer's atlas-miss closure (T4). Reviewers
/// reject a fourth.
#[derive(Resource, Clone)]
pub struct SharedFontSystem(pub Arc<Mutex<FontSystem>>);

impl SharedFontSystem {
    /// Build the engine: registered fonts only, never a system scan
    /// (architecture § 2.1; `FontSystem::new()`/`new_with_fonts()` both mmap
    /// every system font — issue #505).
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(build_font_system())))
    }

    /// Lock the engine. Panics if a previous holder panicked mid-mutation —
    /// a poisoned `FontSystem` has no safe recovery (fail loud, root-cause).
    pub fn lock(&self) -> MutexGuard<'_, FontSystem> {
        self.0
            .lock()
            .expect("SharedFontSystem poisoned: a text system panicked while holding the lock")
    }
}

impl Default for SharedFontSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// The deterministic app-global last-resort fallback (font-assets §§ 3.1, 5–6):
/// injected at construction in place of cosmic-text's platform-varying
/// `PlatformFallback`, so CI resolution never depends on the host platform.
/// The per-script lists name the per-script OFL fixture families (T5 plan
/// decision 14); absent families are skipped harmlessly, so apps that
/// register their own fonts under those names get deterministic resolution
/// and nothing platform-varying ever enters.
pub struct BuiyFallback;

impl Fallback for BuiyFallback {
    fn common_fallback(&self) -> &[&'static str] {
        // Harmless when `default_font` is off: a fallback name absent from
        // the database is simply skipped.
        &[DEFAULT_FONT_FAMILY]
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn script_fallback(&self, script: Script, _locale: &str) -> &[&'static str] {
        // The subsets' DECLARED family names, verbatim (fontdb matches by
        // name; "Noto Sans CJK SC" is what NotoSansCJKsc-Regular.otf
        // declares — not the google-fonts "Noto Sans SC" rebuild's name).
        match script {
            Script::Arabic => &["Noto Sans Arabic"],
            Script::Hebrew => &["Noto Sans Hebrew"],
            Script::Devanagari => &["Noto Sans Devanagari"],
            Script::Han => &["Noto Sans CJK SC"],
            _ => &[],
        }
    }
}

/// The font-set generation counter (architecture § 2.2): bumped exactly once
/// per font-set change (system-scan swap completion here; runtime asset
/// registration joins in T5). `TextSync`'s trigger set consumes it from T2 —
/// every `TextBuffer` reshapes once against the enriched fallback set, so
/// late fonts never leave stale tofu.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FontsGeneration(pub u64);

/// The font-database LINEAGE counter (T5; font-assets § 3.2 corrected —
/// see the T5 plan's erratum 1): bumped ONLY when a FRESH `fontdb::Database`
/// replaces the engine's (the system-scan swap). In-lineage mutations —
/// registry adds, the § 3.1 unregister rebuild, hot-reload remove+re-add —
/// keep surviving IDs valid (`into_locale_and_db` carries the same
/// `Database` by value; slotmap keys of untouched faces never change) and
/// MUST NOT bump this. Consumers: the render-world `FontKeyInterner` clears
/// its ID map per lineage (equal fontdb ID values name DIFFERENT faces
/// across databases — the AtlasKey-aliasing hazard § 3.2's
/// never-persist rule exists for). Every lineage bump is accompanied by a
/// `FontsGeneration` bump (the reshape + producer-rebuild trigger).
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FontDbLineage(pub u64);

/// The registered-fonts-only database: the embedded default font plus the
/// five generic-family pins (font-assets § 4). This is BOTH the construction
/// baseline and the base every system-scan rebuild re-adds before scanning
/// (font-assets § 5). T5's `FontRegistry` extends it with every registered
/// `Source::Binary`.
pub fn registered_fonts_db() -> fontdb::Database {
    #[allow(unused_mut)] // `mut` is unused only when `default_font` is off.
    let mut db = fontdb::Database::new();
    #[cfg(feature = "default_font")]
    {
        // Fira Sans loads FIRST so `db.faces().next()` stays the embedded
        // default (the resolver/atlas tests key the "first face" off it) and
        // the sans-serif generic keeps resolving to it (app-wide default
        // unchanged — Geist is opt-in by name for the parity gallery).
        let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(DEFAULT_FONT_BYTES)));
        debug_assert_eq!(ids.len(), 1, "the embedded subset is a single-face ttf");

        // parity-prototype A1: embed Geist (UI sans) + Geist Mono. Each is a
        // single VARIABLE face whose `wght` axis covers every authored weight
        // (400/450/500/600/700) — cosmic-text applies `Attrs.weight` as the
        // `wght` coordinate (`font/system.rs` `variable_weight_match`), so one
        // face per family suffices. `load_font_source` parses exactly one face.
        let geist_sans = db.load_font_source(fontdb::Source::Binary(Arc::new(GEIST_SANS_BYTES)));
        debug_assert_eq!(
            geist_sans.len(),
            1,
            "Geist-Variable.ttf is a single-face ttf"
        );
        let geist_mono = db.load_font_source(fontdb::Source::Binary(Arc::new(GEIST_MONO_BYTES)));
        debug_assert_eq!(
            geist_mono.len(),
            1,
            "GeistMono-Variable.ttf is a single-face ttf"
        );

        // Pin ALL FIVE generic families so no generic ever dangles
        // (font-assets § 4). Sans-serif/serif/cursive/fantasy stay on the
        // embedded default (the established app-wide sans default is NOT
        // changed — Geist is reached BY NAME: `Family::Name("Geist")`).
        // Monospace re-pins to Geist Mono: it formerly pointed at Fira Sans
        // (NOT a monospace), so `GenericFamily::Monospace` now resolves to a
        // TRUE monospace face — the task's first-class-monospace requirement.
        db.set_sans_serif_family(DEFAULT_FONT_FAMILY);
        db.set_serif_family(DEFAULT_FONT_FAMILY);
        db.set_cursive_family(DEFAULT_FONT_FAMILY);
        db.set_fantasy_family(DEFAULT_FONT_FAMILY);
        db.set_monospace_family(GEIST_MONO_FAMILY);
    }
    db
}

pub(crate) fn build_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db_and_fallback(
        buiy_locale(),
        registered_fonts_db(),
        BuiyFallback,
    )
}

/// Briefly parked in the mutex during the § 3.1 mem::replace rebuild dance
/// (`swap_font_db`, `apply_font_registry`); never observable — every swap
/// completes under one lock hold.
pub(crate) fn placeholder_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db(String::from("en-US"), fontdb::Database::new())
}

/// The locale handed to the registered-only constructor: the system locale
/// via sys-locale, mirroring cosmic-text's own (private) behavior exactly
/// (architecture § 2.1 "Locale via the sys-locale default feature").
fn buiy_locale() -> String {
    sys_locale::get_locale().unwrap_or_else(|| String::from("en-US"))
}
