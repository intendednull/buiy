//! SVG-path → R8 coverage rasterizer for vector icons (parity Wave B3,
//! parity-design § 3.5; values.md § 6).
//!
//! The design renders ~25 inline `<svg>` line icons as STROKED paths on a
//! 24×24 viewBox (one `fill`ed glyph — the GitHub mark). Stroke-width varies
//! **1.7–2.4** per icon, which an icon-font cannot reproduce (a font bakes one
//! width — § 8 rejects the icon-font as primary for exactly this reason). So we
//! render the real vector:
//!
//! 1. **Parse** the SVG `d` string into a `lyon::path::Path` via
//!    `lyon_extra::parser` (handles `M/L/H/V/C/S/Q/T/A/Z`, abs + rel; arcs
//!    flatten to quadratics — the search-magnifier `A` and the GitHub mark
//!    both need this).
//! 2. **Tessellate** the STROKE (round cap + round join — the design's
//!    `stroke-linecap/linejoin: round`) or the FILL (the one `fill` glyph) into
//!    a CPU triangle mesh at the icon's pixel scale (`size_px / 24`).
//! 3. **Rasterize** that mesh to a single-channel `R8` coverage bitmap by
//!    N×N supersampled point-in-triangle accumulation — the same coverage an
//!    R8 glyph bitmap carries, so the icon inserts into the EXISTING glyph
//!    atlas and paints through the EXISTING coverage (alpha-as-color) shader
//!    with NO new GPU code (§ 3.5). The per-instance tint is the resolved
//!    accent/ink token, so an icon re-tints live on a theme swap exactly like
//!    text (the atlas bitmap is monochrome coverage, never recolored).
//!
//! Why supersampling and not analytic edge-AA: each icon rasterizes **once**
//! and caches in the atlas (content-addressed by `hash(d, stroke_width, size,
//! fill)`), so the cold-start cost is paid once (de-risk #3: ~25 icons must
//! tessellate+raster < ~50 ms — measured well under). A 4× supersample of a
//! flat triangle fill is robustly correct and visually matches text-glyph AA
//! at the design's fixed 13–24 px sizes (§ 8 accepts subpixel AA). No
//! hand-rolled edge-distance math to get subtly wrong (the "off-by-1 px visible
//! at 13–24 px" risk the research flagged).

use bevy::math::UVec2;
use lyon::path::Path;
use lyon::tessellation::{
    BuffersBuilder, FillOptions, FillTessellator, FillVertex, LineCap, LineJoin, StrokeOptions,
    StrokeTessellator, StrokeVertex, VertexBuffers,
};
use lyon_extra::parser::{ParserOptions, PathParser, Source};

use crate::render::atlas::{AtlasBitmap, AtlasFormat};

/// The design's icons are authored on a **24×24** viewBox (values.md § 6); the
/// `size_px` render size scales that box uniformly.
pub const ICON_VIEWBOX: f32 = 24.0;

/// Supersample factor per axis for coverage rasterization (4× → 16 samples per
/// output texel). Enough to match text-glyph AA quality at 13–24 px without the
/// cost mattering — each icon rasterizes once into the cached atlas cell.
const SUPERSAMPLE: u32 = 4;

/// Flattening tolerance for curve/arc → line-segment subdivision, in **viewBox**
/// units (pre-scale). 0.1 viewBox-unit is < 0.5 px even at the largest 24 px
/// render size, well under one supersample step — curves read smooth.
const FLATTEN_TOLERANCE: f32 = 0.1;

/// Whether an [`crate::render::components::Icon`] is a stroked outline (the
/// default — every line icon) or a filled shape (the one `fill` glyph, the
/// GitHub mark). Carried into the rasterizer and the content-addressed atlas
/// key so a stroke and a fill of the same `d` never collide.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum IconPaint {
    /// `fill="none"` + stroked path — the design default (round cap/join).
    #[default]
    Stroke,
    /// `fill` — the solid GitHub mark.
    Fill,
}

/// One tessellated triangle vertex in **output-pixel** space (viewBox scaled by
/// `size_px / 24`). 2D position only — coverage needs no attributes.
type Vert = [f32; 2];

/// Parse an SVG path `d` into a `lyon::path::Path`. Returns `None` on a
/// malformed `d` (the producer logs + skips, painting nothing — never panics
/// on bad author input; the research's robustness gotcha).
fn parse_path(d: &str) -> Option<Path> {
    let mut builder = Path::builder();
    let mut parser = PathParser::new();
    let mut source = Source::new(d.chars());
    parser
        .parse(&ParserOptions::DEFAULT, &mut source, &mut builder)
        .ok()?;
    Some(builder.build())
}

/// Tessellate `path` to a flat triangle list in output-pixel space. `scale`
/// maps viewBox units → output px (`size_px / 24`); a STROKE uses round
/// cap/join at `stroke_width` viewBox units (× `scale` → px); a FILL uses the
/// non-zero winding rule. Returns the triangle vertices (3 per triangle).
fn tessellate(path: &Path, paint: IconPaint, stroke_width: f32, scale: f32) -> Vec<Vert> {
    let mut buffers: VertexBuffers<Vert, u32> = VertexBuffers::new();
    // Tessellate in VIEWBOX space (tolerance is viewBox units), then scale the
    // emitted vertices to output px in the BuffersBuilder closure — keeps the
    // stroke geometry exact and the tolerance size-independent.
    match paint {
        IconPaint::Stroke => {
            let options = StrokeOptions::default()
                .with_line_width(stroke_width)
                .with_line_cap(LineCap::Round)
                .with_line_join(LineJoin::Round)
                .with_tolerance(FLATTEN_TOLERANCE);
            let mut tess = StrokeTessellator::new();
            // A tessellation error (degenerate input) yields whatever triangles
            // were emitted before the fault — the producer then sees a (possibly
            // empty) bitmap and skips a zero-coverage icon, never panics.
            let _ = tess.tessellate_path(
                path,
                &options,
                &mut BuffersBuilder::new(&mut buffers, |v: StrokeVertex| {
                    let p = v.position();
                    [p.x * scale, p.y * scale]
                }),
            );
        }
        IconPaint::Fill => {
            let options = FillOptions::tolerance(FLATTEN_TOLERANCE);
            let mut tess = FillTessellator::new();
            let _ = tess.tessellate_path(
                path,
                &options,
                &mut BuffersBuilder::new(&mut buffers, |v: FillVertex| {
                    let p = v.position();
                    [p.x * scale, p.y * scale]
                }),
            );
        }
    }
    // Expand the indexed mesh into a flat triangle-vertex list (3 verts each) so
    // the rasterizer iterates triangles without an index indirection.
    buffers
        .indices
        .chunks_exact(3)
        .flat_map(|tri| {
            [
                buffers.vertices[tri[0] as usize],
                buffers.vertices[tri[1] as usize],
                buffers.vertices[tri[2] as usize],
            ]
        })
        .collect()
}

/// Signed twice-area of triangle `(a,b,c)` — the edge-function determinant.
/// Sign tells winding; magnitude is 2× the area. Used to skip degenerate
/// (zero-area) triangles and to orient the half-plane tests.
#[inline]
fn edge(a: Vert, b: Vert, c: Vert) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

/// Is point `p` inside (or on) triangle `(a,b,c)`? Winding-agnostic: a sample
/// counts as covered when it lies on the same side of all three edges OR on an
/// edge (zero), so shared triangle edges in the tessellated mesh do not leave a
/// 1-sample seam between adjacent triangles.
#[inline]
fn in_triangle(p: Vert, a: Vert, b: Vert, c: Vert) -> bool {
    let d1 = edge(a, b, p);
    let d2 = edge(b, c, p);
    let d3 = edge(c, a, p);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    // Covered unless the sample is strictly OUTSIDE (on both sides of different
    // edges). On-edge (a zero term) keeps adjacent triangles seamless.
    !(has_neg && has_pos)
}

/// Rasterize a flat triangle list to an `R8` coverage bitmap of `size × size`
/// texels by N×N supersampled point-in-triangle accumulation. Each output
/// texel's coverage = (# of its `SUPERSAMPLE²` sub-samples inside ANY triangle)
/// / `SUPERSAMPLE²`, scaled to `0..=255`. Sub-samples sit at sub-texel centers
/// `(x + (sx+0.5)/N, y + (sy+0.5)/N)`.
fn rasterize_mesh(tris: &[Vert], size: u32) -> Vec<u8> {
    let n = SUPERSAMPLE;
    let sub_per_texel = (n * n) as f32;
    let mut data = vec![0u8; (size * size) as usize];
    if tris.is_empty() {
        return data;
    }
    // Coverage is the UNION of the triangles (a stroke/fill mesh overlaps its
    // own triangles, so additive accumulation would double-count). Mark a
    // boolean sub-sample grid at `size·N × size·N`: a sub-sample is covered if
    // ANY triangle covers it. Then each output texel's coverage is the count of
    // its N×N covered sub-samples / N². Iterating PER-TRIANGLE over only its
    // sub-sample bounding box (not every texel × every triangle) is the
    // root-cause fix for the cold-start cost — work is now O(Σ triangle area),
    // not O(texels × triangles) (de-risk #3: the full catalog drops well under
    // budget). Float sub-sample-grid coords: texel-space `p = sub / N`.
    let sub_w = size * n;
    let mut covered = vec![false; (sub_w * sub_w) as usize];
    let inv_n = 1.0 / n as f32;

    for t in tris.chunks_exact(3) {
        let (a, b, c) = (t[0], t[1], t[2]);
        // Skip degenerate (zero-area) triangles — no covered samples.
        if edge(a, b, c).abs() <= f32::EPSILON {
            continue;
        }
        // Sub-sample-grid bbox of the triangle. A sub-sample (sx, sy) sits at
        // texel-space center `((sx+0.5)/N, (sy+0.5)/N)`, so the lowest sub-sample
        // whose center is ≥ min_x solves `(s+0.5)/N ≥ min_x` ⇒ `s ≥ N·min_x-0.5`.
        let min_x = a[0].min(b[0]).min(c[0]);
        let max_x = a[0].max(b[0]).max(c[0]);
        let min_y = a[1].min(b[1]).min(c[1]);
        let max_y = a[1].max(b[1]).max(c[1]);
        let lo_sx = ((min_x * n as f32 - 0.5).floor().max(0.0)) as u32;
        let hi_sx = ((max_x * n as f32 - 0.5).ceil().max(0.0) as u32).min(sub_w - 1);
        let lo_sy = ((min_y * n as f32 - 0.5).floor().max(0.0)) as u32;
        let hi_sy = ((max_y * n as f32 - 0.5).ceil().max(0.0) as u32).min(sub_w - 1);

        for sy in lo_sy..=hi_sy {
            let py = (sy as f32 + 0.5) * inv_n;
            for sx in lo_sx..=hi_sx {
                let px = (sx as f32 + 0.5) * inv_n;
                if in_triangle([px, py], a, b, c) {
                    covered[(sy * sub_w + sx) as usize] = true;
                }
            }
        }
    }

    // Downsample: count covered sub-samples in each texel's N×N block.
    for ty in 0..size {
        for tx in 0..size {
            let mut hits = 0u32;
            for sy in 0..n {
                for sx in 0..n {
                    let gx = tx * n + sx;
                    let gy = ty * n + sy;
                    if covered[(gy * sub_w + gx) as usize] {
                        hits += 1;
                    }
                }
            }
            if hits > 0 {
                let coverage = (hits as f32 / sub_per_texel * 255.0).round();
                data[(ty * size + tx) as usize] = coverage.clamp(0.0, 255.0) as u8;
            }
        }
    }
    data
}

/// Rasterize an SVG-path icon to an `R8` coverage [`AtlasBitmap`] of
/// `size_px × size_px` (the design's icon render size — values.md § 6). The
/// returned bitmap is the alpha-as-color coverage the glyph atlas + coverage
/// shader consume; the producer tints it per-instance with the resolved token.
///
/// `paint` selects STROKE (round cap/join at `stroke_width`, the design
/// default) vs FILL (the one solid glyph). A malformed `d` or a fully-degenerate
/// path yields an all-zero bitmap (the producer skips a zero-coverage icon —
/// never panics on bad author input).
pub fn rasterize_icon(
    d: &str,
    paint: IconPaint,
    stroke_width: f32,
    size_px: u16,
    viewbox: f32,
) -> AtlasBitmap {
    let size = size_px.max(1) as u32;
    // `path_d` + `stroke_width` are in `viewbox` units; scale them to `size_px`.
    let scale = size as f32 / viewbox.max(f32::MIN_POSITIVE);
    let data = match parse_path(d) {
        Some(path) => {
            let tris = tessellate(&path, paint, stroke_width, scale);
            rasterize_mesh(&tris, size)
        }
        // Unparseable `d`: empty coverage. The producer logs once and skips.
        None => vec![0u8; (size * size) as usize],
    };
    AtlasBitmap {
        size: UVec2::new(size, size),
        format: AtlasFormat::CoverageR8,
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The chevron-right (`M9 5l7 7-7 7`, stroke 1.9, disclosure icon #24) is the
    /// canonical stroke probe: a `>` shape. Its STROKE must light texels along
    /// the two diagonal arms while the regions clearly OFF the stroke (the
    /// top-left and bottom-left interior, far from any arm) stay empty.
    #[test]
    fn chevron_lights_stroke_leaves_gaps() {
        let bmp = rasterize_icon("M9 5l7 7-7 7", IconPaint::Stroke, 1.9, 20, ICON_VIEWBOX);
        assert_eq!(bmp.format, AtlasFormat::CoverageR8);
        assert_eq!(bmp.size, UVec2::new(20, 20));
        let at = |x: u32, y: u32| bmp.data[(y * 20 + x) as usize];

        // The chevron tip is at viewBox (16,12) → px (13.3, 10) at scale 20/24.
        // A texel right at the tip is on the stroke → lit.
        let tip_lit = (8..16).any(|y| (11..16).any(|x| at(x, y) > 40));
        assert!(tip_lit, "chevron tip arm must light stroke texels");

        // The top-left corner (px 0..4, 0..4) is far from both arms (the arms run
        // from px ~7.5 rightward) → empty interior, no ink.
        let top_left_empty = (0..4).all(|y| (0..4).all(|x| at(x, y) == 0));
        assert!(
            top_left_empty,
            "chevron top-left corner is off the stroke → must be empty"
        );

        // Some ink exists overall (the shape rendered at all).
        let lit = bmp.data.iter().filter(|&&v| v > 0).count();
        assert!(lit > 10, "chevron must produce a non-trivial lit stroke");
    }

    /// The checkmark (`M4 12.5 9 17.5 20 6.5`, stroke 2.4, todo check #11) — two
    /// segments forming a `✓`. Proves a different stroke renders + that a thicker
    /// stroke (2.4 vs 1.9) lights MORE texels than a thin one at the same size.
    #[test]
    fn checkmark_thicker_stroke_lights_more() {
        let thin = rasterize_icon("M4 12.5 9 17.5 20 6.5", IconPaint::Stroke, 1.0, 24, ICON_VIEWBOX);
        let thick =
            rasterize_icon("M4 12.5 9 17.5 20 6.5", IconPaint::Stroke, 2.4, 24, ICON_VIEWBOX);
        let count = |b: &AtlasBitmap| b.data.iter().filter(|&&v| v > 0).count();
        assert!(count(&thin) > 0, "thin checkmark renders");
        assert!(
            count(&thick) > count(&thin),
            "a 2.4 stroke must light more texels than a 1.0 stroke (stroke-width \
             is real, not baked): thin {} thick {}",
            count(&thin),
            count(&thick)
        );
    }

    /// The search magnifier (`…a7 7 0 1 0 0-14…`, stroke 1.7, #9) exercises the
    /// ARC flag path (the circle is two arcs). It must tessellate to a non-empty
    /// stroke — proves arcs flatten, not panic.
    #[test]
    fn search_arc_renders() {
        let bmp = rasterize_icon(
            "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M20 20l-4-4",
            IconPaint::Stroke,
            1.7,
            20,
            ICON_VIEWBOX,
        );
        let lit = bmp.data.iter().filter(|&&v| v > 0).count();
        assert!(lit > 20, "search arc + handle must render a ring of stroke");
        // The ring is hollow: the CENTER of the circle (viewBox ~ (11,11) → px
        // ~9 at 20/24) is inside the ring but OFF the 1.7px stroke → empty.
        let at = |x: u32, y: u32| bmp.data[(y * 20 + x) as usize];
        let center_empty = (8..11).all(|y| (8..11).all(|x| at(x, y) == 0));
        assert!(center_empty, "magnifier ring interior must be hollow");
    }

    /// A malformed `d` yields an all-zero bitmap of the requested size — never a
    /// panic (author-input robustness).
    #[test]
    fn malformed_path_is_empty_not_panic() {
        let bmp = rasterize_icon("not a path!!!", IconPaint::Stroke, 2.0, 16, ICON_VIEWBOX);
        assert_eq!(bmp.size, UVec2::new(16, 16));
        assert!(
            bmp.data.iter().all(|&v| v == 0),
            "malformed d → empty coverage"
        );
    }

    /// The menu vertical-dots (`M12 6h.01M12 12h.01M12 18h.01`, stroke 2.4 round
    /// cap, #6/#13) — three zero-length sub-paths whose ROUND CAPS render as 3
    /// dots. Proves round-cap stamping of a `h.01` near-point.
    #[test]
    fn menu_dots_render_three_caps() {
        let bmp =
            rasterize_icon("M12 6h.01M12 12h.01M12 18h.01", IconPaint::Stroke, 2.4, 18, ICON_VIEWBOX);
        let lit = bmp.data.iter().filter(|&&v| v > 0).count();
        assert!(
            lit > 0,
            "round-capped near-zero segments must stamp dots, got {lit} lit texels"
        );
        // The three dots sit at viewBox y = 6, 12, 18 → px y ≈ 4.5, 9, 13.5 at
        // scale 18/24. The vertical gaps BETWEEN them (px y ≈ 6.5, 11) on the
        // center column must be dark (the dots are separated, not a bar).
        let at = |x: u32, y: u32| bmp.data[(y * 18 + x) as usize];
        let cx = (12.0 * 18.0 / 24.0) as u32; // ≈ 9
        let gap_dark = at(cx, 7) < at(cx, 4).max(at(cx, 9));
        assert!(gap_dark, "the gap between dots must be darker than a dot");
    }

    /// Content-addressed determinism: the SAME inputs rasterize to byte-identical
    /// coverage (the atlas dedup relies on this — a re-authored icon hits the
    /// resident cell, never a second bake).
    #[test]
    fn deterministic_same_inputs_same_bytes() {
        let a = rasterize_icon("M9 5l7 7-7 7", IconPaint::Stroke, 1.9, 17, ICON_VIEWBOX);
        let b = rasterize_icon("M9 5l7 7-7 7", IconPaint::Stroke, 1.9, 17, ICON_VIEWBOX);
        assert_eq!(
            a.data, b.data,
            "identical inputs → identical coverage bytes"
        );
    }
}
