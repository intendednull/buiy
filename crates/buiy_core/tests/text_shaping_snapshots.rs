//! The multi-script shaping-snapshot corpus (verification §§ 1.2, 2.2):
//! one fixture per F-tier shaping claim — Latin, Arabic (joining/RTL),
//! Devanagari (reordering), CJK, emoji-ZWJ, mixed-BiDi — each pinning
//! `(line_i, glyph_id, font_seat, x, y)` per glyph against the committed
//! per-script OFL fixture fonts. The text analogue of the gate-#5 layout
//! snapshots; breadth beyond one-behavior-per-fixture is upstream's job.
//!
//! Every fixture runs through the FULL production stack —
//! `register_bytes` → `apply_font_registry` → resolver → `TextSync` →
//! `TextCommit` — never a hand-built `FontSystem`: the corpus proves the
//! resolver+registry+engine seam, not cosmic-text in isolation.
//!
//! Snapshot lines are `line_i glyph_id font_seat x y` where `font_seat` is
//! the index of `glyph.font_id` in the fixture's first-seen-order face list
//! (fontdb IDs are NOT stable across processes — seats are; the header maps
//! seat → family name) and `y` is the run baseline plus the glyph offset,
//! so multi-line fixtures show monotonically increasing `y`.
//!
//! Update workflow (the golden `--accept` analogue, human-curated):
//!   BUIY_ACCEPT_SHAPING=1 cargo test -p buiy_core --test text_shaping_snapshots
//! then REVIEW THE DIFF before committing — a snapshot change is a shaping
//! change. `BUIY_DEBUG_SHAPING=1` (with `--nocapture`) dumps the full glyph
//! stream including the cluster start bytes the snapshot omits.

mod support;

use std::fmt::Write as _;
use std::path::PathBuf;

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyTextPlugin, FamilyEntry, FontFamily, FontSize, FontStack, GenericFamily, SharedFontSystem,
    Text, TextBuffer, TextDirection,
};
use cosmic_text::fontdb;
use unicode_script::{Script, UnicodeScript};

// --- the fixture table ------------------------------------------------------

/// A committed fixture font: (declared family, file under
/// `tests/fixtures/fonts/`) — both verbatim from the Task 8 subsets.
type FixtureFont = (&'static str, &'static str);

const ARABIC: FixtureFont = ("Noto Sans Arabic", "NotoSansArabic-arabic.ttf");
const HEBREW: FixtureFont = ("Noto Sans Hebrew", "NotoSansHebrew-hebrew.ttf");
const DEVANAGARI: FixtureFont = ("Noto Sans Devanagari", "NotoSansDevanagari-devanagari.ttf");
const CJK: FixtureFont = ("Noto Sans CJK SC", "NotoSansCJKsc-han.otf");
const EMOJI: FixtureFont = ("Noto Emoji", "NotoEmoji-emoji.ttf");

/// Const-constructible stack entry (`FamilyEntry::Named` owns a `String`,
/// so the corpus table cannot hold `FamilyEntry` directly).
#[derive(Clone, Copy)]
enum Entry {
    Named(&'static str),
    Sans,
}

const SANS: &[Entry] = &[Entry::Sans];
const ARABIC_STACK: &[Entry] = &[Entry::Named("Noto Sans Arabic")];
const DEVA_STACK: &[Entry] = &[Entry::Named("Noto Sans Devanagari")];
const CJK_STACK: &[Entry] = &[Entry::Named("Noto Sans CJK SC")];
const EMOJI_STACK: &[Entry] = &[Entry::Named("Noto Emoji")];
/// Fira first is load-bearing (the `coverage_miss_splits_spans` shape): the
/// Hebrew span exists only on a genuine coverage HIT against the fixture.
const BIDI_STACK: &[Entry] = &[Entry::Named("Fira Sans"), Entry::Named("Noto Sans Hebrew")];

struct Fixture {
    name: &'static str,
    fonts: &'static [FixtureFont],
    stack: &'static [Entry],
    text: &'static str,
    dir: Option<TextDirection>,
}

#[rustfmt::skip]
const CORPUS: &[Fixture] = &[
    Fixture { name: "latin",      fonts: &[],           stack: SANS,         text: "Sphinx of black quartz, judge my vow.", dir: None },
    Fixture { name: "arabic",     fonts: &[ARABIC],     stack: ARABIC_STACK, text: "السلام عليكم", dir: None },
    Fixture { name: "devanagari", fonts: &[DEVANAGARI], stack: DEVA_STACK,   text: "नमस्ते क्षत्रिय", dir: None },
    Fixture { name: "cjk",        fonts: &[CJK],        stack: CJK_STACK,    text: "你好，世界", dir: None },
    Fixture { name: "emoji_zwj",  fonts: &[EMOJI],      stack: EMOJI_STACK,  text: "👨\u{200D}👩\u{200D}👧\u{200D}👦", dir: None },
    Fixture { name: "mixed_bidi", fonts: &[HEBREW],     stack: BIDI_STACK,   text: "hello עולם world", dir: None },
];

fn fixture(name: &str) -> &'static Fixture {
    CORPUS
        .iter()
        .find(|f| f.name == name)
        .expect("fixture name is in CORPUS")
}

// --- the harness ------------------------------------------------------------

struct Glyph {
    glyph_id: u16,
    seat: usize,
    x: f32,
    /// Run baseline + glyph offset (buffer space, logical px).
    y: f32,
    /// Cluster start byte in the run's original line text.
    start: usize,
}

struct Run {
    line_i: usize,
    rtl: bool,
    /// The original line text (the cluster bytes index into it).
    text: String,
    glyphs: Vec<Glyph>,
}

struct Shaped {
    /// Seat → declared family name, in first-seen glyph order.
    seats: Vec<String>,
    runs: Vec<Run>,
}

impl Shaped {
    fn glyph_count(&self) -> usize {
        self.runs.iter().map(|run| run.glyphs.len()).sum()
    }
}

/// Drive one fixture through the production stack and fold the committed
/// buffer's `layout_runs()` into process-stable seats + glyph records.
fn shape(fixture: &Fixture) -> Shaped {
    shape_text(fixture, fixture.text)
}

/// [`shape`] with the fixture's fonts/stack over different text — the
/// devanagari reorder probe shapes auxiliary strings under the same setup.
fn shape_text(fixture: &Fixture, text: &str) -> Shaped {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update();
    for (family, file) in fixture.fonts {
        support::register_fixture_font(&mut app, family, file);
    }

    let stack = FontStack(
        fixture
            .stack
            .iter()
            .map(|entry| match entry {
                Entry::Named(name) => FamilyEntry::Named(String::from(*name)),
                Entry::Sans => FamilyEntry::Generic(GenericFamily::SansSerif),
            })
            .collect(),
    );
    let mut spawned = app.world_mut().spawn((
        buiy_core::Node,
        Style::default().width_px(400.0).height_px(200.0),
        Text(String::from(text)),
        FontFamily(stack),
        FontSize(20.0),
    ));
    if let Some(dir) = fixture.dir {
        spawned.insert(dir);
    }
    let entity = spawned.id();
    for _ in 0..3 {
        app.update();
    }

    let buffer = app
        .world()
        .get::<TextBuffer>(entity)
        .expect("the fixture entity synced a TextBuffer");
    let mut face_seats: Vec<fontdb::ID> = Vec::new();
    let mut runs = Vec::new();
    for run in buffer.buffer.layout_runs() {
        let mut glyphs = Vec::with_capacity(run.glyphs.len());
        for glyph in run.glyphs {
            let seat = face_seats
                .iter()
                .position(|id| *id == glyph.font_id)
                .unwrap_or_else(|| {
                    face_seats.push(glyph.font_id);
                    face_seats.len() - 1
                });
            glyphs.push(Glyph {
                glyph_id: glyph.glyph_id,
                seat,
                x: glyph.x,
                y: run.line_y + glyph.y,
                start: glyph.start,
            });
        }
        runs.push(Run {
            line_i: run.line_i,
            rtl: run.rtl,
            text: String::from(run.text),
            glyphs,
        });
    }
    assert!(
        runs.iter().any(|run| !run.glyphs.is_empty()),
        "fixture `{}` shaped no glyphs",
        fixture.name
    );

    if std::env::var_os("BUIY_DEBUG_SHAPING").is_some() {
        for run in &runs {
            eprintln!(
                "[{}] line {} rtl={} text={:?}",
                fixture.name, run.line_i, run.rtl, run.text
            );
            for glyph in &run.glyphs {
                eprintln!(
                    "  gid={} seat={} start={} x={:.2} y={:.2}",
                    glyph.glyph_id, glyph.seat, glyph.start, glyph.x, glyph.y
                );
            }
        }
    }

    // Seat → family name, off the live engine (read-only; the test owns the
    // app, so this lock contends with nothing).
    let fonts = app.world().resource::<SharedFontSystem>().clone();
    let guard = fonts.lock();
    let seats = face_seats
        .iter()
        .map(|id| {
            guard
                .db()
                .face(*id)
                .expect("committed glyph faces are live")
                .families[0]
                .0
                .clone()
        })
        .collect();
    drop(guard);

    Shaped { seats, runs }
}

// --- the snapshot compare (decision 11: committed text fixtures + accept env)

fn render_snapshot(fixture: &Fixture, shaped: &Shaped) -> String {
    let mut out = String::new();
    writeln!(out, "# buiy shaping snapshot: {}", fixture.name).unwrap();
    writeln!(out, "# text: {:?}", fixture.text).unwrap();
    for (seat, family) in shaped.seats.iter().enumerate() {
        writeln!(out, "# seat {seat} = {family}").unwrap();
    }
    writeln!(out, "# line_i glyph_id font_seat x y").unwrap();
    for run in &shaped.runs {
        for glyph in &run.glyphs {
            writeln!(
                out,
                "{} {} {} {:.2} {:.2}",
                run.line_i, glyph.glyph_id, glyph.seat, glyph.x, glyph.y
            )
            .unwrap();
        }
    }
    out
}

fn snapshot_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/shaping")
        .join(format!("{name}.snap"))
}

/// Labeled line-by-line diff (decision 11's rejected runner-up was `insta`;
/// corpus files are tens of lines, so a positional compare reads fine).
fn print_diff(name: &str, expected: &str, actual: &str) {
    eprintln!("--- tests/fixtures/shaping/{name}.snap (expected)");
    eprintln!("+++ shaped output (actual)");
    let expected: Vec<&str> = expected.lines().collect();
    let actual: Vec<&str> = actual.lines().collect();
    for i in 0..expected.len().max(actual.len()) {
        match (expected.get(i), actual.get(i)) {
            (Some(e), Some(a)) if e == a => eprintln!("  {e}"),
            (e, a) => {
                if let Some(e) = e {
                    eprintln!("- {e}");
                }
                if let Some(a) = a {
                    eprintln!("+ {a}");
                }
            }
        }
    }
}

fn assert_snapshot(name: &str, actual: &str) {
    let path = snapshot_path(name);
    if std::env::var_os("BUIY_ACCEPT_SHAPING").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "no snapshot committed for `{name}` ({e}); run \
             BUIY_ACCEPT_SHAPING=1 cargo test -p buiy_core --test \
             text_shaping_snapshots, REVIEW the generated file, and commit it"
        )
    });
    // Normalize CRLF defensively: the writer always emits LF and .gitattributes
    // pins `*.snap -text`, but a checkout that eol-converted anyway (e.g. a
    // pre-.gitattributes clone with core.autocrlf=true — the PR #56
    // windows-latest failure) must not diverge every snapshot.
    let expected = expected.replace("\r\n", "\n");
    if expected != actual {
        print_diff(name, &expected, actual);
        panic!(
            "shaping snapshot `{name}` diverged — a snapshot change is a \
             shaping change; if intended, regenerate with \
             BUIY_ACCEPT_SHAPING=1, review the diff, and commit"
        );
    }
}

/// Script of the source character a glyph's cluster starts at.
fn script_at(run: &Run, start: usize) -> Script {
    run.text[start..]
        .chars()
        .next()
        .expect("cluster start indexes a char boundary")
        .script()
}

// --- the corpus: structural invariants + the snapshot pin -------------------
//
// Each fixture asserts hand-derivable invariants that hold regardless of the
// snapshot bytes FIRST (an honest failure names the broken shaping property,
// not a byte diff), then pins the exact glyph stream.

#[test]
fn latin_single_seat_x_strictly_increasing() {
    let fixture = fixture("latin");
    let shaped = shape(fixture);
    assert_eq!(
        shaped.seats.len(),
        1,
        "pure-ASCII text under the default sans pin uses exactly one face"
    );
    for run in &shaped.runs {
        for pair in run.glyphs.windows(2) {
            assert!(
                pair[0].x < pair[1].x,
                "no reorder: x strictly increases within a latin line"
            );
        }
    }
    assert_snapshot(fixture.name, &render_snapshot(fixture, &shaped));
}

#[test]
fn arabic_joins_and_reads_rtl() {
    let fixture = fixture("arabic");
    let shaped = shape(fixture);
    assert!(
        shaped.runs.iter().all(|run| run.rtl),
        "an all-Arabic paragraph is first-strong RTL on every run"
    );
    // The joining witness: the repeated lam (3×: medial in السلام twice,
    // عليكم once) shapes to position-dependent DISTINCT glyph ids — dumb 1:1
    // cmap rendering (Shaping::Basic, a missing Arabic shaper) would repeat
    // one nominal glyph. NOT the plan's `glyph count < char count`: Noto
    // Sans Arabic builds lam-alef from contextual PART glyphs (rlig chain
    // contexts + type-1/2 lookups, no count-reducing type-4 ligature), and
    // ccmp ADDS a dots glyph for medial yeh — counts go UP, not down.
    let lam_glyphs: std::collections::BTreeSet<u16> = shaped
        .runs
        .iter()
        .flat_map(|run| {
            run.glyphs
                .iter()
                .filter(|glyph| run.text[glyph.start..].starts_with('ل'))
                .map(|glyph| glyph.glyph_id)
        })
        .collect();
    assert!(
        lam_glyphs.len() >= 2,
        "contextual joining happened: repeated lam shaped to ≥2 distinct \
         glyph ids, got {lam_glyphs:?}"
    );
    assert_eq!(
        shaped.seats,
        vec!["Noto Sans Arabic"],
        "every glyph sits on the Arabic fixture face"
    );
    assert_snapshot(fixture.name, &render_snapshot(fixture, &shaped));
}

#[test]
fn devanagari_forms_conjuncts_and_reorders_the_i_matra() {
    let fixture = fixture("devanagari");
    let shaped = shape(fixture);
    assert_eq!(
        shaped.seats,
        vec!["Noto Sans Devanagari"],
        "every glyph sits on the Devanagari fixture face"
    );

    // The conjunct witness: क्ष (ka + virama + ssa, 3 codepoints) forms the
    // akhand ligature — ONE glyph whose merged cluster spans ≥3 chars.
    let run = &shaped.runs[0];
    let mut starts: Vec<usize> = run.glyphs.iter().map(|glyph| glyph.start).collect();
    starts.dedup();
    let conjunct = starts.windows(2).any(|cluster| {
        let (lo, hi) = (cluster[0], cluster[1]);
        run.glyphs.iter().filter(|glyph| glyph.start == lo).count() == 1
            && run.text[lo..hi].chars().count() >= 3
    });
    assert!(
        conjunct,
        "akhand conjunct formed: a single glyph covers a ≥3-codepoint cluster"
    );

    // The reordering witness, by glyph IDENTITY — the plan's `start byte > a
    // later glyph's` can never fire: harfrust's default MonotoneGraphemes
    // cluster level makes the Indic shaper MERGE clusters across the
    // reordered syllable (ot_shaper_indic.rs merge_clusters), so starts stay
    // monotone. Instead: standalone "त" names the consonant's glyph; in
    // "ति" (logically consonant-then-matra) that glyph must land visually
    // SECOND — the i-matra moved in front of it.
    let ta = shape_text(fixture, "त");
    let ti = shape_text(fixture, "ति");
    assert_eq!(ta.glyph_count(), 1, "standalone त is one glyph");
    let ta_glyph = ta.runs[0].glyphs[0].glyph_id;
    let ti_glyphs = &ti.runs[0].glyphs;
    assert_eq!(ti_glyphs.len(), 2, "ति is consonant + i-matra");
    let (first, second) = (&ti_glyphs[0], &ti_glyphs[1]);
    assert!(first.x < second.x, "layout_runs yields LTR visual order");
    assert_eq!(
        second.glyph_id, ta_glyph,
        "the consonant sits visually second…"
    );
    assert_ne!(
        first.glyph_id, ta_glyph,
        "…behind the reordered i-matra (the § 2.2 reordering witness)"
    );

    assert_snapshot(fixture.name, &render_snapshot(fixture, &shaped));
}

#[test]
fn cjk_maps_one_glyph_per_codepoint() {
    let fixture = fixture("cjk");
    let shaped = shape(fixture);
    assert_eq!(
        shaped.glyph_count(),
        fixture.text.chars().count(),
        "every CJK codepoint produced exactly one glyph"
    );
    assert_eq!(
        shaped.seats,
        vec!["Noto Sans CJK SC"],
        "every glyph sits on the CJK fixture face"
    );
    assert_snapshot(fixture.name, &render_snapshot(fixture, &shaped));
}

#[test]
fn emoji_zwj_sequence_ligates() {
    let fixture = fixture("emoji_zwj");
    let shaped = shape(fixture);
    let (glyphs, scalars) = (shaped.glyph_count(), fixture.text.chars().count());
    assert!(
        glyphs > 0,
        "the monochrome fixture shapes to paintable Mask glyphs (decision 12)"
    );
    assert!(
        glyphs < scalars,
        "ZWJ ligation collapsed the family sequence: {glyphs} glyphs < {scalars} scalars"
    );
    assert_snapshot(fixture.name, &render_snapshot(fixture, &shaped));
}

#[test]
fn mixed_bidi_reorders_the_hebrew_block() {
    let fixture = fixture("mixed_bidi");
    let shaped = shape(fixture);
    assert!(
        shaped.seats.len() >= 2,
        "two faces shaped the line: the latin face + the Hebrew fixture"
    );
    assert!(
        shaped.runs.iter().all(|run| !run.rtl),
        "first-strong is 'hello' — the paragraph stays LTR"
    );

    let run = &shaped.runs[0];
    // The Hebrew glyphs by SOURCE SCRIPT (not seat: the Common spaces that
    // joined the Hebrew resolver span shape on the Hebrew face too, but sit
    // outside the reordered block).
    let mut hebrew: Vec<&Glyph> = run
        .glyphs
        .iter()
        .filter(|glyph| script_at(run, glyph.start) == Script::Hebrew)
        .collect();
    assert!(hebrew.len() >= 2, "עולם shaped to multiple Hebrew glyphs");
    hebrew.sort_by(|a, b| a.x.total_cmp(&b.x));
    for pair in hebrew.windows(2) {
        assert!(
            pair[0].start > pair[1].start,
            "the visual-reorder witness: ascending x ↔ descending logical \
             byte offset within the RTL block"
        );
    }
    let (min_x, max_x) = (hebrew.first().unwrap().x, hebrew.last().unwrap().x);
    for glyph in &run.glyphs {
        if script_at(run, glyph.start) != Script::Hebrew {
            assert!(
                glyph.x < min_x || glyph.x > max_x,
                "the Hebrew block is visually CONTIGUOUS — no latin glyph \
                 interleaves it"
            );
        }
    }
    assert_snapshot(fixture.name, &render_snapshot(fixture, &shaped));
}
