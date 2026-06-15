//! Task 2.10 — the BiDi caret round-trip predicate (#6) driven through the
//! PRODUCTION text stack (`MinimalPlugins + CorePlugin + LayoutPlugin +
//! BuiyTextPlugin`), the same path as `buiy_core`'s `text_shaping_snapshots`.
//!
//! - `prop_bidi_caret_roundtrips` runs the predicate over generated
//!   mixed-direction strings (the `arb_bidi_text` space).
//! - The MANDATORY mutation tests: the six shaping-corpus scripts (Latin,
//!   Arabic, Devanagari, CJK, emoji-ZWJ, mixed-BiDi) are known-good CONTROLS
//!   (`Ok`); an off-by-one caret-map fixture is REJECTED (`Err`) — proving the
//!   round-trip relation has teeth.
//!
//! Closes gate #12.

use std::sync::Arc;

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyTextPlugin, FamilyEntry, FontFaceDescriptors, FontFamily, FontRegistry, FontSize,
    FontStack, GenericFamily, TextBuffer,
};
use buiy_verify::invariant::{arb_bidi_text, bidi_caret_roundtrips, caret_in_cluster};
use cosmic_text::{Buffer, Cursor};
use proptest::prelude::*;

// --- shaping through the production stack ------------------------------------

/// A committed fixture font shared with `buiy_core`'s shaping corpus, read from
/// that crate's fixtures dir (stable workspace layout, same as the snapshot
/// test's hard-coded `tests/fixtures/fonts`).
fn fixture_font_bytes(file_name: &str) -> Arc<Vec<u8>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../buiy_core/tests/fixtures/fonts")
        .join(file_name);
    Arc::new(
        std::fs::read(&path).unwrap_or_else(|e| panic!("fixture font {file_name} missing ({e})")),
    )
}

/// One fixture face: (declared family, file under the shared fonts dir).
type FixtureFont = (&'static str, &'static str);

const ARABIC: FixtureFont = ("Noto Sans Arabic", "NotoSansArabic-arabic.ttf");
const HEBREW: FixtureFont = ("Noto Sans Hebrew", "NotoSansHebrew-hebrew.ttf");
const DEVANAGARI: FixtureFont = ("Noto Sans Devanagari", "NotoSansDevanagari-devanagari.ttf");
const CJK: FixtureFont = ("Noto Sans CJK SC", "NotoSansCJKsc-han.otf");
const EMOJI: FixtureFont = ("Noto Emoji", "NotoEmoji-emoji.ttf");

/// Shape `text` through the production stack with `fonts` registered and
/// `families` as the resolver stack; return the committed `cosmic_text::Buffer`.
fn shape(text: &str, fonts: &[FixtureFont], families: &[FamilyEntry]) -> Buffer {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();

    for (family, file) in fonts {
        app.world_mut()
            .resource_mut::<FontRegistry>()
            .register_bytes(
                *family,
                fixture_font_bytes(file),
                FontFaceDescriptors::default(),
            );
        app.update();
    }

    let entity = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            Style::default().width_px(400.0).height_px(200.0),
            buiy_core::text::Text(String::from(text)),
            FontFamily(FontStack(families.to_vec())),
            FontSize(20.0),
        ))
        .id();
    for _ in 0..4 {
        app.update();
    }

    app.world()
        .get::<TextBuffer>(entity)
        .expect("the fixture entity synced a TextBuffer")
        .buffer
        .clone()
}

fn sans() -> Vec<FamilyEntry> {
    vec![FamilyEntry::Generic(GenericFamily::SansSerif)]
}

fn named(name: &str) -> Vec<FamilyEntry> {
    vec![FamilyEntry::Named(String::from(name))]
}

/// The Latin (Fira Sans) + Hebrew-fixture stack — first-strong LTR with an RTL
/// block, the genuine BiDi mix (mirrors the corpus's `BIDI_STACK`).
fn bidi_stack() -> Vec<FamilyEntry> {
    vec![
        FamilyEntry::Named(String::from("Fira Sans")),
        FamilyEntry::Named(String::from("Noto Sans Hebrew")),
    ]
}

// --- #6 proptest over generated mixed-direction text -------------------------

proptest! {
    #![proptest_config(ProptestConfig { cases: 48, max_shrink_iters: 1024, ..ProptestConfig::default() })]

    /// The caret round-trip holds over generated LTR/RTL-mixed strings. Shaping
    /// drives a full Bevy app, so the case count is lower than the pure-CPU
    /// predicates (still hundreds of caret round-trips per case).
    #[test]
    fn prop_bidi_caret_roundtrips(text in arb_bidi_text(3, 5)) {
        // Hebrew fixture + Fira Sans cover the generated scripts.
        let buffer = shape(&text, &[HEBREW], &bidi_stack());
        prop_assert!(
            bidi_caret_roundtrips(&buffer).is_ok(),
            "text {:?}: {}", text, bidi_caret_roundtrips(&buffer).unwrap_err()
        );
    }
}

// --- the six shaping-corpus scripts as known-good controls -------------------

#[test]
fn control_latin() {
    let b = shape("Sphinx of black quartz, judge my vow.", &[], &sans());
    assert!(
        bidi_caret_roundtrips(&b).is_ok(),
        "{:?}",
        bidi_caret_roundtrips(&b)
    );
}

#[test]
fn control_arabic() {
    let b = shape("السلام عليكم", &[ARABIC], &named("Noto Sans Arabic"));
    assert!(
        bidi_caret_roundtrips(&b).is_ok(),
        "{:?}",
        bidi_caret_roundtrips(&b)
    );
}

#[test]
fn control_devanagari() {
    let b = shape("नमस्ते क्षत्रिय", &[DEVANAGARI], &named("Noto Sans Devanagari"));
    assert!(
        bidi_caret_roundtrips(&b).is_ok(),
        "{:?}",
        bidi_caret_roundtrips(&b)
    );
}

#[test]
fn control_cjk() {
    let b = shape("你好，世界", &[CJK], &named("Noto Sans CJK SC"));
    assert!(
        bidi_caret_roundtrips(&b).is_ok(),
        "{:?}",
        bidi_caret_roundtrips(&b)
    );
}

#[test]
fn control_emoji_zwj() {
    let b = shape(
        "👨\u{200D}👩\u{200D}👧\u{200D}👦",
        &[EMOJI],
        &named("Noto Emoji"),
    );
    assert!(
        bidi_caret_roundtrips(&b).is_ok(),
        "{:?}",
        bidi_caret_roundtrips(&b)
    );
}

#[test]
fn control_mixed_bidi() {
    let b = shape("hello עולם world", &[HEBREW], &bidi_stack());
    assert!(
        bidi_caret_roundtrips(&b).is_ok(),
        "{:?}",
        bidi_caret_roundtrips(&b)
    );
}

// --- the off-by-one mutation: prove the round-trip relation has teeth --------

/// The off-by-one caret-map fixture: feed the #6a relation-check a cursor one
/// byte PAST a single-byte cluster's end. The true round-trip recovers a cursor
/// inside `[start, end]` (accepted); the off-by-one cursor falls outside and is
/// REJECTED — proving the relation is a real identity, not vacuous.
#[test]
fn off_by_one_caret_map_is_rejected() {
    // A single-byte ASCII cluster: `[start, start]` (end == start for a
    // 1-codepoint Latin glyph). `caret_in_cluster` accepts the true start and
    // rejects start + 1.
    let (line, start, end) = (0usize, 3usize, 3usize);

    // True round-trip lands ON the cluster → Ok.
    assert!(
        caret_in_cluster(Cursor::new(line, start), line, start, end).is_ok(),
        "the cluster's own start round-trips"
    );

    // Off-by-one (start + 1) lands past the cluster → Err (the teeth).
    assert!(
        caret_in_cluster(Cursor::new(line, start + 1), line, start, end).is_err(),
        "a caret mapped one byte off the cluster must be rejected"
    );

    // Wrong LINE is also rejected.
    assert!(
        caret_in_cluster(Cursor::new(line + 1, start), line, start, end).is_err(),
        "a caret on the wrong line is rejected"
    );

    // And the real shaper output passes the full predicate (control).
    let buffer = shape("hello עולם world", &[HEBREW], &bidi_stack());
    assert!(
        bidi_caret_roundtrips(&buffer).is_ok(),
        "{:?}",
        bidi_caret_roundtrips(&buffer)
    );
}
