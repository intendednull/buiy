//! GPU end-to-end decoration tests (T6): real entities through TextSync →
//! TextCommit → extract (quads + stamps) → the § 4.6 splice → pixels.
//! decoration-and-paint §§ 3–4. The decoration GEOMETRY (band count, position
//! relative to the glyph ink, thickness, the double-underline gap) is pinned
//! HEADLESS off the extract carriers in `text_decoration_extract.rs` (audit
//! 2026-06-18 #28 / T2.9) and as closed-form algebra in `text_decoration.rs`;
//! the goldens here keep only the rasterization RESIDUE the cheaper tiers
//! cannot see — the quad tier's antialiased band signature (underline) and the
//! § 4.4 paint-ORDER seat (line-through draws OVER the glyphs). Plus the three
//! quad-gate terms regression-pinned end-to-end, and the § 4.5 group test —
//! pinned by T6 as the EXPECTED asymmetry, FLIPPED by T8: everything inside the
//! group (underline, line-through, ink) now dims exactly once through the
//! glyph-buffer partition. All #[ignore]: need a wgpu adapter (CLAUDE.md GPU
//! lane).
//!
//! Run: cargo test -p buiy_core --test text_decoration_gpu -- --ignored --test-threads=1
//!
//! ## Band classification (re-capture IS the golden — the text_gpu idiom)
//!
//! The two decoration tiers leave DIFFERENT pixel signatures, so the row
//! classifiers below are derived from the shaders, not hand-tuned:
//!
//! - **Quad tier (underline/overline):** `shader.wgsl` antialiases the SDF
//!   edge with `alpha = 1 − smoothstep(−aa, aa, d)`, `aa = fwidth(d) ≈ 1`
//!   logical px. **On the pinned lavapipe** a § 3.3-floored 2-physical-px band
//!   therefore has NO full-coverage row: both interior rows sit at pixel-center
//!   distance 0.5 from an edge → alpha `1 − smoothstep(−1, 1, −0.5)` =
//!   **0.84375** (sRGB-encoded pure red ≈ 237), and one bleed row above + below
//!   reads alpha **0.15625** (≈ 110). [`is_strong_red`]'s 200 threshold
//!   separates band rows from bleed rows; counting strong rows recovers the
//!   floored thickness. This AA SIGNATURE is rasterizer-SPECIFIC: `fwidth` is a
//!   derivative the rasterizer computes, and it diverges across hardware — this
//!   host's RX 6700 XT / RADV hard-edges the SAME band to full-coverage red on
//!   BOTH rows (probe: 255 everywhere, zero sub-255 pixels). So the no-full-row
//!   / strong-≈237 pixel CLAIM is gated on [`crate::support::on_pinned_lavapipe`]
//!   (audit #28; mirrors `golden_sdf_corner` / T3.3); the rasterizer-INTERNAL
//!   legs (band count, re-capture determinism) run on EVERY adapter.
//! - **Stamp tier (line-through):** a hard-edged coverage quad (no SDF) at
//!   alpha 1 — interior pixels read the exact
//!   [`crate::support::expected_full_coverage_srgb`] encode ([`is_full_red`], ±4).
//!
//! This supersedes the plan's sketch of one ±4-of-full-coverage matcher for
//! both tiers — that matcher can never see a thin AA'd quad row.
#![allow(deprecated)] // perceptual_diff is deprecated; these GPU sites migrate to buiy_verify::metric in Phase 3 (tier-5 goldens).

use bevy::prelude::*;
use buiy_core::Node;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, Opacity, TextColor};
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::{DecorationLines, FontSize, Text, TextDecorations};
use std::borrow::Cow;
use std::ops::Range;

const W: u32 = 128;
const H: u32 = 64;
/// Glyph tint: white — chroma-orthogonal to the red decoration token, so
/// row classification never confuses ink with lines.
const TEXT_TOKEN: &str = "test.text";
/// `text-decoration-color`: pure red (tier 1 of the § 3.2 precedence).
const DECO_TOKEN: &str = "test.deco";
/// The recolor target for the gate-term test: pure blue.
const DECO_BLUE_TOKEN: &str = "test.deco.blue";

fn deco_red() -> Color {
    Color::srgb(1.0, 0.0, 0.0)
}

fn deco_blue() -> Color {
    Color::srgb(0.0, 0.0, 1.0)
}

/// `TextDecorations` with the shared red token (the fixtures' tier-1 color).
fn red_deco(line: DecorationLines) -> TextDecorations {
    TextDecorations {
        line,
        color: Some(ColorToken::Token(Cow::Borrowed(DECO_TOKEN))),
        ..Default::default()
    }
}

fn insert_theme_tokens(app: &mut App) {
    let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
    theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
    theme.colors.insert(DECO_TOKEN.into(), deco_red());
    theme.colors.insert(DECO_BLUE_TOKEN.into(), deco_blue());
}

/// The `text_gpu.rs::spawn_text_fixture` shape plus the decoration component:
/// "Hi" at 40 px — no descenders, so glyph ink never crosses below the
/// baseline and band classification is unambiguous. Returns
/// `(text, root)` so tests can mutate the component / add siblings.
fn spawn_decorated_fixture(app: &mut App, deco: TextDecorations) -> (Entity, Entity) {
    insert_theme_tokens(app);
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(40.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            deco,
        ))
        .id();
    let root = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .height_px(H as f32),
        ))
        .add_child(text)
        .id();
    (text, root)
}

/// Build app → decorated fixture → capture the first text-ready frame.
fn capture_decorated(deco: TextDecorations) -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic(); // the triad gates this fixture
    let mut app = crate::support::gpu_render_app(W, H);
    spawn_decorated_fixture(&mut app, deco);
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    crate::support::readback_rgba(&mut app, target)
}

// --- row classifiers (module doc: derived from the shader AA math) ---------

/// Full-coverage red — the STAMP tier's interior (hard edges, alpha 1):
/// within ±4/channel of the SrcOver-over-black sRGB encode of the red token.
fn is_full_red(p: [u8; 4]) -> bool {
    let lin = LinearRgba::from(deco_red());
    let e = crate::support::expected_full_coverage_srgb([lin.red, lin.green, lin.blue, lin.alpha]);
    (0..3).all(|ch| (p[ch] as i32 - e[ch] as i32).abs() <= 4)
}

/// Quad-tier band row at FLAT (ungrouped) intensity: ≈237 passes, the
/// ≈110 AA bleed rows fail (module doc) — and so does the Opacity(0.5)
/// group-dimmed row (0.84375 × 0.5 linear → sRGB ≈ 174).
fn is_strong_red(p: [u8; 4]) -> bool {
    p[0] >= 200 && p[1] <= 20 && p[2] <= 20
}

/// Red at any painted-band strength: catches the group-dimmed quad rows
/// (≈174) while still rejecting flat AA bleed (≈110).
fn is_present_red(p: [u8; 4]) -> bool {
    p[0] >= 140 && p[1] <= 20 && p[2] <= 20
}

fn is_strong_blue(p: [u8; 4]) -> bool {
    p[2] >= 200 && p[0] <= 20 && p[1] <= 20
}

/// Glyph-ink white (the white text token at ≥ ~73% coverage).
fn is_white(p: [u8; 4]) -> bool {
    p[0] >= 200 && p[1] >= 200 && p[2] >= 200
}

/// Dimmed glyph ink: the group's white text composited ONCE at 0.5 —
/// full coverage reads ≈ sRGB 188/channel (linear 0.5); the old
/// [`is_white`] ≥ 200 threshold (≈ 73 % coverage undimmed) maps to
/// ≈ 162 dimmed, so ≥ 160 recovers the same row envelope.
fn is_dim_white(p: [u8; 4]) -> bool {
    (0..3).all(|ch| p[ch] >= 160)
}

/// Rows (top→bottom) where ANY pixel satisfies `pred`.
fn rows_where(pixels: &[u8], pred: impl Fn([u8; 4]) -> bool) -> Vec<u32> {
    (0..H)
        .filter(|&y| (0..W).any(|x| pred(crate::support::px(pixels, W, x, y))))
        .collect()
}

/// Coalesce sorted row indices into contiguous bands.
fn bands(rows: &[u32]) -> Vec<Range<u32>> {
    let mut out: Vec<Range<u32>> = Vec::new();
    for &r in rows {
        match out.last_mut() {
            Some(b) if b.end == r => b.end = r + 1,
            _ => out.push(r..r + 1),
        }
    }
    out
}

/// Quad-tier decoration bands at flat intensity.
fn red_bands(pixels: &[u8]) -> Vec<Range<u32>> {
    bands(&rows_where(pixels, is_strong_red))
}

/// The glyph-ink row envelope (first..last+1 white row).
fn white_rows(pixels: &[u8]) -> Range<u32> {
    let rows = rows_where(pixels, is_white);
    let first = *rows.first().expect("the white glyph ink painted");
    let last = *rows.last().expect("the white glyph ink painted");
    first..last + 1
}

/// The dimmed glyph-ink row envelope (the [`white_rows`] mirror for the
/// Opacity(0.5)-group fixture, whose ink never reaches the ≥ 200 bar).
fn dim_white_rows(pixels: &[u8]) -> Range<u32> {
    let rows = rows_where(pixels, is_dim_white);
    let first = *rows.first().expect("the dimmed glyph ink painted");
    let last = *rows.last().expect("the dimmed glyph ink painted");
    first..last + 1
}

// --- the per-kind AA-residue goldens (geometry pushed headless, T2.9) --------

// NOTE (audit 2026-06-18, #28 / T2.9): the decoration GEOMETRY — band count,
// position relative to the glyph ink, thickness, and the double-underline gap —
// moved DOWN to the headless extract tier (`text_decoration_extract.rs`, off
// `ExtractedTextQuads` + `ExtractedGlyphs`, no adapter). It is also pinned as
// closed-form algebra on hand fixtures in `text_decoration.rs`. The goldens
// below keep exactly ONE per kind, asserting only the rasterization RESIDUE the
// cheaper tiers cannot observe: the quad tier's rasterized band residue (the
// `is_strong_red` / bleed split derived from `shader.wgsl`; exact per-row
// coverage is rasterizer/toolchain-pinned — e.g. the floored 2-px underline is
// full-coverage under wgpu29 lavapipe), re-capture determinism, and — for
// line-through — the § 4.4 paint-ORDER seat
// (the solid stamp draws OVER the glyph coverage), which is a pixel fact, not a
// carrier fact.

#[test]
#[ignore = "needs a wgpu adapter; T6 underline quad-tier residue golden (pinned-lavapipe solid band)"]
fn underline_quad_band_residue_on_pinned_lavapipe() {
    // RESIDUE confidence only (geometry is headless now): the quad-tier band
    // rasterizes through `shader.wgsl` and lands as a solid, correctly-floored
    // 2-physical-px band. On the pinned wgpu29 lavapipe both band rows read FULL
    // coverage (the pixel fact this golden guards, gated to lavapipe below —
    // exact coverage is rasterizer/toolchain-pinned, see audit #28 + the 0.19
    // recalibration in follow-ups.md). The band-rasterized + re-capture
    // bit-stability legs are rasterizer-internal and run on EVERY adapter.
    let frame_a = capture_decorated(red_deco(DecorationLines::UNDERLINE));
    let bands = red_bands(&frame_a);
    // Band rasterized (rasterizer-INTERNAL, runs on EVERY adapter): the quad
    // tier painted exactly one contiguous strong-red band where the floored
    // 2-physical-px underline lands. A dropped/duplicated band fails here on the
    // RX and on lavapipe alike.
    assert_eq!(
        bands.len(),
        1,
        "the AA'd underline band rasterized: {bands:?}"
    );

    // The exact band COVERAGE is rasterizer/toolchain-pinned (audit #28;
    // determinism.md § "CI software-rasterizer pin"). `shader.wgsl` antialiases
    // the SDF edge with `aa = fwidth(d)`, a derivative whose value depends on the
    // rasterizer AND the toolchain. Under wgpu27 the pinned lavapipe read the
    // floored 2-px band at AA alpha 0.84375 (≈237, NO full-coverage row) while
    // the RX 6700 XT hard-edged it to full coverage; the wgpu27→29 bump
    // pixel-aligns the band so BOTH now read FULL coverage (255) — the residue
    // recalibrated here (follow-ups.md). We pin the exact-pixel residue to the
    // canonical lavapipe (mirror golden_sdf_corner / T3.3); off it, skip-as-pending
    // after the band-count + determinism legs (which ARE rasterizer-internal)
    // have run. The CI lavapipe leg keeps the residue coverage.
    if !crate::support::on_pinned_lavapipe() {
        eprintln!(
            "underline_quad_band_residue_on_pinned_lavapipe: selected adapter is \
             not the pinned lavapipe — SKIPPING the exact-pixel residue assertion \
             (the solid full-coverage 2-row band). The pinned lavapipe is the \
             canonical pixel contract (determinism.md § \"the local lane does not \
             compare against the stored lavapipe baseline\"); off it the exact \
             coverage is a best-effort, non-baseline fact. The band-count and \
             re-capture-determinism legs above/below DID run."
        );
    } else {
        // Residue signature on the pinned wgpu29 lavapipe. The §3.3-floored
        // 2-physical-px underline band is pixel-aligned and rasterizes to FULL
        // coverage (max red 255) across BOTH of its rows — and the RX 6700 XT
        // agrees. The sub-pixel AA alpha (0.84375 ≈237) this leg asserted under
        // wgpu27 was a rasterizer artifact of the band straddling rows; the
        // wgpu27→29 toolchain bump pixel-aligns it, so the residue is now a SOLID
        // 2-row band. (A toolchain-pinned pixel fact — determinism.md — recalibrated
        // at the 0.19 bump; see follow-ups.md. The band-count + re-capture
        // determinism legs that DID catch a dropped/duplicated/non-deterministic
        // band still run on every adapter.) So the full-coverage rows coincide
        // EXACTLY with the strong-red band rows, and there are two of them.
        let full = rows_where(&frame_a, is_full_red);
        let strong = rows_where(&frame_a, is_strong_red);
        assert_eq!(
            full, strong,
            "the quad-tier underline band is solid full-coverage red across its \
             whole height on the pinned lavapipe (full={full:?} strong={strong:?})"
        );
        assert_eq!(
            full.len(),
            2,
            "the §3.3-floored underline band is exactly 2 physical px (full={full:?})"
        );
    }

    // Re-capture determinism (the hello_text idiom; rasterizer-INTERNAL, runs on
    // EVERY adapter): an independent fresh capture matches — the re-capture IS
    // the golden.
    let frame_b = capture_decorated(red_deco(DecorationLines::UNDERLINE));
    let diff = perceptual_diff(&frame_a, &frame_b);
    assert!(diff < 1e-4, "two fresh captures diverged: {diff}");
}

#[test]
#[ignore = "needs a wgpu adapter; T6 line-through AA-residue golden — THE § 4.4 seat-5 paint-order test (stamp paints OVER the text)"]
fn line_through_paints_over_the_glyph_ink() {
    // RESIDUE confidence: the line-through stamp's PAINT ORDER over the glyph
    // ink is a pixel fact the extract carriers cannot observe (they carry the
    // stamp instance, not its composited result). Geometry — that exactly one
    // stamp intersects the ink at the floored thickness — is headless in
    // `text_decoration_extract.rs`; here we keep only the over-paint seat.
    let frame = capture_decorated(red_deco(DecorationLines::LINE_THROUGH));
    let ink = white_rows(&frame);
    // The stamp tier is hard-edged at alpha 1 → exact full-coverage red.
    let bands = bands(&rows_where(&frame, is_full_red));
    assert_eq!(bands.len(), 1, "exactly one line-through band: {bands:?}");
    let band = &bands[0];
    assert!(
        band.start < ink.end && band.end > ink.start,
        "the line-through ({band:?}) INTERSECTS the glyph ink ({ink:?})"
    );
    assert!(band.start > 0 && band.end < H, "band inside the frame");

    // THE seat assertion: at columns where glyph ink is white directly above
    // AND below the band, the band's own rows read RED — the solid stamp
    // painted over the glyph coverage (CSS Text Decoration L3 painting
    // order). A quad-tier line-through would read white here: quads draw
    // under glyphs (§ 4.1's fixed primitive rank).
    let mut stem_columns = 0;
    for x in 0..W {
        let above = crate::support::px(&frame, W, x, band.start - 1);
        let below = crate::support::px(&frame, W, x, band.end);
        if is_white(above) && is_white(below) {
            stem_columns += 1;
            for y in band.clone() {
                let p = crate::support::px(&frame, W, x, y);
                assert!(
                    is_full_red(p),
                    "stamp painted over the glyph ink at ({x},{y}): got {p:?}"
                );
            }
        }
    }
    assert!(
        stem_columns > 0,
        "at least one glyph stem column crosses the band (the seat test is not vacuous)"
    );
}

// --- the three quad-gate terms, regression-pinned end-to-end ----------------

#[test]
#[ignore = "needs a wgpu adapter; T6 gate-term regression: text_quads.is_changed() repacks the quad buffer"]
fn decoration_recolor_repacks_the_quad_buffer() {
    // The THIRD gate term: a TextDecorations color edit fires
    // Changed<TextDecorations> in the TEXT probe union only — no
    // extract_buiy_nodes union member fires (the ResolvedLayout /
    // ComputedTextLayout writes are idempotent for a color-only edit), so
    // without `text_quads.is_changed()` in prepare's quad gate the buffer
    // never repacks and a STALE RED underline survives below.
    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    let (text, _root) = spawn_decorated_fixture(&mut app, red_deco(DecorationLines::UNDERLINE));
    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame_a = crate::support::readback_rgba(&mut app, target.clone());
    let red_a = red_bands(&frame_a);
    assert_eq!(red_a.len(), 1, "the red underline painted: {red_a:?}");

    // Recolor through the component (NOT the theme — theme.is_changed() is
    // a separate, already-pinned union member).
    app.world_mut()
        .get_mut::<TextDecorations>(text)
        .expect("the fixture's TextDecorations")
        .color = Some(ColorToken::Token(Cow::Borrowed(DECO_BLUE_TOKEN)));
    for _ in 0..3 {
        app.update();
    }
    let frame_b = crate::support::readback_rgba(&mut app, target);

    let blue_b = bands(&rows_where(&frame_b, is_strong_blue));
    assert_eq!(
        blue_b, red_a,
        "the band is now BLUE at the same rows (color-only edit moves nothing)"
    );
    assert!(
        rows_where(&frame_b, |p| p[0] >= 100 && p[1] <= 20 && p[2] <= 20).is_empty(),
        "no red remains anywhere — incl. the AA bleed rows a stale quad buffer would keep"
    );
}

#[test]
#[ignore = "needs a wgpu adapter; T6 gate-term regression: retained quads re-splice through the fresh node list"]
fn sibling_background_change_resplices_retained_quads() {
    // The NODES term + § 4.6 fact (b): a sibling Background edit rebuilds
    // the node list (text union does NOT fire — ExtractedTextQuads is
    // RETAINED), and the retained carrier must land at identical rows
    // through the fresh-node-list walk. A stale-index merge (the spec's
    // rejected round-1 painters_z key) would misplace or drop the band.
    const BG_A: &str = "test.bg.a";
    const BG_B: &str = "test.bg.b";
    let bg_a = Color::srgb(0.0, 1.0, 0.0);
    let bg_b = Color::srgb(0.0, 1.0, 1.0);

    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    let (_text, root) = spawn_decorated_fixture(&mut app, red_deco(DecorationLines::UNDERLINE));
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(BG_A.into(), bg_a);
        theme.colors.insert(BG_B.into(), bg_b);
    }
    // The sibling: an absolute 8×8 box in the top-right corner — clear of
    // the glyph ink, the underline rows, and the red/white classifiers
    // (green/cyan match neither).
    let sibling = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(2.0)),
                    left: Sizing::Length(Length::px(116.0)),
                    ..default()
                })
                .width_px(8.0)
                .height_px(8.0),
            Background {
                color: ColorToken::Token(Cow::Borrowed(BG_A)),
            },
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(sibling);

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 1);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame_a = crate::support::readback_rgba(&mut app, target.clone());
    let bands_a = red_bands(&frame_a);
    assert_eq!(bands_a.len(), 1, "the underline painted: {bands_a:?}");
    let box_px = |frame: &[u8], expected: Color| {
        // Deep interior of the 8×8 box — full SDF coverage, exact encode.
        let got = crate::support::px(frame, W, 120, 6);
        let lin = LinearRgba::from(expected);
        let want =
            crate::support::expected_full_coverage_srgb([lin.red, lin.green, lin.blue, lin.alpha]);
        (0..3).all(|ch| (got[ch] as i32 - want[ch] as i32).abs() <= 4)
    };
    assert!(box_px(&frame_a, bg_a), "the sibling box painted green");

    // The sibling-only edit: Changed<Background> → node walk rebuilds.
    app.world_mut()
        .get_mut::<Background>(sibling)
        .expect("the sibling's Background")
        .color = ColorToken::Token(Cow::Borrowed(BG_B));
    for _ in 0..3 {
        app.update();
    }
    let frame_b = crate::support::readback_rgba(&mut app, target);
    assert!(
        box_px(&frame_b, bg_b),
        "the sibling recolored — the node list really rebuilt"
    );
    assert_eq!(
        red_bands(&frame_b),
        bands_a,
        "the RETAINED underline re-spliced at identical rows through the fresh node list"
    );
}

#[test]
#[ignore = "needs a wgpu adapter; T6 groups term + the § 4.5 asymmetry, FLIPPED by T8 — everything in the group dims exactly once"]
fn opacity_group_dims_underline_line_through_and_ink() {
    // The GROUPS term + the § 4.5 asymmetry, FLIPPED by T8: the underline
    // quad rides pack_view_partitioned and ADOPTS its entity's effect group
    // (dimmed by the off-screen composite at 0.5) — and the line-through
    // stamp (a glyph-tier instance) now rides the group's GLYPH range
    // through the same off-screen target, so the whole subtree dims
    // exactly once.
    let _cfg = GoldenConfig::deterministic();
    let mut app = crate::support::gpu_render_app(W, H);
    insert_theme_tokens(&mut app);
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("Hi")),
            FontSize(40.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            red_deco(DecorationLines::UNDERLINE | DecorationLines::LINE_THROUGH),
        ))
        .id();
    // The Opacity(0.5) card — an EffectGroup former (write_effect_groups
    // marks it) wrapping the text.
    let card = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .height_px(H as f32),
            Opacity(0.5),
        ))
        .add_child(text)
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(W as f32)
                .height_px(H as f32),
        ))
        .add_child(card);

    let target = crate::support::render_to_image(&mut app, W, H);
    crate::support::spawn_capture_camera(&mut app, target.clone());
    crate::support::finish_and_run(&mut app, 4);
    crate::support::wait_for_text_ready(&mut app, 60);
    let frame = crate::support::readback_rgba(&mut app, target);
    // Post-T8 the ink itself dims (full coverage ≈ 188/channel), so the
    // dimmed envelope locates it; zero rows survive at the undimmed
    // `is_white` bar (asserted in half 2).
    let ink = dim_white_rows(&frame);

    // Half 1 — the underline DIMMED (group adoption end-to-end): below the
    // ink there is a red band at composite-dimmed intensity (≈174: quad row
    // alpha 0.84375 × group 0.5), and NO row at flat strength (≈237) — a
    // flat row there would mean the quad escaped its entity's group.
    let strong_below: Vec<u32> = rows_where(&frame, is_strong_red)
        .into_iter()
        .filter(|&r| r >= ink.end)
        .collect();
    assert!(
        strong_below.is_empty(),
        "the underline rode the group's partition range — dimmed, never flat: {strong_below:?}"
    );
    let present_below: Vec<u32> = rows_where(&frame, is_present_red)
        .into_iter()
        .filter(|&r| r >= ink.end)
        .collect();
    assert!(
        !present_below.is_empty(),
        "the dimmed underline IS present below the ink ({ink:?})"
    );

    // Half 2 — FLIPPED by T8: everything inside the group dims exactly
    // once. (a) The ink itself: zero undimmed-white rows anywhere.
    assert!(
        rows_where(&frame, is_white).is_empty(),
        "no undimmed glyph-ink row — the group's glyphs rode its target"
    );
    // (b) The line-through: zero FULL-strength stamp rows anywhere…
    assert!(
        rows_where(&frame, is_full_red).is_empty(),
        "no full-strength line-through row — the stamp rode the group's glyph range"
    );
    // …and the DIMMED stamp band is present over the (dimmed) ink:
    // red @ alpha 1 in the target → composite 0.5 over black ≈ sRGB 188
    // red — passes is_present_red (≥140), fails is_strong_red (≥200).
    let present_over_ink: Vec<u32> = rows_where(&frame, is_present_red)
        .into_iter()
        .filter(|&r| r >= ink.start && r < ink.end)
        .collect();
    assert!(
        !present_over_ink.is_empty(),
        "the dimmed line-through band sits over the ink ({ink:?})"
    );
}
