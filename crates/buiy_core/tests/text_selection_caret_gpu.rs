//! GPU end-to-end selection + caret tests (T7): real entities through
//! TextSync → TextCommit → render-prep (blink) → extract → the § 4.6
//! splice + glyph draw → pixels. decoration-and-paint §§ 5–6;
//! verification §§ 1.3, 3.1 (fixed_clock drives the blink pair). All
//! #[ignore]: need a wgpu adapter (CLAUDE.md GPU lane).
//!
//! Run: cargo test -p buiy_core --test text_selection_caret_gpu -- --ignored --test-threads=1
//!
//! ## Column classification (re-capture IS the golden — the text_gpu idiom)
//!
//! The fixtures use a chroma-orthogonal triple over the opaque-black clear —
//! `color.selection.bg` = pure red, `color.selection.fg` = pure blue, text =
//! white — so each paint source has an unambiguous pixel signature:
//!
//! - **Selection rect (quad tier, red):** a tall solid box spanning the full
//!   line height; the rows between the line-box top/bottom and the glyph ink
//!   read the exact full-coverage red encode, so "column x is inside a
//!   selection rect" ⇔ "ANY pixel in column x is strong red". Projecting
//!   onto columns and coalescing recovers the rect spans — this refines the
//!   plan sketch's single mid-line-row scan, which glyph ink (painted OVER
//!   the rect) would interrupt into spurious runs.
//! - **Re-tinted ink (glyph tier, blue) composites over the red rect:**
//!   linear `(1−c)·red + c·blue` per coverage `c`, so `b ≥ 180 ∧ r ≤ 150`
//!   (≈ c ≥ 0.7 — stroke interiors at 20 px reach it) reads selected ink and
//!   nothing else: white ink has r ≥ b, red has b ≈ 0.
//! - **Unselected ink (white over black):** gray `r = g = b = encode(c)`;
//!   `min(r,g,b) ≥ 180` rejects every red/blue mix (their g ≈ 0).
//! - **Caret (glyph-tier solid stamp, red):** hard-edged at alpha 1 (no SDF
//!   AA) — a § 3.3-snapped 1-physical-px column of the exact red encode.
#![allow(deprecated)] // perceptual_diff is deprecated; these GPU sites migrate to buiy_verify::metric in Phase 3 (tier-5 goldens).

mod support;

use bevy::math::Rect;
use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::render::color::{ColorToken, SELECTION_BG_TOKEN, SELECTION_FG_TOKEN};
use buiy_core::render::components::{CaretColor, TextColor};
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::render::prepare::BufferUploadStats;
use buiy_core::text::{
    CaretVisual, ComputedTextLayout, FamilyEntry, FontFamily, FontSize, FontStack, SelectionVisual,
    Text,
};
use cosmic_text::Cursor;
use std::borrow::Cow;
use std::ops::Range;
use std::time::Duration;

/// Glyph tint: white — chroma-orthogonal to the red/blue selection pair.
const TEXT_TOKEN: &str = "test.text";
/// Explicit `caret-color` (tier 1 of the § 6.2 chain): pure red.
const CARET_TOKEN: &str = "test.caret";

fn sel_red() -> Color {
    Color::srgb(1.0, 0.0, 0.0)
}

fn sel_blue() -> Color {
    Color::srgb(0.0, 0.0, 1.0)
}

// --- pixel classifiers (module doc: derived from the composite math) --------

/// Full-strength red — the selection rect's glyph-free rows and the caret
/// stamp's interior (both alpha 1 over black or under nothing).
fn is_strong_red(p: [u8; 4]) -> bool {
    p[0] >= 200 && p[1] <= 20 && p[2] <= 20
}

/// Re-tinted (selection fg, pure blue) glyph ink over the red selection
/// rect: coverage ≥ ~0.7 reads b ≥ 180 with the red residual ≤ 150.
fn is_blue_ink(p: [u8; 4]) -> bool {
    p[2] >= 180 && p[0] <= 150
}

/// Unselected white glyph ink over black: an achromatic pixel at ≥ ~0.61
/// coverage. `g ≥ 180` alone already rejects every red/blue composite.
fn is_white_ink(p: [u8; 4]) -> bool {
    p[0] >= 180 && p[1] >= 180 && p[2] >= 180
}

/// Rows (top→bottom) where ANY pixel satisfies `pred`.
fn rows_where(pixels: &[u8], w: u32, h: u32, pred: impl Fn([u8; 4]) -> bool) -> Vec<u32> {
    (0..h)
        .filter(|&y| (0..w).any(|x| pred(support::px(pixels, w, x, y))))
        .collect()
}

/// Columns (left→right) where ANY pixel satisfies `pred`.
fn cols_where(pixels: &[u8], w: u32, h: u32, pred: impl Fn([u8; 4]) -> bool) -> Vec<u32> {
    (0..w)
        .filter(|&x| (0..h).any(|y| pred(support::px(pixels, w, x, y))))
        .collect()
}

/// Coalesce sorted indices into contiguous bands.
fn bands(sorted: &[u32]) -> Vec<Range<u32>> {
    let mut out: Vec<Range<u32>> = Vec::new();
    for &i in sorted {
        match out.last_mut() {
            Some(b) if b.end == i => b.end = i + 1,
            _ => out.push(i..i + 1),
        }
    }
    out
}

// --- 1. the mixed-BiDi ::selection golden (decoration-and-paint § 5) --------

const SEL_W: u32 = 256;
const SEL_H: u32 = 64;

/// The T5 mixed-BiDi corpus line with a selection straddling the BiDi
/// boundary, captured fresh (one app per capture — the re-capture IS the
/// golden). Byte map of `"hello עולם world"`: 0..6 `"hello "`, 6..14 the
/// four 2-byte Hebrew chars `ע ו ל ם`, 14..20 `" world"`. The selection
/// `[10, 18)` covers `ל ם " wor"` — logically contiguous, visually TWO
/// disjoint spans (the Hebrew segment displays RTL, so `ל ם` are its two
/// LEFTMOST columns and the unselected `ו ע` remainder sits between them
/// and `" wor"`).
fn capture_bidi_selection() -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = support::gpu_render_app(SEL_W, SEL_H);
    // Finish BEFORE registering: `register_fixture_font` settles one
    // update, and a pre-finish update would run the render schedule
    // without the device/PipelineCache (both land in `finish`).
    support::finish_and_run(&mut app, 0);
    support::register_fixture_font(&mut app, "Noto Sans Hebrew", "NotoSansHebrew-hebrew.ttf");
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme.colors.insert(SELECTION_BG_TOKEN.into(), sel_red());
        theme.colors.insert(SELECTION_FG_TOKEN.into(), sel_blue());
    }
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("hello עולם world")),
            FontFamily(FontStack(vec![
                FamilyEntry::Named(String::from("Fira Sans")),
                FamilyEntry::Named(String::from("Noto Sans Hebrew")),
            ])),
            FontSize(20.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            SelectionVisual::new(Cursor::new(0, 10), Cursor::new(0, 18)),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(SEL_W as f32)
                .height_px(SEL_H as f32),
        ))
        .add_child(text);

    let target = support::render_to_image(&mut app, SEL_W, SEL_H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::wait_for_text_ready(&mut app, 60);
    support::readback_rgba(&mut app, target)
}

#[test]
#[ignore = "needs a wgpu adapter; T7 mixed-BiDi ::selection golden (decoration-and-paint §§ 5.1–5.2; verification § 1.3)"]
fn mixed_bidi_selection_paints_disjoint_rects_and_retints() {
    let frame_a = capture_bidi_selection();

    // THE multi-rect contract as pixels (text.md:89): a logical range
    // straddling the BiDi boundary paints ≥ 2 visually disjoint red rects.
    // Width-filter the coalesced bands so an AA edge column can neither
    // fake a rect nor split one.
    let sel_bands = bands(&cols_where(&frame_a, SEL_W, SEL_H, is_strong_red));
    let wide: Vec<Range<u32>> = sel_bands
        .iter()
        .filter(|b| b.end - b.start >= 3)
        .cloned()
        .collect();
    assert!(
        wide.len() >= 2,
        "mixed-BiDi selection must paint disjoint rects, got {wide:?} (all red bands: {sel_bands:?})"
    );
    for pair in wide.windows(2) {
        assert!(
            pair[1].start - pair[0].end >= 3,
            "rects visually disjoint — the unselected Hebrew remainder sits between: {wide:?}"
        );
    }

    // Re-tint painted over the selection bg (glyph tier over quad tier):
    // blue ink exists INSIDE the red rects…
    let blue_cols = cols_where(&frame_a, SEL_W, SEL_H, is_blue_ink);
    assert!(
        blue_cols.iter().any(|x| wide.iter().any(|b| b.contains(x))),
        "selected glyphs re-tint to the selection fg inside the rects \
         (blue cols {blue_cols:?} vs rects {wide:?})"
    );
    // …and unselected text stays white OUTSIDE them.
    let white_cols = cols_where(&frame_a, SEL_W, SEL_H, is_white_ink);
    assert!(
        white_cols
            .iter()
            .any(|x| !sel_bands.iter().any(|b| b.contains(x))),
        "unselected text paints untinted white outside the rects \
         (white cols {white_cols:?} vs all red bands {sel_bands:?})"
    );

    // Re-capture determinism (the hello_text idiom): an independent fresh
    // capture matches — the re-capture IS the golden.
    let frame_b = capture_bidi_selection();
    let diff = perceptual_diff(&frame_a, &frame_b);
    assert!(diff < 1e-4, "two fresh captures diverged: {diff}");
}

// --- 2. the caret-blink fixed-clock pair (§ 6.3; verification § 3.1) --------

const CARET_W: u32 = 128;
const CARET_H: u32 = 64;

/// The shared blink fixture (the fixed-clock pair + the T8 damage assert
/// pin the SAME scene — pixels there, uploads here — so it lives once).
///
/// GoldenConfig::fixed_clock, realized: PAUSE the virtual clock so the
/// many real-time `app.update()`s the readback polls drive accrue ZERO
/// virtual elapsed — captures land at exactly the chosen instants
/// (t = 0 / 500 ms / 1000 ms) instead of drifting across a blink edge
/// mid-capture. `advance_by` still moves a paused clock; the per-frame
/// TimeSystem advance is what pausing zeroes.
///
/// The scene: the T6 no-descender fixture ("Hi" at 40 px, white) plus a
/// 1×48 pure-red caret at x = 80 — a column safely right of the glyph
/// ink — under a sized column root. Returns the text entity.
fn spawn_blink_fixture(app: &mut App) -> Entity {
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme.colors.insert(CARET_TOKEN.into(), sel_red());
    }
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(40.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            CaretVisual {
                visible: true,
                rect: Rect::new(80.0, 0.0, 81.0, 48.0),
            },
            CaretColor(ColorToken::Token(Cow::Borrowed(CARET_TOKEN))),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(CARET_W as f32)
                .height_px(CARET_H as f32),
        ))
        .add_child(text);
    text
}

#[test]
#[ignore = "needs a wgpu adapter; T7 caret-blink fixed-clock pair (decoration-and-paint § 6.3; verification § 3.1)"]
fn caret_blink_fixed_clock_pair() {
    let _cfg = GoldenConfig::deterministic(); // fixed_clock, realized in the fixture
    let mut app = support::gpu_render_app(CARET_W, CARET_H);
    let text = spawn_blink_fixture(&mut app);

    let target = support::render_to_image(&mut app, CARET_W, CARET_H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);

    // --- capture A: t = 0, the visible phase --------------------------------
    let frame_a = support::readback_rgba(&mut app, target.clone());

    // Exactly one red column band, § 3.3-snapped to 1 physical px at the
    // authored x folded by the entity origin (the headless suite pins the
    // snap math; this pins it end-to-end through the glyph draw).
    let caret_cols = {
        let bands = bands(&cols_where(&frame_a, CARET_W, CARET_H, is_strong_red));
        assert_eq!(bands.len(), 1, "exactly one caret column band: {bands:?}");
        bands.into_iter().next().unwrap()
    };
    assert_eq!(
        caret_cols.end - caret_cols.start,
        1,
        "§ 3.3 floored caret width: 1 physical px at scale 1"
    );
    let origin = {
        let world = app.world();
        world
            .get::<GlobalTransform>(text)
            .expect("the text entity's GlobalTransform")
            .translation()
            .truncate()
            + world
                .get::<ComputedTextLayout>(text)
                .expect("the text entity's ComputedTextLayout")
                .content_offset
    };
    assert_eq!(
        caret_cols.start,
        (origin.x + 80.0).round() as u32,
        "the caret column is the origin-folded, grid-snapped authored x"
    );
    let white_cols = cols_where(&frame_a, CARET_W, CARET_H, is_white_ink);
    assert!(
        caret_cols.start > *white_cols.last().expect("the glyph ink painted"),
        "the caret column sits right of the glyph ink ({white_cols:?})"
    );
    // The stamp spans ≈ the authored 48-row line box (hard-edged quad;
    // ±1 row for fractional-origin rasterization).
    let caret_rows = {
        let bands = bands(&rows_where(&frame_a, CARET_W, CARET_H, is_strong_red));
        assert_eq!(bands.len(), 1, "one contiguous caret row band: {bands:?}");
        bands.into_iter().next().unwrap()
    };
    assert!(
        caret_rows.start.abs_diff(origin.y.round() as u32) <= 1
            && (caret_rows.end - caret_rows.start).abs_diff(48) <= 2,
        "the caret spans the authored line-box rows (got {caret_rows:?}, origin.y {})",
        origin.y
    );

    // --- capture B: t = 500 ms, the hidden phase -----------------------------
    // The writer flips `visible` on the edge; the producer drops the stamp;
    // the value-compared publish leaves the quad carrier untouched.
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(500));
    app.update();
    app.update();
    let frame_b = support::readback_rgba(&mut app, target.clone());
    assert!(
        cols_where(&frame_b, CARET_W, CARET_H, is_strong_red).is_empty(),
        "hidden phase: zero red pixels anywhere"
    );
    // The blink touched NOTHING else: outside the caret columns, A and B
    // are byte-identical (the white glyph ink in particular).
    let leaked: Vec<(u32, u32)> = (0..CARET_H)
        .flat_map(|y| (0..CARET_W).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            !caret_cols.contains(&x)
                && support::px(&frame_a, CARET_W, x, y) != support::px(&frame_b, CARET_W, x, y)
        })
        .collect();
    assert!(
        leaked.is_empty(),
        "the blink edge changed pixels outside the caret columns: {:?}…",
        &leaked[..leaked.len().min(8)]
    );

    // --- capture C: t = 1000 ms — the pair is periodic -----------------------
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(500));
    app.update();
    app.update();
    let frame_c = support::readback_rgba(&mut app, target);
    let diff = perceptual_diff(&frame_a, &frame_c);
    assert!(
        diff < 1e-4,
        "the visible phase re-renders identically a full period later: {diff}"
    );
}

// --- 3. the caret-blink GPU damage assert (T8; § 6.3; verification § 1.3) ---

#[test]
#[ignore = "needs a wgpu adapter; T8 caret-blink GPU damage assert (decoration-and-paint § 6.3 damage property; verification § 1.3 'Caret-blink damage' row)"]
fn caret_blink_reuploads_the_glyph_buffer_only() {
    // The blink-pair fixture + paused virtual clock, verbatim.
    let _cfg = GoldenConfig::deterministic();
    let mut app = support::gpu_render_app(CARET_W, CARET_H);
    let _text = spawn_blink_fixture(&mut app);
    let target = support::render_to_image(&mut app, CARET_W, CARET_H);
    support::spawn_capture_camera(&mut app, target);
    support::finish_and_run(&mut app, 1);
    support::wait_for_text_ready(&mut app, 60);

    let stats = |app: &App| {
        *support::render_world_resource::<BufferUploadStats>(app).expect("BufferUploadStats")
    };

    // Drain to steady state: run frames until an update uploads nothing
    // (pipeline warm-up + the readback poller can dirty early frames).
    let mut base = stats(&app);
    for _ in 0..10 {
        app.update();
        let now = stats(&app);
        if now == base {
            break;
        }
        base = now;
    }
    // Steady frame: O(0) — neither buffer re-uploads.
    app.update();
    assert_eq!(stats(&app), base, "a steady frame uploads NOTHING");

    // The blink edge (paused clock, explicit advance — the pair test's
    // idiom): the writer flips CaretVisual, the producer rebuilds, the
    // value-compared publish leaves the quad carrier untouched…
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(Duration::from_millis(500));
    app.update();
    let after_edge = stats(&app);
    // …so prepare re-uploads the GLYPH buffer exactly once and RETAINS
    // the quad buffer — the GPU half of § 6.3's damage property (T7
    // landed the CPU half; this is the campaign's T8 assertion).
    assert_eq!(
        after_edge.glyph_uploads,
        base.glyph_uploads + 1,
        "the blink edge re-uploaded the glyph buffer exactly once"
    );
    assert_eq!(
        after_edge.quad_uploads, base.quad_uploads,
        "…and did NOT touch the quad buffer"
    );

    // The next frame is steady again (the edge writer is edge-only).
    app.update();
    assert_eq!(stats(&app), after_edge, "post-edge frame is steady");
}
