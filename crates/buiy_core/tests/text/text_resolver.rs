//! FontMatchIndex: the lock-free resolver substrate (T5 plan decision 2) —
//! a fontdb::Database CLONE (same-lineage IDs valid for the live engine)
//! plus lazily extracted per-face coverage (skrifa charmap via
//! with_face_data; Font::unicode_codepoints is feature-gated EMPTY under
//! the default-features pin — Orientation) — and the `FontStack` resolver
//! built on it (font-assets § 6; T5 plan decision 7): per-codepoint stack
//! walk, coverage span-splitting, unicode-range filtering, generic entries
//! terminal, stack-miss → first entry.

use std::ops::Range;
use std::sync::Arc;

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyTextPlugin, ComputedTextLayout, FamilyEntry, FontFaceDescriptors, FontFamily,
    FontMatchIndex, FontRegistry, FontStack, GenericFamily, ResolvedFamily, ResolvedSpan, Text,
    TextBuffer, TextSyncAppliedCount, UnicodeRanges, registered_fonts_db, resolve_spans,
};
use cosmic_text::fontdb::{self, Family, Query};

#[test]
fn query_on_the_snapshot_is_fontdbs_real_matcher() {
    let db = registered_fonts_db();
    let expected = db.faces().next().unwrap().id;
    let index = FontMatchIndex::new(db);
    let hit = index.query(&Query {
        families: &[Family::Name("Fira Sans")],
        weight: fontdb::Weight(400),
        ..Default::default()
    });
    assert_eq!(hit, Some(expected));
    assert_eq!(
        index.query(&Query {
            families: &[Family::SansSerif],
            ..Default::default()
        }),
        Some(expected),
        "generic pins resolve through the snapshot too"
    );
    assert_eq!(
        index.query(&Query {
            families: &[Family::Name("No Such Family")],
            ..Default::default()
        }),
        None
    );
}

#[test]
fn coverage_is_lazily_extracted_and_cached() {
    let db = registered_fonts_db();
    let face = db.faces().next().unwrap().id;
    let mut index = FontMatchIndex::new(db);
    // The latin subset covers ASCII…
    assert!(index.covers(face, 'A'));
    assert!(index.covers(face, 'é'), "U+00E9 is in the latin-1 range");
    // …and not Hebrew/Arabic/CJK.
    assert!(!index.covers(face, 'ע'));
    assert!(!index.covers(face, 'م'));
    assert!(!index.covers(face, '你'));
    // Second probe = pure cache hit (no observable side effect to assert
    // beyond not panicking; the laziness contract is the with_face_data
    // call count, which has no public counter — documented, not asserted).
    assert!(index.covers(face, 'A'));
}

#[test]
fn in_lineage_reset_prunes_dead_coverage_and_swaps_the_snapshot() {
    let mut db = registered_fonts_db();
    let face = db.faces().next().unwrap().id;
    let mut index = FontMatchIndex::new(db.clone());
    assert!(index.covers(face, 'A'));
    db.remove_face(face);
    index.reset_in_lineage(db);
    assert!(!index.covers(face, 'A'), "dead ID: no face, no coverage");
    assert_eq!(
        index.query(&Query {
            families: &[Family::Name("Fira Sans")],
            ..Default::default()
        }),
        None
    );
}

/// THE cross-lineage aliasing tripwire (Orientation fact 1, pinned by
/// `text_fontdb_semantics::fresh_databases_reissue_equal_id_values_for_different_faces`):
/// a fresh db reissues equal ID values for different faces, so a
/// liveness-against-the-new-db prune would RETAIN sets cached under the old
/// lineage's IDs. Cache an EMPTY set by probing a dead ID, then reset onto a
/// fresh db where that same VALUE is live — coverage must be re-extracted
/// from the new face's cmap, never served from the stale cache.
#[test]
fn fresh_lineage_reset_drops_all_cached_coverage() {
    let mut db_a = registered_fonts_db();
    let face = db_a.faces().next().unwrap().id;
    db_a.remove_face(face);
    let mut index = FontMatchIndex::new(db_a);
    assert!(!index.covers(face, 'A'), "dead ID covers nothing (cached)");

    let db_b = registered_fonts_db();
    assert_eq!(
        db_b.faces().next().unwrap().id,
        face,
        "the fresh db reissues the same ID value for its first face"
    );
    index.reset_fresh(db_b);
    assert!(
        index.covers(face, 'A'),
        "fresh-lineage reset re-extracts coverage — a retained stale empty \
         set would make this live face cover nothing forever"
    );
}

// --- the resolver walk (T5 plan decision 7) --------------------------------

fn fira_bytes() -> Arc<Vec<u8>> {
    Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/fonts/FiraSans-Regular-latin.ttf"
        ))
        .unwrap(),
    )
}

/// MinimalPlugins + text, NO AssetPlugin (the text_registry.rs fixture
/// shape) — the resolver substrate must work asset-machinery-free.
fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// Registry + index built ENTIRELY through the production App path (no
/// test-only constructors — testing-anti-patterns): register the given
/// records via the bytes path, settle, lift the resources out for the
/// pure-resolver tests.
fn index_and_registry(
    registrations: Vec<(&str, Arc<Vec<u8>>, FontFaceDescriptors)>,
) -> (FontMatchIndex, FontRegistry) {
    let mut app = text_app();
    app.update();
    for (family, bytes, descriptors) in registrations {
        app.world_mut()
            .resource_mut::<FontRegistry>()
            .register_bytes(family, bytes, descriptors);
    }
    app.update();
    let index = app
        .world_mut()
        .remove_resource::<FontMatchIndex>()
        .expect("BuiyTextPlugin inserts the index");
    let registry = app
        .world_mut()
        .remove_resource::<FontRegistry>()
        .expect("BuiyTextPlugin inits the registry");
    (index, registry)
}

fn named(name: &str) -> FamilyEntry {
    FamilyEntry::Named(String::from(name))
}

fn sans() -> FamilyEntry {
    FamilyEntry::Generic(GenericFamily::SansSerif)
}

fn span(range: Range<usize>, family: ResolvedFamily) -> ResolvedSpan {
    ResolvedSpan { range, family }
}

fn named_span(range: Range<usize>, name: &str) -> ResolvedSpan {
    span(range, ResolvedFamily::Named(String::from(name)))
}

fn generic_span(range: Range<usize>) -> ResolvedSpan {
    span(range, ResolvedFamily::Generic(GenericFamily::SansSerif))
}

#[test]
fn named_entry_wins_for_covered_codepoints() {
    // Stack ["Fira Sans", sans-serif] over pure ASCII: one span, Named —
    // the embedded face covers every char, so the walk never reaches the
    // generic.
    let (mut index, registry) = index_and_registry(vec![]);
    let stack = FontStack(vec![named("Fira Sans"), sans()]);
    let resolution = resolve_spans("hello world", &stack, 400, &registry, &mut index, 0.0);
    assert!(!resolution.blocked);
    assert_eq!(resolution.spans, vec![named_span(0..11, "Fira Sans")]);
}

#[test]
fn generic_entry_is_terminal() {
    // Stack [sans-serif] over ANY content: one span, Generic — no coverage
    // probe, no split (the generic is the author's catch-all; per-glyph
    // gaps are FontFallbackIter's job below the stack). The Hebrew here is
    // NOT in the latin subset — a coverage probe would have split.
    let (mut index, registry) = index_and_registry(vec![]);
    let stack = FontStack(vec![sans()]);
    let text = "abc עברית";
    let resolution = resolve_spans(text, &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(resolution.spans, vec![generic_span(0..text.len())]);
}

#[test]
fn coverage_miss_splits_spans() {
    // Stack ["Fira Sans", "Noto Sans Hebrew"] over "abc עברית xyz" (the
    // Hebrew fixture covers Hebrew only): Latin → Fira (first entry),
    // Hebrew → the fixture (second entry) — THREE spans, adjacent
    // same-winner merged, the Common spaces joining their current span.
    // Fira first is load-bearing: the Hebrew span exists only on a genuine
    // coverage HIT — a fixture that fails to cover Hebrew falls through to
    // first_entry (Fira) and collapses the result to ONE span.
    let hebrew_bytes = Arc::new(
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/fonts/NotoSansHebrew-hebrew.ttf"
        ))
        .expect("Task 8 commits the Hebrew fixture subset"),
    );
    let (mut index, registry) = index_and_registry(vec![(
        "Noto Sans Hebrew",
        hebrew_bytes,
        FontFaceDescriptors::default(),
    )]);
    let stack = FontStack(vec![named("Fira Sans"), named("Noto Sans Hebrew")]);
    let resolution = resolve_spans("abc עברית xyz", &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(
        resolution.spans,
        vec![
            named_span(0..4, "Fira Sans"),
            named_span(4..15, "Noto Sans Hebrew"),
            named_span(15..18, "Fira Sans"),
        ]
    );
}

#[test]
fn unicode_range_filters_per_codepoint() {
    // "Fira Sans" registered with unicode_range = U+0041..=U+005A
    // (uppercase only); stack ["Fira Sans", sans-serif] over "aAbB":
    // lowercase → Generic(sans-serif), uppercase → Named — the § 6.1
    // per-character CSS semantics via the same span machinery.
    let (mut index, registry) = index_and_registry(vec![(
        "Fira Sans",
        fira_bytes(),
        FontFaceDescriptors {
            unicode_range: Some(UnicodeRanges::new(vec![0x41..=0x5A])),
            ..Default::default()
        },
    )]);
    let stack = FontStack(vec![named("Fira Sans"), sans()]);
    let resolution = resolve_spans("aAbB", &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(
        resolution.spans,
        vec![
            generic_span(0..1),
            named_span(1..2, "Fira Sans"),
            generic_span(2..3),
            named_span(3..4, "Fira Sans"),
        ]
    );
}

#[test]
fn common_and_inherited_never_split() {
    // Spaces and punctuation (Common) and combining marks (Inherited) join
    // the current span — the HarfBuzz itemization rule; the prepended
    // LRM/RLM (Common) never fragments either, and leading ones attach to
    // the first resolved span.
    let (mut index, registry) = index_and_registry(vec![]);
    let stack = FontStack(vec![named("Fira Sans"), sans()]);
    let resolution = resolve_spans("ab cd!", &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(resolution.spans, vec![named_span(0..6, "Fira Sans")]);

    let marked = "\u{200E}ab cd!";
    let resolution = resolve_spans(marked, &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(
        resolution.spans,
        vec![named_span(0..marked.len(), "Fira Sans")]
    );

    let combining = "ae\u{0301}b"; // U+0301 COMBINING ACUTE ACCENT: Inherited
    let resolution = resolve_spans(combining, &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(
        resolution.spans,
        vec![named_span(0..combining.len(), "Fira Sans")]
    );
}

#[test]
fn stack_missed_falls_to_first_entry() {
    // No entry covers Hebrew (a named-only stack over the latin subset):
    // the span resolves to the FIRST entry and shaping's FontFallbackIter
    // takes over per-glyph (BuiyFallback's deterministic lists) — asserted
    // here only as span assignment; the shaping side is the corpus's job
    // (Task 9).
    let (mut index, registry) = index_and_registry(vec![]);
    let stack = FontStack(vec![named("Fira Sans")]);
    let resolution = resolve_spans("שלום", &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(resolution.spans, vec![named_span(0..8, "Fira Sans")]);

    // All-Common text never probes the stack at all — same first-entry
    // lowering ('∰' is script Common, like every math operator).
    let resolution = resolve_spans("123 ∰", &stack, 400, &registry, &mut index, 0.0);
    assert_eq!(resolution.spans, vec![named_span(0..7, "Fira Sans")]);
}

// --- end-to-end: the resolver through TextSync -----------------------------

fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

fn spawn_text(app: &mut App, text: &str, stack: FontStack) -> Entity {
    app.world_mut()
        .spawn((
            buiy_core::Node,
            Style::default().width_px(300.0).height_px(60.0),
            Text(String::from(text)),
            FontFamily(stack),
        ))
        .id()
}

#[test]
fn registered_family_wins_over_generic_end_to_end() {
    // The font-assets § 10 round-trip: register a family through the
    // production bytes path, stack [that family, serif], settle — every
    // committed LayoutGlyph carries a face declaring that family
    // (glyph.font_id — layout.rs:43). Until Task 8's distinct-family
    // fixtures, the registered face IS the embedded Fira bytes
    // re-registered under their real name, so the assertion is
    // family-membership of every glyph's face, not a specific ID (two
    // identical same-name faces tie in fontdb's find_best_match).
    let mut app = text_app();
    app.update();
    app.world_mut()
        .resource_mut::<FontRegistry>()
        .register_bytes("Fira Sans", fira_bytes(), FontFaceDescriptors::default());
    let entity = spawn_text(
        &mut app,
        "hello",
        FontStack(vec![
            named("Fira Sans"),
            FamilyEntry::Generic(GenericFamily::Serif),
        ]),
    );
    settle(&mut app);

    let registered = app.world().resource::<FontRegistry>().faces("Fira Sans")[0];
    let buffer = app.world().get::<TextBuffer>(entity).unwrap();
    let glyph_faces: Vec<fontdb::ID> = buffer
        .buffer
        .layout_runs()
        .flat_map(|run| run.glyphs.iter().map(|glyph| glyph.font_id))
        .collect();
    assert!(!glyph_faces.is_empty(), "the entity shaped and committed");
    let fonts = app
        .world()
        .resource::<buiy_core::text::SharedFontSystem>()
        .clone();
    let guard = fonts.lock();
    for face in &glyph_faces {
        let info = guard.db().face(*face).expect("glyph face is live");
        assert!(
            info.families.iter().any(|(name, _)| name == "Fira Sans"),
            "every glyph carries a Fira Sans face (the resolved Named win)"
        );
    }
    assert!(
        guard.db().face(registered).is_some(),
        "the registered face is live in the engine"
    );
    drop(guard);
}

#[test]
fn single_span_resolution_uses_set_text_path() {
    // A plain ASCII entity under the default stack resolves to ONE span,
    // and the sync path stays the T2 set_text shape: no explicit AttrsList
    // spans (set_rich_text would have added them), and steady-state counts
    // stay at the T4 baseline — zero applies on a no-change frame,
    // ComputedTextLayout byte-identical across extra updates.
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            Style::default().width_px(300.0).height_px(60.0),
            Text(String::from("plain ascii")),
        ))
        .id();
    settle(&mut app);

    let buffer = app.world().get::<TextBuffer>(entity).unwrap();
    assert!(
        buffer.buffer.lines[0].attrs_list().spans().is_empty(),
        "≤1 resolved span lowers via set_text — no AttrsList churn"
    );

    let layout_before = app
        .world()
        .get::<ComputedTextLayout>(entity)
        .unwrap()
        .clone();
    app.update();
    assert_eq!(
        app.world().resource::<TextSyncAppliedCount>().0,
        0,
        "steady state: the resolver runs only inside trigger-gated syncs"
    );
    assert_eq!(
        app.world().get::<ComputedTextLayout>(entity).unwrap(),
        &layout_before,
        "idempotent: no reshape without a trigger"
    );
}
