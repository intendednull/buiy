//! Headless (no GPU): the parity Wave B3 vector-icon coverage rasterizer + the
//! content-addressed atlas key. CI-safe regression guards for the pieces that
//! decide correctness BEFORE any adapter is involved:
//!
//!   - `rasterize_icon` produces an `R8` coverage bitmap of the requested size
//!     with the stroke LIT and known-empty interior regions DARK (the coverage
//!     sanity the producer relies on),
//!   - the stroke-width is REAL (a thicker stroke lights more texels — the
//!     exact-parity reason the design rejects an icon-font, § 8),
//!   - `icon_atlas_key` DEDUPS bit-identical icons to one key and gives a
//!     distinct key to a different `(d, stroke_width, size, fill)` — the atlas
//!     dedup contract,
//!   - the icon key never aliases the text solid-stamp key (both ride the `Mask`
//!     coverage kind; the sub-id keeps them disjoint).
//!
//! The GPU readback complement (the icon actually PAINTS + re-tints) is
//! `render_icon_gpu.rs` (`#[ignore]`, real adapter / lavapipe).

use bevy::math::Vec2;
use buiy_core::render::atlas::AtlasFormat;
use buiy_core::render::icon_producer::{icon_atlas_key, icon_paints};
use buiy_core::render::icon_raster::{ICON_VIEWBOX, IconPaint, rasterize_icon};

/// The chevron (`M9 5l7 7-7 7`, stroke 1.9 — disclosure icon #24) rasterizes to
/// an R8 cell of the requested size, with stroke texels lit and a far-off-stroke
/// interior corner dark.
#[test]
fn icon_rasterizes_to_r8_with_lit_stroke_and_empty_corner() {
    let bmp = rasterize_icon("M9 5l7 7-7 7", IconPaint::Stroke, 1.9, 20, ICON_VIEWBOX);
    assert_eq!(bmp.format, AtlasFormat::CoverageR8);
    assert_eq!(bmp.size.x, 20);
    assert_eq!(bmp.size.y, 20);
    assert_eq!(bmp.data.len(), 20 * 20);

    let lit = bmp.data.iter().filter(|&&v| v > 0).count();
    assert!(
        lit > 10,
        "chevron stroke must light a non-trivial coverage area"
    );

    // The top-left corner is far from both chevron arms → empty interior.
    let at = |x: u32, y: u32| bmp.data[(y * 20 + x) as usize];
    assert!(
        (0..4).all(|y| (0..4).all(|x| at(x, y) == 0)),
        "the top-left interior (off the stroke) must carry zero coverage"
    );
}

/// The exact-parity reason for real vectors over an icon-font (§ 8): the
/// stroke-width is REAL, so a thicker stroke lights more coverage at the same
/// size. (An icon-font would bake one width.)
#[test]
fn thicker_stroke_lights_more_coverage() {
    let thin = rasterize_icon(
        "M4 12.5 9 17.5 20 6.5",
        IconPaint::Stroke,
        1.7,
        24,
        ICON_VIEWBOX,
    );
    let thick = rasterize_icon(
        "M4 12.5 9 17.5 20 6.5",
        IconPaint::Stroke,
        2.4,
        24,
        ICON_VIEWBOX,
    );
    let lit = |b: &buiy_core::render::atlas::AtlasBitmap| b.data.iter().filter(|&&v| v > 0).count();
    assert!(
        lit(&thick) > lit(&thin),
        "a 2.4 stroke must light more texels than a 1.7 stroke at the same size: \
         thin {} thick {}",
        lit(&thin),
        lit(&thick)
    );
}

/// `icon_atlas_key` content-addresses: bit-identical inputs → ONE key (the atlas
/// dedup contract — the same icon authored twice hits the resident cell).
#[test]
fn identical_icons_share_one_key() {
    let a = icon_atlas_key("M9 5l7 7-7 7", 1.9, 17, ICON_VIEWBOX, false);
    let b = icon_atlas_key("M9 5l7 7-7 7", 1.9, 17, ICON_VIEWBOX, false);
    assert_eq!(
        a, b,
        "identical (d, width, size, fill) must dedup to one key"
    );
}

/// Each axis of the key is significant: a different path, stroke-width, size, or
/// fill flag yields a DISTINCT key (a re-stroked / re-sized icon is a new cell).
#[test]
fn distinct_inputs_give_distinct_keys() {
    let base = icon_atlas_key("M9 5l7 7-7 7", 1.9, 17, ICON_VIEWBOX, false);
    assert_ne!(
        base,
        icon_atlas_key("M6 9l6 6 6-6", 1.9, 17, ICON_VIEWBOX, false),
        "different d"
    );
    assert_ne!(
        base,
        icon_atlas_key("M9 5l7 7-7 7", 2.4, 17, ICON_VIEWBOX, false),
        "different width"
    );
    assert_ne!(
        base,
        icon_atlas_key("M9 5l7 7-7 7", 1.9, 24, ICON_VIEWBOX, false),
        "different size"
    );
    assert_ne!(
        base,
        icon_atlas_key("M9 5l7 7-7 7", 1.9, 17, 40.0, false),
        "different viewBox (same size at a different author viewBox rasterizes at \
         a different scale — must be a distinct cell)"
    );
    assert_ne!(
        base,
        icon_atlas_key("M9 5l7 7-7 7", 1.9, 17, ICON_VIEWBOX, true),
        "different fill"
    );
}

/// M5 — an `Icon` on a zero-area box (a `Display::None` / collapsed node, which
/// retains a stale `0×0` `ResolvedLayout`) emits NOTHING. An icon paints at its
/// NATIVE `size_px`, not the box size, so without this skip a collapsed node
/// would still rasterize its glyph at the collapsed origin (a stray tofu/glyph).
/// This honors the same zero-rect skip the bg-quad/shadow paths use.
#[test]
fn zero_area_box_emits_no_icon() {
    // A real, authored, opaque chevron — the ONLY reason it must not paint is
    // the zero-area box (the M5 collapse condition).
    let chevron = "M9 5l7 7-7 7";
    assert!(
        !icon_paints(false, chevron, 16, 1.0, Vec2::ZERO),
        "a 0×0 box must suppress the icon (M5: a Display::None/collapsed node)"
    );
    assert!(
        !icon_paints(false, chevron, 16, 1.0, Vec2::new(0.0, 16.0)),
        "a zero-WIDTH box must suppress the icon"
    );
    assert!(
        !icon_paints(false, chevron, 16, 1.0, Vec2::new(16.0, 0.0)),
        "a zero-HEIGHT box must suppress the icon"
    );
}

/// The positive control + the other skip reasons, so the M5 skip is the only new
/// suppression (an opaque authored icon on a real box DOES paint).
#[test]
fn icon_paints_predicate_honors_every_skip_reason() {
    let chevron = "M9 5l7 7-7 7";
    let real_box = Vec2::new(16.0, 16.0);
    // Positive control: authored, opaque, real box, not paint-skipped → paints.
    assert!(
        icon_paints(false, chevron, 16, 1.0, real_box),
        "a real opaque icon on a real box must paint"
    );
    // The computed paint-skip marker (CssVisibility::Hidden / OffscreenAuto).
    assert!(
        !icon_paints(true, chevron, 16, 1.0, real_box),
        "a paint-skipped icon emits nothing"
    );
    // Nothing authored: empty path or zero size_px.
    assert!(
        !icon_paints(false, "", 16, 1.0, real_box),
        "an empty path emits nothing"
    );
    assert!(
        !icon_paints(false, chevron, 0, 1.0, real_box),
        "size_px == 0 emits nothing"
    );
    // Fully transparent tint.
    assert!(
        !icon_paints(false, chevron, 16, 0.0, real_box),
        "a fully transparent tint emits nothing"
    );
}

/// An icon key never aliases the text solid-stamp key. Both ride the `Mask`
/// coverage kind (so both insert into the CoverageR8 atlas), but the stamp is
/// `[Mask, 0]` (text/stamp.rs) and an icon is `[Mask, 1, …hash]` — the sub-id
/// keeps the two key spaces disjoint, so an icon can never sample the stamp cell
/// (or vice versa).
#[test]
fn icon_key_never_aliases_solid_stamp() {
    let icon = icon_atlas_key("M9 5l7 7-7 7", 1.9, 17, ICON_VIEWBOX, false);
    let stamp = buiy_core::text::solid_stamp_key();
    assert_ne!(icon, stamp, "icon and solid-stamp keys must be disjoint");
}
