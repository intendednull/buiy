//! T2.10 (audit #25): the `BuiyFallback::script_fallback` net, guarded at two
//! tiers.
//!
//! `font_system.rs::script_fallback` maps `Script::Arabic`/`Hebrew`/
//! `Devanagari`/`Han` to their covering Noto families as PURE DATA. Every
//! existing non-Latin test NAMES the covering font in the stack, so the
//! resolver wins on a coverage HIT and `FontFallbackIter` (the consumer of
//! `script_fallback`) never decides the face. A broken match-arm wiring or a
//! family-name typo would therefore regress SILENTLY against the old corpus.
//!
//! ## Two complementary guards (why both)
//!
//! 1. **Pure data, per arm** (`script_fallback_arm_*`). The exact regression
//!    the audit names — a typo'd or mis-wired family string — is a property of
//!    the pure `Fallback::script_fallback` function. Asserting each arm's
//!    returned slice catches it deterministically: a `"Noto Sans Arabik"` typo
//!    fails ONLY the Arabic arm's case, with no font setup at all.
//!
//! 2. **Live integration, per arm** (`*_run_resolves_through_fallback`). Shape
//!    a run of each script with a stack that names ONLY a non-covering Latin
//!    font, with just that script's Noto fixture registered, and assert the
//!    glyphs resolve to the Noto face (real, non-`.notdef`). This proves the
//!    live shaping pipeline is healthy end-to-end: font registration reaches the
//!    engine db, the generation/reshape fires, and a run whose covering font is
//!    NOT in the named stack still gets real glyph coverage. It does NOT, on its
//!    own, prove that `BuiyFallback` (or `script_fallback`) is the path taken —
//!    see below.
//!
//! ### Why guard (2) cannot stand alone — and what it does NOT prove
//!
//! cosmic-text's `FontFallbackIter` tries, in order: the stack, then
//! `script_fallback`, then `common_fallback`, then a FULL-DB backstop scan
//! over every registered face (the `other_i` arm — see
//! `cosmic-text/src/font/fallback/mod.rs`). The per-script fixture subsets are
//! deliberately DISJOINT (each covers only its own script), so a script's only
//! covering face is its Noto fixture, and the iterator lands on it REGARDLESS of
//! whether `script_fallback` — or `BuiyFallback` at all — contributed: via step
//! 2 when the arm is correct, via the step-4 backstop otherwise. So guard (2)
//! cannot catch a typo'd arm, and cannot even prove `BuiyFallback` is installed
//! (removing it entirely leaves these tests green — the backstop still resolves
//! the lone covering face). Guard (1), the pure per-arm data assertion, is the
//! actual typo/wiring guard. Together: (1) pins `script_fallback`'s data; (2)
//! pins that the live registration→reshape→coverage pipeline works. Together
//! they cover finding #25.

mod support;

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyFallback, BuiyTextPlugin, FamilyEntry, FontFamily, FontSize, FontStack, SharedFontSystem,
    Text, TextBuffer,
};
use cosmic_text::Fallback;
use unicode_script::Script;

/// One script arm: the script, the Noto family `script_fallback` maps it to
/// (verbatim — the subset's declared name, which the engine matches by), the
/// covering fixture file, and a short run of that script.
struct Arm {
    script: Script,
    noto_family: &'static str,
    fixture_file: &'static str,
    text: &'static str,
}

const ARABIC: Arm = Arm {
    script: Script::Arabic,
    noto_family: "Noto Sans Arabic",
    fixture_file: "NotoSansArabic-arabic.ttf",
    text: "السلام",
};
const HEBREW: Arm = Arm {
    script: Script::Hebrew,
    noto_family: "Noto Sans Hebrew",
    fixture_file: "NotoSansHebrew-hebrew.ttf",
    text: "עולם",
};
const DEVANAGARI: Arm = Arm {
    script: Script::Devanagari,
    noto_family: "Noto Sans Devanagari",
    fixture_file: "NotoSansDevanagari-devanagari.ttf",
    text: "नमस्ते",
};
const HAN: Arm = Arm {
    script: Script::Han,
    noto_family: "Noto Sans CJK SC",
    fixture_file: "NotoSansCJKsc-han.otf",
    text: "你好",
};

// --- guard 1: the pure per-arm data ----------------------------------------

/// Assert one `script_fallback` arm names exactly the expected Noto family.
/// The locale is irrelevant to the per-script arms (they ignore it), so any
/// value works.
fn assert_arm_names_family(arm: &Arm) {
    assert_eq!(
        BuiyFallback.script_fallback(arm.script, "en-US"),
        &[arm.noto_family],
        "the {:?} script_fallback arm must name exactly {:?} (a typo or a \
         mis-wired arm regresses the non-Latin net silently)",
        arm.script,
        arm.noto_family,
    );
}

#[test]
fn script_fallback_arm_arabic_names_noto_sans_arabic() {
    assert_arm_names_family(&ARABIC);
}

#[test]
fn script_fallback_arm_hebrew_names_noto_sans_hebrew() {
    assert_arm_names_family(&HEBREW);
}

#[test]
fn script_fallback_arm_devanagari_names_noto_sans_devanagari() {
    assert_arm_names_family(&DEVANAGARI);
}

#[test]
fn script_fallback_arm_han_names_noto_sans_cjk_sc() {
    assert_arm_names_family(&HAN);
}

#[test]
fn script_fallback_unmapped_script_is_empty() {
    // The `_ => &[]` arm: a script with no per-script pin defers to
    // common_fallback / the backstop, never naming a stray family. Latin is
    // the canonical unmapped case (covered by the stack + common_fallback).
    assert!(
        BuiyFallback
            .script_fallback(Script::Latin, "en-US")
            .is_empty(),
        "unmapped scripts must return no per-script family"
    );
}

// --- guard 2: the live end-to-end integration ------------------------------

/// The Latin-only stack: the embedded `Fira Sans` subset, which has no
/// Arabic/Hebrew/Devanagari/Han coverage. Naming it (and ONLY it) forces the
/// resolver to find no coverage winner, so the run is shaped through the
/// fallback machinery rather than a direct stack match.
fn latin_only_stack() -> FontStack {
    FontStack(vec![FamilyEntry::Named(String::from("Fira Sans"))])
}

/// The headless text app. Mirrors the shaping-snapshot harness rather than the
/// Phase-1 `support::headless_text_app`: this test registers a fixture font
/// through the production `FontRegistry::register_bytes` path
/// (`support::register_fixture_font`), and the embedded `Fira Sans` it relies
/// on as the non-covering stack font is present without an `AssetPlugin`.
fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.update(); // settle plugin init
    app
}

/// Shape `arm.text` under a stack that names ONLY the non-covering Latin font,
/// with the arm's Noto fixture registered, and return the declared family
/// names the committed glyphs actually resolved to (the faces behind
/// `glyph.font_id`) plus whether any glyph is `.notdef` (gid 0) and the total
/// glyph count.
fn resolved_face_families(arm: &Arm) -> (Vec<String>, bool, usize) {
    let mut app = text_app();
    // Register ONLY the covering Noto face — never named in the stack.
    support::register_fixture_font(&mut app, arm.noto_family, arm.fixture_file);

    let entity = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            Style::default().width_px(400.0).height_px(200.0),
            Text(String::from(arm.text)),
            FontFamily(latin_only_stack()),
            FontSize(20.0),
        ))
        .id();
    // register_fixture_font already settled one frame; settle the spawn +
    // shaping (the shaping-snapshot harness's 3-update text settle).
    for _ in 0..3 {
        app.update();
    }

    let buffer = app
        .world()
        .get::<TextBuffer>(entity)
        .expect("the fixture entity synced a TextBuffer");

    let mut face_ids: Vec<cosmic_text::fontdb::ID> = Vec::new();
    let mut saw_notdef = false;
    let mut glyph_total = 0usize;
    for run in buffer.buffer.layout_runs() {
        for glyph in run.glyphs {
            glyph_total += 1;
            if glyph.glyph_id == 0 {
                saw_notdef = true;
            }
            if !face_ids.contains(&glyph.font_id) {
                face_ids.push(glyph.font_id);
            }
        }
    }

    // Map each face ID to its declared family name off the live engine
    // (read-only; the test owns the app, so this lock contends with nothing).
    let fonts = app.world().resource::<SharedFontSystem>().clone();
    let guard = fonts.lock();
    let families = face_ids
        .iter()
        .map(|id| {
            guard
                .db()
                .face(*id)
                .expect("committed glyph faces are live in the engine db")
                .families[0]
                .0
                .clone()
        })
        .collect();
    drop(guard);

    (families, saw_notdef, glyph_total)
}

/// The shared integration assertion for one arm: the run shaped real glyphs
/// that resolved to the expected Noto face — NOT `.notdef`/tofu, and NOT the
/// non-covering Latin stack font. (Proves the live registration→reshape→coverage
/// pipeline is healthy; it does NOT attribute the result to `BuiyFallback` vs
/// cosmic-text's full-DB backstop — the per-arm data tests are the typo/wiring
/// guard. See the module doc.)
fn assert_arm_resolves_live(arm: &Arm) {
    let (families, saw_notdef, glyph_total) = resolved_face_families(arm);
    assert!(
        glyph_total > 0,
        "{} shaped no glyphs at all",
        arm.noto_family
    );
    assert!(
        !saw_notdef,
        "{}: a glyph shaped to .notdef (gid 0) — the fallback never produced \
         real coverage",
        arm.noto_family
    );
    assert!(
        families.iter().any(|f| f == arm.noto_family),
        "{}: glyphs resolved to {:?}, not the expected Noto face — registration \
         never reached the engine, or the reshape did not fire",
        arm.noto_family,
        families,
    );
    assert!(
        !families.iter().any(|f| f == "Fira Sans"),
        "{}: a glyph stayed on the non-covering Latin stack font (Fira Sans) \
         — the run was not routed through fallback at all",
        arm.noto_family,
    );
}

#[test]
fn arabic_run_resolves_through_fallback() {
    assert_arm_resolves_live(&ARABIC);
}

#[test]
fn hebrew_run_resolves_through_fallback() {
    assert_arm_resolves_live(&HEBREW);
}

#[test]
fn devanagari_run_resolves_through_fallback() {
    assert_arm_resolves_live(&DEVANAGARI);
}

#[test]
fn han_run_resolves_through_fallback() {
    assert_arm_resolves_live(&HAN);
}
